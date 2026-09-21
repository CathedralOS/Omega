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
//! (`optimize_artifact_sections` → `lower_optimized_to_target_operations` →
//! `stage_optimized_verified_physical_pipeline` → fragment emission → frame
//! application → text placement → the relocation-free object container) so the
//! emitted artifact carries real retained replay evidence.

use std::sync::Arc;

/// One compiled attached-entry program: the retained container custody, the
/// emitted object artifact built from it, and the selected source signature
/// the checked trees prove for the entry machine.
pub(super) struct CompiledFragmentEntry {
    pub container: Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    pub artifact: image_emission::ObjectArtifact,
    pub signature: program_entry_plan::SelectedProgramEntrySourceSignature,
}

/// The source-to-checked ladder mirrors
/// `native-realization/src/tests/fixtures/checked_source.rs`: one source-free
/// resolution, typed lowering, then settled checking.
fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
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
        .find(|parameter| parameter.is_self)
        .map_or(
            program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            |parameter| {
                program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
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
) -> Arc<object_file::StagedOptimizedRelocationFreeObjectContainer> {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, machine_name)
        .expect("entry machine must reach Terminal");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("semantic bytes");
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("proof bytes");
    let selections = optimization_core::OptimizationSelections::new([]).expect("empty selections");
    let optimized = native_realization::optimize_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("canonical artifact independently verifies and optimizes");
    let post_terminal = optimized.selections().project_post_terminal();
    let optimized_target =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
        )
        .expect("verified plan lowers to target operations");
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        optimized_target,
        post_terminal.selections(),
    )
    .expect("entry machine reaches physical realization");
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("function fragments");
    let framed = machine_emission::stage_function_fragment_frame_application(fragments)
        .expect("frame application");
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed)
        .expect("placed text section");
    Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed)
            .expect("relocation-free object container"),
    )
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
    let container = fragment_container(&checked, machine_name, profile.native_target());
    let artifact = image_emission::build_function_fragment_object_artifact(Arc::clone(&container))
        .expect("fragment object artifact");
    image_emission::validate_function_fragment_object_artifact(&container, &artifact)
        .expect("the retained container replays the emitted object");
    CompiledFragmentEntry {
        container,
        artifact,
        signature,
    }
}
