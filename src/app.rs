// src/app.rs
// This module will contain the AppState struct and related logic.

use crate::ollama_api::{ModelInfo, ShowModelResponse};
use ratatui::widgets::ListState;

#[derive(Debug, PartialEq, Clone)]
pub enum AppMode {
    Normal,
    Filter, // New: Filter input mode
    ConfirmDelete,
    InstallSelectModel,
    InstallSelectTag,
    InstallConfirm,
    Installing,
    RunningOllama,
    Help,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub models: Vec<ModelInfo>,
    pub filtered_models: Vec<ModelInfo>,
    pub list_state: ListState,
    pub selected_model_details: Option<ShowModelResponse>,
    pub status_message: Option<String>,
    pub current_mode: AppMode,
    pub should_quit: bool,
    pub is_fetching_details: bool,

    // Filter-related fields
    pub filter_input: String,      // New: Current filter input
    pub is_filtered: bool,         // New: Whether filter is active
    pub filter_cursor_pos: usize,  // New: Cursor position in filter input

    // Registry-related fields
    pub registry_models: Vec<String>,
    pub registry_tags: Vec<String>,
    pub registry_model_list_state: ListState,
    pub registry_tag_list_state: ListState,
    pub selected_registry_model: Option<String>,
    pub selected_registry_tag: Option<String>,
    pub is_fetching_registry: bool,
    pub install_error: Option<String>,
    pub install_status: Option<String>,
    pub previous_mode: Option<AppMode>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            filtered_models: Vec::new(), // Initialize filtered models
            list_state: ListState::default(),
            selected_model_details: None,
            status_message: Some("Loading models...".to_string()),
            current_mode: AppMode::Normal,
            should_quit: false,
            is_fetching_details: false,

            // --- Initialize New filter fields ---
            filter_input: String::new(),
            is_filtered: false,
            filter_cursor_pos: 0,
            // --- End Initialize New filter fields ---

            // Registry fields
            registry_models: Vec::new(),
            registry_tags: Vec::new(),
            registry_model_list_state: ListState::default(),
            registry_tag_list_state: ListState::default(),
            selected_registry_model: None,
            selected_registry_tag: None,
            is_fetching_registry: false,
            install_error: None,
            install_status: None,
            previous_mode: None,
        }
    }

    pub fn get_current_models(&self) -> &[ModelInfo] {
        if self.is_filtered {
            &self.filtered_models
        } else {
            &self.models
        }
    }

    pub fn apply_filter(&mut self) {
        if self.filter_input.is_empty() {
            self.filtered_models.clear();
            self.is_filtered = false;
        } else {
            let filter_lower = self.filter_input.to_lowercase();
            self.filtered_models = self.models
                .iter()
                .filter(|model| model.name.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect();
            self.is_filtered = true;
        }

        let current_models = self.get_current_models();
        if current_models.is_empty() {
            self.list_state.select(None);
            self.selected_model_details = None;
        } else {
            self.list_state.select(Some(0));
            self.selected_model_details = None; // Clear to trigger refetch
            self.is_fetching_details = false;
        }
    }

    // Clear the filter
    pub fn clear_filter(&mut self) {
        self.filter_input.clear();
        self.filter_cursor_pos = 0;
        self.is_filtered = false;
        self.filtered_models.clear();
        
        // Reset selection to first item in full list
        if self.models.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
            self.selected_model_details = None;
            self.is_fetching_details = false;
        }
    }

    // Add character to filter input
    pub fn filter_input_char(&mut self, c: char) {
        self.filter_input.insert(self.filter_cursor_pos, c);
        self.filter_cursor_pos += 1;
        self.apply_filter();
    }

    // Remove character from filter input (backspace)
    pub fn filter_input_backspace(&mut self) {
        if self.filter_cursor_pos > 0 {
            self.filter_cursor_pos -= 1;
            self.filter_input.remove(self.filter_cursor_pos);
            self.apply_filter();
        }
    }

    pub fn filter_cursor_left(&mut self) {
        if self.filter_cursor_pos > 0 {
            self.filter_cursor_pos -= 1;
        }
    }

    pub fn filter_cursor_right(&mut self) {
        if self.filter_cursor_pos < self.filter_input.len() {
            self.filter_cursor_pos += 1;
        }
    }

    // Selects a model and clears existing details to trigger a fetch
    pub fn select_and_prepare_fetch(&mut self, index: Option<usize>) {
        let current_models = self.get_current_models();
        
        if current_models.is_empty() {
            self.list_state.select(None);
            self.selected_model_details = None;
            self.is_fetching_details = false;
        } else {
            let valid_index = index.unwrap_or(0).min(current_models.len() - 1);
            if self.list_state.selected() != Some(valid_index) || self.selected_model_details.is_none() {
                self.list_state.select(Some(valid_index));
                self.selected_model_details = None;
                self.status_message = Some("Fetching details...".to_string());
                self.is_fetching_details = false;
            }
        }
    }

    pub fn next_model(&mut self) {
        let current_models = self.get_current_models();
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= current_models.len().saturating_sub(1) {
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
        let current_models = self.get_current_models();
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    current_models.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.select_and_prepare_fetch(Some(i));
    }

    pub fn get_selected_model_name(&self) -> Option<String> {
        let current_models = self.get_current_models();
        self.list_state
            .selected()
            .and_then(|i| current_models.get(i))
            .map(|m| m.name.clone())
    }
}
