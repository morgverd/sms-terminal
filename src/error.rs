
#[derive(Debug)]
pub enum AppError {
    HttpError(sms_client::http::error::HttpError),
    SmsError(sms_client::error::ClientError),
    ConfigError(String),
    ViewError(&'static str)
}
impl std::error::Error for AppError {}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::HttpError(e) => write!(f, "HTTP Error: {:?}", e),
            AppError::SmsError(e) => write!(f, "SMS Error: {:?}", e),
            AppError::ConfigError(e) => write!(f, "Config Error: {}", e),
            AppError::ViewError(e) => write!(f, "View Error: {}", e)
        }
    }
}
impl From<sms_client::error::ClientError> for AppError {
    fn from(e: sms_client::error::ClientError) -> Self {
        AppError::SmsError(e)
    }
}

pub type AppResult<T> = Result<T, AppError>;