//! Local Ollama embeddings for semantic memory search (Issues #109, #103).
//!
//! Calls `POST /api/embed` on the local Ollama instance.
//! Gracefully falls back (returns Err) if OLLAMA_URL is not set or unreachable.
//!
//! Model: nomic-embed-text-v2-moe (768-dimensional output)

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Read the Ollama base URL from the `OLLAMA_URL` environment variable.
/// Defaults to `http://192.168.1.108:11434` if the variable is not set.
pub fn ollama_url() -> Option<String> {
    match std::env::var("OLLAMA_URL") {
        Ok(v) if !v.is_empty() => Some(v),
        _ => Some("http://192.168.1.108:11434".to_string()),
    }
}

/// Embed `text` via the local Ollama `/api/embed` endpoint.
///
/// Returns the first embedding vector on success, or an `Err(String)` if:
/// - `OLLAMA_URL` env var is missing / empty (currently defaults, so this won't trigger)
/// - The TCP connection fails
/// - The server returns a non-200 response
/// - The JSON response cannot be parsed
///
/// Uses a raw HTTP/1.1 request over `TcpStream` so there's no async dependency.
pub fn embed(text: &str) -> Result<Vec<f32>, String> {
    let base_url = ollama_url().ok_or_else(|| "OLLAMA_URL not set".to_string())?;

    // Parse host and port from URL like "http://host:port" or "http://host"
    let stripped = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let (host, port) = if let Some(colon) = stripped.rfind(':') {
        let h = &stripped[..colon];
        let p: u16 = stripped[colon + 1..]
            .split('/')
            .next()
            .unwrap_or("11434")
            .parse()
            .map_err(|e| format!("bad port in OLLAMA_URL: {e}"))?;
        (h.to_string(), p)
    } else {
        (stripped.to_string(), 11434u16)
    };

    // Build JSON body
    let body = format!(
        r#"{{"model":"nomic-embed-text-v2-moe","input":{}}}"#,
        serde_json::to_string(text).map_err(|e| format!("json encode error: {e}"))?
    );

    // Connect with a 2-second timeout
    let addr = format!("{host}:{port}");
    let stream = TcpStream::connect(&addr)
        .map_err(|e| format!("connect {addr} failed: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("set_read_timeout: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("set_write_timeout: {e}"))?;

    let request = format!(
        "POST /api/embed HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        len = body.len()
    );

    let mut stream = stream;
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write request: {e}"))?;

    // Read full response
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| format!("read response: {e}"))?;

    // Extract HTTP status
    let status_line = response.lines().next().unwrap_or("");
    // e.g. "HTTP/1.1 200 OK"
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if status_code != 200 {
        return Err(format!("Ollama returned HTTP {status_code}"));
    }

    // Find the JSON body after the blank line separating headers from body
    let json_body = if let Some(idx) = response.find("\r\n\r\n") {
        &response[idx + 4..]
    } else if let Some(idx) = response.find("\n\n") {
        &response[idx + 2..]
    } else {
        return Err("no HTTP body in response".to_string());
    };

    // Handle chunked transfer encoding: strip chunk size lines
    // If content is chunked, the body starts with a hex length, then CRLF, then data.
    // Simple approach: try direct parse first; if that fails, try stripping chunk headers.
    let json_str = if json_body.trim_start().starts_with('{') {
        json_body.to_string()
    } else {
        // Strip chunked encoding
        let mut result = String::new();
        let mut lines = json_body.lines();
        while let Some(line) = lines.next() {
            // Chunk size lines are hex digits
            if line.trim().chars().all(|c| c.is_ascii_hexdigit()) && !line.trim().is_empty() {
                if let Some(data) = lines.next() {
                    result.push_str(data);
                }
            } else {
                result.push_str(line);
            }
        }
        result
    };

    // Parse: {"embeddings": [[f32, ...]]}
    let v: serde_json::Value =
        serde_json::from_str(json_str.trim()).map_err(|e| format!("json parse: {e} — body: {json_str:.200}"))?;

    let embeddings = v
        .get("embeddings")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "missing 'embeddings' array".to_string())?;

    let first = embeddings
        .first()
        .and_then(|row| row.as_array())
        .ok_or_else(|| "embeddings[0] is not an array".to_string())?;

    let vec: Vec<f32> = first
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect();

    if vec.is_empty() {
        return Err("embedding vector is empty".to_string());
    }

    Ok(vec)
}

/// Compute cosine similarity between two vectors.
///
/// Returns 0.0 if either vector is empty, lengths differ, or both norms are zero.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Serialize a `Vec<f32>` to little-endian bytes for SQLite BLOB storage.
pub fn serialize_vec(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for &f in v {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

/// Deserialize little-endian bytes back to a `Vec<f32>`.
pub fn deserialize_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0f32, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_cosine_similarity_length_mismatch() {
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![0.0f32, 1.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let v = vec![1.0f32, 2.5, -0.5, 0.0];
        let bytes = serialize_vec(&v);
        let back = deserialize_vec(&bytes);
        assert_eq!(v.len(), back.len());
        for (a, b) in v.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_serialize_deserialize_empty() {
        let bytes = serialize_vec(&[]);
        let back = deserialize_vec(&bytes);
        assert!(back.is_empty());
    }

    #[test]
    fn test_embed_without_ollama_returns_err() {
        // Without a reachable server on port 1, embed should return Err
        std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1");
        let result = embed("test");
        // Reset
        std::env::remove_var("OLLAMA_URL");
        assert!(result.is_err(), "embed to bad URL should return Err");
    }

    #[test]
    fn test_ollama_url_default() {
        // Temporarily remove the env var if set
        let original = std::env::var("OLLAMA_URL").ok();
        std::env::remove_var("OLLAMA_URL");
        let url = ollama_url();
        assert!(url.is_some());
        assert!(url.unwrap().contains("192.168.1.108"));
        // Restore
        if let Some(v) = original {
            std::env::set_var("OLLAMA_URL", v);
        }
    }

    #[test]
    fn test_ollama_url_custom() {
        std::env::set_var("OLLAMA_URL", "http://10.0.0.1:8080");
        let url = ollama_url();
        std::env::remove_var("OLLAMA_URL");
        assert_eq!(url, Some("http://10.0.0.1:8080".to_string()));
    }
}
