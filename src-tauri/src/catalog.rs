use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::hardware::HardwareInfo;

#[derive(Debug, Clone, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub repo: String,
    #[serde(rename = "default_file")]
    pub default_file: String,
    #[serde(default)]
    pub modalities: Vec<String>,
    #[serde(default)]
    pub context: usize,
    #[serde(default)]
    pub embed: bool,
    pub quants: HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Catalog {
    models: Vec<ModelEntry>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Fitness {
    /// Fits comfortably.
    Fits,
    /// Tight — will run but slowly / may spill to CPU.
    Tight,
    /// Will not fit.
    TooBig,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelVariant {
    pub quant: String,
    pub size_gb: f64,
    pub fitness: Fitness,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub repo: String,
    pub default_file: String,
    pub modalities: Vec<String>,
    pub context: usize,
    pub embed: bool,
    pub variants: Vec<ModelVariant>,
}

fn classify(size_bytes: u64, hw: &HardwareInfo) -> Fitness {
    let vram = hw.vram_bytes.unwrap_or(0);
    let ram = hw.total_ram_bytes;
    // Reserve 2 GB for the OS / other processes.
    let ram_avail = ram.saturating_sub(2u64 * 1024 * 1024 * 1024);

    if vram > 0 {
        if size_bytes <= vram / 2 {
            Fitness::Fits
        } else if size_bytes <= vram {
            Fitness::Tight
        } else if size_bytes <= ram_avail {
            Fitness::Tight // spills to CPU/RAM
        } else {
            Fitness::TooBig
        }
    } else if size_bytes <= ram_avail / 2 {
        Fitness::Fits
    } else if size_bytes <= ram_avail {
        Fitness::Tight
    } else {
        Fitness::TooBig
    }
}

pub fn load_catalog() -> Vec<CatalogModel> {
    let yaml = include_str!("../../catalog/models.yaml");
    let catalog: Catalog =
        serde_yaml::from_str(yaml).unwrap_or(Catalog { models: vec![] });
    let hw = crate::hardware::probe_cached();
    catalog
        .models
        .into_iter()
        .map(|m| {
            let variants = m
                .quants
                .iter()
                .map(|(quant, gb)| {
                    let size_bytes = (gb * 1024.0 * 1024.0 * 1024.0) as u64;
                    ModelVariant {
                        quant: quant.clone(),
                        size_gb: *gb,
                        fitness: classify(size_bytes, hw),
                    }
                })
                .collect();
            CatalogModel {
                id: m.id,
                name: m.name,
                repo: m.repo,
                default_file: m.default_file,
                modalities: m.modalities,
                context: m.context,
                embed: m.embed,
                variants,
            }
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct HfModel {
    id: String,
    #[serde(default)]
    pipeline_tag: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    downloads: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub repo: String,
    pub default_file: String,
    pub modalities: Vec<String>,
    pub context: usize,
    pub embed: bool,
    pub size_gb: f64,
    pub fitness: Fitness,
    pub downloads: u64,
}

/// Search HuggingFace for GGUF models by free-text query.
/// Returns best-effort matches with the largest GGUF file as the default.
pub fn search_hf(query: String) -> Vec<SearchHit> {
    let url = format!(
        "https://huggingface.co/api/models?search={}&filter=gguf&limit=25",
        urlencode(&query)
    );
    let client = reqwest::blocking::Client::builder()
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());
    let resp = match client.get(&url).send() {
        Ok(r) if r.status().is_success() => r,
        _ => return vec![],
    };
    let models: Vec<HfModel> = match resp.json() {
        Ok(m) => m,
        Err(_) => return vec![],
    };
    let hw = crate::hardware::probe_cached();
    models
        .into_iter()
        .map(|m| {
            let repo = m.id.clone();
            let modalities = infer_modalities(&m);
            let embed = m.tags.iter().any(|t| t.to_lowercase().contains("embed"))
                || repo.to_lowercase().contains("embed");
            // GGUF repos usually have a single primary file; we can't know exact
            // size without listing files, so estimate from downloads rank instead.
            let size_gb = estimate_size_gb(&m);
            let fitness = classify((size_gb * 1024.0 * 1024.0 * 1024.0) as u64, hw);
            SearchHit {
                id: repo.clone(),
                repo,
                default_file: guess_default_file(&m),
                modalities,
                context: 8192,
                embed,
                size_gb,
                fitness,
                downloads: m.downloads.unwrap_or(0),
            }
        })
        .collect()
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn infer_modalities(m: &HfModel) -> Vec<String> {
    let mut mods = vec!["text".to_string()];
    let all = format!("{:?}", m.tags);
    if all.to_lowercase().contains("vision") {
        mods.push("vision".to_string());
    }
    if all.to_lowercase().contains("audio") {
        mods.push("audio".to_string());
    }
    mods
}

fn guess_default_file(m: &HfModel) -> String {
    // Common GGUF naming: <Model>-Q4_K_M.gguf. We can't list files without
    // another request, so use a sensible default the user can edit.
    format!("{}-Q4_K_M.gguf", m.id.split('/').last().unwrap_or(&m.id))
}

fn estimate_size_gb(m: &HfModel) -> f64 {
    // Heuristic: smaller models are more common; rank by downloads if present.
    // Without file listing we can't know exactly, so default to a mid size.
    if m.tags.iter().any(|t| t.to_lowercase().contains("embedding")) {
        return 0.5;
    }
    4.0
}
