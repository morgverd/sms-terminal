#[derive(Debug, Clone)]
pub enum AppError {
    HttpError(String),
    ConfigError(String),
    NoPhoneNumber,
}
impl std::error::Error for AppError {}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::HttpError(e) => write!(f, "HTTP Error: {}", e),
            AppError::ConfigError(e) => write!(f, "Config Error: {}", e),
            AppError::NoPhoneNumber => write!(f, "No phone number provided"),
        }
    }
}