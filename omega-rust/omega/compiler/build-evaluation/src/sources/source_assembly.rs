use artifacts::compile_timings::CompileTimings;
use artifacts::compile_timings::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use package_compilation::source_imports::{SourceStorage, validate_package_source_frontier};
use package_compilation::source_imports::{discover_imports, discover_imports_with_packages};
use source_files_to_tokens::ImportQueue;
use source_files_to_tokens::{lex_sources, load_package_generated_source, load_sources};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use tokens_to_syntax_trees::parse_sources;

mod checkpoint;
pub use checkpoint::ImmutableSourceParseCheckpoint;

#[derive(Clone)]
pub struct AssembledSyntax {
    pub syntax_trees: SyntaxTrees,
    pub sources: Arc<source::SourceMap>,
    /// Exact companion `build.omg` selected during project discovery. Build
    /// authority is attached to this source, never reconstructed from a leaf
    /// filename after imports have expanded the source frontier.
    pub build_source_id: Option<source::SourceId>,
    /// The validated authored application declaration (name and artifact-only
    /// intent) when the selected build root declares `builder.application(...)`;
    /// `None` for package/workspace roles or a missing build root. Publication
    /// never re-derives it.
    pub application: Option<build_declarations::ApplicationDeclaration>,
    pub source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    /// Sources the build scope owns: the selected build entry, the
    /// root-local helpers it transitively imports, and every source of a
    /// package the root reaches only through build-purpose edges (including
    /// such a package's own ordinary dependencies) — its physical files and
    /// the generated source its build handed off alike, since both execute in
    /// the same host context. Their target-scoped declarations select against
    /// the admitted build execution profile, never the product target; every
    /// other source — product packages, dual-purpose packages, and their
    /// generated dependency source — is product scope.
    pub build_scope_sources: HashSet<source::SourceId>,
}

fn append_dependency_generated_sources_to_storage(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    package_inputs: &PackageCompilationInputs,
    timings: &mut CompileTimings,
) -> Result<Vec<(source::SourceId, build_output::PackageGeneratedSource)>, Vec<Diagnostic>> {
    // A virtual path can name different bytes in the two checked scopes.
    // Route from the source graph to its exact activation bundle, not from a
    // package-wide 'build-only' classification. The acquisition remains shared.
    let mut entries = Vec::new();
    for (purpose, bundle) in package_inputs.dependency_generated_source_instances() {
        let scope = match purpose {
            build_declarations::DependencyPurpose::Product => source::DependencyScope::Product,
            build_declarations::DependencyPurpose::Build => source::DependencyScope::Build,
        };
        let package_root = package_inputs
            .package_root(bundle.package())
            .expect("validated generated-source bundle retains its package root");
        for source in bundle.sources() {
            // Keep path errors in authored order with physical collisions.
            entries.push((
                generated_source_logical_path(package_root, source),
                source,
                bundle.package(),
                scope,
            ));
        }
    }
    {
        let mut loaded_paths = HashSet::with_capacity(entries.len());
        for (logical_path, _, _, scope) in &entries {
            let logical_path = logical_path.as_ref().map_err(Clone::clone)?;
            if logical_path.exists() || !loaded_paths.insert((logical_path.as_path(), *scope)) {
                return Err(vec![Diagnostic::error(format!(
                    "generated dependency source logical path `{}` collides with another source",
                    logical_path.display(),
                ))]);
            }
        }
    }

    for (logical_path, _, _, scope) in &entries {
        let logical_path = logical_path.as_ref().map_err(Clone::clone)?;
        imports.mark_loaded(logical_path, *scope);
    }

    let mut retained = Vec::with_capacity(entries.len());
    for (logical_path, source, package, scope) in entries {
        let logical_path = logical_path?;
        let text = std::str::from_utf8(source.bytes()).map_err(|_| {
            vec![Diagnostic::error(format!(
                "included generated source `{}` is not UTF-8 Omega source",
                logical_path.display(),
            ))]
        })?;
        let source_id = source::SourceId(source_storage.next_source_id());
        let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
            lex_sources(load_package_generated_source(
                logical_path,
                text,
                source_id.0,
                scope,
            ))
        })?;
        let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
            parse_sources(lexed, &mut source_storage.syntax_trees)
        })?;
        let contract_custody = source_storage.toolchain_contract_custody();
        let discovered = discover_imports_with_packages(
            &parsed,
            &source_storage.syntax_trees,
            package_inputs,
            Some(package),
            &mut source_storage.resolved_imports,
            &contract_custody,
        )?;
        imports.enqueue(discovered)?;
        source_storage.extend(parsed)?;
        retained.push((source_id, source.clone()));
    }
    Ok(retained)
}

fn generated_source_logical_path(
    package_root: &Path,
    source: &build_output::PackageGeneratedSource,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let mut logical_path = package_root.join(".omega/generated");
    for component in source.relative_path().split(|byte| *byte == b'/') {
        logical_path.push(std::str::from_utf8(component).map_err(|_| {
            vec![Diagnostic::error(
                "generated dependency source path is not canonical UTF-8",
            )]
        })?);
    }
    Ok(logical_path)
}

fn load_pending_imports(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    root_path: &Path,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    while imports.has_pending() {
        let frontier = imports.take_frontier();
        let frontier = match package_inputs {
            Some(package_inputs) => {
                validate_package_source_frontier(frontier, package_inputs, source_storage)?
            }
            None => frontier,
        };
        let first_source_id = source_storage.next_source_id();
        let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
            let sources = load_sources(frontier, first_source_id)?;
            lex_sources(sources)
        })?;
        let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
            parse_sources(lexed, &mut source_storage.syntax_trees)
        })?;
        let contract_custody = source_storage.toolchain_contract_custody();
        let discovered_imports = match package_inputs {
            Some(package_inputs) => discover_imports_with_packages(
                &parsed,
                &source_storage.syntax_trees,
                package_inputs,
                None,
                &mut source_storage.resolved_imports,
                &contract_custody,
            )?,
            None => discover_imports(
                &parsed,
                &source_storage.syntax_trees,
                root_path,
                &mut source_storage.resolved_imports,
            )?,
        };

        imports.enqueue(discovered_imports)?;
        source_storage.extend(parsed)?;
    }

    Ok(())
}

fn assemble_syntax(
    sources: SourceStorage,
    build_source_id: Option<source::SourceId>,
    application: Option<build_declarations::ApplicationDeclaration>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
) -> Result<AssembledSyntax, Vec<Diagnostic>> {
    // Each loaded instance already carries the scope it was checked under:
    // the build entry, the root-local helpers its build-scope instance
    // transitively imported, and every source of a build-only package
    // (physical and generated alike, stamped at retention).
    let build_scope_sources = sources
        .sources
        .files()
        .filter(|file| file.dependency_scope == source::DependencyScope::Build)
        .map(|file| file.source_id)
        .collect();
    Ok(AssembledSyntax {
        syntax_trees: sources.syntax_trees,
        sources: Arc::new(sources.sources),
        build_source_id,
        application,
        source_scoped_top_level_bindings,
        generated_source_custody,
        build_scope_sources,
    })
}
