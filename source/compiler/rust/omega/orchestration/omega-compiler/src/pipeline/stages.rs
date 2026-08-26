use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::frontend::{
    discover_imports, discover_imports_with_packages, extend_source_storage, lex_sources,
    load_injected_source, load_sources, parse_sources, read_bundled_std_source,
};
use crate::pipeline::project::{project_roots, validate_selected_target};
use crate::pipeline::source::{ImportQueue, SourceStorage};
use crate::pipeline::stage::{
    ABSTRACT_OPERATIONS_TO_TARGET_OPERATIONS, ASSIGNED_TARGET_OPERATIONS_TO_MACHINE_INSTRUCTIONS,
    BACKEND_PLAN_TO_NATIVE_IMAGE_PAYLOAD, CHECKED_TREES_TO_STATE_GRAPH,
    CONTROL_FLOW_TO_ABSTRACT_OPERATIONS, SOURCE_FILES_TO_TOKENS, STATE_GRAPH_TO_CONTROL_FLOW,
    SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
    TARGET_OPERATIONS_TO_ASSIGNED_TARGET_OPERATIONS, TOKENS_TO_SYNTAX_TREES,
    TYPED_TREES_TO_CHECKED_TREES,
};
use crate::pipeline::timing::CompileTimings;
use omega_control_flow::ControlFlowPlan;
use omega_emission_planning::{EmissionPlanningInput, build_emission_plan};
use omega_object_file::SectionKind;
use omega_state_graph::StateGraph;
use omega_target::NativeTarget;
use psi_checked_trees::CheckedTrees as CheckedProgram;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_syntax_trees::SyntaxTrees;
use psi_typed_trees::TypedTrees;
use std::collections::HashMap;
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
}

pub(super) fn append_retained_generated_sources(
    assembled: &mut AssembledSyntax,
    package_root: &Path,
    package_identity: Option<psi_core::PackageKeyIdentity>,
    generated_sources: &[crate::pipeline::build_staged_output::BuildStagedSource],
) -> Result<
    Vec<(
        psi_source::SourceId,
        crate::pipeline::build_staged_output::BuildStagedSource,
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
    Ok(retained)
}

pub(super) struct CheckedProgramSurface {
    pub(super) program: Arc<CheckedProgram>,
    pub(super) selected_provider_plans: Arc<omega_effects::SelectedProviderPlanFacts>,
    pub(super) component_progress: Option<Arc<omega_effects::ComponentProgressManifest>>,
    pub(super) task_activations: Arc<omega_task_plans::TaskActivationPlanSet>,
    pub(super) callback_placements: Arc<[omega_backend_plan::BoundNominalCallbackPlacement]>,
}

pub(super) struct BackendPlanningSurface {
    pub(super) plan: omega_backend_plan::BackendPlan,
}

pub(super) struct EmittedProgram {
    pub(super) target: NativeTarget,
    /// PE optional-header Subsystem resolved from the selected target's
    /// `subsystem <word>` (console 3 by default). The PE writer stamps it into
    /// the image; the Mach-O output path translates gui (2) into an `.app`
    /// bundle beside the flat binary; other formats ignore it.
    pub(super) subsystem: u16,
    pub(super) planned_text_bytes: usize,
    pub(super) callback_placement_identity_fingerprint: u64,
    pub(super) object: omega_object_file::ObjectPlan,
    pub(super) relocations: omega_object_file::RelocationPlan,
    pub(super) encoded_machine_code: omega_machine_bytes::EncodedMachineCode,
    pub(super) encoded_machine_semantics: omega_machine_bytes::EncodedMachineSemanticSummary,
    pub(super) text_bytes: Vec<u8>,
    pub(super) data_bytes: Vec<u8>,
}

pub(super) fn source_files_to_syntax_trees(
    root_path: &Path,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
    // The native-image path substitutes target-specific providers. The interpreter
    // keeps abstract boundary traits for its headless stubs.
    source_files_to_syntax_trees_for_engine(root_path, target_name, true, package_inputs, timings)
}

