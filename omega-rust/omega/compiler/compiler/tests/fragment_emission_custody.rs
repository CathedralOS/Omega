//! Function-fragment emission custody mutation coverage.
//!
//! `machine_emission::stage_optimized_function_fragment_emission` publishes one
//! [`FunctionFragmentEmissionPlan`] of function-relative bytes, block and
//! instruction spans, decoded branch evidence, unresolved internal-machine
//! fixups, and semantic control provenance, plus a
//! [`FunctionFragmentEmissionManifest`] binding every retained identity and an
//! in-memory custody receipt over the validated function-relative realization.
//! Independent replay (`validate_optimized_function_fragment_emission`)
//! re-validates the whole source pipeline, rejoins every claimed span against
//! the admitted selected program and resolved layout, rejoins every manifest
//! field, and re-derives the receipt, so this family is exercised here —
//! beneath the real pipeline that produces the staged custody — instead of
//! inside `machine-emission` where no honest staged fixture exists.
//!
//! Every representable field of the fragment plan, each function/block/
//! instruction row, the branch evidence, the internal fixup, the selected
//! provenance, the control provenance, the manifest, and the custody receipt
//! is mutated independently. A field that still encodes canonically under an
//! honestly recomputed containing identity must be rejected by replay; a field
//! whose value is closed by the representation (stage tag, vocabulary marker,
//! fuel-schedule marker, architecture and object-format tags, and the six
//! unavailable markers) is mutated on the wire and must fail decoding before
//! any custody decision. Stale containing identities fail the identity check
//! inside canonical decoding or artifact validation.

use machine_code::{
    FunctionFragmentBranchEvidence, FunctionFragmentConditionalBranchEvidence,
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionPlan,
    FunctionFragmentEmissionStage, FunctionFragmentInstructionSpan,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentJumpEvidence,
    FunctionFragmentSuccessorProvenance, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity,
};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    OptimizationSelections, PostAllocationOptimizationManifestIdentity,
};
use proof_admission::AdmissionProfile;
use selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, SelectedSuccessorRole,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, ScalarType, StructuralTypeId, ValueId,
};
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ProofBundle,
    SemanticFingerprint, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};

