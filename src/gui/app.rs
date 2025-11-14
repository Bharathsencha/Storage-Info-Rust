use crate::gui::{disk_scanner::scan_disks, stat_card};
use crate::models::DiskInfo;
use eframe::egui;
use regex::Regex;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Main application state for the eframe app.
pub struct AppState {
    /// The discovered drives (wrapped in Arc for cheap cloning into UI).
    drives: Vec<Arc<DiskInfo>>,

    /// Index of currently selected drive in `drives`.
    selected: usize,

    /// Last error message (if any) from scanning drives.
    last_error: Option<String>,

    /// Cached CPU temperature (average) in Celsius.
    cpu_temp: Option<f32>,

    /// Cached GPU temperature in Celsius.
    gpu_temp: Option<f32>,

    /// Instant when the last automatic refresh happened.
    last_refresh: Instant,

    /// How often to automatically refresh (seconds).
    refresh_interval: Duration,
}

impl AppState {
    /// Create a new app state. Sets light visuals, triggers an immediate refresh,
    /// and initializes the periodic refresh timer.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Use light theme for consistent visuals.
        cc.egui_ctx.set_visuals(egui::Visuals::light());

        let mut s = Self {
            drives: Vec::new(),
            selected: 0,
            last_error: None,
            cpu_temp: None,
            gpu_temp: None,
            last_refresh: Instant::now() - Duration::from_secs(10), // force immediate refresh
            refresh_interval: Duration::from_secs(5), // auto refresh every 5 seconds
        };

        // Perform initial data collection
        s.refresh();
        s.update_system_temps();

