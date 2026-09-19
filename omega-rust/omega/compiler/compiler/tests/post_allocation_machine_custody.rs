//! Post-allocation machine custody mutation coverage.
//!
//! `post_allocation_machine::stage_optimized_post_allocation_machine_plan`
//! seals a [`StagedOptimizedPostAllocationMachinePlan`] whose
//! `StagedOptimizedPostAllocationMachineCustodyReceipt` pins the replayed
//! allocation evidence, the pre-allocation machine-effect identity, the
//! post-allocation machine identity, and the retained plan's function,
//! instruction, operand, and unit-action counts. The receipt never encodes —
//! it is consumed in memory by the fixed-frame realization stage — so its
//! independent checker is the validator
//! `validate_optimized_post_allocation_machine_plan_custody`, which replays the
//! allocation source, re-validates machine effects and the machine plan, and
//! rebuilds the expected receipt before comparing it against the retained
//! claim. A substitution in any field makes the rebuilt receipt diverge, so
//! replay rejects with `ReceiptMismatch` before any downstream custody
//! decision.
//!
//! Because the family has no wire form, every field is representable; none is
//! rejected at canonical encoding. This test is exercised beneath the real
//! optimized physical pipeline — the same `native_realization` staging route
//! `realization_custody.rs` uses — because no honest staged fixture exists
//! inside `register-homes-to-post-allocation-machine`: the sealed
//! `AllocationSource` is produced only by the real register-allocation stage.

use machine_emission::StagedFixedFrameFunctionRelativeRealization;
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError, PostAllocationMachineCustodyFieldForTest,
    StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_custody,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use target::NativeTarget;
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ProofBundle,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};

/// One callee returning a constant and one entry caller invoking it: the
/// smallest module that retains real machine rows for the custody receipt's
/// counts rather than a fabricated record.
fn calling_module() -> TerminalModule {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type,
    };
    let contract = |id: u64| MachineContract {
        id: ContractId::new(id).unwrap(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
        crash_routes: Vec::new(),
    };
    let machine = |id: u64,
                   parameters: Vec<ValueDeclaration>,
                   result: TerminalMachineResult,
                   entry: BlockId,
                   operations: Vec<Operation>,
                   terminator: Terminator| TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(id).unwrap(),
        attachment: None,
        parameters,
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            operations,
            terminator,
        }],
        contract: contract(id + 1000),
    };
    let callee = machine(
        2,
        vec![declaration(21)],
        TerminalMachineResult::Scalar(declaration(26)),
        BlockId::new(23).unwrap(),
        vec![Operation {
            static_reach_binding: None,
            id: OperationId::new(24).unwrap(),
            result: OperationResult::Scalar(declaration(22)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(41),
            },
        }],
        Terminator::Return {
            edge: EdgeId::new(25).unwrap(),
            value: ValueId::new(22).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    let caller = machine(
        1,
        Vec::new(),
        TerminalMachineResult::Scalar(declaration(17)),
        BlockId::new(11).unwrap(),
        vec![
            Operation {
                static_reach_binding: None,
                id: OperationId::new(12).unwrap(),
                result: OperationResult::Scalar(declaration(14)),
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(7),
                },
            },
            Operation {
                static_reach_binding: None,
                id: OperationId::new(15).unwrap(),
                result: OperationResult::Scalar(declaration(13)),
                kind: OperationKind::Call {
                    callee: MachineId::new(2).unwrap(),
                    arguments: vec![ValueId::new(14).unwrap()],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
        ],
        Terminator::Return {
            edge: EdgeId::new(16).unwrap(),
            value: ValueId::new(13).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(1).unwrap(),
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
    }
}

/// Stage the calling module through the real optimized physical pipeline for
/// `target`. The retained post-allocation machine plan replays cleanly before
/// any mutation.
fn staged_realization(target: NativeTarget) -> StagedFixedFrameFunctionRelativeRealization {
    let module = calling_module();
    let proof = ProofBundle::default();
    let semantic = terminal_codec::encode_module(&module).expect("encode calling module");
    let proof_bytes =
        terminal_codec::encode_proof_section(&module, &proof).expect("encode calling proof");
    let optimized = native_realization::optimize_artifact_sections(
        &semantic,
        &proof_bytes,
        &AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
    )
    .expect("calling module optimizes");
    let staged =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .expect("calling module reaches physical staging")
        .into_fixed_frame_for_test();
    assert_eq!(
        validate_optimized_post_allocation_machine_plan_custody(
            staged.allocation(),
            staged.machine()
        ),
        Ok(staged.machine().custody().clone()),
        "{target:?}: honest staged machine plan must replay before mutation",
    );
    staged
}

#[test]
fn post_allocation_machine_custody_rejects_every_one_field_substitution() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let staged = staged_realization(target);
        // An authentic foreign realization on the opposite architecture
        // supplies the donor allocation evidence below.
        let foreign = staged_realization(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            target::Architecture::Aarch64 => NativeTarget::linux_x64(),
        });
        assert_ne!(
            staged.machine().custody().source(),
            foreign.machine().custody().source(),
            "{target:?}: the foreign target must produce distinct allocation evidence",
        );

        // Every receipt field is representable in memory and substitutes
        // independently; the validator is the named independent checker and
        // each leg must be rejected there.
        let fields: [(&str, PostAllocationMachineCustodyFieldForTest); 7] = [
            ("source", PostAllocationMachineCustodyFieldForTest::Source),
            ("effects", PostAllocationMachineCustodyFieldForTest::Effects),
            ("machine", PostAllocationMachineCustodyFieldForTest::Machine),
            (
                "function_count",
                PostAllocationMachineCustodyFieldForTest::FunctionCount,
            ),
            (
                "instruction_count",
                PostAllocationMachineCustodyFieldForTest::InstructionCount,
            ),
            (
                "operand_count",
                PostAllocationMachineCustodyFieldForTest::OperandCount,
            ),
            (
                "unit_action_count",
                PostAllocationMachineCustodyFieldForTest::UnitActionCount,
            ),
        ];
        for (name, field) in fields {
            let mut substituted: StagedOptimizedPostAllocationMachinePlan =
                staged.machine().clone();
            substituted.corrupt_custody_for_test(field, foreign.machine());
            assert_eq!(
                validate_optimized_post_allocation_machine_plan_custody(
                    staged.allocation(),
                    &substituted,
                ),
                Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch),
                "{target:?}: independent replay must reject substituted machine-custody field {name}",
            );
        }
    }
}
