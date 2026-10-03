//! Shared fragment-custody fixture: compile one source program through the
//! ordinary Psi front-end, verified optimization, the physical pipeline, and
//! the fragment-emission ladder into the
//! `StagedOptimizedRelocationFreeObjectContainer` custody that only
//! `build_function_fragment_object_artifact` admits onto an emitted object.
//!
//! Every other fixture in this tree constructs `MachineCodePlan` carriers by
//! hand; those objects carry no `fragment_replay` evidence, so the routes that
//! require current common-pipeline custody — the hosted receiver bridge, the
//! free hosted Unit entry adapters, and graph-storage structural rows — were
//! reachable only from downstream crates. This ladder keeps the same
//! production sequence the native-realization entry route drives
//! (`lower_artifact` → `optimize_abstract_operations` → `lower_optimized_to_target_operations` →
//! `stage_optimized_verified_physical_pipeline` → fragment emission → frame
//! application → text placement → the relocation-free object container) so the
//! emitted artifact carries real retained replay evidence.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use std::sync::Arc;

/// One compiled attached-entry program: the retained container custody, the
/// emitted object artifact built from it, and the selected source signature
/// the checked trees prove for the entry machine.
pub(super) struct CompiledFragmentEntry {
    pub container: Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    pub artifact: image_emission::ObjectArtifact,
    pub signature: program_entry_plan::SelectedProgramEntrySourceSignature,
    pub checked_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
}

/// The source-to-checked ladder: one source-free resolution, typed lowering,
/// then settled checking.
fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check")
}

/// Re-derive the source signature the entry route admits: the terminal
/// selection's own machine/state symbols, its normalized callable spelling,
/// and the receiver's normalized type identity when the entry carries
/// `&mut self`.
fn entry_signature(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
    profile: target::TargetProfile,
) -> program_entry_plan::SelectedProgramEntrySourceSignature {
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == machine_name)
        .expect("source-selected entry");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == selection.machine)
        .expect("checked source entry");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("entry state");
    let receiver = checked
        .state_parameters(state)
        .iter()
        .enumerate()
        .find(|(_, parameter)| {
            parameter.is_self
                || matches!(
                    checked
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    checked_trees::types::TypeReferenceNode::Reference { .. }
                )
        })
        .map_or(
            program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            |(source_position, parameter)| {
                program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                    source_position: u32::try_from(source_position)
                        .expect("source position fits u32"),
                    source_symbol: parameter.symbol,
                    normalized_type_identity: checked
                        .normalized_type_identity(parameter.type_reference)
                        .into_string(),
                }
            },
        );
    let provisioned = matches!(
        receiver,
        program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable { .. }
    );
    program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        profile.program_entry_slot(),
        selection.machine,
        selection.machine,
        selection.name.clone(),
        "entry".into(),
        if provisioned {
            format!("test::{machine_name}(&mut self) -> Unit")
        } else {
            format!("test::{machine_name}() -> Unit")
        },
        receiver,
        Vec::new(),
    )
    .expect("selected source signature")
}

/// Compile `machine_name` in `source` for `target` and stage the retained
/// fragment custody the emitted object replays against.
fn fragment_container(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
    target: target::NativeTarget,
    signature: &program_entry_plan::SelectedProgramEntrySourceSignature,
) -> (
    Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    terminal_psi::CheckedProgramEntryTerminalReceipt,
) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name(machine_name),
    )
    .expect("entry machine must reach Terminal");
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == machine_name)
        .expect("source-selected entry");
    let checked_entry = lowered_psi_to_terminal_psi::prepare_program_entry_receipt(
        checked,
        selection,
        signature.identity().bytes(),
        &lowered,
    )
    .expect("checked entry receipt");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("semantic bytes");
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("proof bytes");
    let selections = optimization_core::OptimizationSelections::new([]).expect("empty selections");
    let admitted = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("canonical artifact independently verifies");
    let budget = optimization_core::OptimizationWorkBudget::new(
        1_000_000, 100_000, 100_000, 100_000, 100_000,
    )
    .expect("fixture work budget");
    let optimized = abstract_operations_to_abstract_operations::optimize_abstract_operations(
        admitted
            .into_optimization_artifact()
            .into_optimization_input(),
        &selections,
        &selections.project_psi(),
        budget,
    )
    .expect("verified artifact optimizes");
    let optimized_target =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
        )
        .expect("verified plan lowers to target operations");
    let physical = compiler::native::stage_optimized_verified_physical_pipeline(optimized_target)
        .expect("entry machine reaches physical realization");
    let fragments = machine_emission::stage_optimized_function_fragment_emission(physical)
        .expect("function fragments");
    let framed = machine_emission::stage_function_fragment_frame_application(fragments)
        .expect("frame application");
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed)
        .expect("placed text section");
    let container = Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed)
            .expect("relocation-free object container"),
    );
    (container, checked_entry)
}

/// The canonical Terminal structural declarations of `machine_name`'s module,
/// for fixtures that must spell a receiver field's exact route and identity.
pub(super) fn terminal_structural_types(
    source: &str,
    machine_name: &str,
) -> Vec<terminal_psi::StructuralTypeDeclaration> {
    checked_trees_to_lowered_psi::lower_machine(
        &checked_source(source),
        TerminalMachineSelection::Name(machine_name),
    )
    .expect("entry machine must reach Terminal")
    .semantic_module
    .structural_types
}

/// Compile one attached-entry source program for `profile`, emit its object
/// artifact through the fragment route, and replay the retained container
/// against the emitted object once so callers start from verified custody.
pub(super) fn compile_attached_entry(
    source: &str,
    machine_name: &str,
    profile: target::TargetProfile,
) -> CompiledFragmentEntry {
    let checked = checked_source(source);
    let signature = entry_signature(&checked, machine_name, profile);
    let (container, checked_entry) =
        fragment_container(&checked, machine_name, profile.native_target(), &signature);
    let artifact = image_emission::build_function_fragment_object_artifact(Arc::clone(&container))
        .expect("fragment object artifact");
    image_emission::validate_function_fragment_object_artifact(&container, &artifact)
        .expect("the retained container replays the emitted object");
    CompiledFragmentEntry {
        container,
        artifact,
        signature,
        checked_entry,
    }
}
