// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hardware;
mod stress_cpu;
mod stress_gpu;
mod stress_ram;
mod stress_disk;
mod report;

use hardware::{HardwareMonitor, RealtimeInfo};
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

pub static LIVE_RAM_THROUGHPUT: AtomicU32 = AtomicU32::new(0);
pub static LIVE_DISK_THROUGHPUT: AtomicU32 = AtomicU32::new(0);
/// T3: Global cancel flag — set to true by cancel_test(), checked in all test loops.
pub static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

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

/// T3: Cancels the currently running stress test sequence.
#[tauri::command]
fn cancel_test() {
    CANCEL_REQUESTED.store(true, Ordering::Relaxed);
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

/// T5: Non-blocking async sleep loop — allows cancel and keeps Tauri UI responsive.
async fn interruptible_sleep(duration_secs: u64) {
    // Reset cancel at the very start (only run_cpu_test calls this first)
    let start = tokio::time::Instant::now();
    let target = Duration::from_secs(duration_secs);
    while start.elapsed() < target {
        if CANCEL_REQUESTED.load(Ordering::Relaxed) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tauri::command]
async fn run_cpu_test(duration: u64) {
    // T3+T5: Reset cancel flag at the start of a new sequence (CPU is always first)
    CANCEL_REQUESTED.store(false, Ordering::Relaxed);
    let mut cpu = CpuStress::new();
    cpu.start();
    interruptible_sleep(duration).await;
    cpu.stop();
    LAST_CPU_SCORE.store(cpu.get_score(), Ordering::Relaxed);
}

#[tauri::command]
async fn run_gpu_test(duration: u64) {
    if CANCEL_REQUESTED.load(Ordering::Relaxed) { return; }
    let mut gpu = GpuStress::new();
    gpu.start();
    interruptible_sleep(duration).await;
    gpu.stop();
    // T13: Score = total compute passes * 50 (gives ~100 for iGPU, ~5000 for discrete GPU)
    LAST_GPU_SCORE.store((gpu.get_total_passes() * 50) as u32, Ordering::Relaxed);
}

#[tauri::command]
async fn run_ram_test(duration: u64) {
    if CANCEL_REQUESTED.load(Ordering::Relaxed) { return; }
    let mut ram = RamStress::new();
    ram.start();
    
    // Polling loop for live data
    for _ in 0..(duration * 10) {
        if CANCEL_REQUESTED.load(Ordering::Relaxed) { break; }
        LIVE_RAM_THROUGHPUT.store(ram.get_throughput(), Ordering::Relaxed);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    ram.stop();
    let final_score = ram.get_throughput();
    LIVE_RAM_THROUGHPUT.store(final_score, Ordering::Relaxed);
    LAST_RAM_SCORE.store(final_score, Ordering::Relaxed);
}

#[tauri::command]
async fn get_live_ram_throughput() -> u32 {
    LIVE_RAM_THROUGHPUT.load(Ordering::Relaxed)
}

#[tauri::command]
async fn run_disk_test(duration: u64) {
    if CANCEL_REQUESTED.load(Ordering::Relaxed) { return; }
    let mut disk = DiskStress::new();
    disk.start();
    
    for _ in 0..(duration * 10) {
        if CANCEL_REQUESTED.load(Ordering::Relaxed) { break; }
        LIVE_DISK_THROUGHPUT.store(disk.get_throughput(), Ordering::Relaxed);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    disk.stop();
    let final_score = disk.get_throughput();
    LIVE_DISK_THROUGHPUT.store(final_score, Ordering::Relaxed);
    LAST_DISK_SCORE.store(final_score, Ordering::Relaxed);
}

#[tauri::command]
async fn get_live_disk_throughput() -> u32 {
    LIVE_DISK_THROUGHPUT.load(Ordering::Relaxed)
}

#[tauri::command]
async fn run_smart_and_export(state: tauri::State<'_, std::sync::Mutex<HardwareMonitor>>) -> Result<String, String> {
    let c_score = LAST_CPU_SCORE.load(Ordering::Relaxed);
    let g_score = LAST_GPU_SCORE.load(Ordering::Relaxed);
    let r_score = LAST_RAM_SCORE.load(Ordering::Relaxed);
    let d_score = LAST_DISK_SCORE.load(Ordering::Relaxed);

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
        let _ = thread::spawn(move || {
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
            get_live_ram_throughput,
            run_disk_test,
            get_live_disk_throughput,
            run_smart_and_export,
            set_telemetry_consent,
            cancel_test
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
