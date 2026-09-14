//! A caller whose outgoing ABI area exceeds one stack-commit granule exercises
//! the committed-stack probe roster end to end: the layout commits an exact
//! touch schedule, the x86-64 protocol emits one granule chunk per touch, and
//! the whole program still reaches ordinary callable publication. The AArch64
//! targets keep their real single-instruction frame bound: the layout records
//! the roster demand and the protocol stage rejects what it cannot encode.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::*;

/// Six register arguments plus 524 stack arguments push the caller's outgoing
/// ABI area past one 4 KiB commit granule on every admitted target.
const ARGUMENTS: u64 = 530;

const CALLER: u64 = 1;
const CALLEE: u64 = 2;
const CALLER_BASE: u64 = 31_000;
const CALLEE_BASE: u64 = 32_000;

fn wide_scalar_call_artifact() -> (Vec<u8>, Vec<u8>) {
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: u64_type,
    };
    let argument = ValueId::new(CALLER_BASE + 1).unwrap();
    let call_result = ValueId::new(CALLER_BASE + 2).unwrap();
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(CALLER).unwrap(),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(CALLER_BASE + 3)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(CALLER_BASE).unwrap(),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(CALLER_BASE).unwrap(),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(CALLER_BASE + 1).unwrap(),
                    result: OperationResult::Scalar(value(CALLER_BASE + 1)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(5),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(CALLER_BASE + 2).unwrap(),
                    result: OperationResult::Scalar(value(CALLER_BASE + 2)),
                    kind: OperationKind::Call {
                        callee: MachineId::new(CALLEE).unwrap(),
                        arguments: vec![argument; ARGUMENTS as usize],
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                },
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(CALLER_BASE + 4).unwrap(),
                value: call_result,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(CALLER_BASE + 5).unwrap(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
            crash_routes: Vec::new(),
        },
    };
    let callee = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(CALLEE).unwrap(),
        attachment: None,
        parameters: (0..ARGUMENTS)
            .map(|index| value(CALLEE_BASE + index))
            .collect(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(CALLEE_BASE + ARGUMENTS)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(CALLEE_BASE).unwrap(),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(CALLEE_BASE).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: EdgeId::new(CALLEE_BASE + 1).unwrap(),
                value: ValueId::new(CALLEE_BASE + ARGUMENTS - 1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(CALLEE_BASE + 2).unwrap(),
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
        entry: MachineId::new(CALLER).unwrap(),
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
        machines: vec![caller, callee],
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
    let (semantic, proof) = wide_scalar_call_artifact();
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
fn wide_outgoing_area_commits_through_exact_probe_roster_and_publication() {
    let caller = MachineId::new(CALLER).unwrap();
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
    ] {
        let (target_program, post_terminal) = lower(target);
        let physical = crate::stage_optimized_verified_physical_pipeline(
            target_program,
            post_terminal.selections(),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let realization = physical.fixed_frame_for_test();
        let layout = realization.frame();
        let row = layout
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        assert!(row.contains_call, "{target:?}");
        assert!(
            row.frame_size_bytes > isa_x86_64::X86_64_STACK_PROBE_INTERVAL_BYTES,
            "{target:?}: outgoing ABI area must exceed one commit granule: {row:?}"
        );
        assert_eq!(
            row.stack_probe.interval_bytes,
            isa_x86_64::X86_64_STACK_PROBE_INTERVAL_BYTES,
            "{target:?}"
        );
        assert_eq!(
            u64::from(row.stack_probe.touches),
            row.frame_size_bytes
                .div_ceil(isa_x86_64::X86_64_STACK_PROBE_INTERVAL_BYTES),
            "{target:?}"
        );
        assert_eq!(layout.receipt().probed_function_count(), 1, "{target:?}");
        let protocol = realization.protocol().plan();
        let encoding = protocol
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        let prologue = encoding.prologue.bytes(&protocol.bytes).unwrap();
        // Each committed chunk is one granule move followed by a touch of the
        // newly entered page: `sub rsp, 4096 ; cmp byte ptr [rsp], 0`.
        let chunk = [
            0x48, 0x81, 0xec, 0x00, 0x10, 0x00, 0x00, 0x80, 0x3c, 0x24, 0x00,
        ];
        assert!(
            prologue.starts_with(&chunk),
            "{target:?}: prologue {prologue:02x?}"
        );
        let emitted = machine_emission::stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let applied = machine_emission::stage_function_fragment_frame_application(emitted)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let object = object_file::stage_optimized_relocation_free_object_container(text)
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        let (semantic, proof) = wide_scalar_call_artifact();
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
    for target in [
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (target_program, post_terminal) = lower(target);
        let error = crate::stage_optimized_verified_physical_pipeline(
            target_program,
            post_terminal.selections(),
        )
        .expect_err("AArch64 keeps its single-instruction frame bound");
        assert!(
            matches!(
                error,
                crate::OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization(
                    machine_emission::FunctionRelativeOptimizationRealizationError::FrameProtocol(
                        machine_emission::TargetFrameProtocolEncodingError::Aarch64(
                            isa_aarch64::Aarch64FrameProtocolError::InvalidFrameSize,
                        ),
                    ),
                )
            ),
            "{target:?}: {error:?}"
        );
    }
}
