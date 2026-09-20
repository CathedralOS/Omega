use crate::frontend::{
    discover_imports, discover_imports_with_packages, extend_source_storage, lex_sources,
    load_package_generated_source, load_sources, parse_sources,
};
use crate::source::{ImportQueue, SourceStorage};
use artifacts::compile_timings::CompileTimings;
use artifacts::compile_timings::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syntax_trees::SyntaxTrees;

mod build_prelude;
mod build_vocabulary;
mod checkpoint;
mod entry_contract_seed;
pub use checkpoint::{ExactTargetSourceAssembly, ImmutableSourceParseCheckpoint};

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

/// Exact parsed extension produced by one admitted build activation.
///
/// Keeping these roots separate from the retained base is the source-side
/// input needed by the D18 continuation. Each unit remains separate so the
/// ordinary pre-resolution evaluator cannot synthesize a generic instance
/// across generated-source custody boundaries.
pub struct RetainedGeneratedSyntaxExtension {
    units: Vec<RetainedGeneratedSyntaxUnit>,
    sources: Arc<source::SourceMap>,
    generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    base_sources: Arc<source::SourceMap>,
}

struct RetainedGeneratedSyntaxUnit {
    source_id: source::SourceId,
    path: PathBuf,
    syntax_trees: SyntaxTrees,
    root_item_count: usize,
}

impl RetainedGeneratedSyntaxExtension {
    pub fn source_count(&self) -> usize {
        self.units.len()
    }

    pub fn generated_source_custody(
        &self,
    ) -> &[(source::SourceId, build_output::PackageGeneratedSource)] {
        &self.generated_source_custody
    }

    pub fn into_pre_resolution_inputs(
        self,
        base_sources: &Arc<source::SourceMap>,
    ) -> Result<(Vec<SyntaxTrees>, Arc<source::SourceMap>), Vec<Diagnostic>> {
        if !Arc::ptr_eq(base_sources, &self.base_sources) {
            return Err(vec![Diagnostic::error(
                "retained generated-source extension no longer matches its base source frontier",
            )]);
        }
        for unit in &self.units {
            if unit.syntax_trees.root_item_count() != unit.root_item_count
                || !self.sources.get(unit.source_id).is_some_and(|source| {
                    source.path == unit.path
                        && source.resolution_stratum
                            == source::SourceResolutionStratum::CurrentActivationExtension
                })
            {
                return Err(vec![Diagnostic::error(
                    "retained generated-source unit no longer matches its parsed roots and source custody",
                )]);
            }
        }
        Ok((
            self.units
                .into_iter()
                .map(|unit| unit.syntax_trees)
                .collect(),
            self.sources,
        ))
    }
}

pub fn retain_generated_syntax_extension(
    base_sources: &Arc<source::SourceMap>,
    package_root: &Path,
    package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    generated_sources: &[build_output::PackageGeneratedSource],
) -> Result<RetainedGeneratedSyntaxExtension, Vec<Diagnostic>> {
    let mut sources = (**base_sources).clone();
    let mut units = Vec::with_capacity(generated_sources.len());
    let mut retained = Vec::with_capacity(generated_sources.len());
    // Included paths have already been validated by staged-output custody.
    // Retain their owners while collision checks borrow generated paths.
    let logical_paths = generated_sources
        .iter()
        .map(|generated| {
            let mut logical_path = package_root.join(".omega/generated");
            for component in generated.relative_path().split(|byte| *byte == b'/') {
                logical_path.push(
                    std::str::from_utf8(component)
                        .expect("validated generated source paths retain UTF-8 components"),
                );
            }
            logical_path
        })
        .collect::<Vec<_>>();
    let collisions = match logical_paths.as_slice() {
        [] => Vec::new(),
        [logical_path] => vec![
            base_sources
                .files()
                .any(|existing| existing.path == *logical_path),
        ],
        _ => {
            // Index only the requested extension, not every retained base file.
            // Later duplicates are already collisions; base matches need mark
            // only the first authored occurrence of each generated path.
            let mut generated_positions = HashMap::with_capacity(logical_paths.len());
            let mut collisions = vec![false; logical_paths.len()];
            for (position, logical_path) in logical_paths.iter().enumerate() {
                match generated_positions.entry(logical_path.as_path()) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(position);
                    }
                    std::collections::hash_map::Entry::Occupied(_) => {
                        collisions[position] = true;
                    }
                }
            }
            for existing in base_sources.files() {
                if let Some(&position) = generated_positions.get(existing.path.as_path()) {
                    collisions[position] = true;
                }
            }
            collisions
        }
    };
    for ((generated, logical_path), collides) in
        generated_sources.iter().zip(logical_paths).zip(collisions)
    {
        let source = std::str::from_utf8(generated.bytes()).map_err(|_| {
            vec![Diagnostic::error(format!(
                "included generated source `{}` is not UTF-8 Omega source",
                String::from_utf8_lossy(generated.relative_path())
            ))]
        })?;
        if collides {
            return Err(vec![Diagnostic::error(format!(
                "generated source logical path `{}` collides with an existing source",
                logical_path.display()
            ))]);
        }

        let source_id = source::SourceId(sources.len());
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .map_err(|error| {
                let position = source::SourcePosition::of(source, error.span.start);
                vec![Diagnostic::error(format!(
                    "{}:{}:{}: {}",
                    logical_path.display(),
                    position.line,
                    position.column,
                    error.message
                ))]
            })?;
        let parsed = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .map_err(|error| {
                let position = source::SourcePosition::of(source, error.source_span.span.start);
                vec![Diagnostic::error(format!(
                    "{}:{}:{}: {}",
                    logical_path.display(),
                    position.line,
                    position.column,
                    error.message
                ))]
            })?;
        let root_item_count = parsed.root_item_count();
        // Generated extension sources are the producing activation's
        // product-context handoff; they keep the base compilation's product
        // instance semantics rather than growing a second checked instance.
        let added = sources.add_checked_instance(
            logical_path.clone(),
            source.to_owned(),
            package_root.to_path_buf(),
            package_identity,
            source::SourceOrigin::User,
            source::SourceResolutionStratum::CurrentActivationExtension,
            source::DependencyScope::Product,
        );
        debug_assert_eq!(added.source_id, source_id);
        units.push(RetainedGeneratedSyntaxUnit {
            source_id,
            path: logical_path,
            syntax_trees: parsed,
            root_item_count,
        });
        retained.push((source_id, generated.clone()));
    }
    Ok(RetainedGeneratedSyntaxExtension {
        units,
        sources: Arc::new(sources),
        generated_source_custody: retained,
        base_sources: base_sources.clone(),
    })
}

