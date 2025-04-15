use crate::{
    error::AppError,
    events::AppEvent,
    ollama_api::OllamaClient,
    registry_api,
    tui,
};
use tokio::sync::mpsc;

type EventSender = mpsc::Sender<AppEvent>;

/// Fetches details for a specific model.
pub async fn fetch_model_details(client: OllamaClient, tx: EventSender, name: String) {
    let result = client.show_model_details(&name).await;
    let _ = tx
        .send(AppEvent::ModelDetailsFetched(result.map_err(AppError::Api)))
        .await;
}

/// Fetches the list of models from the Ollama registry.
pub async fn fetch_registry_models(tx: EventSender) {
    let result = registry_api::fetch_registry_models().await;
    let _ = tx.send(AppEvent::RegistryModelsFetched(result)).await;
}

/// Fetches the list of tags for a specific model from the Ollama registry.
pub async fn fetch_registry_tags(tx: EventSender, model_name: String) {
    let result = registry_api::fetch_registry_tags(&model_name).await;
    let _ = tx.send(AppEvent::RegistryTagsFetched(result)).await;
}

/// Deletes a local model and triggers a refresh.
pub async fn delete_model(client: OllamaClient, tx: EventSender, model_name: String) {
    match client.delete_model(&model_name).await {
        Ok(_) => {
            let refresh_result = client.list_models().await;
            let _ = tx
                .send(AppEvent::LocalModelsRefreshed(
                    refresh_result.map_err(AppError::Api),
                ))
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(AppEvent::ModelPullCompleted(Err(AppError::Api(e)))) // Reusing event for error reporting
                .await;
        }
    }
}

/// Pulls a model from the registry and triggers a refresh.
pub async fn pull_model(
    client: OllamaClient,
    tx: EventSender,
    model: String,
    tag: String,
) {
    let model_tag = format!("{}:{}", model, tag);

    if let Err(e) = tui::suspend_tui() {
        eprintln!("Error suspending TUI for pull: {}", e);
        // Optionally send an error event back?
    }

    println!("\n--- Starting 'ollama pull {}' ---", model_tag);
    println!("--- (Application will resume after pull completes) ---");

    let command_result = tokio::process::Command::new("ollama")
        .arg("pull")
        .arg(&model_tag)
        .status()
        .await;

    if let Err(e) = tui::resume_tui() {
        eprintln!("Error resuming TUI after pull: {}", e);
        // Optionally send an error event back?
    }

    let pull_result = match command_result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(AppError::Command(format!(
            "ollama pull command failed with status: {}",
            status
        ))),
        Err(e) => Err(AppError::Io(e)),
    };

    let _ = tx.send(AppEvent::ModelPullCompleted(pull_result)).await;

    // Trigger refresh regardless of pull success/failure
    let refresh_result = client.list_models().await;
    let _ = tx
        .send(AppEvent::LocalModelsRefreshed(
            refresh_result.map_err(AppError::Api),
        ))
        .await;
}

/// Runs 'ollama run' for the specified model.
pub async fn run_ollama(tx: EventSender, model_name: String) {
    let suspend_result = tui::suspend_tui();
    if let Err(e) = &suspend_result {
        eprintln!("Error suspending TUI for run: {}", e);
    }

    let run_result = match suspend_result {
        Ok(_) => {
            println!("\n--- Starting 'ollama run {}' ---", model_name);
            println!("--- (Type '/bye' or press Ctrl+D to exit) ---");

            // Use std::process::Command for blocking wait()
            let command_result = std::process::Command::new("ollama")
                .arg("run")
                .arg(&model_name)
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

            if let Err(e) = tui::resume_tui() {
                eprintln!("Error resuming TUI after run: {}", e);
                // Combine resume error with final_result?
                // For now, prioritize the command result error.
            }
            final_result
        }
        Err(e) => Err(e),
    };

    let _ = tx.send(AppEvent::OllamaRunCompleted(run_result)).await;
}