/// One callee returning a constant and one entry caller that invokes it, then
/// branches on the result and rejoins through a block parameter: the smallest
/// module that still retains two functions, one unresolved internal machine
/// fixup, one conditional branch evidence row, two jump evidence rows, and
/// non-empty successor bindings, so every mutation below exercises real
/// retained custody rather than a fabricated record.
fn branching_calling_module() -> TerminalModule {
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
    let block = |id: u64,
                 parameters: Vec<ValueDeclaration>,
                 operations: Vec<Operation>,
                 terminator: Terminator| Block {
        id: BlockId::new(id).unwrap(),
        parameters,
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator,
    };
    let machine = |id: u64,
                   parameters: Vec<ValueDeclaration>,
                   result: TerminalMachineResult,
                   entry: u64,
                   blocks: Vec<Block>| TerminalMachine {
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
        entry: BlockId::new(entry).unwrap(),
        blocks,
        contract: contract(id + 1000),
    };
    let operation = |id: u64, result: OperationResult, kind: OperationKind| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result,
        kind,
    };
    let successor = |edge: u64, target: u64| SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let callee = machine(
        2,
        vec![declaration(21)],
        TerminalMachineResult::Scalar(declaration(26)),
        23,
        vec![block(
            23,
            Vec::new(),
            vec![operation(
                24,
                OperationResult::Scalar(declaration(22)),
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(41),
                },
            )],
            Terminator::Return {
                edge: EdgeId::new(25).unwrap(),
                value: ValueId::new(22).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
    );
    let caller = machine(
        1,
        Vec::new(),
        TerminalMachineResult::Scalar(declaration(17)),
        11,
        vec![
            block(
                11,
                Vec::new(),
                vec![
                    operation(
                        12,
                        OperationResult::Scalar(declaration(14)),
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    ),
                    operation(
                        15,
                        OperationResult::Scalar(declaration(13)),
                        OperationKind::Call {
                            callee: MachineId::new(2).unwrap(),
                            arguments: vec![ValueId::new(14).unwrap()],
                            erased_arguments: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    ),
                    operation(
                        16,
                        OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: ValueId::new(18).unwrap(),
                            scalar_type: ScalarType::Boolean,
                        }),
                        OperationKind::IntegerLessOrEqual {
                            left: ValueId::new(14).unwrap(),
                            right: ValueId::new(13).unwrap(),
                        },
                    ),
                ],
                Terminator::Conditional {
                    condition: ValueId::new(18).unwrap(),
                    when_true: successor(31, 32),
                    when_false: successor(33, 34),
                },
            ),
            block(
                32,
                Vec::new(),
                Vec::new(),
                Terminator::Jump {
                    edge: EdgeId::new(35).unwrap(),
                    target: BlockId::new(36).unwrap(),
                    arguments: vec![ValueId::new(13).unwrap()],
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            ),
            block(
                34,
                Vec::new(),
                Vec::new(),
                Terminator::Jump {
                    edge: EdgeId::new(37).unwrap(),
                    target: BlockId::new(36).unwrap(),
                    arguments: vec![ValueId::new(13).unwrap()],
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            ),
            block(
                36,
                vec![declaration(41)],
                Vec::new(),
                Terminator::Return {
                    edge: EdgeId::new(38).unwrap(),
                    value: ValueId::new(41).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ),
        ],
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

/// Stage the branching module through the real optimized pipeline up to and
/// including function-fragment emission for `target`. The returned staged
/// emission replays cleanly before any mutation and retains the rows the
/// mutation matrix below expects.
fn staged_emission(
    target: NativeTarget,
) -> machine_emission::StagedOptimizedFunctionFragmentEmission {
    let module = branching_calling_module();
    let proof = ProofBundle::default();
    let semantic = terminal_codec::encode_module(&module).expect("encode branching module");
    let proof_bytes =
        terminal_codec::encode_proof_section(&module, &proof).expect("encode branching proof");
    let optimized = native_realization::optimize_artifact_sections(
        &semantic,
        &proof_bytes,
        &AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
    )
    .expect("branching module optimizes");
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .expect("branching module reaches physical staging");
    let staged = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("fragment emission");
    assert_eq!(
        machine_emission::validate_optimized_function_fragment_emission(&staged),
        Ok(staged.custody()),
        "{target:?}: honest staged fragment emission must replay before mutation",
    );
    staged
}

/// `(function, block, instruction)` coordinates of one retained span.
type SpanAt = (usize, usize, usize);

fn find_span(
    plan: &FunctionFragmentEmissionPlan,
    predicate: impl Fn(&FunctionFragmentInstructionSpan) -> bool,
) -> SpanAt {
    let predicate = &predicate;
    plan.functions
        .iter()
        .enumerate()
        .flat_map(|(f, function)| {
            function
                .blocks
                .iter()
                .enumerate()
                .flat_map(move |(b, block)| {
                    block
                        .instructions
                        .iter()
                        .enumerate()
                        .map(move |(i, span)| (f, b, i, predicate(span)))
                })
        })
        .find_map(|(f, b, i, matched)| matched.then_some((f, b, i)))
        .expect("fixture retains the requested span")
}

fn span_at(plan: &FunctionFragmentEmissionPlan, at: SpanAt) -> &FunctionFragmentInstructionSpan {
    &plan.functions[at.0].blocks[at.1].instructions[at.2]
}

fn span_mut<'a>(
    plan: &'a mut FunctionFragmentEmissionPlan,
    at: SpanAt,
) -> &'a mut FunctionFragmentInstructionSpan {
    &mut plan.functions[at.0].blocks[at.1].instructions[at.2]
}

/// Rows discovered on the authentic plan that the mutation matrix exercises.
/// Passed explicitly so the mutation tables below stay plain function
/// pointers.
#[derive(Clone, Copy)]
struct Rows {
    caller: usize,
    call: SpanAt,
    conditional: SpanAt,
    jump: SpanAt,
    ret: SpanAt,
    ordinary: SpanAt,
}

/// Recompute the mutated plan's containing identity, then require independent
/// replay to reject the substitution. `staged` is restored afterward so later
/// mutations stay independent of this one.
fn assert_plan_field(
    staged: &mut machine_emission::StagedOptimizedFunctionFragmentEmission,
    original: &FunctionFragmentEmissionPlan,
    target: NativeTarget,
    field: &str,
    mutate: impl Fn(&mut FunctionFragmentEmissionPlan),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    *staged.fragments_mut() = mutated;
    assert_eq!(
        machine_emission::validate_optimized_function_fragment_emission(staged),
        Err(machine_emission::FunctionFragmentEmissionError::ArtifactMismatch),
        "{target:?}: independent replay must reject substituted fragment-plan field {field}",
    );
    *staged.fragments_mut() = original.clone();
}

/// Recompute the mutated manifest's containing identity, require the envelope
/// to stay canonical, then require independent replay to reject the
/// substitution. `staged` is restored afterward so later mutations stay
/// independent of this one.
fn assert_manifest_field(
    staged: &mut machine_emission::StagedOptimizedFunctionFragmentEmission,
    original: &FunctionFragmentEmissionManifest,
    target: NativeTarget,
    field: &str,
    mutate: impl Fn(&mut FunctionFragmentEmissionManifest),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&mutated.encode()),
        Ok(mutated.clone()),
        "{target:?}: reauthenticated manifest field {field} must remain a canonical envelope",
    );
    *staged.manifest_record_mut() = mutated;
    assert_eq!(
        machine_emission::validate_optimized_function_fragment_emission(staged),
        Err(machine_emission::FunctionFragmentEmissionError::ManifestMismatch),
        "{target:?}: independent replay must reject substituted manifest field {field}",
    );
    *staged.manifest_record_mut() = original.clone();
}

fn assert_manifest_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: FunctionFragmentEmissionManifestDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&encoded),
        Err(expected),
    );
}

/// Encoded-manifest layout: an 8-byte magic, 4-byte version, and 32-byte
/// identity envelope, then a one-byte stage tag, `source_realization` (32),
/// `selections` (32), the `psi` vocabulary marker (2) and program fingerprint
/// (32), `fuel_schedule` (4), `selected` (32), `post_allocation_manifest` (32),
/// `post_allocation_machine` (32), `final_pre_layout` (32),
/// `final_resolved_layout` (32), `whole_function_exit_contract` (32),
/// `fragments` (32), `target` (1+1+8+8), `statistics` (64), and six
/// single-byte unavailable markers.
struct ManifestWireOffsets {
    stage: usize,
    vocabulary: usize,
    fuel_schedule: usize,
    architecture: usize,
    object_format: usize,
    pointer_size: usize,
    statistics: usize,
    unavailable: usize,
}

fn manifest_wire_offsets(encoded: &[u8]) -> ManifestWireOffsets {
    let stage = 8 + 4 + 32;
    let vocabulary = stage + 1 + 32 + 32;
    let fuel_schedule = vocabulary + 2 + 32;
    let architecture = fuel_schedule + 4 + 7 * 32;
    let object_format = architecture + 1;
    let pointer_size = object_format + 1;
    let statistics = pointer_size + 8 + 8;
    let unavailable = statistics + 8 * 8;
    assert_eq!(encoded.len(), unavailable + 6);
    ManifestWireOffsets {
        stage,
        vocabulary,
        fuel_schedule,
        architecture,
        object_format,
        pointer_size,
        statistics,
        unavailable,
    }
}

