//! Fixed-frame function-relative realization custody mutation coverage.
//!
//! `machine_emission::stage_fixed_frame_function_relative_realization`
//! publishes one [`ValidatedFunctionRelativeOptimizationRealizationManifest`]
//! binding every retained phase identity and an in-memory custody receipt over
//! the retained allocation evidence, post-allocation machine, selected-form
//! encoding, baseline and optimized resolved layouts, canonical frame layout,
//! frame protocol, callee-saved requirements and storage, and the validated
//! whole-function exit contract. Independent replay
//! (`validate_fixed_frame_function_relative_realization`) replays allocation
//! and machine custody, re-validates encoding, layout, frame, and the exit
//! record, re-derives the manifest and receipt, and compares them against the
//! retained claims, so this family is exercised here — beneath the real
//! pipeline that produces the staged custody — instead of inside
//! `machine-emission` where no honest staged fixture exists.
//!
//! Every representable field of the manifest, the custody receipt, and the
//! retained exit contract is mutated independently. A field that still encodes
//! canonically under an honestly recomputed containing identity must be
//! rejected by replay; a field whose value is closed by the representation
//! (single-variant stage, scope, and unavailable markers) is mutated on the
//! wire and must fail decoding before any custody decision. Stale containing
//! identities fail the identity check inside canonical decoding or artifact
//! validation.

use machine_code::{
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, SelectedFunctionLayoutPolicy,
    TargetFrameLayoutIdentity, TargetFrameProtocolEncodingIdentity,
    WholeFunctionExitContractIdentity, WholeFunctionProcessExitEvidence,
    X86BranchRelaxationIdentity,
};
use machine_emission::{
    FixedFramePublicationCustodyFieldForTest, FunctionRelativeFrameDisposition,
    FunctionRelativeOptimizationRealizationError as RealizationError,
    FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationManifestDecodeError as ManifestDecodeError,
    StagedFixedFrameFunctionRelativeRealization, WholeFunctionEntryAssumption,
    WholeFunctionExitContract, WholeFunctionExitContractError, WholeFunctionExitLayoutCustody,
    WholeFunctionExitPolicy, WholeFunctionFrameDisposition, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence,
};
use optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};
use proof_admission::AdmissionProfile;
use register_model::{
    PhysicalRegisterModelIdentity, RegisterUnitId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    MachineEncodedTrapBehavior, PreAllocationMachineEffectIdentity, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ProofBundle,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};

/// One callee returning a constant and one entry caller invoking it: the
/// smallest module that still retains two exit-contract function rows and two
/// return rows, so every mutation below exercises real retained custody rather
/// than a fabricated record.
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
        erased_scalar_formals: Vec::new(),
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
            erased_scalar_formals: Vec::new(),
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
                    erased_arguments: Vec::new(),
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

/// Stage the calling module through the real optimized pipeline to the
/// fixed-frame function-relative realization for `target`. The returned staged
/// record replays cleanly before any mutation and retains the rows the
/// mutation matrix below expects.
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
        machine_emission::validate_fixed_frame_function_relative_realization(&staged),
        Ok(staged.custody().clone()),
        "{target:?}: honest staged realization must replay before mutation",
    );
    assert_eq!(
        staged.exit_contract().contract().functions.len(),
        2,
        "{target:?}: fixture must retain both function exit rows",
    );
    assert!(
        staged
            .exit_contract()
            .contract()
            .functions
            .iter()
            .all(|function| function.returns.len() == 1),
        "{target:?}: fixture must retain one return row per function",
    );
    assert!(
        matches!(
            staged.manifest().record().frame,
            FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 { .. }
        ),
        "{target:?}: fixture must carry the canonical fixed-frame disposition",
    );
    staged
}

