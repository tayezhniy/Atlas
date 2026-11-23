#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Manager, generate_handler};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .invoke_handler(generate_handler![
            // Команды между UI и ядром (раздел 12.3 ТЗ)
            // atlas_core_ping,
            // atlas_store_open,
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска приложения Atlas");
}