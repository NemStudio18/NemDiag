use serde::{Serialize, Deserialize};
use std::fs;
use crate::hardware::{HardwareMonitor, get_nvml_info};

#[derive(Serialize, Deserialize)]
pub struct ReportData {
    pub os_name: String,
    pub kernel_version: String,
    pub host_name: String,
    pub cpu_name: String,
    pub core_count: usize,
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    
    // Performance
    pub cpu_stress_running: bool,
    pub gpu_stress_running: bool,
    pub gpu_fps: u32,
    pub ram_stress_running: bool,
    pub ram_throughput_mbs: u32,
    pub disk_stress_running: bool,
    pub disk_throughput_mbs: u32,
    
    // Scores
    pub cpu_score: u64,
    pub gpu_score: u32,
    pub ram_score: u32,
    pub disk_score: u32,

    // Recommendations
    pub recommendations: Vec<String>,

    // NVML data if available
    pub nvidia_gpus: Vec<NvidiaGpuData>,
}

#[derive(Serialize, Deserialize)]
pub struct NvidiaGpuData {
    pub name: String,
    pub temperature: u32,
    pub utilization: u32,
}

pub fn generate_report(
    monitor: &HardwareMonitor,
    cpu_score: u64,
    gpu_score: u32,
    ram_score: u32,
    disk_score: u32,
) -> Result<String, String> {
    let info = monitor.get_static_info();
    
    let nvml_raw = get_nvml_info();
    let nvidia_gpus = nvml_raw.into_iter().map(|(name, temperature, utilization)| NvidiaGpuData {
        name,
        temperature,
        utilization,
    }).collect();

    let mut recommendations = Vec::new();
    let max_temp = monitor.get_temperatures().into_iter().map(|(_, t)| t).fold(0.0, f32::max);
    if max_temp > 85.0 {
        recommendations.push("Surchauffe détectée (> 85°C). Pensez à dépoussiérer les ventilateurs ou changer la pâte thermique.".to_string());
    }

    if info.memory_total > 0 && (info.memory_used as f64 / info.memory_total as f64) > 0.85 {
        recommendations.push("Manque de mémoire vive (RAM) détecté, ce qui ralentit le système. Envisagez une augmentation.".to_string());
    }
    
    // Simple mock heuristic for SSD/HDD
    if disk_score < 100 {
        recommendations.push("La vitesse de stockage est particulièrement faible. Le disque pourrait présenter des signes de faiblesse ou être un vieux HDD.".to_string());
    }

    let data = ReportData {
        os_name: info.os_name,
        kernel_version: info.kernel_version,
        host_name: info.host_name,
        cpu_name: info.cpu_name,
        core_count: info.core_count,
        memory_total_mb: info.memory_total / 1024 / 1024,
        memory_used_mb: info.memory_used / 1024 / 1024,
        cpu_stress_running: false,
        gpu_stress_running: false,
        gpu_fps: 0,
        ram_stress_running: false,
        ram_throughput_mbs: 0,
        disk_stress_running: false,
        disk_throughput_mbs: 0,
        cpu_score,
        gpu_score,
        ram_score,
        disk_score,
        recommendations,
        nvidia_gpus,
    };

    let json = serde_json::to_string_pretty(&data).map_err(|e| format!("Failed to serialize: {}", e))?;
    
    let path = "/tmp/nemdiag_report.json";
    fs::write(path, &json).map_err(|e| format!("Failed to write file: {}", e))?;

    // Asynchronously send telemetry
    let telemetry_json = json.clone();
    if crate::TELEMETRY_CONSENT.load(std::sync::atomic::Ordering::Relaxed) {
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let _ = client.post("https://nemdiag.nhtml.ynh.fr/api/telemetry")
                .header("Content-Type", "application/json")
                .body(telemetry_json)
                .send()
                .await;
        });
    }
    
    Ok(json) // We return the JSON content directly to frontend instead of just the path
}
