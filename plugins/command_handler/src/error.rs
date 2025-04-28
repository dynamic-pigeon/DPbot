#[derive(thiserror::Error, Debug)]
pub enum SubmissionError {
    #[error("Failed to fetch response")]
    FetchError,
    #[error("Failed to parse response")]
    NoSubmission,
}
