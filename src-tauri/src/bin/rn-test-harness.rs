// Test harness binary for E2E testing
// Run with: cargo run --bin rn-test-harness --features test-harness

fn main() {
    rn_desktop_2_lib::create_test_harness_builder()
        .run(tauri::generate_context!("tauri.test.conf.json"))
        .expect("error while running test harness");
}
