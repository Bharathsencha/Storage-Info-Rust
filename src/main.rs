// Application entry point for the SSD Health Checker GUI

// Import the GUI module containing the main application state
mod gui;
// Import data models for disk information
mod models;

/// Entry point for the application.
/// Initializes the eframe window with fixed dimensions and launches the GUI.
fn main() -> eframe::Result<()> {
    // Configure window options with fixed size of 1200x675 pixels
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 675.0])
            .with_resizable(false),
        ..Default::default()
    };

    // Start the native eframe application with the configured options
    // Creates a new AppState instance to manage the application
    eframe::run_native(
        "SSD Health Checker",
        options,
        Box::new(|cc| Ok(Box::new(gui::AppState::new(cc)))),
    )
}