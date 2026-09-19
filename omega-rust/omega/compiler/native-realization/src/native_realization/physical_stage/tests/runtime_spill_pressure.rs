//! A loop machine whose forty `u64` xor results are defined before the loop and
//! carried into it, exceeding every target's allocatable GPR supply, so register
//! allocation must recover through runtime spill.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::*;

const LIVE: u64 = 40;

fn artifact() -> (Vec<u8>, Vec<u8>) {
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: u64_type,
    };
    let parameter = 1;
    let mut next_value = 2;
    let mut operations = Vec::new();
    let operation = |result: u64, kind: OperationKind| Operation {
        static_reach_binding: None,
        id: OperationId::new(result).unwrap(),
        result: OperationResult::Scalar(value(result)),
        kind,
    };
    let mut live_values = Vec::new();
    for ordinal in 0..LIVE {
        let constant = next_value;
        next_value += 1;
        operations.push(operation(
            constant,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0x0101_0101_0101_0101u128 * u128::from(ordinal + 1)),
            },
        ));
        let mixed = next_value;
        next_value += 1;
        operations.push(operation(
            mixed,
            OperationKind::IntegerBitwiseXor {
                left: ValueId::new(parameter).unwrap(),
                right: ValueId::new(constant).unwrap(),
            },
        ));
        live_values.push(mixed);
    }
    let acc_parameter = next_value;
    next_value += 1;
    let mut loop_operations = Vec::new();
    let mut acc = acc_parameter;
    for &mixed in &live_values {
        let sum = next_value;
        next_value += 1;
        loop_operations.push(operation(
            sum,
            OperationKind::IntegerBitwiseXor {
                left: ValueId::new(acc).unwrap(),
                right: ValueId::new(mixed).unwrap(),
            },
        ));
        acc = sum;
    }
    let zero = next_value;
    next_value += 1;
    loop_operations.push(operation(
        zero,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0),
        },
    ));
    let condition = next_value;
    next_value += 1;
    loop_operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(condition).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(condition).unwrap(),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerEqual {
            left: ValueId::new(acc).unwrap(),
            right: ValueId::new(zero).unwrap(),
        },
    });
    let result_parameter = next_value;
    next_value += 1;
    let successor = |id, block, argument: u64| SuccessorEdge {
        erased_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        edge: EdgeId::new(id).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: vec![ValueId::new(argument).unwrap()],
        trivial_affine_discards: Vec::new(),
    };
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(1).unwrap(),
        attachment: None,
        parameters: vec![value(parameter)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(next_value)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operations,
                terminator: Terminator::Jump {
                    erased_arguments: Vec::new(),
                    edge: EdgeId::new(2).unwrap(),
                    target: BlockId::new(2).unwrap(),
                    arguments: vec![ValueId::new(live_values[0]).unwrap()],
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(2).unwrap(),
                parameters: vec![value(acc_parameter)],
                operations: loop_operations,
                terminator: Terminator::Conditional {
                    condition: ValueId::new(condition).unwrap(),
                    when_true: successor(3, 3, acc),
                    when_false: successor(4, 2, acc),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(3).unwrap(),
                parameters: vec![value(result_parameter)],
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge: EdgeId::new(1).unwrap(),
                    value: ValueId::new(result_parameter).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            },
        ],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(1).unwrap(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
            crash_routes: Vec::new(),
        },
    };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine],
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &ProofBundle::default()).unwrap(),
    )
}