pub(super) fn source_files_to_syntax_trees_for_engine(
    root_path: &Path,
    target_name: Option<&str>,
    native: bool,
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
        imports.seed(
            crate::pipeline::frontend::bundled_omega_root().join("language/std/filesystem.omg"),
        );
        load_pending_imports(
            &mut source_storage,
            &mut imports,
            root_path,
            target_name,
            package_inputs,
            timings,
        )?;
    }

    if native {
        substitute_native_gui_provider(
            &mut source_storage,
            root_path,
            target_name,
            &mut imports,
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
    )?;

    Ok((source_file_count, syntax))
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
}
pub data Optimizations {
    control_flow_cleanup: u8 in Trapping;
    sparse_conditional_constant_propagation: u8 in Trapping;
    copy_propagation: u8 in Trapping;
    global_value_numbering: u8 in Trapping;
    dead_pure_scalar_elimination: u8 in Trapping;
    proof_check_elision: u8 in Trapping;
    selected_incoming_u12_exact_add_immediate: u8 in Trapping;
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
}
pub data Optimizations {
    control_flow_cleanup: u8 in Trapping;
    sparse_conditional_constant_propagation: u8 in Trapping;
    copy_propagation: u8 in Trapping;
    global_value_numbering: u8 in Trapping;
    dead_pure_scalar_elimination: u8 in Trapping;
    proof_check_elision: u8 in Trapping;
    selected_incoming_u12_exact_add_immediate: u8 in Trapping;
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
}
pub machine BuildSource::resolve<'path>(&self, relative: &'path [u8] in Path) -> &'path [u8] in Path {
    relative
}
pub machine BuildOutput::resolve<'path>(&self, relative: &'path [u8] in Path) -> &'path [u8] in Path {
    relative
}
pub machine BuildOutput::include_source(&mut self, generated: &[u8] in Path) {
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

/// The darwin boundary-provider substitution (tasks #57/#60). The samples call the
/// UNCHANGED Win32-shaped `boundary trait Gui` / `boundary trait Input` through
/// `gui: Gui` / `input: Input` fields. On darwin there is no host lowering for these;
/// instead the bundled `omega::language::std::macos_gui` module provides `MacosGui` /
/// `MacosInput` data types that implement every op by composing objc / Core Graphics
/// boundary calls. This (1) injects that provider module (which the sample never
/// `use`s) and (2) rewrites each matching FIELD's type to its provider, so
/// `self.gui.op(..)` / `self.input.op(..)` become ordinary provider value-calls — the
/// shape proven to run natively (window_demo, fire 25). NATIVE-only: the interpreter's
/// `compile_to_checked` keeps the abstract boundary traits (its own headless stubs,
/// item #9). Registry-driven so Clock and future providers follow the same pattern.
const DARWIN_BOUNDARY_PROVIDERS: &[(&str, &str)] = &[("Gui", "MacosGui"), ("Input", "MacosInput")];

fn substitute_native_gui_provider(
    source_storage: &mut SourceStorage,
    root_path: &Path,
    target_name: Option<&str>,
    imports: &mut ImportQueue,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    // Gate strictly on darwin/Mach-O: on Windows `Gui`/`Input` have real Win32 host
    // lowerings (do NOT substitute); on linux they have none (a clean diagnostic).
    let is_darwin = NativeTarget::from_omega_target_name(target_name)
        .map(|target| target.object_format == omega_target::ObjectFormat::MachO)
        .unwrap_or(false);
    if !is_darwin {
        return Ok(());
    }

    // Which registered `boundary trait`s does the program declare, and is the provider
    // module already present? (`MacosGui` names the bundled module; it carries both
    // MacosGui + MacosInput.)
    let mut has_any_boundary = false;
    let mut has_provider_module = false;
    for (_, file) in source_storage.files.iter() {
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                psi_syntax_trees::item::Item::Trait(trait_definition)
                    if trait_definition.is_boundary
                        && DARWIN_BOUNDARY_PROVIDERS
                            .iter()
                            .any(|(boundary, _)| *boundary == trait_definition.name.as_str()) =>
                {
                    has_any_boundary = true;
                }
                psi_syntax_trees::item::Item::Data(data) if data.name.as_str() == "MacosGui" => {
                    has_provider_module = true;
                }
                _ => {}
            }
        }
    }
    if !has_any_boundary {
        return Ok(());
    }

    // Inject the bundled provider module, then load its ordinary dependency
    // closure. Injection happens after the root import queue is exhausted, so
    // simply parsing its `use` declarations is not enough: newly discovered
    // imports must re-enter the same queue and recursive loader as authored
    // sources. This became observable when MacosGui adopted the public numeric
    // conversion machines.
    if !has_provider_module {
        let provider_source = read_bundled_std_source("macos_gui")?;
        let first_source_id = source_storage.next_source_id();
        let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
            let sources =
                load_injected_source("<macos-gui-provider>", &provider_source, first_source_id);
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
        load_pending_imports(
            source_storage,
            imports,
            root_path,
            target_name,
            package_inputs,
            timings,
        )?;
    }

    // Rewrite each `<field>: <Boundary>` FIELD -> its provider data type. Collect first
    // (immutable borrows), then mutate, exactly like the plan-laid value-type desugar.
    let mut rewrites: Vec<(psi_syntax_trees::types::TypeReferenceHandle, &'static str)> =
        Vec::new();
    for (_, file) in source_storage.files.iter() {
        for root_item in &file.root_items {
            let psi_syntax_trees::item::Item::Data(definition) =
                source_storage.syntax_trees.root_item(*root_item)
            else {
                continue;
            };
            let members = definition.members;
            for member in source_storage
                .syntax_trees
                .tables
                .items
                .data_members(members)
            {
                let psi_syntax_trees::item::DataMember::Field(field) = member else {
                    continue;
                };
                if let psi_syntax_trees::types::TypeReferenceNode::Named(name) = source_storage
                    .syntax_trees
                    .tables
                    .type_references
                    .type_reference(field.type_reference)
                    && let Some((_, provider)) = DARWIN_BOUNDARY_PROVIDERS
                        .iter()
                        .find(|(boundary, _)| *boundary == name.as_str())
                {
                    rewrites.push((field.type_reference, provider));
                }
            }
        }
    }
    for (handle, provider) in rewrites {
        source_storage
            .syntax_trees
            .tables
            .type_references
            .replace_type_reference(
                handle,
                psi_syntax_trees::types::TypeReferenceNode::Named(
                    psi_syntax_trees::identifier::Identifier::generated(provider.to_string()),
                ),
            );
    }

    Ok(())
}

