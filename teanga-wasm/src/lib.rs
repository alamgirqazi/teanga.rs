// teanga-wasm/src/lib.rs
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use web_sys::console;

// Import the actual Teanga types but only the in-memory ones for WASM
use teanga::{
    SimpleCorpus, LayerType, DataType, Layer, Corpus, ReadableCorpus, WriteableCorpus,
    LayerDesc, Document, Value, TeangaError
};

// Setup panic hook for better debugging
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console::log_1(&"🦀 Teanga WASM module initialized".into());
}

// JavaScript-friendly error type
#[wasm_bindgen]
pub struct WasmError {
    message: String,
}

#[wasm_bindgen]
impl WasmError {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

impl From<TeangaError> for WasmError {
    fn from(err: TeangaError) -> Self {
        WasmError {
            message: format!("{}", err),
        }
    }
}

impl From<serde_json::Error> for WasmError {
    fn from(err: serde_json::Error) -> Self {
        WasmError {
            message: format!("JSON error: {}", err),
        }
    }
}

// Main WASM wrapper for Teanga corpus
#[wasm_bindgen]
pub struct TeangaWasm {
    corpus: SimpleCorpus,
}

#[wasm_bindgen]
impl TeangaWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> TeangaWasm {
        console::log_1(&"Creating new Teanga corpus in Rust/WASM".into());
        TeangaWasm {
            corpus: SimpleCorpus::new(),
        }
    }

    #[wasm_bindgen]
    pub fn add_layer_meta(
        &mut self,
        name: &str,
        layer_type: &str,
        base: Option<String>,
        data_type: Option<String>,
    ) -> Result<(), WasmError> {
        let layer_type = match layer_type {
            "characters" => LayerType::characters,
            "span" => LayerType::span,
            "seq" => LayerType::seq,
            "div" => LayerType::div,
            "element" => LayerType::element,
            _ => return Err(WasmError { 
                message: format!("Invalid layer type: {}", layer_type) 
            }),
        };

        let data = match data_type.as_deref() {
            Some("string") => Some(DataType::String),
            Some("link") => Some(DataType::Link),
            Some(enum_str) if enum_str.starts_with('[') => {
                let values: Vec<String> = serde_json::from_str(enum_str)?;
                Some(DataType::Enum(values))
            }
            None => None,
            Some(other) => return Err(WasmError { 
                message: format!("Invalid data type: {}", other) 
            }),
        };

        self.corpus.add_layer_meta(
            name.to_string(),
            layer_type.clone(),
            base,
            data,
            None, // link_types
            None, // target
            None, // default
            HashMap::new(), // meta
        )?;

        console::log_1(&format!("✅ Added layer: {} ({})", name, layer_type).into());
        Ok(())
    }

    #[wasm_bindgen]
    pub fn add_doc(&mut self, doc_json: &str) -> Result<String, WasmError> {
        // Parse the JSON into a map
        let doc_data: HashMap<String, serde_json::Value> = serde_json::from_str(doc_json)?;

        // Convert JSON values to Teanga layers
        let mut layers = HashMap::new();
        for (key, value) in doc_data {
            let layer = self.json_value_to_layer(value)?;
            layers.insert(key, layer);
        }

        let doc_id = self.corpus.add_doc(layers)?;
        console::log_1(&format!("📄 Added document: {}", doc_id).into());
        Ok(doc_id)
    }

    #[wasm_bindgen]
    pub fn get_doc_by_id(&self, id: &str) -> Result<String, WasmError> {
        let doc = self.corpus.get_doc_by_id(id)?;
        
        // Convert document to JSON-serializable format
        let mut doc_map = HashMap::new();
        for (key, layer) in &doc.content {
            doc_map.insert(key.clone(), self.layer_to_json_value(layer));
        }
        
        let json = serde_json::to_string(&doc_map)?;
        Ok(json)
    }

