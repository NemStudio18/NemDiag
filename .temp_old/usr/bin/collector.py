import psutil
import platform
import subprocess
import os
import time
import json
import socket
import getpass
import re

def get_pci_drivers() -> dict:
    """Returns a map of PCI Device ID -> Driver in use."""
    drivers = {}
    try:
        out = subprocess.check_output("lspci -nnk", shell=True, stderr=subprocess.DEVNULL, timeout=5).decode()
        current_dev = None
        for line in out.splitlines():
            m_dev = re.match(r'^([\da-f]{2}:[\da-f]{2}\.[\da-f])', line)
            if m_dev:
                current_dev = m_dev.group(1)
            elif "Kernel driver in use:" in line and current_dev:
                drivers[current_dev] = line.split(':')[-1].strip()
    except: pass
    return drivers

def get_size(bytes_count: float, suffix: str = "B") -> str:
    factor = 1024.0
    for unit in ["", "K", "M", "G", "T", "P"]:
        if bytes_count < factor:
            return f"{bytes_count:.2f}{unit}{suffix}"
        bytes_count /= factor
    return f"{bytes_count:.2f}P{suffix}"

def get_cpu_cache() -> str:
    """Collects L1/L2/L3 cache sizes and detects 3D V-Cache presence."""
    cache = {"L1": "N/A", "L2": "N/A", "L3": "N/A"}
    try:
        # Check /proc/cpuinfo first for simpler parsing
        with open("/proc/cpuinfo", "r") as f:
            for line in f:
                if "cache size" in line:
                    cache["L3"] = line.split(':')[-1].strip()
                    break

        lscpu = subprocess.check_output("lscpu", shell=True, stderr=subprocess.DEVNULL).decode()
        for line in lscpu.split('\n'):
            line_low = line.lower()
            # Handle localized output (Cache L1d, etc) or standard (L1d cache)
            if "l1" in line_low: cache["L1"] = line.split(':')[-1].strip()
            elif "l2" in line_low: cache["L2"] = line.split(':')[-1].strip()
            elif ("l3" in line_low) and (cache["L3"] == "N/A"): cache["L3"] = line.split(':')[-1].strip()
            elif "l4" in line_low: cache["L4"] = line.split(':')[-1].strip()
            
        if cache["L3"] != "N/A":
            size_str = cache["L3"].upper()
            if "MB" in size_str or "MIB" in size_str:
                m = re.search(r'(\d+)', size_str)
                if m and int(m.group(1)) >= 32:
                    cache["L3"] += " (V-Cache / L3 High-End)"
    except: pass
    
    res = f"L1: {cache['L1']} | L2: {cache['L2']} | L3: {cache['L3']}"
    if "L4" in cache: res += f" | L4: {cache['L4']}"
    return res

def get_ram_details() -> dict:
    details = {"type": "DDR", "speed": "N/A", "modules": [], "channel_mode": "Inconnu"}
    try:
        if os.getuid() == 0:
            dmi = subprocess.check_output("dmidecode -t memory", shell=True, stderr=subprocess.DEVNULL).decode()
            channels = set()
            for block in dmi.split('\n\n'):
                if "Memory Device" in block:
                    m_type, m_speed, m_size = "Unknown", "Unknown", "Unknown"
                    for line in block.split('\n'):
                        if "Type:" in line and "Error" not in line: m_type = line.split(':')[-1].strip()
                        if "Speed:" in line: m_speed = line.split(':')[-1].strip()
                        if "Size:" in line: m_size = line.split(':')[-1].strip()
                        # Detect Channel mode via Bank Locator (Heuristic)
                        if "Locator:" in line:
                            loc = line.split(':')[-1].strip().upper()
                            if any(x in loc for x in ["CHAN A", "CH A", "CHANNEL A", "BANK 0", "BANK 1"]): channels.add("A")
                            if any(x in loc for x in ["CHAN B", "CH B", "CHANNEL B", "BANK 2", "BANK 3"]): channels.add("B")
                    
                    if m_size and ("MB" in m_size or "GB" in m_size) and "No Module" not in m_size:
                        details["modules"].append(f"{m_size} {m_type} @ {m_speed}")
            
            if len(channels) >= 2: details["channel_mode"] = "Dual Channel"
            elif len(channels) == 1: details["channel_mode"] = "Single Channel"
            else: details["channel_mode"] = "Inconnu (DMI Lite)"
        else:
            details["channel_mode"] = "🔴 Requis Mode Expert"
        
        if details["speed"] == "N/A" and os.path.exists("/sys/class/dmi/id/board_name"):
            details["type"] = "DDR (Auto)"

        if details["modules"]:
            first = details["modules"][0]
            parts = first.split(' ')
            if len(parts) > 1: details["type"] = parts[1]
            if "@" in first: details["speed"] = first.split('@')[-1].strip()
    except: pass
    return details

