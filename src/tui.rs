use crate::error::{AppError, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{CrosstermBackend},
    Terminal,
};
use std::io::{self, Stdout};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initializes the terminal for TUI display.
/// Sets up raw mode, enters alternate screen, and enables mouse capture.
pub fn init_terminal() -> Result<Tui> {
    enable_raw_mode().map_err(AppError::Io)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).map_err(AppError::Io)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).map_err(AppError::Io)
}

/// Restores the terminal to its original state.
/// Disables raw mode, leaves alternate screen, and disables mouse capture.
pub fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode().map_err(AppError::Io)?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(AppError::Io)?;
    terminal.show_cursor().map_err(AppError::Io)
}

/// Temporarily suspends the TUI to allow external command execution.
pub fn suspend_tui() -> Result<()> {
    disable_raw_mode().map_err(AppError::Io)?;
    execute!(io::stdout(), LeaveAlternateScreen).map_err(AppError::Io)?;
    Ok(())
}

/// Resumes the TUI after suspension.
pub fn resume_tui() -> Result<()> {
    execute!(io::stdout(), EnterAlternateScreen).map_err(AppError::Io)?;
    enable_raw_mode().map_err(AppError::Io)?;
    Ok(())
}