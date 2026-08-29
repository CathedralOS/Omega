use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::frontend::{
    discover_imports, discover_imports_with_packages, extend_source_storage, lex_sources,
    load_package_generated_source, load_sources, parse_sources,
};
use crate::pipeline::project::{project_roots, validate_selected_target};
use crate::pipeline::source::{ImportQueue, SourceStorage};
use crate::pipeline::stage::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};
use crate::pipeline::timing::CompileTimings;
use psi_diagnostics::Diagnostic;
use psi_syntax_trees::SyntaxTrees;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct AssembledSyntax {
    pub(super) syntax_trees: SyntaxTrees,
    pub(super) files: Vec<crate::pipeline::source::SourceFile>,
    pub(super) sources: Arc<psi_source::SourceMap>,
    /// Exact companion `build.omg` selected during project discovery. Build
    /// authority is attached to this source, never reconstructed from a leaf
    /// filename after imports have expanded the source frontier.
    pub(super) build_source_id: Option<psi_source::SourceId>,
    pub(super) source_scoped_top_level_bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
    pub(super) generated_source_custody: Vec<(
        psi_source::SourceId,
        omega_build_output::PackageGeneratedSource,
    )>,
}

pub(super) fn append_retained_generated_sources(
    assembled: &mut AssembledSyntax,
    package_root: &Path,
    package_identity: Option<psi_core::PackageKeyIdentity>,
    generated_sources: &[omega_build_output::PackageGeneratedSource],
) -> Result<
    Vec<(
        psi_source::SourceId,
        omega_build_output::PackageGeneratedSource,
    )>,
    Vec<Diagnostic>,
> {
    let mut retained = Vec::with_capacity(generated_sources.len());
    for generated in generated_sources {
        let source = std::str::from_utf8(generated.bytes()).map_err(|_| {
            vec![Diagnostic::error(format!(
                "included generated source `{}` is not UTF-8 Omega source",
                String::from_utf8_lossy(generated.relative_path())
            ))]
        })?;
        let mut logical_path = package_root.join(".omega/generated");
        for component in generated.relative_path().split(|byte| *byte == b'/') {
            logical_path.push(
                std::str::from_utf8(component)
                    .expect("validated generated source paths retain UTF-8 components"),
            );
        }
        if assembled
            .sources
            .files()
            .any(|existing| existing.path == logical_path)
        {
            return Err(vec![Diagnostic::error(format!(
                "generated source logical path `{}` collides with an existing source",
                logical_path.display()
            ))]);
        }

        let source_id = psi_source::SourceId(assembled.sources.len());
        let tokens = crate::lexer::Lexer::new(source)
            .tokenize()
            .map_err(|error| {
                let position = psi_source::SourcePosition::of(source, error.span.start);
                vec![Diagnostic::error(format!(
                    "{}:{}:{}: {}",
                    logical_path.display(),
                    position.line,
                    position.column,
                    error.message
                ))]
            })?;
        let root_items = crate::parser::parse_syntax_trees_into_with_id(
            &mut assembled.syntax_trees,
            source_id,
            &tokens,
        )
        .map_err(|error| {
            let position = psi_source::SourcePosition::of(source, error.source_span.span.start);
            vec![Diagnostic::error(format!(
                "{}:{}:{}: {}",
                logical_path.display(),
                position.line,
                position.column,
                error.message
            ))]
        })?;
        let added = Arc::make_mut(&mut assembled.sources).add_with_metadata(
            logical_path.clone(),
            source.to_owned(),
            package_root.to_path_buf(),
            package_identity,
            psi_source::SourceOrigin::User,
        );
        debug_assert_eq!(added.source_id, source_id);
        assembled.files.push(crate::pipeline::source::SourceFile {
            source_id,
            path: logical_path,
            root_items,
        });
        retained.push((source_id, generated.clone()));
    }
    assembled
        .generated_source_custody
        .extend(retained.iter().cloned());
    Ok(retained)
}

