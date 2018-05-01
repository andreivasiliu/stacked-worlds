pub use failure::{Error, ResultExt};

#[derive(Debug, Fail)]
pub enum GameError {
    #[fail(display = "cannot create game window: {}", reason)]
    WindowError { reason: String }
}

