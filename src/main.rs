// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hardware;
mod stress_cpu;
mod stress_gpu;
mod stress_ram;
mod stress_disk;
mod report;
mod telemetry;

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
pub static THROTTLING_DETECTED: AtomicBool = AtomicBool::new(false);

pub static LIVE_RAM_THROUGHPUT: AtomicU32 = AtomicU32::new(0);
pub static LIVE_DISK_THROUGHPUT: AtomicU32 = AtomicU32::new(0);
/// T3: Global cancel flag — set to true by cancel_test(), checked in all test loops.
pub static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(serde::Serialize)]
struct SysInfo {
    os_name: String,
    kernel_version: String,
    cpu_name: String,
    memory_total_mb: u64,
}

#[tauri::command]
async fn set_telemetry_consent(consent: bool) {
    if !consent {
        let _ = std::fs::remove_file("/tmp/nemdiag_telemetry_consent");
    } else {
        let _ = std::fs::write("/tmp/nemdiag_telemetry_consent", "1");
    }
}

#[tauri::command]
async fn export_report(format: String, report_json: String) -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let mut desktop = std::path::PathBuf::from(&home).join("Bureau");
    if !desktop.exists() {
        desktop = std::path::PathBuf::from(&home).join("Desktop");
    }
    if !desktop.exists() {
        desktop = std::path::PathBuf::from(&home);
    }
    
    let filename = if format == "html" {
        "NemDiag_Report.html"
    } else {
        "NemDiag_Report.json"
    };
    
    let filepath = desktop.join(filename);
    
    let content = if format == "html" {
        let parsed: serde_json::Value = serde_json::from_str(&report_json).map_err(|e| e.to_string())?;
        format!("
            <html>
                <head>
                    <meta charset='UTF-8'>
                    <title>Rapport NemDiag</title>
                    <style>
                        body {{ font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; background: #020617; color: #f8fafc; padding: 40px; }}
                        h1 {{ color: #00e676; }}
                        .score {{ font-size: 24px; font-weight: bold; color: #38bdf8; }}
                        .card {{ background: rgba(15, 23, 42, 0.6); padding: 20px; border-radius: 10px; border: 1px solid rgba(56, 189, 248, 0.15); margin-bottom: 20px; }}
                        .advice {{ color: #94a3b8; font-style: italic; margin-top: 10px; }}
                    </style>
                </head>
                <body>
                    <h1>Rapport de Diagnostic NemDiag</h1>
                    <div class='card'>
                        <h2>Système : {}</h2>
                        <p>CPU: {} ({} cœurs)</p>
                        <p>RAM: {} Mo</p>
                    </div>
                    <div class='card'>
                        <h2>Score CPU : <span class='score'>{}</span></h2>
                        <p class='advice'>{}</p>
                    </div>
                    <div class='card'>
                        <h2>Score GPU : <span class='score'>{}</span></h2>
                        <p class='advice'>{}</p>
                    </div>
                    <div class='card'>
                        <h2>Score RAM : <span class='score'>{} Mo/s</span></h2>
                        <p class='advice'>{}</p>
                    </div>
                    <div class='card'>
                        <h2>Score Disque : <span class='score'>{} Mo/s</span></h2>
                        <p class='advice'>{}</p>
                    </div>
                    <p style='text-align: center; color: #555; font-size: 0.8rem; margin-top: 40px;'>Généré par NemDiag Linux</p>
                </body>
            </html>
        ", 
        parsed["os_name"].as_str().unwrap_or("Inconnu"),
        parsed["cpu_name"].as_str().unwrap_or("Inconnu"),
        parsed["core_count"].as_i64().unwrap_or(0),
        parsed["memory_total_mb"].as_i64().unwrap_or(0),
        parsed["cpu_score"].as_i64().unwrap_or(0),
        parsed["cpu_advice"].as_str().unwrap_or(""),
        parsed["gpu_score"].as_i64().unwrap_or(0),
        parsed["gpu_advice"].as_str().unwrap_or(""),
        parsed["ram_score"].as_i64().unwrap_or(0),
        parsed["ram_advice"].as_str().unwrap_or(""),
        parsed["disk_score"].as_i64().unwrap_or(0),
        parsed["disk_advice"].as_str().unwrap_or("")
        )
    } else {
        let parsed: serde_json::Value = serde_json::from_str(&report_json).map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&parsed).unwrap_or(report_json)
    };

    std::fs::write(&filepath, content).map_err(|e| format!("Erreur d'écriture: {}", e))?;
    
    Ok(filepath.to_string_lossy().to_string())
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
        fan_speeds: monitor.get_fan_speeds(),
    }
}

#[tauri::command]
fn get_sys_info(state: tauri::State<std::sync::Mutex<HardwareMonitor>>) -> SysInfo {
    let monitor = state.lock().unwrap();
    let info = monitor.get_static_info();
    SysInfo {
        os_name: info.os_name,
        kernel_version: info.kernel_version,
        cpu_name: info.cpu_name,
        memory_total_mb: info.memory_total / 1024 / 1024,
    }
}

#[derive(serde::Serialize)]
struct CompanionAdvice {
    resources_advice: String,
    thermal_advice: String,
    drivers_advice: String,
}

#[tauri::command]
async fn get_companion_advice(state: tauri::State<'_, std::sync::Mutex<HardwareMonitor>>) -> Result<CompanionAdvice, String> {
    let mut resources_advice = String::new();
    let thermal_advice;
    let drivers_advice;
    
    let (ram_total_mb, core_count, temps) = {
        let mut monitor = state.lock().unwrap();
        monitor.refresh();
        let info = monitor.get_static_info();
        let temps = monitor.get_temperatures();
        (info.memory_total / 1024 / 1024, info.core_count, temps)
    };
    
    // CPU & RAM Scores
    let c_score = LAST_CPU_SCORE.load(Ordering::Relaxed);
    let r_score = LAST_RAM_SCORE.load(Ordering::Relaxed);
    
    // RAM Check
    if r_score > 0 {
        if r_score < 5000 {
            resources_advice.push_str(&format!("⚠️ <strong>RAM Lente (Score: {})</strong> : Bande passante faible. Vous êtes sûrement en Single-Channel (une seule barrette). Ajouter une barrette doublerait vos performances.<br>", r_score));
        } else if r_score < 12000 {
            resources_advice.push_str(&format!("✅ <strong>RAM Correcte (Score: {})</strong> : Bande passante suffisante pour un usage courant.<br>", r_score));
        } else {
            resources_advice.push_str(&format!("🚀 <strong>RAM Rapide (Score: {})</strong> : Excellente bande passante, mémoire très performante.<br>", r_score));
        }
    } else {
        if ram_total_mb < 8000 {
            resources_advice.push_str("⚠️ <strong>Mémoire faible</strong> : Moins de 8 Go de RAM. Une mise à niveau est recommandée.<br>");
        } else if ram_total_mb < 16000 {
            resources_advice.push_str("✅ <strong>Mémoire adéquate</strong> : Vos 8-16 Go sont suffisants pour un usage courant.<br>");
        } else {
            resources_advice.push_str("🚀 <strong>Mémoire excellente</strong> : Plus de 16 Go, parfait pour les tâches lourdes.<br>");
        }
    }
    
    // CPU Check
    if c_score > 0 {
        if c_score < 4000 {
            resources_advice.push_str(&format!("⚠️ <strong>Processeur lent (Score: {})</strong> : Score faible, ce processeur risque de peiner sur du multitâche lourd.<br>", c_score));
        } else if c_score < 15000 {
            resources_advice.push_str(&format!("✅ <strong>Processeur moyen (Score: {})</strong> : Bonne capacité pour le jeu et le quotidien.<br>", c_score));
        } else {
            resources_advice.push_str(&format!("🚀 <strong>Processeur performant (Score: {})</strong> : Puissance de calcul massive pour la productivité.<br>", c_score));
        }
    } else {
        if core_count < 4 {
            resources_advice.push_str("⚠️ <strong>Processeur vieillissant</strong> : Moins de 4 cœurs détectés. Multitâche limité.<br>");
        } else if core_count <= 8 {
            resources_advice.push_str("✅ <strong>Processeur polyvalent</strong> : Bonne capacité pour le jeu et le quotidien.<br>");
        } else {
            resources_advice.push_str("🚀 <strong>Processeur performant</strong> : Puissance de calcul massive pour la productivité.<br>");
        }
    }
    
    // Thermal Check
    let max_temp = temps.iter().map(|(_, t)| *t as i32).max().unwrap_or(0);
    if temps.is_empty() {
        thermal_advice = "⚠️ Sondes indisponibles. Impossible de vérifier la température au repos.".to_string();
    } else if max_temp > 80 {
        thermal_advice = format!("⚠️ <strong>Surchauffe détectée au repos ({}°C)</strong> : Pensez à nettoyer la poussière ou changer la pâte thermique !", max_temp);
    } else if max_temp > 60 {
        thermal_advice = format!("⚠️ <strong>Température tiède ({}°C)</strong> : À surveiller en charge.", max_temp);
    } else {
        thermal_advice = format!("✅ <strong>Températures excellentes ({}°C max)</strong> : Votre système est bien refroidi au repos.", max_temp);
    }
    
    // GPU Advice
    let gpu_advice;
    let g_score = LAST_GPU_SCORE.load(Ordering::Relaxed);
    if g_score == 0 {
        gpu_advice = "⚠️ <strong>Carte Graphique</strong> : Test non effectué ou impossible. Pilotes Vulkan absents ou matériel trop ancien.".to_string();
    } else if g_score < 5000 {
        gpu_advice = format!("⚠️ <strong>GPU faible (Score: {})</strong> : Puce graphique basique. Inadapté pour le jeu 3D complexe.", g_score);
    } else if g_score < 40000 {
        gpu_advice = format!("✅ <strong>GPU convenable (Score: {})</strong> : Adapté pour du jeu en 1080p ou usage multimédia.", g_score);
    } else {
        gpu_advice = format!("🚀 <strong>GPU performant (Score: {})</strong> : Excellente carte graphique pour la 3D et l'IA.<br>", g_score);
    }
    resources_advice.push_str(&gpu_advice);
    
    // Drivers Check — détection multi-distro via lspci + modinfo
    {
        // Détecter si une carte NVIDIA/AMD est présente
        let lspci_out = std::process::Command::new("lspci").output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
            .unwrap_or_default();

        let has_nvidia = lspci_out.contains("nvidia");
        let has_amd_gpu = lspci_out.contains("amd") && (lspci_out.contains("vga") || lspci_out.contains("display"));

        // Vérifier si le module propriétaire est chargé
        let loaded_modules = std::process::Command::new("lsmod").output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
            .unwrap_or_default();

        let nvidia_loaded = loaded_modules.contains("nvidia");
        let amdgpu_loaded = loaded_modules.contains("amdgpu");

        // ubuntu-drivers en fallback si disponible
        let ubuntu_check = std::process::Command::new("ubuntu-drivers")
            .arg("devices").output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("recommended"));

        drivers_advice = if let Some(true) = ubuntu_check {
            "⚠️ <strong>Pilote propriétaire recommandé.</strong> Ouvrez le Gestionnaire de pilotes de votre OS.".to_string()
        } else if has_nvidia && !nvidia_loaded {
            "⚠️ <strong>Carte NVIDIA détectée mais pilote propriétaire (nvidia) non chargé.</strong> Installez-le via votre gestionnaire de paquets.".to_string()
        } else if has_amd_gpu && !amdgpu_loaded {
            "⚠️ <strong>Carte AMD détectée mais module amdgpu non chargé.</strong> Vérifiez votre installation.".to_string()
        } else if has_nvidia && nvidia_loaded {
            "✅ <strong>Pilote NVIDIA propriétaire actif.</strong> Votre carte graphique est correctement configurée.".to_string()
        } else if has_amd_gpu && amdgpu_loaded {
            "✅ <strong>Pilote AMD (amdgpu) actif.</strong> Votre carte graphique est correctement configurée.".to_string()
        } else {
            "✅ <strong>Pilotes système à jour.</strong> Aucun pilote propriétaire manquant détecté.".to_string()
        };
    }
    
    Ok(CompanionAdvice {
        resources_advice,
        thermal_advice,
        drivers_advice,
    })
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
async fn run_cpu_test(duration: u64, state: tauri::State<'_, std::sync::Mutex<HardwareMonitor>>) -> Result<(), String> {
    // T3+T5: Reset cancel flag at the start of a new sequence (CPU is always first)
    CANCEL_REQUESTED.store(false, Ordering::Relaxed);
    THROTTLING_DETECTED.store(false, Ordering::Relaxed);
    
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_all();
    let initial_freq = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);
    
    let mut cpu = CpuStress::new();
    cpu.start();
    
    let start = tokio::time::Instant::now();
    let target = Duration::from_secs(duration);
    
    let mut min_freq_in_load = initial_freq;
    let mut max_temp_in_load = 0.0;
    
    while start.elapsed() < target {
        if CANCEL_REQUESTED.load(Ordering::Relaxed) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        sys.refresh_cpu_all();
        let current_freq = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);
        if current_freq > 0 && current_freq < min_freq_in_load {
            min_freq_in_load = current_freq;
        }
        
        if let Ok(mut monitor) = state.try_lock() {
            monitor.refresh();
            let temps = monitor.get_temperatures();
            let mt = temps.iter().map(|(_, t)| *t).fold(0.0, f32::max);
            if mt > max_temp_in_load {
                max_temp_in_load = mt;
            }
        }
    }
    
    // Throttling Check : Si on perd > 15% de freq initiale alors qu'on est au dessus de 80°C
    if initial_freq > 0 && max_temp_in_load > 80.0 {
        let drop_ratio = (initial_freq as f64 - min_freq_in_load as f64) / initial_freq as f64;
        if drop_ratio > 0.15 {
            THROTTLING_DETECTED.store(true, Ordering::Relaxed);
        }
    }
    
    cpu.stop();
    LAST_CPU_SCORE.store(cpu.get_score(), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
async fn run_gpu_test(duration: u64) {
    if CANCEL_REQUESTED.load(Ordering::Relaxed) { return; }
    let mut gpu = GpuStress::new();
    gpu.start();
    interruptible_sleep(duration).await;
    
    let handle = gpu.stop_signal();
    if let Some(h) = handle {
        tokio::task::spawn_blocking(move || { let _ = h.join(); });
    }
    
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
    
    // Arrêt non-bloquant : on signale l'arrêt sans bloquer le thread async
    // (join() est synchrone et gèle Tauri sinon)
    let handle = ram.stop_signal();
    let final_score = ram.get_throughput();
    // Laisser le thread se terminer seul en arrière-plan
    if let Some(h) = handle {
        tokio::task::spawn_blocking(move || { let _ = h.join(); });
    }
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
    
    let handle = disk.stop_signal();
    let final_score = disk.get_throughput();
    if let Some(h) = handle {
        tokio::task::spawn_blocking(move || { let _ = h.join(); });
    }
    
    LIVE_DISK_THROUGHPUT.store(final_score, Ordering::Relaxed);
    LAST_DISK_SCORE.store(final_score, Ordering::Relaxed);
}

#[tauri::command]
async fn get_live_disk_throughput() -> u32 {
    LIVE_DISK_THROUGHPUT.load(Ordering::Relaxed)
}

#[tauri::command]
async fn run_smart_and_export(user_id: String, state: tauri::State<'_, std::sync::Mutex<HardwareMonitor>>) -> Result<String, String> {
    let c_score = LAST_CPU_SCORE.load(Ordering::Relaxed);
    let g_score = LAST_GPU_SCORE.load(Ordering::Relaxed);
    let r_score = LAST_RAM_SCORE.load(Ordering::Relaxed);
    let d_score = LAST_DISK_SCORE.load(Ordering::Relaxed);

    let monitor = state.lock().unwrap();
    match generate_report(&*monitor, c_score, g_score, r_score, d_score, user_id) {
        Ok(path) => Ok(path),
        Err(e) => Err(format!("Erreur: {}", e)),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--cli".to_string()) || args.contains(&"-c".to_string()) {
        println!("=== NemDiag CLI Mode ===");
        let monitor = HardwareMonitor::new();
        let info = monitor.get_static_info();
        println!("OS: {}", info.os_name);
        println!("Kernel: {}", info.kernel_version);
        println!("CPU: {} ({} coeurs)", info.cpu_name, info.core_count);
        println!("RAM Totale: {} Mo", info.memory_total / 1024 / 1024);
        
        println!("\nRécupération des informations détaillées (peut demander le mot de passe root)...");
        if let Ok(detailed) = hardware::gather_detailed_info_linux() {
            println!("\n--- CARTE MÈRE ---");
            println!("{}", detailed.motherboard);
            println!("\n--- MÉMOIRE ---");
            println!("{}", detailed.ram_details);
            println!("\n--- STOCKAGE ---");
            println!("{}", detailed.disks_details);
            println!("\n--- RÉSEAU ---");
            println!("{}", detailed.network_details);
            println!("{}", detailed.wifi_details);
        } else {
            println!("Impossible de récupérer les détails matériels.");
        }
        return;
    }

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
            get_companion_advice,
            set_telemetry_consent,
            export_report,
            cancel_test
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
