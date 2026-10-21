//! Take in the stage's input: read an ordered list of source files into memory.
//!
//! All file reads finish here before `lex_sources` runs, so an unreadable file
//! is reported before any lexical error. In-memory inputs skip this step and go
//! straight to `lex_sources`.

use diagnostics::Diagnostic;
use source::{DependencyScope, LoadedSource, LoadedSources, SourceFileSystem, SourceId};
use std::path::PathBuf;

/// Read an ordered list of files, assigning consecutive source coordinates
/// from `first_source_id`. Equal paths in different dependency scopes remain
/// separate source instances. The supplied view owns backing selection;
/// captured reads never reopen files. The first unreadable file stops loading.
pub fn load_sources(
    files: Vec<(PathBuf, DependencyScope)>,
    first_source_id: usize,
    view: &SourceFileSystem,
) -> Result<LoadedSources, Vec<Diagnostic>> {
    let mut loaded = Vec::with_capacity(files.len());

    for (source_index, (path, scope)) in files.into_iter().enumerate() {
        let source = view.read_to_string(&path).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to read {}: {error}",
                path.display()
            ))]
        })?;

        loaded.push(LoadedSource {
            source_id: SourceId(first_source_id + source_index),
            path,
            source,
            origin: None,
            scope,
        });
    }

    Ok(loaded.into_iter().collect())
}

#[cfg(test)]
mod tests;
