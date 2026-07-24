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
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};

pub static TELEMETRY_CONSENT: AtomicBool = AtomicBool::new(false);
pub static LAST_CPU_SCORE: AtomicU64 = AtomicU64::new(0);
pub static LAST_GPU_SCORE: AtomicU32 = AtomicU32::new(0);
pub static LAST_RAM_SCORE: AtomicU32 = AtomicU32::new(0);
pub static LAST_DISK_SCORE: AtomicU32 = AtomicU32::new(0);

#[derive(serde::Serialize)]
struct SysInfo {
    os_name: String,
    cpu_name: String,
    memory_total_mb: u64,
}

#[tauri::command]
fn set_telemetry_consent(consent: bool) {
    TELEMETRY_CONSENT.store(consent, Ordering::Relaxed);
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
async fn get_detailed_system_info() -> Result<hardware::DetailedSystemInfo, String> {
    hardware::gather_detailed_info_linux()
}

#[tauri::command]
async fn run_cpu_test(duration: u64) {
    let mut cpu = CpuStress::new();
    cpu.start();
    thread::sleep(Duration::from_secs(duration));
    cpu.stop();
    LAST_CPU_SCORE.store(cpu.get_score(), Ordering::Relaxed);
}

#[tauri::command]
async fn run_gpu_test(duration: u64) {
    let mut gpu = GpuStress::new();
    gpu.start();
    thread::sleep(Duration::from_secs(duration));
    gpu.stop();
    LAST_GPU_SCORE.store(gpu.get_fps() * 10, Ordering::Relaxed);
}

#[tauri::command]
async fn run_ram_test(duration: u64) {
    let mut ram = RamStress::new();
    ram.start();
    thread::sleep(Duration::from_secs(duration));
    ram.stop();
    LAST_RAM_SCORE.store(ram.get_throughput(), Ordering::Relaxed);
}

#[tauri::command]
async fn run_disk_test(duration: u64) {
    let mut disk = DiskStress::new();
    disk.start();
    thread::sleep(Duration::from_secs(duration));
    disk.stop();
    LAST_DISK_SCORE.store(disk.get_throughput(), Ordering::Relaxed);
}

#[tauri::command]
async fn run_smart_and_export(state: tauri::State<'_, std::sync::Mutex<HardwareMonitor>>) -> Result<String, String> {
    let c_score = LAST_CPU_SCORE.load(Ordering::Relaxed);
    let g_score = LAST_GPU_SCORE.load(Ordering::Relaxed);
    let r_score = LAST_RAM_SCORE.load(Ordering::Relaxed);
    let d_score = LAST_DISK_SCORE.load(Ordering::Relaxed);

    // We no longer fetch SMART here manually since it was moved to the dashboard startup,
    // and these blocking pkexec calls cause the test completion to hang.

    let monitor = state.lock().unwrap();
    match generate_report(&*monitor, c_score, g_score, r_score, d_score) {
        Ok(path) => Ok(path),
        Err(e) => Err(format!("Erreur: {}", e)),
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        if !TELEMETRY_CONSENT.load(Ordering::Relaxed) {
            return;
        }
        let payload = format!("Nemdiag Crash: {:?}", info);
        let _ = std::thread::spawn(move || {
            if let Ok(client) = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(2)).build() {
                let _ = client.post("https://nemdiag.nhtml.ynh.fr/api/crash")
                    .body(payload)
                    .send();
            }
        }).join();
    }));

    tauri::Builder::default()
        .manage(std::sync::Mutex::new(HardwareMonitor::new()))
        .invoke_handler(tauri::generate_handler![
            get_sys_info,
            get_detailed_system_info,
            get_realtime_stats,
            run_cpu_test,
            run_gpu_test,
            run_ram_test,
            run_disk_test,
            run_smart_and_export,
            set_telemetry_consent
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