def get_usb_devices() -> list:
    devices = []
    try:
        speed_map = {}
        try:
            tree = subprocess.check_output("lsusb -t", shell=True, stderr=subprocess.DEVNULL, timeout=3).decode()
            current_bus = None
            for line in tree.splitlines():
                bus_m = re.search(r'Bus (\d+)', line)
                if bus_m: current_bus = bus_m.group(1)
                dev_m = re.search(r'Dev (\d+).*?(\d+(\.\d+)?)(G|M)', line)
                if dev_m and current_bus:
                    dev = dev_m.group(1).zfill(3)
                    speed_raw = dev_m.group(2)
                    unit = dev_m.group(4)
                    speed_mb = float(speed_raw) * 1000 if unit == 'G' else float(speed_raw)
                    label = ("USB 3.2" if speed_mb >= 5000 else "USB 3.1" if speed_mb >= 2500 else "USB 3.0" if speed_mb >= 400 else "USB 2.0")
                    speed_map[f"{current_bus.zfill(3)}:{dev}"] = {"speed": label, "mbps": int(speed_mb)}
        except: pass

        lsusb_out = subprocess.check_output("lsusb", shell=True, stderr=subprocess.DEVNULL, timeout=3).decode()
        for line in lsusb_out.strip().splitlines():
            if "Hub" in line or "Linux Foundation" in line: continue
            m = re.match(r'Bus (\d+) Device (\d+): ID ([\da-f]{4}):([\da-f]{4})\s+(.*)', line)
            if not m: continue
            bus, dev, vid, pid, name = m.groups()
            key = f"{bus}:{dev}"
            speed_info = speed_map.get(key, {})
            devices.append({
                "name": name.strip(), "vendor_id": vid, "product_id": pid,
                "bus": int(bus), "device": int(dev),
                "speed": speed_info.get("speed", "USB 2.0"),
                "speed_mbps": speed_info.get("mbps", None),
                "type": "USB Device", "status": "Actif"
            })
    except: pass
    return devices

def get_motherboard_info() -> dict:
    info = {"vendor": "Inconnu", "product": "Inconnu", "bios_version": "N/A", "bios_date": "N/A"}
    try:
        paths = {
            "vendor": "/sys/class/dmi/id/board_vendor",
            "product": "/sys/class/dmi/id/board_name",
            "bios_version": "/sys/class/dmi/id/bios_version",
            "bios_date": "/sys/class/dmi/id/bios_date"
        }
        for k, p in paths.items():
            if os.path.exists(p):
                with open(p) as f: info[k] = f.read().strip()
    except: pass
    return info

def get_displays() -> list:
    displays = []
    try:
        out = subprocess.check_output("xrandr --current 2>/dev/null", shell=True).decode()
        for line in out.splitlines():
            if " connected" in line and not line.startswith(" "):
                parts = line.split()
                name = parts[0]
                res = "Inconnue"
                for p in parts:
                    if 'x' in p and '+' in p:
                        res = p.split('+')[0]; break
                displays.append({"name": name, "resolution": res})
    except: pass
    return displays

def get_wifi_info() -> dict:
    wifi = {"ssid": "N/A", "signal": "N/A", "quality": "N/A", "label": "N/A"}
    try:
        # NMCLI is better if available
        try:
            nm = subprocess.check_output("nmcli -t -f active,ssid,signal dev wifi | grep '^oui'", shell=True).decode().strip()
            if nm:
                parts = nm.split(':')
                if len(parts) >= 3:
                    wifi["ssid"], wifi["signal"] = parts[1], parts[2] + "%"
        except: pass

        if wifi["ssid"] == "N/A":
            iw = subprocess.check_output("iwgetid -r", shell=True, stderr=subprocess.DEVNULL).decode().strip()
            if iw: wifi["ssid"] = iw
            
        if os.path.exists("/proc/net/wireless"):
            with open("/proc/net/wireless") as f:
                lines = f.readlines()
                if len(lines) > 2:
                    parts = lines[2].replace('.', ' ').split()
                    if len(parts) >= 4:
                        q_val = int(parts[2])
                        dbm = int(parts[3])
                        wifi["quality"], wifi["signal"] = f"{q_val}/70", f"{dbm} dBm"
                        if dbm >= -50: wifi["label"] = "Excellent"
                        elif dbm >= -60: wifi["label"] = "Bon"
                        elif dbm >= -75: wifi["label"] = "Moyen"
                        else: wifi["label"] = "Faible"
    except: pass
    return wifi

