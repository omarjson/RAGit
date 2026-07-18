use serde::Serialize;
use std::process::Command;

/// Cross-platform hardware probe used to decide whether a model fits on this machine.
#[derive(Debug, Serialize, Clone)]
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

fn detect_os() -> String {
    std::env::consts::OS.to_string()
}

fn detect_arch() -> String {
    std::env::consts::ARCH.to_string()
}

fn detect_cpu() -> (usize, String) {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let brand = if let Ok(out) = Command::new("wmic")
        .args(["cpu", "get", "name"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        s.lines()
            .filter(|l| !l.trim().is_empty() && !l.contains("Name"))
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string()
    } else {
        "unknown".to_string()
    };
    (cores, brand)
}

fn detect_ram() -> u64 {
    if let Ok(out) = Command::new("wmic")
        .args(["ComputerSystem", "get", "TotalPhysicalMemory"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(line) = s
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.contains("Total"))
            .next()
        {
            return line.trim().parse::<u64>().unwrap_or(0);
        }
    }
    0
}

fn detect_gpu() -> (String, Option<String>, Option<u64>) {
    // Best-effort: report the first discrete/integrated GPU we can find.
    if let Ok(out) = Command::new("wmic")
        .args(["path", "win32_VideoController", "get", "name,AdapterRAM"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.contains("name") || line.contains("AdapterRAM") {
                continue;
            }
            let mut parts = line.split_whitespace();
            let ram = parts.next_back().and_then(|v| v.parse::<u64>().ok());
            let name = line
                .trim_end_matches(|c: char| c.is_numeric() || c.is_whitespace())
                .trim()
                .to_string();
            if !name.is_empty() {
                return ("unknown".to_string(), Some(name), ram);
            }
        }
    }
    ("unknown".to_string(), None, None)
}

pub fn probe() -> HardwareInfo {
    let (cores, cpu_brand) = detect_cpu();
    let (gpu_backend, gpu_name, vram) = detect_gpu();
    HardwareInfo {
        os: detect_os(),
        arch: detect_arch(),
        cpu_cores: cores,
        cpu_brand,
        total_ram_bytes: detect_ram(),
        gpu_backend,
        gpu_name,
        vram_bytes: vram,
    }
}
