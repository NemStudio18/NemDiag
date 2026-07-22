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
        let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
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
            .map(|c| (c.label().to_string(), c.temperature().unwrap_or(0.0)))
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
