//! Fixed-frame text-section custody mutation coverage.
//!
//! `machine_emission::stage_optimized_fixed_frame_text_section` publishes one
//! relocation-free [`RelocationFreeTextSectionPlacement`], a
//! [`FunctionFragmentTextSectionManifest`] binding every retained identity, and
//! an in-memory custody receipt over a validated frame application.
//! Independent replay (`validate_optimized_fixed_frame_text_section`)
//! re-validates the frame-application source, re-derives the dense placement
//! from the retained fragments, rejoins every manifest field against the
//! admitted inputs, and re-derives the receipt, so this family is exercised
//! here — beneath the real pipeline that produces the staged custody —
//! instead of inside `machine-emission` where no honest staged fixture exists.
//!
//! Every representable field of the placement, the manifest, and the custody
//! receipt is mutated independently. A field that still encodes canonically
//! under an honestly recomputed containing identity must be rejected by
//! replay; a field whose value is closed by the representation (single-variant
//! stage, placement-policy, relocation-requirement, and unavailable markers)
//! is mutated on the wire and must fail decoding before any custody decision.
//! Stale containing identities fail the identity check inside canonical
//! decoding or artifact validation.

use machine_code::{
    FunctionFragmentFrameApplicationIdentity, FunctionFragmentTextSectionManifest,
    FunctionFragmentTextSectionManifestDecodeError, InternalMachineCallResolutionKind,
    RelocationFreeTextSectionPlacement, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity,
};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    OptimizationSelections, PostAllocationOptimizationManifestIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
use proof_admission::AdmissionProfile;
use selected_instructions::{
    MachineAlternativeFamily, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, OperationId, ScalarType, ValueId,
};
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ProofBundle,
    SemanticFingerprint, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};

/// One callee returning a constant and one entry caller invoking it: the
/// smallest module that still places two functions and resolves one internal
/// machine call, so every mutation below exercises real retained custody —
/// including the resolved-call roster — rather than a fabricated record.
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

/// Stage the calling module through the real optimized pipeline, fragment
/// emission, frame application, and the text-placement publication join for
/// `target`. The returned staged section replays cleanly before any mutation.
fn staged_text_section(
    target: NativeTarget,
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
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
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .expect("calling module reaches physical staging");
    let emitted = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("fragment emission");
    let framed = machine_emission::stage_function_fragment_frame_application(emitted)
        .expect("frame application");
    let staged =
        machine_emission::stage_optimized_fixed_frame_text_section(framed).expect("text section");
    assert_eq!(
        machine_emission::validate_optimized_fixed_frame_text_section(&staged),
        Ok(staged.custody()),
        "{target:?}: honest staged text section must replay before mutation",
    );
    assert_eq!(
        staged.text_section().resolved_internal_machine_calls.len(),
        1,
        "{target:?}: fixture must retain one resolved internal call",
    );
    assert_eq!(staged.text_section().functions.len(), 2, "{target:?}");
    staged
}

/// Recompute the mutated placement's containing identity, then require
/// independent replay to reject the substitution at `expected`. `staged` is
/// restored afterward so later mutations stay independent of this one.
fn assert_placement_field(
    staged: &mut machine_emission::StagedOptimizedFixedFrameTextSection,
    original: &RelocationFreeTextSectionPlacement,
    field: &str,
    mutate: impl Fn(&mut RelocationFreeTextSectionPlacement),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    *staged.text_section_mut() = mutated;
    assert_eq!(
        machine_emission::validate_optimized_fixed_frame_text_section(staged),
        Err(machine_emission::RelocationFreeTextSectionPlacementError::ArtifactMismatch),
        "independent replay must reject substituted placement field {field}",
    );
    *staged.text_section_mut() = original.clone();
}

/// Recompute the mutated manifest's containing identity, require the envelope
/// to stay canonical, then require independent replay to reject the
/// substitution. `staged` is restored afterward so later mutations stay
/// independent of this one.
fn assert_manifest_field(
    staged: &mut machine_emission::StagedOptimizedFixedFrameTextSection,
    original: &FunctionFragmentTextSectionManifest,
    field: &str,
    mutate: impl Fn(&mut FunctionFragmentTextSectionManifest),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&mutated.encode()),
        Ok(mutated.clone()),
        "reauthenticated manifest field {field} must remain a canonical envelope",
    );
    *staged.manifest_mut().record_mut() = mutated;
    assert_eq!(
        machine_emission::validate_optimized_fixed_frame_text_section(staged),
        Err(machine_emission::RelocationFreeTextSectionPlacementError::ManifestMismatch),
        "independent replay must reject substituted manifest field {field}",
    );
    *staged.manifest_mut().record_mut() = original.clone();
}

