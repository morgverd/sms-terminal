#[derive(Debug)]
pub enum AppError {
    Http(sms_client::http::error::HttpError),
    Sms(sms_client::error::ClientError),
    Config(String),
}
impl std::error::Error for AppError {}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Http(e) => write!(f, "HTTP Error: {e}"),
            AppError::Sms(e) => write!(f, "SMS Error: {e}"),
            AppError::Config(e) => write!(f, "Config Error: {e}"),
        }
    }
}
impl From<sms_client::error::ClientError> for AppError {
    fn from(e: sms_client::error::ClientError) -> Self {
        AppError::Sms(e)
    }
}

pub type AppResult<T> = Result<T, AppError>;
