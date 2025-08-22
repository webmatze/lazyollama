// src/ui.rs
// Handles rendering the TUI layout and widgets.

use crate::app::{AppMode, AppState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Cursor character for filter input fields
/// Uses ASCII underline character for maximum terminal compatibility
const CURSOR_CHAR: char = '_';

fn draw_help_modal(f: &mut Frame) {
    let block = Block::default()
        .title("Help - Shortcuts")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));

    let help_text = vec![
        Line::from(Span::styled("--- General ---", Style::default().bold().underlined())),
        Line::from("  q          : Quit"),
        Line::from("  h / ?      : Show/Hide Help"),
        Line::from(""),
        Line::from(Span::styled("--- Model List ---", Style::default().bold().underlined())),
        Line::from("  ↓ / j      : Move Down"),
        Line::from("  ↑ / k      : Move Up"),
        Line::from("  d          : Delete Selected Model (Opens Confirm Dialog)"),
        Line::from("  i          : Install New Model (Opens Install Dialog)"),
        Line::from("  Enter      : Run Selected Model (Suspends TUI)"),
        Line::from("  /          : Filter Models (Type to Search)"),
        Line::from("  Ctrl+C     : Clear Filter"),
        Line::from(""),
        Line::from(Span::styled("--- Filter Mode ---", Style::default().bold().underlined())),
        Line::from("  Type       : Enter Filter Text"),
        Line::from("  Backspace  : Remove Character"),
        Line::from("  Enter      : Confirm Filter"),
        Line::from("  Esc        : Cancel Filter"),
        Line::from("  Ctrl+C     : Clear Input"),
        Line::from(""),
        Line::from(Span::styled("--- Install Mode ---", Style::default().bold().underlined())),
        Line::from("  ↓ / ↑ / j / k: Navigate Models/Tags"),
        Line::from("  /          : Filter Registry Models"),
        Line::from("  Ctrl+C     : Clear Registry Filter"),
        Line::from("  Enter      : Select Model/Tag"),
        Line::from("  Esc / q    : Cancel / Go Back"),
        Line::from(""),
        Line::from(Span::styled("--- Dialogs ---", Style::default().bold().underlined())),
        Line::from("  y / Y      : Confirm Action"),
        Line::from("  n / N / Esc: Cancel / Go Back"),
        Line::from(""),
        Line::from(Span::styled("--- Help Dialog ---", Style::default().bold().underlined())),
        Line::from("  h/?/q/Esc  : Close Help"),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    let area = centered_rect(80, 80, f.size());

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

pub fn draw(f: &mut Frame, app: &AppState) {
    // Main layout: 90% for content, 10% for status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.size());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
        .split(chunks[0]);

    draw_model_list(f, app, main_chunks[0]);
    draw_model_details(f, app, main_chunks[1]);
    draw_status_bar(f, app, chunks[1]);

    // --- Render Modals ---
    match app.current_mode {
        AppMode::ConfirmDelete => {
            if let Some(model_name) = app.get_selected_model_name() {
                draw_confirmation_dialog(f, &model_name);
            }
        }
        AppMode::InstallSelectModel => draw_install_model_select_dialog(f, app),
        AppMode::InstallSelectModelFilter => draw_install_model_select_dialog(f, app),
        AppMode::InstallSelectTag => draw_install_tag_select_dialog(f, app),
        AppMode::InstallConfirm => draw_install_confirm_dialog(f, app),
        AppMode::Help => draw_help_modal(f),
        _ => {}
    }
    // --- End Render Modals ---
}

fn draw_model_list(f: &mut Frame, app: &AppState, area: Rect) {
    // Split the model list area to include filter input if in filter mode
    let (list_area, filter_area) = if app.current_mode == AppMode::Filter {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(area);
        (split[1], Some(split[0]))
    } else {
        (area, None)
    };

    // Get the current models (filtered or full list)
    let current_models = app.get_current_models();
    let items: Vec<ListItem> = current_models
        .iter()
        .map(|m| {
            ListItem::new(Line::from(Span::styled(
                m.name.clone(),
                Style::default(),
            )))
        })
        .collect();

    // Create title with filter indicator
    let title = if app.is_filtered {
        format!("Models (filtered: {}/{})", current_models.len(), app.models.len())
    } else {
        "Models".to_string()
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut list_state = app.list_state.clone();
    f.render_stateful_widget(list, list_area, &mut list_state);

    // Draw filter input if in filter mode
    if let Some(filter_area) = filter_area {
        draw_filter_input(f, app, filter_area);
    }
}

fn draw_filter_input(f: &mut Frame, app: &AppState, area: Rect) {
    let input_style = if app.current_mode == AppMode::Filter {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Filter")
        .border_style(input_style);

    // Create the input display with cursor
    let mut input_display = app.filter_input.clone();
    if app.current_mode == AppMode::Filter {
        // Insert cursor character at cursor position (using ASCII-safe cursor)
        input_display.insert(app.filter_cursor_pos, CURSOR_CHAR);
    }

    let input_paragraph = Paragraph::new(input_display)
        .block(block)
        .style(input_style);

    f.render_widget(input_paragraph, area);
}

fn draw_model_details(f: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Details");

    let mut text_lines: Vec<Line> = Vec::new();

    if let Some(selected_index) = app.list_state.selected() {
        let current_models = app.get_current_models();
        if let Some(basic_info) = current_models.get(selected_index) {
            // Display basic info first
            text_lines.push(Line::from(vec![
                Span::styled("Name: ", Style::default().bold()),
                Span::raw(basic_info.name.clone()),
            ]));
            text_lines.push(Line::from(vec![
                Span::styled("Size: ", Style::default().bold()),
                Span::raw(basic_info.size_formatted()),
            ]));
            text_lines.push(Line::from(vec![
                Span::styled("Modified: ", Style::default().bold()),
                Span::raw(basic_info.modified_at.clone()),
            ]));
            text_lines.push(Line::from(vec![
                Span::styled("Digest: ", Style::default().bold()),
                Span::raw(basic_info.digest.chars().take(12).collect::<String>() + "..."),
            ]));
            text_lines.push(Line::from(""));

            // Check if detailed info is available
            if let Some(details) = &app.selected_model_details {
                text_lines.push(Line::from(Span::styled("--- Details ---", Style::default().italic())));

                if let Some(extra) = &details.details {
                    if let Some(val) = &extra.family { 
                        text_lines.push(Line::from(vec![Span::styled("Family: ", Style::default().bold()), Span::raw(val)])); 
                    }
                    if let Some(val) = &extra.format { 
                        text_lines.push(Line::from(vec![Span::styled("Format: ", Style::default().bold()), Span::raw(val)])); 
                    }
                    if let Some(val) = &extra.parameter_size { 
                        text_lines.push(Line::from(vec![Span::styled("Param Size: ", Style::default().bold()), Span::raw(val)])); 
                    }
                    if let Some(val) = &extra.quantization_level { 
                        text_lines.push(Line::from(vec![Span::styled("Quant Level: ", Style::default().bold()), Span::raw(val)])); 
                    }
                    if let Some(families) = &extra.families {
                        if !families.is_empty() {
                            text_lines.push(Line::from(vec![Span::styled("Families: ", Style::default().bold()), Span::raw(families.join(", "))]));
                        }
                    }
                }

                if let Some(val) = &details.parameters { 
                    text_lines.push(Line::from("")); 
                    text_lines.push(Line::from(Span::styled("Parameters:", Style::default().bold()))); 
                    text_lines.push(Line::from(Span::raw(val.clone()))); 
                }
                if let Some(val) = &details.template { 
                    text_lines.push(Line::from("")); 
                    text_lines.push(Line::from(Span::styled("Template:", Style::default().bold()))); 
                    text_lines.push(Line::from(Span::raw(val.clone()))); 
                }
                if let Some(val) = &details.modelfile { 
                    text_lines.push(Line::from("")); 
                    text_lines.push(Line::from(Span::styled("Modelfile:", Style::default().bold()))); 
                    text_lines.push(Line::from(Span::raw(val.clone()))); 
                }
                if let Some(val) = &details.license { 
                    text_lines.push(Line::from("")); 
                    text_lines.push(Line::from(Span::styled("License:", Style::default().bold()))); 
                    text_lines.push(Line::from(Span::raw(val.clone()))); 
                }
            } else {
                if let Some(status) = &app.status_message {
                    if status.contains("Fetching") {
                        text_lines.push(Line::from(Span::styled("Fetching details...", Style::default().italic())));
                    }
                }
            }
        } else {
            text_lines.push(Line::from("Error: Selected index out of bounds."));
        }
    } else {
        text_lines.push(Line::from("Select a model to see details."));
    }

    let paragraph = Paragraph::new(Text::from(text_lines))
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, app: &AppState, area: Rect) {
    let status_text = if let Some(err) = &app.install_error {
        format!("Error: {}", err).red().to_string()
    } else if let Some(status) = &app.install_status {
        status.clone().yellow().to_string()
    } else {
        match app.current_mode {
            AppMode::Normal => {
                if app.is_filtered {
                    format!("Filter: '{}' ({} models) | /: Filter | Ctrl+C: Clear | q: Quit", 
                            app.filter_input, app.get_current_models().len())
                } else {
                    app.status_message.clone().unwrap_or_else(||
                        "q: Quit | ↓/j: Down | ↑/k: Up | d: Delete | i: Install | Enter: Run | /: Filter".to_string()
                    )
                }
            }
            AppMode::Filter => {
                format!("Filter Mode: Type to search | Enter: Confirm | Esc: Cancel | Ctrl+C: Clear")
            }
            AppMode::ConfirmDelete => "Confirm delete? (y/N)".to_string(),
            AppMode::InstallSelectModel => {
                if app.is_registry_filtered {
                    format!("Filter: '{}' ({} models) | /: Filter | Ctrl+C: Clear | ↑/↓: Select | Enter: Choose Tags | Esc: Cancel", 
                            app.registry_filter_input, app.get_current_registry_models().len())
                } else {
                    "↑/↓: Select | Enter: Choose Tags | /: Filter | Esc: Cancel".to_string()
                }
            },
            AppMode::InstallSelectModelFilter => "Filter Mode: Type to search | Enter: Confirm | Esc: Cancel | Ctrl+C: Clear".to_string(),
            AppMode::InstallSelectTag => "↑/↓: Select | Enter: Confirm | Esc: Back".to_string(),
            AppMode::InstallConfirm => "Confirm install? (y/N) | Esc: Back".to_string(),
            AppMode::Installing => app.install_status.clone().unwrap_or_else(|| "Installing...".to_string()),
            AppMode::RunningOllama => "Running ollama... (TUI Suspended)".to_string(),
            AppMode::Help => "h/?/q/Esc: Close Help".to_string(),
        }
    };

    let status_line = Line::from(status_text);

    let paragraph = Paragraph::new(status_line)
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(paragraph, area);
}

fn draw_confirmation_dialog(f: &mut Frame, model_name: &str) {
    let block = Block::default()
        .title("Confirm Deletion")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));

    let text = format!("Are you sure you want to delete '{}'? (y/N)", model_name);
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true });

    let area = centered_rect(60, 20, f.size());

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_install_model_select_dialog(f: &mut Frame, app: &AppState) {
    // Split the dialog area to include filter input if in filter mode
    let (list_area, filter_area) = if app.current_mode == AppMode::InstallSelectModelFilter {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(centered_rect(70, 50, f.size()));
        (split[1], Some(split[0]))
    } else {
        (centered_rect(70, 50, f.size()), None)
    };

    // Create title with filter indicator
    let title = if app.is_registry_filtered {
        format!("Install Model: Select Model (filtered: {}/{})", 
                app.get_current_registry_models().len(), app.registry_models.len())
    } else {
        "Install Model: Select Model".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(Clear, list_area);
    if let Some(filter_area) = filter_area {
        f.render_widget(Clear, filter_area);
    }

    if app.is_fetching_registry {
        let loading_text = Paragraph::new("Loading models...")
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(loading_text, list_area);
    } else {
        let current_models = app.get_current_registry_models();
        let items: Vec<ListItem> = current_models
            .iter()
            .map(|m| ListItem::new(Line::from(m.clone())))
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut list_state = app.registry_model_list_state.clone();
        f.render_stateful_widget(list, list_area, &mut list_state);
    }

    // Draw filter input if in filter mode
    if let Some(filter_area) = filter_area {
        draw_registry_filter_input(f, app, filter_area);
    }
}

fn draw_registry_filter_input(f: &mut Frame, app: &AppState, area: Rect) {
    let input_style = if app.current_mode == AppMode::InstallSelectModelFilter {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Filter Registry Models")
        .border_style(input_style)
        .style(Style::default().bg(Color::DarkGray));

    // Create the input display with cursor
    let mut input_display = app.registry_filter_input.clone();
    if app.current_mode == AppMode::InstallSelectModelFilter {
        // Insert cursor character at cursor position (using ASCII-safe cursor)
        input_display.insert(app.registry_filter_cursor_pos, CURSOR_CHAR);
    }

    let input_paragraph = Paragraph::new(input_display)
        .block(block)
        .style(input_style);

    f.render_widget(input_paragraph, area);
}

fn draw_install_tag_select_dialog(f: &mut Frame, app: &AppState) {
    let model_name = app.selected_registry_model.as_deref().unwrap_or("Unknown");
    let title = format!("Install Model: Select Tag for '{}'", model_name);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));

    let area = centered_rect(60, 50, f.size());

    f.render_widget(Clear, area);

    if app.is_fetching_registry {
        let loading_text = Paragraph::new("Loading tags...")
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(loading_text, area);
    } else {
        let items: Vec<ListItem> = app
            .registry_tags
            .iter()
            .map(|t| ListItem::new(Line::from(t.clone())))
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut list_state = app.registry_tag_list_state.clone();
        f.render_stateful_widget(list, area, &mut list_state);
    }
}

fn draw_install_confirm_dialog(f: &mut Frame, app: &AppState) {
    let model = app.selected_registry_model.as_deref().unwrap_or("??");
    let tag = app.selected_registry_tag.as_deref().unwrap_or("??");
    let block = Block::default()
        .title("Confirm Installation")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));

    let text = format!("Install model '{}:{}'? (y/N)", model, tag);
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(ratatui::layout::Alignment::Center);

    let area = centered_rect(60, 20, f.size());

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Helper function to create a centered rectangle.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