fn assemble_syntax(
    sources: SourceStorage,
    build_source_id: Option<psi_source::SourceId>,
    source_scoped_top_level_bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
) -> Result<AssembledSyntax, Vec<Diagnostic>> {
    let files = sources.files.storage_slice().to_vec();
    Ok(AssembledSyntax {
        syntax_trees: sources.syntax_trees,
        files,
        sources: Arc::new(sources.sources),
        build_source_id,
        source_scoped_top_level_bindings,
    })
}

pub(super) fn syntax_trees_to_symbol_resolved_trees(
    syntax: AssembledSyntax,
    timings: &mut CompileTimings,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    timings.record(SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES, || {
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources_and_top_level_bindings(
            &syntax.syntax_trees,
            syntax.sources,
            syntax.source_scoped_top_level_bindings,
        )
    })
}

pub(super) fn symbol_resolved_trees_to_typed_trees(
    resolved: SymbolResolvedTrees,
    timings: &mut CompileTimings,
) -> Result<TypedTrees, Vec<Diagnostic>> {
    timings.record(SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, || {
        psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_owned(resolved)
            .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn typed_trees_to_checked_trees(
    typed: TypedTrees,
    timings: &mut CompileTimings,
) -> Result<CheckedProgramSurface, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_CHECKED_TREES, || {
        let program = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)?;
        crate::pipeline::provider_approval::check_boundary_provider_approval(&program)?;
        Ok(CheckedProgramSurface {
            program: Arc::new(program),
            selected_provider_plans: Arc::new(omega_effects::SelectedProviderPlanFacts::default()),
            component_progress: None,
            task_activations: Arc::new(omega_task_plans::TaskActivationPlanSet::default()),
            callback_placements: Arc::from([]),
        })
    })
}

