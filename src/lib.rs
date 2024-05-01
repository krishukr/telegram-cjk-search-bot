pub mod db;
pub mod handlers;
pub mod types;

pub static BOT_USERNAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
