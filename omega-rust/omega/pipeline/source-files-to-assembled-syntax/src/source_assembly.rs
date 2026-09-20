use crate::frontend::{
    build_only_packages, discover_imports, discover_imports_with_packages, extend_source_storage,
    lex_sources, load_package_generated_source, load_sources, parse_sources,
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

mod build_vocabulary;
mod checkpoint;
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
    // A generated bundle joins the checked instance of the context that
    // produced it: a build-only package's handoff is host-context source,
    // while a dual-purpose or product package's bundle embodies product-
    // target decisions and stays product scope — it is never checked a
    // second time under the other scope.
    let build_only = build_only_packages(package_inputs);
    let mut entries = Vec::new();
    for bundle in package_inputs.dependency_generated_source_bundles() {
        let package_root = package_inputs
            .package_root(bundle.package())
            .expect("validated generated-source bundle retains its package root");
        for source in bundle.sources() {
            // Keep path errors in authored order with physical collisions.
            entries.push((
                generated_source_logical_path(package_root, source),
                source,
                bundle.package(),
            ));
        }
    }
    {
        let mut loaded_paths = HashSet::with_capacity(entries.len());
        for (logical_path, _, _) in &entries {
            let logical_path = logical_path.as_ref().map_err(Clone::clone)?;
            if logical_path.exists() || !loaded_paths.insert(logical_path.as_path()) {
                return Err(vec![Diagnostic::error(format!(
                    "generated dependency source logical path `{}` collides with another source",
                    logical_path.display(),
                ))]);
            }
        }
    }

    for (logical_path, _, _) in &entries {
        let logical_path = logical_path.as_ref().map_err(Clone::clone)?;
        // The single retained instance answers imports from either scope, so
        // no scope may re-dispatch the virtual path for a physical load.
        imports.mark_loaded(logical_path, source::DependencyScope::Product);
        imports.mark_loaded(logical_path, source::DependencyScope::Build);
    }

    let mut retained = Vec::with_capacity(entries.len());
    for (logical_path, source, package) in entries {
        let logical_path = logical_path?;
        let scope = if build_only.contains(&package) {
            source::DependencyScope::Build
        } else {
            source::DependencyScope::Product
        };
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

/// One seeded hosted physical entry contract: the contract file, the root its
/// authored imports close inside, and whether that root is a closed toolchain
/// subtree. The bundled fallback owns an entire closed subtree so its
/// transitive `use` closure keeps toolchain custody. A reconciled supplier's
/// contract and ordinary imports retain that package's custody instead.
pub(crate) struct HostedEntryContractSeed {
    pub(crate) source: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) closed_subtree: bool,
}

/// The selected profile's physical entry contract must join compilation even
/// when no authored `use` names it. A reconciled supplier remains an ordinary
/// package subject to exact consumer acceptance; only the bundled fallback is
/// closed toolchain-owned source. The
/// seed defers until after authored imports resolve so an explicitly imported
/// contract copy keeps its own package custody and accepted-binding
/// requirement. Freestanding (`ProgramStorageApplication`) profiles keep
/// authored-import semantics and are never seeded.
fn hosted_entry_contract_seed(
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    source_storage: &SourceStorage,
) -> Option<HostedEntryContractSeed> {
    // Targetless checking owns no exact profile: `from_omega_target_name`
    // would resolve `None` to the host and silently seed platform content.
    let target_name = target_name?;
    let profile = target::TargetProfile::from_omega_target_name(Some(target_name)).ok()?;
    let slot = profile.program_entry_slot();
    if slot.schema != target::ProgramEntrySchema::HostedApplication {
        return None;
    }
    let contract_package = slot.physical_contract_package?;
    let relative_source = contract_package.package_relative_source();
    let already_loaded = source_storage
        .files
        .iter()
        .any(|(_, file)| file.path.ends_with(relative_source));
    if already_loaded {
        return None;
    }
    match package_inputs {
        // Standalone custody is the byte-exact bundled toolchain contract.
        None => {
            let contract_root = crate::frontend::bundled_omega_root().join("std");
            Some(HostedEntryContractSeed {
                source: contract_root.join(relative_source),
                root: contract_root,
                closed_subtree: true,
            })
        }
        // Package-aware compilation loads the contract from inside the
        // reconciled dependency closure, preserving its package identity and
        // source frontier. Its location grants no toolchain provenance or
        // consumer acceptance. A consumer's accepted
        // binding names the supplying package when more than one qualifies.
        // When no package in the closure carries the contract and no accepted
        // binding claims a supplier, the bundled toolchain copy is the same
        // closed target definition the slot vocabulary already names.
        Some(inputs) => {
            let accepted_role =
                build_evaluation::program_entry_semantic_binding_role(contract_package);
            let accepted_package = inputs
                .accepted_semantic_binding(accepted_role)
                .map(|binding| binding.package());
            let mut suppliers = inputs
                .packages()
                .filter(|(_, root)| root.join(relative_source).is_file())
                .collect::<Vec<_>>();
            suppliers.sort_by_key(|(identity, _)| *identity);
            let supplier = match suppliers.as_slice() {
                [single] => Some(*single),
                _ => suppliers
                    .iter()
                    .find(|(identity, _)| Some(*identity) == accepted_package)
                    .copied(),
            };
            supplier
                .map(|(_, root)| HostedEntryContractSeed {
                    source: root.join(relative_source),
                    root: root.to_path_buf(),
                    closed_subtree: false,
                })
                .or_else(|| {
                    (suppliers.is_empty() && accepted_package.is_none()).then(|| {
                        let contract_root = crate::frontend::bundled_omega_root().join("std");
                        HostedEntryContractSeed {
                            source: contract_root.join(relative_source),
                            root: contract_root,
                            closed_subtree: true,
                        }
                    })
                })
        }
    }
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

/// Toolchain-provided build vocabulary (wiki/spec/build/declarations.md): a
/// build.omg has exactly one free `machine build(builder: &mut Build) { ... }`
/// entry. The `Build` / `Subsystem` types are CORE-DEFINED, never authored per
/// file. When a build.omg root declares that build-machine shape and no `Build` data of
/// its own, the build-machine fragment is injected as a virtual source (a
/// program-declared `Build` wins, which keeps migration and deliberate
/// overrides possible). Package identity uses the same injected `Build`
/// surface through `builder.package(name)`; there is no second declaration
/// type or compiler-only constant shape.
const BUILD_PRELUDE: &str = r#"
// Toolchain-provided build vocabulary.
pub data Subsystem {
    case Console;
    case Gui;
    case EfiApplication;
    case Unspecified(value: u16);
}
// Compiler-owned composition choice for one exact provider selection. The
// omitted `select_provider` argument means Fused; Independent is never
// inferred from provider source.
pub data CompositionMode [copy] {
    case Fused;
    case Independent;
}
// Compiler-owned crash-cause vocabulary for build behavior exclusions
// (wiki/spec/build/behavior_exclusions.md). `builder.exclude_crash(case)`
// selects a product-admission requirement over the exact selected executable
// composition; it never masks effects on ordinary callable contracts.
pub data CrashCause [copy] {
    case Trap;
    case Abort;
}
// compiler-owned TargetProfile declaration
// compiler-owned X86DeploymentFeatures declaration
// compiler-owned optimization declarations
// Compiler-owned package-build path carrier. The evaluator replaces values
// produced by `resolve` with activation-local rooted authority; an authored
// `BuildPath {}` has the right static shape but no usable runtime root.
pub data BuildPath [copy] {
}
pub data BuildSource {
}
pub data BuildOutput {
}
pub data BuildLog {
}
// Compiler-owned product-selection facet. `builder.product.entry`,
// `builder.product.provider`, and `builder.product.schema` are the only routes
// to compiler-issued product descriptions: the evaluator answers each query
// by lexical lookup under the call occurrence's package and returns an opaque
// marker; the declared bodies are never executed.
pub data BuildProduct {
}
// A restricted, non-callable product-entry description. Authored code can
// retain, copy, and hand it to helpers, but only a `roots.bind` delegated
// operand can consume it, and only the compiler-issued marker value carries
// the exact selected identity.
pub data ProductEntryRef [copy] {
}
// A restricted, non-callable product-provider description (the
// `ProductProviderRef` of wiki/spec/build/scoped_execution.md). Authored
// code can retain, copy, and hand it to helpers, and `provider.path()`
// inspects the declaration it describes, but only the compiler-issued
// marker value carries the exact selected identity.
pub data ProductProviderRef [copy] {
}
// A restricted, non-callable product-type-schema description (the
// `ProductTypeSchema` of wiki/spec/build/scoped_execution.md). Authored
// code can retain, copy, and hand it to helpers, and `schema.path()`
// inspects the declaration it describes, but only the compiler-issued
// marker value carries the exact selected identity.
pub data ProductTypeSchema [copy] {
}
// Compiler-owned required-output obligation marker. `builder.output.require`
// is the only route to one: the evaluator issues an opaque marker whose row
// lives in its private obligation table; an authored `RequiredOutput {}` has
// the same static shape but carries no obligation. Obligations are linear at
// the settlement boundary: `complete` or `fail` consumes them exactly once.
pub data RequiredOutput {
}
// Compiler-owned output-completion receipt. `builder.output.complete` issues
// it when the obligation's sealed file is accepted; evaluated code may
// retain or copy it, but only the compiler-issued marker names a completed
// output.
pub data OutputReceipt [copy] {
}
// Result of `builder.output.complete`. `Sealed` carries the compiler-issued
// completion receipt; `Retry` returns the obligation and file custody when
// the named output was not yet sealed, so an explicit retry stays possible.
pub data OutputCompletion {
    case Sealed(receipt: OutputReceipt);
    case Retry(obligation: RequiredOutput, file: BuildPath);
}
// Optional proof-carrying product requests (wiki/spec/proofs/publication.md).
// Both flags are independent and default to false; they request adjacent
// `.proof` sidecars, never a different pipeline or weaker checking.
pub data Pcc {
    psi: bool;
    native: bool;
}
pub data Build {
    // compiler-owned Build.target field
    // compiler-owned Build.x86_deployment_features field
    subsystem: Subsystem;
    freestanding: bool;
    // Authored application identifier (wiki/spec/build/macos_application.md):
    // supplies the GUI CodeDirectory signing identity and CFBundleIdentifier.
    // Empty means no authored identifier; console output falls back to the
    // validated executable leaf. It is validated separately from the
    // application name and is never inferred from the PE subsystem word.
    identifier: &[u8];
    optimizations: Optimizations;
    pcc: Pcc;
    source: BuildSource;
    output: BuildOutput;
    log: BuildLog;
    product: BuildProduct;
}
pub data PackageSelection {
    case Root;
    case Named(package: &[u8]);
}
pub data Source {
    case Path(location: &[u8]);
    case Git(repository: &[u8], revision: &[u8], selection: PackageSelection);
}
pub machine Build::depend(&mut self, source: Source) {
}
pub machine Build::depend_as(&mut self, alias: &[u8], source: Source) {
}
pub machine Build::build_depend(&mut self, source: Source) {
}
pub machine Build::build_depend_as(&mut self, alias: &[u8], source: Source) {
}
pub machine Build::package(&mut self, name: &[u8]) {
}
pub machine Build::application(&mut self, name: &[u8]) {
}
pub machine Build::member(&mut self, path: &[u8]) {
}
// Artifact-only application modifier: a declaration statement harvested
// statically; the declared body is the evaluator no-op.
pub machine Build::artifact_only(&mut self) {
}
// Behavior exclusion (wiki/spec/build/behavior_exclusions.md): a
// product-admission requirement that the selected executable composition
// contains no reachable site of the named crash cause. The selection is
// harvested statically with its authored span from the build machine's
// checked call scope; the declared body is the evaluator no-op, and a
// declaration that permits Trap cannot override it.
pub machine Build::exclude_crash(&mut self, cause: CrashCause) {
}
// Independent-component assumption acceptance
// (wiki/spec/build/component_publication.md): a verified component
// description binds its environment-mediating mechanisms — an immediate
// port-space write, a declared physical mechanism — by digest. The
// consuming build accepts one exact digest per call, authored as the same
// lowercase hex spelling verification diagnostics report. The declaration
// is harvested statically with its authored span; the declared body is the
// evaluator no-op.
pub machine Build::accept_component_assumption(&mut self, digest: &[u8]) {
}
pub machine BuildSource::resolve(&self, relative: &[u8]) -> BuildPath {
    BuildPath {}
}
pub machine BuildOutput::resolve(&self, relative: &[u8]) -> BuildPath {
    BuildPath {}
}
pub machine BuildSource::open(&self, path: BuildPath, flags: i32) -> i32 {
    0
}
pub machine BuildSource::read(&self, descriptor: i32, buffer: &mut [u8], count: u64) -> i64 {
    0
}
pub machine BuildSource::close(&self, descriptor: i32) -> i32 {
    0
}
pub machine BuildOutput::create(&mut self, path: BuildPath, mode: i32) -> i32 {
    0
}
pub machine BuildOutput::write(&mut self, descriptor: i32, bytes: &[u8]) -> i64 {
    0
}
pub machine BuildOutput::close(&mut self, descriptor: i32) -> i32 {
    0
}
pub machine BuildOutput::include_source(&mut self, generated: BuildPath) {
}
// Required-output obligation declaration: reserve the canonical output name
// and return its obligation marker. Duplicate or colliding names reject in
// the evaluator; this declared body never executes.
pub machine BuildOutput::require(&mut self, name: &[u8]) -> RequiredOutput {
    RequiredOutput {}
}
// Settle one obligation against its sealed staged file. The evaluator
// intercepts the call: a sealed name yields `OutputCompletion::Sealed` with
// the receipt, an unsealed file yields `Retry` carrying the obligation and
// file custody back, and every other custody violation is a hard error.
pub machine BuildOutput::complete(&mut self, obligation: RequiredOutput, file: BuildPath) -> OutputCompletion {
    OutputCompletion::Retry { obligation: obligation, file: file }
}
// Consume an obligation with an authored diagnostic. The failure is sticky:
// the activation can never publish a successful product set.
pub machine BuildOutput::fail(&mut self, obligation: RequiredOutput, diagnostic: &[u8]) {
}
// The obligation's declared canonical output name.
pub machine RequiredOutput::path(&self) -> &[u8] {
    ""
}
// The described product declaration's canonical path. The evaluator answers
// from its private description table; the declared body never executes.
pub machine ProductTypeSchema::path(&self) -> &[u8] {
    ""
}
// The described provider declaration's canonical path. The evaluator answers
// from its private description table; the declared body never executes.
pub machine ProductProviderRef::path(&self) -> &[u8] {
    ""
}
pub machine BuildLog::write_line(&mut self, text: &[u8]) {
}
// Fallible logical query: select the exact product machine `path` declares in
// this call occurrence's own package for root slot `slot`. The evaluator
// intercepts the call and returns the restricted description marker; this
// declared body is only a statically well-typed stand-in.
pub machine BuildProduct::entry(&self, path: &[u8], slot: &[u8]) -> ProductEntryRef {
    ProductEntryRef {}
}
// Fallible logical query: select the exact product data declaration `path`
// names in this call occurrence's own package. The evaluator intercepts the
// call and returns the restricted schema-description marker; this declared
// body is only a statically well-typed stand-in.
pub machine BuildProduct::schema(&self, path: &[u8]) -> ProductTypeSchema {
    ProductTypeSchema {}
}
// Fallible logical query: select the exact product provider declaration `path`
// names in this call occurrence's own package. A provider declaration is a
// nominal data type owning at least one `satisfies` machine; other data does
// not qualify. The evaluator intercepts the call and returns the restricted
// provider-description marker; this declared body is only a statically
// well-typed stand-in.
pub machine BuildProduct::provider(&self, path: &[u8]) -> ProductProviderRef {
    ProductProviderRef {}
}
// compiler-owned optimization enable machine
// compiler-owned optimization report machine
"#;

const BUILD_TARGET_FIELD_SLOT: &str = "    // compiler-owned Build.target field\n";
const BUILD_TARGET_FIELD: &str = "    target: TargetProfile;\n";
const BUILD_X86_DEPLOYMENT_FEATURES_FIELD_SLOT: &str =
    "    // compiler-owned Build.x86_deployment_features field\n";
const BUILD_X86_DEPLOYMENT_FEATURES_FIELD: &str =
    "    x86_deployment_features: X86DeploymentFeatures;\n";
const BUILD_TARGET_PROFILE_SLOT: &str = "// compiler-owned TargetProfile declaration\n";
const BUILD_TARGET_PROFILE: &str = r#"pub data TargetProfile {
    case LinuxArm64;
    case LinuxX86_64;
    case MacosArm64;
    case WindowsX86_64;
    case UefiX86_64;
    case CrossPlatformCli;
    case LocalUnchecked;
}
"#;
const BUILD_X86_DEPLOYMENT_FEATURES_SLOT: &str =
    "// compiler-owned X86DeploymentFeatures declaration\n";
const BUILD_X86_DEPLOYMENT_FEATURES: &str = r#"pub data X86DeploymentFeatures {
    case Baseline;
    case AvxFma3;
}
"#;