fn append_dependency_generated_sources_to_storage(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    target_name: Option<&str>,
    package_inputs: &PackageCompilationInputs,
    timings: &mut CompileTimings,
) -> Result<Vec<(source::SourceId, build_output::PackageGeneratedSource)>, Vec<Diagnostic>> {
    let selected_target = target_name
        .map(|target_name| target::TargetProfile::from_omega_target_name(Some(target_name)))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    package_inputs
        .validate_dependency_generated_source_target(selected_target)
        .map_err(|errors| {
            errors
                .into_iter()
                .map(|error| Diagnostic::error(error.to_string()))
                .collect::<Vec<_>>()
        })?;
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
        extend_source_storage(source_storage, parsed)?;
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

/// Require the exact selected free build root to declare its project role
/// through the same compiler-neutral grammar used by package orchestration.
/// An application declaration is retained whole for admission and
/// publication: its one authored name supplies the `.app` basename and
/// executable leaf, and its `artifact_only` modifier narrows the admitted
/// route (wiki/spec/build/macos_application.md).
fn validate_selected_build_role(
    source_storage: &SourceStorage,
    build_source_id: Option<source::SourceId>,
) -> Result<Option<build_declarations::ApplicationDeclaration>, Vec<Diagnostic>> {
    let Some(build_source_id) = build_source_id else {
        return Ok(None);
    };
    let source = source_storage.sources.get(build_source_id).ok_or_else(|| {
        vec![Diagnostic::error(
            "selected build source has no retained source text",
        )]
    })?;
    build_declarations::project_build_declaration_from_source(&source.source)
        .map(|declaration| match declaration {
            build_declarations::BuildDeclaration::Application(application) => Some(application),
            _ => None,
        })
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "{}: invalid project build declaration: {error}",
                source.path.display()
            ))]
        })
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
        extend_source_storage(source_storage, parsed)?;
    }

    Ok(())
}

fn validate_package_source_frontier(
    frontier: Vec<(PathBuf, source::DependencyScope)>,
    package_inputs: &PackageCompilationInputs,
    source_storage: &SourceStorage,
) -> Result<Vec<(PathBuf, source::DependencyScope)>, Vec<Diagnostic>> {
    let toolchain_root = crate::frontend::bundled_core_root();
    let mut validated = Vec::with_capacity(frontier.len());
    let mut diagnostics = Vec::new();

    for (source, scope) in frontier {
        let canonical = match source.canonicalize() {
            Ok(canonical) => canonical,
            Err(error) => {
                diagnostics.push(Diagnostic::error(format!(
                    "failed to canonicalize package source {} before loading: {error}",
                    source.display()
                )));
                continue;
            }
        };
        if canonical.starts_with(&toolchain_root)
            || source_storage.is_toolchain_contract_source(&canonical)
        {
            validated.push((canonical, scope));
            continue;
        }

        let Some(owner) = package_inputs.package_for_source(&canonical) else {
            diagnostics.push(Diagnostic::error(format!(
                "package source {} escapes every reconciled source root",
                canonical.display()
            )));
            continue;
        };
        if owner != package_inputs.root()
            && canonical.file_name().and_then(|name| name.to_str()) == Some("build.omg")
        {
            diagnostics.push(Diagnostic::error(format!(
                "dependency build file {} may not join the compiled program",
                canonical.display()
            )));
            continue;
        }
        validated.push((canonical, scope));
    }

    if diagnostics.is_empty() {
        Ok(validated)
    } else {
        Err(diagnostics)
    }
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

#[cfg(test)]
mod tests;
