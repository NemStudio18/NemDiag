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

    // Component Advice
    pub cpu_advice: String,
    pub gpu_advice: String,
    pub ram_advice: String,
    pub disk_advice: String,

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

    let mut cpu_advice = String::new();
    if cpu_score < 100 {
        cpu_advice = "Score très faible. Le processeur est probablement très ancien ou souffre de thermal throttling sévère. Envisagez de nettoyer le système de refroidissement ou de remplacer la machine pour des tâches modernes.".to_string();
    } else if cpu_score < 400 {
        cpu_advice = "Score moyen. Le processeur est adapté à de la bureautique et de la navigation web, mais risque de peiner sur du traitement lourd (vidéo, jeux).".to_string();
    } else {
        cpu_advice = "Excellent score ! Votre processeur est puissant et très performant pour le multitâche lourd et le jeu.".to_string();
    }

    let max_temp = monitor.get_temperatures().into_iter().map(|(_, t)| t).fold(0.0, f32::max);
    if max_temp > 85.0 {
        cpu_advice.push_str(" AVERTISSEMENT : Surchauffe détectée (> 85°C). Pensez à dépoussiérer les ventilateurs ou changer la pâte thermique.");
    }

    let mut gpu_advice = String::new();
    if gpu_score == 0 {
        gpu_advice = "Aucun GPU matériel performant détecté, ou test impossible (ex: serveur sans interface graphique, machine virtuelle).".to_string();
    } else if gpu_score < 300 {
        gpu_advice = "Score faible. Puce graphique intégrée ou ancienne. Suffisant pour l'affichage classique, mais inadapté pour le jeu 3D ou le montage vidéo.".to_string();
    } else if gpu_score < 1500 {
        gpu_advice = "Score convenable. GPU dédié de milieu de gamme. Permet de jouer dans des conditions acceptables à la plupart des jeux.".to_string();
    } else {
        gpu_advice = "Très haut score ! Carte graphique très performante, taillée pour la haute résolution ou les traitements lourds (IA, 3D).".to_string();
    }

    let mut ram_advice = String::new();
    if ram_score < 5000 {
        ram_advice = "Bande passante très faible (< 5000 Mo/s). Vous utilisez très certainement de la DDR3 ancienne ou vous êtes en Single-Channel (une seule barrette installée). Ajouter une barrette identique doublerait vos performances.".to_string();
    } else if ram_score < 12000 {
        ram_advice = "Bande passante correcte (DDR4 classique ou DDR3 très rapide en Dual-Channel). Suffisant pour 90% des usages.".to_string();
    } else {
        ram_advice = "Excellente bande passante (DDR4/DDR5 haute fréquence en Dual/Quad Channel). Mémoire extrêmement rapide.".to_string();
    }

    if info.memory_total > 0 && (info.memory_used as f64 / info.memory_total as f64) > 0.85 {
        ram_advice.push_str(" AVERTISSEMENT : Plus de 85% de la RAM est actuellement utilisée ! Votre système risque de ralentir (swap). Envisagez d'ajouter de la mémoire.");
    }

    let mut disk_advice = String::new();
    if disk_score < 150 {
        disk_advice = "Vitesse extrêmement faible. Il s'agit probablement d'un vieux disque dur mécanique (HDD) ou d'un SSD SATA défectueux. Remplacer ce disque par un SSD NVMe donnerait une seconde vie spectaculaire à votre PC.".to_string();
    } else if disk_score < 600 {
        disk_advice = "Vitesse moyenne (limite SATA 3 : ~500 Mo/s). Vous avez un SSD SATA. Les performances sont très correctes pour un usage quotidien.".to_string();
    } else {
        disk_advice = "Vitesse excellente (> 600 Mo/s). Vous possédez un SSD NVMe performant. Le chargement de votre OS et de vos applications est optimal.".to_string();
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
        cpu_advice,
        gpu_advice,
        ram_advice,
        disk_advice,
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
