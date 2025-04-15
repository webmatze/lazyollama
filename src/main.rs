mod app;
mod error;
mod events;
mod handlers;
mod ollama_api;
mod registry_api;
mod tasks;
mod tui;
mod ui;

use crate::{
    app::{AppMode, AppState},
    error::{AppError, Result},
    events::AppEvent,
    ollama_api::OllamaClient,
};

use crossterm::{
    event::{self, Event},
};

use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    let mut terminal = tui::init_terminal()?;

    let ollama_host = ollama_api::get_ollama_host();
    let client = OllamaClient::new(ollama_host.clone());
    let mut app_state = AppState::new();

    let res = run_app(&mut terminal, client, &mut app_state).await;

    tui::restore_terminal(&mut terminal)?;

    if let Err(err) = res {
        println!("Error running app: {:?}", err);
        // Consider returning the error if main should indicate failure
        // return Err(err);
    }

    Ok(())
}


async fn run_app(
    terminal: &mut tui::Tui,
    client: OllamaClient,
    app: &mut AppState,
) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<AppEvent>(32);

    match client.list_models().await {
        Ok(models) => {
            app.models = models;
            if !app.models.is_empty() {
                app.list_state.select(Some(0));
                app.selected_model_details = None;
                app.is_fetching_details = false;
            }
            app.status_message = None;
        }
        Err(e) => {
            app.status_message = Some(format!("Error loading models: {}", e));
        }
    }

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // Only trigger fetches if not running an external command
        if app.current_mode != AppMode::RunningOllama {
            if app.list_state.selected().is_some()
                && app.selected_model_details.is_none()
                && !app.is_fetching_details
            {
                if let Some(name) = app.get_selected_model_name() {
                    app.is_fetching_details = true;
                    app.status_message = Some("Fetching details...".to_string());

                    let client_clone = client.clone();
                    let tx_clone = tx.clone();
                    let name_clone = name.clone();
                    tokio::spawn(async move {
                        tasks::fetch_model_details(client_clone, tx_clone, name_clone).await;
                    });
                }
            }
        }

        if app.current_mode == AppMode::RunningOllama {
            if let Some(event) = rx.recv().await {
                match event {
                    AppEvent::OllamaRunCompleted(result) => {
                        if handlers::handle_ollama_run_completion(result, app, terminal)? {
                             break Ok(()); // Exit loop if handler signals channel closure
                        }
                    }
                    // Ignore any other events that might arrive while RunningOllama was active
                    _ => {}
                }
            } else {
                // Channel closed, something went wrong.
                app.status_message = Some("Error: Event channel closed unexpectedly.".to_string());
                break Ok(());
            }
        } else {
            tokio::select! {
                maybe_term_event_res = tokio::task::spawn_blocking(|| -> Result<Option<Event>> {
                    // Poll with a timeout to keep the loop responsive
                    if crossterm::event::poll(Duration::from_millis(100)).map_err(AppError::Io)? {
                        let event = event::read().map_err(AppError::Io)?;
                        Ok(Some(event))
                    } else {
                        Ok(None)
                    }
                }) => {
                    match maybe_term_event_res {
                        Ok(Ok(Some(Event::Key(key)))) => {
                            // The handler returns true if 'q' was pressed in Normal mode
                            if handlers::handle_key_event(key, app, &client, &tx).await? {
                                app.should_quit = true;
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
                           break Ok(());
                        }
                    }
                },

                maybe_app_event = rx.recv() => {
                    if let Some(event) = maybe_app_event {
                        // Excludes OllamaRunCompleted as it's handled in the RunningOllama mode block
                        handlers::handle_app_event(event, app);
                    } else {
                        // Channel closed
                        app.status_message = Some("Error: Event channel closed unexpectedly.".to_string());
                        break Ok(());
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
