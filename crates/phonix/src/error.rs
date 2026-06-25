use thiserror::Error;

/// All fallible operations in phonix return [`Result`].
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced by the phonix core and (under `cli`) the binary adapters.
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to load model: {0}")]
    ModelLoad(String),
    #[error("unexpected model tensor shape: {0}")]
    ModelShape(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("audio device error: {0}")]
    Audio(String),
    #[error("resample error: {0}")]
    Resample(String),
    #[error("wav error: {0}")]
    Wav(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_includes_context() {
        let e = Error::ModelLoad("bad path".into());
        assert_eq!(e.to_string(), "failed to load model: bad path");
    }
}
