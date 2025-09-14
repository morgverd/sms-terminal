
#[derive(Debug)]
pub enum AppError {
    HttpError(sms_client::http::error::HttpError),
    SmsError(sms_client::error::ClientError),
    ConfigError(String)
}
impl std::error::Error for AppError {}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::HttpError(e) => write!(f, "HTTP Error: {}", e.to_string()),
            AppError::SmsError(e) => write!(f, "SMS Error: {}", e.to_string()),
            AppError::ConfigError(e) => write!(f, "Config Error: {}", e)
        }
    }
}
impl From<sms_client::error::ClientError> for AppError {
    fn from(e: sms_client::error::ClientError) -> Self {
        AppError::SmsError(e)
    }
}

pub type AppResult<T> = Result<T, AppError>;