fn construct_build_prelude(base: &str, has_exact_target: bool) -> String {
    assert_eq!(
        base.matches(BUILD_TARGET_FIELD_SLOT).count(),
        1,
        "build prelude must contain exactly one compiler-owned target slot"
    );
    assert_eq!(
        base.matches(BUILD_TARGET_PROFILE_SLOT).count(),
        1,
        "build prelude must contain exactly one compiler-owned target profile slot"
    );
    assert_eq!(
        base.matches(BUILD_X86_DEPLOYMENT_FEATURES_FIELD_SLOT)
            .count(),
        1,
        "build prelude must contain exactly one compiler-owned x86 deployment-feature field slot"
    );
    assert_eq!(
        base.matches(BUILD_X86_DEPLOYMENT_FEATURES_SLOT).count(),
        1,
        "build prelude must contain exactly one compiler-owned x86 deployment-feature declaration slot"
    );
    let replacement = if has_exact_target {
        BUILD_TARGET_FIELD
    } else {
        ""
    };
    let with_target = base
        .replacen(BUILD_TARGET_PROFILE_SLOT, BUILD_TARGET_PROFILE, 1)
        .replacen(
            BUILD_X86_DEPLOYMENT_FEATURES_SLOT,
            BUILD_X86_DEPLOYMENT_FEATURES,
            1,
        )
        .replacen(BUILD_TARGET_FIELD_SLOT, replacement, 1)
        .replacen(
            BUILD_X86_DEPLOYMENT_FEATURES_FIELD_SLOT,
            if has_exact_target {
                BUILD_X86_DEPLOYMENT_FEATURES_FIELD
            } else {
                ""
            },
            1,
        );
    build_vocabulary::install(&with_target)
}

