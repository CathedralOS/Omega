//! A System V leaf whose recovered runtime-spill storage fits inside the ABI
//! red zone commits no stack bytes at all: the validated layout records the
//! whole addressed extent as resident, the frame protocol emits an empty
//! prologue, and every resident access resolves to a signed below-RSP
//! displacement. The same recovered spill stays an ordinary committed frame on
//! every other admitted ABI, a resident claim there replays false, and the
//! whole program still reaches ordinary callable publication.

use crate::tests::{
    AdmissionProfile, Block, BlockId, CertificateEnvelope, ContractId, EdgeId, EvidenceIdentity,
    EvidenceRoute, IntegerSign, IntegerType, IntegerValue, MachineContract, MachineId,
    NativeTarget, ObligationEvidence, ObligationId, Operation, OperationId, OperationKind,
    OperationResult, OptimizationSelections, ProofBundle, ProofNode, ProofRule, ProofSystemMarker,
    ScalarType, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, ValueId, VocabularyMarker, canonical_artifact, compiler_baseline_request_v1,
    optimize_artifact_sections, stage_function_fragment_frame_application,
    stage_optimized_fixed_frame_text_section, stage_optimized_function_fragment_emission,
    stage_optimized_relocation_free_object_container,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    stage_validated_optimized_object_artifact, stage_validated_optimized_ordinary_callable_entry,
    validate_optimized_ordinary_callable_entry, validate_target_frame_layout,
};
use semantic_vocabulary::{Proposition, ScalarTerm};

/// `dividend = left ^ right` stays live across `quotient = dividend / right`
/// because the return reads it. On x86-64 the divide pins `dividend` to RAX at
/// its operand use and `quotient` to RAX at its result def while `dividend` is
/// still live: one physical view cannot serve both, so register allocation
/// recovers through runtime spill. Recovery keeps the whole spill extent
/// inside a small number of eight-byte slots, so a System V leaf fits inside
/// the 128-byte red zone and commits nothing.
fn resident_leaf_artifact() -> (Vec<u8>, Vec<u8>) {
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

/// Recomputes the signed post-prologue displacement an encoded access must
/// carry under the validated frame row, mirroring the frame-address resolver.
/// Incoming storage starts above the committed extent plus the
/// caller-activation return address; outgoing and local slots resolve to their
/// frame-space offset minus the resident extent, so resident slots go
/// negative below the unadjusted entry stack pointer.
fn expected_frame_displacement(
    function: &::physical_instructions::PostAllocationMachineFunction,
    frame: &machine_code::FunctionTargetFrameLayout,
    symbolic: &::physical_instructions::PhysicalAddressOperation,
) -> i64 {
    use ::physical_instructions::PhysicalAddressOperation as Address;
    use ::selected_instructions::FrameStorageSlotId;
    let resident = i64::try_from(frame.red_zone_resident_bytes).unwrap();
    let frame_slot_start = |slot: FrameStorageSlotId| match slot {
        FrameStorageSlotId::Incoming {
            abi_stack_byte_offset,
            ..
        } => {
            let return_bytes = match frame.return_address {
                machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                    size_bytes,
                    ..
                } => u64::from(size_bytes),
                _ => 0,
            };
            let committed = frame
                .frame_size_bytes
                .checked_sub(frame.red_zone_resident_bytes)
                .unwrap();
            i64::try_from(committed + return_bytes + u64::from(abi_stack_byte_offset)).unwrap()
        }
        FrameStorageSlotId::Outgoing(id) => {
            i64::from(
                function
                    .outgoing_arguments
                    .iter()
                    .find(|slot| slot.id == id)
                    .unwrap_or_else(|| panic!("unresolved outgoing slot {id:?}"))
                    .abi_stack_byte_offset,
            ) - resident
        }
        FrameStorageSlotId::Local(id) => {
            i64::try_from(
                frame
                    .local_storage_slots
                    .iter()
                    .find(|slot| slot.id == id)
                    .unwrap_or_else(|| panic!("unresolved local slot {id:?}"))
                    .frame_offset_bytes,
            )
            .unwrap()
                - resident
        }
    };
    match symbolic {
        Address::FrameAddress { slot, byte_offset } | Address::Store64 { slot, byte_offset } => {
            frame_slot_start(*slot) + i64::from(*byte_offset)
        }
        Address::Store { byte_offset, .. }
        | Address::AddressOffset { byte_offset, .. }
        | Address::Load8 { byte_offset, .. }
        | Address::Load16 { byte_offset, .. }
        | Address::Load32 { byte_offset, .. }
        | Address::Load64 { byte_offset, .. }
        | Address::LoadPacked { byte_offset, .. }
        | Address::StorePacked { byte_offset, .. } => i64::from(*byte_offset),
        Address::Load8Indexed { .. } => 0,
        Address::HostedReadByte { .. } | Address::HostedWriteByteI32 { .. } => {
            panic!("hosted address kinds do not occur in this fixture")
        }
    }
}

