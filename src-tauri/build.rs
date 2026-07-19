fn main() {
    tauri_build::build();

    // Copy zvec_c_api.dll next to the exe so it resolves at runtime.
    // zvec-rust-sys with `bundled` places it at:
    //   target/<profile>/build/zvec-rust-sys-<hash>/out/zvec-prebuilt/
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let target_dir = std::path::Path::new(&manifest)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target").join(&profile))
        .expect("target dir");
    let dll_name = "zvec_c_api.dll";
    let dest = target_dir.join(dll_name);
    if dest.exists() {
        return;
    }

    let build_dir = target_dir.parent().and_then(|t| t.parent()).map(|t| t.join("build")).expect("build dir");
    if let Ok(entries) = std::fs::read_dir(&build_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.contains("zvec-rust-sys") {
                let src = entry.path().join("out").join("zvec-prebuilt").join(dll_name);
                if src.exists() {
                    if std::fs::copy(&src, &dest).is_ok() {
                        println!("cargo:warning=zvec DLL copied to {}", dest.display());
                    }
                }
                break;
            }
        }
    }
}
