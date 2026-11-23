//! Runs source packages through L0-L5 and keeps the first refusal with its producer and source location.
#![allow(dead_code)]
use omega_lexer::{LineIndex, SourceSpan};
#[path = "../support/preparation.rs"]
mod preparation;
use omega_resolver::{ParsedSource, StageSource};
use std::fmt;

/// The package identity given to files supplied without one.
pub const ROOT: &str = "root";

/// The first refusal, located only in bytes that belong to the refusing package's file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    /// The producing layer, including `L3-resumed` after a static answer.
    pub stage: &'static str,
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    /// One-based line and byte column when the package file is local and contains the span start.
    pub position: Option<(usize, usize)>,
    /// The bytes covered by the span when they are local. A span that is empty or at the end of the file covers no bytes.
    pub occurrence: Option<Vec<u8>>,
    /// The producer's complete reason.
    pub reason: String,
}

impl Refusal {
    pub fn locate(
        stage: &'static str,
        package: &str,
        file: &str,
        span: SourceSpan,
        reason: String,
        sources: &[(&str, &str, &[u8])],
    ) -> Self {
        let bytes = sources
            .iter()
            .find(|(owner, path, _)| *owner == package && *path == file)
            .map(|(_, _, bytes)| *bytes);
        let position = bytes
            .filter(|bytes| span.get_start_offset() <= bytes.len())
            .map(|bytes| {
                let position = LineIndex::new(bytes).get_position(span.get_start_offset());
                (position.get_line(), position.get_column())
            });
        Self {
            stage,
            package: package.into(),
            file: file.into(),
            span,
            position,
            occurrence: bytes.and_then(|bytes| span.slice(bytes)).map(<[u8]>::to_vec),
            reason,
        }
    }

    /// One `OMEGA_SAMPLE_REFUSAL` line. `-` marks an unavailable value; a local empty or end-of-file span leaves the occurrence empty.
    pub fn marker(
        &self,
        label: &str,
    ) -> String {
        format!("OMEGA_SAMPLE_REFUSAL\t{}", self.baseline_row(label))
    }

    pub fn baseline_row(&self, label: &str) -> String {
        for field in [label, self.package.as_str(), self.file.as_str()] {
            assert!(!field.contains(['\t', '\r', '\n']), "marker field needs escaping: {field:?}");
        }
        let optional = |value: Option<usize>| value.map_or_else(|| "-".to_owned(), |value| value.to_string());
        let occurrence = match self.occurrence.as_deref() {
            None => "-".to_owned(),
            // A literal dash must not read as an unavailable occurrence.
            Some(b"-") => "%2D".to_owned(),
            Some(bytes) => escape_bytes(bytes),
        };
        format!(
            "{label}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{occurrence}\t{}",
            self.stage,
            self.package,
            self.file,
            self.span.get_start_offset(),
            self.span.get_end_offset(),
            optional(self.position.map(|(line, _)| line)),
            optional(self.position.map(|(_, column)| column)),
            escape(&self.reason)
        )
    }
}

/// The baseline row form: `stage<TAB>file:line:column<TAB>reason` for a local root file.
impl fmt::Display for Refusal {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let owner = if self.package == ROOT {
            String::new()
        } else {
            format!("<{}>/", self.package)
        };
        match self.position {
            Some((line, column)) => write!(formatter, "{}\t{owner}{}:{line}:{column}\t{}", self.stage, self.file, self.reason),
            None => write!(
                formatter,
                "{}\t<{}>/{} (bytes {}..{})\t{}",
                self.stage,
                self.package,
                self.file,
                self.span.get_start_offset(),
                self.span.get_end_offset(),
                self.reason
            ),
        }
    }
}

pub use spec_corpus::diagnostic_field as escape;

/// Escapes text like `escape` and writes each byte of an invalid UTF-8 sequence as `%XX`.
fn escape_bytes(mut bytes: &[u8]) -> String {
    let mut escaped = String::new();
    loop {
        match std::str::from_utf8(bytes) {
            Ok(text) => {
                escaped.push_str(&escape(text));
                return escaped;
            }
            Err(error) => {
                let (valid, rest) = bytes.split_at(error.valid_up_to());
                escaped.push_str(&escape(std::str::from_utf8(valid).unwrap()));
                let invalid = error.error_len().unwrap_or(rest.len());
                for byte in &rest[..invalid] {
                    escaped.push_str(&format!("%{byte:02X}"));
                }
                bytes = &rest[invalid..];
            }
        }
    }
}

/// Lowers one package whose files all belong to `ROOT`.
pub fn lower(files: &[(&str, Vec<u8>)]) -> Result<omega_psi::LoweredPackage, Refusal> {
    let sources: Vec<_> = files
        .iter()
        .map(|(path, bytes)| (ROOT, *path, bytes.as_slice()))
        .collect();
    lower_packages(&sources)
}

/// Lowers `(package, package-relative file, bytes)` sources together.
pub fn lower_packages(files: &[(&str, &str, &[u8])]) -> Result<omega_psi::LoweredPackage, Refusal> {
    lower_admitted(files, &[])
}

/// Lowers sources together with the direct dependency edges Build admitted for their packages.
pub fn lower_admitted(
    files: &[(&str, &str, &[u8])],
    admitted: &[omega_resolver::Dependency],
) -> Result<omega_psi::LoweredPackage, Refusal> {
    let refuse = |stage: &'static str, package: &str, file: &str, span: SourceSpan, reason: String| Refusal::locate(stage, package, file, span, reason, files);
    let mut token_files = Vec::new();
    for (package, path, bytes) in files {
        token_files.push(omega_lexer::lex(bytes).map_err(|err| refuse("L0", package, path, err.get_span(), err.to_string()))?);
    }
    let mut trees = Vec::new();
    for ((package, path, bytes), tokens) in files.iter().zip(&token_files) {
        trees.push(omega_parser::parse_file(bytes, tokens).map_err(|err| refuse("L1", package, path, err.get_span(), err.to_string()))?);
    }
    let mut sources = Vec::new();
    for (((package, path, bytes), tokens), tree) in files.iter().zip(&token_files).zip(&trees) {
        // L0 has already refused invalid UTF-8; keep its refusal shape if that ever changes.
        let text = std::str::from_utf8(bytes).map_err(|err| {
            let start = err.valid_up_to();
            let end = start + err.error_len().unwrap_or(bytes.len() - start);
            refuse("L0", package, path, SourceSpan::new(start, end), err.to_string())
        })?;
        sources.push(StageSource {
            package,
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        });
    }
    let resolved =
        omega_toolchain::resolve_admitted_packages(&sources, admitted).map_err(|err| refuse("L2", &err.package, &err.file, err.span, err.message))?;
    let prepared = preparation::prepare(&resolved).map_err(|(stage, err)| refuse(stage, &err.package, &err.file, err.span, err.message))?;
    omega_lowering::lower_package(&prepared).map_err(|err| refuse("L5", &err.site.package, &err.site.file, err.site.span, err.message))
}
