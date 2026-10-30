//! Take in the stage's input: read an ordered list of source files into memory.
//!
//! All file reads finish here before `lex_sources` runs, so an unreadable file
//! is reported before any lexical error. In-memory inputs skip this step and go
//! straight to `lex_sources`.

use diagnostics::Diagnostic;
use source::{DependencyScope, SourceContents, SourceFileSystem};
use std::path::PathBuf;

/// Read an ordered list of files without assigning source coordinates. Equal
/// paths in different dependency scopes remain separate inputs. The supplied
/// view owns backing selection;
/// captured reads never reopen files. The first unreadable file stops loading.
pub fn load_sources(
    files: Vec<(PathBuf, DependencyScope)>,
    view: &SourceFileSystem,
) -> Result<Vec<SourceContents>, Vec<Diagnostic>> {
    let mut loaded = Vec::with_capacity(files.len());

    for (path, scope) in files {
        let source = view.read_to_string(&path).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to read {}: {error}",
                path.display()
            ))]
        })?;

        loaded.push(SourceContents {
            path,
            source,
            scope,
        });
    }

    Ok(loaded)
}

#[cfg(test)]
mod tests;