pub(super) fn source_files_to_syntax_trees_for_engine(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
    let project_roots = project_roots(root_path);
    let selected_build_path = project_roots
        .build
        .as_deref()
        .map(|path| {
            path.canonicalize().map_err(|error| {
                vec![Diagnostic::error(format!(
                    "failed to establish exact build source {}: {error}",
                    path.display()
                ))]
            })
        })
        .transpose()?;
    let mut imports = ImportQueue::default();
    for root in project_roots.sources {
        imports.seed(root);
    }

    let toolchain_root = crate::pipeline::frontend::bundled_omega_root();
    let mut source_storage = match package_inputs {
        Some(package_inputs) => {
            package_inputs.validate_for_compilation(root_path, &toolchain_root)?;
            let root_package = package_inputs
                .package_root(package_inputs.root())
                .expect("validated package inputs retain their root")
                .to_path_buf();
            let mut storage = SourceStorage::for_package_compilation(
                root_package,
                package_inputs.root(),
                toolchain_root,
            );
            for (identity, source_root) in package_inputs.packages() {
                storage.register_reconciled_package_root(source_root.to_path_buf(), identity);
            }
            storage
        }
        None => {
            let root_package = root_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            SourceStorage::for_compilation(root_package, toolchain_root)
        }
    };
    let generated_source_custody = match package_inputs {
        Some(package_inputs) => append_dependency_generated_sources_to_storage(
            &mut source_storage,
            &mut imports,
            target_name,
            package_inputs,
            timings,
        )?,
        None => Vec::new(),
    };
    load_pending_imports(
        &mut source_storage,
        &mut imports,
        root_path,
        target_name,
        package_inputs,
        timings,
    )?;

    let build_source_id = selected_build_path
        .as_deref()
        .map(|selected| {
            source_storage
                .files
                .iter()
                .find(|(_, file)| {
                    file.path
                        .canonicalize()
                        .is_ok_and(|loaded| loaded == selected)
                })
                .map(|(_, file)| file.source_id)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "selected build source {} disappeared from the loaded frontier",
                        selected.display()
                    ))]
                })
        })
        .transpose()?;
    validate_selected_build_role(&source_storage, build_source_id)?;
    let (build_requires_filesystem_layout, source_scoped_top_level_bindings) =
        inject_build_prelude(&mut source_storage, build_source_id, timings)?;
    if build_requires_filesystem_layout {
        imports.seed(crate::pipeline::frontend::bundled_omega_root().join("std/filesystem.omg"));
        load_pending_imports(
            &mut source_storage,
            &mut imports,
            root_path,
            target_name,
            package_inputs,
            timings,
        )?;
    }

    validate_selected_target(&source_storage, target_name)?;
    let source_file_count = source_storage.file_count();
    let syntax = assemble_syntax(
        source_storage,
        build_source_id,
        source_scoped_top_level_bindings,
        generated_source_custody,
    )?;

    Ok((source_file_count, syntax))
}

fn append_dependency_generated_sources_to_storage(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    target_name: Option<&str>,
    package_inputs: &PackageCompilationInputs,
    timings: &mut CompileTimings,
) -> Result<
    Vec<(
        psi_source::SourceId,
        omega_build_output::PackageGeneratedSource,
    )>,
    Vec<Diagnostic>,
> {
    let selected_target = target_name
        .map(|target_name| omega_target::TargetProfile::from_omega_target_name(Some(target_name)))
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
    let mut entries = Vec::new();
    for bundle in package_inputs.dependency_generated_source_bundles() {
        let package_root = package_inputs
            .package_root(bundle.package())
            .expect("validated generated-source bundle retains its package root");
        for source in bundle.sources() {
            let logical_path = generated_source_logical_path(package_root, source)?;
            if logical_path.exists()
                || entries
                    .iter()
                    .any(|(existing, _): &(PathBuf, _)| existing == &logical_path)
            {
                return Err(vec![Diagnostic::error(format!(
                    "generated dependency source logical path `{}` collides with another source",
                    logical_path.display(),
                ))]);
            }
            entries.push((logical_path, source.clone()));
        }
    }

    for (logical_path, _) in &entries {
        imports.mark_loaded(logical_path.clone());
    }

    let mut retained = Vec::with_capacity(entries.len());
    for (logical_path, source) in entries {
        let text = std::str::from_utf8(source.bytes()).map_err(|_| {
            vec![Diagnostic::error(format!(
                "included generated source `{}` is not UTF-8 Omega source",
                logical_path.display(),
            ))]
        })?;
        let source_id = psi_source::SourceId(source_storage.next_source_id());
        let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
            lex_sources(load_package_generated_source(
                logical_path,
                text,
                source_id.0,
            ))
        })?;
        let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
            parse_sources(lexed, &mut source_storage.syntax_trees)
        })?;
        let discovered = discover_imports_with_packages(
            &parsed,
            &source_storage.syntax_trees,
            target_name,
            package_inputs,
        )?;
        imports.enqueue(discovered)?;
        extend_source_storage(source_storage, parsed)?;
        retained.push((source_id, source));
    }
    Ok(retained)
}