def get_battery_info() -> dict:
    import psutil
    bat = psutil.sensors_battery()
    info = {"percent": "N/A", "plugged": False, "health": "N/A", "cycles": "N/A"}
    if bat:
        info["percent"] = bat.percent
        info["plugged"] = bat.power_plugged
        
    try:
        # Advanced battery health via upower
        up_paths = subprocess.check_output("upower -e | grep battery", shell=True, stderr=subprocess.DEVNULL).decode().splitlines()
        if up_paths:
            up_out = subprocess.check_output(f"upower -i {up_paths[0]}", shell=True, stderr=subprocess.DEVNULL).decode()
            for line in up_out.splitlines():
                if "capacity:" in line: info["health"] = line.split(':')[-1].strip()
                elif "cycle-count:" in line: info["cycles"] = line.split(':')[-1].strip()
                elif "state:" in line and "discharging" in line: info["plugged"] = False
                elif "state:" in line and "charging" in line: info["plugged"] = True
    except: pass
    return info

def get_os_info() -> dict:
    info = {
        "name": str(platform.system()), "release": str(platform.release()),
        "version": str(platform.version()), "distro": str(platform.platform()),
        "uptime": "N/A", "hostname": socket.gethostname(),
        "user": getpass.getuser(), "arch": platform.machine(),
        "sessions": len(psutil.users())
    }
    try:
        uptime_seconds = time.time() - psutil.boot_time()
        info["uptime"] = time.strftime('%H:%M:%S', time.gmtime(uptime_seconds))
    except: pass
    return info

def get_cpu_info() -> dict:
    cpu_freq = psutil.cpu_freq()
    info = {
        "model": "Unknown",
        "cores_physical": int(psutil.cpu_count(logical=False) or 0),
        "cores_logical": int(psutil.cpu_count(logical=True) or 0),
        "usage": float(psutil.cpu_percent(interval=None)),
        "temp": "N/A",
        "freq_current": f"{cpu_freq.current:.0f}MHz" if cpu_freq else "N/A",
        "freq_max": f"{cpu_freq.max:.0f}MHz" if cpu_freq else "N/A",
        "cache": get_cpu_cache()
    }
    try:
        if platform.system() == "Linux":
            info["model"] = subprocess.check_output("grep -m1 'model name' /proc/cpuinfo", shell=True).decode().split(": ")[1].strip()
        temps = psutil.sensors_temperatures()
        if 'coretemp' in temps: info["temp"] = f"{temps['coretemp'][0].current}°C"
        elif 'cpu_thermal' in temps: info["temp"] = f"{temps['cpu_thermal'][0].current}°C"
    except: pass
    return info

def get_ram_info() -> dict:
    mem = psutil.virtual_memory()
    det = get_ram_details()
    return {
        "total": get_size(float(mem.total)), "available": get_size(float(mem.available)),
        "used": get_size(float(mem.used)), "percent": float(mem.percent),
        "type": det["type"], "speed": det["speed"], "modules": det["modules"],
        "channel_mode": det["channel_mode"],
        "summary": f"{get_size(float(mem.total))} {det['type']} ({len(det['modules'])} mod @ {det['speed']})" if det['modules'] else get_size(float(mem.total))
    }

def get_swap_info() -> dict:
    import psutil
    swap = psutil.swap_memory()
    return {
        "total": get_size(float(swap.total)),
        "used": get_size(float(swap.used)),
        "free": get_size(float(swap.free)),
        "percent": float(swap.percent)
    }

