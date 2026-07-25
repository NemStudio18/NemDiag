use sysinfo::{System, Disks, Networks, Components};
use std::process::Command;

pub struct HardwareInfo {
    pub os_name: String,
    pub kernel_version: String,
    pub host_name: String,
    pub cpu_name: String,
    pub core_count: usize,
    pub memory_total: u64,
    pub memory_used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(serde::Serialize)]
pub struct RealtimeInfo {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub temperatures: Vec<(String, f32)>,
}

pub struct HardwareMonitor {
    sys: System,
    disks: Disks,
    networks: Networks,
    components: Components,
}

impl HardwareMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
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
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
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
}

pub fn get_baseboard_info_sudo() -> Result<String, String> {
    // This function will use pkexec to run dmidecode
    // Note: We use dmidecode for baseboard as it's the standard on Linux.
    let output = Command::new("pkexec")
        .arg("dmidecode")
        .arg("-t")
        .arg("baseboard")
        .output()
        .map_err(|e| format!("Failed to execute pkexec: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

pub fn get_smart_info(disk_path: &str) -> Result<String, String> {
    let output = Command::new("pkexec")
        .arg("smartctl")
        .arg("-H")
        .arg("-A")
        .arg(disk_path)
        .output()
        .map_err(|e| format!("Failed to execute pkexec smartctl: {}", e))?;

    // smartctl returns bitmask exit statuses, so we can't strictly check for success
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
}

pub fn gather_detailed_info_linux() -> Result<DetailedSystemInfo, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // Security fix (T4): pass script via stdin instead of writing to /tmp
    // This eliminates the TOCTOU race condition vulnerability.
    let script = r#"
echo "{"
echo "\"system\": \"$(dmidecode -t system 2>/dev/null | base64 -w 0)\","
echo "\"bios\": \"$(dmidecode -t bios 2>/dev/null | base64 -w 0)\","
echo "\"motherboard\": \"$(dmidecode -t baseboard 2>/dev/null | base64 -w 0)\","
echo "\"memory\": \"$(dmidecode -t memory 2>/dev/null | base64 -w 0)\","
echo "\"cpu\": \"$(lscpu 2>/dev/null | base64 -w 0)\","
echo "\"disks\": \"$(lsblk -J -o NAME,SIZE,FSTYPE,TYPE,MOUNTPOINT,MODEL,ROTA 2>/dev/null | base64 -w 0)\","
echo "\"usb\": \"$(lsusb 2>/dev/null | base64 -w 0)\","
echo "\"gpu\": \"$(lspci -vnn 2>/dev/null | grep -i -A 12 'vga\|3d\|display' | base64 -w 0)\","
echo "\"battery\": \"$(upower -i $(upower -e 2>/dev/null | grep 'BAT' | head -n 1) 2>/dev/null | base64 -w 0)\","
echo "\"display\": \"$(xrandr 2>/dev/null | grep -E ' connected|[*]' | base64 -w 0)\""
echo "}"
"#;

    let mut child = Command::new("pkexec")
        .arg("sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn pkexec: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(script.as_bytes()).map_err(|e| e.to_string())?;
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to wait for script: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    
    let decode = |v: &serde_json::Value, key: &str| -> String {
        v.get(key)
            .and_then(|v| v.as_str())
            .and_then(|s| STANDARD.decode(s).ok())
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|| "Information non disponible".to_string())
    };

    Ok(DetailedSystemInfo {
        system_details: decode(&parsed, "system"),
        bios_details: decode(&parsed, "bios"),
        motherboard: decode(&parsed, "motherboard"),
        cpu_details: decode(&parsed, "cpu"),
        ram_details: decode(&parsed, "memory"),
        disks_details: decode(&parsed, "disks"),
        usb_details: decode(&parsed, "usb"),
        gpu_details: decode(&parsed, "gpu"),
        battery_details: decode(&parsed, "battery"),
        display_details: decode(&parsed, "display"),
    })
}
