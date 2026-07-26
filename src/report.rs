use serde::{Serialize, Deserialize};
use std::fs;
use crate::hardware::{HardwareMonitor, get_nvml_info};

/// Only the fields sent to the remote leaderboard server (GDPR-safe: no host_name).
#[derive(Serialize)]
struct TelemetryPayload {
    os_name: String,
    cpu_name: String,
    core_count: usize,
    memory_total_mb: u64,
    cpu_score: u64,
    gpu_score: u32,
    ram_score: u32,
    disk_score: u32,
    user_id: String,
    system_details: Option<String>,
}

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

    // Component Advice
    pub cpu_advice: String,
    pub gpu_advice: String,
    pub ram_advice: String,
    pub disk_advice: String,

    // NVML data if available
    pub nvidia_gpus: Vec<NvidiaGpuData>,

    // Telemetry ID from the server
    pub run_id: Option<String>,
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
    user_id: String,
) -> Result<String, String> {
    let info = monitor.get_static_info();
    
    let nvml_raw = get_nvml_info();
    let nvidia_gpus = nvml_raw.into_iter().map(|(name, temperature, utilization)| NvidiaGpuData {
        name,
        temperature,
        utilization,
    }).collect();

    let mut cpu_advice = if cpu_score < 4000 {
        format!("Score : {}. Score plutôt faible. Le processeur est probablement très ancien ou souffre de thermal throttling sévère. Envisagez de nettoyer le système de refroidissement.", cpu_score)
    } else if cpu_score < 10000 {
        format!("Score : {}. Score moyen (bureautique avancée). Pour vous donner un ordre d'idée, un CPU de bureau classique récent tourne autour de 6000-8000. Il risque de peiner sur du traitement très lourd.", cpu_score)
    } else {
        format!("Score : {}. Excellent score ! Votre processeur est surpuissant et très performant pour le multitâche lourd, le jeu ou le rendu 3D. (Moyenne haute : ~10000).", cpu_score)
    };

    let max_temp = monitor.get_temperatures().into_iter().map(|(_, t)| t).fold(0.0, f32::max);
    if max_temp > 85.0 {
        cpu_advice.push_str(" AVERTISSEMENT : Surchauffe détectée (> 85°C). Pensez à dépoussiérer les ventilateurs ou changer la pâte thermique.");
    }

    let gpu_advice = if gpu_score == 0 {
        "Aucun GPU matériel performant détecté, ou test impossible (ex: serveur sans interface graphique, machine virtuelle).".to_string()
    } else if gpu_score < 300 {
        "Score faible. Puce graphique intégrée ou ancienne. Suffisant pour l'affichage classique, mais inadapté pour le jeu 3D ou le montage vidéo.".to_string()
    } else if gpu_score < 1500 {
        "Score convenable. GPU dédié de milieu de gamme. Permet de jouer dans des conditions acceptables à la plupart des jeux.".to_string()
    } else {
        "Très haut score ! Carte graphique très performante, taillée pour la haute résolution ou les traitements lourds (IA, 3D).".to_string()
    };

    let mut ram_advice = if ram_score < 5000 {
        "Bande passante très faible (< 5000 Mo/s). Vous utilisez très certainement de la DDR3 ancienne ou vous êtes en Single-Channel (une seule barrette installée). Ajouter une barrette identique doublerait vos performances.".to_string()
    } else if ram_score < 12000 {
        "Bande passante correcte (DDR4 classique ou DDR3 très rapide en Dual-Channel). Suffisant pour 90% des usages.".to_string()
    } else {
        "Excellente bande passante (DDR4/DDR5 haute fréquence en Dual/Quad Channel). Mémoire extrêmement rapide.".to_string()
    };

    if info.memory_total > 0 && (info.memory_used as f64 / info.memory_total as f64) > 0.85 {
        ram_advice.push_str(" AVERTISSEMENT : Plus de 85% de la RAM est actuellement utilisée ! Votre système risque de ralentir (swap). Envisagez d'ajouter de la mémoire.");
    }

    let disk_advice = if disk_score < 150 {
        "Vitesse extrêmement faible. Il s'agit probablement d'un vieux disque dur mécanique (HDD) ou d'un SSD SATA défectueux. Remplacer ce disque par un SSD NVMe donnerait une seconde vie spectaculaire à votre PC.".to_string()
    } else if disk_score < 600 {
        "Vitesse moyenne (limite SATA 3 : ~500 Mo/s). Vous avez un SSD SATA. Les performances sont très correctes pour un usage quotidien.".to_string()
    } else {
        "Vitesse excellente (> 600 Mo/s). Vous possédez un SSD NVMe performant. Le chargement de votre OS et de vos applications est optimal.".to_string()
    };

    let mut data = ReportData {
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
        cpu_advice,
        gpu_advice,
        ram_advice,
        disk_advice,
        nvidia_gpus,
        run_id: None,
    };

    let json = serde_json::to_string_pretty(&data).map_err(|e| format!("Failed to serialize: {}", e))?;
    
    let path = "/tmp/nemdiag_report.json";
    fs::write(path, &json).map_err(|e| format!("Failed to write file: {}", e))?;

    // Asynchronously send telemetry (GDPR fix T2: only send leaderboard-relevant fields, no host_name)
    if crate::TELEMETRY_CONSENT.load(std::sync::atomic::Ordering::Relaxed) {
        // Gather basic hardware info without sudo/pkexec
        let mb_name = std::fs::read_to_string("/sys/devices/virtual/dmi/id/board_name").unwrap_or_default();
        let mb_vendor = std::fs::read_to_string("/sys/devices/virtual/dmi/id/board_vendor").unwrap_or_default();
        let motherboard = format!("{} {}", mb_vendor.trim(), mb_name.trim());
        
        let mut graphics = "Unknown GPU".to_string();
        if let Ok(out) = std::process::Command::new("lspci").output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if line.to_lowercase().contains("vga") || line.to_lowercase().contains("3d controller") {
                    graphics = line.split(": ").nth(1).unwrap_or(line).to_string();
                    break;
                }
            }
        }
        
        let mut disks_json = String::new();
        if let Ok(out) = std::process::Command::new("lsblk").args(["-J", "-o", "NAME,SIZE,MODEL"]).output() {
            disks_json = String::from_utf8_lossy(&out.stdout).to_string();
        }

        // Create a safe system_details JSON
        let safe_details = serde_json::json!({
            "os_name": data.os_name,
            "kernel": data.kernel_version,
            "cpu": data.cpu_name,
            "motherboard": motherboard,
            "graphics": graphics,
            "disks": disks_json
        });
        
        let payload = TelemetryPayload {
            os_name: data.os_name.clone(),
            cpu_name: data.cpu_name.clone(),
            core_count: data.core_count,
            memory_total_mb: data.memory_total_mb,
            cpu_score: data.cpu_score,
            gpu_score: data.gpu_score,
            ram_score: data.ram_score,
            disk_score: data.disk_score,
            user_id: user_id,
            system_details: Some(safe_details.to_string()),
        };
        if let Ok(telemetry_json) = serde_json::to_string(&payload) {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default();
            
            if let Ok(resp) = client.post("https://diag-nem.flexcb.fr/api/telemetry.php")
                .header("Content-Type", "application/json")
                .body(telemetry_json)
                .send() {
                if let Ok(resp_json) = resp.json::<serde_json::Value>() {
                    if let Some(id_val) = resp_json.get("id") {
                        if let Some(id_str) = id_val.as_str() {
                            data.run_id = Some(id_str.to_string());
                        } else if let Some(id_u64) = id_val.as_u64() {
                            data.run_id = Some(id_u64.to_string());
                        }
                    }
                }
            }
        }
    }
    
    let json = serde_json::to_string_pretty(&data).map_err(|e| format!("Failed to serialize: {}", e))?;
    Ok(json) // We return the JSON content directly to frontend instead of just the path
}
