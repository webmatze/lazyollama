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
use std::process::Command;
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
    OllamaRunCompleted(Result<()>), // New: 'ollama run' finished
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

        // --- Conditional Background Fetch ---
        // Only trigger fetches if not running an external command
        if app.current_mode != AppMode::RunningOllama {
            // --- Start Background Detail Fetch if Needed ---
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
            // --- End Background Detail Fetch ---

            // --- Other Background Fetches (Registry, Pull) are triggered by input below ---
        }

        // --- Handle Events ---
        if app.current_mode == AppMode::RunningOllama {
            // --- Running Ollama Mode: Block waiting ONLY for completion ---
            if let Some(event) = rx.recv().await { // Blocking wait for the completion event
                match event {
                    AppEvent::OllamaRunCompleted(result) => {
                        // Always return to normal mode after the command finishes
                        app.current_mode = AppMode::Normal;
                        match result {
                            Ok(_) => {
                                // Optionally clear status or show a success message briefly
                                app.status_message = None;
                                // Force a redraw to clear any potential command output remnants
                                // This needs to be done carefully, maybe redraw is enough?
                                // Let's try without clear first, redraw happens at loop start.
                                // terminal.clear()?;
                            }
                            Err(e) => {
                                app.status_message = Some(format!("'ollama run' failed: {}", e));
                            }
                        }
                        // Force a redraw after handling the event and changing mode
                        terminal.draw(|f| ui::draw(f, app))?;
                    }
                    // Ignore any other events that might arrive while RunningOllama was active
                    _ => {}
                }
            } else {
                // Channel closed, something went wrong.
                app.status_message = Some("Error: Event channel closed unexpectedly.".to_string());
                break Ok(()); // Exit the main loop with success
            }
        } else {
            // --- Other Modes: Use select! for concurrent terminal and async events ---
            tokio::select! {
                // Branch 1: Handle Terminal Input Events (non-blocking via spawn_blocking)
                maybe_term_event_res = tokio::task::spawn_blocking(|| -> Result<Option<Event>> {
                    // Poll with a timeout to keep the loop responsive
                    if crossterm::event::poll(Duration::from_millis(100)).map_err(AppError::Io)? {
                        let event = event::read().map_err(AppError::Io)?;
                        Ok(Some(event)) // Read the event if available
                    } else {
                        Ok(None) // Timeout, no event
                    }
                }) => {
                    match maybe_term_event_res {
                        Ok(Ok(Some(Event::Key(key)))) => { // Successfully read a key event
                            if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                                // --- Input Handling Logic (Moved Here) ---
                                let current_mode = app.current_mode.clone(); // Clone mode for matching
                                // NOTE: Match only modes relevant when NOT RunningOllama
                                match current_mode {
                                    AppMode::Normal => match key.code {
                                        KeyCode::Char('q') => app.should_quit = true,
                                        KeyCode::Char('h') | KeyCode::Char('?') => {
                                            app.current_mode = AppMode::Help;
                                            app.status_message = None; // Clear status for help modal
                                        }
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
                                        KeyCode::Enter => {
                                            if let Some(name) = app.get_selected_model_name() {
                                                app.current_mode = AppMode::RunningOllama;
                                                app.status_message = None; // Clear status while running

                                                let tx_clone = tx.clone();
                                                let model_name_clone = name.clone();

                                                // Spawn the task to run ollama
                                                tokio::spawn(async move {
                                                    // --- Suspend TUI ---
                                                    let suspend_result = {
                                                        let mut stdout = io::stdout();
                                                        disable_raw_mode()
                                                            .and_then(|_| execute!(stdout, LeaveAlternateScreen))
                                                            .map_err(AppError::Io)
                                                    };

                                                    let run_result = match suspend_result {
                                                        Ok(_) => {
                                                            println!("\n--- Starting 'ollama run {}' ---", model_name_clone);
                                                            println!("--- (Type '/bye' or press Ctrl+D to exit) ---");

                                                            // --- Execute Command ---
                                                            let command_result = std::process::Command::new("ollama")
                                                                .arg("run")
                                                                .arg(&model_name_clone)
                                                                .stdin(std::process::Stdio::inherit())
                                                                .stdout(std::process::Stdio::inherit())
                                                                .stderr(std::process::Stdio::inherit())
                                                                .spawn();

                                                            let status_result = match command_result {
                                                                Ok(mut child) => child.wait().map_err(AppError::Io),
                                                                Err(e) => Err(AppError::Io(e)),
                                                            };

                                                            let final_result = status_result.and_then(|status| {
                                                                if status.success() {
                                                                    Ok(())
                                                                } else {
                                                                    Err(AppError::Command(format!(
                                                                        "'ollama run' failed with status: {}",
                                                                        status
                                                                    )))
                                                                }
                                                            });

                                                            // --- Resume TUI ---
                                                            let resume_result = {
                                                                 let mut stdout = io::stdout();
                                                                 execute!(stdout, EnterAlternateScreen)
                                                                    .and_then(|_| enable_raw_mode())
                                                                    .map_err(AppError::Io)
                                                            };
                                                            resume_result.and(final_result)
                                                        }
                                                        Err(e) => Err(e), // Failed to suspend TUI
                                                    };
                                                    // Send completion event
                                                    let _ = tx_clone.send(AppEvent::OllamaRunCompleted(run_result)).await;
                                                });
                                            }
                                        }
                                        _ => {}
                                    },
                                    AppMode::ConfirmDelete => match key.code {
                                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                                            if let Some(name) = app.get_selected_model_name() {
                                                app.status_message = Some(format!("Deleting {}...", name));
                                                // Draw immediately? Maybe not needed as loop redraws
                                                // terminal.draw(|f| ui::draw(f, app))?;

                                                let client_clone = client.clone();
                                                let tx_clone = tx.clone();
                                                let model_name_clone = name.clone();

                                                tokio::spawn(async move {
                                                    match client_clone.delete_model(&model_name_clone).await {
                                                        Ok(_) => {
                                                            let refresh_result = client_clone.list_models().await;
                                                            let _ = tx_clone.send(AppEvent::LocalModelsRefreshed(refresh_result.map_err(AppError::Api))).await;
                                                        }
                                                        Err(e) => {
                                                             let _ = tx_clone.send(AppEvent::ModelPullCompleted(Err(AppError::Api(e)))).await; // Reusing event for error reporting
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
                                                        let result = registry_api::fetch_registry_tags(&model_name).await;
                                                        let _ = tx_clone.send(AppEvent::RegistryTagsFetched(result)).await;
                                                    });
                                                }
                                            }
                                        }
                                        KeyCode::Char('q') | KeyCode::Esc => {
                                            app.current_mode = AppMode::Normal;
                                            app.install_error = None;
                                            app.is_fetching_registry = false;
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
                                            app.selected_registry_model = None;
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
                                                let client_clone_for_refresh = client.clone();
                                                tokio::spawn(async move {
                                                    // Restore terminal temporarily
                                                    let _ = disable_raw_mode();
                                                    let _ = execute!(io::stdout(), LeaveAlternateScreen);

                                                    println!("\n--- Starting 'ollama pull {}:{}' ---", model, tag);
                                                    println!("--- (Application will resume after pull completes) ---");

                                                    let command_result = tokio::process::Command::new("ollama") // Using tokio::process here is fine
                                                        .arg("pull")
                                                        .arg(format!("{}:{}", model, tag))
                                                        .status()
                                                        .await;

                                                    // Re-enable raw mode and enter alternate screen
                                                    let _ = execute!(io::stdout(), EnterAlternateScreen);
                                                    let _ = enable_raw_mode();
                                                    // println!("--- Pull command finished ---"); // Not visible

                                                    let pull_result = match command_result {
                                                        Ok(status) if status.success() => Ok(()),
                                                        Ok(status) => Err(AppError::Scraping(format!( // Using Scraping as placeholder
                                                            "ollama pull command failed with status: {}", status
                                                        ))),
                                                        Err(e) => Err(AppError::Io(e)),
                                                    };

                                                    let _ = tx_clone.send(AppEvent::ModelPullCompleted(pull_result)).await;

                                                    // Trigger refresh regardless
                                                    let refresh_result = client_clone_for_refresh.list_models().await;
                                                    let _ = tx_clone.send(AppEvent::LocalModelsRefreshed(refresh_result.map_err(AppError::Api))).await;
                                                });
                                            } else {
                                                 app.install_error = Some("Model or tag not selected.".to_string());
                                                 app.current_mode = AppMode::InstallSelectTag;
                                            }
                                        }
                                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                            app.current_mode = AppMode::InstallSelectTag;
                                            app.selected_registry_tag = None;
                                            app.install_error = None;
                                        }
                                        _ => {}
                                    },
                                    AppMode::Installing => {
                                        // Ignore input while installing.
                                    }
                                    // RunningOllama is handled in the outer if/else
                                    AppMode::RunningOllama => unreachable!(), // Should not be reached here
                                    AppMode::Help => match key.code {
                                        // Dismiss help with h, ?, q, or Esc
                                        KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
                                            app.current_mode = AppMode::Normal;
                                            app.status_message = None;
                                        }
                                        _ => {} // Ignore other keys in help mode
                                    }
                                }
                                // --- End Input Handling ---
                            }
                        }
                        Ok(Ok(Some(_))) => {} // Other terminal event types (ignore for now)
                        Ok(Ok(None)) => {} // Poll timeout, no input, continue loop
                        Ok(Err(e)) => { // Error reading/polling crossterm event
                            app.status_message = Some(format!("Input error: {}", e));
                            // Potentially break or log error more formally
                        }
                        Err(e) => { // Task panicked
                           app.status_message = Some(format!("Input task panicked: {}", e));
                           break Ok(()); // Likely fatal, exit loop
                        }
                    }
                }

                // Branch 2: Handle Async App Events from the channel
                maybe_app_event = rx.recv() => {
                    if let Some(event) = maybe_app_event {
                        // --- Async Event Handling Logic (Moved Here) ---
                        // NOTE: Excludes OllamaRunCompleted as it's handled above
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
                                        // Refresh is triggered by LocalModelsRefreshed event sent from the task
                                    }
                                    Err(e) => {
                                        app.install_error = Some(format!("Model pull/delete failed: {}", e)); // Adjusted message
                                        app.current_mode = AppMode::Normal; // Return to normal on failure
                                    }
                                }
                                // Reset install selections (safe even if it was a delete error)
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
                                            Some(old_selection_index.unwrap_or(0).min(app.models.len().saturating_sub(1)))
                                        };
                                        app.select_and_prepare_fetch(new_selection); // Trigger fetch for new selection

                                        // Clear status only if it was a success message
                                        if app.status_message.as_deref() == Some("Model pull successful! Refreshing list...") {
                                             app.status_message = None;
                                        }
                                    }
                                    Err(e) => {
                                        // Keep existing error if pull/delete failed, otherwise show refresh error
                                        if app.install_error.is_none() {
                                            app.status_message = Some(format!("Error refreshing models: {}", e));
                                        }
                                    }
                                }
                                // Ensure mode is Normal after refresh attempt completes
                                app.current_mode = AppMode::Normal;
                                app.install_status = None; // Clear any lingering install status
                            }
                            // OllamaRunCompleted is handled in the outer if block
                            AppEvent::OllamaRunCompleted(_) => unreachable!("Should be handled in RunningOllama mode"),
                        }
                        // --- End Async Event Handling ---
                    } else {
                        // Channel closed
                        app.status_message = Some("Error: Event channel closed unexpectedly.".to_string());
                        break Ok(()); // Exit the main loop
                    }
                }
            }
        }

        // Check if should quit after handling events
        if app.should_quit {
            return Ok(());
        }
    }
} // End of loop is implicit
