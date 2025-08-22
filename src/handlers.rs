use crate::{
    app::{AppMode, AppState},
    error::Result,
    events::AppEvent,
    ollama_api::OllamaClient,
    tasks,
    tui,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;

type EventSender = mpsc::Sender<AppEvent>;

/// Handles terminal key events.
/// Returns `Ok(true)` if the application should quit, `Ok(false)` otherwise.
pub async fn handle_key_event(
    key: KeyEvent,
    app: &mut AppState,
    client: &OllamaClient,
    tx: &EventSender,
) -> Result<bool> {
    if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
        let mut handled_globally = false;
        if app.current_mode != AppMode::RunningOllama && app.current_mode != AppMode::Help && app.current_mode != AppMode::Filter && app.current_mode != AppMode::InstallSelectModelFilter {
            match key.code {
                KeyCode::Char('h') | KeyCode::Char('?') => {
                    app.previous_mode = Some(app.current_mode.clone());
                    app.current_mode = AppMode::Help;
                    app.status_message = None;
                    handled_globally = true;
                }
                _ => {}
            }
        }

        if !handled_globally {
            let current_mode = app.current_mode.clone();
            match current_mode {
                AppMode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Char('j') | KeyCode::Down => app.next_model(),
                    KeyCode::Char('k') | KeyCode::Up => app.previous_model(),
                    KeyCode::Char('/') => {
                        // Enter filter mode
                        app.current_mode = AppMode::Filter;
                        app.filter_input.clear();
                        app.filter_cursor_pos = 0;
                        app.status_message = None;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Clear filter with Ctrl+C
                        if app.is_filtered {
                            app.clear_filter();
                        }
                    }
                    KeyCode::Char('d') => {
                        if app.list_state.selected().is_some() {
                            app.current_mode = AppMode::ConfirmDelete;
                            app.status_message = None;
                        }
                    }
                    KeyCode::Char('i') => {
                        app.current_mode = AppMode::InstallSelectModel;
                        app.is_fetching_registry = true;
                        app.install_error = None;
                        app.registry_models.clear();
                        app.registry_model_list_state.select(None);

                        let tx_clone = tx.clone();
                        tokio::spawn(async move {
                            tasks::fetch_registry_models(tx_clone).await;
                        });
                    }
                    KeyCode::Enter => {
                        if let Some(name) = app.get_selected_model_name() {
                            app.current_mode = AppMode::RunningOllama;
                            app.status_message = None;

                            let tx_clone = tx.clone();
                            let model_name_clone = name.clone();

                            tokio::spawn(async move {
                                tasks::run_ollama(tx_clone, model_name_clone).await;
                            });
                        }
                    }
                    _ => {}
                },
                AppMode::Filter => match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Clear filter input with Ctrl+C
                        app.filter_input.clear();
                        app.filter_cursor_pos = 0;
                        app.apply_filter();
                    }
                    KeyCode::Char(c) => {
                        // Add character to filter input
                        app.filter_input_char(c);
                    }
                    KeyCode::Backspace => {
                        // Remove character from filter input
                        app.filter_input_backspace();
                    }
                    KeyCode::Left => {
                        app.filter_cursor_left();
                    }
                    KeyCode::Right => {
                        app.filter_cursor_right();
                    }
                    KeyCode::Enter => {
                        // Confirm filter and return to normal mode
                        app.current_mode = AppMode::Normal;
                        app.status_message = if app.is_filtered {
                            Some(format!("Filter: '{}' ({} models)", app.filter_input, app.get_current_models().len()))
                        } else {
                            None
                        };
                    }
                    KeyCode::Esc => {
                        // Cancel filter - clear it and return to normal mode
                        app.clear_filter();
                        app.current_mode = AppMode::Normal;
                        app.status_message = Some("Filter cleared".to_string());
                    }
                    _ => {}
                },
                AppMode::ConfirmDelete => match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        if let Some(name) = app.get_selected_model_name() {
                            app.status_message = Some(format!("Deleting {}...", name));

                            let client_clone = client.clone();
                            let tx_clone = tx.clone();
                            let model_name_clone = name.clone();

                            tokio::spawn(async move {
                                tasks::delete_model(client_clone, tx_clone, model_name_clone).await;
                            });
                        }
                        app.current_mode = AppMode::Normal;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        app.current_mode = AppMode::Normal;
                        app.status_message = None;
                    }
                    _ => {}
                },
                AppMode::InstallSelectModel => match key.code {
                    KeyCode::Char('/') => {
                        // Enter registry filter mode
                        app.current_mode = AppMode::InstallSelectModelFilter;
                        app.registry_filter_input.clear();
                        app.registry_filter_cursor_pos = 0;
                        app.install_error = None;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Clear registry filter with Ctrl+C
                        if app.is_registry_filtered {
                            app.clear_registry_filter();
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        let len = app.get_current_registry_models().len();
                        if len > 0 {
                            let i = match app.registry_model_list_state.selected() {
                                Some(i) => (i + 1) % len,
                                None => 0,
                            };
                            app.registry_model_list_state.select(Some(i));
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let len = app.get_current_registry_models().len();
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
                            if let Some(model_name) = app.get_current_registry_models().get(selected_index).cloned() {
                                app.selected_registry_model = Some(model_name.clone());
                                app.current_mode = AppMode::InstallSelectTag;
                                app.is_fetching_registry = true;
                                app.install_error = None;
                                app.registry_tags.clear();
                                app.registry_tag_list_state.select(None);

                                let tx_clone = tx.clone();
                                let model_name_clone = model_name.clone();
                                tokio::spawn(async move {
                                    tasks::fetch_registry_tags(tx_clone, model_name_clone).await;
                                });
                            }
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.current_mode = AppMode::Normal;
                        app.install_error = None;
                        app.is_fetching_registry = false;
                        app.clear_registry_filter();
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
                            let model_clone = model.clone();
                            let tag_clone = tag.clone();
                            tokio::spawn(async move {
                                tasks::pull_model(client_clone_for_refresh, tx_clone, model_clone, tag_clone).await;
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
                    // Input is ignored while installing.
                }
                AppMode::RunningOllama => unreachable!(),
                AppMode::InstallSelectModelFilter => match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Clear filter input with Ctrl+C
                        app.registry_filter_input.clear();
                        app.registry_filter_cursor_pos = 0;
                        app.apply_registry_filter();
                    }
                    KeyCode::Char(c) => {
                        // Add character to registry filter input
                        app.registry_filter_input_char(c);
                    }
                    KeyCode::Backspace => {
                        // Remove character from registry filter input
                        app.registry_filter_input_backspace();
                    }
                    KeyCode::Left => {
                        app.registry_filter_cursor_left();
                    }
                    KeyCode::Right => {
                        app.registry_filter_cursor_right();
                    }
                    KeyCode::Enter => {
                        // Confirm filter and return to install select mode
                        app.current_mode = AppMode::InstallSelectModel;
                        app.install_error = if app.is_registry_filtered {
                            Some(format!("Filter: '{}' ({} models)", app.registry_filter_input, app.get_current_registry_models().len()))
                        } else {
                            None
                        };
                    }
                    KeyCode::Esc => {
                        // Cancel filter - clear it and return to install select mode
                        app.clear_registry_filter();
                        app.current_mode = AppMode::InstallSelectModel;
                        app.install_error = Some("Filter cleared".to_string());
                    }
                    _ => {}
                },
                AppMode::Help => match key.code {
                    KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
                        app.current_mode = app.previous_mode.take().unwrap_or(AppMode::Normal);
                        app.status_message = None;
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(false)
}

/// Handles asynchronous events received from tasks.
pub fn handle_app_event(event: AppEvent, app: &mut AppState) {
     match event {
        AppEvent::ModelDetailsFetched(result) => {
            app.is_fetching_details = false;
            match result {
                Ok(details) => {
                    app.selected_model_details = Some(details);
                    app.status_message = None;
                }
                Err(e) => {
                    app.selected_model_details = None;
                    app.status_message = Some(format!("Error fetching details: {}", e));
                }
            }
        }
        AppEvent::RegistryModelsFetched(result) => {
            app.is_fetching_registry = false;
            match result {
                Ok(models) => {
                    app.registry_models = models;
                    // Reapply filter if it was active
                    if app.is_registry_filtered {
                        app.apply_registry_filter();
                    } else if !app.registry_models.is_empty() {
                        app.registry_model_list_state.select(Some(0));
                    } else {
                        app.registry_model_list_state.select(None);
                    }
                    app.install_error = None;
                }
                Err(e) => {
                    app.install_error = Some(format!("Failed to fetch models: {}", e));
                    app.current_mode = AppMode::Normal;
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
                        app.current_mode = AppMode::InstallSelectModel;
                    }
                    app.install_error = None;
                }
                Err(e) => {
                    app.install_error = Some(format!("Failed to fetch tags: {}", e));
                    app.current_mode = AppMode::InstallSelectModel;
                }
            }
        }
        AppEvent::ModelPullCompleted(result) => {
            app.install_status = None;
            match result {
                Ok(_) => {
                    app.status_message = Some("Model pull successful! Refreshing list...".to_string());
                }
                Err(e) => {
                    app.install_error = Some(format!("Model pull/delete failed: {}", e));
                    app.current_mode = AppMode::Normal;
                }
            }
            app.selected_registry_model = None;
            app.selected_registry_tag = None;
        }
        AppEvent::LocalModelsRefreshed(result) => {
            match result {
                Ok(models) => {
                    let old_selection_index = app.list_state.selected();
                    app.models = models;
                    
                    // Reapply filter if it was active
                    if app.is_filtered {
                        app.apply_filter();
                    }
                    
                    let current_models = app.get_current_models();
                    let new_selection = if current_models.is_empty() {
                        None
                    } else {
                        Some(old_selection_index.unwrap_or(0).min(current_models.len().saturating_sub(1)))
                    };
                    app.select_and_prepare_fetch(new_selection);

                    if app.status_message.as_deref() == Some("Model pull successful! Refreshing list...") {
                         app.status_message = None;
                    }
                }
                Err(e) => {
                    if app.install_error.is_none() {
                        app.status_message = Some(format!("Error refreshing models: {}", e));
                    }
                }
            }
            app.current_mode = AppMode::Normal;
            app.install_status = None;
        }
        AppEvent::OllamaRunCompleted(_) => {
             eprintln!("Warning: OllamaRunCompleted event received outside of RunningOllama mode.");
             app.current_mode = AppMode::Normal;
        }
    }
}

/// Handles the completion event specifically when in RunningOllama mode.
/// Returns `Ok(true)` if the app should exit due to channel closure, `Ok(false)` otherwise.
/// Forces a redraw on the passed terminal.
pub fn handle_ollama_run_completion(
    result: Result<()>,
    app: &mut AppState,
    terminal: &mut tui::Tui,
) -> Result<bool> {
    app.current_mode = AppMode::Normal;
    match result {
        Ok(_) => {
            app.status_message = None;
        }
        Err(e) => {
            app.status_message = Some(format!("'ollama run' failed: {}", e));
        }
    }
    terminal.draw(|f| crate::ui::draw(f, app))?;
    Ok(false)
}
