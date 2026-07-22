mod hardware;
mod stress_cpu;
mod stress_gpu;

use eframe::egui;
use hardware::{HardwareMonitor, get_baseboard_info_sudo};
use stress_cpu::CpuStress;
use stress_gpu::GpuStress;

struct NemdiagApp {
    monitor: HardwareMonitor,
    cpu_stress: CpuStress,
    gpu_stress: GpuStress,
    baseboard_info: String,
    show_sudo_error: bool,
    sudo_error_msg: String,
}

impl NemdiagApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            monitor: HardwareMonitor::new(),
            cpu_stress: CpuStress::new(),
            gpu_stress: GpuStress::new(),
            baseboard_info: String::new(),
            show_sudo_error: false,
            sudo_error_msg: String::new(),
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
            ui.label("No temperature sensors detected (or missing permissions).");
        } else {
            for (label, temp) in temps {
                ui.label(format!("{}: {:.1} °C", label, temp));
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

        ui.add_space(5.0);

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

        ui.separator();

        // --- HARDWARE DETAILS (requires pkexec) ---
        ui.heading("Advanced Hardware Info (Requires Sudo/pkexec)");
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

        if self.show_sudo_error {
            ui.colored_label(egui::Color32::RED, format!("Sudo Error: {}", self.sudo_error_msg));
        } else if !self.baseboard_info.is_empty() {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(&self.baseboard_info);
            });
        }

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
