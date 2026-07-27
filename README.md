# NemDiag 🚀

![NemDiag Banner](https://via.placeholder.com/1200x400.png?text=NemDiag+-+Diagnostic+Systeme+Linux)

**NemDiag** est un utilitaire de diagnostic système open-source conçu spécifiquement pour Linux, développé en **Rust** (Tauri + WGPU) pour offrir des performances maximales et un stress-test matériel rigoureux, le tout enrobé dans une interface graphique (NHTML) ultra-moderne et réactive.

## 🌟 Fonctionnalités

- **🔍 Détection Matérielle Complète** : Analyse en profondeur du CPU, GPU, RAM, Disques, Interfaces Réseaux (Ethernet/Wi-Fi), Périphériques USB, et Températures.
- **🔥 Tests de Stress (Benchmark)** :
  - **CPU** : Calculs multithreadés asynchrones intenses (`rayon` / `tokio`).
  - **GPU** : Calculs par Shaders via Vulkan (`wgpu`) pour stresser la puce graphique dédiée ou intégrée (iGPU).
  - **RAM** : Stress d'allocation et de bande passante avec vérification de parité (Bit-flip).
  - **Disque** : Tests d'écriture/lecture séquentielle en RAM (Non-destructifs pour les SSD).
- **🤖 Assistant d'Analyse (Compagnon)** : Algorithme intelligent qui analyse les résultats de vos tests pour vous donner des conseils personnalisés sur votre matériel (Goulots d'étranglement, chauffe, drivers manquants).
- **🏆 Télémétrie & Podium** : Partagez vos résultats sur notre serveur de classement global ! (Optionnel)

## 📦 Installation

NemDiag est distribué sous forme de fichiers prêts à l'emploi. Vous n'avez pas besoin d'installer Rust pour l'utiliser.

### Option 1 : AppImage (Universel)
1. Téléchargez le fichier `.AppImage` dans la section [Releases](https://github.com/NemStudio18/NemDiag/releases).
2. Rendez-le exécutable :
   ```bash
   chmod +x nemdiag_0.2.0_amd64.AppImage
   ```
3. Lancez-le !

### Option 2 : Paquet Debian (.deb)
Pour Ubuntu, Debian, Pop!_OS, Linux Mint :
1. Téléchargez le `.deb`.
2. Installez-le avec `dpkg` ou `apt` :
   ```bash
   sudo apt install ./nemdiag_0.2.0_amd64.deb
   ```

## 🛠️ Développement (Compilation depuis les sources)

Si vous souhaitez contribuer ou compiler vous-même le logiciel :

### Prérequis
- `Rust` et `Cargo` (via rustup)
- Dépendances Tauri (Debian/Ubuntu) :
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### Lancement
```bash
# Lancer l'environnement de développement
cargo run

# Compiler la version Release (Production)
cargo build --release
```

## 📝 Architecture

- **Backend (Rust)** : Extraction d'informations via `sysinfo`, exécution de commandes natives (`lspci`, `nmcli`), et tests de stress non-bloquants (`tokio::task::spawn_blocking`).
- **GPU Engine** : `wgpu` (Vulkan) pour la compatibilité avec toutes les architectures modernes et la génération de passes de calcul Shader.
- **Frontend (Tauri + NHTML)** : Interface HTML/CSS/JS ultra légère intégrée directement dans WebKit (sans embarquer un lourd navigateur Chromium comme Electron).

---
*Projet développé par NemStudio18.*
