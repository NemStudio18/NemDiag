import json

def diff_hardware(current_info, last_info):
    """Detects changes in CPU, RAM or Storage between two scans."""
    if not last_info: return []
    events = []
    
    # RAM change
    curr_ram = current_info.get('ram', {}).get('total', 'N/A')
    last_ram = last_info.get('ram', {}).get('total', 'N/A')
    if curr_ram != last_ram:
        events.append(f"🔄 **Événement Mémoire** : RAM modifiée ({last_ram} ➔ {curr_ram})")
        
    # CPU change
    curr_cpu = current_info.get('cpu', {}).get('model', 'N/A')
    last_cpu = last_info.get('cpu', {}).get('model', 'N/A')
    if curr_cpu != last_cpu:
        events.append(f"🧠 **Événement Processeur** : Nouveau CPU détecté ({curr_cpu})")
        
    # Disk change (compare counts and types)
    curr_disks = sorted([d.get('name', 'N/A') for d in current_info.get('disks', [])])
    last_disks = sorted([d.get('name', 'N/A') for d in last_info.get('disks', [])])
    if curr_disks != last_disks:
        events.append(f"💿 **Événement Stockage** : Configuration des disques modifiée.")
        
    return events

def analyze_upgrade_potential(results, info):
    """Suggests upgrades based on bottlenecks (Qualitative Analysis)."""
    suggestions = []
    
    # 1. Bottleneck: HDD vs SSD
    is_hdd = False
    for d in info.get('disks', []):
        if "HDD" in str(d.get('type', '')) or d.get('rotational'): is_hdd = True
        
    disk_score = 100
    disk_res = results.get('disk', {})
    if isinstance(disk_res, dict):
        speed = disk_res.get('write_speed_raw', 500)
        if speed < 100: disk_score = 30
        elif speed < 300: disk_score = 60
        
    if is_hdd and disk_score < 70:
        suggestions.append("🚀 **Upgrade Prioritaire** : Votre système utilise un disque HDD mécanique. Passer à un stockage SSD transformerait radicalement la réactivité de ce PC.")
    
    # 2. RAM Constraint
    ram_total_gb = 0
    try:
        ram_str = info.get('ram', {}).get('total', '0GB')
        ram_total_gb = float(ram_str.replace('GB', '').strip().replace(',', '.'))
    except: pass
    
    if ram_total_gb > 0 and ram_total_gb < 8:
        suggestions.append(f"💾 **Mémoire Limitée** : Avec {ram_total_gb} Go de RAM, le multitâche est bridé. Une extension à 8 Go minimum est recommandée.")
        
    # 3. Throttling vs Cooling
    cpu_res = results.get('cpu_stress', {})
    if isinstance(cpu_res, dict) and cpu_res.get('throttling_detected'):
        suggestions.append("🌡️ **Maintenance Thermique** : Le CPU surchauffe et réduit sa vitesse. Un nettoyage des ventilateurs ou un changement de pâte thermique est nécessaire.")

    return suggestions

def watchdog_processes(history):
    """Finds persistent resource-heavy processes over last N scans."""
    if not history or len(history) < 2: return []
    hogs = {}
    
    # Analyze last 5 scans
    for entry in history[:5]:
        data = entry.get('data', {})
        if isinstance(data, str):
            try: data = json.loads(data)
            except: continue
            
        top_cpu = data.get('info', {}).get('top_cpu', [])
        for p in top_cpu[:1]: # Check only the #1 resource hog
            name = p.get('name')
            if name: hogs[name] = hogs.get(name, 0) + 1
            
    alerts = []
    for name, count in hogs.items():
        if count >= 3: # Constant presence in top
            alerts.append(f"🕵️ **Processus Persistant** : '{name}' consomme beaucoup de ressources sur {count} des derniers scans.")
            
    return alerts

def check_thermal_trends(history):
    """Detects rising idle temperatures over time (Dry Paste indicator)."""
    if len(history) < 5: return []
    
    temps = []
    for entry in history[:20]: # Analyze up to last 20 scans
        data = entry.get('data', {})
        if isinstance(data, str):
            try: data = json.loads(data)
            except: continue
            
        t_str = data.get('info', {}).get('cpu', {}).get('temp', '0°C')
        try:
            t_val = float(t_str.replace('°C', '').strip())
            if t_val > 20: # Exclude unrealistic/error values
                temps.append(t_val)
        except: continue
        
    if len(temps) < 6: return []
    
    # Compare latest 3 vs oldest 3 in the sample
    latest_avg = sum(temps[:3]) / 3
    oldest_avg = sum(temps[-3:]) / 3
    
    delta = latest_avg - oldest_avg
    if delta > 8:
        return [f"🌡️ **Dérive Thermique** : Température au repos en hausse (+{delta:.1f}°C). Nettoyage ou changement de pâte thermique recommandé."]
    return []
