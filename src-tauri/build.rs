use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // Workaround for Tauri bundler issue:
    // The bundler tries to copy all binaries listed in Cargo.toml, even those with required-features.
    // When test-harness feature is not enabled, rn-test-harness won't be built, but Tauri still
    // tries to copy it. We create an empty stub file to prevent the bundler from failing.
    #[cfg(not(feature = "test-harness"))]
    {
        if let Ok(profile) = env::var("PROFILE") {
            let target_dir = PathBuf::from(env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
                // Default target directory is at workspace root
                let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
                manifest_dir
                    .parent()
                    .unwrap()
                    .join("target")
                    .to_string_lossy()
                    .to_string()
            }));

            let bin_name = if cfg!(windows) {
                "rn-test-harness.exe"
            } else {
                "rn-test-harness"
            };

            let stub_path = target_dir.join(&profile).join(bin_name);

            // Only create stub if it doesn't already exist
            if !stub_path.exists() {
                println!("cargo:warning=Creating stub for rn-test-harness to satisfy bundler");
                // Create an empty file
                let _ = fs::write(&stub_path, "");
                // Make it executable on Unix
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&stub_path, fs::Permissions::from_mode(0o755));
                }
            }
        }
    }
}
