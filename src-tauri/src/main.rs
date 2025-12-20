// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "test-harness")]
    {
        rn_desktop_2_lib::create_test_harness_builder()
            .run(tauri::generate_context!("tauri.test.conf.json"))
            .expect("error while running test harness");
    }

    #[cfg(not(feature = "test-harness"))]
    {
        rn_desktop_2_lib::run()
    }
}
