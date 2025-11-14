    use eframe::egui;

    pub fn stat_card(ui: &mut egui::Ui, width: f32, height: f32, label: &str, value: &str, color: egui::Color32) {
        egui::Frame::none()
            .fill(egui::Color32::WHITE)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(230)))
            .rounding(10.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_width(width);
                ui.set_height(height);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .size(11.0)
                            .color(egui::Color32::from_gray(120)),
                    );
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(value).size(22.0).color(color).strong());
                });
            });
    }