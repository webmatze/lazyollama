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
    let tag_selector = Selector::parse("body > main > div > section > div > div > div > div > div.flex.space-x-2.items-center > a > div") // This is a guess, might need refinement
        .map_err(|e| AppError::Scraping(format!("Failed to parse tag selector: {}", e)))?;

    let mut tags = Vec::new();
    for element in document.select(&tag_selector) {
        let tag_text = element.text().collect::<String>().trim().to_string();
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
        // Often 'latest' is present, maybe sort it to the top? Or just alphabetical.
        tags.sort();
        Ok(tags)
    }
}