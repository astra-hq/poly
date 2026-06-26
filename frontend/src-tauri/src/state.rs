use crate::database::manager::DatabaseManager;
use crate::poly_config::ConfigRepository;

pub struct AppState {
    pub db_manager: DatabaseManager,
    pub config_repo: ConfigRepository,
}
