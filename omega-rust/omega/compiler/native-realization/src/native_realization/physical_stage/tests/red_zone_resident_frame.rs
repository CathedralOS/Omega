//! A System V leaf whose exact u64 dividend keeps its fixed RAX operand while
//! remaining live across the divide's fixed RAX result cannot be colored, so
//! register allocation recovers through runtime spill. The recovered storage
//! is allocator-owned spill slots only — no calls, no preservation slots, and
//! no hosted or structural bytes — so the whole addressed extent fits inside
//! the 128-byte red zone, every slot resolves below the unadjusted entry RSP,
//! and the frame commits nothing. Non-System-V targets keep the same spill
//! storage in an ordinary committed frame.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::*;

/// `dividend = left ^ right` is an ordinary instruction result — not a
/// `MaterializeI64`, so pressure rematerialization admission rejects it — and
/// it stays live across `quotient = dividend / right` because the final xor
/// reads it. On x86-64 the divide pins `dividend` to RAX at its operand use
/// and `quotient` to RAX at its result def while `dividend` is still live:
/// one physical view cannot serve both, so assignment fails with
/// `NoCompatibleHome` and the runtime-spill route stores one side. Recovery
/// keeps the whole spill extent inside a small number of eight-byte slots.
fn artifact() -> (Vec<u8>, Vec<u8>) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type,
    };
    let machine = MachineId::new(61).unwrap();
    let block = BlockId::new(62).unwrap();
    let left = ValueId::new(63).unwrap();
    let right = ValueId::new(64).unwrap();
    let dividend = ValueId::new(65).unwrap();
    let quotient = ValueId::new(66).unwrap();
    let result = ValueId::new(68).unwrap();
    let obligation = ObligationId::new(69).unwrap();
    let operation = |id: u64, result: ValueId, kind: OperationKind| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(declaration(result.get())),
        kind,
    };
    let one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
    let goal = Proposition::LessOrEqual(one, ScalarTerm::value(right, scalar_type));
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            parameters: vec![declaration(left.get()), declaration(right.get())],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result.get())),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    operation(
                        69,
                        dividend,
                        OperationKind::IntegerBitwiseXor { left, right },
                    ),
                    operation(
                        70,
                        quotient,
                        OperationKind::ExactIntegerDivide {
                            left: dividend,
                            right,
                            obligation,
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(72).unwrap(),
                    value: dividend,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(73).unwrap(),
                crash_routes: Vec::new(),
                requires: vec![goal.clone()],
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(74).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
    )
}

fn lower(
    target: target::NativeTarget,
) -> (
    abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    optimization_core::PostTerminalOptimizationSelectionProjection,
) {
    let (semantic, proof) = artifact();
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
        crate::compiler_baseline_request_v1(&optimization_core::OptimizationSelections::default()),
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

/// The frame-plan inputs replay needs: the post-allocation machine plan plus
/// the validated preservation requirements, storage, and environment.
fn frame_inputs(
    target: target::NativeTarget,
    artifact: (Vec<u8>, Vec<u8>),
) -> (
    register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan,
    selected_instructions_to_register_homes::ValidatedAllocatedCalleeSavedRequirements,
    machine_emission::frame_layout::ValidatedNonAuthoritativeCalleeSaveStorage,
    register_environment::ValidatedTargetRegisterEnvironment,
    machine_code::TargetFrameLayoutPlan,
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
        crate::compiler_baseline_request_v1(&optimization_core::OptimizationSelections::default()),
    )
    .unwrap();
    let target_program =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
        )
        .unwrap();
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    let selected =
        target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            target_program,
            environment,
        )
        .unwrap();
    let selected =
        selected_instructions_to_selected_instructions::optimize_selected_instructions(selected)
            .unwrap();
    let allocation =
        selected_instructions_to_register_homes::stage_register_allocation(selected).unwrap();
    let current = allocation.current();
    let machine =
        register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan(
            &current,
        )
        .unwrap();
    let requirements =
        selected_instructions_to_register_homes::stage_allocated_callee_saved_requirements(
            &current,
            selected_instructions_to_register_homes::AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
            current.budget_per_pass(),
        )
        .unwrap();
    let storage = machine_emission::frame_layout::stage_non_authoritative_callee_save_storage(
        &requirements,
        current.register_environment(),
        machine_emission::frame_layout::NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        current.budget_per_pass(),
    )
    .unwrap();
    let layout = machine_emission::frame_layout::stage_target_frame_layout(
        &machine,
        &requirements,
        &storage,
        current.register_environment(),
        machine_code::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
    )
    .unwrap();
    (
        machine,
        requirements,
        storage,
        current.register_environment().clone(),
        layout.plan().clone(),
    )
}

