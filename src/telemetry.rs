use sha2::{Sha256, Digest};
use std::fs;

pub fn get_machine_id(cpu_name: &str) -> String {
    // Lecture de l'UUID unique de la carte mère (souvent nécessite root, mais parfois lisible par tous)
    let product_uuid = fs::read_to_string("/sys/class/dmi/id/product_uuid")
        .unwrap_or_else(|_| "UNKNOWN_UUID".to_string())
        .trim()
        .to_string();

    // Combinaison de l'UUID et du nom du CPU pour éviter les collisions si UUID générique
    let combined = format!("{}-{}", product_uuid, cpu_name);
    
    // Hash SHA-256 pour anonymisation parfaite (irréversible)
    let mut hasher = Sha256::new();
    hasher.update(combined.as_bytes());
    let result = hasher.finalize();
    
    let mut hex = String::with_capacity(64);
    for byte in result {
        use std::fmt::Write;
        write!(&mut hex, "{:02x}", byte).unwrap();
    }
    hex
}