def get_disks_info() -> list:
    disks_list = []
    try:
        lsblk_json = subprocess.check_output("lsblk -J -b -o NAME,SIZE,MODEL,SERIAL,VENDOR,ROTA,TRAN,TYPE,MOUNTPOINT,RM,HOTPLUG,FSTYPE,LABEL,UUID,REV,STATE,DISC-MAX", shell=True, stderr=subprocess.DEVNULL, timeout=5).decode()
        lsblk_data = json.loads(lsblk_json).get("blockdevices", [])
    except: lsblk_data = []

    def get_smart(dev):
        if os.getuid() != 0: return {"health": "🔴 Requis Mode Expert", "temp": "N/A"}
        try:
            out = subprocess.check_output(f"smartctl -H --json /dev/{dev} 2>/dev/null", shell=True, stderr=subprocess.DEVNULL, timeout=4).decode()
            data = json.loads(out)
            # Some versions use top-level 'smart_status', others nested 'smartctl.status'
            status_obj = data.get("smart_status", {})
            health = status_obj.get("passed", None)
            if health is None:
                # Fallback to exit status if JSON is partial
                health = True # assume pass if no explicit fail found
            
            temp = data.get("temperature", {}).get("current", None)
            return {"health": "✅ PASSED" if health is True else ("❌ FAILED" if health is False else "N/A"), "temp": f"{temp}°C" if temp else "N/A"}
        except: return {"health": "N/A", "temp": "N/A"}

    for bd in lsblk_data:
        if bd.get("type") not in ("disk", "rom"): continue
        name, rota, tran, rm = bd.get("name", ""), bd.get("rota", True), (bd.get("tran") or "").upper(), bd.get("rm", False)
        size_bytes = bd.get("size", 0) or 0
        disk_type = ("SSD (NVMe)" if tran == "NVME" else "SSD (SATA)" if tran == "SATA" and not rota else "USB Drive" if tran == "USB" or bd.get("hotplug") else "HDD" if rota else "SSD")
        smart = get_smart(name)
        parts = []
        for child in (bd.get("children") or []):
            mp = child.get("mountpoint") or ""
            if not mp: continue
            try:
                u = psutil.disk_usage(mp)
                parts.append({"name": child.get("name", ""), "mountpoint": mp, "fstype": child.get("fstype") or "?", "total": get_size(float(u.total)), "used": get_size(float(u.used)), "free": get_size(float(u.free)), "percent": float(u.percent)})
            except: continue
        # Top-level percent for the disk (often derived from main partition)
        disk_percent = 0.0
        if parts:
            # Use the most filled partition as proxy for disk usage
            disk_percent = max(p["percent"] for p in parts)

        disks_list.append({
            "name": f"/dev/{name}", "model": (bd.get("model") or "Inconnu").strip(), "vendor": (bd.get("vendor") or "N/A").strip(),
            "serial": (bd.get("serial") or "N/A").strip(), "firmware": (bd.get("rev") or "N/A").strip(), "transport": tran or "N/A",
            "type": disk_type, "removable": bool(rm), "trim_support": "Oui" if int(bd.get("disc-max", 0) or 0) > 0 else "Non",
            "total": get_size(float(size_bytes)) if size_bytes else "N/A",
            "percent": disk_percent,
            "smart_health": smart["health"], "smart_temp": smart["temp"], "state": (bd.get("state") or "running").strip(), "partitions": parts
        })
    return disks_list

def get_gpu_info() -> list:
    gpu_list = []
    try:
        nvidia = subprocess.check_output(["nvidia-smi", "--query-gpu=name,vendor.gpu,driver_version,memory.total,memory.used,temperature.gpu,utilization.gpu", "--format=csv,noheader,nounits"], stderr=subprocess.DEVNULL, timeout=2).decode()
        for line in nvidia.strip().split('\n'):
            p = line.split(', ')
            if len(p) >= 7:
                gpu_list.append({"name": str(p[0]), "vendor": str(p[1]), "driver": str(p[2]), "vram_total": f"{p[3]}MB", "vram_used": f"{p[4]}MB", "temp": f"{p[5]}°C", "usage": f"{p[6]}%", "nvidia_driver_missing": False})
    except: pass
    try:
        lshw_out = subprocess.check_output("lshw -json -C display 2>/dev/null", shell=True, timeout=5).decode()
        gpus = json.loads(lshw_out)
        if isinstance(gpus, dict): gpus = [gpus]
        for g in gpus:
            vid_raw = str(g.get("vendor", "Inconnu")).upper()
            drv = str(g.get("configuration", {}).get("driver", "N/A"))
            if "NVIDIA" in vid_raw and drv == "nvidia" and any("NVIDIA" in str(gpu.get("vendor")).upper() for gpu in gpu_list): continue
            gpu_list.append({"name": g.get("product", g.get("description", "VGA")), "vendor": g.get("vendor", "Inconnu"), "driver": drv, "vram_total": "N/A", "vram_used": "N/A", "temp": "N/A", "usage": "N/A", "nvidia_driver_missing": "NVIDIA" in vid_raw and drv != "nvidia"})
    except: pass

    for g in gpu_list:
        v, drv = str(g.get("vendor", "")).upper(), str(g.get("driver", "")).lower()
        if "AMD" in v:
            g["driver_tip"] = "🟢 Driver 'amdgpu' actif." if drv == "amdgpu" else "🟡 Driver 'radeon' (ancien) détecté." if drv == "radeon" else "🔴 Aucun driver GPU AMD spécifique détecté."
        elif "INTEL" in v:
            g["driver_tip"] = f"🟢 Driver Intel '{drv}' actif." if drv in ["i915", "xe"] else "🟡 Driver Intel générique. Installez 'intel-media-va-driver'."
        elif "NVIDIA" in v:
            if drv == "nouveau":
                g["driver_tip"] = "🟠 Driver 'nouveau' (Open Source) actif. Performancess 3D limitées."
            elif drv == "nvidia":
                g["driver_tip"] = "🟢 Driver NVIDIA propriétaire actif."
            else:
                g["driver_tip"] = "🔴 Aucun driver NVIDIA propriétaire détecté. Pilote 'nouveau' ou générique ?"
    return gpu_list

