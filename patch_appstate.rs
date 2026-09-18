#[derive(Default, Clone)]
pub struct FolderState {
    pub status: String,
    pub is_syncing: bool,
    pub error: String,
}

#[derive(Default)]
pub struct AppState {
    pub tasks: std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>,
    pub folder_states: std::collections::HashMap<String, FolderState>,
}
