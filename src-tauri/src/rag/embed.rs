use serde_json::Value;

/// Calls the local llama-server /v1/embeddings endpoint.
/// Returns an embedding vector (dimensions depend on the loaded embed model).
pub fn embed(_port: u16, text: &str) -> Result<Vec<f32>, String> {
    let port = crate::engine::embed_port().ok_or("no embedding engine running")?;
    let url = format!("http://127.0.0.1:{port}/v1/embeddings");
    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({
        "model": "ragit-model",
        "input": [text],
        "encoding_format": "base64",
    });
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("embed HTTP {}", resp.status()));
    }
    let json: Value = resp.json().map_err(|e| e.to_string())?;
    let b64 = json
        .pointer("/data/0/embedding")
        .and_then(|v| v.as_str())
        .ok_or("no embedding in response")?;
    decode_base64_f32(b64)
}

fn decode_base64_f32(b64: &str) -> Result<Vec<f32>, String> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let bytes = STANDARD.decode(b64).map_err(|e| e.to_string())?;
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}