/// Recompute the mutated manifest's containing identity, require the envelope
/// to stay canonical, then require independent replay to reject the
/// substitution. `staged` is restored afterward so later mutations stay
/// independent of this one.
fn assert_manifest_field(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
    original: &FunctionRelativeOptimizationRealizationManifest,
    field: &str,
    mutate: impl Fn(&mut FunctionRelativeOptimizationRealizationManifest),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&mutated.encode()),
        Ok(mutated.clone()),
        "reauthenticated manifest field {field} must remain a canonical envelope",
    );
    *staged.manifest_mut().record_mut() = mutated;
    assert_eq!(
        machine_emission::validate_fixed_frame_function_relative_realization(staged),
        Err(RealizationError::ReceiptMismatch),
        "independent replay must reject substituted manifest field {field}",
    );
    *staged.manifest_mut().record_mut() = original.clone();
}

/// Recompute the mutated exit contract's own identity and rebind the
/// containing manifest field and manifest identity, so the only remaining lie
/// is the substituted field itself; independent replay must still reject it at
/// the exit-record join. `staged` is restored afterward.
fn assert_contract_field(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
    original_contract: &WholeFunctionExitContract,
    original_manifest: &FunctionRelativeOptimizationRealizationManifest,
    field: &str,
    mutate: impl Fn(&mut WholeFunctionExitContract),
) {
    let mut mutated = original_contract.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    *staged.exit_contract_mut().contract_mut() = mutated;
    let exit_identity = staged.exit_contract().identity();
    let record = staged.manifest_mut().record_mut();
    record.whole_function_exit_contract = exit_identity;
    record.identity = record.recomputed_identity();
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&record.encode()),
        Ok(record.clone()),
        "manifest reauthenticated over contract field {field} must remain canonical",
    );
    assert_eq!(
        machine_emission::validate_fixed_frame_function_relative_realization(staged),
        Err(RealizationError::ExitContract(
            WholeFunctionExitContractError::ArtifactMismatch,
        )),
        "independent replay must reject substituted exit-contract field {field}",
    );
    *staged.exit_contract_mut().contract_mut() = original_contract.clone();
    *staged.manifest_mut().record_mut() = original_manifest.clone();
}

fn assert_manifest_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: ManifestDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&encoded),
        Err(expected),
    );
}

/// Encoded-manifest layout: an 8-byte magic, 4-byte version, and 32-byte
/// identity envelope, then a one-byte stage tag, `selections` (32),
/// `selected_lowering_selections` (32), the optional
/// `selected_lowering_completion` status (1, plus 32 when present), twelve
/// 32-byte identity fields through `resolved_layout`, the optional
/// `x86_branch_relaxation` status (1, plus 32 when present), the optional
/// `post_allocation_machine_optimization` status (1, plus 153 when present),
/// `whole_function_exit_contract` (32), `target` (1+1+8+8),
/// `layout_policy` (1), `scope` (1), `statistics` (48), the frame disposition
/// tag (1, plus 64 for the canonical fixed frame), and seven single-byte
/// unavailable markers.
struct ManifestWireOffsets {
    stage: usize,
    completion_status: usize,
    relaxation_status: usize,
    optimization_status: usize,
    architecture: usize,
    object_format: usize,
    layout_policy: usize,
    scope: usize,
    frame: usize,
    unavailable: [usize; 7],
}

fn manifest_wire_offsets(
    encoded: &[u8],
    record: &FunctionRelativeOptimizationRealizationManifest,
) -> ManifestWireOffsets {
    let stage = 8 + 4 + 32;
    let mut at = stage + 1 + 32 + 32;
    let completion_status = at;
    at += 1 + if record.selected_lowering_completion.is_some() {
        32
    } else {
        0
    };
    // allocation_recovery, post_allocation_machine_selections,
    // function_relative_layout_selections, pre_physical_manifest,
    // post_allocation_manifest, selected, pre_allocation_machine_effects,
    // post_allocation_machine, baseline_pre_layout, pre_layout,
    // baseline_resolved_layout, resolved_layout.
    at += 12 * 32;
    let relaxation_status = at;
    at += 1 + if record.x86_branch_relaxation.is_some() {
        32
    } else {
        0
    };
    let optimization_status = at;
    at += 1 + if record.post_allocation_machine_optimization.is_some() {
        1 + 32 + 32 + 32 + 32 + 8 + 8 + 8
    } else {
        0
    };
    at += 32; // whole_function_exit_contract
    let architecture = at;
    let object_format = at + 1;
    at += 2 + 8 + 8;
    let layout_policy = at;
    at += 1;
    let scope = at;
    at += 1;
    at += 48; // statistics
    let frame = at;
    at += 1 + if matches!(
        record.frame,
        FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 { .. }
    ) {
        64
    } else {
        0
    };
    let unavailable = [at, at + 1, at + 2, at + 3, at + 4, at + 5, at + 6];
    at += 7;
    assert_eq!(encoded.len(), at);
    ManifestWireOffsets {
        stage,
        completion_status,
        relaxation_status,
        optimization_status,
        architecture,
        object_format,
        layout_policy,
        scope,
        frame,
        unavailable,
    }
}

