//! Read and parse source files, then resolve an explicitly supplied package. Build a line index on failure.

use omega_lexer::{LineIndex, SourceSpan};
use std::{fmt, fs, path::Path};

fn diagnostic(
    path: impl fmt::Display,
    bytes: Option<&[u8]>,
    span: SourceSpan,
    stage: &str,
    error: impl fmt::Display,
) -> String {
    let Some(bytes) = bytes else {
        return error.to_string();
    };
    let position = LineIndex::new(bytes).get_position(span.get_start_offset());
    format!("{path}:{}:{}: {stage}: {error}", position.get_line(), position.get_column())
}

pub(crate) fn parse_file(path: &Path) -> Result<(), String> {
    read_file(path).map(|_| ())
}

struct ParsedFile {
    bytes: Vec<u8>,
    tokens: Vec<omega_lexer::Token>,
    tree: omega_parser::SyntaxTree,
}

fn read_file(path: &Path) -> Result<ParsedFile, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: could not read source: {error}", path.display()))?;
    let tokens = omega_lexer::lex(&bytes).map_err(|e| diagnostic(path.display(), Some(&bytes), e.get_span(), "lexer", e))?;
    let tree = omega_parser::parse_file(&bytes, &tokens).map_err(|e| diagnostic(path.display(), Some(&bytes), e.get_span(), "parser", e))?;
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
        inputs.push((components.join("/"), read_file(path)?));
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
    let stage_error = |layer: &str, package: &str, path: &str, span, message: String| {
        let bytes = sources
            .iter()
            .find(|input| input.package == package && input.source.logical_path == path)
            .map(|input| input.source.text.as_bytes());
        diagnostic(path, bytes, span, layer, message)
    };
    let resolved = omega_toolchain::resolve_source_package(&sources).map_err(|e| stage_error("resolver", &e.package, &e.file, e.span, e.message))?;
    if matches!(stage, Stage::Type) {
        omega_typer::type_source_package(&resolved).map_err(|e| stage_error("typer", &e.package, &e.file, e.span, e.message))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diagnostic_without_local_source_bytes_preserves_the_stage_message() {
        assert_eq!(
            diagnostic("", None, SourceSpan::new(0, 0), "resolver", "conflicting dependency aliases"),
            "conflicting dependency aliases"
        );
    }
}
