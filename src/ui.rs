// src/ui.rs
// Handles rendering the TUI layout and widgets.

use crate::app::{AppMode, AppState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize}, // Added Stylize
    text::{Line, Span, Text}, // Added Text
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

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

    // Draw confirmation dialog if needed
    if app.current_mode == AppMode::ConfirmDelete {
        if let Some(model_name) = app.get_selected_model_name() {
             draw_confirmation_dialog(f, &model_name);
        }
    }
}

fn draw_model_list(f: &mut Frame, app: &AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .models
        .iter()
        .map(|m| {
            ListItem::new(Line::from(Span::styled(
                m.name.clone(),
                Style::default(),
            )))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Models"))
        .highlight_style(
            Style::default()
                .bg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    // Temporary workaround: Pass a mutable clone of list_state
    let mut list_state = app.list_state.clone();
    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_model_details(f: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Details");

    let mut text_lines: Vec<Line> = Vec::new();

    if let Some(selected_index) = app.list_state.selected() {
        if let Some(basic_info) = app.models.get(selected_index) {
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
                Span::raw(basic_info.modified_at.clone()), // Consider formatting
            ]));
             text_lines.push(Line::from(vec![
                Span::styled("Digest: ", Style::default().bold()),
                Span::raw(basic_info.digest.chars().take(12).collect::<String>() + "..."),
            ]));
            text_lines.push(Line::from("")); // Spacer

            // Check if detailed info is available
            if let Some(details) = &app.selected_model_details {
                 text_lines.push(Line::from(Span::styled("--- Details ---", Style::default().italic())));

                 if let Some(extra) = &details.details {
                    if let Some(val) = &extra.family { text_lines.push(Line::from(vec![Span::styled("Family: ", Style::default().bold()), Span::raw(val)])); }
                    if let Some(val) = &extra.format { text_lines.push(Line::from(vec![Span::styled("Format: ", Style::default().bold()), Span::raw(val)])); }
                    if let Some(val) = &extra.parameter_size { text_lines.push(Line::from(vec![Span::styled("Param Size: ", Style::default().bold()), Span::raw(val)])); }
                    if let Some(val) = &extra.quantization_level { text_lines.push(Line::from(vec![Span::styled("Quant Level: ", Style::default().bold()), Span::raw(val)])); }
                    if let Some(families) = &extra.families {
                        if !families.is_empty() {
                             text_lines.push(Line::from(vec![Span::styled("Families: ", Style::default().bold()), Span::raw(families.join(", "))]));
                        }
                    }
                    // Add more fields from ModelExtraDetails if needed
                 }

                 if let Some(val) = &details.parameters { text_lines.push(Line::from("")); text_lines.push(Line::from(Span::styled("Parameters:", Style::default().bold()))); text_lines.push(Line::from(Span::raw(val.clone()))); }
                 if let Some(val) = &details.template { text_lines.push(Line::from("")); text_lines.push(Line::from(Span::styled("Template:", Style::default().bold()))); text_lines.push(Line::from(Span::raw(val.clone()))); }
                 if let Some(val) = &details.modelfile { text_lines.push(Line::from("")); text_lines.push(Line::from(Span::styled("Modelfile:", Style::default().bold()))); text_lines.push(Line::from(Span::raw(val.clone()))); }
                 if let Some(val) = &details.license { text_lines.push(Line::from("")); text_lines.push(Line::from(Span::styled("License:", Style::default().bold()))); text_lines.push(Line::from(Span::raw(val.clone()))); }

            } else {
                // Details not yet loaded
                if let Some(status) = &app.status_message {
                    if status.contains("Fetching") { // Check if fetching is in progress
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

    let paragraph = Paragraph::new(Text::from(text_lines)) // Use Text::from(Vec<Line>)
        .block(block)
        .wrap(Wrap { trim: false }); // Allow scrolling for long content

    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, app: &AppState, area: Rect) {
    let status_text = match &app.status_message {
        Some(msg) => msg.clone(),
        None => "q: Quit | ↓/j: Down | ↑/k: Up | d: Delete".to_string(),
    };

    let paragraph = Paragraph::new(Line::from(status_text))
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