/// A representable post-allocation optimization custody claim for the
/// manifest's optional field: the optimization tag is chosen for the record's
/// own architecture so the substitution stays semantically shaped.
fn foreign_optimization_custody(
    record: &FunctionRelativeOptimizationRealizationManifest,
) -> PostAllocationMachineOptimizationCustody {
    let optimization = match record.target.architecture {
        Architecture::X86_64 => Optimization::X86SelectXorZeroI64MaterializationV1,
        Architecture::Aarch64 => Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
    };
    PostAllocationMachineOptimizationCustody::from_parts(
        optimization,
        [0xd2; 32],
        OptimizationSelectionIdentity::from_bytes([0xd3; 32]),
        OptimizationSelectionIdentity::from_bytes([0xd4; 32]),
        PostAllocationMachineIdentity::from_bytes([0xd5; 32]),
        1,
        64,
        63,
    )
}

#[test]
fn fixed_frame_realization_custody_rejects_every_one_field_substitution() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut staged = staged_realization(target);
        let other_architecture = match staged.manifest().record().target.architecture {
            Architecture::X86_64 => Architecture::Aarch64,
            Architecture::Aarch64 => Architecture::X86_64,
        };
        let other_format = match staged.manifest().record().target.object_format {
            ObjectFormat::Elf => ObjectFormat::MachO,
            ObjectFormat::MachO => ObjectFormat::Coff,
            ObjectFormat::Coff => ObjectFormat::Elf,
        };
        let other_policy = match staged.manifest().record().layout_policy {
            SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => {
                SelectedFunctionLayoutPolicy::SingleEntryBlockV1
            }
            _ => SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1,
        };
        // An authentic foreign realization on the opposite architecture
        // supplies the donor receipt evidence below.
        let foreign = staged_realization(match staged.manifest().record().target.architecture {
            Architecture::X86_64 => NativeTarget::linux_arm64(),
            Architecture::Aarch64 => NativeTarget::linux_x64(),
        });

        // Every representable manifest field mutates independently under an
        // honestly recomputed containing identity; replay rejects each one
        // against the re-derived expected manifest. `stage` and `scope` are
        // single-variant and each unavailable marker is closed; their closed
        // tags are covered by the wire legs below.
        let original_manifest = staged.manifest().record().clone();
        let manifest_mutations: [(
            &str,
            fn(&mut FunctionRelativeOptimizationRealizationManifest),
        ); 24] = [
            ("selections", |record| {
                record.selections = OptimizationSelectionIdentity::from_bytes([0xc1; 32])
            }),
            ("selected_lowering_selections", |record| {
                record.selected_lowering_selections =
                    OptimizationSelectionIdentity::from_bytes([0xc2; 32])
            }),
            ("allocation_recovery_selections", |record| {
                record.allocation_recovery_selections =
                    OptimizationSelectionIdentity::from_bytes([0xc3; 32])
            }),
            ("post_allocation_machine_selections", |record| {
                record.post_allocation_machine_selections =
                    OptimizationSelectionIdentity::from_bytes([0xc4; 32])
            }),
            ("function_relative_layout_selections", |record| {
                record.function_relative_layout_selections =
                    OptimizationSelectionIdentity::from_bytes([0xc5; 32])
            }),
            ("pre_physical_manifest", |record| {
                record.pre_physical_manifest =
                    PrePhysicalOptimizationManifestIdentity::from_bytes([0xc6; 32])
            }),
            ("post_allocation_manifest", |record| {
                record.post_allocation_manifest =
                    PostAllocationOptimizationManifestIdentity::from_bytes([0xc7; 32])
            }),
            ("selected", |record| {
                record.selected = SelectedInstructionPlanIdentity::from_bytes([0xc8; 32])
            }),
            ("pre_allocation_machine_effects", |record| {
                record.pre_allocation_machine_effects =
                    PreAllocationMachineEffectIdentity::from_bytes([0xc9; 32])
            }),
            ("post_allocation_machine", |record| {
                record.post_allocation_machine =
                    PostAllocationMachineIdentity::from_bytes([0xca; 32])
            }),
            ("baseline_pre_layout", |record| {
                record.baseline_pre_layout = SelectedFormEncodingIdentity::from_bytes([0xcb; 32])
            }),
            ("pre_layout", |record| {
                record.pre_layout = SelectedFormEncodingIdentity::from_bytes([0xcc; 32])
            }),
            ("baseline_resolved_layout", |record| {
                record.baseline_resolved_layout =
                    ResolvedSelectedFormLayoutIdentity::from_bytes([0xcd; 32])
            }),
            ("resolved_layout", |record| {
                record.resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes([0xce; 32])
            }),
            ("x86_branch_relaxation", |record| {
                record.x86_branch_relaxation =
                    Some(X86BranchRelaxationIdentity::from_bytes([0xcf; 32]))
            }),
            ("whole_function_exit_contract", |record| {
                record.whole_function_exit_contract =
                    WholeFunctionExitContractIdentity::from_bytes([0xd0; 32])
            }),
            ("target.pointer_size", |record| {
                record.target.pointer_size = 4
            }),
            ("target.pointer_alignment", |record| {
                record.target.pointer_alignment = 4
            }),
            ("statistics.functions", |record| {
                record.statistics.functions += 1
            }),
            ("statistics.blocks", |record| record.statistics.blocks += 1),
            ("statistics.instructions", |record| {
                record.statistics.instructions += 1
            }),
            ("statistics.bytes", |record| record.statistics.bytes += 1),
            ("statistics.resolved_conditional_branches", |record| {
                record.statistics.resolved_conditional_branches += 1
            }),
            ("statistics.unresolved_internal_machine_fixups", |record| {
                record.statistics.unresolved_internal_machine_fixups += 1
            }),
        ];
        for (field, mutate) in manifest_mutations {
            assert_manifest_field(&mut staged, &original_manifest, field, mutate);
        }
        // Optional and multi-variant fields whose substitute depends on the
        // fixture's own values.
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "selected_lowering_completion",
            |record| {
                record.selected_lowering_completion =
                    if record.selected_lowering_completion.is_some() {
                        None
                    } else {
                        Some(SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                            [0xd1; 32],
                        ))
                    }
            },
        );
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "post_allocation_machine_optimization",
            |record| {
                record.post_allocation_machine_optimization =
                    Some(foreign_optimization_custody(record))
            },
        );
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "target.architecture",
            |record| record.target.architecture = other_architecture,
        );
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "target.object_format",
            |record| record.target.object_format = other_format,
        );
        assert_manifest_field(&mut staged, &original_manifest, "layout_policy", |record| {
            record.layout_policy = other_policy
        });
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "frame.disposition",
            |record| record.frame = FunctionRelativeFrameDisposition::Unavailable,
        );
        assert_manifest_field(&mut staged, &original_manifest, "frame.layout", |record| {
            if let FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 { layout, .. } =
                &mut record.frame
            {
                *layout = TargetFrameLayoutIdentity::from_bytes([0xd6; 32]);
            }
        });
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "frame.protocol",
            |record| {
                if let FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
                    protocol, ..
                } = &mut record.frame
                {
                    *protocol = TargetFrameProtocolEncodingIdentity::from_bytes([0xd7; 32]);
                }
            },
        );

        // The containing identity itself is sealed over the content: a foreign
        // identity, or honest content drift carried under the stale identity,
        // fails the identity check inside canonical decoding and at replay.
        let mut stale = original_manifest.clone();
        stale.identity =
            optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                b"foreign realization manifest identity",
            );
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&stale.encode()),
            Err(ManifestDecodeError::IdentityMismatch),
            "{target:?}: foreign manifest identity must fail canonical decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            machine_emission::validate_fixed_frame_function_relative_realization(&staged),
            Err(RealizationError::ReceiptMismatch),
            "{target:?}: replay must reject a foreign manifest identity",
        );
        let mut stale = original_manifest.clone();
        stale.statistics.bytes += 1;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&stale.encode()),
            Err(ManifestDecodeError::IdentityMismatch),
            "{target:?}: drifted content under a stale identity must fail decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            machine_emission::validate_fixed_frame_function_relative_realization(&staged),
            Err(RealizationError::ReceiptMismatch),
            "{target:?}: replay must reject drifted manifest content under a stale identity",
        );
        *staged.manifest_mut().record_mut() = original_manifest.clone();

        // The custody receipt fields have no independent wire form; each is
        // substituted in place and replay rejects every one.
        for (field, which) in [
            ("source", FixedFramePublicationCustodyFieldForTest::Source),
            ("machine", FixedFramePublicationCustodyFieldForTest::Machine),
            (
                "requirements",
                FixedFramePublicationCustodyFieldForTest::Requirements,
            ),
            ("storage", FixedFramePublicationCustodyFieldForTest::Storage),
            ("frame", FixedFramePublicationCustodyFieldForTest::Frame),
            (
                "protocol",
                FixedFramePublicationCustodyFieldForTest::Protocol,
            ),
            (
                "exit_contract",
                FixedFramePublicationCustodyFieldForTest::ExitContract,
            ),
            (
                "realization",
                FixedFramePublicationCustodyFieldForTest::Realization,
            ),
        ] {
            let mut mutated = staged_realization(target);
            mutated.corrupt_publication_custody_for_test(which, &foreign);
            assert_eq!(
                machine_emission::validate_fixed_frame_function_relative_realization(&mutated),
                Err(RealizationError::ReceiptMismatch),
                "{target:?}: replay must reject a substituted custody receipt field {field}",
            );
        }

        // Retained-component substitutions: the replay must re-run the
        // component validations rather than trusting the claimed identities.
        let mut swapped = staged_realization(target);
        let mut foreign_swapped = staged_realization(match target.architecture {
            Architecture::X86_64 => NativeTarget::linux_arm64(),
            Architecture::Aarch64 => NativeTarget::linux_x64(),
        });
        machine_emission::swap_fixed_frame_realization_source_for_test(
            &mut swapped,
            &mut foreign_swapped,
        );
        assert!(
            machine_emission::validate_fixed_frame_function_relative_realization(&swapped).is_err(),
            "{target:?}: replay must reject a detached foreign allocation source",
        );

        let mut corrupt = staged_realization(target);
        machine_emission::corrupt_fixed_frame_realization_encoding_for_test(&mut corrupt);
        assert!(
            matches!(
                machine_emission::validate_fixed_frame_function_relative_realization(&corrupt),
                Err(RealizationError::Encoding(_)),
            ),
            "{target:?}: replay must reject a corrupted retained encoding",
        );

        let mut corrupt = staged_realization(target);
        machine_emission::corrupt_fixed_frame_realization_layout_for_test(&mut corrupt);
        assert!(
            matches!(
                machine_emission::validate_fixed_frame_function_relative_realization(&corrupt),
                Err(RealizationError::LayoutOptimization(_)),
            ),
            "{target:?}: replay must reject a corrupted retained baseline layout",
        );

        let mut detached = staged_realization(target);
        machine_emission::replace_fixed_frame_realization_exit_for_test(&mut detached, &foreign);
        assert_eq!(
            machine_emission::validate_fixed_frame_function_relative_realization(&detached),
            Err(RealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            )),
            "{target:?}: replay must reject a detached foreign exit contract",
        );

        // Every representable retained exit-contract field mutates
        // independently under an honestly recomputed contract identity and a
        // manifest rebind; replay rejects each one at the exit-record join.
        // `hardening` is single-variant and closed by the representation.
        let original_contract = staged.exit_contract().contract().clone();
        let contract_mutations: [(&str, fn(&mut WholeFunctionExitContract)); 26] = [
            ("selected", |contract| {
                contract.selected = SelectedInstructionPlanIdentity::from_bytes([0xe1; 32])
            }),
            ("post_allocation_manifest", |contract| {
                contract.post_allocation_manifest =
                    PostAllocationOptimizationManifestIdentity::from_bytes([0xe2; 32])
            }),
            ("post_allocation_machine", |contract| {
                contract.post_allocation_machine =
                    PostAllocationMachineIdentity::from_bytes([0xe3; 32])
            }),
            ("register_environment", |contract| {
                contract.register_environment =
                    TargetRegisterEnvironmentIdentity::from_bytes([0xe4; 32])
            }),
            ("physical_register_model", |contract| {
                contract.physical_register_model =
                    PhysicalRegisterModelIdentity::from_bytes([0xe5; 32])
            }),
            ("pre_layout", |contract| {
                contract.pre_layout = SelectedFormEncodingIdentity::from_bytes([0xe6; 32])
            }),
            ("resolved_layout", |contract| {
                contract.resolved_layout =
                    ResolvedSelectedFormLayoutIdentity::from_bytes([0xe7; 32])
            }),
            ("layout_custody", |contract| {
                contract.layout_custody = match contract.layout_custody {
                    WholeFunctionExitLayoutCustody::BaselineNearLayoutV1 => {
                        WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                            relaxation: X86BranchRelaxationIdentity::from_bytes([0xe8; 32]),
                        }
                    }
                    _ => WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
                }
            }),
            ("target.pointer_size", |contract| {
                contract.target.pointer_size = 4
            }),
            ("policy", |contract| {
                contract.policy = match contract.policy {
                    WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1 => {
                        WholeFunctionExitPolicy::MicrosoftX64CanonicalFixedFrameV1
                    }
                    _ => WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1,
                }
            }),
            ("frame.disposition", |contract| {
                contract.frame = WholeFunctionFrameDisposition::FramelessV1
            }),
            ("frame.layout", |contract| {
                if let WholeFunctionFrameDisposition::CanonicalFixedFrameV1 { layout, .. } =
                    &mut contract.frame
                {
                    *layout = TargetFrameLayoutIdentity::from_bytes([0xe9; 32]);
                }
            }),
            ("frame.protocol", |contract| {
                if let WholeFunctionFrameDisposition::CanonicalFixedFrameV1 { protocol, .. } =
                    &mut contract.frame
                {
                    *protocol = TargetFrameProtocolEncodingIdentity::from_bytes([0xea; 32]);
                }
            }),
            ("entry_assumption", |contract| {
                contract.entry_assumption = match contract.entry_assumption {
                    WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => {
                        WholeFunctionEntryAssumption::CallerLinkRegisterV1 {
                            link_register: RegisterViewId(u16::MAX),
                        }
                    }
                    WholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. } => {
                        WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
                    }
                }
            }),
            ("stack_pointer", |contract| {
                contract.stack_pointer = RegisterViewId(u16::MAX)
            }),
            ("stack_alignment", |contract| contract.stack_alignment += 1),
            ("red_zone_bytes", |contract| contract.red_zone_bytes += 1),
            ("result_view", |contract| {
                contract.result_view = RegisterViewId(u16::MAX - 1)
            }),
            ("callee_saved_units", |contract| {
                contract.callee_saved_units.push(RegisterUnitId(u16::MAX))
            }),
            ("functions.dropped", |contract| {
                contract.functions.pop();
            }),
            ("functions.duplicated", |contract| {
                let row = contract.functions[0].clone();
                contract.functions.push(row);
            }),
            ("functions[0].machine", |contract| {
                contract.functions[0].machine = contract.functions[1].machine
            }),
            ("functions[0].entry_block", |contract| {
                contract.functions[0].entry_block = SelectedBlockId(913)
            }),
            ("functions[0].body_stack_delta", |contract| {
                contract.functions[0].body_stack_delta += 8
            }),
            ("functions[0].modified_callee_saved_units", |contract| {
                contract.functions[0]
                    .modified_callee_saved_units
                    .push(RegisterUnitId(913))
            }),
            ("functions[0].process_exits.extended", |contract| {
                contract.functions[0]
                    .process_exits
                    .push(WholeFunctionProcessExitEvidence {
                        block: SelectedBlockId(913),
                        nominal_return_edge: EdgeId::new(914).unwrap(),
                        instruction: SelectedInstructionId(915),
                        offset: u64::MAX,
                        bytes: vec![0x00],
                    })
            }),
        ];
        for (field, mutate) in contract_mutations {
            assert_contract_field(
                &mut staged,
                &original_contract,
                &original_manifest,
                field,
                mutate,
            );
        }

        // The retained return rows join the layout: each representable row
        // field mutates independently under the recomputed contract identity.
        let return_mutations: [(&str, fn(&mut WholeFunctionExitContract)); 8] = [
            ("returns.dropped", |contract| {
                contract.functions[0].returns.pop();
            }),
            ("returns[0].block", |contract| {
                contract.functions[0].returns[0].block = SelectedBlockId(913)
            }),
            ("returns[0].psi_return_edge", |contract| {
                contract.functions[0].returns[0].psi_return_edge = EdgeId::new(913).unwrap()
            }),
            ("returns[0].instruction", |contract| {
                contract.functions[0].returns[0].instruction = SelectedInstructionId(913)
            }),
            ("returns[0].offset", |contract| {
                contract.functions[0].returns[0].offset += 1
            }),
            ("returns[0].bytes", |contract| {
                contract.functions[0].returns[0].bytes[0] ^= 1
            }),
            ("returns[0].trap", |contract| {
                let trap = &mut contract.functions[0].returns[0].trap;
                *trap = match trap {
                    MachineEncodedTrapBehavior::NeverV1 => {
                        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                    }
                    _ => MachineEncodedTrapBehavior::NeverV1,
                }
            }),
            ("returns[0].mechanism", |contract| {
                match &mut contract.functions[0].returns[0].mechanism {
                    WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                        pop_bytes, ..
                    } => *pop_bytes += 8,
                    WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                        link_register,
                        ..
                    } => *link_register = RegisterViewId(u16::MAX),
                }
            }),
        ];
        for (field, mutate) in return_mutations {
            assert_contract_field(
                &mut staged,
                &original_contract,
                &original_manifest,
                field,
                mutate,
            );
        }
        assert_contract_field(
            &mut staged,
            &original_contract,
            &original_manifest,
            "returns[0].value",
            |contract| {
                let value = &mut contract.functions[0].returns[0].value;
                *value = match value.clone() {
                    WholeFunctionReturnValueEvidence::ScalarV1 {
                        virtual_register,
                        units,
                        ..
                    } => WholeFunctionReturnValueEvidence::ScalarV1 {
                        virtual_register,
                        view: RegisterViewId(u16::MAX),
                        units,
                    },
                    _ => WholeFunctionReturnValueEvidence::UnitV1,
                };
            },
        );

        // A stale or foreign contract identity fails the sealed-identity check
        // inside exit-record validation, before the manifest join.
        let mut stale_contract = original_contract.clone();
        stale_contract.result_view = RegisterViewId(u16::MAX);
        *staged.exit_contract_mut().contract_mut() = stale_contract;
        assert_eq!(
            machine_emission::validate_fixed_frame_function_relative_realization(&staged),
            Err(RealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            )),
            "{target:?}: drifted contract content under a stale identity must reject",
        );
        let mut stale_contract = original_contract.clone();
        stale_contract.identity = WholeFunctionExitContractIdentity::from_bytes([0xee; 32]);
        *staged.exit_contract_mut().contract_mut() = stale_contract;
        assert_eq!(
            machine_emission::validate_fixed_frame_function_relative_realization(&staged),
            Err(RealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            )),
            "{target:?}: a foreign contract identity must reject",
        );
        *staged.exit_contract_mut().contract_mut() = original_contract.clone();
        *staged.manifest_mut().record_mut() = original_manifest.clone();

        // Closed axes and envelope corruption reject at canonical decoding,
        // before any custody decision.
        let encoded = original_manifest.encode();
        let offsets = manifest_wire_offsets(&encoded, &original_manifest);
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[0] ^= 1,
            ManifestDecodeError::WrongMagic,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[8..12].copy_from_slice(&17_u32.to_le_bytes()),
            ManifestDecodeError::UnsupportedVersion(17),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.stage] = 9,
            ManifestDecodeError::UnknownStage(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.completion_status] = 9,
            ManifestDecodeError::UnknownSelectedLoweringCompletionStatus(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.relaxation_status] = 9,
            ManifestDecodeError::UnknownX86BranchRelaxationStatus(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.optimization_status] = 9,
            ManifestDecodeError::UnknownPostAllocationMachineOptimizationStatus(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.architecture] = 9,
            ManifestDecodeError::UnknownArchitecture(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.object_format] = 9,
            ManifestDecodeError::UnknownObjectFormat(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.layout_policy] = 9,
            ManifestDecodeError::UnknownLayoutPolicy(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.scope] = 9,
            ManifestDecodeError::UnknownScope(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.frame] = 9,
            ManifestDecodeError::UnknownFrameDisposition(9),
        );
        for offset in offsets.unavailable {
            assert_manifest_decode_error(
                &encoded,
                |bytes| bytes[offset] = 9,
                ManifestDecodeError::UnknownUnavailableStatus(9),
            );
        }
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes.push(0),
            ManifestDecodeError::TrailingBytes,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes.truncate(encoded.len() - 1);
            },
            ManifestDecodeError::Truncated,
        );

        // The optional post-allocation custody has its own optimization tag;
        // and the two physical-transformation slots are mutually exclusive at
        // decode. `pointer_size`/`pointer_alignment` and the custody
        // `action_count` are open u64 fields decoded through `usize::try_from`,
        // which accepts every pattern on a 64-bit host, so
        // `TargetLayoutOverflow`/`ActionCountOverflow` are unreachable here
        // and those substitutions are sealed by the identity check instead.
        let mut optimized_record = original_manifest.clone();
        optimized_record.post_allocation_machine_optimization =
            Some(foreign_optimization_custody(&original_manifest));
        optimized_record.identity = optimized_record.recomputed_identity();
        let optimized_encoded = optimized_record.encode();
        let optimized_offsets = manifest_wire_offsets(&optimized_encoded, &optimized_record);
        assert_manifest_decode_error(
            &optimized_encoded,
            |bytes| bytes[optimized_offsets.optimization_status + 1] = 9,
            ManifestDecodeError::UnknownPostAllocationMachineOptimization(9),
        );

        let mut conflicted = original_manifest.clone();
        conflicted.x86_branch_relaxation =
            Some(X86BranchRelaxationIdentity::from_bytes([0xf1; 32]));
        conflicted.post_allocation_machine_optimization =
            Some(foreign_optimization_custody(&original_manifest));
        conflicted.identity = conflicted.recomputed_identity();
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&conflicted.encode()),
            Err(ManifestDecodeError::ConflictingPhysicalTransformations),
            "{target:?}: simultaneous physical transformations must fail decoding",
        );
    }
}
