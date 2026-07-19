use std::time::Duration;

use serde_json::Value;

/// Returns an embedding vector for a single text.
pub fn embed(port: u16, text: &str) -> Result<Vec<f32>, String> {
    let dim = crate::engine::embed_dim();
    let vec = call_embed(port, &[text])?
        .into_iter()
        .next()
        .ok_or("empty embed response")?;
    if let Some(expected) = dim {
        if vec.len() != expected {
            return Err(format!(
                "embedding dimension mismatch: got {}, expected {}",
                vec.len(),
                expected
            ));
        }
    }
    Ok(vec)
}

/// Batch-embed multiple texts in a single API call. Returns one vector per text.
pub fn embed_batch(port: u16, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
    let dim = crate::engine::embed_dim();
    let results = call_embed(port, texts)?;
    if let Some(expected) = dim {
        for (i, v) in results.iter().enumerate() {
            if v.len() != expected {
                return Err(format!(
                    "embedding dimension mismatch at index {i}: got {}, expected {}",
                    v.len(),
                    expected
                ));
            }
        }
    }
    Ok(results)
}

fn call_embed(port: u16, inputs: &[&str]) -> Result<Vec<Vec<f32>>, String> {
    let url = format!("http://127.0.0.1:{port}/v1/embeddings");
    let client = reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({
        "model": "ragit-model",
        "input": inputs,
        "encoding_format": "base64",
    });
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| format!("embed request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("embed HTTP {status}: {text}"));
    }
    let json: Value = resp.json().map_err(|e| e.to_string())?;
    let data = json
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or("no data in embed response")?;
    let mut results = Vec::with_capacity(data.len());
    for item in data {
        let b64 = item
            .get("embedding")
            .and_then(|v| v.as_str())
            .ok_or("no embedding in data item")?;
        results.push(decode_base64_f32(b64)?);
    }
    if results.is_empty() {
        return Err("embed response had no data items".into());
    }
    Ok(results)
}

fn decode_base64_f32(b64: &str) -> Result<Vec<f32>, String> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let bytes = STANDARD.decode(b64).map_err(|e| e.to_string())?;
    if bytes.len() % 4 != 0 {
        return Err("embedding bytes not aligned to f32".into());
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}
