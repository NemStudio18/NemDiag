mod hardware;
mod stress_cpu;
mod stress_gpu;
mod stress_ram;
mod stress_disk;
mod report;

use eframe::egui;
use hardware::{HardwareMonitor, get_baseboard_info_sudo, get_smart_info, get_nvml_info};
use stress_cpu::CpuStress;
use stress_gpu::GpuStress;
use stress_ram::RamStress;
use stress_disk::DiskStress;
use report::generate_report;

struct NemdiagApp {
    monitor: HardwareMonitor,
    cpu_stress: CpuStress,
    gpu_stress: GpuStress,
    ram_stress: RamStress,
    disk_stress: DiskStress,
    baseboard_info: String,
    show_sudo_error: bool,
    sudo_error_msg: String,
    smart_info: String,
    report_path: String,
}

impl NemdiagApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            monitor: HardwareMonitor::new(),
            cpu_stress: CpuStress::new(),
            gpu_stress: GpuStress::new(),
            ram_stress: RamStress::new(),
            disk_stress: DiskStress::new(),
            baseboard_info: String::new(),
            show_sudo_error: false,
            sudo_error_msg: String::new(),
            smart_info: String::new(),
            report_path: String::new(),
        }
    }
}

impl eframe::App for NemdiagApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.monitor.refresh();
        
        ui.heading("Nemdiag (Linux Diagnostics)");

        ui.separator();

        // --- SYSTEM INFO ---
        let info = self.monitor.get_static_info();
        ui.label(format!("OS: {}", info.os_name));
        ui.label(format!("Kernel: {}", info.kernel_version));
        ui.label(format!("CPU: {} ({} cores)", info.cpu_name, info.core_count));
        ui.label(format!("Memory: {} MB / {} MB", info.memory_used / 1024 / 1024, info.memory_total / 1024 / 1024));

        ui.separator();

        // --- LIVE MONITORING ---
        ui.heading("Live Monitoring");
        let cpu_usage = self.monitor.get_cpu_usage();
        ui.add(egui::ProgressBar::new(cpu_usage / 100.0).text(format!("CPU Usage: {:.1}%", cpu_usage)));

        ui.label("Temperatures:");
        let temps = self.monitor.get_temperatures();
        if temps.is_empty() {
            ui.label("No standard temperature sensors detected.");
        } else {
            for (label, temp) in temps {
                ui.label(format!("{}: {:.1} °C", label, temp));
            }
        }

        let nv_info = get_nvml_info();
        if !nv_info.is_empty() {
            ui.label("NVIDIA GPUs:");
            for (name, temp, util) in nv_info {
                ui.label(format!("{} - Temp: {}°C - Load: {}%", name, temp, util));
            }
        }

        ui.separator();

        // --- STRESS TESTS ---
        ui.heading("Stress Tests");
        ui.horizontal(|ui| {
            if !self.cpu_stress.is_running() {
                if ui.button("Start CPU Stress Test").clicked() {
                    self.cpu_stress.start();
                }
            } else {
                if ui.button("Stop CPU Stress Test").clicked() {
                    self.cpu_stress.stop();
                }
                ui.label(egui::RichText::new("CPU Stress is RUNNING").color(egui::Color32::RED));
            }
        });

        ui.horizontal(|ui| {
            if !self.gpu_stress.is_running() {
                if ui.button("Start GPU Stress Test (WGPU)").clicked() {
                    self.gpu_stress.start();
                }
            } else {
                if ui.button("Stop GPU Stress Test").clicked() {
                    self.gpu_stress.stop();
                }
                ui.label(egui::RichText::new(format!("GPU Stress RUNNING - FPS: {}", self.gpu_stress.get_fps())).color(egui::Color32::RED));
            }
        });

        ui.horizontal(|ui| {
            if !self.ram_stress.is_running() {
                if ui.button("Start RAM Stress Test").clicked() {
                    self.ram_stress.start();
                }
            } else {
                if ui.button("Stop RAM Stress Test").clicked() {
                    self.ram_stress.stop();
                }
                ui.label(egui::RichText::new(format!("RAM Stress RUNNING - {} MB/s", self.ram_stress.get_throughput())).color(egui::Color32::RED));
            }
        });

        ui.horizontal(|ui| {
            if !self.disk_stress.is_running() {
                if ui.button("Start Disk I/O Stress Test").clicked() {
                    self.disk_stress.start();
                }
            } else {
                if ui.button("Stop Disk I/O Stress Test").clicked() {
                    self.disk_stress.stop();
                }
                ui.label(egui::RichText::new(format!("Disk Stress RUNNING - {} MB/s", self.disk_stress.get_throughput())).color(egui::Color32::RED));
            }
        });

        ui.separator();

        // --- ADVANCED DIAGNOSTICS & EXPORT ---
        ui.heading("Advanced Diagnostics & Export");
        ui.horizontal(|ui| {
            if ui.button("Get Baseboard Info (pkexec)").clicked() {
                match get_baseboard_info_sudo() {
                    Ok(info) => {
                        self.baseboard_info = info;
                        self.show_sudo_error = false;
                    }
                    Err(e) => {
                        self.show_sudo_error = true;
                        self.sudo_error_msg = e;
                    }
                }
            }

            if ui.button("Get S.M.A.R.T Info (pkexec, /dev/sda)").clicked() {
                match get_smart_info("/dev/sda") {
                    Ok(info) => {
                        self.smart_info = info;
                        self.show_sudo_error = false;
                    }
                    Err(e) => {
                        self.show_sudo_error = true;
                        self.sudo_error_msg = e;
                    }
                }
            }
        });

        if ui.button("Export Diagnostic Report (JSON)").clicked() {
            match generate_report(&self.monitor, &self.cpu_stress, &self.gpu_stress, &self.ram_stress, &self.disk_stress) {
                Ok(path) => {
                    self.report_path = format!("Report saved to: {}", path);
                }
                Err(e) => {
                    self.report_path = format!("Export failed: {}", e);
                }
            }
        }
        if !self.report_path.is_empty() {
            ui.label(&self.report_path);
        }

        if self.show_sudo_error {
            ui.colored_label(egui::Color32::RED, format!("Sudo Error: {}", self.sudo_error_msg));
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            if !self.baseboard_info.is_empty() {
                ui.label(egui::RichText::new("Baseboard Info:").strong());
                ui.label(&self.baseboard_info);
            }
            if !self.smart_info.is_empty() {
                ui.label(egui::RichText::new("S.M.A.R.T Info:").strong());
                ui.label(&self.smart_info);
            }
        });

        // Request continuous repaint for live monitoring
        ui.ctx().request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 700.0])
            .with_title("Nemdiag (Rust)"),
        ..Default::default()
    };
    eframe::run_native(
        "Nemdiag",
        native_options,
        Box::new(|cc| Ok(Box::new(NemdiagApp::new(cc)))),
    )
}
