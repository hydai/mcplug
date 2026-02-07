pub mod editors;
pub mod env;
pub mod loader;
pub mod types;

pub use loader::load_config;
pub use types::{AnnotatedServerConfig, Lifecycle, McplugConfig, ServerConfig};