/// One straight-line block: the parameter feeds forty xor mixes and a sum
/// chain, so every mixed value is simultaneously live without any cross-block
/// range. The fixed/precolored front-end admits this topology; post-copy
/// assignment is the leg that runs out of allocatable views.
fn acyclic_artifact() -> (Vec<u8>, Vec<u8>) {
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: u64_type,
    };
    let parameter = 1;
    let mut next_value = 2;
    let operation = |result: u64, kind: OperationKind| Operation {
        static_reach_binding: None,
        id: OperationId::new(result).unwrap(),
        result: OperationResult::Scalar(value(result)),
        kind,
    };
    let mut operations = Vec::new();
    let mut live_values = Vec::new();
    for ordinal in 0..LIVE {
        let constant = next_value;
        next_value += 1;
        operations.push(operation(
            constant,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0x0101_0101_0101_0101u128 * u128::from(ordinal + 1)),
            },
        ));
        let mixed = next_value;
        next_value += 1;
        operations.push(operation(
            mixed,
            OperationKind::IntegerBitwiseXor {
                left: ValueId::new(parameter).unwrap(),
                right: ValueId::new(constant).unwrap(),
            },
        ));
        live_values.push(mixed);
    }
    let mut acc = live_values[0];
    for &mixed in &live_values[1..] {
        let sum = next_value;
        next_value += 1;
        operations.push(operation(
            sum,
            OperationKind::IntegerBitwiseXor {
                left: ValueId::new(acc).unwrap(),
                right: ValueId::new(mixed).unwrap(),
            },
        ));
        acc = sum;
    }
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(1).unwrap(),
        attachment: None,
        parameters: vec![value(parameter)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(next_value)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            operations,
            terminator: Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: ValueId::new(acc).unwrap(),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(1).unwrap(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
            crash_routes: Vec::new(),
        },
    };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine],
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &ProofBundle::default()).unwrap(),
    )
}

fn lower(
    target: target::NativeTarget,
) -> (
    abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    optimization_core::PostTerminalOptimizationSelectionProjection,
) {
    lower_with_selections(
        target,
        &artifact(),
        &optimization_core::OptimizationSelections::default(),
    )
}

fn lower_with_selections(
    target: target::NativeTarget,
    artifact: &(Vec<u8>, Vec<u8>),
    selections: &optimization_core::OptimizationSelections,
) -> (
    abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    optimization_core::PostTerminalOptimizationSelectionProjection,
) {
    let (semantic, proof) = artifact;
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    let optimized = crate::optimize_verified_abstract_input(
        input,
        crate::compiler_baseline_request_v1(selections),
    )
    .unwrap();
    let post_terminal = optimized.selections().project_post_terminal();
    let target_program =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
        )
        .unwrap();
    (target_program, post_terminal)
}

#[test]
fn loop_carried_u64_pressure_recovers_through_runtime_spill() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::uefi_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (target_program, _) = lower(target);
        let register_environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let selected =
            target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                target_program,
                register_environment,
            )
            .unwrap();
        let selected =
            selected_instructions_to_selected_instructions::optimize_selected_instructions(
                selected,
            )
            .unwrap();
        let allocation =
            selected_instructions_to_register_homes::stage_register_allocation(selected)
                .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        assert!(
            matches!(
                allocation.current().evidence(),
                selected_instructions_to_register_homes::AllocationEvidence::RuntimeSpill(_)
            ),
            "{target:?}: {:?}",
            allocation.current().evidence()
        );
        let (target_program, post_terminal) = lower(target);
        crate::stage_optimized_verified_physical_pipeline(
            target_program,
            post_terminal.selections(),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
    }
}

#[test]
fn loop_carried_spill_frame_replays_private_accesses_through_callable_publication() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::uefi_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (target_program, post_terminal) = lower(target);
        let physical = crate::stage_optimized_verified_physical_pipeline(
            target_program,
            post_terminal.selections(),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let layout = physical.fixed_frame_for_test().frame().plan();
        let frame = layout.functions.first().unwrap();
        assert!(
            !frame.local_storage_slots.is_empty() && frame.frame_size_bytes != 0,
            "{target:?}: spill storage must occupy a nonzero frame: {frame:?}"
        );
        let emitted = machine_emission::stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let function = emitted.fragments().functions.first().unwrap();
        assert!(
            function.blocks.len() > 3,
            "{target:?}: {}",
            function.blocks.len()
        );
        let has_backward_branch = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|row| row.branch.as_deref())
            .any(|branch| match branch {
                machine_code::FunctionFragmentBranchEvidence::Conditional(branch) => {
                    branch.byte_displacement < 0
                }
                machine_code::FunctionFragmentBranchEvidence::Jump(jump) => {
                    jump.byte_displacement < 0
                }
            });
        assert!(has_backward_branch, "{target:?}: loop back edge");
        let applied = machine_emission::stage_function_fragment_frame_application(emitted)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        assert_eq!(applied.receipt().epilogue_application_count(), 1);
        let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let object = object_file::stage_optimized_relocation_free_object_container(text)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let (semantic, proof) = artifact();
        let module = terminal_codec::decode_module(&semantic).unwrap();
        let proof = terminal_codec::decode_proof_section_for(&module, &proof).unwrap();
        let optimization =
            terminal_codec::build_identity_optimization_execution_record(&module, &proof).unwrap();
        let terminal = terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &proof,
            &optimization,
            None,
        )
        .unwrap();
        let artifact = object_file::stage_validated_optimized_object_artifact(terminal, object)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let callable = native_artifact::stage_validated_optimized_ordinary_callable_entry(artifact)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        assert_eq!(callable.entry().returns.len(), 1, "{target:?}");
    }
}