fn other_role(role: SelectedSuccessorRole) -> SelectedSuccessorRole {
    match role {
        SelectedSuccessorRole::Semantic => SelectedSuccessorRole::EdgeTransferContinuation,
        _ => SelectedSuccessorRole::Semantic,
    }
}

fn other_predicate(
    predicate: FunctionFragmentConditionalBranchPredicate,
) -> FunctionFragmentConditionalBranchPredicate {
    match predicate {
        FunctionFragmentConditionalBranchPredicate::NonZeroV1 => {
            FunctionFragmentConditionalBranchPredicate::U64LessThanV1
        }
        _ => FunctionFragmentConditionalBranchPredicate::NonZeroV1,
    }
}

#[test]
fn optimized_function_fragment_emission_custody_rejects_every_one_field_substitution() {
    use machine_code::FunctionFragmentControlProvenance as Control;
    use machine_emission::FunctionFragmentEmissionError as EmissionError;
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut staged = staged_emission(target);
        let original = staged.fragments().clone();
        let original_manifest = staged.manifest().record().clone();
        assert_eq!(
            original_manifest.stage,
            FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
            "{target:?}: fixture must retain the unresolved-fixup stage",
        );
        assert_eq!(original.functions.len(), 2, "{target:?}");
        assert_eq!(original.entry, MachineId::new(1).unwrap(), "{target:?}");
        let caller = original
            .functions
            .iter()
            .position(|function| function.machine == MachineId::new(1).unwrap())
            .expect("caller fragment");
        let callee = original
            .functions
            .iter()
            .position(|function| function.machine == MachineId::new(2).unwrap())
            .expect("callee fragment");
        let call = find_span(&original, |span| span.internal_machine_fixup.is_some());
        let conditional = find_span(&original, |span| {
            matches!(
                span.branch.as_deref(),
                Some(FunctionFragmentBranchEvidence::Conditional(_))
            )
        });
        let jump = find_span(&original, |span| {
            matches!(
                span.branch.as_deref(),
                Some(FunctionFragmentBranchEvidence::Jump(_))
            )
        });
        let bound_jump = find_span(&original, |span| match &span.control {
            Control::Jump { successor } => !successor.bindings.is_empty(),
            _ => false,
        });
        let ret = find_span(&original, |span| {
            matches!(span.control, Control::Return { .. })
        });
        let ordinary = find_span(&original, |span| matches!(span.control, Control::None));
        let rows = Rows {
            caller,
            call,
            conditional,
            jump,
            ret,
            ordinary,
        };
        assert_eq!(
            original_manifest
                .statistics
                .unresolved_internal_machine_fixups,
            1,
            "{target:?}: fixture must retain exactly one unresolved internal fixup",
        );
        assert_eq!(
            original_manifest.statistics.resolved_conditional_branches, 1,
            "{target:?}: fixture must retain one resolved conditional branch",
        );
        let call_fixup = span_at(&original, call).internal_machine_fixup.unwrap();
        let other_fixup_kind = match call_fixup.kind {
            FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => {
                FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1
            }
            FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => {
                FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1
            }
        };
        let jump_evidence = match span_at(&original, jump).branch.as_deref() {
            Some(FunctionFragmentBranchEvidence::Jump(evidence)) => evidence.clone(),
            _ => unreachable!("jump span retains jump evidence"),
        };
        let conditional_evidence = match span_at(&original, conditional).branch.as_deref() {
            Some(FunctionFragmentBranchEvidence::Conditional(evidence)) => evidence.clone(),
            _ => unreachable!("conditional span retains conditional evidence"),
        };
        // A decoded register view from the retained encoding rows is an
        // authentic value for `decoded_register_reads` on targets whose
        // conditional branch consumes flags rather than a general register.
        let view_donor = staged
            .source()
            .encoding()
            .rows
            .iter()
            .find_map(|row| match &row.state {
                machine_code::SelectedFormEncodingState::Encoded { footprint, .. }
                | machine_code::SelectedFormEncodingState::UnresolvedInternalMachineCall {
                    footprint,
                    ..
                }
                | machine_code::SelectedFormEncodingState::UnresolvedNormalizedForeignCall {
                    footprint,
                    ..
                } => footprint
                    .register_reads
                    .first()
                    .or(footprint.register_writes.first())
                    .copied(),
                machine_code::SelectedFormEncodingState::DeferredControl { .. } => None,
            })
            .expect("fixture retains one decoded register view donor");
        let binding_donor = match &span_at(&original, bound_jump).control {
            Control::Jump { successor } => successor.bindings[0],
            _ => unreachable!("bound jump span retains jump control"),
        };
        let fuel_donor = original
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find_map(|span| match &span.control {
                Control::Jump { successor } => successor.fuel.first().cloned(),
                Control::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } => when_taken
                    .fuel
                    .first()
                    .or(when_fallthrough.fuel.first())
                    .cloned(),
                _ => None,
            })
            .expect("fixture retains one successor fuel settlement donor");
        let other_architecture = match original.target.architecture {
            Architecture::X86_64 => Architecture::Aarch64,
            Architecture::Aarch64 => Architecture::X86_64,
        };
        let other_format = match original.target.object_format {
            ObjectFormat::Elf => ObjectFormat::MachO,
            ObjectFormat::MachO => ObjectFormat::Coff,
            ObjectFormat::Coff => ObjectFormat::Elf,
        };
        let other_family = match span_at(&original, call).alternative.family {
            MachineAlternativeFamily::ReturnAggregate => MachineAlternativeFamily::CopyBytes,
            _ => MachineAlternativeFamily::ReturnAggregate,
        };
        // A post-allocation machine identity from the opposite architecture is
        // an authentic foreign value for the manifest binding below; the field
        // is `pub`/`Copy` so naming its representation type is unnecessary.
        let foreign_post_allocation_machine = staged_emission(match original.target.architecture {
            Architecture::X86_64 => NativeTarget::linux_arm64(),
            Architecture::Aarch64 => NativeTarget::linux_x64(),
        })
        .manifest()
        .record()
        .post_allocation_machine;

        // Every representable plan field mutates independently: the recomputed
        // identity keeps the artifact honest, and replay rejects the
        // substitution against the admitted selected program and resolved
        // layout. The `psi` vocabulary marker has no second representable
        // value; its closed tag is covered by the manifest wire legs below.
        let plan_mutations: [(&str, fn(&mut FunctionFragmentEmissionPlan)); 6] = [
            ("psi.program_fingerprint", |plan| {
                plan.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xb1; 32])
            }),
            ("fuel_schedule", |plan| {
                plan.fuel_schedule =
                    FuelScheduleIdentity::new(plan.fuel_schedule.marker() + 1).unwrap()
            }),
            ("selected", |plan| {
                plan.selected = SelectedInstructionPlanIdentity::from_bytes([0xb2; 32])
            }),
            ("entry", |plan| plan.entry = MachineId::new(913).unwrap()),
            ("target.pointer_size", |plan| plan.target.pointer_size = 4),
            ("target.pointer_alignment", |plan| {
                plan.target.pointer_alignment = 4
            }),
        ];
        for (field, mutate) in plan_mutations {
            assert_plan_field(&mut staged, &original, target, field, mutate);
        }
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "target.architecture",
            |plan| plan.target.architecture = other_architecture,
        );
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "target.object_format",
            |plan| plan.target.object_format = other_format,
        );

        // The fragment-function roster: cardinality, order, and each row field
        // join the selected functions and the resolved layout.
        let function_mutations: [(&str, fn(&mut FunctionFragmentEmissionPlan, usize, usize)); 10] = [
            ("functions.dropped", |plan, _, _| {
                plan.functions.pop();
            }),
            ("functions.duplicated", |plan, _, _| {
                let row = plan.functions[0].clone();
                plan.functions.push(row);
            }),
            ("functions.swapped", |plan, _, _| plan.functions.swap(0, 1)),
            ("functions[caller].machine", |plan, caller, callee| {
                plan.functions[caller].machine = plan.functions[callee].machine
            }),
            ("functions[caller].attachment", |plan, caller, _| {
                plan.functions[caller].attachment = Some(StructuralTypeId::new(913).unwrap())
            }),
            (
                "functions[caller].provenance.operations",
                |plan, caller, _| {
                    plan.functions[caller]
                        .provenance
                        .operations
                        .push(OperationId::new(913).unwrap())
                },
            ),
            ("functions[caller].provenance.edges", |plan, caller, _| {
                plan.functions[caller]
                    .provenance
                    .edges
                    .push(EdgeId::new(913).unwrap())
            }),
            ("functions[caller].byte_count", |plan, caller, _| {
                plan.functions[caller].byte_count += 1
            }),
            ("functions[caller].bytes", |plan, caller, _| {
                plan.functions[caller].bytes[0] ^= 1
            }),
            ("functions[caller].bytes.dropped", |plan, caller, _| {
                plan.functions[caller].bytes.pop();
            }),
        ];
        for (field, mutate) in function_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                mutate(plan, caller, callee)
            });
        }

        // The block roster joins the resolved layout row-for-row under
        // function-relative coordinates; the instruction roster joins the
        // selected block's instruction and terminator rows.
        let block_mutations: [(&str, fn(&mut FunctionFragmentEmissionPlan, usize)); 6] = [
            ("blocks.dropped", |plan, caller| {
                plan.functions[caller].blocks.pop();
            }),
            ("blocks.duplicated", |plan, caller| {
                let row = plan.functions[caller].blocks[0].clone();
                plan.functions[caller].blocks.push(row);
            }),
            ("blocks.swapped", |plan, caller| {
                plan.functions[caller].blocks.swap(0, 1)
            }),
            ("blocks[0].block", |plan, caller| {
                plan.functions[caller].blocks[0].block = SelectedBlockId(913)
            }),
            ("blocks[0].offset", |plan, caller| {
                plan.functions[caller].blocks[0].offset += 1
            }),
            ("blocks[0].byte_count", |plan, caller| {
                plan.functions[caller].blocks[0].byte_count += 1
            }),
        ];
        for (field, mutate) in block_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                mutate(plan, caller)
            });
        }

        let span_mutations: [(&str, fn(&mut FunctionFragmentEmissionPlan, Rows)); 10] = [
            ("instructions.dropped", |plan, rows| {
                plan.functions[rows.caller].blocks[0].instructions.pop();
            }),
            ("instructions.duplicated", |plan, rows| {
                let row = plan.functions[rows.caller].blocks[0].instructions[0].clone();
                plan.functions[rows.caller].blocks[0].instructions.push(row);
            }),
            ("instructions.swapped", |plan, rows| {
                plan.functions[rows.caller].blocks[0]
                    .instructions
                    .swap(0, 1);
            }),
            ("span.instruction", |plan, rows| {
                span_mut(plan, rows.call).instruction = SelectedInstructionId(913)
            }),
            ("span.offset", |plan, rows| {
                span_mut(plan, rows.call).offset += 1
            }),
            ("span.bytes.flip", |plan, rows| {
                span_mut(plan, rows.call).bytes[0] ^= 1
            }),
            ("span.bytes.dropped", |plan, rows| {
                span_mut(plan, rows.call).bytes.pop();
            }),
            ("span.alternative.variant", |plan, rows| {
                span_mut(plan, rows.call).alternative.variant += 1
            }),
            ("span.provenance.obligations", |plan, rows| {
                span_mut(plan, rows.call)
                    .provenance
                    .obligations
                    .push(ObligationId::new(913).unwrap())
            }),
            ("span.provenance.edges", |plan, rows| {
                span_mut(plan, rows.call)
                    .provenance
                    .edges
                    .push(EdgeId::new(913).unwrap())
            }),
        ];
        for (field, mutate) in span_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                mutate(plan, rows)
            });
        }
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "span.alternative.family",
            |plan| span_mut(plan, call).alternative.family = other_family,
        );

        // The selected-instruction provenance row joins the selected
        // instruction's provenance fields exactly.
        let provenance_mutations: [(&str, fn(&mut FunctionFragmentEmissionPlan, Rows)); 3] = [
            ("span.provenance.operations", |plan, rows| {
                span_mut(plan, rows.call)
                    .provenance
                    .operations
                    .push(OperationId::new(913).unwrap())
            }),
            ("span.provenance.values", |plan, rows| {
                span_mut(plan, rows.call)
                    .provenance
                    .values
                    .push(ValueId::new(913).unwrap())
            }),
            ("span.provenance.fuel", |plan, rows| {
                let donor = plan
                    .functions
                    .iter()
                    .flat_map(|function| &function.blocks)
                    .flat_map(|block| &block.instructions)
                    .find_map(|row| row.provenance.fuel.first().cloned());
                let span = span_mut(plan, rows.call);
                if let Some(settlement) = span.provenance.fuel.first_mut() {
                    settlement.units += 1;
                } else {
                    span.provenance
                        .fuel
                        .push(donor.expect("fixture retains one fuel settlement donor"));
                }
            }),
        ];
        for (field, mutate) in provenance_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                mutate(plan, rows)
            });
        }

        // Decoded branch evidence joins the resolved layout's conditional and
        // jump rows field-for-field.
        let conditional_mutations: [(&str, fn(&mut FunctionFragmentConditionalBranchEvidence));
            11] = [
            ("branch.predicate", |evidence| {
                evidence.predicate = other_predicate(evidence.predicate)
            }),
            ("branch.source_block", |evidence| {
                evidence.source_block = SelectedBlockId(913)
            }),
            ("branch.when_taken_edge", |evidence| {
                evidence.when_taken_edge = EdgeId::new(913).unwrap()
            }),
            ("branch.when_taken_block", |evidence| {
                evidence.when_taken_block = SelectedBlockId(913)
            }),
            ("branch.when_taken_offset", |evidence| {
                evidence.when_taken_offset += 1
            }),
            ("branch.when_fallthrough_edge", |evidence| {
                evidence.when_fallthrough_edge = EdgeId::new(914).unwrap()
            }),
            ("branch.when_fallthrough_block", |evidence| {
                evidence.when_fallthrough_block = SelectedBlockId(914)
            }),
            ("branch.when_fallthrough_offset", |evidence| {
                evidence.when_fallthrough_offset += 1
            }),
            ("branch.byte_displacement", |evidence| {
                evidence.byte_displacement += 1
            }),
            (
                "branch.decoded_effects.external_operand_reads",
                |evidence| evidence.decoded_effects.external_operand_reads.push(7),
            ),
            ("branch.decoded_effects.control", |evidence| {
                evidence.decoded_effects.control = match evidence.decoded_effects.control {
                    MachineEncodedControlEffect::FallThroughV1 => {
                        MachineEncodedControlEffect::ConditionalRelativeBranchV1
                    }
                    _ => MachineEncodedControlEffect::FallThroughV1,
                }
            }),
        ];
        for (field, mutate) in conditional_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                let Some(FunctionFragmentBranchEvidence::Conditional(evidence)) =
                    span_mut(plan, conditional).branch.as_deref_mut()
                else {
                    unreachable!("conditional span retains conditional evidence")
                };
                mutate(evidence);
            });
        }
        // The conditional's decoded register reads: poke one retained view, or
        // push an authentic view from the retained encoding rows on targets
        // whose branch consumes flags rather than a general register.
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "branch.decoded_register_reads",
            |plan| {
                let Some(FunctionFragmentBranchEvidence::Conditional(evidence)) =
                    span_mut(plan, conditional).branch.as_deref_mut()
                else {
                    unreachable!("conditional span retains conditional evidence")
                };
                if let Some(read) = evidence.decoded_register_reads.first_mut() {
                    read.0 ^= 1;
                } else {
                    evidence.decoded_register_reads.push(view_donor);
                }
            },
        );
        let jump_mutations: [(&str, fn(&mut FunctionFragmentJumpEvidence)); 6] = [
            ("branch.source_block", |evidence| {
                evidence.source_block = SelectedBlockId(915)
            }),
            ("branch.target_edge", |evidence| {
                evidence.target_edge = EdgeId::new(915).unwrap()
            }),
            ("branch.target_block", |evidence| {
                evidence.target_block = SelectedBlockId(915)
            }),
            ("branch.target_offset", |evidence| {
                evidence.target_offset += 1
            }),
            ("branch.byte_displacement", |evidence| {
                evidence.byte_displacement += 1
            }),
            (
                "branch.decoded_effects.external_operand_writes",
                |evidence| evidence.decoded_effects.external_operand_writes.push(7),
            ),
        ];
        for (field, mutate) in jump_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                let Some(FunctionFragmentBranchEvidence::Jump(evidence)) =
                    span_mut(plan, jump).branch.as_deref_mut()
                else {
                    unreachable!("jump span retains jump evidence")
                };
                mutate(evidence);
            });
        }
        // Evidence variants and presence: a conditional row replaced by jump
        // evidence, a jump row replaced by conditional evidence, an absent row
        // on the conditional span, and a fabricated row on the call span all
        // fail the variant and presence join.
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "branch[conditional].jump_substituted",
            |plan| {
                span_mut(plan, conditional).branch = Some(Box::new(
                    FunctionFragmentBranchEvidence::Jump(jump_evidence.clone()),
                ))
            },
        );
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "branch[jump].conditional_substituted",
            |plan| {
                span_mut(plan, jump).branch = Some(Box::new(
                    FunctionFragmentBranchEvidence::Conditional(conditional_evidence.clone()),
                ))
            },
        );
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "branch[conditional].dropped",
            |plan| span_mut(plan, conditional).branch = None,
        );
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "branch[call].fabricated",
            |plan| {
                span_mut(plan, call).branch = Some(Box::new(FunctionFragmentBranchEvidence::Jump(
                    jump_evidence.clone(),
                )))
            },
        );

        // The unresolved internal-machine fixup joins the decoded call row:
        // every retained coordinate and the kind pairing are checked against
        // the resolved source fixup. `state` is single-variant, so no second
        // representable in-memory value exists for it.
        let fixup_mutations: [(&str, fn(&mut FunctionFragmentEmissionPlan, Rows)); 7] = [
            ("fixup.callee", |plan, rows| {
                span_mut(plan, rows.call)
                    .internal_machine_fixup
                    .as_mut()
                    .unwrap()
                    .callee = MachineId::new(913).unwrap()
            }),
            ("fixup.opcode_function_offset", |plan, rows| {
                span_mut(plan, rows.call)
                    .internal_machine_fixup
                    .as_mut()
                    .unwrap()
                    .opcode_function_offset += 1
            }),
            ("fixup.patch_function_offset", |plan, rows| {
                span_mut(plan, rows.call)
                    .internal_machine_fixup
                    .as_mut()
                    .unwrap()
                    .patch_function_offset += 1
            }),
            ("fixup.reference_function_offset", |plan, rows| {
                span_mut(plan, rows.call)
                    .internal_machine_fixup
                    .as_mut()
                    .unwrap()
                    .reference_function_offset += 1
            }),
            ("fixup.patch_byte_width", |plan, rows| {
                span_mut(plan, rows.call)
                    .internal_machine_fixup
                    .as_mut()
                    .unwrap()
                    .patch_byte_width += 1
            }),
            ("fixup.addend", |plan, rows| {
                span_mut(plan, rows.call)
                    .internal_machine_fixup
                    .as_mut()
                    .unwrap()
                    .addend += 1
            }),
            ("fixup.dropped", |plan, rows| {
                span_mut(plan, rows.call).internal_machine_fixup = None
            }),
        ];
        for (field, mutate) in fixup_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                mutate(plan, rows)
            });
        }
        assert_plan_field(&mut staged, &original, target, "fixup.kind", |plan| {
            span_mut(plan, call)
                .internal_machine_fixup
                .as_mut()
                .unwrap()
                .kind = other_fixup_kind
        });
        assert_plan_field(&mut staged, &original, target, "fixup.fabricated", |plan| {
            span_mut(plan, ordinary).internal_machine_fixup = Some(call_fixup)
        });

        // Semantic control provenance joins the selected block's terminator
        // and successor rows field-for-field.
        let control_mutations: [(&str, fn(&mut FunctionFragmentEmissionPlan, Rows)); 8] = [
            ("control[call].callee", |plan, rows| {
                span_mut(plan, rows.call).control = Control::DirectInternalCall {
                    callee: MachineId::new(913).unwrap(),
                }
            }),
            ("control[call].dropped", |plan, rows| {
                span_mut(plan, rows.call).control = Control::None
            }),
            ("control[return].psi_return_edge", |plan, rows| {
                let Control::Return { psi_return_edge } = &mut span_mut(plan, rows.ret).control
                else {
                    unreachable!("return span retains return control")
                };
                *psi_return_edge = EdgeId::new(916).unwrap();
            }),
            ("control[return].hosted_exit_substituted", |plan, rows| {
                span_mut(plan, rows.ret).control = Control::HostedExitProcess {
                    nominal_return_edge: EdgeId::new(916).unwrap(),
                }
            }),
            ("control[ordinary].call_substituted", |plan, rows| {
                span_mut(plan, rows.ordinary).control = Control::DirectInternalCall {
                    callee: MachineId::new(913).unwrap(),
                }
            }),
            ("control[ordinary].hosted_exit_substituted", |plan, rows| {
                span_mut(plan, rows.ordinary).control = Control::HostedExitProcess {
                    nominal_return_edge: EdgeId::new(916).unwrap(),
                }
            }),
            ("control[conditional].dropped", |plan, rows| {
                span_mut(plan, rows.conditional).control = Control::None
            }),
            ("control[jump].dropped", |plan, rows| {
                span_mut(plan, rows.jump).control = Control::None
            }),
        ];
        for (field, mutate) in control_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                mutate(plan, rows)
            });
        }
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "control[conditional].predicate",
            |plan| {
                let Control::ConditionalBranch { predicate, .. } =
                    &mut span_mut(plan, conditional).control
                else {
                    unreachable!("conditional span retains conditional control")
                };
                *predicate = other_predicate(*predicate);
            },
        );

        // Every successor-provenance field of the conditional's taken edge and
        // of the bound jump edge mutates independently.
        let successor_mutations: [(&str, fn(&mut FunctionFragmentSuccessorProvenance)); 4] = [
            ("role", |successor| {
                successor.role = other_role(successor.role)
            }),
            ("psi_edge", |successor| {
                successor.psi_edge = EdgeId::new(917).unwrap()
            }),
            ("block", |successor| successor.block = SelectedBlockId(917)),
            ("source_target", |successor| {
                successor.source_target = BlockId::new(917).unwrap()
            }),
        ];
        for (field, mutate) in successor_mutations {
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                let Control::ConditionalBranch { when_taken, .. } =
                    &mut span_mut(plan, conditional).control
                else {
                    unreachable!("conditional span retains conditional control")
                };
                mutate(when_taken);
            });
            assert_plan_field(&mut staged, &original, target, field, |plan| {
                let Control::Jump { successor } = &mut span_mut(plan, bound_jump).control else {
                    unreachable!("jump span retains jump control")
                };
                mutate(successor);
            });
        }
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "successor[jump].bindings.parameter",
            |plan| {
                let Control::Jump { successor } = &mut span_mut(plan, bound_jump).control else {
                    unreachable!("jump span retains jump control")
                };
                successor.bindings[0].parameter = ValueId::new(917).unwrap();
            },
        );
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "successor[jump].bindings.duplicated",
            |plan| {
                let Control::Jump { successor } = &mut span_mut(plan, bound_jump).control else {
                    unreachable!("jump span retains jump control")
                };
                let binding = successor.bindings[0];
                successor.bindings.push(binding);
            },
        );
        // A conditional successor rebound onto the bound jump row's bindings
        // is representable content the artifact never produced.
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "successor[conditional].bindings.fabricated",
            |plan| {
                let Control::ConditionalBranch { when_taken, .. } =
                    &mut span_mut(plan, conditional).control
                else {
                    unreachable!("conditional span retains conditional control")
                };
                when_taken.bindings.push(binding_donor);
            },
        );
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "successor[conditional].fuel",
            |plan| {
                let Control::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } = &mut span_mut(plan, conditional).control
                else {
                    unreachable!("conditional span retains conditional control")
                };
                if let Some(settlement) = when_taken.fuel.first_mut() {
                    settlement.units += 1;
                } else if let Some(settlement) = when_fallthrough.fuel.first_mut() {
                    settlement.units += 1;
                } else {
                    when_taken.fuel.push(fuel_donor);
                }
            },
        );
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "successor[jump].fuel",
            |plan| {
                let Control::Jump { successor } = &mut span_mut(plan, bound_jump).control else {
                    unreachable!("jump span retains jump control")
                };
                if let Some(settlement) = successor.fuel.first_mut() {
                    settlement.units += 1;
                } else {
                    successor.fuel.push(fuel_donor);
                }
            },
        );
        // The fallthrough successor is a separate representable field of the
        // conditional control row.
        assert_plan_field(
            &mut staged,
            &original,
            target,
            "control[conditional].when_fallthrough.psi_edge",
            |plan| {
                let Control::ConditionalBranch {
                    when_fallthrough, ..
                } = &mut span_mut(plan, conditional).control
                else {
                    unreachable!("conditional span retains conditional control")
                };
                when_fallthrough.psi_edge = EdgeId::new(918).unwrap();
            },
        );

        // A foreign plan identity fails the artifact identity check before any
        // field comparison.
        *staged.fragments_mut() = original.clone();
        staged.fragments_mut().identity =
            FunctionFragmentEmissionIdentity::from_canonical_bytes(b"foreign fragment identity");
        assert_eq!(
            machine_emission::validate_optimized_function_fragment_emission(&staged),
            Err(EmissionError::ArtifactMismatch),
            "{target:?}: replay must reject a foreign fragment identity",
        );
        *staged.fragments_mut() = original.clone();

        // Every representable manifest field mutates independently under an
        // honestly recomputed containing identity; replay rejects each one
        // against the admitted source manifest and the checked fragments. The
        // six unavailable-data markers are single-variant in memory — no second
        // representable value exists — so they are covered by the wire legs.
        let manifest_mutations: [(&str, fn(&mut FunctionFragmentEmissionManifest)); 21] = [
            ("stage", |record| {
                record.stage = match record.stage {
                    FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1 => {
                        FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1
                    }
                    FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1 => {
                        FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
                    }
                }
            }),
            ("source_realization", |record| {
                record.source_realization =
                    FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                        b"substituted realization",
                    )
            }),
            ("selections", |record| {
                record.selections = OptimizationSelectionIdentity::from_bytes([0xc2; 32])
            }),
            ("psi.program_fingerprint", |record| {
                record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xc3; 32])
            }),
            ("fuel_schedule", |record| {
                record.fuel_schedule =
                    FuelScheduleIdentity::new(record.fuel_schedule.marker() + 1).unwrap()
            }),
            ("selected", |record| {
                record.selected = SelectedInstructionPlanIdentity::from_bytes([0xc4; 32])
            }),
            ("post_allocation_manifest", |record| {
                record.post_allocation_manifest =
                    PostAllocationOptimizationManifestIdentity::from_canonical_bytes(
                        b"substituted post-allocation manifest",
                    )
            }),
            ("final_pre_layout", |record| {
                record.final_pre_layout = SelectedFormEncodingIdentity::from_bytes([0xc6; 32])
            }),
            ("final_resolved_layout", |record| {
                record.final_resolved_layout =
                    ResolvedSelectedFormLayoutIdentity::from_bytes([0xc7; 32])
            }),
            ("whole_function_exit_contract", |record| {
                record.whole_function_exit_contract =
                    WholeFunctionExitContractIdentity::from_bytes([0xc8; 32])
            }),
            ("fragments", |record| {
                record.fragments =
                    FunctionFragmentEmissionIdentity::from_canonical_bytes(b"substituted fragments")
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
            ("statistics.instruction_spans", |record| {
                record.statistics.instruction_spans += 1
            }),
            ("statistics.zero_byte_instruction_spans", |record| {
                record.statistics.zero_byte_instruction_spans += 1
            }),
            ("statistics.bytes", |record| record.statistics.bytes += 1),
            ("statistics.resolved_conditional_branches", |record| {
                record.statistics.resolved_conditional_branches += 1
            }),
            ("statistics.logical_fuel_settlements", |record| {
                record.statistics.logical_fuel_settlements += 1
            }),
            ("statistics.unresolved_internal_machine_fixups", |record| {
                record.statistics.unresolved_internal_machine_fixups += 1
            }),
        ];
        for (field, mutate) in manifest_mutations {
            assert_manifest_field(&mut staged, &original_manifest, target, field, mutate);
        }
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            target,
            "post_allocation_machine",
            |record| record.post_allocation_machine = foreign_post_allocation_machine,
        );
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            target,
            "target.architecture",
            |record| record.target.architecture = other_architecture,
        );
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            target,
            "target.object_format",
            |record| record.target.object_format = other_format,
        );

        // The containing identity itself is sealed over the content: a foreign
        // identity, or honest content drift carried under the stale identity,
        // fails the identity check inside canonical decoding and at replay.
        let mut stale = original_manifest.clone();
        stale.identity = FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(
            b"foreign manifest identity",
        );
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&stale.encode()),
            Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch),
            "{target:?}: foreign manifest identity must fail canonical decoding",
        );
        *staged.manifest_record_mut() = stale;
        assert_eq!(
            machine_emission::validate_optimized_function_fragment_emission(&staged),
            Err(EmissionError::ManifestMismatch),
            "{target:?}: replay must reject a foreign manifest identity",
        );
        let mut stale = original_manifest.clone();
        stale.statistics.bytes += 1;
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&stale.encode()),
            Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch),
            "{target:?}: drifted content under a stale identity must fail decoding",
        );
        *staged.manifest_record_mut() = stale;
        assert_eq!(
            machine_emission::validate_optimized_function_fragment_emission(&staged),
            Err(EmissionError::ManifestMismatch),
            "{target:?}: replay must reject drifted manifest content under a stale identity",
        );
        *staged.manifest_record_mut() = original_manifest.clone();

        // The custody receipt fields have no independent wire form; each is
        // substituted in place and replay rejects every one.
        let receipt_mutations: [(
            &str,
            fn(&mut machine_emission::StagedOptimizedFunctionFragmentEmission),
        ); 3] = [
            (
                "source_realization",
                machine_emission::StagedOptimizedFunctionFragmentEmission::corrupt_custody_source_realization_for_test,
            ),
            (
                "fragments",
                machine_emission::StagedOptimizedFunctionFragmentEmission::corrupt_custody_fragments_for_test,
            ),
            (
                "manifest",
                machine_emission::StagedOptimizedFunctionFragmentEmission::corrupt_custody_manifest_for_test,
            ),
        ];
        for (field, corrupt) in receipt_mutations {
            let mut mutated = staged_emission(target);
            corrupt(&mut mutated);
            assert_eq!(
                machine_emission::validate_optimized_function_fragment_emission(&mutated),
                Err(EmissionError::ReceiptMismatch),
                "{target:?}: replay must reject a substituted custody receipt field {field}",
            );
        }

        // Closed axes and envelope corruption reject at canonical decoding,
        // before any custody decision.
        let encoded = original_manifest.encode();
        let offsets = manifest_wire_offsets(&encoded);
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[0] ^= 1,
            FunctionFragmentEmissionManifestDecodeError::WrongMagic,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[8..12].copy_from_slice(&17_u32.to_le_bytes()),
            FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(17),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.stage] = 9,
            FunctionFragmentEmissionManifestDecodeError::UnknownStage(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes[offsets.vocabulary..offsets.vocabulary + 2]
                    .copy_from_slice(&u16::MAX.to_le_bytes())
            },
            FunctionFragmentEmissionManifestDecodeError::UnknownVocabulary(u16::MAX),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes[offsets.fuel_schedule..offsets.fuel_schedule + 4]
                    .copy_from_slice(&0_u32.to_le_bytes())
            },
            FunctionFragmentEmissionManifestDecodeError::InvalidFuelSchedule,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.architecture] = 9,
            FunctionFragmentEmissionManifestDecodeError::UnknownArchitecture(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.object_format] = 9,
            FunctionFragmentEmissionManifestDecodeError::UnknownObjectFormat(9),
        );
        // `pointer_size`/`pointer_alignment` are open u64 fields on the wire;
        // on a 64-bit host `usize::try_from` accepts every pattern, so
        // `TargetLayoutOverflow` is unreachable here and the substitution is
        // sealed by the identity check instead.
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes[offsets.pointer_size..offsets.pointer_size + 8]
                    .copy_from_slice(&4_u64.to_le_bytes())
            },
            FunctionFragmentEmissionManifestDecodeError::IdentityMismatch,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.statistics] ^= 1,
            FunctionFragmentEmissionManifestDecodeError::IdentityMismatch,
        );
        for index in 0..6 {
            assert_manifest_decode_error(
                &encoded,
                |bytes| bytes[offsets.unavailable + index] = 0,
                FunctionFragmentEmissionManifestDecodeError::UnknownUnavailableStatus,
            );
        }
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes.push(0),
            FunctionFragmentEmissionManifestDecodeError::TrailingBytes,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes.pop();
            },
            FunctionFragmentEmissionManifestDecodeError::Truncated,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes.clear(),
            FunctionFragmentEmissionManifestDecodeError::Truncated,
        );

        // The authentic staged artifact restored after every substitution
        // still replays cleanly.
        assert_eq!(
            machine_emission::validate_optimized_function_fragment_emission(&staged),
            Ok(staged.custody()),
            "{target:?}: restored staged fragment emission must replay after all substitutions",
        );
    }
}
