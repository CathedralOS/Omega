//! Shared-borrow match joins execute natively against the original storage.
//!
//! `choose` selects `&a.left` or `&b.right` (or `&a` / `&b`) into one
//! `SharedBorrow` block parameter and forwards it to a borrowed-parameter
//! callee. The join carries the referent's address, never a copy: each arm's
//! home stays live and unmoved until `choose` returns. Both arms of both
//! customers publish on all four native targets and execute on a matching
//! host.

#[path = "common/front_end.rs"]
mod front_end;
#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
#[path = "common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

/// `borrowed_results::PRIMITIVE_CALL_SOURCE`: a projected leaf join.
const PRIMITIVE_CALL_SOURCE: &str = "data Payload { left: u64; right: u64; }
    machine read(value: &u64) -> u64 { value }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &u64 = match other { true -> &a.left, false -> &b.right };
        read(view)
    }";

/// `borrowed_results::RECORD_CALL_SOURCE`: a whole-local record join.
const RECORD_CALL_SOURCE: &str = "data Payload { left: u64; right: u64; }
    machine read(value: &Payload) -> u64 { value.left ^ value.right }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &Payload = match other { true -> &a, false -> &b };
        read(view)
    }";

fn produce(source: &str) -> CanonicalTerminalArtifact {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    // The customer is a real join: one shared-borrow block parameter bound
    // from two edges, never an owned transfer.
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.structural_parameters)
            .any(|parameter| parameter.access == terminal_psi::StructuralAccess::SharedBorrow)
    );
    artifact
}

fn publish(
    artifact: &CanonicalTerminalArtifact,
    target: NativeTarget,
) -> (image_emission::ExecutableImage, usize) {
    publish_with(artifact, target, &OptimizationSelections::new([]).unwrap())
}

fn publish_with(
    artifact: &CanonicalTerminalArtifact,
    target: NativeTarget,
    selections: &OptimizationSelections,
) -> (image_emission::ExecutableImage, usize) {
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(selections),
    )
    .unwrap_or_else(|error| panic!("admit shared join for {target:?}: {error:#?}"));
    let post_terminal = optimized.selections().project_post_terminal();
    let target_operations =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
        )
        .unwrap_or_else(|error| panic!("lower shared join for {target:?}: {error:#?}"));
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        target_operations,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| panic!("select shared join for {target:?}: {error:#?}"));
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap_or_else(|error| panic!("emit shared join fragments for {target:?}: {error:#?}"));
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&text).unwrap();
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone())
        .unwrap_or_else(|error| panic!("publish shared join for {target:?}: {error:#?}"));
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let entry_offset = object.entry_function().text_offset;
    let image = image_emission::emit_direct_executable_image(&object, 3)
        .unwrap_or_else(|error| panic!("image shared join for {target:?}: {error:#?}"));
    image_emission::validate_direct_executable_image(&object, &image).unwrap();
    (image, entry_offset)
}

fn assert_four_targets(source: &str) {
    let artifact = produce(source);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, _) = publish(&artifact, target);
        assert!(!image.output().final_text_bytes.is_empty());
    }
}

fn assert_host_execution(source: &str, when_true: u64, when_false: u64) {
    assert_host_execution_with(
        source,
        &OptimizationSelections::new([]).unwrap(),
        when_true,
        when_false,
    );
}

fn assert_host_execution_with(
    source: &str,
    selections: &OptimizationSelections,
    when_true: u64,
    when_false: u64,
) {
    let artifact = produce(source);
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, entry_offset) = publish_with(&artifact, NativeTarget::host(), selections);
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            &format!(
                "#include <stdbool.h>\n#include <stdint.h>\n#include <unistd.h>\n\
                 extern uint64_t omega_entry(bool other);\n\
                 int main(void) {{\n    alarm(10);\n\
                 if (omega_entry(true) != {when_true}u) return 1;\n\
                 if (omega_entry(false) != {when_false}u) return 2;\n\
                 if (omega_entry(true) != {when_true}u) return 3;\n    return 0;\n}}\n"
            ),
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = (artifact, selections, when_true, when_false);
        eprintln!(
            "SKIP: shared-join C runtime requires Linux x86-64/AArch64 or macOS AArch64; Windows has publication coverage only"
        );
    }
}

