mod app;
mod error;
mod events;
mod handlers;
mod ollama_api;
mod registry_api;
mod tasks;
mod tui;
mod ui;

use clap::Parser;
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


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)] // Reads version from Cargo.toml
struct CliArgs {
    // No arguments needed yet, but the struct is required for clap
    // The `version` attribute on `command` handles the --version flag
}

// Synchronous main function
fn main() -> Result<()> {
    CliArgs::parse();

    let rt = tokio::runtime::Runtime::new().map_err(AppError::Io)?; // Map the std::io::Error to AppError::Io
    rt.block_on(run_async_app())
}

async fn run_async_app() -> Result<()> {
    let mut terminal = tui::init_terminal()?;

    let result = async {
        let ollama_host = ollama_api::get_ollama_host();
        let client = OllamaClient::new(ollama_host.clone());
        let mut app_state = AppState::new();
        run_app(&mut terminal, client, &mut app_state).await
    }.await;

    tui::restore_terminal(&mut terminal)?;

    if let Err(err) = &result {
         if !matches!(err, AppError::Io(_)) {
            eprintln!("Error running app: {:?}", err);
         }
    }

    result
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
            // Initialize filtered_models to empty since no filter is active initially
            app.filtered_models.clear();
            app.is_filtered = false;
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
                             break Ok(());
                        }
                    }
                    _ => {}
                }
            } else {
                app.status_message = Some("Error: Event channel closed unexpectedly.".to_string());
                break Ok(());
            }
        } else {
            tokio::select! {
                maybe_term_event_res = tokio::task::spawn_blocking(|| -> Result<Option<Event>> {
                    if crossterm::event::poll(Duration::from_millis(100)).map_err(AppError::Io)? {
                        let event = event::read().map_err(AppError::Io)?;
                        Ok(Some(event))
                    } else {
                        Ok(None)
                    }
                }) => {
                    match maybe_term_event_res {
                        Ok(Ok(Some(Event::Key(key)))) => {
                            if handlers::handle_key_event(key, app, &client, &tx).await? {
                                app.should_quit = true;
                            }
                        }
                         Ok(Ok(Some(_))) => {}
                        Ok(Ok(None)) => {}
                        Ok(Err(e)) => {
                            app.status_message = Some(format!("Input error: {}", e));
                        }
                        Err(e) => {
                           app.status_message = Some(format!("Input task panicked: {}", e));
                           break Ok(());
                        }
                    }
                },

                maybe_app_event = rx.recv() => {
                    if let Some(event) = maybe_app_event {
                        handlers::handle_app_event(event, app);
                    } else {
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
