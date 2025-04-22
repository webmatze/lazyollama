// src/registry_api.rs
// Functions for interacting with the Ollama registry website (scraping)

use crate::error::{ApiError, AppError, Result}; // Result is the alias for std::result::Result<T, AppError>
use scraper::{Html, Selector};

const REGISTRY_BASE_URL: &str = "https://registry.ollama.ai";

/// Fetches the list of available models from the Ollama registry library page.
pub async fn fetch_registry_models() -> Result<Vec<String>> { // Use Result alias
    let url = format!("{}/library", REGISTRY_BASE_URL);
    let html_content = reqwest::get(&url)
        .await
        .map_err(|e| AppError::Api(ApiError::Reqwest(e)))? // Map Reqwest error
        .text()
        .await
        .map_err(|e| AppError::Api(ApiError::Reqwest(e)))?; // Map Reqwest error

    let document = Html::parse_document(&html_content);
    // Selector targeting the links to model pages. Assumes href starts with /library/
    // Example: <a href="/library/llama3" ...>
    let model_link_selector = Selector::parse("a[href^='/library/']")
        .map_err(|e| AppError::Scraping(format!("Failed to parse model link selector: {}", e)))?;

    let mut models = Vec::new();
    for element in document.select(&model_link_selector) {
        if let Some(href) = element.value().attr("href") {
            let parts: Vec<&str> = href.split('/').collect();
            // Expecting href like "/library/modelname" or "/library/modelname/tags"
            // We only want the ones pointing directly to a model page (3 parts: "", "library", "modelname")
            if parts.len() == 3 && parts[1] == "library" && !parts[2].is_empty() {
                 // Avoid adding duplicates if the selector matches multiple elements per model
                 let model_name = parts[2].to_string();
                 if !models.contains(&model_name) {
                    models.push(model_name);
                 }
            }
        }
    }

    if models.is_empty() {
         // If selector failed or page structure changed, return an error
         Err(AppError::Scraping("Could not find or parse model names from registry page.".to_string()))
    } else {
        models.sort(); // Sort alphabetically
        Ok(models)
    }
}

/// Fetches the list of available tags for a specific model from the Ollama registry.
pub async fn fetch_registry_tags(model_name: &str) -> Result<Vec<String>> { // Use Result alias
    let url = format!("{}/library/{}/tags", REGISTRY_BASE_URL, model_name);
     let html_content = reqwest::get(&url)
        .await
        .map_err(|e| AppError::Api(ApiError::Reqwest(e)))? // Map Reqwest error
        .text()
        .await
        .map_err(|e| AppError::Api(ApiError::Reqwest(e)))?; // Add missing semicolon

    let document = Html::parse_document(&html_content);
    let tag_selector = Selector::parse("body > main > div > section > ul > li a") // Updated selector to target anchor tags directly
        .map_err(|e| AppError::Scraping(format!("Failed to parse tag selector: {}", e)))?;

    let mut tags = Vec::new();
    for element in document.select(&tag_selector) {
        let full_text = element.text().collect::<String>().trim().to_string();
        let tag_text = if let Some(pos) = full_text.find(':') {
            full_text[pos + 1..].to_string()
        } else {
            full_text
        };
        if !tag_text.is_empty() && !tags.contains(&tag_text) {
            tags.push(tag_text);
        }
    }

     if tags.is_empty() {
         // If selector failed, page structure changed, or no tags were found.
         Err(AppError::Scraping(format!(
             "Could not find/parse tags for model '{}'. (Selector might be outdated or model has no tags)", // Add hint
             model_name
         )))
    } else {
        // Filter out any tag that exactly matches the model_name
        tags.retain(|tag| tag != model_name);

        // Ensure "latest" tag exists, adding it if necessary
        if !tags.contains(&"latest".to_string()) {
            tags.push("latest".to_string());
        }

        // Sort all tags alphabetically
        tags.sort();

        // Move "latest" to the front
        if let Some(pos) = tags.iter().position(|t| t == "latest") {
            // Only move if it's not already at the front
            if pos > 0 {
            let latest_tag = tags.remove(pos);
            tags.insert(0, latest_tag);
            }
        }
        // If "latest" wasn't found (shouldn't happen here) or was already at pos 0, do nothing.

        Ok(tags)
    }
}