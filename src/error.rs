use sms_client::error::ClientError;

#[derive(Debug, Clone)]
pub enum AppError {
    HttpError(String),
    ConfigError(String),
    SmsError(String)
}
impl std::error::Error for AppError {}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::HttpError(e) => write!(f, "HTTP Error: {}", e),
            AppError::ConfigError(e) => write!(f, "Config Error: {}", e),
            AppError::SmsError(e) => write!(f, "SMS Error: {}", e)
        }
    }
}
impl From<ClientError> for AppError {
    fn from(e: ClientError) -> Self {
        AppError::SmsError(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;