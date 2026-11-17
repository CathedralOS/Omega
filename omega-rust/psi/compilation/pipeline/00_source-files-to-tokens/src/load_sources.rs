//! Read ordered source inputs before lexical scanning begins.

use diagnostics::Diagnostic;
use source::{DependencyScope, SourceContents, SourceFileSystem};
use std::path::PathBuf;

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