        s
    }

    /// Refresh disk list by calling your `scan_disks` function.
    /// On success we replace the drives vector; on error we clear drives
    /// and store the error message for display.
    fn refresh(&mut self) {
        self.last_error = None;
        match scan_disks() {
            Ok(list) => {
                self.drives = list.into_iter().map(Arc::new).collect();

                // If current selection is out of range after refresh, clamp to zero
                if !self.drives.is_empty() && self.selected >= self.drives.len() {
                    self.selected = 0;
                }

                // If no drives found, clear selection
                if self.drives.is_empty() {
                    self.selected = 0;
                }
            }
            Err(e) => {
                self.drives.clear();
                self.last_error = Some(e);
            }
        }
    }

    /// Update CPU and GPU temperature readings using external commands.
    ///
    /// This function parses `sensors` output for CPU temps and `nvidia-smi`
    /// output for NVIDIA GPU temperature. Failures are silently ignored
    /// (fields remain `None`).
    fn update_system_temps(&mut self) {
        // Update CPU temperature by parsing the `sensors` output (if available).
        // We look for common labels: tctl, tdie, package, core and parse +XX.X°C.
        if let Ok(output) = Command::new("sensors").output() {
            if let Ok(text) = String::from_utf8(output.stdout) {
                // Regex captures numbers like +47.0°C or +47°C
                let temp_re = Regex::new(r"\+([0-9]+(?:\.[0-9]+)?)°C").unwrap();
                let mut temps: Vec<f32> = Vec::new();

                for line in text.lines() {
                    let lower = line.to_lowercase();
                    // Only consider lines that most likely contain CPU temps.
                    if lower.contains("tctl")
                        || lower.contains("tdie")
                        || lower.contains("package")
                        || lower.contains("core")
                    {
                        if let Some(caps) = temp_re.captures(line) {
                            if let Some(m) = caps.get(1) {
                                if let Ok(v) = m.as_str().parse::<f32>() {
                                    temps.push(v);
                                }
                            }
                        }
                    }
                }

                // Compute simple average (if we found any values).
                if !temps.is_empty() {
                    self.cpu_temp = Some(temps.iter().sum::<f32>() / temps.len() as f32);
                }
            }
        }

        // Update GPU temperature using `nvidia-smi` if available.
        if let Ok(output) = Command::new("nvidia-smi")
            .args(&["--query-gpu=temperature.gpu", "--format=csv,noheader,nounits"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(temp) = text.trim().parse::<f32>() {
                    self.gpu_temp = Some(temp);
                }
            }
        }
    }

    /// Trigger a manual refresh and update temps; also update the last_refresh instant.
    fn manual_refresh(&mut self) {
        self.refresh();
        self.update_system_temps();
        self.last_refresh = Instant::now();
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Schedule a repaint so animations and the UI stay responsive.
        // This is independent of the data-refresh frequency.
        ctx.request_repaint_after(Duration::from_secs(1));

        // If the refresh interval has elapsed, perform an automatic refresh.
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.refresh();
            self.update_system_temps();
            self.last_refresh = Instant::now();
        }

        // LEFT: Sidebar with drives list and manual refresh button
        egui::SidePanel::left("drive_panel")
            .resizable(false)
            .exact_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(10.0);

                // Title centered at the top
                ui.vertical_centered(|ui| {
                    ui.heading(egui::RichText::new("Drives").size(18.0).strong());
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                // Drive entries
                for (i, d) in self.drives.iter().enumerate() {
                    let is_selected = self.selected == i;

                    // Visual frame changes when selected.
                    let frame = if is_selected {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(100, 180, 255))
                            .rounding(8.0)
                            .inner_margin(12.0)
                    } else {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(240, 240, 245))
                            .rounding(8.0)
                            .inner_margin(12.0)
                    };

                    // Show device path and truncated model string
                    let response = frame.show(ui, |ui| {
                        ui.vertical(|ui| {
                            let text_color = if is_selected {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::BLACK
                            };

                            // Device path (e.g., /dev/nvme0n1)
                            ui.label(
                                egui::RichText::new(&d.dev)
                                    .strong()
                                    .size(14.0)
                                    .color(text_color),
                            );

                            // Display model if present (truncate for sidebar)
                            if let Some(model) = &d.model {
                                let display = if model.len() > 20 {
                                    format!("{}...", &model[..20])
                                } else {
                                    model.clone()
                                };
                                ui.label(
                                    egui::RichText::new(display)
                                        .size(10.0)
                                        .color(if is_selected {
                                            egui::Color32::from_gray(220)
                                        } else {
                                            egui::Color32::from_gray(100)
                                        }),
                                );
                            }
                        });
                    });

                    // Select on click
                    if response.response.interact(egui::Sense::click()).clicked() {
                        self.selected = i;
                    }

                    ui.add_space(8.0);
                }

                // Bottom-aligned manual refresh button
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);

                    let refresh_response = egui::Frame::none()
                        .fill(egui::Color32::from_rgb(59, 130, 246))
                        .rounding(6.0)
                        .inner_margin(egui::vec2(20.0, 8.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("Refresh")
                                    .size(12.0)
                                    .color(egui::Color32::WHITE),
                            );
                        });

                    if refresh_response.response.interact(egui::Sense::click()).clicked() {
                        self.manual_refresh();
                    }

                    ui.add_space(10.0);
                });
            });

        // CENTRAL: Main display area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(250, 251, 252)))
            .show(ctx, |ui| {
                // If we have no drives, show helpful guidance and return early.
                if self.drives.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("No drives detected");
                            ui.add_space(8.0);
                            ui.label("Run with sudo and ensure smartctl is installed");
                            if let Some(err) = &self.last_error {
                                ui.add_space(6.0);
                                ui.label(format!("Last error: {}", err));
                            }
                        });
                    });
                    return;
                }

                // Grab the currently selected DiskInfo
                let di = self.drives[self.selected].as_ref();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(15.0);

                    // --- Model label (Option A): shown directly ABOVE the health bar, left-aligned ---
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        if let Some(model) = &di.model {
                            // Bold model label
                            ui.label(
                                egui::RichText::new(format!("Model: {}", model))
                                    .strong()
                                    .size(13.0),
                            );
                        } else {
                            ui.label(egui::RichText::new("Model: --").strong().size(13.0));
                        }
                    });

                    ui.add_space(8.0);

                    // --- Health Status Bar ---
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        let health_pct = di.health_percent.unwrap_or(0);
                        let (bar_color, status_text) = match health_pct {
                            p if p > 84 => (egui::Color32::from_rgb(34, 197, 94), "Good"),
                            p if p >= 50 => (egui::Color32::from_rgb(245, 158, 11), "Warning"),
                            _ => (egui::Color32::from_rgb(239, 68, 68), "Critical"),
                        };

                        // White rounded panel that contains the bar
                        egui::Frame::none()
                            .fill(egui::Color32::WHITE)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(220)))
                            .rounding(10.0)
                            .inner_margin(15.0)
                            .show(ui, |ui| {
                                // Make the bar occupy the available width minus outer gaps.
                                ui.set_width(ui.available_width() - 40.0);

                                let bar_width = ui.available_width();
                                let rect = ui.allocate_space(egui::vec2(bar_width, 26.0)).1;

                                // Background track
                                ui.painter().rect_filled(rect, 8.0, egui::Color32::from_gray(230));

                                // Filled portion based on percentage
                                let filled_width = rect.width() * (health_pct as f32 / 100.0);
                                let filled_rect = egui::Rect::from_min_size(rect.min, egui::vec2(filled_width, rect.height()));
                                ui.painter().rect_filled(filled_rect, 8.0, bar_color);

                                // Centered text inside the bar
                                let text = format!("{} ({}%)", status_text, health_pct);
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    text,
                                    egui::FontId::new(15.0, egui::FontFamily::Proportional),
                                    egui::Color32::WHITE,
                                );
                            });

                        ui.add_space(20.0);
                    });

                    ui.add_space(12.0);

                    // --- Partition table 
                    if !di.partitions.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            egui::Frame::none()
                                .fill(egui::Color32::WHITE)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(220)))
                                .rounding(10.0)
                                .inner_margin(15.0)
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width() - 40.0);

                                    ui.label(egui::RichText::new("Partitions").size(14.0).strong());
                                    ui.add_space(8.0);

                                    egui::Grid::new("part_grid")
                                        .striped(true)
                                        .spacing([25.0, 10.0])
                                        .show(ui, |ui| {
                                            // Compute some column width based on available space
                                            let total_cols = 7.0;
                                            let col_width = ui.available_width() / total_cols;

                                            // Headers
                                            for header in &["Partition", "Mount point", "Type", "Total", "Used", "Free", "Free%"] {
                                                ui.set_min_width(col_width);
                                                ui.label(egui::RichText::new(*header).strong().size(11.0));
                                            }
                                            ui.end_row();

                                            // Each partition row
                                            for part in &di.partitions {
                                                let partition_name =
                                                    part.mount_point.rsplit('/').next().unwrap_or(&part.mount_point).to_string();

                                                ui.set_min_width(col_width);
                                                ui.label(egui::RichText::new(partition_name).size(11.0));

                                                ui.set_min_width(col_width);
                                                ui.label(egui::RichText::new(&part.mount_point).size(11.0));

                                                ui.set_min_width(col_width);
                                                ui.label(egui::RichText::new(&part.fs_type).size(11.0));

                                                ui.set_min_width(col_width);
                                                ui.label(egui::RichText::new(format!("{:.1} GB", part.total_gb)).size(11.0));

                                                ui.set_min_width(col_width);
                                                ui.label(egui::RichText::new(format!("{:.1} GB", part.used_gb)).size(11.0));

                                                ui.set_min_width(col_width);
                                                ui.label(egui::RichText::new(format!("{:.1} GB", part.free_gb)).size(11.0));

                                                let free_pct = 100.0 - part.used_percent;
                                                let color = if free_pct < 10.0 {
                                                    egui::Color32::from_rgb(239, 68, 68)
                                                } else if free_pct < 25.0 {
                                                    egui::Color32::from_rgb(245, 158, 11)
                                                } else {
                                                    egui::Color32::from_rgb(34, 197, 94)
                                                };

                                                ui.set_min_width(col_width);
                                                ui.colored_label(color, egui::RichText::new(format!("{:.1}%", free_pct)).size(11.0));

                                                ui.end_row();
                                            }
                                        });
                                });
                            ui.add_space(20.0);
                        });

                        ui.add_space(12.0);
                    }

                    // --- Drive information card ---
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        egui::Frame::none()
                            .fill(egui::Color32::WHITE)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(220)))
                            .rounding(10.0)
                            .inner_margin(15.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width() - 40.0);

                                ui.label(egui::RichText::new("Drive Information").size(14.0).strong());
                                ui.add_space(8.0);

                                egui::Grid::new("info_grid")
                                    .striped(true)
                                    .spacing([15.0, 6.0])
                                    .show(ui, |ui| {
                                        for header in &["Serial no.", "Firmware", "Type"] {
                                            ui.label(egui::RichText::new(*header).strong().size(11.0));
                                        }
                                        ui.end_row();

                                        ui.label(egui::RichText::new(di.serial.as_deref().unwrap_or("--")).size(11.0));
                                        ui.label(egui::RichText::new(di.firmware.as_deref().unwrap_or("--")).size(11.0));
                                        ui.label(egui::RichText::new(di.device_type.as_deref().unwrap_or("--")).size(11.0));
                                        ui.end_row();
                                    });
                            });
                        ui.add_space(20.0);
                    });

                    ui.add_space(12.0);

                    // --- Stats cards (3 columns per row) ---
                    let card_width = 283.0;
                    let card_spacing = 11.0;
                    let card_height = 75.0;

                    // Row 1: SSD Temp, CPU Temp, GPU Temp
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "SSD Temperature",
                            &di.temp_c.map(|t| format!("{}°C", t)).unwrap_or("--".into()),
                            egui::Color32::from_rgb(59, 130, 246),
                        );

                        ui.add_space(card_spacing);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "CPU Temp",
                            &self.cpu_temp.map(|t| format!("{:.1}°C", t)).unwrap_or("--".into()),
                            egui::Color32::from_rgb(139, 92, 246),
                        );

                        ui.add_space(card_spacing);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "GPU Temp",
                            &self.gpu_temp.map(|t| format!("{:.1}°C", t)).unwrap_or("--".into()),
                            egui::Color32::from_rgb(236, 72, 153),
                        );
                    });

                    ui.add_space(10.0);

                    // Row 2: Data written, Data read, Power on hours
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "Data written",
                            &di.data_written_tb.map(|t| format!("{:.1} TB", t)).unwrap_or("--".into()),
                            egui::Color32::from_rgb(34, 197, 94),
                        );

                        ui.add_space(card_spacing);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "Data read",
                            &di.data_read_tb.map(|t| format!("{:.1} TB", t)).unwrap_or("--".into()),
                            egui::Color32::from_rgb(251, 146, 60),
                        );

                        ui.add_space(card_spacing);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "Power on hours",
                            &di.power_on_hours.map(|h| h.to_string()).unwrap_or("--".into()),
                            egui::Color32::from_rgb(168, 85, 247),
                        );
                    });

                    ui.add_space(10.0);

                    // Row 3: Power cycles, Unsafe shutdowns, Rotation speed
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "Power cycles",
                            &di.power_cycles.map(|c| c.to_string()).unwrap_or("--".into()),
                            egui::Color32::from_rgb(59, 130, 246),
                        );

                        ui.add_space(card_spacing);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "Unsafe shutdown",
                            &di.unsafe_shutdowns.map(|us| us.to_string()).unwrap_or("--".into()),
                            egui::Color32::from_rgb(239, 68, 68),
                        );

                        ui.add_space(card_spacing);

                        stat_card(
                            ui,
                            card_width,
                            card_height,
                            "HDD rotation speed",
                            &di.rotation_rpm.map(|rpm| format!("{} RPM", rpm)).unwrap_or("SSD Detected".into()),
                            egui::Color32::from_rgb(139, 92, 246),
                        );
                    });

                    ui.add_space(15.0);
                });
            });
    }
}
