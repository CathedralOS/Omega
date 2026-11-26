//! Seed the selected core Build, prelude and language vocabulary from captured source.

use super::BuildSourceSelection;
use super::imports::{SourceRouting, load_source_frontiers};
use crate::source::source_imports::{ImportQueue, ResolvedSourceImport, SourceStorage};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use source_admission::{SourceContents, SourceOrigin, SourceResolutionStratum};
use source_compiler::SourceInput;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(super) fn append_default_build(
    storage: &mut SourceStorage,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<BuildSourceSelection, Vec<Diagnostic>> {
    let core = storage.core_capture().ok_or_else(|| {
        vec![Diagnostic::error(
            "default Build requires the captured core library",
        )]
    })?;
    let source = core
        .tree()
        .read_to_string(Path::new("defaults/build.omg"))
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot read core's default Build: {error}"
            ))]
        })?;
    let admitted = storage.admit_source(
        SourceContents {
            path: PathBuf::from(source_admission::source_identity::DEFAULT_BUILD_PATH),
            source,
            scope: source_provenance::DependencyScope::Build,
        },
        Some(SourceOrigin::Toolchain),
        SourceResolutionStratum::Base,
        catalogue,
    )?;
    let source_id = admitted.source_id;
    // A package with no Build-scope source of its own has parsed nothing
    // yet, so the default Build is the first source of this cohort and its
    // primary coordinate, as the first loaded source is in `load_imports`.
    // Only the first package of a compilation gets coordinate zero by
    // accident; a dependency without `build.omg` is admitted later.
    if storage.source_roots.is_empty() {
        storage.syntax_trees.set_primary_source(admitted.source_id);
    }
    let parsed = SourceInput([admitted].into_iter().collect())
        .append(&mut storage.syntax_trees, timings)?;
    let [unit] = parsed.sources.span_or_empty(parsed.batch) else {
        return Err(vec![Diagnostic::error(
            "parser lost the core default Build source",
        )]);
    };
    let [name] = unit.root_items.build_machines() else {
        return Err(vec![Diagnostic::error(
            "core's default Build must declare one free build machine",
        )]);
    };
    let entry = source_text::SourceSpan::new(source_id, *name);
    storage.attach_parsed(parsed);
    Ok(BuildSourceSelection::compiler_default(entry))
}

