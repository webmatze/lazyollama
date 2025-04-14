# LazyOllama - Easy llama, easy life
![easy_llama_easy_life](https://github.com/user-attachments/assets/8dc90386-4f91-47d5-81c9-37e30feaea17)

A terminal user interface (TUI) application for managing local Ollama models, written in Rust.

https://github.com/user-attachments/assets/5fcdb0eb-c398-4662-aed9-4fd8359d7063

## Features

*   **List Models:** Displays a scrollable list of locally installed Ollama models.
*   **Run Models:** Run any of the locally installed Ollama models.
*   **Inspect Models:** Shows detailed information for the selected model (size, modification date, digest, family, parameters, etc.).
*   **Delete Models:** Allows deleting the selected model with a confirmation prompt.
*   **Install Models:** Allows to pull new models from the ollama registry.
*   **Environment Variable:** Uses `OLLAMA_HOST` environment variable for the Ollama API endpoint (defaults to `http://localhost:11434`).

## Installation

### Prerequisites

*   Rust toolchain (Install from [rustup.rs](https://rustup.rs/))
*   A running Ollama instance ([ollama.com](https://ollama.com/))

### Method 1: Using the Installation Script (Recommended)

This is the simplest way to build and install LazyOllama to a system-wide location:

```bash
# 1. Clone the repository
git clone https://github.com/webmatze/lazyollama.git
cd lazyollama

# 2. Run the installation script
chmod +x install.sh
./install.sh
```

The script will:
- Check for required dependencies
- Build the release version
- Install it to the appropriate location for your OS (typically `/usr/local/bin` on Unix-like systems)
- Set appropriate permissions

### Method 2: Using Cargo Install

If you have Rust installed, you can install directly using Cargo:

```bash
# 1. Clone the repository
git clone https://github.com/webmatze/lazyollama.git
cd lazyollama

# 2. Install using cargo
cargo install --path .
```

This will install the binary to your Cargo bin directory (typically `~/.cargo/bin/`), which should be in your PATH.

### Method 3: Manual Build and Installation

If you prefer to manually build and place the binary:

```bash
# 1. Clone the repository
git clone https://github.com/webmatze/lazyollama.git
cd lazyollama

# 2. Build the application
cargo build --release

# 3. Copy the binary to a location in your PATH (optional)
# On Linux/macOS (may require sudo)
sudo cp target/release/lazyollama /usr/local/bin/
```

The executable will be located at `target/release/lazyollama`.

### Platform-Specific Considerations

- **Linux/macOS**: Installation to system directories (like `/usr/local/bin`) typically requires root privileges (sudo).
- **Windows**: The installation script will attempt to install to an appropriate location, but you may need to adjust your PATH environment variable.

### Verifying Installation

After installation, verify that lazyollama is correctly installed and accessible:

```bash
# Check if the command is available
which lazyollama

# Run lazyollama
lazyollama
```

If the command isn't found, ensure the installation location is in your PATH.

## Usage

1.  **Run the application:**
    ```bash
    lazyollama
    ```
2.  **Set Custom Ollama Host (Optional):**
    If your Ollama instance is running on a different host or port, set the `OLLAMA_HOST` environment variable before running:
    ```bash
    export OLLAMA_HOST="http://your-ollama-host:port"
    lazyollama
    ```

## Keybindings

*   `q`: Quit the application.
*   `↓` / `j`: Move selection down.
*   `↑` / `k`: Move selection up.
*   `d`: Initiate deletion of the selected model (shows confirmation).
*   `y` / `Y`: Confirm deletion (when in confirmation mode).
*   `n` / `N` / `Esc`: Cancel deletion (when in confirmation mode).
*   `i`: Install/Pull new models
*   `Enter`: Run selected model in ollama

## Dependencies

This project uses the following main Rust crates:

*   `ratatui`: For building the TUI.
*   `crossterm`: Terminal manipulation backend for `ratatui`.
*   `tokio`: Asynchronous runtime.
*   `reqwest`: HTTP client for interacting with the Ollama API.
*   `serde`: For serializing/deserializing API data.
*   `humansize`: For formatting file sizes.
*   `thiserror`: For error handling boilerplate.
*   `dotenvy`: (Optional) For loading `.env` files if needed.

See `Cargo.toml` for the full list and specific versions.

## Architecture Overview

The application follows a simple event loop architecture:

1.  **Initialization:** Sets up the terminal, initializes `AppState`, and fetches the initial list of models from the Ollama API.
2.  **Event Loop:**
    *   Draws the UI based on the current `AppState`.
    *   Checks for user input (keyboard events) and results from background tasks (via channels).
    *   Handles input: Updates `AppState` (e.g., changes selection, enters delete mode, quits).
    *   Handles background task results (e.g., updates model details).
    *   Triggers background tasks (e.g., fetching model details) when necessary.
3.  **Cleanup:** Restores the terminal state on exit.

## Architecture Diagram

```mermaid
graph TD
    A[User Input] --> B[Event Loop]
    B --> C[AppState]
    C --> D[UI Renderer]
    D --> E[Terminal Display]
    
    B --> F[Background Tasks]
    F --> G[Ollama API]
    G --> F
    F --> C
    
    subgraph Event Handler
        B
        C
    end
    
    subgraph UI Layer
        D
        E
    end
    
    subgraph API Layer
        F
        G
    end
```

## Troubleshooting

*   **Connection Errors:** Ensure your Ollama instance is running and accessible at the specified `OLLAMA_HOST` (or the default `http://localhost:11434`). Check firewalls if necessary.
*   **API Errors:** If the Ollama API returns errors, they should be displayed in the status bar. Refer to the Ollama server logs for more details.
*   **Rendering Issues:** Terminal rendering can vary. Ensure you are using a modern terminal emulator with good Unicode and color support.
