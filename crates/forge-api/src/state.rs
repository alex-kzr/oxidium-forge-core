use forge_model::Config;
use forge_store::Store;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(store: Store, config: Config) -> Self {
        AppState {
            store: Arc::new(store),
            config: Arc::new(config),
        }
    }
}