fn generated_source_logical_path(
    package_root: &Path,
    source: &omega_build_output::PackageGeneratedSource,
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
///
/// Scoped `Owner::build` roots remain in the explicit Q4 compatibility lane.
/// They are not accepted as declarations here, and package-aware readers still
/// reject them. The standalone compiler preserves their current behavior only
/// until that owner question is settled.
fn validate_selected_build_role(
    source_storage: &SourceStorage,
    build_source_id: Option<psi_source::SourceId>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(build_source_id) = build_source_id else {
        return Ok(());
    };
    let selected = source_storage
        .files
        .iter()
        .find_map(|(_, file)| (file.source_id == build_source_id).then_some(file))
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "selected build source has no retained syntax file",
            )]
        })?;
    let has_scoped_build = selected.root_items.iter().any(|handle| {
        matches!(
            source_storage.syntax_trees.items.item(*handle),
            psi_syntax_trees::item::Item::Machine(machine)
                if machine.attached_data.is_some()
                    && machine.name.as_str().rsplit("::").next() == Some("build")
        )
    });
    if has_scoped_build {
        return Ok(());
    }
    let source = source_storage.sources.get(build_source_id).ok_or_else(|| {
        vec![Diagnostic::error(
            "selected build source has no retained source text",
        )]
    })?;
    omega_build_declarations::project_build_declaration_from_source(&source.source)
        .map(|_| ())
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
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    while imports.has_pending() {
        let frontier = imports.take_frontier();
        let frontier = match package_inputs {
            Some(package_inputs) => validate_package_source_frontier(frontier, package_inputs)?,
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
        let discovered_imports = match package_inputs {
            Some(package_inputs) => discover_imports_with_packages(
                &parsed,
                &source_storage.syntax_trees,
                target_name,
                package_inputs,
            )?,
            None => discover_imports(
                &parsed,
                &source_storage.syntax_trees,
                root_path,
                target_name,
            )?,
        };

        imports.enqueue(discovered_imports)?;
        extend_source_storage(source_storage, parsed)?;
    }

    Ok(())
}

