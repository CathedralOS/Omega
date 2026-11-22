//! `read_file` lexes and parses one file. `check_files` resolves the package, runs the requested stage and formats its diagnostics.

use omega_lexer::{LineIndex, SourceSpan};
use std::{fmt, fs, path::Path, sync::Arc};

fn diagnostic(
    path: impl fmt::Display,
    bytes: Option<&[u8]>,
    span: SourceSpan,
    stage: &str,
    error: impl fmt::Display,
) -> String {
    let path = path.to_string();
    let Some(bytes) = bytes else {
        return if path.is_empty() {
            format!("{stage}: {error}")
        } else {
            format!("{path}: {stage}: {error} (bytes {}..{})", span.get_start_offset(), span.get_end_offset())
        };
    };
    let position = LineIndex::new(bytes).get_position(span.get_start_offset());
    format!(
        "{path}:{}:{}: {stage}: {error} (bytes {}..{})",
        position.get_line(),
        position.get_column(),
        span.get_start_offset(),
        span.get_end_offset()
    )
}

pub(crate) fn parse_file(path: &Path) -> Result<(), String> {
    read_file(path).map(|_| ())
}

struct ParsedFile {
    bytes: Arc<[u8]>,
    tokens: Vec<omega_lexer::Token>,
    tree: omega_parser::SyntaxTree,
}

fn read_file(path: &Path) -> Result<ParsedFile, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: could not read source: {error}", path.display()))?;
    parse_bytes(path, Arc::from(bytes))
}

fn parse_bytes(
    path: &Path,
    bytes: Arc<[u8]>,
) -> Result<ParsedFile, String> {
    let tokens = omega_lexer::lex(&bytes).map_err(|e| diagnostic(path.display(), Some(&bytes), e.get_span(), "lexer", e.get_kind().get_message()))?;
    let tree = omega_parser::parse_file(&bytes, &tokens)
        .map_err(|e| diagnostic(path.display(), Some(&bytes), e.get_span(), "parser", e.get_kind().get_message(e.get_expected())))?;
    Ok(ParsedFile { bytes, tokens, tree })
}

pub(crate) enum Stage {
    Resolve,
    Type,
    Prepare,
    Lower,
    CheckScalarReplacements,
}

fn read_package_file(
    path: &Path,
    build: Option<&omega_toolchain::DefaultBuild>,
) -> Result<(String, ParsedFile), String> {
    use std::path::Component;
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
    let logical_path = components.join("/");
    let file = match build {
        Some(build) => {
            let bytes = build
                .source_bytes(Path::new(&logical_path))
                .map_err(|error| format!("{logical_path}: could not read source: {error}"))?;
            parse_bytes(path, bytes)?
        }
        None => read_file(path)?,
    };
    Ok((logical_path, file))
}

pub(crate) fn check_files(
    paths: &[std::ffi::OsString],
    stage: Stage,
    root: Option<&Path>,
) -> Result<(), String> {
    let build = root
        .map(|root| omega_toolchain::DefaultBuild::capture(root).map_err(|error| format!("{}: Build: {error}", root.display())))
        .transpose()?;
    let mut inputs = Vec::new();
    let mut file_errors = Vec::new();
    for path in paths.iter().map(Path::new) {
        match read_package_file(path, build.as_ref()) {
            Ok(input) => inputs.push(input),
            Err(error) => file_errors.push(error),
        }
    }
    // Each file can be parsed independently. Resolution needs the complete package.
    if !file_errors.is_empty() {
        return Err(file_errors.join("\n"));
    }
    let mut sources = Vec::with_capacity(inputs.len());
    for (logical_path, file) in &inputs {
        let text = std::str::from_utf8(&file.bytes).map_err(|error| format!("{logical_path}: {error}"))?;
        sources.push(omega_resolver::StageSource {
            package: omega_toolchain::DEFAULT_ROOT_PACKAGE,
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
        if bytes.is_none() && !package.is_empty() && path.is_empty() {
            format!("{package}: {layer}: {message}")
        } else if bytes.is_none() && !package.is_empty() {
            let origin = format!("{package}/{path}");
            diagnostic(origin, None, span, layer, message)
        } else {
            diagnostic(path, bytes, span, layer, message)
        }
    };
    let mut resolved = omega_toolchain::resolve_source_package(&sources).map_err(|e| stage_error("resolver", &e.package, &e.file, e.span, e.message))?;
    if let Some(build) = &build {
        resolved.bind_package_key(omega_toolchain::DEFAULT_ROOT_PACKAGE, build.package_key())?;
    }
    match stage {
        Stage::Resolve => {}
        Stage::Type => {
            omega_typer::type_source_package(&resolved).map_err(|e| stage_error("typer", &e.package, &e.file, e.span, e.message))?;
        }
        Stage::Prepare | Stage::Lower | Stage::CheckScalarReplacements => {
            use omega_preparation::PreparationFailure;
            let mut prepared = omega_preparation::prepare_source_package_diagnosed(&resolved).map_err(|failure| {
                let (stage, e) = match failure {
                    PreparationFailure::Typing(e) => ("typer", e),
                    // L3 owns the refusal, but only after L4 evaluated a demanded identity.
                    PreparationFailure::ResumedTyping(e) => ("typer (after static evaluation)", e),
                    PreparationFailure::Preparation(refusal) => ("preparation", refusal.diagnostic),
                    PreparationFailure::MissingTarget(refusal) => ("preparation (target input)", refusal.diagnostic),
                };
                stage_error(stage, &e.package, &e.file, e.span, e.message)
            })?;
            if matches!(stage, Stage::Lower | Stage::CheckScalarReplacements) {
                let answers = omega::prepare_policy_answers(&mut prepared, omega_interpreter::Ceilings::default())
                    .map_err(|e| stage_error("policy preparation", &e.site.package, &e.site.file, e.site.span, e.message))?;
                let package = omega::lower_package_with_policy_answers(&prepared, &answers)
                    .map_err(|e| stage_error("lowering", &e.site.package, &e.site.file, e.site.span, e.message))?;
                if matches!(stage, Stage::CheckScalarReplacements) {
                    omega_checker::check_scalar_replacements(&package)
                        .map_err(|e| stage_error("checker", &e.site.package, &e.site.file, e.site.span, e.message))?;
                }
            }
        }
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
            "resolver: conflicting dependency aliases"
        );
        assert_eq!(
            diagnostic("library/types.omg", None, SourceSpan::new(7, 10), "typer", "unknown carrier"),
            "library/types.omg: typer: unknown carrier (bytes 7..10)"
        );
    }
}
