// src/ollama_api.rs
// Handles interactions with the Ollama REST API.

use crate::error::ApiError;
use humansize::{format_size, BINARY};
use serde::{Deserialize, Serialize};
use std::env;

const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";

// Structs matching Ollama API responses

#[derive(Deserialize, Debug, Clone)]
pub struct ListTagsResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
    // family: Option<String>, // Not present in /api/tags according to docs
    // format: Option<String>, // Not present in /api/tags
    // parameter_size: Option<String>, // Not present in /api/tags
    // quantization_level: Option<String>, // Not present in /api/tags
}

impl ModelInfo {
    pub fn size_formatted(&self) -> String {
        format_size(self.size, BINARY)
    }
    // Add methods to format other fields if needed
}


#[derive(Serialize, Debug)]
pub struct ShowModelRequest {
    pub name: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ShowModelResponse {
    pub license: Option<String>,
    pub modelfile: Option<String>,
    pub parameters: Option<String>,
    pub template: Option<String>,
    pub details: Option<ModelExtraDetails>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelExtraDetails {
    pub format: Option<String>,
    pub family: Option<String>,
    pub families: Option<Vec<String>>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
    // Added based on potential API output, adjust as needed
    pub parent_model: Option<String>,
    pub general: Option<GeneralDetails>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GeneralDetails {
   pub architecture: Option<String>,
   pub file_type: Option<u32>, // Example, adjust type if needed
   pub quantization_version: Option<u32>, // Example, adjust type if needed
   // Add other general fields if present
}


#[derive(Serialize, Debug)]
pub struct DeleteModelRequest {
    pub name: String,
}

// --- API Client Functions ---

pub fn get_ollama_host() -> String {
    // Consider using dotenvy here if needed
    env::var("OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string())
}

// Placeholder for the actual client implementation
#[derive(Clone)] // Added Clone
pub struct OllamaClient {
    client: reqwest::Client,
    host: String,
}

impl OllamaClient {
    pub fn new(host: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            host,
        }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, ApiError> {
        let url = format!("{}/api/tags", self.host);
        let res = self.client.get(&url).send().await?;

        if !res.status().is_success() {
            return Err(ApiError::ResponseError(format!(
                "API Error: {} - {}",
                res.status(),
                res.text().await.unwrap_or_else(|_| "Unknown error".to_string())
            )));
        }

        let body: ListTagsResponse = res.json().await?;
        Ok(body.models)
    }

    pub async fn show_model_details(&self, name: &str) -> Result<ShowModelResponse, ApiError> {
        let url = format!("{}/api/show", self.host);
        let request_body = ShowModelRequest { name: name.to_string() };
        let res = self.client.post(&url).json(&request_body).send().await?;

        if !res.status().is_success() {
            return Err(ApiError::ResponseError(format!(
                "API Error: {} - {}",
                res.status(),
                res.text().await.unwrap_or_else(|_| "Unknown error".to_string())
            )));
        }

        let body: ShowModelResponse = res.json().await?;
        Ok(body)
    }

    pub async fn delete_model(&self, name: &str) -> Result<(), ApiError> {
        let url = format!("{}/api/delete", self.host);
         let request_body = DeleteModelRequest { name: name.to_string() };
        let res = self.client.delete(&url).json(&request_body).send().await?; // Changed to DELETE

        if !res.status().is_success() {
             return Err(ApiError::ResponseError(format!(
                "API Error: {} - {}",
                res.status(),
                res.text().await.unwrap_or_else(|_| "Unknown error".to_string())
            )));
        }
        // Check for specific success status if needed, otherwise assume 2xx is OK
        Ok(())
    }
}