#[derive(thiserror::Error, Debug)]
pub enum SubmissionError {
    #[error("Failed to fetch response")]
    FetchError,
    #[error("No submission found")]
    NoSubmission,
}
