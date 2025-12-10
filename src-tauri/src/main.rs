//! Tauri entrypoint

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    echovault_lib::run()
}