    #[wasm_bindgen]
    pub fn get_doc_ids(&self) -> String {
        let ids = self.corpus.get_docs();
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn get_meta(&self) -> String {
        // Convert metadata to JSON-serializable format
        let mut meta_map = HashMap::new();
        for (name, layer_desc) in self.corpus.get_meta() {
            let mut desc_map = HashMap::new();
            desc_map.insert("layer_type".to_string(), 
                serde_json::Value::String(format!("{}", layer_desc.layer_type)));
            
            if let Some(ref base) = layer_desc.base {
                desc_map.insert("base".to_string(), serde_json::Value::String(base.clone()));
            }
            
            if let Some(ref data) = layer_desc.data {
                let data_value = match data {
                    DataType::String => serde_json::Value::String("string".to_string()),
                    DataType::Link => serde_json::Value::String("link".to_string()),
                    DataType::Enum(vals) => serde_json::Value::Array(
                        vals.iter().map(|v| serde_json::Value::String(v.clone())).collect()
                    ),
                };
                desc_map.insert("data".to_string(), data_value);
            }
            
            meta_map.insert(name.clone(), serde_json::Value::Object(
                desc_map.into_iter().collect()
            ));
        }
        
        serde_json::to_string(&meta_map).unwrap_or_else(|_| "{}".to_string())
    }

    #[wasm_bindgen]
    pub fn tokenize_simple(&self, text: &str) -> String {
        let tokens = simple_tokenize(text);
        serde_json::to_string(&tokens).unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn to_yaml(&self) -> Result<String, WasmError> {
        // Generate YAML manually since serde_yaml might not work well in WASM
        let mut yaml = String::new();
        
        // Add metadata
        yaml.push_str("_meta:\n");
        for (name, layer_desc) in self.corpus.get_meta() {
            yaml.push_str(&format!("  {}:\n", name));
            yaml.push_str(&format!("    type: {}\n", layer_desc.layer_type));
            
            if let Some(ref base) = layer_desc.base {
                yaml.push_str(&format!("    base: {}\n", base));
            }
            
            if let Some(ref data) = layer_desc.data {
                match data {
                    DataType::String => yaml.push_str("    data: string\n"),
                    DataType::Link => yaml.push_str("    data: link\n"),
                    DataType::Enum(values) => {
                        yaml.push_str(&format!("    data: {:?}\n", values));
                    }
                }
            }
        }
        
        // Add documents
        for doc_id in self.corpus.get_docs() {
            if let Ok(doc) = self.corpus.get_doc_by_id(&doc_id) {
                yaml.push_str(&format!("{}:\n", doc_id));
                for (layer_name, layer) in &doc.content {
                    match layer {
                        Layer::Characters(text) => {
                            let escaped = text.replace("\"", "\\\"").replace("\n", "\\n");
                            yaml.push_str(&format!("  {}: \"{}\"\n", layer_name, escaped));
                        }
                        other => {
                            let json_val = self.layer_to_json_value(other);
                            yaml.push_str(&format!("  {}: {}\n", layer_name, 
                                serde_json::to_string(&json_val).unwrap_or("null".to_string())));
                        }
                    }
                }
            }
        }
        
        Ok(yaml)
    }

    #[wasm_bindgen]
    pub fn corpus_info(&self) -> String {
        let meta = self.corpus.get_meta();
        let docs = self.corpus.get_docs();
        
        let info = serde_json::json!({
            "layer_count": meta.len(),
            "document_count": docs.len(),
            "layer_names": meta.keys().collect::<Vec<_>>(),
            "document_ids": docs,
            "implementation": "Rust WASM"
        });
        
        serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string())
    }

    // Helper methods
    fn json_value_to_layer(&self, value: serde_json::Value) -> Result<Layer, WasmError> {
        match value {
            serde_json::Value::String(text) => Ok(Layer::Characters(text)),
            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    return Ok(Layer::L1(vec![]));
                }
                
                match &arr[0] {
                    serde_json::Value::Number(_) => {
                        // Array of numbers -> L1
                        let nums: Result<Vec<u32>, _> = arr.iter()
                            .map(|v| v.as_u64().map(|n| n as u32).ok_or_else(|| 
                                WasmError { message: "Expected number".to_string() }))
                            .collect();
                        Ok(Layer::L1(nums?))
                    }
                    serde_json::Value::Array(inner) => {
                        // Array of arrays
                        if inner.len() == 2 {
                            // Span layer [[start, end], ...]
                            let spans: Result<Vec<(u32, u32)>, _> = arr.iter()
                                .map(|v| {
                                    let inner_arr = v.as_array().ok_or_else(|| 
                                        WasmError { message: "Expected array".to_string() })?;
                                    if inner_arr.len() >= 2 {
                                        let start = inner_arr[0].as_u64().ok_or_else(|| 
                                            WasmError { message: "Expected number".to_string() })? as u32;
                                        let end = inner_arr[1].as_u64().ok_or_else(|| 
                                            WasmError { message: "Expected number".to_string() })? as u32;
                                        Ok((start, end))
                                    } else {
                                        Err(WasmError { message: "Expected array of length >= 2".to_string() })
                                    }
                                })
                                .collect();
                            Ok(Layer::L2(spans?))
                        } else if inner.len() == 3 {
                            // Triple array
                            let triples: Result<Vec<(u32, u32, u32)>, _> = arr.iter()
                                .map(|v| {
                                    let inner_arr = v.as_array().ok_or_else(|| 
                                        WasmError { message: "Expected array".to_string() })?;
                                    if inner_arr.len() >= 3 {
                                        let a = inner_arr[0].as_u64().ok_or_else(|| 
                                            WasmError { message: "Expected number".to_string() })? as u32;
                                        let b = inner_arr[1].as_u64().ok_or_else(|| 
                                            WasmError { message: "Expected number".to_string() })? as u32;
                                        let c = inner_arr[2].as_u64().ok_or_else(|| 
                                            WasmError { message: "Expected number".to_string() })? as u32;
                                        Ok((a, b, c))
                                    } else {
                                        Err(WasmError { message: "Expected array of length >= 3".to_string() })
                                    }
                                })
                                .collect();
                            Ok(Layer::L3(triples?))
                        } else {
                            Err(WasmError { message: "Unsupported array structure".to_string() })
                        }
                    }
                    serde_json::Value::String(_) => {
                        // Array of strings -> LS
                        let strings: Result<Vec<String>, _> = arr.iter()
                            .map(|v| v.as_str().map(|s| s.to_string()).ok_or_else(|| 
                                WasmError { message: "Expected string".to_string() }))
                            .collect();
                        Ok(Layer::LS(strings?))
                    }
                    _ => Err(WasmError { message: "Unsupported array content".to_string() }),
                }
            }
            _ => Err(WasmError { message: "Unsupported value type".to_string() }),
        }
    }

    fn layer_to_json_value(&self, layer: &Layer) -> serde_json::Value {
        match layer {
            Layer::Characters(text) => serde_json::Value::String(text.clone()),
            Layer::L1(data) => serde_json::Value::Array(
                data.iter().map(|&n| serde_json::Value::Number(n.into())).collect()
            ),
            Layer::L2(data) => serde_json::Value::Array(
                data.iter().map(|&(a, b)| serde_json::Value::Array(vec![
                    serde_json::Value::Number(a.into()),
                    serde_json::Value::Number(b.into())
                ])).collect()
            ),
            Layer::L3(data) => serde_json::Value::Array(
                data.iter().map(|&(a, b, c)| serde_json::Value::Array(vec![
                    serde_json::Value::Number(a.into()),
                    serde_json::Value::Number(b.into()),
                    serde_json::Value::Number(c.into())
                ])).collect()
            ),
            Layer::LS(data) => serde_json::Value::Array(
                data.iter().map(|s| serde_json::Value::String(s.clone())).collect()
            ),
            Layer::L1S(data) => serde_json::Value::Array(
                data.iter().map(|(n, s)| serde_json::Value::Array(vec![
                    serde_json::Value::Number((*n).into()),
                    serde_json::Value::String(s.clone())
                ])).collect()
            ),
            Layer::L2S(data) => serde_json::Value::Array(
                data.iter().map(|(a, b, s)| serde_json::Value::Array(vec![
                    serde_json::Value::Number((*a).into()),
                    serde_json::Value::Number((*b).into()),
                    serde_json::Value::String(s.clone())
                ])).collect()
            ),
            Layer::L3S(data) => serde_json::Value::Array(
                data.iter().map(|(a, b, c, s)| serde_json::Value::Array(vec![
                    serde_json::Value::Number((*a).into()),
                    serde_json::Value::Number((*b).into()),
                    serde_json::Value::Number((*c).into()),
                    serde_json::Value::String(s.clone())
                ])).collect()
            ),
            Layer::MetaLayer(data) => {
                // Convert Value to serde_json::Value
                match data {
                    Some(val) => self.value_to_json_value(val),
                    None => serde_json::Value::Null,
                }
            }
        }
    }

    fn value_to_json_value(&self, value: &Value) -> serde_json::Value {
        match value {
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Int(i) => serde_json::Value::Number((*i).into()),
            Value::Float(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Array(arr) => serde_json::Value::Array(
                arr.iter().map(|v| self.value_to_json_value(v)).collect()
            ),
            Value::Object(obj) => serde_json::Value::Object(
                obj.iter().map(|(k, v)| (k.clone(), self.value_to_json_value(v))).collect()
            ),
        }
    }
}

// Simple tokenization function
fn simple_tokenize(text: &str) -> Vec<(u32, u32)> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut in_word = false;
    
    for (i, ch) in text.char_indices() {
        if ch.is_alphabetic() || ch.is_numeric() {
            if !in_word {
                start = i;
                in_word = true;
            }
        } else {
            if in_word {
                tokens.push((start as u32, i as u32));
                in_word = false;
            }
            if !ch.is_whitespace() {
                // Add punctuation as separate token
                tokens.push((i as u32, (i + ch.len_utf8()) as u32));
            }
        }
    }
    
    // Handle final token
    if in_word {
        tokens.push((start as u32, text.len() as u32));
    }
    
    tokens
}