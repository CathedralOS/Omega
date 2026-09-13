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
                structural_parameters: Vec::new(),
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operations,
                terminator: Terminator::Jump {
                    edge: EdgeId::new(2).unwrap(),
                    target: BlockId::new(2).unwrap(),
                    arguments: vec![ValueId::new(live_values[0]).unwrap()],
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            },
            Block {
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
        terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
    )
}

fn lower(
    target: target::NativeTarget,
) -> (
    abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    optimization_core::PostTerminalOptimizationSelectionProjection,
) {
    let (semantic, proof) = artifact();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let optimized = crate::optimize_verified_abstract_input(
        input,
        crate::compiler_baseline_request_v1(&optimization_core::OptimizationSelections::default()),
    )
    .unwrap();
    let post_terminal = optimized.selections().project_post_terminal();
    let target_program =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized, target,
        )
        .unwrap();
    (target_program, post_terminal)
}

#[test]
fn loop_carried_u64_pressure_recovers_through_runtime_spill() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
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
        let proof = terminal_codec::decode_proof_bundle(&proof).unwrap();
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