pub(super) fn checked_trees_to_state_graph(
    checked: &CheckedProgramSurface,
    workers: omega_core::parallel::WorkerPoolHandle,
    timings: &mut CompileTimings,
) -> Result<StateGraph, Vec<Diagnostic>> {
    timings.record(CHECKED_TREES_TO_STATE_GRAPH, || {
        omega_checked_trees_to_state_graph::build_state_graph_with_workers(
            Arc::clone(&checked.program),
            workers,
        )
        .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn state_graph_to_control_flow(
    state_graph: StateGraph,
    timings: &mut CompileTimings,
) -> Result<ControlFlowPlan, Vec<Diagnostic>> {
    timings.record(STATE_GRAPH_TO_CONTROL_FLOW, || {
        omega_state_graph_to_control_flow::build_control_flow_plan_owned(state_graph)
            .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn control_flow_to_backend_plan(
    checked: CheckedProgramSurface,
    entry_machine_name: Option<&str>,
    entry_boundary_plan: Option<omega_calling_conventions::BoundaryEntryPlan>,
    target_name: Option<&str>,
    freestanding: bool,
    external_binding_rows: &[omega_calling_conventions::ExternalBindingRow],
    control_flow: ControlFlowPlan,
    workers: omega_core::parallel::WorkerPoolHandle,
    timings: &mut CompileTimings,
) -> Result<BackendPlanningSurface, Vec<Diagnostic>> {
    let target_profile = omega_target::TargetProfile::from_omega_target_name(target_name)
        .map_err(|diagnostic| vec![diagnostic])?;

    let plan = omega_backend_pipeline::build_backend_plan_from_control_flow_with_workers(
        checked.program,
        checked.selected_provider_plans,
        entry_machine_name,
        entry_boundary_plan,
        checked.callback_placements,
        target_profile,
        freestanding,
        external_binding_rows,
        Arc::new(control_flow),
        workers,
    )
    .map_err(|diagnostic| vec![diagnostic])?;

    record_backend_phase_as_stage(
        timings,
        &plan,
        "abstract operations",
        CONTROL_FLOW_TO_ABSTRACT_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "target operations",
        ABSTRACT_OPERATIONS_TO_TARGET_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "assigned target operations",
        TARGET_OPERATIONS_TO_ASSIGNED_TARGET_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "machine instructions",
        ASSIGNED_TARGET_OPERATIONS_TO_MACHINE_INSTRUCTIONS,
    )?;

    Ok(BackendPlanningSurface { plan })
}

fn plan_emission(plan: &omega_backend_plan::BackendPlan) -> omega_artifacts::EmissionPlan {
    let mut emission = build_emission_plan(&EmissionPlanningInput {
        receiver_bases: &plan.receiver_bases,
        state_contexts: &plan.state_contexts,
        target: plan.target,
        entry_key: plan.entry_key,
        host_abi: &plan.host_abi,
        host_calls: &plan.host_calls,
        state_calls: &plan.state_calls,
        state_storage: &plan.state_storage,
        state_values: &plan.state_values,
        data: &plan.data,
        instructions: &plan.target_operations,
        control_flow: &plan.control_flow,
        runtime_flow: &plan.runtime_flow,
        runtime_bodies: &plan.runtime_bodies,
        runtime_branching_calls: &plan.runtime_branching_calls,
        runtime_dispatch_loop: &plan.runtime_dispatch_loop,
        runtime_storage: &plan.runtime_storage,
        runtime_text: &plan.runtime_text,
        state_guards: &plan.state_guards,
        layouts: &plan.layouts,
        machine_instructions: &plan.machine_instructions,
        encoded_machine: &plan.encoded_machine,
        object: &plan.object,
        relocations: &plan.relocations,
    });
    retain_callback_thunk_emission_blockers(
        &mut emission,
        &plan.callback_placements,
        &plan.callback_thunks,
        &plan.encoded_machine,
        &plan.object,
    );
    emission
}

/// Callback planning is authority, not evidence that native code exists. Keep
/// image emission fail-closed until every private identity owns one exact
/// encoded function and one matching private text symbol.
fn retain_callback_thunk_emission_blockers(
    emission: &mut omega_artifacts::EmissionPlan,
    callback_placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
    callback_thunks: &[omega_backend_plan::CallbackThunkPlan],
    encoded_machine: &omega_machine_bytes::EncodedMachinePlan,
    object: &omega_object_file::ObjectPlan,
) {
    let mut placement_thunk_counts = vec![0usize; callback_placements.len()];
    let mut private_identity_counts = HashMap::<&str, usize>::new();
    for thunk in callback_thunks {
        if let Some(count) = placement_thunk_counts.get_mut(thunk.placement_index) {
            *count += 1;
        }
        *private_identity_counts
            .entry(thunk.private_symbol.as_ref())
            .or_default() += 1;
    }
    for (placement_index, placement) in callback_placements.iter().enumerate() {
        let thunk_count = placement_thunk_counts[placement_index];
        if thunk_count != 1 {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "validated callback placement {placement_index} for `{}` resolves to {thunk_count} private thunk plans; exactly one is required",
                    placement.canonical_requirement_overload
                ),
            ));
        }
    }

    for thunk in callback_thunks {
        let Some(placement) = callback_placements.get(thunk.placement_index) else {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` cites missing placement row {}",
                    thunk.private_symbol, thunk.placement_index
                ),
            ));
            continue;
        };
        if let Err(error) = omega_backend_plan::validate_bound_nominal_callback_placement(placement)
        {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` retained an invalid target calling plan: {error}",
                    thunk.private_symbol
                ),
            ));
            continue;
        }
        let placement_identity = omega_backend_plan::callback_placement_binding_identity(placement);
        if thunk.placement_identity != placement_identity {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` placement identity drifted from placement row {}",
                    thunk.private_symbol, thunk.placement_index
                ),
            ));
            continue;
        }
        let selected_entry = omega_control_flow::StateKey {
            machine: placement.selected_machine,
            state: placement.selected_entry,
            segment_index: 0,
        };
        if thunk.entry_key != selected_entry {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` targets {:?}, not placement row {} selected machine/entry {:?}",
                    thunk.private_symbol,
                    thunk.entry_key,
                    thunk.placement_index,
                    selected_entry,
                ),
            ));
            continue;
        }
        let thunk_identity_count = private_identity_counts[thunk.private_symbol.as_ref()];
        if thunk_identity_count != 1 {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback identity `{}` occurs {thunk_identity_count} times; exactly one is required",
                    thunk.private_symbol
                ),
            ));
            continue;
        }
        let canonical_private_symbol =
            omega_backend_plan::canonical_callback_private_symbol(placement);
        if thunk.private_symbol != canonical_private_symbol {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` does not match placement row {} canonical identity `{canonical_private_symbol}`",
                    thunk.private_symbol, thunk.placement_index
                ),
            ));
            continue;
        }
        if !thunk.entry_key.is_valid() {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` has an invalid selected-entry key",
                    thunk.private_symbol
                ),
            ));
            continue;
        }
        let canonical_function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(
                thunk.entry_key,
                thunk.placement_index,
            );
        if canonical_function_identity != Some(thunk.function_identity) {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` has function identity {:?}, not the canonical identity for placement row {} and selected entry {:?}",
                    thunk.private_symbol,
                    thunk.function_identity,
                    thunk.placement_index,
                    thunk.entry_key
                ),
            ));
            continue;
        }

        let encoded_functions = encoded_machine
            .code
            .functions
            .iter()
            .filter(|(_, function)| function.symbol.as_ref() == thunk.private_symbol.as_ref())
            .map(|(_, function)| function)
            .collect::<Vec<_>>();
        if encoded_functions.len() != 1 {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` resolves to {} encoded functions; exactly one is required",
                    thunk.private_symbol,
                    encoded_functions.len()
                ),
            ));
            continue;
        }
        let encoded = encoded_functions[0];
        if encoded.identity != thunk.function_identity {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` encoded function targets {:?}, not its selected entry {:?}",
                    thunk.private_symbol, encoded.identity, thunk.entry_key
                ),
            ));
            continue;
        }

        let symbols = object
            .layout
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.name == thunk.private_symbol.as_ref())
            .map(|(_, symbol)| symbol)
            .collect::<Vec<_>>();
        if symbols.len() != 1 {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` resolves to {} object symbols; exactly one is required",
                    thunk.private_symbol,
                    symbols.len()
                ),
            ));
            continue;
        }
        let symbol = symbols[0];
        let Some((_, identity_symbol)) =
            omega_object_file::object_function_symbol(object, thunk.function_identity)
        else {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` has no exact object-function binding for identity {:?}",
                    thunk.private_symbol, thunk.function_identity
                ),
            ));
            continue;
        };
        if identity_symbol.name != thunk.private_symbol.as_ref() {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback identity {:?} binds object symbol `{}`, not `{}`",
                    thunk.function_identity, identity_symbol.name, thunk.private_symbol
                ),
            ));
            continue;
        }
        if symbol.kind != omega_object_file::SymbolKind::Function
            || symbol.section
                != omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Text)
            || symbol.offset != encoded.byte_offset
            || symbol.size != encoded.byte_count
        {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` object symbol does not match its encoded function interval",
                    thunk.private_symbol
                ),
            ));
        }
    }
}

