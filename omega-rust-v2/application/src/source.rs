//! Read and parse source files, then resolve an explicitly supplied package. Build a line index on failure.

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
    read_file(path).map(|_| ())
}

struct ParsedFile {
    bytes: Vec<u8>,
    tokens: Vec<omega_lexer::Token>,
    tree: omega_parser::SyntaxTree,
}

fn read_file(path: &Path) -> Result<ParsedFile, FileError<'_>> {
    let bytes = fs::read(path).map_err(|error| FileError::Read(path, error))?;
    let position = |offset| LineIndex::new(&bytes).get_position(offset);
    let tokens = omega_lexer::lex(&bytes).map_err(|error| FileError::Lex(path, position(error.get_span().get_start_offset()), error))?;
    let tree = omega_parser::parse_file(&bytes, &tokens).map_err(|error| FileError::Parse(path, position(error.get_span().get_start_offset()), error))?;
    Ok(ParsedFile { bytes, tokens, tree })
}

pub(crate) enum Stage {
    Resolve,
    Type,
}

pub(crate) fn check_files(
    paths: &[std::ffi::OsString],
    stage: Stage,
) -> Result<(), String> {
    use std::path::Component;
    let mut inputs = Vec::new();
    for path in paths.iter().map(Path::new) {
        let mut components = Vec::new();
        for part in path.components() {
            match part {
                Component::Normal(name) => components.push(
                    name.to_str()
                        .ok_or_else(|| format!("{}: logical paths must be UTF-8", path.display()))?,
                ),
                Component::CurDir => {}
                _ => return Err(format!("{}: expected a path within the current package", path.display())),
            }
        }
        inputs.push((components.join("/"), read_file(path).map_err(|e| e.to_string())?));
    }
    let mut sources = Vec::with_capacity(inputs.len());
    for (logical_path, file) in &inputs {
        let text = std::str::from_utf8(&file.bytes).map_err(|error| format!("{logical_path}: {error}"))?;
        sources.push(omega_resolver::StageSource {
            package: "root",
            source: omega_resolver::ParsedSource {
                logical_path,
                text,
                tokens: &file.tokens,
                tree: &file.tree,
            },
        });
    }
    let diagnostic = |layer: &str, path: &str, span: omega_lexer::SourceSpan, message: String| {
        let Some((_, file)) = inputs.iter().find(|(name, _)| name == path) else {
            return message;
        };
        let position = LineIndex::new(&file.bytes).get_position(span.get_start_offset());
        format!("{path}:{}:{}: {layer}: {message}", position.get_line(), position.get_column())
    };
    let resolved = omega_typer::resolve_source_package(&sources).map_err(|e| diagnostic("resolver", &e.file, e.span, e.message))?;
    if matches!(stage, Stage::Type) {
        omega_typer::type_source_package(&resolved).map_err(|e| diagnostic("typer", &e.file, e.span, e.message))?;
    }
    Ok(())
}