#[test]
fn primitive_leaf_join_publishes_on_four_targets() {
    assert_four_targets(PRIMITIVE_CALL_SOURCE);
}

#[test]
fn primitive_leaf_join_executes_both_arms() {
    assert_host_execution(PRIMITIVE_CALL_SOURCE, 1, 4);
}

#[test]
fn record_join_publishes_on_four_targets() {
    assert_four_targets(RECORD_CALL_SOURCE);
}

#[test]
fn record_join_executes_both_arms() {
    assert_host_execution(RECORD_CALL_SOURCE, 1 ^ 2, 3 ^ 4);
}

/// Control-flow cleanup threads the join's empty arms into the conditional,
/// and the selected-lowering rewrites see the address transports and the
/// carrier load, yet both arms still observe their own roots. Each other Psi
/// rewrite also executes both arms when run alone; combined Psi rewrites leave
/// threaded conditional edges that legalization's single-provenance branch
/// replay rejects for a scalar-only `match` too, independently of the join.
#[test]
fn both_joins_execute_under_psi_and_selected_rewrites() {
    use optimization_core::{Optimization, OptimizationExecutionPhase};
    for selections in [
        OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap(),
        OptimizationSelections::new(Optimization::ALL.into_iter().filter(|optimization| {
            optimization.execution_phase() == OptimizationExecutionPhase::SelectedLowering
        }))
        .unwrap(),
    ] {
        assert_host_execution_with(PRIMITIVE_CALL_SOURCE, &selections, 1, 4);
        assert_host_execution_with(RECORD_CALL_SOURCE, &selections, 1 ^ 2, 3 ^ 4);
    }
}

/// The target plan, legalized plan and staged selection for `choose`.
fn staged(
    source: &str,
    target: NativeTarget,
) -> target_operations_to_selected_instructions::StagedOptimizedSelectedInstructions {
    let artifact = produce(source);
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let compiled = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized,
        abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    target_operations_to_selected_instructions::stage_optimized_instruction_selection(
        compiled,
        environment,
    )
    .unwrap()
}

#[test]
fn legalization_rejects_forged_address_join_arguments_and_edges() {
    use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstructionKind};
    use target_operations::TargetStructuralArgumentSource;
    for source in [PRIMITIVE_CALL_SOURCE, RECORD_CALL_SOURCE] {
        let target = NativeTarget::macos_arm64();
        let staged = staged(source, target);
        let compiled = staged.optimized_target();
        let validate = |plan| {
            target_operations_to_selected_instructions::validate_legalized_operations(
                compiled.target_operations(),
                compiled.optimized().plan(),
                compiled.optimized(),
                plan,
            )
        };
        validate(staged.legalized().plan().clone()).unwrap();
        for mutation in 0..6 {
            let mut proposed = staged.legalized().plan().clone();
            if mutation == 5 {
                // Lend the other arm's leaf: the edge no longer names its
                // retained root and path.
                let binding = proposed
                    .scalar_functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .find_map(|block| match &mut block.terminator {
                        legalized_operations::LegalizedScalarTerminator::Jump {
                            successor, ..
                        } => successor.structural_bindings.first_mut(),
                        _ => None,
                    })
                    .unwrap();
                binding.argument.path =
                    vec![terminal_psi::StructuralPathSegment::Field("unknown".into())];
            } else {
                let argument = proposed
                    .scalar_functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .flat_map(|block| &mut block.instructions)
                    .find_map(|row| match &mut row.kind {
                        LegalizedScalarInstructionKind::Call(call) => call
                            .arguments
                            .iter_mut()
                            .find_map(|argument| match argument {
                                LegalizedScalarArgument::Structural { target, .. }
                                    if matches!(
                                        target.source,
                                        TargetStructuralArgumentSource::BlockParameter { .. }
                                    ) =>
                                {
                                    Some(target)
                                }
                                _ => None,
                            }),
                        _ => None,
                    })
                    .unwrap();
                match mutation {
                    // A wider referent than the join carries.
                    0 => {
                        argument.shape = calling_conventions::ValueShape::borrowed_reference(
                            argument.shape.byte_size + 8,
                            8,
                        )
                    }
                    // Access widening past the shared join.
                    1 => argument.access = terminal_psi::StructuralAccess::MutableBorrow,
                    // A substituted referent type.
                    2 => {
                        argument.structural_type =
                            semantic_vocabulary::StructuralTypeId::new(9_999).unwrap();
                        argument.root_structural_type = argument.structural_type;
                    }
                    // A forged home: another block's carrier.
                    3 => {
                        let TargetStructuralArgumentSource::BlockParameter { block, .. } =
                            &mut argument.source
                        else {
                            unreachable!()
                        };
                        *block = semantic_vocabulary::BlockId::new(9_999).unwrap();
                    }
                    // A projection that no longer starts at the joined referent.
                    _ => argument.source_byte_offset = 8,
                }
            }
            let outcome = validate(proposed);
            assert!(
                outcome.is_err(),
                "legalized mutation {mutation} must be rejected: {:?}",
                outcome.map(|_| ())
            );
        }
    }
}