pub(super) fn ensure_emission_ready(
    emission_plan: &omega_artifacts::EmissionPlan,
) -> Result<(), Vec<Diagnostic>> {
    if emission_plan.blockers.is_empty() {
        return Ok(());
    }

    Err(emission_plan
        .blockers
        .iter()
        .map(|(_, blocker)| Diagnostic::error(format!("{}: {}", blocker.stage, blocker.reason)))
        .collect())
}

pub(super) fn backend_plan_to_native_image_payload(
    backend: &BackendPlanningSurface,
    subsystem: u16,
    timings: &mut CompileTimings,
) -> Result<(omega_artifacts::EmissionPlan, EmittedProgram), Vec<Diagnostic>> {
    timings.record(BACKEND_PLAN_TO_NATIVE_IMAGE_PAYLOAD, || {
        let emission_plan = plan_emission(&backend.plan);
        ensure_emission_ready(&emission_plan)?;
        let plan = &backend.plan;
        let text_bytes = plan.encoded_machine.code.bytes.storage_slice().to_vec();
        let emitted = EmittedProgram {
            target: plan.target,
            subsystem,
            planned_text_bytes: object_text_size(&plan.object),
            callback_placement_identity_fingerprint:
                omega_backend_plan::callback_thunk_placement_identity_fingerprint(
                    &plan.callback_thunks,
                ),
            object: plan.object.clone(),
            relocations: plan.relocations.clone(),
            encoded_machine_code: plan.encoded_machine.code.clone(),
            encoded_machine_semantics: plan.encoded_machine.semantics.clone(),
            text_bytes,
            data_bytes: plan.data.bytes.storage_slice().to_vec(),
        };
        Ok((emission_plan, emitted))
    })
}

