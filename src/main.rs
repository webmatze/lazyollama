// src/main.rs

mod app;
mod error;
mod ollama_api;
mod ui;

use crate::{
    app::{AppMode, AppState},
    error::{ApiError, Result}, // Added ApiError
    ollama_api::{OllamaClient, ShowModelResponse}, // Added ShowModelResponse
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
use tokio::sync::mpsc; // Added mpsc

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

// Define a type for messages sent from async tasks
type AppEvent = Result<ShowModelResponse>; // Corrected: Result already includes AppError

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    client: OllamaClient, // Take client by value as it's cloneable
    app: &mut AppState,
) -> Result<()> {
    // Channel for communication between async tasks and the main loop
    let (tx, mut rx) = mpsc::channel::<AppEvent>(1); // Buffer size 1

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
                    let _ = tx_clone.send(Ok(result?)).await; // Wrap result in Ok for the channel type
                    // Propagate ApiError via the channel
                    Ok::<(), ApiError>(()) // Return type for the spawn block if needed
                });
            }
        }

        // --- Handle Input and Channel Events ---
        // Poll for events with a smaller timeout to remain responsive
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                    match app.current_mode {
                        AppMode::Normal => match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Char('j') | KeyCode::Down => app.next_model(),
                            KeyCode::Char('k') | KeyCode::Up => app.previous_model(),
                            KeyCode::Char('d') => {
                                if app.list_state.selected().is_some() {
                                    app.current_mode = AppMode::ConfirmDelete;
                                }
                            }
                            _ => {}
                        },
                        AppMode::ConfirmDelete => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                if let Some(name) = app.get_selected_model_name() {
                                    app.status_message = Some(format!("Deleting {}...", name));
                                    terminal.draw(|f| ui::draw(f, app))?; // Show deleting message

                                    // Use the original client for deletion
                                    match client.delete_model(&name).await {
                                        Ok(_) => {
                                            app.status_message = Some(format!("Deleted {}", name));
                                            // Refresh model list
                                            match client.list_models().await {
                                                Ok(models) => {
                                                    let old_selection = app.list_state.selected().unwrap_or(0);
                                                    app.models = models;
                                                    let new_selection = if app.models.is_empty() {
                                                        None
                                                    } else {
                                                        Some(old_selection.min(app.models.len().saturating_sub(1)))
                                                    };
                                                    // Trigger fetch for the new selection after delete
                                                    app.select_and_prepare_fetch(new_selection); // Needs to be public
                                                }
                                                Err(e) => {
                                                    app.status_message = Some(format!("Error refreshing models: {}", e));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            app.status_message = Some(format!("Error deleting {}: {}", name, e));
                                        }
                                    }
                                }
                                app.current_mode = AppMode::Normal;
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.current_mode = AppMode::Normal;
                                app.status_message = None;
                            }
                            _ => {}
                        },
                    }
                }
            }
        }

        // Check for results from the fetch task without blocking
        match rx.try_recv() {
            Ok(fetch_result) => {
                app.is_fetching_details = false; // Reset flag
                match fetch_result {
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