/// The same recovered spill realizes two different frame shapes: resident on
/// System V AMD64, committed everywhere else. Each produced frame is replayed
/// independently against the retained roots, every symbolic access is checked
/// against the exact signed displacement the resolver computes, a residency
/// claim outside the produced geometry fails closed, and the artifact still
/// publishes as an ordinary callable on all five targets.
#[test]
fn resident_leaf_spill_frame_publishes_through_ordinary_callable_entry() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (semantic, proof) = resident_leaf_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap_or_else(|error| panic!("{target:?} physical: {error:?}"));
        let realization = physical.fixed_frame_for_test();
        let layout = realization.frame().plan();
        let frame = layout.functions.first().unwrap();
        assert!(!frame.contains_call, "{target:?}: the leaf has no call");
        let protocol = realization.protocol().plan();
        let encoding_row = protocol
            .functions
            .iter()
            .find(|function| function.machine == frame.machine)
            .unwrap();
        let prologue = encoding_row.prologue.bytes(&protocol.bytes).unwrap();
        let epilogue = encoding_row.epilogue.bytes(&protocol.bytes).unwrap();
        let system_v = layout.abi
            == machine_emission::frame_layout::FrameAbiPreservationConvention::SystemVAMD64;
        if system_v {
            // The recovered allocation keeps every flexible register in a
            // caller-saved view, so no preservation storage is committed and
            // every addressed byte lives below the unadjusted entry RSP: the
            // frame commits nothing and its protocol emits no stack movement.
            assert!(
                frame.callee_save_slots.is_empty(),
                "{target:?}: the leaf carries no callee-save storage: {frame:?}"
            );
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
            assert!(prologue.is_empty(), "{target:?}: {prologue:02x?}");
            assert!(epilogue.is_empty(), "{target:?}: {epilogue:02x?}");
        } else {
            assert_eq!(
                frame.red_zone_resident_bytes, 0,
                "{target:?}: no other admitted ABI exposes a red zone: {frame:?}"
            );
        }
        // Every symbolic frame access replays against the validated geometry
        // through the same equations the frame-address resolver applies;
        // resident slots surface as negative below-RSP displacements.
        let machine_plan = realization.machine().machine().plan();
        let mut rows = realization.encoding().rows().iter();
        let mut resident_accesses = 0usize;
        for function in &machine_plan.functions {
            let function_frame = layout
                .functions
                .iter()
                .find(|row| row.machine == function.machine)
                .unwrap();
            for block in &function.blocks {
                for instruction in &block.instructions {
                    let row = rows.next().unwrap();
                    assert_eq!(row.instruction, instruction.instruction, "{target:?}");
                    let Some(symbolic) = instruction.address else {
                        continue;
                    };
                    let resolved = row.address.unwrap_or_else(|| {
                        panic!("{target:?} unresolved address for {symbolic:?}")
                    });
                    assert_eq!(resolved.symbolic, symbolic, "{target:?}");
                    assert_eq!(
                        resolved.displacement,
                        expected_frame_displacement(function, function_frame, &symbolic),
                        "{target:?} {symbolic:?}"
                    );
                    if resolved.displacement < 0 {
                        resident_accesses += 1;
                    }
                }
            }
        }
        assert!(rows.next().is_none(), "{target:?} trailing encoding rows");
        if system_v {
            assert!(
                resident_accesses > 0,
                "{target:?}: resident spill storage must be accessed below RSP"
            );
        }
        // Residency is a property of the produced layout, never a claim a
        // candidate can assert: the produced plan replays exactly, while a
        // suppressed or invented resident extent fails on every target.
        let plan = layout.clone();
        let current = realization.allocation().current();
        let validate = |candidate: machine_code::TargetFrameLayoutPlan| {
            validate_target_frame_layout(
                realization.machine(),
                realization.requirements(),
                realization.storage(),
                current.register_environment(),
                candidate,
            )
        };
        assert!(validate(plan.clone()).is_ok(), "{target:?}");
        if system_v {
            let mut denied = plan.clone();
            denied.functions[0].red_zone_resident_bytes = 0;
            assert!(
                validate(denied).is_err(),
                "{target:?}: a committed claim on resident geometry is not canonical"
            );
        } else {
            let mut claimed = plan.clone();
            claimed.functions[0].red_zone_resident_bytes =
                claimed.functions[0].frame_size_bytes.max(8);
            assert!(
                validate(claimed).is_err(),
                "{target:?}: resident bytes cannot appear outside System V AMD64"
            );
        }
        // The same program still reaches ordinary callable publication on
        // every admitted target, resident frame included.
        let emitted = stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap_or_else(|error| panic!("{target:?} emission: {error:?}"));
        let framed = stage_function_fragment_frame_application(emitted)
            .unwrap_or_else(|error| panic!("{target:?} frame: {error:?}"));
        let text = stage_optimized_fixed_frame_text_section(framed)
            .unwrap_or_else(|error| panic!("{target:?} text: {error:?}"));
        let object = stage_optimized_relocation_free_object_container(text)
            .unwrap_or_else(|error| panic!("{target:?} object: {error:?}"));
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap_or_else(|error| panic!("{target:?} artifact: {error:?}"));
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact)
            .unwrap_or_else(|error| panic!("{target:?} callable: {error:?}"));
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&callable).unwrap(),
            callable.custody(),
            "{target:?}"
        );
        assert_eq!(callable.entry().parameters.len(), 2, "{target:?}");
        assert_eq!(callable.entry().returns.len(), 1, "{target:?}");
    }
}