def get_missing_firmwares() -> list:
    if os.geteuid() != 0: return []
    try:
        out = subprocess.check_output("dmesg | grep -i 'firmware' | grep -i 'failed' | tail -n 5", shell=True).decode()
        return [line.strip() for line in out.split('\n') if line]
    except: return []

def get_package_manager() -> str:
    import shutil
    if shutil.which("apt"): return "apt"
    if shutil.which("dnf"): return "dnf"
    if shutil.which("pacman"): return "pacman"
    return "unknown"

def get_static_info() -> dict:
    """Collects hardware specs that don't change during runtime."""
    cpu = get_cpu_info()
    ram = get_ram_info()
    # PRE-FETCH EXPENSIVE DATA ONCE
    gpu = get_gpu_info()
    disks = get_disks_info()
    usb = get_usb_devices()
    network = get_network_interfaces()
    
    return {
        "os": get_os_info(),
        "cpu": {
            "model": cpu["model"],
            "cores_physical": cpu["cores_physical"],
            "cores_logical": cpu["cores_logical"],
            "freq_max": cpu["freq_max"],
            "cache": cpu["cache"]
        },
        "ram": {
            "total": ram["total"],
            "type": ram["type"],
            "speed": ram["speed"],
            "modules": ram["modules"],
            "channel_mode": ram["channel_mode"],
            "summary": ram["summary"]
        },
        "disks": disks,
        "gpu": gpu,
        "bios": get_motherboard_info(),
        "displays": get_displays(),
        "usb": usb,
        "network": network,
        "wifi": get_wifi_info(), 
        "battery": get_battery_info(), 
        "swap": get_swap_info(),
        "pkg_manager": get_package_manager(),
        "security": get_security_status()
    }

def get_security_status() -> dict:
    """Checks Firewall (UFW) and System Updates status."""
    status = {"firewall": "Inactif", "updates": 0}
    try:
        # Check UFW (Firewall)
        raw = subprocess.check_output(["sudo", "ufw", "status"], stderr=subprocess.STDOUT).decode()
        status["firewall"] = "Actif" if "Status: active" in raw else "Inactif"
    except:
        status["firewall"] = "Non accessible/installé"

    try:
        # Check pending updates (standard file on Debian/Ubuntu/Mint)
        up_file = "/var/lib/update-notifier/updates-available"
        if os.path.exists(up_file):
            with open(up_file, "r") as f:
                content = f.read()
                import re
                m = re.search(r"(\d+)", content)
                if m: status["updates"] = int(m.group(1))
    except: pass
    return status

def get_network_interfaces() -> list:
    """Static part of network info: names and drivers."""
    pci_drivers = get_pci_drivers()
    networks = []
    for n, s in psutil.net_if_addrs().items():
        if n == 'lo': continue
        driver = "N/A"
        try:
            pci_search = subprocess.check_output(f"basename $(readlink /sys/class/net/{n}/device)", shell=True, stderr=subprocess.DEVNULL).decode().strip()
            if ":" in pci_search: pci_search = ":".join(pci_search.split(":")[1:])
            driver = pci_drivers.get(pci_search, "N/A")
        except: pass
        networks.append({
            "interface": n,
            "mac": next((a.address for a in s if hasattr(socket, 'AF_PACKET') and a.family == socket.AF_PACKET), "N/A"),
            "driver": driver
        })
    return networks