#[test]
fn acyclic_u64_pressure_recovers_through_runtime_spill_on_the_default_route() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (target_program, _) = lower_with_selections(
            target,
            &acyclic_artifact(),
            &optimization_core::OptimizationSelections::default(),
        );
        let register_environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let selected =
            target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                target_program,
                register_environment,
            )
            .unwrap();
        let selected =
            selected_instructions_to_selected_instructions::optimize_selected_instructions(
                selected,
            )
            .unwrap();
        let allocation =
            selected_instructions_to_register_homes::stage_register_allocation(selected)
                .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        assert!(
            matches!(
                allocation.current().evidence(),
                selected_instructions_to_register_homes::AllocationEvidence::RuntimeSpill(_)
            ),
            "{target:?}: {:?}",
            allocation.current().evidence()
        );
    }
}

// The declared shared-entry fixed-view route is entered before ordinary
// assignment even when no authenticated boundary materializes copies, and its
// fixed/precolored front-end is stricter than direct assignment: this
// program's simultaneous liveness exceeds what segment-home placement can
// assign on some targets and exceeds the front-end's per-pass work budget on
// the wider aarch64 files, so the route probes the front-end on the borrowed
// legality and declines the sequence to runtime-spill recovery on either
// capacity verdict before custody is consumed. Residual pressure on a
// declared fixed-view route is a distinct failure class from unassigned
// entry transitions: runtime-spill recovery stays reachable, whether the
// sequence is skipped up front or the spill extends the reanalyzed post-copy
// program. When copies did materialize they stay ahead of the spill steps in
// one manifest.
#[test]
fn loop_carried_u64_pressure_after_declared_fixed_view_sequence_recovers_through_runtime_spill() {
    use selected_instructions_to_register_homes::{
        AllocationSource, PostAllocationSelectedTransformation,
    };
    let selections = optimization_core::OptimizationSelections::new([
        optimization_core::Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
    ])
    .unwrap();
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (target_program, _) = lower_with_selections(target, &acyclic_artifact(), &selections);
        let register_environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let selected =
            target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                target_program,
                register_environment,
            )
            .unwrap();
        let selected =
            selected_instructions_to_selected_instructions::optimize_selected_instructions(
                selected,
            )
            .unwrap();
        let allocation =
            selected_instructions_to_register_homes::stage_register_allocation(selected)
                .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let transformations = &allocation
            .current()
            .post_allocation_manifest()
            .record()
            .selected_transformations;
        let first_spill = transformations.iter().position(|transformation| {
            matches!(
                transformation,
                PostAllocationSelectedTransformation::RuntimeSpill(_)
                    | PostAllocationSelectedTransformation::RuntimeRematerialization(_)
            )
        });
        assert!(first_spill.is_some(), "{target:?}: {transformations:?}");
        // A fixed-view transformation, when the sequence ran far enough to
        // record one, stays ahead of every runtime-spill step in the manifest.
        assert!(
            transformations
                .iter()
                .skip(first_spill.expect("asserted above"))
                .all(|transformation| !matches!(
                    transformation,
                    PostAllocationSelectedTransformation::FixedViewCopy(_)
                )),
            "{target:?}: {transformations:?}"
        );
        allocation
            .replay_allocation()
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let (target_program, post_terminal) =
            lower_with_selections(target, &acyclic_artifact(), &selections);
        crate::stage_optimized_verified_physical_pipeline(
            target_program,
            post_terminal.selections(),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
    }
}
