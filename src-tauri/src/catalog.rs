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
    let hw = crate::hardware::probe();
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
                        fitness: classify(size_bytes, &hw),
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
