fn main() {
    tauri_build::build();

    // Copy the zvec shared library next to the exe so it resolves at runtime.
    // zvec-rust-sys with `bundled` downloads a prebuilt library to:
    //   target/<profile>/build/zvec-rust-sys-<hash>/out/zvec-prebuilt/
    //
    // On Windows: zvec_c_api.dll
    // On Linux:   libzvec_c_api.so
    // On macOS:   libzvec_c_api.dylib
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let target_dir = std::path::Path::new(&manifest)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target").join(&profile))
        .expect("target dir");

    let lib_name = if cfg!(windows) {
        "zvec_c_api.dll"
    } else if cfg!(target_os = "linux") {
        "libzvec_c_api.so"
    } else if cfg!(target_os = "macos") {
        "libzvec_c_api.dylib"
    } else {
        return;
    };

    let dest = target_dir.join(lib_name);
    if dest.exists() {
        return;
    }

    let build_dir = target_dir.parent().and_then(|t| t.parent()).map(|t| t.join("build")).expect("build dir");
    if let Ok(entries) = std::fs::read_dir(&build_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.contains("zvec-rust-sys") {
                let src = entry.path().join("out").join("zvec-prebuilt").join(lib_name);
                if src.exists() {
                    if std::fs::copy(&src, &dest).is_ok() {
                        println!("cargo:warning=zvec library copied to {}", dest.display());
                    }
                }
                break;
            }
        }
    }
}
