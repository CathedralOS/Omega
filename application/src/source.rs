//! Reads one file and reports the first layer that refuses it. Build a line index only on failure.

use omega_lexer::{LexError, LineIndex, SourcePosition};
use omega_parser::ParseError;
use std::{fmt, fs, io, path::Path};

pub(crate) enum FileError<'a> {
    Read(&'a Path, io::Error),
    Lex(&'a Path, SourcePosition, LexError),
    Parse(&'a Path, SourcePosition, ParseError),
}

impl fmt::Display for FileError<'_> {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let (path, position, stage, error): (_, _, _, &dyn fmt::Display) = match self {
            Self::Read(path, error) => return write!(formatter, "{}: could not read source: {error}", path.display()),
            Self::Lex(path, position, error) => (path, position, "lexer", error),
            Self::Parse(path, position, error) => (path, position, "parser", error),
        };
        write!(
            formatter,
            "{}:{}:{}: {stage}: {error}",
            path.display(),
            position.get_line(),
            position.get_column()
        )
    }
}

pub(crate) fn parse_file(path: &Path) -> Result<(), FileError<'_>> {
    let bytes = fs::read(path).map_err(|error| FileError::Read(path, error))?;
    let position = |offset| LineIndex::new(&bytes).get_position(offset);
    let tokens = omega_lexer::lex(&bytes).map_err(|error| FileError::Lex(path, position(error.get_span().get_start_offset()), error))?;
    omega_parser::parse_file(&bytes, &tokens).map_err(|error| FileError::Parse(path, position(error.get_span().get_start_offset()), error))?;
    Ok(())
}
