// Reusable UI components for the disk health interface

// Import egui for UI rendering
use eframe::egui;

/// Renders a styled statistics card with a label and value.
/// Used to display metrics like temperature, data written, power cycles, etc.
///
/// # Arguments
/// * `ui` - The egui UI context to render into
/// * `width` - Card width in pixels
/// * `height` - Card height in pixels
/// * `label` - Descriptive text shown at the top (e.g., "SSD Temperature")
/// * `value` - Main value displayed prominently (e.g., "45°C")
/// * `color` - Color used for the value text
pub fn stat_card(ui: &mut egui::Ui, width: f32, height: f32, label: &str, value: &str, color: egui::Color32) {
    // Create a white card with rounded corners and a subtle border
    egui::Frame::none()
        .fill(egui::Color32::WHITE)
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(230)))
        .rounding(10.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            // Set fixed dimensions for consistent layout
            ui.set_width(width);
            ui.set_height(height);
            ui.vertical(|ui| {
                // Display label in small gray text
                ui.label(
                    egui::RichText::new(label)
                        .size(11.0)
                        .color(egui::Color32::from_gray(120)),
                );
                ui.add_space(8.0);
                // Display value in large colored text
                ui.label(egui::RichText::new(value).size(22.0).color(color).strong());
            });
        });
}