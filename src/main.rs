// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hardware;
mod stress_cpu;
mod stress_gpu;
mod stress_ram;
mod stress_disk;
mod report;

use hardware::{HardwareMonitor, RealtimeInfo, get_baseboard_info_sudo, get_smart_info};
use stress_cpu::CpuStress;
use stress_gpu::GpuStress;
use stress_ram::RamStress;
use stress_disk::DiskStress;
use report::generate_report;
use std::time::Duration;
use std::thread;

#[derive(serde::Serialize)]
struct SysInfo {
    os_name: String,
    cpu_name: String,
    memory_total_mb: u64,
}

#[tauri::command]
fn get_realtime_stats(state: tauri::State<std::sync::Mutex<HardwareMonitor>>) -> RealtimeInfo {
    let mut monitor = state.lock().unwrap();
    monitor.refresh();
    RealtimeInfo {
        cpu_usage: monitor.get_cpu_usage(),
        memory_used: monitor.get_static_info().memory_used,
        temperatures: monitor.get_temperatures(),
    }
}

#[tauri::command]
fn get_sys_info(state: tauri::State<std::sync::Mutex<HardwareMonitor>>) -> SysInfo {
    let monitor = state.lock().unwrap();
    let info = monitor.get_static_info();
    SysInfo {
        os_name: info.os_name,
        cpu_name: info.cpu_name,
        memory_total_mb: info.memory_total / 1024 / 1024,
    }
}

#[tauri::command]
async fn run_cpu_test(duration: u64) {
    let mut cpu = CpuStress::new();
    cpu.start();
    thread::sleep(Duration::from_secs(duration));
    cpu.stop();
}

#[tauri::command]
async fn run_gpu_test(duration: u64) {
    let mut gpu = GpuStress::new();
    gpu.start();
    thread::sleep(Duration::from_secs(duration));
    gpu.stop();
}

#[tauri::command]
async fn run_ram_test(duration: u64) {
    let mut ram = RamStress::new();
    ram.start();
    thread::sleep(Duration::from_secs(duration));
    ram.stop();
}

#[tauri::command]
async fn run_disk_test(duration: u64) {
    let mut disk = DiskStress::new();
    disk.start();
    thread::sleep(Duration::from_secs(duration));
    disk.stop();
}

#[tauri::command]
async fn run_smart_and_export(state: tauri::State<'_, std::sync::Mutex<HardwareMonitor>>) -> Result<String, String> {
    let cpu = CpuStress::new();
    let gpu = GpuStress::new();
    let ram = RamStress::new();
    let disk = DiskStress::new();

    // Try fetching SMART in the background (will prompt user via pkexec if on Linux)
    let _ = get_smart_info("/dev/sda");
    let _ = get_baseboard_info_sudo();

    let monitor = state.lock().unwrap();
    match generate_report(&*monitor, &cpu, &gpu, &ram, &disk) {
        Ok(path) => Ok(path),
        Err(e) => Err(format!("Erreur: {}", e)),
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let payload = format!("Nemdiag Crash: {:?}", info);
        let _ = std::thread::spawn(move || {
            if let Ok(client) = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(2)).build() {
                let _ = client.post("http://localhost:8080/api/crash")
                    .body(payload)
                    .send();
            }
        }).join();
    }));

    tauri::Builder::default()
        .manage(std::sync::Mutex::new(HardwareMonitor::new()))
        .invoke_handler(tauri::generate_handler![
            get_sys_info,
            get_realtime_stats,
            run_cpu_test,
            run_gpu_test,
            run_ram_test,
            run_disk_test,
            run_smart_and_export
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
