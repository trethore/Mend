use crate::parser::ParseError;
use crate::patcher::PatchError;
use std::error::Error;
use std::io;

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    Patch(PatchError),
    Parse(ParseError),
    Clipboard(String),
    EmptyDiff,
    NoInput,
    NoMatchingChanges { target_file: String },
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "A file system error occurred: {err}"),
            AppError::Patch(err) => write!(f, "{err}"),
            AppError::Parse(err) => write!(f, "Failed to parse the diff:\n{err}"),
            AppError::Clipboard(err) => write!(f, "Could not access the clipboard: {err}"),
            AppError::EmptyDiff => write!(f, "The provided diff content is empty."),
            AppError::NoInput => write!(
                f,
                "No diff file, clipboard flag, or stdin pipe was provided.\n\n\
                                           Usage examples:\n  \
                                           mend my_changes.diff\n  \
                                           mend -c path/to/file\n  \
                                           git diff | mend"
            ),
            AppError::NoMatchingChanges { target_file } => {
                write!(
                    f,
                    "The diff contains no changes for the specified file: {target_file}"
                )
            }
        }
    }
}

impl Error for AppError {}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<PatchError> for AppError {
    fn from(err: PatchError) -> Self {
        AppError::Patch(err)
    }
}

impl From<ParseError> for AppError {
    fn from(err: ParseError) -> Self {
        AppError::Parse(err)
    }
}
