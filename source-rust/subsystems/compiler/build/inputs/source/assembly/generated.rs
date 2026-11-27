//! Admit every file in the frozen view before resolving imports between them.

use super::imports::SourceRouting;
use crate::source::source_imports::{GeneratedFiles, ImportQueue, SourceStorage};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use source_admission::{SourceContents, SourceOrigin};
use source_compiler::SourceInput;
use std::sync::Arc;

pub(super) fn admit_generated_files(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    frozen: GeneratedFiles<'_>,
    routing: SourceRouting<'_>,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<
    Vec<(
        source_text::SourceId,
        build_filesystem::trees::PackageGeneratedSource,
    )>,
    Vec<Diagnostic>,
> {
    let entries = frozen.entries;
    let mut retained = Vec::with_capacity(entries.len());
    let mut files = Vec::with_capacity(entries.len());
    for entry in entries {
        imports.mark_loaded(&entry.logical_path, entry.scope);
        let source = Arc::from(entry.source_text()?);
        let admitted = source_storage.admit_source(
            SourceContents {
                path: entry.logical_path,
                source,
                scope: entry.scope,
            },
            Some(SourceOrigin::User),
            entry.stratum,
            catalogue,
        )?;
        retained.push((admitted.source_id, entry.source.clone()));
        files.push(admitted);
    }
    if !files.is_empty() {
        let parsed = SourceInput(files.into_iter().collect())
            .append(&mut source_storage.syntax_trees, timings)?;
        let discovered = routing.discover(&parsed, source_storage, &mut Vec::new())?;
        imports.enqueue(discovered)?;
        source_storage.attach_parsed(parsed);
    }
    Ok(retained)
}
