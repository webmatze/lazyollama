// src/main.rs

mod app;
mod error;
mod ollama_api;
mod registry_api;
mod ui;

use crate::{
    app::{AppMode, AppState},
    error::{AppError, Result},
    ollama_api::{OllamaClient, ShowModelResponse, ModelInfo},
    // registry_api is used via `mod registry_api`
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{io, time::Duration};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let ollama_host = ollama_api::get_ollama_host();
    // Client needs to be cloneable
    let client = OllamaClient::new(ollama_host.clone());
    let mut app_state = AppState::new(ollama_host);

    let res = run_app(&mut terminal, client, &mut app_state).await; // Pass client by value

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error running app: {:?}", err);
        // Consider returning the error if main should indicate failure
        // return Err(err);
    }

    Ok(())
}

// Define the types of events that can be sent from async tasks to the main loop
#[derive(Debug)]
enum AppEvent {
    // Results here should use the top-level AppError
    ModelDetailsFetched(Result<ShowModelResponse>),
    RegistryModelsFetched(Result<Vec<String>>),
    RegistryTagsFetched(Result<Vec<String>>),
    ModelPullCompleted(Result<()>),
    LocalModelsRefreshed(Result<Vec<ModelInfo>>),
}
async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    client: OllamaClient, // Take client by value as it's cloneable
    app: &mut AppState,
) -> Result<()> {
    // Channel for communication between async tasks and the main loop
    let (tx, mut rx) = mpsc::channel::<AppEvent>(32); // Increased buffer size for potentially concurrent events

    // Initial model fetch
    match client.list_models().await {
        Ok(models) => {
            app.models = models;
            if !app.models.is_empty() {
                app.list_state.select(Some(0));
                // Trigger fetch for the initially selected model
                app.selected_model_details = None;
                app.is_fetching_details = false; // Allow initial fetch
            }
            app.status_message = None;
        }
        Err(e) => {
            app.status_message = Some(format!("Error loading models: {}", e));
        }
    }

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // --- Start Background Fetch if Needed ---
        if app.list_state.selected().is_some()
            && app.selected_model_details.is_none()
            && !app.is_fetching_details
        {
            if let Some(name) = app.get_selected_model_name() {
                app.is_fetching_details = true;
                app.status_message = Some("Fetching details...".to_string()); // Show status

                let client_clone = client.clone();
                let tx_clone = tx.clone();

                tokio::spawn(async move {
                    let result = client_clone.show_model_details(&name).await;
                    // Send result back to the main loop, ignore error if receiver dropped
                    // Map ApiError to AppError before sending
                    let _ = tx_clone.send(AppEvent::ModelDetailsFetched(result.map_err(AppError::Api))).await;
                });
            }
        }
        // --- End Background Fetch ---

        // --- Start Background Registry Fetch if Needed ---
        // (Triggered by input handling below)

        // --- Start Background Pull if Needed ---
        // (Triggered by input handling below)

        // --- Handle Input and Channel Events ---
        // Poll for events with a smaller timeout to remain responsive
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                    // --- Input Handling Logic ---
                    let current_mode = app.current_mode.clone(); // Clone mode for matching
                    match current_mode {
                        AppMode::Normal => match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Char('j') | KeyCode::Down => app.next_model(),
                            KeyCode::Char('k') | KeyCode::Up => app.previous_model(),
                            KeyCode::Char('d') => {
                                if app.list_state.selected().is_some() {
                                    app.current_mode = AppMode::ConfirmDelete;
                                    app.status_message = None; // Clear status for confirm prompt
                                }
                            }
                            KeyCode::Char('i') => {
                                // --- Start Install Flow ---
                                app.current_mode = AppMode::InstallSelectModel;
                                app.is_fetching_registry = true;
                                app.install_error = None; // Clear previous errors
                                app.registry_models.clear(); // Clear old list
                                app.registry_model_list_state.select(None); // Reset selection

                                let tx_clone = tx.clone();
                                tokio::spawn(async move {
                                    // Call the actual registry fetching function
                                    let result = registry_api::fetch_registry_models().await;
                                    let _ = tx_clone.send(AppEvent::RegistryModelsFetched(result)).await;
                                });
                            }
                            _ => {}
                        },
                        AppMode::ConfirmDelete => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                if let Some(name) = app.get_selected_model_name() {
                                    app.status_message = Some(format!("Deleting {}...", name));
                                    terminal.draw(|f| ui::draw(f, app))?; // Show deleting message immediately

                                    let client_clone = client.clone();
                                    let tx_clone = tx.clone();
                                    let model_name_clone = name.clone();

                                    tokio::spawn(async move {
                                        match client_clone.delete_model(&model_name_clone).await {
                                            Ok(_) => {
                                                // Request refresh after delete
                                                let refresh_result = client_clone.list_models().await;
                                                // Map ApiError to AppError before sending
                                                let _ = tx_clone.send(AppEvent::LocalModelsRefreshed(refresh_result.map_err(AppError::Api))).await;
                                            }
                                            Err(e) => {
                                                 // Send error back if delete fails
                                                 // We might want a specific AppEvent for delete errors
                                                 // For now, reuse pull completed with error
                                                 // Map ApiError to AppError before sending
                                                 let _ = tx_clone.send(AppEvent::ModelPullCompleted(Err(AppError::Api(e)))).await;
                                            }
                                        }
                                    });
                                }
                                app.current_mode = AppMode::Normal; // Return to normal while delete happens
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.current_mode = AppMode::Normal;
                                app.status_message = None;
                            }
                            _ => {}
                        },
                        AppMode::InstallSelectModel => match key.code {
                            KeyCode::Char('j') | KeyCode::Down => {
                                let len = app.registry_models.len();
                                if len > 0 {
                                    let i = match app.registry_model_list_state.selected() {
                                        Some(i) => (i + 1) % len,
                                        None => 0,
                                    };
                                    app.registry_model_list_state.select(Some(i));
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                let len = app.registry_models.len();
                                if len > 0 {
                                    let i = match app.registry_model_list_state.selected() {
                                        Some(i) => (i + len - 1) % len,
                                        None => len - 1,
                                    };
                                    app.registry_model_list_state.select(Some(i));
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(selected_index) = app.registry_model_list_state.selected() {
                                    if let Some(model_name) = app.registry_models.get(selected_index).cloned() {
                                        app.selected_registry_model = Some(model_name.clone());
                                        app.current_mode = AppMode::InstallSelectTag;
                                        app.is_fetching_registry = true;
                                        app.install_error = None;
                                        app.registry_tags.clear();
                                        app.registry_tag_list_state.select(None);

                                        let tx_clone = tx.clone();
                                        tokio::spawn(async move {
                                            // Call the actual tag fetching function
                                            let result = registry_api::fetch_registry_tags(&model_name).await;
                                            let _ = tx_clone.send(AppEvent::RegistryTagsFetched(result)).await;
                                        });
                                    }
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.current_mode = AppMode::Normal;
                                app.install_error = None;
                                app.is_fetching_registry = false; // Stop any potential fetch display
                            }
                            _ => {}
                        },
                        AppMode::InstallSelectTag => match key.code {
                             KeyCode::Char('j') | KeyCode::Down => {
                                let len = app.registry_tags.len();
                                if len > 0 {
                                    let i = match app.registry_tag_list_state.selected() {
                                        Some(i) => (i + 1) % len,
                                        None => 0,
                                    };
                                    app.registry_tag_list_state.select(Some(i));
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                let len = app.registry_tags.len();
                                if len > 0 {
                                    let i = match app.registry_tag_list_state.selected() {
                                        Some(i) => (i + len - 1) % len,
                                        None => len - 1,
                                    };
                                    app.registry_tag_list_state.select(Some(i));
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(selected_index) = app.registry_tag_list_state.selected() {
                                     if let Some(tag_name) = app.registry_tags.get(selected_index).cloned() {
                                        app.selected_registry_tag = Some(tag_name);
                                        app.current_mode = AppMode::InstallConfirm;
                                        app.install_error = None;
                                     }
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.current_mode = AppMode::InstallSelectModel;
                                app.selected_registry_model = None; // Clear selection
                                app.registry_tags.clear();
                                app.install_error = None;
                                app.is_fetching_registry = false;
                            }
                            _ => {}
                        },
                         AppMode::InstallConfirm => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                if let (Some(model), Some(tag)) = (app.selected_registry_model.clone(), app.selected_registry_tag.clone()) {
                                    app.current_mode = AppMode::Installing;
                                    app.install_status = Some(format!("Starting pull for {}:{}...", model, tag));
                                    app.install_error = None;

                                    let tx_clone = tx.clone();
                                    let client_clone_for_refresh = client.clone(); // Clone client for refresh task later
                                    tokio::spawn(async move {
                                        // Restore terminal temporarily to show pull output
                                        let _ = disable_raw_mode();
                                        let _ = execute!(io::stdout(), LeaveAlternateScreen);

                                        println!("\n--- Starting 'ollama pull {}:{}' ---", model, tag);
                                        println!("--- (Application will resume after pull completes) ---");

                                        let command_result = tokio::process::Command::new("ollama")
                                            .arg("pull")
                                            .arg(format!("{}:{}", model, tag))
                                            .status() // Wait for the command to complete
                                            .await;

                                        // Re-enable raw mode and enter alternate screen
                                        let _ = execute!(io::stdout(), EnterAlternateScreen);
                                        let _ = enable_raw_mode();
                                        println!("--- Pull command finished ---"); // This won't be visible in alt screen

                                        let pull_result = match command_result {
                                            Ok(status) => {
                                                if status.success() {
                                                    Ok(())
                                                } else {
                                                    // Use AppError::Io for command execution issues if possible,
                                                    // or a generic AppError::Scraping/Other for status failure.
                                                    Err(AppError::Scraping(format!( // Using Scraping as a placeholder for command failure
                                                        "ollama pull command failed with status: {}",
                                                        status
                                                    )))
                                                }
                                            }
                                            Err(e) => Err(AppError::Io(e)), // Map IO error from command execution
                                        };

                                        // Send completion status
                                        let _ = tx_clone.send(AppEvent::ModelPullCompleted(pull_result)).await;

                                        // Trigger local model refresh regardless of pull success/failure
                                        // Use the client cloned earlier specifically for this
                                        let refresh_result = client_clone_for_refresh.list_models().await;
                                        // Map ApiError to AppError before sending
                                        let _ = tx_clone.send(AppEvent::LocalModelsRefreshed(refresh_result.map_err(AppError::Api))).await;

                                    });
                                } else {
                                     // Should not happen, but handle gracefully
                                     app.install_error = Some("Model or tag not selected.".to_string());
                                     app.current_mode = AppMode::InstallSelectTag; // Go back
                                }
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.current_mode = AppMode::InstallSelectTag;
                                app.selected_registry_tag = None; // Clear selection
                                app.install_error = None;
                            }
                            _ => {}
                        },
                        AppMode::Installing => {
                            // Maybe handle Esc for cancellation attempt in the future?
                            // For now, ignore input while installing.
                        }
                    }
                    // --- End Input Handling ---
                }
            }
        }

        // Check for results from the fetch task without blocking
        // --- Handle Events from Async Tasks ---
        match rx.try_recv() {
            Ok(event) => {
                 match event {
                    AppEvent::ModelDetailsFetched(result) => {
                        app.is_fetching_details = false; // Reset flag
                        match result {
                            Ok(details) => {
                                app.selected_model_details = Some(details);
                                app.status_message = None; // Clear status on success
                            }
                            Err(e) => {
                                app.selected_model_details = None; // Clear potentially stale data
                                app.status_message = Some(format!("Error fetching details: {}", e));
                            }
                        }
                    }
                    AppEvent::RegistryModelsFetched(result) => {
                        app.is_fetching_registry = false;
                        match result {
                            Ok(models) => {
                                app.registry_models = models;
                                if !app.registry_models.is_empty() {
                                    app.registry_model_list_state.select(Some(0));
                                } else {
                                    app.registry_model_list_state.select(None);
                                }
                                app.install_error = None;
                            }
                            Err(e) => {
                                app.install_error = Some(format!("Failed to fetch models: {}", e));
                                app.current_mode = AppMode::Normal; // Go back to normal on error
                            }
                        }
                    }
                    AppEvent::RegistryTagsFetched(result) => {
                         app.is_fetching_registry = false;
                        match result {
                            Ok(tags) => {
                                app.registry_tags = tags;
                                 if !app.registry_tags.is_empty() {
                                    app.registry_tag_list_state.select(Some(0));
                                } else {
                                    app.registry_tag_list_state.select(None);
                                    // Maybe show a message if no tags found?
                                    app.install_error = Some("No tags found for this model.".to_string());
                                    app.current_mode = AppMode::InstallSelectModel; // Go back
                                }
                                app.install_error = None; // Clear previous errors
                            }
                            Err(e) => {
                                app.install_error = Some(format!("Failed to fetch tags: {}", e));
                                app.current_mode = AppMode::InstallSelectModel; // Go back
                            }
                        }
                    }
                    AppEvent::ModelPullCompleted(result) => {
                        app.install_status = None; // Clear "Pulling..." message
                        match result {
                            Ok(_) => {
                                app.status_message = Some("Model pull successful! Refreshing list...".to_string());
                                // Trigger refresh
                                let tx_clone = tx.clone();
                                let client_clone = client.clone();
                                tokio::spawn(async move {
                                     let refresh_result = client_clone.list_models().await;
                                     // Map ApiError to AppError before sending
                                     let _ = tx_clone.send(AppEvent::LocalModelsRefreshed(refresh_result.map_err(AppError::Api))).await;
                                });
                            }
                            Err(e) => {
                                app.install_error = Some(format!("Model pull failed: {}", e));
                                // Stay in Installing mode briefly to show error, then return? Or return immediately?
                                app.current_mode = AppMode::Normal; // Return to normal on failure for now
                            }
                        }
                        // Reset install selections
                        app.selected_registry_model = None;
                        app.selected_registry_tag = None;
                    }
                    AppEvent::LocalModelsRefreshed(result) => {
                        match result {
                            Ok(models) => {
                                let old_selection_index = app.list_state.selected();
                                app.models = models;
                                let new_selection = if app.models.is_empty() {
                                    None
                                } else {
                                    // Try to keep selection, default to 0 if index is now invalid
                                    Some(old_selection_index.unwrap_or(0).min(app.models.len().saturating_sub(1)))
                                };
                                // Trigger fetch for the new selection after refresh
                                app.select_and_prepare_fetch(new_selection);
                                // Clear status only if it was the "refreshing" message
                                if app.status_message.as_deref() == Some("Model pull successful! Refreshing list...") {
                                     app.status_message = None;
                                }
                            }
                            Err(e) => {
                                // Keep existing error/status if pull failed, otherwise show refresh error
                                if app.install_error.is_none() {
                                    app.status_message = Some(format!("Error refreshing models: {}", e));
                                }
                            }
                        }
                        // Ensure mode is Normal after refresh attempt
                        app.current_mode = AppMode::Normal;
                        app.install_status = None; // Clear any lingering install status
                        // Don't clear install_error here automatically, let it be shown if pull failed
                    }
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                // No message received, continue loop
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                // Channel disconnected, should not happen in this setup unless task panics badly
                app.status_message = Some("Error: Background task channel disconnected.".to_string());
                // Consider logging this or handling it more robustly
            }
        }


        if app.should_quit {
            return Ok(());
        }
    }
}
