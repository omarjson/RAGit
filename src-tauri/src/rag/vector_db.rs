use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use zvec_rust::{
    initialize, Collection, CollectionSchema, ConfigBuilder, DataType, IndexParams, MetricType,
};

/// Initialize the zvec-rust runtime. Must be called once before any other zvec API.
pub fn init() -> Result<(), String> {
    let config = ConfigBuilder::new()
        .memory_limit(512 * 1024 * 1024)
        .num_threads(4)
        .build();
    initialize(Some(&config)).map_err(|e| format!("zvec_rust::initialize: {e}"))
}

pub struct ZvecPool {
    collections: Mutex<HashMap<String, Arc<Collection>>>,
    base_path: PathBuf,
    dim: AtomicU32,
}

impl ZvecPool {
    pub fn new(dim: u32) -> Result<Self, String> {
        let base_path = dirs::data_dir()
            .ok_or_else(|| "no data directory".to_string())?
            .join("ragit")
            .join("zvec");
        std::fs::create_dir_all(&base_path).map_err(|e| e.to_string())?;
        Ok(ZvecPool {
            collections: Mutex::new(HashMap::new()),
            base_path,
            dim: AtomicU32::new(dim),
        })
    }

    pub fn dim(&self) -> u32 {
        self.dim.load(Ordering::Relaxed)
    }

    pub fn set_dim(&self, dim: u32) {
        self.dim.store(dim, Ordering::Relaxed);
    }

    pub fn collection_for(&self, library_id: &str) -> Result<Arc<Collection>, String> {
        let mut map = self.collections.lock().map_err(|e| e.to_string())?;
        if let Some(coll) = map.get(library_id) {
            return Ok(Arc::clone(coll));
        }
        let dir = self.base_path.join(sanitize(library_id));
        let path_str = dir.to_str().ok_or_else(|| "invalid path".to_string())?;
        let coll = if dir.join("collection.zv").exists() {
            Collection::open(path_str, None).map_err(|e| e.to_string())?
        } else {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let dim = crate::engine::embed_dim().map(|d| d as u32).unwrap_or_else(|| self.dim());
            let schema = Self::build_schema(dim)?;
            Collection::create_and_open(path_str, &schema, None).map_err(|e| e.to_string())?
        };
        let arc = Arc::new(coll);
        map.insert(library_id.to_string(), Arc::clone(&arc));
        Ok(arc)
    }

    pub fn flush_all(&self) -> Result<(), String> {
        let map = self.collections.lock().map_err(|e| e.to_string())?;
        for coll in map.values() {
            coll.flush().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn optimize(&self, library_id: &str) -> Result<(), String> {
        let coll = self.collection_for(library_id)?;
        coll.optimize().map_err(|e| e.to_string())
    }

    pub fn remove_collection(&self, library_id: &str) {
        if let Ok(mut map) = self.collections.lock() {
            map.remove(library_id);
        }
    }

    fn build_schema(dim: u32) -> Result<CollectionSchema, String> {
        CollectionSchema::builder("ragit_library")
            .add_indexed_field(
                "library_id",
                DataType::String,
                IndexParams::invert(true, false).map_err(|e| e.to_string())?,
            )
            .add_indexed_field(
                "file_name",
                DataType::String,
                IndexParams::invert(true, false).map_err(|e| e.to_string())?,
            )
            .add_indexed_field(
                "chunk_index",
                DataType::Int64,
                IndexParams::invert(true, false).map_err(|e| e.to_string())?,
            )
            .add_indexed_field(
                "content",
                DataType::String,
                IndexParams::fts(Some("standard"), None, None).map_err(|e| e.to_string())?,
            )
            .add_indexed_field(
                "level",
                DataType::Int32,
                IndexParams::invert(true, false).map_err(|e| e.to_string())?,
            )
            .add_vector_field(
                "embedding",
                DataType::VectorFp32,
                dim,
                IndexParams::hnsw(MetricType::Cosine, 16, 200).map_err(|e| e.to_string())?,
            )
            .build()
            .map_err(|e| e.to_string())
    }
}

fn sanitize(s: &str) -> String {
    s.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_")
}