fn assert_manifest_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: FunctionFragmentTextSectionManifestDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&encoded),
        Err(expected),
    );
}

/// Encoded-manifest layout: an 8-byte magic, 4-byte version, and 32-byte
/// identity envelope, then a one-byte stage tag, `frame_application` (32),
/// `source_fragment_manifest` (32), `source_realization` (32), `selections`
/// (32), the `psi` vocabulary marker (2) and program fingerprint (32),
/// `fuel_schedule` (4), `selected` (32), `post_allocation_manifest` (32),
/// `post_allocation_machine` (32), `final_pre_layout` (32),
/// `final_resolved_layout` (32), `whole_function_exit_contract` (32),
/// `fragments` (32), `target` (1+1+8+8), `semantic_entry` (8),
/// `semantic_entry_offset` (8), `placement_policy` (1), `text_section` (32),
/// `relocation_requirements` (1), `statistics` (80), and six single-byte
/// unavailable markers.
struct ManifestWireOffsets {
    stage: usize,
    vocabulary: usize,
    fuel_schedule: usize,
    architecture: usize,
    object_format: usize,
    pointer_size: usize,
    semantic_entry: usize,
    placement_policy: usize,
    relocation_requirements: usize,
    unavailable: [usize; 6],
}

fn manifest_wire_offsets(encoded: &[u8]) -> ManifestWireOffsets {
    let stage = 8 + 4 + 32;
    let vocabulary = stage + 1 + 32 + 32 + 32 + 32;
    let fuel_schedule = vocabulary + 2 + 32;
    let architecture = fuel_schedule + 4 + 32 + 32 + 32 + 32 + 32 + 32 + 32;
    let object_format = architecture + 1;
    let pointer_size = object_format + 1;
    let semantic_entry = pointer_size + 8 + 8;
    let semantic_entry_offset = semantic_entry + 8;
    let placement_policy = semantic_entry_offset + 8;
    let relocation_requirements = placement_policy + 1 + 32;
    let unavailable_start = relocation_requirements + 1 + 10 * 8;
    let unavailable = [
        unavailable_start,
        unavailable_start + 1,
        unavailable_start + 2,
        unavailable_start + 3,
        unavailable_start + 4,
        unavailable_start + 5,
    ];
    assert_eq!(encoded.len(), unavailable[5] + 1);
    ManifestWireOffsets {
        stage,
        vocabulary,
        fuel_schedule,
        architecture,
        object_format,
        pointer_size,
        semantic_entry,
        placement_policy,
        relocation_requirements,
        unavailable,
    }
}

