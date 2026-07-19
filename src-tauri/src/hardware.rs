use serde::Serialize;
use std::process::Command;
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

fn detect_gpu() -> (String, Option<String>, Option<u64>) {
    // Real GPU detection via WMI/CIM (works on Windows 10/11).
    let tmp = std::env::temp_dir().join("ragit_hw_probe.ps1");
    let query = "(Get-CimInstance Win32_VideoController | Select-Object -First 1).Name";
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

    let name = match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        }
        _ => None,
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

    (backend, name, None)
}

fn detect_vram() -> Option<u64> {
    // sysinfo does not expose VRAM on Windows reliably, so best-effort via
    // PowerShell / CIM. Returns None if it cannot be determined.
    let tmp = std::env::temp_dir().join("ragit_hw_probe.ps1");
    let query = "Get-CimInstance Win32_VideoController | Where-Object { $_.AdapterRAM } | Select-Object -First 1 | ForEach-Object { $_.AdapterRAM }";
    if std::fs::write(&tmp, query).is_err() {
        return None;
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
    if let Ok(out) = out {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            return s
                .trim()
                .replace(',', "")
                .parse::<u64>()
                .ok();
        }
    }
    None
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

    let (gpu_backend, gpu_name, _) = detect_gpu();
    let vram_bytes = detect_vram();

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