#[test]
fn selection_rejects_forged_address_transports() {
    use selected_instructions::{SelectedAddressBase, SelectedStructuralTransport};
    for source in [PRIMITIVE_CALL_SOURCE, RECORD_CALL_SOURCE] {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_x64()] {
            let staged = staged(source, target);
            let environment = staged.register_environment();
            let constraints = target_operations_to_selected_instructions::selection_constraints(
                staged.legalized(),
                environment,
            );
            let validate = |plan| {
                target_operations_to_selected_instructions::validate_selected_instructions(
                    staged.legalized(),
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    plan,
                )
            };
            validate(staged.selected().plan().clone()).unwrap();
            let transports = staged
                .selected()
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .filter_map(|block| match &block.terminator {
                    selected_instructions::SelectedTerminator::Jump { successor, .. } => {
                        Some(successor)
                    }
                    _ => None,
                })
                .flat_map(|successor| &successor.structural_bindings)
                .filter(|binding| {
                    matches!(
                        binding.transport,
                        SelectedStructuralTransport::Address { .. }
                    )
                })
                .count();
            assert_eq!(transports, 2, "both arms lend an address");
            for mutation in 0..5 {
                let mut proposed = staged.selected().plan().clone();
                let transport = proposed
                    .functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .find_map(|block| match &mut block.terminator {
                        selected_instructions::SelectedTerminator::Jump { successor, .. } => {
                            successor
                                .structural_bindings
                                .iter_mut()
                                .find_map(|binding| {
                                    matches!(
                                        binding.transport,
                                        SelectedStructuralTransport::Address { .. }
                                    )
                                    .then_some(&mut binding.transport)
                                })
                        }
                        _ => None,
                    })
                    .unwrap();
                let SelectedStructuralTransport::Address {
                    base,
                    byte_offset,
                    byte_count,
                    destination,
                } = transport
                else {
                    unreachable!()
                };
                match mutation {
                    0 => *byte_offset += 8,
                    1 => *byte_count += 8,
                    2 => {
                        *destination = selected_instructions::LocalStorageSlotId::Spill {
                            register: selected_instructions::VirtualRegisterId(0),
                        }
                    }
                    3 => {
                        *base = SelectedAddressBase::Register(
                            selected_instructions::VirtualRegisterId(0),
                        )
                    }
                    // The lent address is not a descriptor copy.
                    _ => {
                        *transport = SelectedStructuralTransport::Descriptor {
                            argument: selected_instructions::VirtualRegisterId(0),
                            destination: *destination,
                        }
                    }
                }
                assert!(
                    validate(proposed).is_err(),
                    "selected mutation {mutation} on {target:?} must be rejected"
                );
            }
        }
    }
}