#[test]
fn fixed_rax_dividend_spill_stays_inside_the_sysv_red_zone() {
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
            !frame.contains_call,
            "{target:?}: the fixed-register leaf has no call: {frame:?}"
        );
        let is_x86_64 = matches!(target.architecture, target::Architecture::X86_64);
        if is_x86_64 {
            // The fixed rax/rdx divide pressure produces exactly allocator
            // spill storage on every x86-64 ABI.
            assert!(
                !frame.local_storage_slots.is_empty()
                    && frame.local_storage_slots.iter().all(|slot| {
                        matches!(
                            slot.id,
                            selected_instructions::LocalStorageSlotId::Spill { .. }
                        )
                    }),
                "{target:?}: recovery produced only allocator spill slots: {frame:?}"
            );
        }
        if layout.abi
            == machine_emission::frame_layout::FrameAbiPreservationConvention::SystemVAMD64
        {
            // The recovered allocation keeps every flexible register in a
            // caller-saved view, so no preservation storage is committed.
            assert!(
                frame.callee_save_slots.is_empty(),
                "{target:?}: the leaf carries no callee-save storage: {frame:?}"
            );
            // Every addressed byte lives below the unadjusted entry RSP: the
            // prologue commits nothing and the return address sits at RSP+0.
            assert!(frame.frame_size_bytes <= 128, "{target:?}: {frame:?}");
            assert_eq!(
                frame.red_zone_resident_bytes, frame.frame_size_bytes,
                "{target:?}: {frame:?}"
            );
            assert_eq!(
                frame.return_address,
                machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: 0,
                    size_bytes: 8,
                },
                "{target:?}"
            );
            assert_eq!(frame.stack_probe.touches, 0, "{target:?}");
            // The resident slots encode signed below-RSP displacements.
            let negative = physical
                .fixed_frame_for_test()
                .encoding()
                .rows()
                .iter()
                .filter_map(|row| row.address)
                .any(|address| address.displacement < 0);
            assert!(
                negative,
                "{target:?}: resident spill access must resolve below RSP"
            );
        } else {
            assert_eq!(
                frame.red_zone_resident_bytes, 0,
                "{target:?}: no other admitted ABI exposes a red zone: {frame:?}"
            );
        }
        // The same program still reaches the ordinary callable publication on
        // every target, with the resident form replayed independently.
        let applied = machine_emission::stage_function_fragment_frame_application(
            machine_emission::stage_optimized_function_fragment_emission(
                physical.into_function_fragment_emission_source(),
            )
            .unwrap_or_else(|error| panic!("{target:?}: {error:?}")),
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
        assert_eq!(
            applied.receipt().epilogue_application_count(),
            1,
            "{target:?}"
        );
    }
}

#[test]
fn unwind_replay_rejects_custody_outside_the_declared_mechanism() {
    // The unwind matrix pins each declared (architecture, object-format)
    // pair's continuation mechanism: a submitted row whose recorded
    // return-address custody belongs to another mechanism fails closed in
    // replay even when its coordinates are self-consistent. The host Linux
    // targets cover both declared mechanisms: linux-x64 keeps the
    // continuation in caller-stack custody while linux-arm64 unwinds
    // through the link register.
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        for program in [
            artifact(),
            super::general_call_frame::preserving_call_artifact(),
        ] {
            let (machine, requirements, storage, environment, plan) = frame_inputs(target, program);
            let replay = |candidate: machine_code::TargetFrameLayoutPlan| {
                machine_emission::frame_layout::validate_target_frame_layout(
                    &machine,
                    &requirements,
                    &storage,
                    &environment,
                    candidate,
                )
            };
            // The produced rows replay exactly before any drift is applied.
            assert!(replay(plan.clone()).is_ok(), "{target:?}");
            for index in 0..plan.functions.len() {
                let row = &plan.functions[index];
                let foreign = match row.return_address {
                    machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                        post_prologue_offset_bytes,
                        size_bytes,
                    } => machine_code::ReturnAddressFrameCustody::SavedLinkRegister {
                        view: row.stack_pointer,
                        frame_offset_bytes: post_prologue_offset_bytes,
                        size_bytes,
                    },
                    machine_code::ReturnAddressFrameCustody::LiveLinkRegister { .. }
                    | machine_code::ReturnAddressFrameCustody::SavedLinkRegister { .. } => {
                        machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                            post_prologue_offset_bytes: 0,
                            size_bytes: 8,
                        }
                    }
                };
                // Keep the unwind roster's restated custody consistent with
                // the drifted record so the only check that can fail is the
                // declared mechanism gate itself.
                let mut drifted = plan.clone();
                drifted.functions[index].return_address = foreign;
                drifted.functions[index].unwind.return_address = foreign;
                assert!(
                    replay(drifted).is_err(),
                    "{target:?}: custody outside the declared mechanism must fail closed: {foreign:?}"
                );
            }
        }
    }
}

