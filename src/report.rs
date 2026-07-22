use serde::{Serialize, Deserialize};
use std::fs;
use crate::hardware::{HardwareMonitor, get_nvml_info};
use crate::stress_cpu::CpuStress;
use crate::stress_gpu::GpuStress;
use crate::stress_ram::RamStress;
use crate::stress_disk::DiskStress;

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
    cpu: &CpuStress,
    gpu: &GpuStress,
    ram: &RamStress,
    disk: &DiskStress,
) -> Result<String, String> {
    let info = monitor.get_static_info();
    
    let nvml_raw = get_nvml_info();
    let nvidia_gpus = nvml_raw.into_iter().map(|(name, temperature, utilization)| NvidiaGpuData {
        name,
        temperature,
        utilization,
    }).collect();

    let data = ReportData {
        os_name: info.os_name,
        kernel_version: info.kernel_version,
        host_name: info.host_name,
        cpu_name: info.cpu_name,
        core_count: info.core_count,
        memory_total_mb: info.memory_total / 1024 / 1024,
        memory_used_mb: info.memory_used / 1024 / 1024,
        cpu_stress_running: cpu.is_running(),
        gpu_stress_running: gpu.is_running(),
        gpu_fps: gpu.get_fps(),
        ram_stress_running: ram.is_running(),
        ram_throughput_mbs: ram.get_throughput(),
        disk_stress_running: disk.is_running(),
        disk_throughput_mbs: disk.get_throughput(),
        nvidia_gpus,
    };

    let json = serde_json::to_string_pretty(&data).map_err(|e| format!("Failed to serialize: {}", e))?;
    
    let path = "/tmp/nemdiag_report.json";
    fs::write(path, &json).map_err(|e| format!("Failed to write file: {}", e))?;
    
    Ok(path.to_string())
}