#[test]
fn fixed_frame_text_section_custody_rejects_every_one_field_substitution() {
    use machine_emission::RelocationFreeTextSectionPlacementError as PlacementError;
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut staged = staged_text_section(target);
        let other_architecture = match staged.text_section().target.architecture {
            Architecture::X86_64 => Architecture::Aarch64,
            Architecture::Aarch64 => Architecture::X86_64,
        };
        let other_format = match staged.text_section().target.object_format {
            ObjectFormat::Elf => ObjectFormat::MachO,
            ObjectFormat::MachO => ObjectFormat::Coff,
            ObjectFormat::Coff => ObjectFormat::Elf,
        };
        // A post-allocation machine identity from the opposite architecture is
        // an authentic foreign value for the manifest binding below; the field
        // is `pub`/`Copy` so naming its representation type is unnecessary.
        let foreign_post_allocation_machine =
            staged_text_section(match staged.text_section().target.architecture {
                Architecture::X86_64 => NativeTarget::linux_arm64(),
                Architecture::Aarch64 => NativeTarget::linux_x64(),
            })
            .manifest()
            .record()
            .post_allocation_machine;

        // Every representable placement field mutates independently: the
        // recomputed identity keeps the artifact honest, and replay rejects
        // the substitution against the admitted frame application and the
        // re-derived dense placement. The single-variant policy and
        // relocation-requirement axes have no second representable value;
        // their closed tags are covered by the wire legs below.
        let original_section = staged.text_section().clone();
        let placement_mutations: [(&str, fn(&mut RelocationFreeTextSectionPlacement)); 12] = [
            ("source_fragments", |section| {
                section.source_fragments = FunctionFragmentEmissionIdentity::from_canonical_bytes(
                    b"substituted source fragments",
                )
            }),
            ("psi.program_fingerprint", |section| {
                section.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xb1; 32])
            }),
            ("fuel_schedule", |section| {
                section.fuel_schedule =
                    FuelScheduleIdentity::new(section.fuel_schedule.marker() + 1).unwrap()
            }),
            ("selected", |section| {
                section.selected = SelectedInstructionPlanIdentity::from_bytes([0xb2; 32])
            }),
            ("target.pointer_size", |section| {
                section.target.pointer_size = 4
            }),
            ("target.pointer_alignment", |section| {
                section.target.pointer_alignment = 4
            }),
            ("semantic_entry", |section| {
                section.semantic_entry = MachineId::new(913).unwrap()
            }),
            ("semantic_entry_offset", |section| {
                section.semantic_entry_offset += 1
            }),
            ("section_alignment", |section| {
                section.section_alignment += 1
            }),
            ("byte_count", |section| section.byte_count += 1),
            ("bytes", |section| section.bytes[0] ^= 1),
            ("bytes.resolved_call_field", |section| {
                let field = section.resolved_internal_machine_calls[0].field_section_offset;
                section.bytes[field as usize] ^= 1
            }),
        ];
        for (field, mutate) in placement_mutations {
            assert_placement_field(&mut staged, &original_section, field, mutate);
        }
        assert_placement_field(
            &mut staged,
            &original_section,
            "target.architecture",
            |section| section.target.architecture = other_architecture,
        );
        assert_placement_field(
            &mut staged,
            &original_section,
            "target.object_format",
            |section| section.target.object_format = other_format,
        );

        // The placed-function roster: cardinality, order, and each row field
        // join the fragment roster and the computed section offsets.
        let function_mutations: [(&str, fn(&mut RelocationFreeTextSectionPlacement)); 7] = [
            ("functions.dropped", |section| {
                section.functions.pop();
            }),
            ("functions.duplicated", |section| {
                section.functions.push(section.functions[0].clone())
            }),
            ("functions.swapped", |section| section.functions.swap(0, 1)),
            ("functions[0].source_function_index", |section| {
                section.functions[0].source_function_index += 1
            }),
            ("functions[0].machine", |section| {
                section.functions[0].machine = section.functions[1].machine
            }),
            ("functions[0].section_offset", |section| {
                section.functions[0].section_offset += 1
            }),
            ("functions[0].byte_count", |section| {
                section.functions[0].byte_count += 1
            }),
        ];
        for (field, mutate) in function_mutations {
            assert_placement_field(&mut staged, &original_section, field, mutate);
        }

        // Block and instruction spans join the source block/instruction
        // rosters row-for-row under function-relative and section-relative
        // coordinates. Selected ids restart per function, so a foreign row id
        // must come from outside the fixture's id space rather than from the
        // callee's first row.
        let other_block = SelectedBlockId(913);
        let other_instruction = SelectedInstructionId(913);
        let other_family = match staged.text_section().functions[0].blocks[0].instructions[0]
            .alternative
            .family
        {
            MachineAlternativeFamily::ReturnAggregate => MachineAlternativeFamily::CopyBytes,
            _ => MachineAlternativeFamily::ReturnAggregate,
        };
        let span_mutations: [(
            &str,
            fn(
                &mut RelocationFreeTextSectionPlacement,
                SelectedBlockId,
                SelectedInstructionId,
                MachineAlternativeFamily,
            ),
        ); 14] = [
            ("blocks.dropped", |section, _, _, _| {
                section.functions[0].blocks.pop();
            }),
            ("blocks.duplicated", |section, _, _, _| {
                let row = section.functions[0].blocks[0].clone();
                section.functions[0].blocks.push(row);
            }),
            ("blocks[0].block", |section, other, _, _| {
                section.functions[0].blocks[0].block = other
            }),
            ("blocks[0].function_offset", |section, _, _, _| {
                section.functions[0].blocks[0].function_offset += 1
            }),
            ("blocks[0].section_offset", |section, _, _, _| {
                section.functions[0].blocks[0].section_offset += 1
            }),
            ("blocks[0].byte_count", |section, _, _, _| {
                section.functions[0].blocks[0].byte_count += 1
            }),
            ("instructions.dropped", |section, _, _, _| {
                section.functions[0].blocks[0].instructions.pop();
            }),
            ("instructions.duplicated", |section, _, _, _| {
                let row = section.functions[0].blocks[0].instructions[0].clone();
                section.functions[0].blocks[0].instructions.push(row);
            }),
            ("instructions[0].instruction", |section, _, other, _| {
                section.functions[0].blocks[0].instructions[0].instruction = other
            }),
            (
                "instructions[0].alternative.family",
                |section, _, _, family| {
                    section.functions[0].blocks[0].instructions[0]
                        .alternative
                        .family = family
                },
            ),
            ("instructions[0].alternative.variant", |section, _, _, _| {
                section.functions[0].blocks[0].instructions[0]
                    .alternative
                    .variant += 1
            }),
            ("instructions[0].function_offset", |section, _, _, _| {
                section.functions[0].blocks[0].instructions[0].function_offset += 1
            }),
            ("instructions[0].section_offset", |section, _, _, _| {
                section.functions[0].blocks[0].instructions[0].section_offset += 1
            }),
            ("instructions[0].byte_count", |section, _, _, _| {
                section.functions[0].blocks[0].instructions[0].byte_count += 1
            }),
        ];
        for (field, mutate) in span_mutations {
            assert_placement_field(&mut staged, &original_section, field, |section| {
                mutate(section, other_block, other_instruction, other_family)
            });
        }

        // Every resolved internal machine call row field binds the source
        // fixup, both coordinate systems, and the decoded call bytes. `kind`
        // and `state` are each closed per target; `state` is single-variant
        // so only `kind` can be substituted in memory — the closed tags are
        // covered by the manifest wire legs below.
        let call_mutations: [(&str, fn(&mut RelocationFreeTextSectionPlacement)); 21] = [
            ("calls.dropped", |section| {
                section.resolved_internal_machine_calls.clear()
            }),
            ("calls.duplicated", |section| {
                let row = section.resolved_internal_machine_calls[0];
                section.resolved_internal_machine_calls.push(row)
            }),
            ("calls[0].kind", |section| {
                section.resolved_internal_machine_calls[0].kind = match section
                    .resolved_internal_machine_calls[0]
                    .kind
                {
                    InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1 => {
                        InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1
                    }
                    InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => {
                        InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1
                    }
                }
            }),
            ("calls[0].caller", |section| {
                section.resolved_internal_machine_calls[0].caller =
                    section.resolved_internal_machine_calls[0].callee
            }),
            ("calls[0].block", |section| {
                section.resolved_internal_machine_calls[0].block = SelectedBlockId(913)
            }),
            ("calls[0].instruction", |section| {
                section.resolved_internal_machine_calls[0].instruction = SelectedInstructionId(913)
            }),
            ("calls[0].operation", |section| {
                section.resolved_internal_machine_calls[0].operation =
                    OperationId::new(913).unwrap()
            }),
            ("calls[0].callee", |section| {
                section.resolved_internal_machine_calls[0].callee =
                    section.resolved_internal_machine_calls[0].caller
            }),
            ("calls[0].call_function_offset", |section| {
                section.resolved_internal_machine_calls[0].call_function_offset += 1
            }),
            ("calls[0].call_section_offset", |section| {
                section.resolved_internal_machine_calls[0].call_section_offset += 1
            }),
            ("calls[0].call_byte_count", |section| {
                section.resolved_internal_machine_calls[0].call_byte_count += 1
            }),
            ("calls[0].opcode_function_offset", |section| {
                section.resolved_internal_machine_calls[0].opcode_function_offset += 1
            }),
            ("calls[0].opcode_section_offset", |section| {
                section.resolved_internal_machine_calls[0].opcode_section_offset += 1
            }),
            ("calls[0].field_function_offset", |section| {
                section.resolved_internal_machine_calls[0].field_function_offset += 1
            }),
            ("calls[0].field_section_offset", |section| {
                section.resolved_internal_machine_calls[0].field_section_offset += 1
            }),
            ("calls[0].next_instruction_function_offset", |section| {
                section.resolved_internal_machine_calls[0].next_instruction_function_offset += 1
            }),
            ("calls[0].next_instruction_section_offset", |section| {
                section.resolved_internal_machine_calls[0].next_instruction_section_offset += 1
            }),
            ("calls[0].callee_section_offset", |section| {
                section.resolved_internal_machine_calls[0].callee_section_offset += 1
            }),
            ("calls[0].field_byte_width", |section| {
                section.resolved_internal_machine_calls[0].field_byte_width += 1
            }),
            ("calls[0].addend", |section| {
                section.resolved_internal_machine_calls[0].addend += 1
            }),
            ("calls[0].displacement", |section| {
                section.resolved_internal_machine_calls[0].displacement += 1
            }),
        ];
        for (field, mutate) in call_mutations {
            assert_placement_field(&mut staged, &original_section, field, mutate);
        }

        // A foreign placement identity fails the artifact identity check
        // before any field comparison.
        *staged.text_section_mut() = original_section.clone();
        staged.text_section_mut().identity =
            TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                b"foreign text-section identity",
            );
        assert_eq!(
            machine_emission::validate_optimized_fixed_frame_text_section(&staged),
            Err(PlacementError::ArtifactMismatch),
            "{target:?}: replay must reject a foreign placement identity",
        );
        *staged.text_section_mut() = original_section.clone();

        // Every representable manifest field mutates independently under an
        // honestly recomputed containing identity; replay rejects each one
        // against the admitted source manifest and the checked section.
        let original_manifest = staged.manifest().record().clone();
        let manifest_mutations: [(&str, fn(&mut FunctionFragmentTextSectionManifest)); 27] = [
            ("frame_application", |record| {
                record.frame_application =
                    FunctionFragmentFrameApplicationIdentity::from_bytes([0xc1; 32])
            }),
            ("source_fragment_manifest", |record| {
                record.source_fragment_manifest =
                    FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(
                        b"substituted fragment manifest",
                    )
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
            ("semantic_entry", |record| {
                record.semantic_entry = MachineId::new(913).unwrap()
            }),
            ("semantic_entry_offset", |record| {
                record.semantic_entry_offset += 1
            }),
            ("text_section", |record| {
                record.text_section =
                    TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                        b"substituted text section",
                    )
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
            ("statistics.padding_bytes", |record| {
                record.statistics.padding_bytes += 1
            }),
            ("statistics.relocation_requirements", |record| {
                record.statistics.relocation_requirements += 1
            }),
            ("statistics.source_internal_machine_fixups", |record| {
                record.statistics.source_internal_machine_fixups += 1
            }),
            ("statistics.resolved_internal_machine_fixups", |record| {
                record.statistics.resolved_internal_machine_fixups += 1
            }),
            ("statistics.remaining_internal_machine_fixups", |record| {
                record.statistics.remaining_internal_machine_fixups += 1
            }),
        ];
        for (field, mutate) in manifest_mutations {
            assert_manifest_field(&mut staged, &original_manifest, field, mutate);
        }
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "post_allocation_machine",
            |record| record.post_allocation_machine = foreign_post_allocation_machine,
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

        // The containing identity itself is sealed over the content: a foreign
        // identity, or honest content drift carried under the stale identity,
        // fails the identity check inside canonical decoding and at replay.
        let mut stale = original_manifest.clone();
        stale.identity = FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(
            b"foreign manifest identity",
        );
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&stale.encode()),
            Err(FunctionFragmentTextSectionManifestDecodeError::IdentityMismatch),
            "{target:?}: foreign manifest identity must fail canonical decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            machine_emission::validate_optimized_fixed_frame_text_section(&staged),
            Err(PlacementError::ManifestMismatch),
            "{target:?}: replay must reject a foreign manifest identity",
        );
        let mut stale = original_manifest.clone();
        stale.statistics.bytes += 1;
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&stale.encode()),
            Err(FunctionFragmentTextSectionManifestDecodeError::IdentityMismatch),
            "{target:?}: drifted content under a stale identity must fail decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            machine_emission::validate_optimized_fixed_frame_text_section(&staged),
            Err(PlacementError::ManifestMismatch),
            "{target:?}: replay must reject drifted manifest content under a stale identity",
        );
        *staged.manifest_mut().record_mut() = original_manifest.clone();

        // The custody receipt fields have no independent wire form; each is
        // substituted in place and replay rejects every one.
        let receipt_mutations: [(
            &str,
            fn(&mut machine_emission::StagedOptimizedFixedFrameTextSection),
        ); 4] = [
            (
                "frame_application",
                machine_emission::StagedOptimizedFixedFrameTextSection::corrupt_custody_frame_application_for_test,
            ),
            (
                "fragments",
                machine_emission::StagedOptimizedFixedFrameTextSection::corrupt_custody_fragments_for_test,
            ),
            (
                "text_section",
                machine_emission::StagedOptimizedFixedFrameTextSection::corrupt_custody_text_section_for_test,
            ),
            (
                "manifest",
                machine_emission::StagedOptimizedFixedFrameTextSection::corrupt_custody_manifest_for_test,
            ),
        ];
        for (field, corrupt) in receipt_mutations {
            let mut mutated = staged_text_section(target);
            corrupt(&mut mutated);
            assert_eq!(
                machine_emission::validate_optimized_fixed_frame_text_section(&mutated),
                Err(PlacementError::ReceiptMismatch),
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
            FunctionFragmentTextSectionManifestDecodeError::WrongMagic,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[8..12].copy_from_slice(&17_u32.to_le_bytes()),
            FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(17),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.stage] = 9,
            FunctionFragmentTextSectionManifestDecodeError::UnknownStage(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes[offsets.vocabulary..offsets.vocabulary + 2]
                    .copy_from_slice(&u16::MAX.to_le_bytes())
            },
            FunctionFragmentTextSectionManifestDecodeError::UnknownVocabulary(u16::MAX),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes[offsets.fuel_schedule..offsets.fuel_schedule + 4]
                    .copy_from_slice(&0_u32.to_le_bytes())
            },
            FunctionFragmentTextSectionManifestDecodeError::InvalidFuelSchedule,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.architecture] = 9,
            FunctionFragmentTextSectionManifestDecodeError::UnknownArchitecture(9),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.object_format] = 9,
            FunctionFragmentTextSectionManifestDecodeError::UnknownObjectFormat(9),
        );
        // `pointer_size`/`pointer_alignment` are open u64 fields on the wire;
        // on a 64-bit host `usize::try_from` accepts every pattern, so
        // `TargetLayoutOverflow` is unreachable here and the substitution is
        // sealed by the identity check instead.
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes[offsets.pointer_size..offsets.pointer_size + 8]
                    .copy_from_slice(&u64::MAX.to_le_bytes())
            },
            FunctionFragmentTextSectionManifestDecodeError::IdentityMismatch,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes[offsets.semantic_entry..offsets.semantic_entry + 8]
                    .copy_from_slice(&0_u64.to_le_bytes())
            },
            FunctionFragmentTextSectionManifestDecodeError::InvalidSemanticEntry,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.placement_policy] = 7,
            FunctionFragmentTextSectionManifestDecodeError::UnknownPlacementPolicy(7),
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes[offsets.relocation_requirements] = 7,
            FunctionFragmentTextSectionManifestDecodeError::UnknownRelocationRequirements(7),
        );
        for offset in offsets.unavailable {
            assert_manifest_decode_error(
                &encoded,
                |bytes| bytes[offset] = 0,
                FunctionFragmentTextSectionManifestDecodeError::UnknownUnavailableStatus,
            );
        }
        assert_manifest_decode_error(
            &encoded,
            |bytes| bytes.push(0),
            FunctionFragmentTextSectionManifestDecodeError::TrailingBytes,
        );
        assert_manifest_decode_error(
            &encoded,
            |bytes| {
                bytes.truncate(encoded.len() - 1);
            },
            FunctionFragmentTextSectionManifestDecodeError::Truncated,
        );
    }
}