/// Adds the toolchain's Build prelude to a program that has a Build script.
///
/// The prelude is ordinary source and names core declarations through `use`
/// items, as any Build-scope source may (`wiki/spec/build/scoped_execution.md`,
/// "Two checked contexts": "Core's exact toolchain vocabulary is directly
/// available"). Its imports are therefore discovered and loaded under
/// `routing`, the rules every other source of this compilation went through.
/// It is added after the program's own sources are loaded because whether it
/// is needed, and under which name, is read from them.
pub(super) fn append_build_prelude(
    storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    routing: SourceRouting<'_>,
    build_source: Option<BuildSourceSelection>,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<
    Vec<source_compiler::representations::source_symbols::SourceScopedTopLevelBinding>,
    Vec<Diagnostic>,
> {
    let Some(prelude) = crate::source::source_imports::prepare_build_prelude(
        storage,
        build_source.map(BuildSourceSelection::source_id),
    ) else {
        return Ok(Vec::new());
    };
    let admitted = storage.admit_source(
        SourceContents {
            path: PathBuf::from(source_admission::source_identity::BUILD_PRELUDE_ROOT),
            source: Arc::from(prelude.source.declarations),
            scope: source_provenance::DependencyScope::Build,
        },
        Some(SourceOrigin::Toolchain),
        SourceResolutionStratum::Base,
        catalogue,
    )?;
    let mut bindings: Vec<_> = prelude
        .alias_for
        .map(|build_source| {
            source_compiler::representations::source_symbols::SourceScopedTopLevelBinding::new(
                build_source,
                admitted.source_id,
                "Build",
            )
        })
        .into_iter()
        .collect();
    let mut units = vec![admitted];
    let mut namespaces = Vec::with_capacity(prelude.source.modules.len());
    for (namespace, text) in prelude.source.modules {
        let admitted = storage.admit_source(
            SourceContents {
                path: Path::new(source_admission::source_identity::BUILD_PRELUDE_ROOT)
                    .join(namespace)
                    .join("mod.omg"),
                source: Arc::from(text),
                scope: source_provenance::DependencyScope::Build,
            },
            Some(SourceOrigin::Toolchain),
            SourceResolutionStratum::Base,
            catalogue,
        )?;
        namespaces.push((namespace, admitted.source_id));
        units.push(admitted);
    }
    let parsed = SourceInput(units.into_iter().collect())
        .append(&mut storage.syntax_trees, timings)?;
    let mut package_requests = Vec::new();
    let discovered = routing.discover(&parsed, storage, &mut package_requests)?;
    storage.attach_parsed(parsed);
    imports.enqueue(discovered)?;
    package_requests.extend(load_source_frontiers(
        storage, imports, routing, timings, catalogue,
    )?);
    if !package_requests.is_empty() {
        return Err(vec![Diagnostic::error(
            "the toolchain Build prelude imports only core, and one of its imports selects a package",
        )]);
    }
    // Build's directly available vocabulary includes real code namespaces.
    // Import their admitted source identities just as the loader does for an
    // authored module, rather than making a same-spelled data type own helpers.
    for file in storage
        .sources
        .selected_files()
        .filter(|file| file.dependency_scope == source_provenance::DependencyScope::Build)
    {
        for &(namespace, declaration_source) in &namespaces {
            bindings.push(
                source_compiler::representations::source_symbols::SourceScopedTopLevelBinding::module_import(
                    file.source_id,
                    declaration_source,
                    namespace,
                    0,
                ),
            );
        }
    }
    for (_, file) in storage.source_roots.iter() {
        for import in &file.build_imports {
            let source_compiler::SourceImportTarget::File { root, path, alias } = &import.target
            else {
                continue;
            };
            if !import.only_product_operands
                && let Some(namespace) =
                    build_declarations::build_file_namespace(root.as_deref(), path)
            {
                let (_, source_id) = namespaces
                    .iter()
                    .find(|(name, _)| *name == namespace)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "Build library import lost its compiler-owned source",
                        )]
                    })?;
                let declaration = storage
                    .sources
                    .get(*source_id)
                    .expect("just-admitted Build library");
                storage.resolved_imports.push(ResolvedSourceImport::file(
                    file.source_id,
                    import.ordinal,
                    import.target.to_string(),
                    alias,
                    declaration,
                ));
            }
        }
    }
    Ok(bindings)
}

// Both ordinary assembly and a paused Build's Product query use the same
// captured core declarations. Queueing a path grants no source authority:
// loading still goes through SourceStorage's immutable capture and scoped
// source selection before resolution authenticates any catalog declaration.
pub(super) fn seed_core_vocabulary(
    imports: &mut ImportQueue,
    core_root: &Path,
    scope: source_provenance::DependencyScope,
) {
    for file in ["code_layout.omg", "indexing/indexing.omg"] {
        imports.seed(core_root.join(file), scope);
    }
}

pub(super) fn sources_mention_floats(
    source_storage: &crate::source::source_imports::SourceStorage,
    scope: source_provenance::DependencyScope,
) -> bool {
    source_storage
        .sources
        .selected_files()
        .filter(|file| file.dependency_scope == scope)
        .any(|file| {
            let text = file.source.as_ref();
            text.contains("f32")
                || text.contains("f64")
                || text.contains("Float")
                || text.contains("F32")
                || text.contains("F64")
        })
        || source_storage
            .syntax_trees
            .float_literal_spans()
            .any(|span| {
                source_storage.sources.file_at(span).is_some_and(|file| {
                    source_storage.sources.source_is_selected(file.source_id)
                        && file.dependency_scope == scope
                })
            })
}