fn inject_build_prelude(
    source_storage: &mut SourceStorage,
    build_source_id: Option<source::SourceId>,
    has_exact_target: bool,
    timings: &mut CompileTimings,
) -> Result<Vec<symbols::SourceScopedTopLevelBinding>, Vec<Diagnostic>> {
    let mut has_build_machine = false;
    let mut build_source_declares_build_data = false;
    let mut program_declares_build_data = false;
    for (_, file) in source_storage.files.iter() {
        let is_build_file = Some(file.source_id) == build_source_id;
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                syntax_trees::item::Item::Machine(machine)
                    if is_build_file && machine.name.as_str() == "build" =>
                {
                    has_build_machine = true;
                }
                syntax_trees::item::Item::Data(data) if data.name.as_str() == "Build" => {
                    if is_build_file {
                        build_source_declares_build_data = true;
                    } else {
                        program_declares_build_data = true;
                    }
                }
                _ => {}
            }
        }
    }
    let inject_build_vocabulary = has_build_machine && !build_source_declares_build_data;
    if !inject_build_vocabulary {
        return Ok(Vec::new());
    }

    // Targetless checking is not an artifact activation and therefore exposes
    // no synthetic target. Exact-target requests receive the canonical field;
    // retaining the former Build shape here keeps targetless semantic checks
    // honest while product requests migrate to immutable activation.
    let prelude = construct_build_prelude(BUILD_PRELUDE, has_exact_target);

    let first_source_id = source_storage.next_source_id();
    let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
        let sources =
            crate::frontend::load_injected_source("<build-prelude>", &prelude, first_source_id);
        lex_sources(sources)
    })?;
    let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
        parse_sources(lexed, &mut source_storage.syntax_trees)
    })?;
    extend_source_storage(source_storage, parsed)?;
    let bindings = match (program_declares_build_data, build_source_id) {
        (true, Some(build_source_id)) => vec![symbols::SourceScopedTopLevelBinding::new(
            build_source_id,
            source::SourceId(first_source_id),
            "Build",
        )],
        _ => Vec::new(),
    };
    Ok(bindings)
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