fn validate_package_source_frontier(
    frontier: Vec<PathBuf>,
    package_inputs: &PackageCompilationInputs,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let toolchain_root = crate::pipeline::frontend::bundled_omega_root();
    let mut validated = Vec::with_capacity(frontier.len());
    let mut diagnostics = Vec::new();

    for source in frontier {
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
        if canonical.starts_with(&toolchain_root) {
            validated.push(canonical);
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
        validated.push(canonical);
    }

    if diagnostics.is_empty() {
        Ok(validated)
    } else {
        Err(diagnostics)
    }
}

/// The TOOLCHAIN-PROVIDED build vocabulary (build_and_package_model.md): a
/// build.omg is just `machine build(builder: &mut Build) { ... }` or the scoped
/// `machine Owner::build(&mut self, builder: &mut Build) { ... }` -- the `Build` /
/// `Subsystem` types are CORE-DEFINED, never authored per file. When a
/// build.omg root declares either build-machine shape and no `Build` data of
/// its own, the build-machine fragment is injected as a virtual source (a
/// program-declared `Build` wins, which keeps migration and deliberate
/// overrides possible). Package identity uses the same injected `Build`
/// surface through `builder.package(name)`; there is no second declaration
/// type or compiler-only constant shape.
const BUILD_PRELUDE: &str = r#"
// Toolchain-provided build vocabulary (virtual source; build_and_package_model.md).
pub data Subsystem {
    case Console;
    case Gui;
    case EfiApplication;
    case Unspecified(value: u16);
}
pub data Optimization {
    case ControlFlowCleanup;
    case SparseConditionalConstantPropagation;
    case CopyPropagation;
    case GlobalValueNumbering;
    case DeadPureScalarElimination;
    case ProofCheckElision;
    case SelectedIncomingU12ExactAddImmediate;
    case X86RelaxConditionalBranchesToRel8V1;
    case SelectedIncomingU12ExactSubtractImmediate;
    case Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1;
    case SharedEntryFixedViewCopyAfterCompareBeforeBranchV1;
    case ActiveResidentImmediateU64MultiUseRematerializationV1;
    case Aarch64SelectShortestMovnSeededI64MaterializationV1;
    case X86SelectXorZeroI64MaterializationV1;
}
pub data Optimizations {
    human_report: u8 in Trapping;
    control_flow_cleanup: u8 in Trapping;
    sparse_conditional_constant_propagation: u8 in Trapping;
    copy_propagation: u8 in Trapping;
    global_value_numbering: u8 in Trapping;
    dead_pure_scalar_elimination: u8 in Trapping;
    proof_check_elision: u8 in Trapping;
    selected_incoming_u12_exact_add_immediate: u8 in Trapping;
    x86_relax_conditional_branches_to_rel8_v1: u8 in Trapping;
    selected_incoming_u12_exact_subtract_immediate: u8 in Trapping;
    aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1: u8 in Trapping;
    shared_entry_fixed_view_copy_after_compare_before_branch_v1: u8 in Trapping;
    active_resident_immediate_u64_multi_use_rematerialization_v1: u8 in Trapping;
    aarch64_select_shortest_movn_seeded_i64_materialization_v1: u8 in Trapping;
    x86_select_xor_zero_i64_materialization_v1: u8 in Trapping;
}
pub data Build {
    subsystem: Subsystem;
    freestanding: bool;
    optimizations: Optimizations;
}
pub data Source {
    case Path(location: &[u8]);
    case Git(repository: &[u8], revision: &[u8]);
}
pub machine Build::depend(&mut self, source: Source) {
}
pub machine Build::depend_as(&mut self, alias: &[u8], source: Source) {
}
pub machine Build::package(&mut self, name: &[u8]) {
}
pub machine Build::application(&mut self, name: &[u8]) {
}
pub machine Build::member(&mut self, path: &[u8]) {
}
pub machine Optimizations::enable(&mut self, optimization: Optimization) {
    transition optimization {
        Optimization::ControlFlowCleanup -> control_flow_cleanup()
        Optimization::SparseConditionalConstantPropagation -> sparse_conditional_constant_propagation()
        Optimization::CopyPropagation -> copy_propagation()
        Optimization::GlobalValueNumbering -> global_value_numbering()
        Optimization::DeadPureScalarElimination -> dead_pure_scalar_elimination()
        Optimization::ProofCheckElision -> proof_check_elision()
        Optimization::SelectedIncomingU12ExactAddImmediate -> selected_incoming_u12_exact_add_immediate()
        Optimization::X86RelaxConditionalBranchesToRel8V1 -> x86_relax_conditional_branches_to_rel8_v1()
        Optimization::SelectedIncomingU12ExactSubtractImmediate -> selected_incoming_u12_exact_subtract_immediate()
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 -> aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1()
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 -> shared_entry_fixed_view_copy_after_compare_before_branch_v1()
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1 -> active_resident_immediate_u64_multi_use_rematerialization_v1()
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 -> aarch64_select_shortest_movn_seeded_i64_materialization_v1()
        Optimization::X86SelectXorZeroI64MaterializationV1 -> x86_select_xor_zero_i64_materialization_v1()
    }

    state control_flow_cleanup(&mut self) {
        self.control_flow_cleanup = self.control_flow_cleanup + 1;
    }

    state sparse_conditional_constant_propagation(&mut self) {
        self.sparse_conditional_constant_propagation = self.sparse_conditional_constant_propagation + 1;
    }

    state copy_propagation(&mut self) {
        self.copy_propagation = self.copy_propagation + 1;
    }

    state global_value_numbering(&mut self) {
        self.global_value_numbering = self.global_value_numbering + 1;
    }

    state dead_pure_scalar_elimination(&mut self) {
        self.dead_pure_scalar_elimination = self.dead_pure_scalar_elimination + 1;
    }

    state proof_check_elision(&mut self) {
        self.proof_check_elision = self.proof_check_elision + 1;
    }

    state selected_incoming_u12_exact_add_immediate(&mut self) {
        self.selected_incoming_u12_exact_add_immediate = self.selected_incoming_u12_exact_add_immediate + 1;
    }

    state x86_relax_conditional_branches_to_rel8_v1(&mut self) {
        self.x86_relax_conditional_branches_to_rel8_v1 = self.x86_relax_conditional_branches_to_rel8_v1 + 1;
    }

    state selected_incoming_u12_exact_subtract_immediate(&mut self) {
        self.selected_incoming_u12_exact_subtract_immediate = self.selected_incoming_u12_exact_subtract_immediate + 1;
    }

    state aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1(&mut self) {
        self.aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1 = self.aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1 + 1;
    }

    state shared_entry_fixed_view_copy_after_compare_before_branch_v1(&mut self) {
        self.shared_entry_fixed_view_copy_after_compare_before_branch_v1 = self.shared_entry_fixed_view_copy_after_compare_before_branch_v1 + 1;
    }

    state active_resident_immediate_u64_multi_use_rematerialization_v1(&mut self) {
        self.active_resident_immediate_u64_multi_use_rematerialization_v1 = self.active_resident_immediate_u64_multi_use_rematerialization_v1 + 1;
    }

    state aarch64_select_shortest_movn_seeded_i64_materialization_v1(&mut self) {
        self.aarch64_select_shortest_movn_seeded_i64_materialization_v1 = self.aarch64_select_shortest_movn_seeded_i64_materialization_v1 + 1;
    }

    state x86_select_xor_zero_i64_materialization_v1(&mut self) {
        self.x86_select_xor_zero_i64_materialization_v1 = self.x86_select_xor_zero_i64_materialization_v1 + 1;
    }
}
pub machine Optimizations::emit_report(&mut self) {
    self.human_report = self.human_report + 1;
}
"#;

const FILESYSTEM_BUILD_PRELUDE: &str = r#"
// Toolchain-provided build vocabulary (virtual source; build_and_package_model.md).
pub data Subsystem {
    case Console;
    case Gui;
    case EfiApplication;
    case Unspecified(value: u16);
}
pub data Optimization {
    case ControlFlowCleanup;
    case SparseConditionalConstantPropagation;
    case CopyPropagation;
    case GlobalValueNumbering;
    case DeadPureScalarElimination;
    case ProofCheckElision;
    case SelectedIncomingU12ExactAddImmediate;
    case X86RelaxConditionalBranchesToRel8V1;
    case SelectedIncomingU12ExactSubtractImmediate;
    case Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1;
    case SharedEntryFixedViewCopyAfterCompareBeforeBranchV1;
    case ActiveResidentImmediateU64MultiUseRematerializationV1;
    case Aarch64SelectShortestMovnSeededI64MaterializationV1;
    case X86SelectXorZeroI64MaterializationV1;
}
pub data Optimizations {
    human_report: u8 in Trapping;
    control_flow_cleanup: u8 in Trapping;
    sparse_conditional_constant_propagation: u8 in Trapping;
    copy_propagation: u8 in Trapping;
    global_value_numbering: u8 in Trapping;
    dead_pure_scalar_elimination: u8 in Trapping;
    proof_check_elision: u8 in Trapping;
    selected_incoming_u12_exact_add_immediate: u8 in Trapping;
    x86_relax_conditional_branches_to_rel8_v1: u8 in Trapping;
    selected_incoming_u12_exact_subtract_immediate: u8 in Trapping;
    aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1: u8 in Trapping;
    shared_entry_fixed_view_copy_after_compare_before_branch_v1: u8 in Trapping;
    active_resident_immediate_u64_multi_use_rematerialization_v1: u8 in Trapping;
    aarch64_select_shortest_movn_seeded_i64_materialization_v1: u8 in Trapping;
    x86_select_xor_zero_i64_materialization_v1: u8 in Trapping;
}
pub data BuildSource {
}
pub data BuildOutput {
}
pub data Build {
    subsystem: Subsystem;
    freestanding: bool;
    optimizations: Optimizations;
    source: BuildSource;
    output: BuildOutput;
    filesystem: FilesystemHost;
}
pub data Source {
    case Path(location: &[u8]);
    case Git(repository: &[u8], revision: &[u8]);
}
pub machine Build::depend(&mut self, source: Source) {
}
pub machine Build::depend_as(&mut self, alias: &[u8], source: Source) {
}
pub machine Build::package(&mut self, name: &[u8]) {
}
pub machine Build::application(&mut self, name: &[u8]) {
}
pub machine Build::member(&mut self, path: &[u8]) {
}
pub machine Optimizations::enable(&mut self, optimization: Optimization) {
    transition optimization {
        Optimization::ControlFlowCleanup -> control_flow_cleanup()
        Optimization::SparseConditionalConstantPropagation -> sparse_conditional_constant_propagation()
        Optimization::CopyPropagation -> copy_propagation()
        Optimization::GlobalValueNumbering -> global_value_numbering()
        Optimization::DeadPureScalarElimination -> dead_pure_scalar_elimination()
        Optimization::ProofCheckElision -> proof_check_elision()
        Optimization::SelectedIncomingU12ExactAddImmediate -> selected_incoming_u12_exact_add_immediate()
        Optimization::X86RelaxConditionalBranchesToRel8V1 -> x86_relax_conditional_branches_to_rel8_v1()
        Optimization::SelectedIncomingU12ExactSubtractImmediate -> selected_incoming_u12_exact_subtract_immediate()
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 -> aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1()
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 -> shared_entry_fixed_view_copy_after_compare_before_branch_v1()
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1 -> active_resident_immediate_u64_multi_use_rematerialization_v1()
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 -> aarch64_select_shortest_movn_seeded_i64_materialization_v1()
        Optimization::X86SelectXorZeroI64MaterializationV1 -> x86_select_xor_zero_i64_materialization_v1()
    }

    state control_flow_cleanup(&mut self) {
        self.control_flow_cleanup = self.control_flow_cleanup + 1;
    }

    state sparse_conditional_constant_propagation(&mut self) {
        self.sparse_conditional_constant_propagation = self.sparse_conditional_constant_propagation + 1;
    }

    state copy_propagation(&mut self) {
        self.copy_propagation = self.copy_propagation + 1;
    }

    state global_value_numbering(&mut self) {
        self.global_value_numbering = self.global_value_numbering + 1;
    }

    state dead_pure_scalar_elimination(&mut self) {
        self.dead_pure_scalar_elimination = self.dead_pure_scalar_elimination + 1;
    }

    state proof_check_elision(&mut self) {
        self.proof_check_elision = self.proof_check_elision + 1;
    }

    state selected_incoming_u12_exact_add_immediate(&mut self) {
        self.selected_incoming_u12_exact_add_immediate = self.selected_incoming_u12_exact_add_immediate + 1;
    }

    state x86_relax_conditional_branches_to_rel8_v1(&mut self) {
        self.x86_relax_conditional_branches_to_rel8_v1 = self.x86_relax_conditional_branches_to_rel8_v1 + 1;
    }

    state selected_incoming_u12_exact_subtract_immediate(&mut self) {
        self.selected_incoming_u12_exact_subtract_immediate = self.selected_incoming_u12_exact_subtract_immediate + 1;
    }

    state aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1(&mut self) {
        self.aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1 = self.aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1 + 1;
    }

    state shared_entry_fixed_view_copy_after_compare_before_branch_v1(&mut self) {
        self.shared_entry_fixed_view_copy_after_compare_before_branch_v1 = self.shared_entry_fixed_view_copy_after_compare_before_branch_v1 + 1;
    }

    state active_resident_immediate_u64_multi_use_rematerialization_v1(&mut self) {
        self.active_resident_immediate_u64_multi_use_rematerialization_v1 = self.active_resident_immediate_u64_multi_use_rematerialization_v1 + 1;
    }

    state aarch64_select_shortest_movn_seeded_i64_materialization_v1(&mut self) {
        self.aarch64_select_shortest_movn_seeded_i64_materialization_v1 = self.aarch64_select_shortest_movn_seeded_i64_materialization_v1 + 1;
    }

    state x86_select_xor_zero_i64_materialization_v1(&mut self) {
        self.x86_select_xor_zero_i64_materialization_v1 = self.x86_select_xor_zero_i64_materialization_v1 + 1;
    }
}
pub machine BuildSource::resolve<'path>(&self, relative: &'path [u8] in Path) -> &'path [u8] in Path {
    relative
}
pub machine BuildOutput::resolve<'path>(&self, relative: &'path [u8] in Path) -> &'path [u8] in Path {
    relative
}
pub machine BuildOutput::include_source(&mut self, generated: &[u8] in Path) {
}
pub machine Optimizations::emit_report(&mut self) {
    self.human_report = self.human_report + 1;
}
"#;