#[test]
fn unwind_replay_rejects_in_mechanism_custody_drift() {
    // A saved-link frame that drifts to the live-link form — still an
    // instance of the pair's declared mechanism — fails the canonical-shape
    // check inside the row rather than the mechanism gate.
    let target = target::NativeTarget::linux_arm64();
    let (machine, requirements, storage, environment, plan) = frame_inputs(
        target,
        super::general_call_frame::preserving_call_artifact(),
    );
    let caller = plan
        .functions
        .iter()
        .position(|row| row.contains_call)
        .expect("the preserving caller contains calls");
    let machine_code::ReturnAddressFrameCustody::SavedLinkRegister { view, .. } =
        plan.functions[caller].return_address
    else {
        panic!("a calling AArch64 frame must save its link register");
    };
    let mut drifted = plan.clone();
    drifted.functions[caller].return_address =
        machine_code::ReturnAddressFrameCustody::LiveLinkRegister { view };
    drifted.functions[caller].unwind.return_address =
        machine_code::ReturnAddressFrameCustody::LiveLinkRegister { view };
    assert!(
        machine_emission::frame_layout::validate_target_frame_layout(
            &machine,
            &requirements,
            &storage,
            &environment,
            drifted,
        )
        .is_err(),
        "{target:?}: a calling frame cannot record the leaf live-link form"
    );
}

#[test]
fn resident_frame_replay_rejects_every_ineligible_shape() {
    let target = target::NativeTarget::linux_x64();
    let (machine, requirements, storage, environment, plan) = frame_inputs(target, artifact());
    let layout = || {
        machine_emission::frame_layout::validate_target_frame_layout(
            &machine,
            &requirements,
            &storage,
            &environment,
            plan.clone(),
        )
    };
    // The produced resident frame replays exactly.
    assert!(layout().is_ok());
    let mut partial = plan.clone();
    partial.functions[0].red_zone_resident_bytes = partial.functions[0].frame_size_bytes / 2;
    assert!(
        machine_emission::frame_layout::validate_target_frame_layout(
            &machine,
            &requirements,
            &storage,
            &environment,
            partial,
        )
        .is_err(),
        "partial residency is not a canonical frame"
    );
    let mut over = plan.clone();
    over.functions[0].red_zone_resident_bytes = over.functions[0].frame_size_bytes + 8;
    assert!(
        machine_emission::frame_layout::validate_target_frame_layout(
            &machine,
            &requirements,
            &storage,
            &environment,
            over,
        )
        .is_err(),
        "resident bytes can never exceed the addressed extent"
    );
    // A frame carrying a call or preservation storage can never be resident.
    let (call_machine, call_requirements, call_storage, call_environment, call_plan) = frame_inputs(
        target,
        super::general_call_frame::preserving_call_artifact(),
    );
    let caller = call_plan
        .functions
        .iter()
        .position(|row| row.contains_call)
        .expect("the preserving caller contains calls");
    assert!(
        !call_plan.functions[caller].callee_save_slots.is_empty(),
        "the preserving caller carries callee-save storage"
    );
    let mut resident_call = call_plan.clone();
    resident_call.functions[caller].red_zone_resident_bytes =
        resident_call.functions[caller].frame_size_bytes;
    assert!(
        machine_emission::frame_layout::validate_target_frame_layout(
            &call_machine,
            &call_requirements,
            &call_storage,
            &call_environment,
            resident_call,
        )
        .is_err(),
        "a call frame with preservation storage cannot be resident"
    );
    // No other admitted ABI exposes a red zone, so the same recovered spill
    // stays a committed frame elsewhere and a resident claim replays false.
    for other in [
        target::NativeTarget::windows_x64(),
        target::NativeTarget::uefi_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (other_machine, other_requirements, other_storage, other_environment, other_plan) =
            frame_inputs(other, artifact());
        let mut resident = other_plan;
        // Where the same fixed-register pressure spilled nothing at all, the
        // claim still fails closed: resident bytes cannot exceed the extent.
        resident.functions[0].red_zone_resident_bytes =
            resident.functions[0].frame_size_bytes.max(8);
        assert!(
            machine_emission::frame_layout::validate_target_frame_layout(
                &other_machine,
                &other_requirements,
                &other_storage,
                &other_environment,
                resident,
            )
            .is_err(),
            "{other:?}: resident bytes cannot appear outside System V AMD64"
        );
    }
}
