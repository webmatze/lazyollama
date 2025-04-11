// src/app.rs
// This module will contain the AppState struct and related logic.

use crate::ollama_api::{ModelInfo, ShowModelResponse};
use ratatui::widgets::ListState;

#[derive(Debug, PartialEq, Clone)]
pub enum AppMode {
    Normal,
    ConfirmDelete,
    InstallSelectModel, // New: Selecting model from registry list
    InstallSelectTag,   // New: Selecting tag for the chosen model
    InstallConfirm,     // New: Confirming the installation
    Installing,         // New: Installation in progress
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub ollama_host: String,
    pub models: Vec<ModelInfo>,
    pub list_state: ListState,
    pub selected_model_details: Option<ShowModelResponse>,
    pub status_message: Option<String>,
    pub current_mode: AppMode,
    pub should_quit: bool,
    pub is_fetching_details: bool, // Added flag

    // --- New fields for Installation ---
    pub registry_models: Vec<String>, // List of models from registry
    pub registry_tags: Vec<String>,   // List of tags for selected registry model
    pub registry_model_list_state: ListState, // State for registry model list
    pub registry_tag_list_state: ListState,   // State for registry tag list
    pub selected_registry_model: Option<String>, // Model selected from registry
    pub selected_registry_tag: Option<String>,   // Tag selected for the model
    pub is_fetching_registry: bool, // Flag for loading state (models/tags)
    pub install_error: Option<String>, // To store/display install-related errors
    pub install_status: Option<String>, // To show "Pulling..." message
                                        // --- End New fields ---
}

impl AppState {
    pub fn new(host: String) -> Self {
        Self {
            ollama_host: host,
            models: Vec::new(),
            list_state: ListState::default(),
            selected_model_details: None,
            status_message: Some("Loading models...".to_string()),
            current_mode: AppMode::Normal,
            should_quit: false,
            is_fetching_details: false, // Initialize flag
            // --- Initialize New fields ---
            registry_models: Vec::new(),
            registry_tags: Vec::new(),
            registry_model_list_state: ListState::default(),
            registry_tag_list_state: ListState::default(),
            selected_registry_model: None,
            selected_registry_tag: None,
            is_fetching_registry: false,
            install_error: None,
            install_status: None,
            // --- End Initialize New fields ---
        }
    }

    // Selects a model and clears existing details to trigger a fetch
    pub fn select_and_prepare_fetch(&mut self, index: Option<usize>) { // Added pub
        if self.models.is_empty() {
            self.list_state.select(None);
            self.selected_model_details = None;
            self.is_fetching_details = false; // No fetch needed if empty
        } else {
            let valid_index = index.unwrap_or(0).min(self.models.len() - 1);
            // Only clear and fetch if selection actually changes or details are missing
            if self.list_state.selected() != Some(valid_index) || self.selected_model_details.is_none() {
                self.list_state.select(Some(valid_index));
                self.selected_model_details = None; // Clear details on selection change
                self.status_message = Some("Fetching details...".to_string()); // Indicate loading
                self.is_fetching_details = false; // Reset flag to allow fetching
            }
        }
    }


    pub fn next_model(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.models.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.select_and_prepare_fetch(Some(i));
    }

    pub fn previous_model(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.models.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.select_and_prepare_fetch(Some(i));
    }

     pub fn get_selected_model_name(&self) -> Option<String> {
        self.list_state
            .selected()
            .and_then(|i| self.models.get(i))
            .map(|m| m.name.clone())
    }
}