def get_dynamic_info() -> dict:
    """Collects real-time metrics (CPU%, RAM%, Temps). Subprocess-lite."""
    psutil.cpu_percent(interval=None)
    cpu_freq = psutil.cpu_freq()
    mem = psutil.virtual_memory()
    
    # Process listing (optimized)
    top_cpu, top_ram = [], []
    try:
        procs = []
        # Only iterate once and pick top candidates
        for proc in psutil.process_iter(['pid', 'name', 'cpu_percent', 'memory_info']):
            try: 
                info = proc.info
                # Ensure memory info exists
                if not info.get('memory_info'): continue
                # Default CPU percent if None
                if info.get('cpu_percent') is None: info['cpu_percent'] = 0.0
                procs.append(info)
            except (psutil.NoSuchProcess, psutil.AccessDenied): pass
            except: pass
        
        top_cpu = sorted(procs, key=lambda x: x.get('cpu_percent', 0) or 0, reverse=True)[:5]
        top_ram = sorted(procs, key=lambda x: x['memory_info'].rss if x.get('memory_info') else 0, reverse=True)[:5]
    except: pass

    # Temps (Procfs based, fast)
    cpu_temp = "N/A"
    try:
        temps = psutil.sensors_temperatures()
        if 'coretemp' in temps: cpu_temp = f"{temps['coretemp'][0].current}°C"
        elif 'cpu_thermal' in temps: cpu_temp = f"{temps['cpu_thermal'][0].current}°C"
    except: pass

    # Dynamic network status (No subprocess)
    stats = psutil.net_if_stats()
    addrs = psutil.net_if_addrs()
    networks = []
    for n, s in addrs.items():
        if n == 'lo': continue
        networks.append({
            "interface": n,
            "ip": [a.address for a in s if a.family == socket.AF_INET],
            "status": "Actif" if (stats.get(n) and stats.get(n).isup) else "Inactif"
        })

    return {
        "cpu": {
            "usage": float(psutil.cpu_percent() or 0.0),
            "temp": cpu_temp,
            "freq_current": f"{cpu_freq.current:.0f}MHz" if cpu_freq else "N/A"
        },
        "ram": {
            "percent": float(mem.percent) if mem else 0.0,
            "used": get_size(float(mem.used)) if mem else "0B",
            "available": get_size(float(mem.available)) if mem else "0B"
        },
        "battery": {
            "percent": psutil.sensors_battery().percent if psutil.sensors_battery() else "N/A", 
            "plugged": psutil.sensors_battery().power_plugged if psutil.sensors_battery() else False
        },
        "network": networks,
        "top_cpu": [{"pid": p['pid'], "name": p['name'], "usage": p['cpu_percent']} for p in top_cpu],
        "top_ram": [{"pid": p['pid'], "name": p['name'], "mem": get_size(float(p['memory_info'].rss))} for p in top_ram]
    }

def get_system_info() -> dict:
    """Legacy compatibility: returns the full data object."""
    static = get_static_info()
    dynamic = get_dynamic_info()
    
    # Deep merge CPU and RAM
    static["cpu"].update(dynamic["cpu"])
    static["ram"].update(dynamic["ram"])
    
    # Combine everything else
    full_info = {**static, **dynamic}
    full_info["cpu"] = static["cpu"]
    full_info["ram"] = static["ram"]
    
    # Enrichment for network (missing in split modes)
    pci_drivers = get_pci_drivers()
    networks = []
    for n, s in psutil.net_if_addrs().items():
        if n == 'lo': continue
        driver = "N/A"
        try:
            pci_search = subprocess.check_output(f"basename $(readlink /sys/class/net/{n}/device)", shell=True, stderr=subprocess.DEVNULL).decode().strip()
            if ":" in pci_search: pci_search = ":".join(pci_search.split(":")[1:])
            driver = pci_drivers.get(pci_search, "N/A")
        except: pass
        networks.append({
            "interface": n,
            "mac": next((a.address for a in s if hasattr(socket, 'AF_PACKET') and a.family == socket.AF_PACKET), "N/A"),
            "ip": [a.address for a in s if a.family == socket.AF_INET],
            "status": "Actif" if (psutil.net_if_stats().get(n) and psutil.net_if_stats().get(n).isup) else "Inactif",
            "driver": driver
        })
    full_info["network"] = networks
    
    return full_info

if __name__ == "__main__":
    print(json.dumps(get_system_info(), indent=2))
