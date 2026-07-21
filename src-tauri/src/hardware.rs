use serde::Serialize;
use std::process::Command;
use std::sync::OnceLock;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

/// Cross-platform hardware probe used to decide whether a model fits on this machine.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub os: String,
    pub arch: String,
    pub cpu_cores: usize,
    pub cpu_brand: String,
    pub total_ram_bytes: u64,
    pub gpu_backend: String,
    pub gpu_name: Option<String>,
    pub vram_bytes: Option<u64>,
}

fn detect_gpu_and_vram() -> (String, Option<String>, Option<u64>) {
    // Real GPU detection via WMI/CIM (works on Windows 10/11).
    // Single PowerShell call returns both GPU name and VRAM, avoiding temp-file races.
    let tmp = std::env::temp_dir().join(format!(
        "ragit_hw_probe_{}.ps1",
        std::process::id()
    ));
    let query = r#"
$v = Get-CimInstance Win32_VideoController | Select-Object -First 1
if ($v) {
    $v.Name
    $v.AdapterRAM
}
"#;
    if std::fs::write(&tmp, query).is_err() {
        return ("unknown".to_string(), None, None);
    }
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &tmp.to_string_lossy(),
        ])
        .output();
    let _ = std::fs::remove_file(&tmp);

    let (name, vram) = match out {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            let name = lines
                .first()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let vram = lines
                .get(1)
                .and_then(|s| s.trim().replace(',', "").parse::<u64>().ok());
            (name, vram)
        }
        Ok(o) => {
            eprintln!(
                "hardware probe: PowerShell exited with status {:?}, stderr: {}",
                o.status,
                String::from_utf8_lossy(&o.stderr).trim()
            );
            (None, None)
        }
        Err(e) => {
            eprintln!("hardware probe: failed to run PowerShell: {e}");
            (None, None)
        }
    };

    let (backend, name) = match name {
        Some(n) => {
            let b = if n.to_lowercase().contains("nvidia") {
                "CUDA".to_string()
            } else if n.to_lowercase().contains("amd") || n.to_lowercase().contains("radeon") {
                "ROCm".to_string()
            } else if n.to_lowercase().contains("intel") {
                "Intel".to_string()
            } else {
                "unknown".to_string()
            };
            (b, Some(n))
        }
        None => ("unknown".to_string(), None),
    };

    (backend, name, vram)
}

static HW_CACHE: OnceLock<HardwareInfo> = OnceLock::new();

/// Cached hardware probe — avoids repeated PowerShell calls on every catalog load.
pub fn probe_cached() -> &'static HardwareInfo {
    HW_CACHE.get_or_init(probe)
}

pub fn probe() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cores = sys.cpus().len().max(1);
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let total_ram_bytes = sys.total_memory(); // sysinfo 0.33 reports bytes

    let (gpu_backend, gpu_name, vram_bytes) = detect_gpu_and_vram();

    HardwareInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_cores: cores,
        cpu_brand,
        total_ram_bytes,
        gpu_backend,
        gpu_name,
        vram_bytes,
    }
}
