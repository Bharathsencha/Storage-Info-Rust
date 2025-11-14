// GUI module organization and public exports

// Main application state and UI logic
mod app;
// Reusable UI components (stat cards, etc.)
mod components;
// Disk scanning and SMART data collection
mod disk_scanner;

// Export AppState for use in main.rs
pub use app::AppState;
// Export all component functions (stat_card)
pub use components::*;