fn inject_build_prelude(
    source_storage: &mut SourceStorage,
    build_source_id: Option<psi_source::SourceId>,
    timings: &mut CompileTimings,
) -> Result<(bool, Vec<psi_symbols::SourceScopedTopLevelBinding>), Vec<Diagnostic>> {
    let mut has_build_machine = false;
    let mut build_source_declares_build_data = false;
    let mut program_declares_build_data = false;
    let mut build_reaches_filesystem = false;
    for (_, file) in source_storage.files.iter() {
        let is_build_file = Some(file.source_id) == build_source_id;
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                psi_syntax_trees::item::Item::Machine(machine)
                    if is_build_file
                        && (machine.name.as_str() == "build"
                            || machine.name.as_str().ends_with("::build")) =>
                {
                    has_build_machine = true;
                    build_reaches_filesystem |= source_storage
                        .syntax_trees
                        .items
                        .identifier_path_members(machine.service_reaches)
                        .iter()
                        .any(|service| service.as_str() == "FilesystemHost");
                }
                psi_syntax_trees::item::Item::Data(data) if data.name.as_str() == "Build" => {
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
        return Ok((build_reaches_filesystem, Vec::new()));
    }

    let build_prelude = if build_reaches_filesystem {
        FILESYSTEM_BUILD_PRELUDE
    } else {
        BUILD_PRELUDE
    };
    let prelude = build_prelude.to_owned();

    let first_source_id = source_storage.next_source_id();
    let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
        let sources = crate::pipeline::frontend::load_injected_source(
            "<build-prelude>",
            &prelude,
            first_source_id,
        );
        lex_sources(sources)
    })?;
    let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
        parse_sources(lexed, &mut source_storage.syntax_trees)
    })?;
    extend_source_storage(source_storage, parsed)?;
    let bindings = match (program_declares_build_data, build_source_id) {
        (true, Some(build_source_id)) => vec![psi_symbols::SourceScopedTopLevelBinding::new(
            build_source_id,
            psi_source::SourceId(first_source_id),
            "Build",
        )],
        _ => Vec::new(),
    };
    Ok((build_reaches_filesystem, bindings))
}

