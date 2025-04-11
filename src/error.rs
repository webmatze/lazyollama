// src/error.rs
// Defines custom error types for the application.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Terminal I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("API error: {0}")]
    Api(#[from] ApiError),

    #[error("Web scraping error: {0}")]
    Scraping(String),

    #[error("External command error: {0}")]
    Command(String),
    // Add other application-specific errors here if needed
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Network request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    Deserialization(#[from] serde_json::Error),

    #[error("API returned an error: {0}")]
    ResponseError(String),
    // Add specific API error variants if needed
}

// Define a type alias for Result using our AppError
pub type Result<T> = std::result::Result<T, AppError>;