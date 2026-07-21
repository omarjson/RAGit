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

    // CARGO_MANIFEST_DIR is src-tauri/. The target dir is src-tauri/../target/<profile>.
    let target_dir = std::path::Path::new(&manifest)
        .parent()
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

    // Search ALL build directories (debug + release) for the zvec-rust-sys prebuilt library.
    // This handles the case where only one profile has been compiled.
    let target_parent = target_dir.parent().expect("target parent");
    if let Ok(entries) = std::fs::read_dir(target_parent) {
        for profile_entry in entries.flatten() {
            let build_dir = profile_entry.path().join("build");
            if let Ok(build_entries) = std::fs::read_dir(&build_dir) {
                for entry in build_entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.contains("zvec-rust-sys") {
                        let src = entry.path().join("out").join("zvec-prebuilt").join(lib_name);
                        if src.exists() {
                            if std::fs::copy(&src, &dest).is_ok() {
                                println!("cargo:warning=zvec library copied to {}", dest.display());
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    println!("cargo:warning=zvec library NOT found — ensure zvec-rust-sys is compiled for this profile");
}
