import psutil
import time
import subprocess
import os
import json
import re
import shutil

def check_network():
    """Multi-target network latency test for reliability."""
    targets = ["8.8.8.8", "1.1.1.1", "9.9.9.9"]
    latencies = []
    
    if not shutil.which("ping"):
        return {"status": "error", "message": "Outil 'ping' manquant. Installez 'iputils-ping'."}
        
    for target in targets:
        try:
            start = time.time()
            subprocess.check_output(["ping", "-c", "2", "-W", "2", target], stderr=subprocess.DEVNULL)
            latencies.append((time.time() - start) / 2 * 1000)
        except: continue
    
    if not latencies:
        return {"status": "error", "message": "Aucune cible réseau joignable."}
    
    # Calculate median
    latencies.sort()
    median = latencies[len(latencies)//2]
    
    return {
        "status": "ok",
        "latency": f"{median:.1f}ms",
        "latency_ms": median,
        "targets_tested": len(latencies)
    }

def disk_benchmark(intensity="Quick"):
    """Simple disk R/W benchmark using a temporary file in /tmp."""
    file_path = "/tmp/nemdiag_bench.tmp"
    try:
        if intensity == "Quick": size_mb = 50
        elif intensity == "Standard": size_mb = 250
        else: size_mb = 2000 # Ultra (2GB file)
        
        # Security: Check available space in /tmp
        try:
            _, _, free = shutil.disk_usage("/tmp")
            free_mb = free / (1024 * 1024)
            if free_mb < (size_mb * 1.1): # Keep 10% safety margin
                return {"status": "error", "message": f"Espace insuffisant sur /tmp ({free_mb:.0f}MB libres, {size_mb}MB requis)."}
        except: pass
        
        # Use /tmp to ensure we benchmark local storage
        data = os.urandom(1024 * 1024) # 1MB chunk
        
        # Write
        start_w = time.time()
        with open(file_path, "wb") as f:
            for _ in range(size_mb):
                f.write(data)
        end_w = time.time()
        
        # Read
        start_r = time.time()
        with open(file_path, "rb") as f:
            while f.read(1024 * 1024): pass
        end_r = time.time()
        
        write_speed = size_mb / (end_w - start_w)
        read_speed = size_mb / (end_r - start_r)
        
        return {
            "write_speed": f"{write_speed:.1f} MB/s",
            "read_speed": f"{read_speed:.1f} MB/s",
            "write_speed_raw": write_speed,
            "size": f"{size_mb} MB"
        }
    except Exception as e:
        return {"error": str(e)}
    finally:
        if os.path.exists(file_path):
            try: os.remove(file_path)
            except: pass

def stress_test_cpu(intensity="Quick"):
    """Busy loop to stress CPU while monitoring for thermal throttling."""
    try:
        if intensity == "Quick": duration = 3; threads = psutil.cpu_count()
        elif intensity == "Standard": duration = 15; threads = psutil.cpu_count()
        else: duration = 45; threads = psutil.cpu_count() * 2  # Ultra hyper-threading stress
        
        # Throttling Detection Metrics
        freq_max = psutil.cpu_freq().max if psutil.cpu_freq() else 0
        stats = {"min_freq": 99999, "max_temp": 0}
        monitoring = True
        
        def monitor():
            while monitoring:
                curr_f = psutil.cpu_freq().current if psutil.cpu_freq() else 0
                if curr_f > 0 and curr_f < stats["min_freq"]: stats["min_freq"] = curr_f
                try:
                    temps = psutil.sensors_temperatures()
                    t = 0
                    if 'coretemp' in temps: t = temps['coretemp'][0].current
                    elif 'cpu_thermal' in temps: t = temps['cpu_thermal'][0].current
                    elif 'package id 0' in temps: t = temps['package id 0'][0].current
                    if t > stats["max_temp"]: stats["max_temp"] = t
                except: pass
                time.sleep(0.5)
        
        def burn():
            stop = time.time() + duration
            while time.time() < stop:
                _ = 999999.99**0.5 # Heavy floating point calculation
        
        from threading import Thread
        m_thread = Thread(target=monitor)
        m_thread.start()
        
        ts = [Thread(target=burn) for _ in range(threads)]
        for t in ts: t.start()
        for t in ts: t.join()
        
        monitoring = False
        m_thread.join()
        
        # Throttling Analysis (Drop > 20% while Temp > 80C)
        throttled = False
        if freq_max > 0 and stats["min_freq"] < (0.8 * freq_max) and stats["max_temp"] >= 80:
            throttled = True

        return {
            "status": "ok",
            "message": f"CPU Stress Test ({intensity}) terminé.",
            "threads_used": threads,
            "throttling_detected": throttled,
            "min_freq_mhz": stats["min_freq"] if stats["min_freq"] < 99999 else "N/A",
            "max_temp_c": stats["max_temp"],
            "freq_max_mhz": freq_max
        }
    except Exception as e:
        return {"error": str(e)}

def stress_test_ram(intensity="Quick"):
    """Allocates memory and verifies data integrity with a pattern (0xDEADBEEF)."""
    try:
        if intensity == "Quick": size_gb = 0.5; duration = 3
        elif intensity == "Standard": size_gb = 3.0; duration = 15
        else: 
            avail = psutil.virtual_memory().available / (1024**3)
            size_gb = max(1.0, avail * 0.85)
            duration = 30

        pattern = b"\xDE\xAD\xBE\xEF"
        data = []
        chunk_size = 50 * 1024 * 1024 # 50MB chunks
        num_chunks = int((size_gb * 1024**3) / chunk_size)
        
        # 1. Fill RAM with pattern
        for _ in range(num_chunks):
            chunk = bytearray(pattern * (chunk_size // 4))
            data.append(chunk)

        time.sleep(1) # Wait for potential bit flips

        # 2. Verify Integrity (Sampling)
        stability_failed = False
        for chunk in data:
            # Check start, middle and end for speed
            if chunk[0:4] != pattern or chunk[chunk_size//2 : chunk_size//2+4] != pattern or chunk[-4:] != pattern:
                stability_failed = True
                break
        
        data.clear()
        return {
            "status": "ok",
            "message": f"RAM Stress Test ({intensity}) terminé.",
            "allocated_gb": size_gb,
            "stability_failed": stability_failed
        }
    except Exception as e:
        return {"status": "error", "message": f"Erreur RAM Stress: {str(e)}"}

def calculate_health_score(info, results):
    """
    Returns:
        dict with 'global' score (int 0-100), 'components' (per-component scores),
        and 'warnings' (contextual explanations for low scores).
    """
    components = {"cpu": 100, "ram": 100, "disk": 100, "network": 100, "gpu": 100}
    warnings = []

    try:
        # 1. CPU Analysis
        cpu_usage = info.get('cpu', {}).get('usage', 0)
        top_cpu = info.get('top_cpu', [])
        cpu_offender = top_cpu[0]['name'] if top_cpu else "Inconnu"
        
        if cpu_usage > 95:
            components["cpu"] -= 40
            warnings.append(f"🔴 <b>CPU Critique</b> ({cpu_usage:.0f}%) — Charge extrême détectée. Offenseur : <b>{cpu_offender}</b>.")
        elif cpu_usage > 75:
            components["cpu"] -= 15
            warnings.append(f"🟡 <b>CPU Élevé</b> ({cpu_usage:.0f}%) — Ralentissements possibles. Offenseur : <b>{cpu_offender}</b>.")

        # Support both names for the stress test results
        cpu_res = results.get('cpu', results.get('cpu_stress', {}))
        if isinstance(cpu_res, dict) and cpu_res.get('throttling_detected'):
            components["cpu"] = max(0, components["cpu"] - 25)
            warnings.append("🌡️ <b>CPU THROTTLING</b> : Ralentissement thermique détecté sous charge !")

        # 2. RAM Analysis
        ram_percent = info.get('ram', {}).get('percent', 0)
        top_ram = info.get('top_ram', [])
        ram_offender = top_ram[0]['name'] if top_ram else "Inconnu"

        if ram_percent > 90:
            components["ram"] -= 50
            warnings.append(f"🔴 <b>Saturation RAM</b> ({ram_percent:.0f}%) — Risque de swap/plantage. Offenseur : <b>{ram_offender}</b>.")
        elif ram_percent > 80:
            components["ram"] -= 20
            warnings.append(f"🟡 <b>RAM Tendue</b> ({ram_percent:.0f}%) — Fermez quelques applications. Offenseur : <b>{ram_offender}</b>.")

        # Support both names for the stress test results
        ram_res = results.get('ram', results.get('ram_stress', {}))
        if isinstance(ram_res, dict) and ram_res.get("stability_failed"):
            components["ram"] = max(0, components["ram"] - 70)
            warnings.append("☢️ <b>INSTABILITÉ RAM</b> : Des erreurs d'intégrité ont été détectées (Bit Flips). Votre matériel est peut-être défectueux.")

        # 3. Disk Performance Analysis (SSD vs HDD)
        disk_res = results.get('disk', {})
        if disk_res and 'write_speed_raw' in disk_res:
            ws = disk_res['write_speed_raw']
            is_ssd = any("SSD" in str(d.get("type", "")) for d in info.get("disks", []))
            
            if is_ssd:
                if ws < 80:
                    components["disk"] -= 60
                    warnings.append(f"🔴 <b>SSD Dégradé</b> ({ws:.0f} MB/s) — Performances anormalement basses pour un SSD.")
                elif ws < 250:
                    components["disk"] -= 20
                    warnings.append(f"🟡 <b>SSD Modéré</b> ({ws:.0f} MB/s) — SATA 2 ou disque très rempli.")
            else:
                if ws < 25:
                    components["disk"] -= 40
                    warnings.append(f"🔴 <b>HDD Lent</b> ({ws:.0f} MB/s) — Fragmentation ou usure mécanique.")
                elif ws < 80:
                    components["disk"] -= 5
                    warnings.append(f"🟢 <b>HDD Standard</b> ({ws:.0f} MB/s) — Performances normales.")

        # 4. GPU & Drivers
        gpus = info.get("gpu", [])
        for g in gpus:
            tip = g.get("driver_tip", "")
            if "🔴" in tip: 
                components["gpu"] -= 40
                warnings.append(f"🔴 <b>GPU Alert</b> : {tip}")
            elif "🟡" in tip or "🟠" in tip:
                components["gpu"] -= 15
                warnings.append(f"🟠 <b>GPU Note</b> : {tip}")
        
        gpu_res = results.get('gpu', results.get('gpu_stress', {}))
        if gpu_res and 'estimated_fps' in gpu_res:
            fps = float(gpu_res['estimated_fps'])
            if fps < 10.0:
                components["gpu"] -= 30
                warnings.append(f"🔴 <b>FPS Bas</b> ({fps} FPS) — Accélération matérielle potentiellement manquante.")
            elif fps < 40.0:
                components["gpu"] -= 10
                warnings.append(f"🟡 <b>FPS Moyen</b> ({fps} FPS) — GPU de bureau ou intégré.")

        # 5. Network Latency
        net_res = results.get('network', {})
        if net_res:
            lat = net_res.get('latency_ms', 0)
            if net_res.get('status') == 'error':
                components["network"] = 0
                warnings.append("🔴 <b>Réseau Inaccessible</b> — Impossible de joindre les serveurs de test.")
            elif lat > 800:
                components["network"] -= 50
                warnings.append(f"🔴 <b>Latence Critique</b> ({lat:.0f}ms) — Lag sévère détecté.")
            elif lat > 150:
                components["network"] -= 20
                warnings.append(f"🟡 <b>Latence Instable</b> ({lat:.0f}ms) — Vérifiez votre WiFi ou votre box.")

        # 6. Firmware & SMART
        disks = info.get("disks", [])
        failed_smart = any("FAILED" in str(d.get("smart_health", "")) for d in disks)
        if failed_smart:
            components["disk"] = max(0, components["disk"] - 80)
            warnings.append("☢️ <b>DANGER SMART</b> : Un disque signale une panne imminente !")

        fw_errs = info.get("firmware_errors", [])
        if fw_errs:
            components["cpu"] = max(0, components["cpu"] - 10)
            warnings.append(f"⚠️ {len(fw_errs)} erreurs de firmware trouvées dans les logs système.")

    except Exception as e:
        warnings.append(f"⚠️ Erreur analyse: {str(e)}")

    for k in components: components[k] = max(0, min(100, components[k]))

    weights = {"cpu": 0.3, "ram": 0.3, "disk": 0.2, "gpu": 0.1, "network": 0.1}
    global_score = sum(components[k] * weights[k] for k in weights)

    return {
        "global": int(global_score),
        "components": components,
        "warnings": warnings
    }

def run_step(step_name, intensity="Quick"):
    try:
        if step_name == "network": return check_network()
        elif step_name == "disk": return disk_benchmark(intensity)
        elif step_name == "cpu": return stress_test_cpu(intensity)
        elif step_name == "ram": return stress_test_ram(intensity)
        return {"status": "error", "message": f"Étape {step_name} inconnue."}
    except Exception as e:
        return {"status": "error", "message": str(e)}

def run_diagnostics(intensity="Quick"):
    start = time.time()
    results = {
        "network": run_step("network", intensity),
        "disk": run_step("disk", intensity),
        "cpu_stress": run_step("cpu", intensity),
        "ram_stress": run_step("ram", intensity),
        "duration": f"{time.time() - start:.1f}s"
    }
    return results
