use sysinfo::{System, Components};

pub struct HardwareInfo {
    pub os_name: String,
    pub kernel_version: String,
    pub host_name: String,
    pub cpu_name: String,
    pub core_count: usize,
    pub memory_total: u64,
    pub memory_used: u64,
}

#[derive(serde::Serialize)]
pub struct RealtimeInfo {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub temperatures: Vec<(String, f32)>,
    pub fan_speeds: Vec<(String, u32)>,
}

pub struct HardwareMonitor {
    sys: System,
    components: Components,
}

impl HardwareMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys,
            components: Components::new_with_refreshed_list(),
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
        self.components.refresh(true);
    }

    pub fn get_static_info(&self) -> HardwareInfo {
        let os_name = System::long_os_version().unwrap_or_else(|| System::name().unwrap_or_else(|| "Unknown".to_string()));
        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let host_name = System::host_name().unwrap_or_else(|| "Unknown".to_string());
        let cpus = self.sys.cpus();
        let cpu_name = cpus.first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Unknown CPU".to_string());
        let core_count = cpus.len();

        HardwareInfo {
            os_name,
            kernel_version,
            host_name,
            cpu_name,
            core_count,
            memory_total: self.sys.total_memory(),
            memory_used: self.sys.used_memory(),
        }
    }

    pub fn get_cpu_usage(&self) -> f32 {
        self.sys.global_cpu_usage()
    }
    pub fn get_temperatures(&self) -> Vec<(String, f32)> {
        self.components
            .iter()
            .filter_map(|c| {
                let t = c.temperature().unwrap_or(0.0);
                // Filter out aberrant values (e.g. millidegrees returned as degrees)
                if t > 0.5 && t < 120.0 {
                    Some((c.label().to_string(), t))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_fan_speeds(&self) -> Vec<(String, u32)> {
        let mut fans = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = std::fs::read_to_string(path.join("name")).unwrap_or_default().trim().to_string();
                if let Ok(files) = std::fs::read_dir(&path) {
                    for f in files.flatten() {
                        let fname = f.file_name().to_string_lossy().to_string();
                        if fname.starts_with("fan") && fname.ends_with("_input") {
                            if let Ok(val_str) = std::fs::read_to_string(f.path()) {
                                if let Ok(rpm) = val_str.trim().parse::<u32>() {
                                    if rpm > 0 {
                                        // Attempt to get label if exists
                                        let label_file = fname.replace("_input", "_label");
                                        let label = std::fs::read_to_string(path.join(label_file))
                                            .unwrap_or_else(|_| format!("{} - {}", name, fname));
                                        fans.push((label.trim().to_string(), rpm));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        fans
    }
}

use nvml_wrapper::Nvml;

pub fn get_nvml_info() -> Vec<(String, u32, u32)> {
    let mut nv_info = Vec::new();
    match Nvml::init() {
        Ok(nvml) => {
            if let Ok(count) = nvml.device_count() {
                for i in 0..count {
                    if let Ok(device) = nvml.device_by_index(i) {
                        let name = device.name().unwrap_or_else(|_| "Unknown NVIDIA GPU".to_string());
                        let temp = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).unwrap_or(0);
                        let util = device.utilization_rates().map(|u| u.gpu).unwrap_or(0);
                        nv_info.push((name, temp, util));
                    }
                }
            }
        },
        Err(_) => {}
    }
    nv_info
}
#[derive(serde::Serialize, Default)]
pub struct DetailedSystemInfo {
    pub system_details: String,
    pub bios_details: String,
    pub motherboard: String,
    pub cpu_details: String,
    pub ram_details: String,
    pub disks_details: String,
    pub usb_details: String,
    pub gpu_details: String,
    pub battery_details: String,
    pub display_details: String,
    pub network_details: String,
    pub wifi_details: String,
}

pub fn gather_detailed_info_linux() -> Result<DetailedSystemInfo, String> {
    use std::fs;
    use std::process::Command;

    // Lecture directe sysfs/proc — pas besoin de root
    let read_dmi = |field: &str| -> String {
        fs::read_to_string(format!("/sys/class/dmi/id/{}", field))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "Non disponible".to_string())
    };

    // Infos système
    let product_name   = read_dmi("product_name");
    let product_version = read_dmi("product_version");
    let sys_vendor     = read_dmi("sys_vendor");
    let system_details = format!(
        "Fabricant : {}\nProduit   : {} {}\n",
        sys_vendor, product_name, product_version
    );

    // BIOS
    let bios_vendor  = read_dmi("bios_vendor");
    let bios_version = read_dmi("bios_version");
    let bios_date    = read_dmi("bios_date");
    let bios_details = format!(
        "Fabriquant : {}\nVersion    : {}\nDate       : {}\n",
        bios_vendor, bios_version, bios_date
    );

    // Carte mère
    let board_vendor  = read_dmi("board_vendor");
    let board_name    = read_dmi("board_name");
    let board_version = read_dmi("board_version");
    let motherboard = format!(
        "Fabricant : {}\nModèle    : {}\nVersion   : {}\n",
        board_vendor, board_name, board_version
    );

    // CPU — lscpu
    let cpu_details = Command::new("lscpu")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|_| "lscpu non disponible".to_string());

    // RAM — /proc/meminfo + dmidecode (pour Dual Channel)
    let mut ram_details = String::new();
    let dmi_ram = Command::new("pkexec")
        .args(["dmidecode", "-t", "memory"])
        .output();
    if let Ok(out) = dmi_ram {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut installed_sticks = 0;
        let mut speeds = Vec::new();
        let mut types = Vec::new();
        for line in stdout.lines() {
            if line.contains("Size:") && !line.contains("No Module Installed") {
                installed_sticks += 1;
            }
            if line.contains("Speed:") && !line.contains("Unknown") {
                speeds.push(line.trim().replace("Speed: ", ""));
            }
            if line.contains("Type:") && !line.contains("Unknown") && !line.contains("Error") {
                let t = line.trim().replace("Type: ", "");
                if !types.contains(&t) { types.push(t); }
            }
        }
        if installed_sticks > 0 {
            ram_details.push_str(&format!(
                "--- Mémoire Physique ---\nBarrettes : {}\nMode probable : {}\nType : {}\nFréquences : {:?}\n\n",
                installed_sticks,
                if installed_sticks == 1 { "Single-Channel" } else if installed_sticks == 2 { "Dual-Channel" } else { "Quad-Channel" },
                types.join(", "),
                speeds
            ));
        }
    }
    ram_details.push_str(&fs::read_to_string("/proc/meminfo").unwrap_or_else(|_| "Non disponible".to_string()));

    // Disques — lsblk + SMART
    let mut disks_details = Command::new("lsblk")
        .args(["-o", "NAME,SIZE,FSTYPE,TYPE,MOUNTPOINT,MODEL,ROTA"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|_| "lsblk non disponible".to_string());

    disks_details.push_str("\n--- Santé S.M.A.R.T ---\n");
    let smart_cmd = Command::new("pkexec")
        .args(["bash", "-c", "for dev in $(lsblk -d -n -o NAME | grep -v loop); do echo \"/dev/$dev:\"; smartctl -H /dev/$dev | grep -E -i '(test result|health)'; echo \"\"; done"])
        .output();
    
    if let Ok(out) = smart_cmd {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.trim().is_empty() {
            disks_details.push_str("Aucune donnée SMART ou smartmontools non installé.\n");
        } else {
            disks_details.push_str(&stdout);
        }
    } else {
        disks_details.push_str("Impossible d'exécuter smartctl.\n");
    }

    // USB — lsusb + lsusb -t
    let mut usb_details = Command::new("lsusb")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|_| "lsusb non disponible".to_string());
    
    if let Ok(out) = Command::new("lsusb").arg("-t").output() {
        usb_details.push_str("\n--- Topologie et Vitesses USB ---\n");
        usb_details.push_str(&String::from_utf8_lossy(&out.stdout));
    }

    // GPU — lspci
    let gpu_details = Command::new("lspci")
        .args(["-vmm"])
        .output()
        .map(|o| {
            let raw = String::from_utf8_lossy(&o.stdout);
            raw.lines()
                .collect::<Vec<_>>()
                .split(|l| l.is_empty())
                .filter(|block| block.iter().any(|l| {
                    let lo = l.to_lowercase();
                    lo.contains("vga") || lo.contains("3d") || lo.contains("display")
                        || lo.contains("nvidia") || lo.contains("amd")
                }))
                .map(|b| b.join("\n"))
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_else(|_| "lspci non disponible".to_string());

    // Batterie
    let battery_details = std::fs::read_dir("/sys/class/power_supply")
        .ok()
        .and_then(|mut d| d.find(|e| {
            e.as_ref().ok().map_or(false, |e| {
                e.file_name().to_string_lossy().starts_with("BAT")
            })
        }))
        .and_then(|e| e.ok())
        .map(|e| {
            let base = e.path();
            let cap  = fs::read_to_string(base.join("capacity")).unwrap_or_default().trim().to_string();
            let stat = fs::read_to_string(base.join("status")).unwrap_or_default().trim().to_string();
            format!("Capacité : {}%\nÉtat : {}\n", cap, stat)
        })
        .unwrap_or_else(|| "Aucune batterie détectée".to_string());

    // Affichage — xrandr si disponible
    let display_details = Command::new("xrandr")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.contains(" connected") || l.contains('*'))
            .collect::<Vec<_>>()
            .join("\n"))
        .unwrap_or_else(|_| "xrandr non disponible".to_string());

    let mut network_details = String::new();
    if let Ok(out) = Command::new("ip").args(["-br", "addr"]).output() {
        network_details = String::from_utf8_lossy(&out.stdout).to_string();
    }

    let mut wifi_details = String::new();
    if let Ok(out) = Command::new("nmcli").args(["-t", "-f", "active,ssid,signal", "dev", "wifi"]).output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if line.starts_with("oui:") || line.starts_with("yes:") {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    wifi_details = format!("SSID: {}, Signal: {}%", parts[1], parts[2]);
                }
                break;
            }
        }
    }
    if wifi_details.is_empty() {
        if let Ok(wireless_raw) = fs::read_to_string("/proc/net/wireless") {
            let lines: Vec<&str> = wireless_raw.lines().collect();
            if lines.len() > 2 {
                let parts: Vec<&str> = lines[2].split_whitespace().collect();
                if parts.len() > 3 {
                    let mut signal_dbm = parts[3].trim_end_matches('.').to_string();
                    if !signal_dbm.starts_with('-') {
                        signal_dbm = format!("-{}", signal_dbm); // Sometimes missing negative sign
                    }
                    wifi_details = format!("Connecté, Signal: {} dBm", signal_dbm);
                }
            }
        }
    }
    if wifi_details.is_empty() {
        wifi_details = "Non connecté au Wi-Fi / Non détecté".to_string();
    }

    Ok(DetailedSystemInfo {
        system_details,
        bios_details,
        motherboard,
        cpu_details,
        ram_details,
        disks_details,
        usb_details,
        gpu_details,
        battery_details,
        display_details,
        network_details,
        wifi_details,
    })
}
