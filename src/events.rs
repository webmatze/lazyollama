use crate::{
    error::Result,
    ollama_api::{ModelInfo, ShowModelResponse},
};

/// Define the types of events that can be sent from async tasks to the main loop
#[derive(Debug)]
pub enum AppEvent {
    ModelDetailsFetched(Result<ShowModelResponse>),
    RegistryModelsFetched(Result<Vec<String>>),
    RegistryTagsFetched(Result<Vec<String>>),
    ModelPullCompleted(Result<()>),
    LocalModelsRefreshed(Result<Vec<ModelInfo>>),
    OllamaRunCompleted(Result<()>),
}