fn assemble_syntax(
    sources: SourceStorage,
    build_source_id: Option<psi_source::SourceId>,
    source_scoped_top_level_bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
    generated_source_custody: Vec<(
        psi_source::SourceId,
        omega_build_output::PackageGeneratedSource,
    )>,
) -> Result<AssembledSyntax, Vec<Diagnostic>> {
    let files = sources.files.storage_slice().to_vec();
    Ok(AssembledSyntax {
        syntax_trees: sources.syntax_trees,
        files,
        sources: Arc::new(sources.sources),
        build_source_id,
        source_scoped_top_level_bindings,
        generated_source_custody,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prelude_owns_canonical_dependency_vocabulary() {
        let tokens = psi_source_files_to_tokens::Lexer::new(BUILD_PRELUDE)
            .tokenize()
            .expect("toolchain build prelude must lex");
        let syntax_trees = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("toolchain build prelude must parse as ordinary Omega");

        let source = syntax_trees
            .root_items()
            .find_map(|item| match item {
                psi_syntax_trees::item::Item::Data(data) if data.name.as_str() == "Source" => {
                    Some(data)
                }
                _ => None,
            })
            .expect("build prelude must define Source");
        let source_cases = syntax_trees.items.data_members(source.members);
        let [
            psi_syntax_trees::item::DataMember::Variant(path),
            psi_syntax_trees::item::DataMember::Variant(git),
        ] = source_cases
        else {
            panic!("Source must contain exactly Path and Git cases");
        };
        assert_eq!(path.name.as_str(), "Path");
        assert_eq!(
            syntax_trees
                .items
                .data_payload_fields(path.payload)
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["location"]
        );
        assert_eq!(git.name.as_str(), "Git");
        assert_eq!(
            syntax_trees
                .items
                .data_payload_fields(git.payload)
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["repository", "revision"]
        );

        let mut dependency_methods = syntax_trees
            .root_items()
            .filter_map(|item| match item {
                psi_syntax_trees::item::Item::Machine(machine)
                    if machine
                        .attached_data
                        .as_ref()
                        .is_some_and(|owner| owner.as_str() == "Build") =>
                {
                    Some(machine)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        dependency_methods.sort_by_key(|machine| machine.name.as_str());
        assert_eq!(dependency_methods.len(), 5);
        assert_eq!(dependency_methods[0].name.as_str(), "Build::application");
        assert_eq!(dependency_methods[1].name.as_str(), "Build::depend");
        assert_eq!(dependency_methods[2].name.as_str(), "Build::depend_as");
        assert_eq!(dependency_methods[3].name.as_str(), "Build::member");
        assert_eq!(dependency_methods[4].name.as_str(), "Build::package");

        let parameter_names = |machine: &psi_syntax_trees::item::Machine| {
            let [entry] = syntax_trees.items.state_handles(machine.states) else {
                panic!("dependency method must have exactly one entry state");
            };
            syntax_trees
                .items
                .state_parameters(syntax_trees.items.state(*entry).parameters)
                .iter()
                .map(|handle| syntax_trees.items.state_parameter(*handle).name.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(parameter_names(dependency_methods[0]), ["self", "name"]);
        assert_eq!(parameter_names(dependency_methods[1]), ["self", "source"]);
        assert_eq!(
            parameter_names(dependency_methods[2]),
            ["self", "alias", "source"]
        );
        assert_eq!(parameter_names(dependency_methods[3]), ["self", "path"]);
        assert_eq!(parameter_names(dependency_methods[4]), ["self", "name"]);
        assert!(!syntax_trees.root_items().any(|item| matches!(
            item,
            psi_syntax_trees::item::Item::Machine(machine)
                if machine.attached_data.is_none() && machine.name.as_str() == "path"
        )));
    }

    #[test]
    fn both_build_preludes_own_the_exact_optimization_vocabulary() {
        for prelude in [BUILD_PRELUDE, FILESYSTEM_BUILD_PRELUDE] {
            let tokens = psi_source_files_to_tokens::Lexer::new(prelude)
                .tokenize()
                .expect("toolchain build prelude must lex");
            let syntax_trees = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
                .expect("toolchain build prelude must parse as ordinary Omega");
            let optimization = syntax_trees
                .root_items()
                .find_map(|item| match item {
                    psi_syntax_trees::item::Item::Data(data)
                        if data.name.as_str() == "Optimization" =>
                    {
                        Some(data)
                    }
                    _ => None,
                })
                .expect("build prelude must define Optimization");
            assert_eq!(
                syntax_trees
                    .items
                    .data_members(optimization.members)
                    .iter()
                    .filter_map(|member| match member {
                        psi_syntax_trees::item::DataMember::Variant(variant) => {
                            Some(variant.name.as_str())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                [
                    "ControlFlowCleanup",
                    "SparseConditionalConstantPropagation",
                    "CopyPropagation",
                    "GlobalValueNumbering",
                    "DeadPureScalarElimination",
                    "ProofCheckElision",
                    "SelectedIncomingU12ExactAddImmediate",
                    "X86RelaxConditionalBranchesToRel8V1",
                    "SelectedIncomingU12ExactSubtractImmediate",
                    "Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1",
                    "SharedEntryFixedViewCopyAfterCompareBeforeBranchV1",
                    "ActiveResidentImmediateU64MultiUseRematerializationV1",
                    "Aarch64SelectShortestMovnSeededI64MaterializationV1",
                    "X86SelectXorZeroI64MaterializationV1",
                ]
            );
            let optimizations = syntax_trees
                .root_items()
                .find_map(|item| match item {
                    psi_syntax_trees::item::Item::Data(data)
                        if data.name.as_str() == "Optimizations" =>
                    {
                        Some(data)
                    }
                    _ => None,
                })
                .expect("build prelude must define Optimizations");
            assert_eq!(
                syntax_trees
                    .items
                    .data_members(optimizations.members)
                    .iter()
                    .filter_map(|member| match member {
                        psi_syntax_trees::item::DataMember::Field(field) => {
                            Some(field.name.as_str())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                [
                    "human_report",
                    "control_flow_cleanup",
                    "sparse_conditional_constant_propagation",
                    "copy_propagation",
                    "global_value_numbering",
                    "dead_pure_scalar_elimination",
                    "proof_check_elision",
                    "selected_incoming_u12_exact_add_immediate",
                    "x86_relax_conditional_branches_to_rel8_v1",
                    "selected_incoming_u12_exact_subtract_immediate",
                    "aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1",
                    "shared_entry_fixed_view_copy_after_compare_before_branch_v1",
                    "active_resident_immediate_u64_multi_use_rematerialization_v1",
                    "aarch64_select_shortest_movn_seeded_i64_materialization_v1",
                    "x86_select_xor_zero_i64_materialization_v1",
                ]
            );
            let build = syntax_trees
                .root_items()
                .find_map(|item| match item {
                    psi_syntax_trees::item::Item::Data(data) if data.name.as_str() == "Build" => {
                        Some(data)
                    }
                    _ => None,
                })
                .expect("build prelude must define Build");
            assert!(
                syntax_trees
                    .items
                    .data_members(build.members)
                    .iter()
                    .any(|member| matches!(
                        member,
                        psi_syntax_trees::item::DataMember::Field(field)
                            if field.name.as_str() == "optimizations"
                    ))
            );
            assert!(syntax_trees.root_items().any(|item| matches!(
                item,
                psi_syntax_trees::item::Item::Machine(machine)
                    if machine.name.as_str() == "Optimizations::enable"
            )));
            assert!(syntax_trees.root_items().any(|item| matches!(
                item,
                psi_syntax_trees::item::Item::Machine(machine)
                    if machine.name.as_str() == "Optimizations::emit_report"
            )));
        }
    }
}