fn object_text_size(object: &omega_object_file::ObjectPlan) -> usize {
    object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Text)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn record_backend_phase_as_stage(
    timings: &mut CompileTimings,
    plan: &omega_backend_plan::BackendPlan,
    backend_phase: &str,
    stage: crate::pipeline::stage::StageMeta,
) -> Result<(), Vec<Diagnostic>> {
    let Some((_, phase_timing)) = plan
        .phase_timings
        .iter()
        .find(|(_, phase_timing)| phase_timing.phase == backend_phase)
    else {
        return Err(vec![Diagnostic::error(format!(
            "backend phase `{backend_phase}` was not recorded for {}",
            stage.label()
        ))]);
    };

    timings.add_completed(
        stage,
        phase_timing.microseconds,
        phase_timing.allocations.clone(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy};
    use omega_control_flow::StateKey;
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

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
                    "control_flow_cleanup",
                    "sparse_conditional_constant_propagation",
                    "copy_propagation",
                    "global_value_numbering",
                    "dead_pure_scalar_elimination",
                    "proof_check_elision",
                    "selected_incoming_u12_exact_add_immediate",
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
        }
    }

    fn empty_emission(target: NativeTarget) -> omega_artifacts::EmissionPlan {
        omega_artifacts::EmissionPlan {
            image_format: target.object_format,
            entry_symbol: String::new(),
            sections: 0,
            symbols: 0,
            host_bindings: 0,
            host_calls: 0,
            data_bytes: 0,
            selected_instructions: 0,
            instruction_operands: 0,
            machine_code_bytes: 0,
            encoded_machine_bytes: 0,
            relocations: 0,
            blockers: psi_arena::Arena::new(),
        }
    }

    fn state_key(state: u32) -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        }
    }

    fn thunk(
        entry_key: StateKey,
        placement: &omega_backend_plan::BoundNominalCallbackPlacement,
    ) -> omega_backend_plan::CallbackThunkPlan {
        let function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(entry_key, 0)
                .unwrap_or_default();
        let private_symbol = omega_backend_plan::canonical_callback_private_symbol(placement);
        let root_schedule = Arc::new(
            omega_backend_plan::plan_callback_root_schedule(
                0,
                placement,
                entry_key,
                function_identity,
                Arc::clone(&private_symbol),
            )
            .expect("valid callback fixture root schedule"),
        );
        omega_backend_plan::CallbackThunkPlan {
            placement_index: 0,
            placement_identity: omega_backend_plan::callback_placement_binding_identity(placement),
            entry_key,
            function_identity,
            private_symbol,
            root_schedule,
        }
    }

    fn placement(entry_key: StateKey) -> omega_backend_plan::BoundNominalCallbackPlacement {
        let validated = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature::default(),
        )
        .expect("empty callback entry plan");
        omega_backend_plan::BoundNominalCallbackPlacement {
            site: psi_checked_trees::NominalMachineUseSite::Expression(
                psi_checked_trees::expression::ExpressionHandle::from_arena_index(9),
            ),
            registration_operation: SymbolHandle::from_arena_index(3),
            static_machine_ordinal: 0,
            selected_machine: entry_key.machine,
            selected_entry: entry_key.state,
            satisfaction_trait: SymbolHandle::from_arena_index(4),
            satisfaction_requirement: SymbolHandle::from_arena_index(5),
            canonical_requirement_overload: "Handler::call".to_owned(),
            boundary_calling_plan_fingerprint: validated.contract_fingerprint(),
            boundary_entry_plan: validated.plan().clone(),
        }
    }

    fn encoded_machine(
        target: NativeTarget,
        keys: &[StateKey],
        symbol: &str,
    ) -> omega_machine_bytes::EncodedMachinePlan {
        let mut encoded =
            omega_machine_bytes::EncodedMachinePlan::with_capacity(target, keys.len(), 0, 0);
        for key in keys {
            encoded
                .code
                .functions
                .insert(omega_machine_bytes::EncodedMachineFunction {
                    symbol: Arc::from(symbol),
                    identity: omega_control_flow::MachineFunctionIdentity::callback_thunk(*key, 0)
                        .unwrap(),
                    byte_offset: 7,
                    byte_count: 11,
                    instructions: Default::default(),
                });
        }
        encoded
    }

    fn object_with_symbols(
        target: NativeTarget,
        thunk: &omega_backend_plan::CallbackThunkPlan,
        symbols: &[(usize, usize)],
    ) -> omega_object_file::ObjectPlan {
        let mut object = omega_object_file::ObjectPlan::with_capacity(target, 0, symbols.len());
        for (symbol_index, (offset, size)) in symbols.iter().enumerate() {
            let symbol = object.layout.symbols.insert(omega_object_file::SymbolPlan {
                name: thunk.private_symbol.to_string(),
                section: omega_object_file::SymbolSection::Section(
                    omega_object_file::SectionKind::Text,
                ),
                offset: *offset,
                size: *size,
                kind: omega_object_file::SymbolKind::Function,
                import_library: String::new(),
            });
            if symbol_index == 0 {
                object
                    .layout
                    .function_symbols
                    .insert(omega_object_file::FunctionSymbolPlan {
                        identity: thunk.function_identity,
                        symbol,
                    });
            }
        }
        object
    }

    fn callback_blockers(
        placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
        thunks: &[omega_backend_plan::CallbackThunkPlan],
        encoded: &omega_machine_bytes::EncodedMachinePlan,
        object: &omega_object_file::ObjectPlan,
    ) -> Vec<String> {
        let mut emission = empty_emission(encoded.target);
        retain_callback_thunk_emission_blockers(&mut emission, placements, thunks, encoded, object);
        emission
            .blockers
            .iter()
            .map(|(_, blocker)| blocker.reason.clone())
            .collect()
    }

    #[test]
    fn callback_thunk_emission_accepts_one_exact_function_and_private_symbol() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let blockers = callback_blockers(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[key], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );
        assert!(blockers.is_empty(), "{blockers:?}");
    }

    #[test]
    fn callback_thunk_emission_rejects_invalid_or_redirected_entry_keys() {
        let target = NativeTarget::host();
        let mut invalid_placement = placement(state_key(2));
        let mut invalid_thunk = thunk(state_key(2), &invalid_placement);
        invalid_placement.selected_machine = StateKey::default().machine;
        invalid_placement.selected_entry = StateKey::default().state;
        invalid_thunk.entry_key = StateKey::default();
        invalid_thunk.function_identity = Default::default();
        invalid_thunk.placement_identity =
            omega_backend_plan::callback_placement_binding_identity(&invalid_placement);
        invalid_thunk.private_symbol =
            omega_backend_plan::canonical_callback_private_symbol(&invalid_placement);
        let invalid = callback_blockers(
            &[invalid_placement],
            std::slice::from_ref(&invalid_thunk),
            &encoded_machine(target, &[state_key(2)], &invalid_thunk.private_symbol),
            &object_with_symbols(target, &invalid_thunk, &[(7, 11)]),
        );
        assert_eq!(invalid.len(), 1);
        assert!(invalid[0].contains("invalid selected-entry key"));

        let redirected_placement = placement(state_key(2));
        let redirected_thunk = thunk(state_key(2), &redirected_placement);
        let redirected = callback_blockers(
            &[redirected_placement],
            std::slice::from_ref(&redirected_thunk),
            &encoded_machine(target, &[state_key(3)], &redirected_thunk.private_symbol),
            &object_with_symbols(target, &redirected_thunk, &[(7, 11)]),
        );
        assert_eq!(redirected.len(), 1);
        assert!(redirected[0].contains("not its selected entry"));
    }

    #[test]
    fn callback_thunk_emission_rejects_missing_or_duplicate_encoded_symbols() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let object = object_with_symbols(target, &thunk, &[(7, 11)]);

        let missing = callback_blockers(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[], &thunk.private_symbol),
            &object,
        );
        assert_eq!(missing.len(), 1);
        assert!(missing[0].contains("resolves to 0 encoded functions"));

        for duplicate_keys in [[key, key], [key, state_key(3)]] {
            let duplicate = callback_blockers(
                std::slice::from_ref(&placement),
                std::slice::from_ref(&thunk),
                &encoded_machine(target, &duplicate_keys, &thunk.private_symbol),
                &object,
            );
            assert_eq!(duplicate.len(), 1);
            assert!(duplicate[0].contains("resolves to 2 encoded functions"));
        }
    }

    #[test]
    fn callback_thunk_emission_rejects_object_cardinality_or_interval_drift() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let encoded = encoded_machine(target, &[key], &thunk.private_symbol);

        for symbols in [Vec::new(), vec![(7, 11), (7, 11)]] {
            let blockers = callback_blockers(
                std::slice::from_ref(&placement),
                std::slice::from_ref(&thunk),
                &encoded,
                &object_with_symbols(target, &thunk, &symbols),
            );
            assert_eq!(blockers.len(), 1);
            assert!(blockers[0].contains("object symbols; exactly one is required"));
        }

        let drifted = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded,
            &object_with_symbols(target, &thunk, &[(7, 10)]),
        );
        assert_eq!(drifted.len(), 1);
        assert!(drifted[0].contains("does not match its encoded function interval"));
    }

    #[test]
    fn callback_thunk_emission_rejects_missing_duplicate_or_unknown_placement_joins() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let encoded = encoded_machine(target, &[key], &thunk.private_symbol);
        let object = object_with_symbols(target, &thunk, &[(7, 11)]);

        let missing = callback_blockers(std::slice::from_ref(&placement), &[], &encoded, &object);
        assert_eq!(missing.len(), 1);
        assert!(missing[0].contains("resolves to 0 private thunk plans"));

        let duplicate = callback_blockers(
            std::slice::from_ref(&placement),
            &[thunk.clone(), thunk.clone()],
            &encoded,
            &object,
        );
        assert!(
            duplicate
                .iter()
                .any(|blocker| blocker.contains("resolves to 2 private thunk plans"))
        );
        assert!(
            duplicate
                .iter()
                .any(|blocker| blocker.contains("occurs 2 times"))
        );

        let mut unknown = thunk;
        unknown.placement_index = 1;
        let unknown = callback_blockers(&[placement], &[unknown], &encoded, &object);
        assert!(
            unknown
                .iter()
                .any(|blocker| blocker.contains("cites missing placement row 1"))
        );
    }

    #[test]
    fn callback_thunk_emission_rejects_entry_drift_from_placement_row() {
        let target = NativeTarget::host();
        let selected = state_key(2);
        let drifted = state_key(3);
        let placement = placement(selected);
        let mut thunk = thunk(selected, &placement);
        thunk.entry_key = drifted;
        thunk.function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(drifted, 0).unwrap();

        let blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[drifted], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("not placement row 0 selected machine/entry"));
    }

    #[test]
    fn callback_thunk_emission_rejects_selected_entry_segment_drift() {
        let target = NativeTarget::host();
        let selected = state_key(2);
        let placement = placement(selected);
        let segmented = StateKey {
            segment_index: 1,
            ..selected
        };
        let mut thunk = thunk(selected, &placement);
        thunk.entry_key = segmented;
        thunk.function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(segmented, 0).unwrap();

        let blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[segmented], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("not placement row 0 selected machine/entry"));
    }

    #[test]
    fn callback_thunk_emission_rejects_private_symbol_drift_from_placement() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let mut thunk = thunk(key, &placement);
        thunk.private_symbol = Arc::from("__omega_callback_tampered");

        let blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[key], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("does not match placement row 0 canonical identity"));
    }

    #[test]
    fn callback_thunk_emission_rejects_retained_boundary_plan_drift() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let mut placement = placement(key);
        let thunk = thunk(key, &placement);
        placement.boundary_entry_plan.state.preemption =
            omega_calling_conventions::Preemption::ProviderDefined;

        let blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[key], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("drifted from its retained fingerprint"));
    }

    #[test]
    fn callback_thunk_emission_rejects_registration_or_satisfaction_identity_drift() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let encoded = encoded_machine(target, &[key], &thunk.private_symbol);
        let object = object_with_symbols(target, &thunk, &[(7, 11)]);

        let mut registration_drift = placement.clone();
        registration_drift.registration_operation = SymbolHandle::from_parts(3, 2);
        let blockers = callback_blockers(
            &[registration_drift],
            std::slice::from_ref(&thunk),
            &encoded,
            &object,
        );
        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("placement identity drifted"));

        let mut satisfaction_drift = placement;
        satisfaction_drift.satisfaction_trait = SymbolHandle::from_parts(4, 2);
        let blockers = callback_blockers(
            &[satisfaction_drift],
            std::slice::from_ref(&thunk),
            &encoded,
            &object,
        );
        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("placement identity drifted"));
    }

    #[test]
    fn callback_thunk_emission_rejects_function_role_or_placement_drift() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let mut source_role = thunk(key, &placement);
        source_role.function_identity = omega_control_flow::MachineFunctionIdentity::source(key);

        let source_role_blockers = callback_blockers(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&source_role),
            &encoded_machine(target, &[key], &source_role.private_symbol),
            &object_with_symbols(target, &source_role, &[(7, 11)]),
        );
        assert_eq!(source_role_blockers.len(), 1);
        assert!(source_role_blockers[0].contains("not the canonical identity"));

        let mut wrong_placement = thunk(key, &placement);
        wrong_placement.function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(key, 1).unwrap();
        let wrong_placement_blockers = callback_blockers(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&wrong_placement),
            &encoded_machine(target, &[key], &wrong_placement.private_symbol),
            &object_with_symbols(target, &wrong_placement, &[(7, 11)]),
        );
        assert_eq!(wrong_placement_blockers.len(), 1);
        assert!(wrong_placement_blockers[0].contains("not the canonical identity"));

        let exact = thunk(key, &placement);
        let mut redirected_object = object_with_symbols(target, &exact, &[(7, 11)]);
        let binding = redirected_object
            .layout
            .function_symbols
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .unwrap();
        redirected_object
            .layout
            .function_symbols
            .get_mut(binding)
            .identity = omega_control_flow::MachineFunctionIdentity::source(key);
        let object_role_blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&exact),
            &encoded_machine(target, &[key], &exact.private_symbol),
            &redirected_object,
        );
        assert_eq!(object_role_blockers.len(), 1);
        assert!(object_role_blockers[0].contains("no exact object-function binding"));
    }
}
