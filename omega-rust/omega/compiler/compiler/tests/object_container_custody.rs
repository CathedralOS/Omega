//! Relocation-free object-container custody mutation coverage.
//!
//! `object_file::stage_optimized_relocation_free_object_container` publishes
//! one relocation-free [`RelocationFreeObjectPlan`], its canonical container
//! bytes, a [`FunctionFragmentObjectContainerManifest`] binding every retained
//! identity, and an in-memory custody receipt over a retained fixed-frame
//! text section. Independent replay
//! (`validate_optimized_relocation_free_object_container`) re-derives the
//! object from the retained text section, recomputes the container identity,
//! decodes the container bytes back into a plan, rejoins every manifest
//! field, and re-derives the receipt, so this family is exercised here —
//! beneath the real pipeline that produces the staged custody — instead of
//! inside `object-file` where no honest staged fixture exists.
//!
//! Every representable field of the manifest, the custody receipt, the
//! container, and the published object plan is mutated independently. A field
//! that still encodes canonically under an honestly recomputed containing
//! identity must be rejected by replay; a field whose value is closed by the
//! representation (single-variant stage/policy/requirement/unavailable
//! markers) is mutated on the wire and must fail decoding before any custody
//! decision. Stale containing identities fail the identity check inside
//! canonical decoding or object validation. The object plan's own codec
//! surface — symbol naming, interval density, and target admission — is
//! covered by `object-file`'s relocation-free object tests; the legs below
//! pin the custody joins this stage adds on top.

use object_file::{
    FunctionFragmentObjectContainerManifest, FunctionFragmentObjectContainerManifestDecodeError,
    ObjectLocalSymbolId, RelocationFreeObjectContainerError, RelocationFreeObjectDecodeError,
    RelocationFreeObjectError, RelocationFreeObjectPlan, RelocationFreeObjectSymbolRole,
    StagedOptimizedRelocationFreeObjectContainer,
    validate_optimized_relocation_free_object_container,
};
use optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    OptimizationSelectionIdentity, OptimizationSelections, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use proof_admission::AdmissionProfile;
use selected_instructions::SelectedInstructionPlanIdentity;
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

/// One machine returning one integer constant: the smallest module that still
/// reaches a published object container, so every mutation below exercises
/// real retained custody rather than a fabricated record.
fn minimal_module() -> TerminalModule {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type,
    };
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(3)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(1).unwrap(),
                    result: OperationResult::Scalar(declaration(1)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(2).unwrap(),
                    value: ValueId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
                erased_scalar_formals: Vec::new(),
                crash_routes: Vec::new(),
            },
        }],
    }
}

/// Stage the minimal module through the real optimized pipeline, fragment
/// emission, frame application, text placement, and the object-container
/// publication join for `target`. The returned staged container replays
/// cleanly before any mutation.
fn staged_container(target: NativeTarget) -> StagedOptimizedRelocationFreeObjectContainer {
    let module = minimal_module();
    let proof = ProofBundle::default();
    let semantic = terminal_codec::encode_module(&module).expect("encode minimal module");
    let proof_bytes =
        terminal_codec::encode_proof_section(&module, &proof).expect("encode minimal proof");
    let optimized = native_realization::optimize_artifact_sections(
        &semantic,
        &proof_bytes,
        &AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
    )
    .expect("minimal module optimizes");
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .expect("minimal module reaches physical staging");
    let emitted = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("fragment emission");
    let framed = machine_emission::stage_function_fragment_frame_application(emitted)
        .expect("frame application");
    let placed =
        machine_emission::stage_optimized_fixed_frame_text_section(framed).expect("text section");
    let staged = object_file::stage_optimized_relocation_free_object_container(placed)
        .expect("object container");
    assert_eq!(
        validate_optimized_relocation_free_object_container(&staged),
        Ok(staged.custody()),
        "{target:?}: honest staged container must replay before mutation",
    );
    staged
}

/// Recompute the mutated manifest's containing identity, require the envelope
/// to stay canonical, then require independent replay to reject the
/// substitution. `staged` is restored afterward so later mutations stay
/// independent of this one.
fn assert_manifest_field(
    staged: &mut StagedOptimizedRelocationFreeObjectContainer,
    original: &FunctionFragmentObjectContainerManifest,
    field: &str,
    mutate: impl Fn(&mut FunctionFragmentObjectContainerManifest),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&mutated.encode()),
        Ok(mutated.clone()),
        "reauthenticated manifest field {field} must remain a canonical envelope",
    );
    *staged.manifest_mut().record_mut() = mutated;
    assert_eq!(
        validate_optimized_relocation_free_object_container(staged),
        Err(RelocationFreeObjectContainerError::ManifestMismatch),
        "independent replay must reject substituted manifest field {field}",
    );
    *staged.manifest_mut().record_mut() = original.clone();
}

/// Same contract for the published object plan: the mutated field still
/// carries an honestly recomputed plan identity, and replay rejects the
/// substitution at the join named by `expected`.
fn assert_object_field(
    staged: &mut StagedOptimizedRelocationFreeObjectContainer,
    original: &RelocationFreeObjectPlan,
    field: &str,
    expected: RelocationFreeObjectContainerError,
    mutate: impl Fn(&mut RelocationFreeObjectPlan),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated
        .recomputed_identity()
        .expect("mutated object plan must keep a recomputed identity");
    *staged.object_mut() = mutated;
    assert_eq!(
        validate_optimized_relocation_free_object_container(staged),
        Err(expected),
        "independent replay must reject substituted object field {field}",
    );
    *staged.object_mut() = original.clone();
}

fn assert_manifest_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: FunctionFragmentObjectContainerManifestDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&encoded),
        Err(expected),
    );
}

/// Encoded-manifest layout: an 8-byte magic, 4-byte version, and 32-byte
/// identity envelope, then a one-byte stage tag,
/// `source_text_section_manifest` (32), `text_section` (32), the `psi`
/// vocabulary marker (2) and program fingerprint (32), `fuel_schedule` (4),
/// `selections` (32), `selected` (32), `target` (1+1+8+8), `semantic_entry`
/// (8), `semantic_entry_symbol` (8), `symbol_policy` (1), `object` (32),
/// `object_container` (32), `relocation_requirements` (1), `statistics` (56),
/// and four single-byte unavailable markers.
struct ManifestWireOffsets {
    stage: usize,
    vocabulary: usize,
    fuel_schedule: usize,
    architecture: usize,
    object_format: usize,
    semantic_entry: usize,
    semantic_entry_symbol: usize,
    symbol_policy: usize,
    relocation_requirements: usize,
    unavailable: [usize; 4],
}

fn manifest_wire_offsets(encoded: &[u8]) -> ManifestWireOffsets {
    let stage = 8 + 4 + 32;
    let vocabulary = stage + 1 + 32 + 32;
    let fuel_schedule = vocabulary + 2 + 32;
    let architecture = fuel_schedule + 4 + 32 + 32;
    let object_format = architecture + 1;
    let semantic_entry = object_format + 1 + 8 + 8;
    let semantic_entry_symbol = semantic_entry + 8;
    let symbol_policy = semantic_entry_symbol + 8;
    let relocation_requirements = symbol_policy + 1 + 32 + 32;
    let unavailable_start = relocation_requirements + 1 + 7 * 8;
    let unavailable = [
        unavailable_start,
        unavailable_start + 1,
        unavailable_start + 2,
        unavailable_start + 3,
    ];
    assert_eq!(encoded.len(), unavailable[3] + 1);
    ManifestWireOffsets {
        stage,
        vocabulary,
        fuel_schedule,
        architecture,
        object_format,
        semantic_entry,
        semantic_entry_symbol,
        symbol_policy,
        relocation_requirements,
        unavailable,
    }
}

#[test]
fn relocation_free_object_container_custody_rejects_every_one_field_substitution() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut staged = staged_container(target);
        let other_architecture = match staged.object().target.architecture {
            Architecture::X86_64 => Architecture::Aarch64,
            Architecture::Aarch64 => Architecture::X86_64,
        };
        let other_format = match staged.object().target.object_format {
            ObjectFormat::Elf => ObjectFormat::MachO,
            ObjectFormat::MachO => ObjectFormat::Coff,
            ObjectFormat::Coff => ObjectFormat::Elf,
        };
        let other_target = if target == NativeTarget::linux_x64() {
            NativeTarget::macos_arm64()
        } else {
            NativeTarget::linux_x64()
        };

        // Every representable manifest field mutates independently: the
        // recomputed identity keeps the envelope canonical, and replay rejects
        // the substitution against the retained text section, object plan,
        // and container bytes.
        let original_manifest = staged.manifest().record().clone();
        let manifest_mutations: [(&str, fn(&mut FunctionFragmentObjectContainerManifest)); 16] = [
            ("source_text_section_manifest", |record| {
                record.source_text_section_manifest =
                    FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(
                        b"substituted source manifest",
                    )
            }),
            ("text_section", |record| {
                record.text_section =
                    TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                        b"substituted text section",
                    )
            }),
            ("psi.program_fingerprint", |record| {
                record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
            }),
            ("fuel_schedule", |record| {
                record.fuel_schedule =
                    FuelScheduleIdentity::new(record.fuel_schedule.marker() + 1).unwrap()
            }),
            ("selections", |record| {
                record.selections = OptimizationSelectionIdentity::from_bytes([0x5e; 32])
            }),
            ("selected", |record| {
                record.selected = SelectedInstructionPlanIdentity::from_bytes([0x5c; 32])
            }),
            ("semantic_entry", |record| {
                record.semantic_entry = MachineId::new(913).unwrap()
            }),
            ("semantic_entry_symbol", |record| {
                record.semantic_entry_symbol = ObjectLocalSymbolId::new(913).unwrap()
            }),
            ("object", |record| {
                record.object =
                    RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"substituted object")
            }),
            ("object_container", |record| {
                record.object_container =
                    RelocationFreeObjectContainerIdentity::from_canonical_bytes(
                        b"substituted object container",
                    )
            }),
            ("statistics.sections", |record| {
                record.statistics.sections += 1
            }),
            ("statistics.function_symbols", |record| {
                record.statistics.function_symbols += 1
            }),
            ("statistics.object_local_symbols", |record| {
                record.statistics.object_local_symbols += 1
            }),
            ("statistics.external_symbols", |record| {
                record.statistics.external_symbols += 1
            }),
            ("statistics.text_bytes", |record| {
                record.statistics.text_bytes += 1
            }),
            ("statistics.container_bytes", |record| {
                record.statistics.container_bytes += 1
            }),
        ];
        for (field, mutate) in manifest_mutations {
            assert_manifest_field(&mut staged, &original_manifest, field, mutate);
        }
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "statistics.relocation_records",
            |record| record.statistics.relocation_records += 1,
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
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "target.pointer_size",
            |record| record.target.pointer_size = 4,
        );
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "target.pointer_alignment",
            |record| record.target.pointer_alignment = 4,
        );

        // The containing identity itself is sealed over the content: a foreign
        // identity, or honest content drift carried under the stale identity,
        // fails the identity check inside canonical decoding and at replay.
        let mut stale = original_manifest.clone();
        stale.identity = FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(
            b"foreign manifest identity",
        );
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&stale.encode()),
            Err(FunctionFragmentObjectContainerManifestDecodeError::IdentityMismatch),
            "{target:?}: foreign manifest identity must fail canonical decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Err(RelocationFreeObjectContainerError::ManifestMismatch),
            "{target:?}: replay must reject a foreign manifest identity",
        );
        let mut stale = original_manifest.clone();
        stale.statistics.text_bytes += 1;
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&stale.encode()),
            Err(FunctionFragmentObjectContainerManifestDecodeError::IdentityMismatch),
            "{target:?}: drifted content under a stale identity must fail decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Err(RelocationFreeObjectContainerError::ManifestMismatch),
            "{target:?}: replay must reject drifted manifest content under a stale identity",
        );
        *staged.manifest_mut().record_mut() = original_manifest.clone();

        // The custody receipt fields have no independent wire form; each is
        // substituted in place and replay rejects every one.
        let receipt_mutations: [(
            &str,
            fn(&mut StagedOptimizedRelocationFreeObjectContainer),
        ); 5] = [
            (
                "source_text_section_manifest",
                StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_source_text_section_manifest_for_test,
            ),
            (
                "text_section",
                StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_text_section_for_test,
            ),
            (
                "object",
                StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_object_for_test,
            ),
            (
                "object_container",
                StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_object_container_for_test,
            ),
            (
                "manifest",
                StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_manifest_for_test,
            ),
        ];
        for (field, corrupt) in receipt_mutations {
            let mut substituted = staged_container(target);
            corrupt(&mut substituted);
            assert_eq!(
                validate_optimized_relocation_free_object_container(&substituted),
                Err(RelocationFreeObjectContainerError::ReceiptMismatch),
                "{target:?}: replay must reject substituted custody receipt field {field}",
            );
        }

        // The container binds the object identity and its own canonical
        // bytes: a substituted join identity or foreign container identity is
        // rejected before decoding, and a truncated canonical body that keeps
        // an honest container identity fails decoding outright.
        let original_container = staged.container().clone();
        let mut substituted = original_container.clone();
        substituted.object =
            RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"substituted object");
        *staged.container_mut() = substituted;
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Err(RelocationFreeObjectContainerError::ContainerMismatch),
            "{target:?}: replay must reject a substituted container object identity",
        );
        let mut substituted = original_container.clone();
        substituted.identity = RelocationFreeObjectContainerIdentity::from_canonical_bytes(
            b"foreign container identity",
        );
        *staged.container_mut() = substituted;
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Err(RelocationFreeObjectContainerError::ContainerMismatch),
            "{target:?}: replay must reject a foreign container identity",
        );
        let mut substituted = original_container.clone();
        substituted.bytes.pop();
        substituted.identity =
            RelocationFreeObjectContainerIdentity::from_canonical_bytes(&substituted.bytes);
        *staged.container_mut() = substituted;
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Err(RelocationFreeObjectContainerError::InvalidContainer(
                RelocationFreeObjectDecodeError::Truncated,
            )),
            "{target:?}: replay must reject a truncated container under an honest identity",
        );
        *staged.container_mut() = original_container;

        // Every representable object-plan field mutates independently under an
        // honestly recomputed plan identity. Canonical-admission fields are
        // rejected by object validation; retained-source fields survive it and
        // are rejected by the source join.
        let original_object = staged.object().clone();
        let artifact = || RelocationFreeObjectContainerError::ArtifactMismatch;
        let invalid = |error: RelocationFreeObjectError| {
            RelocationFreeObjectContainerError::InvalidObject(error)
        };
        let object_mutations: [(
            &str,
            RelocationFreeObjectContainerError,
            fn(&mut RelocationFreeObjectPlan),
        ); 22] = [
            ("source_text_section", artifact(), |object| {
                object.source_text_section =
                    TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                        b"substituted text section",
                    )
            }),
            ("psi.program_fingerprint", artifact(), |object| {
                object.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
            }),
            ("fuel_schedule", artifact(), |object| {
                object.fuel_schedule =
                    FuelScheduleIdentity::new(object.fuel_schedule.marker() + 1).unwrap()
            }),
            ("selected", artifact(), |object| {
                object.selected = SelectedInstructionPlanIdentity::from_bytes([0x5c; 32])
            }),
            ("selections", artifact(), |object| {
                object.selections = OptimizationSelectionIdentity::from_bytes([0x5e; 32])
            }),
            (
                "target.pointer_size",
                invalid(RelocationFreeObjectError::NonCanonicalTarget),
                |object| object.target.pointer_size = 4,
            ),
            (
                "semantic_entry",
                invalid(RelocationFreeObjectError::WrongSemanticEntryRole),
                |object| object.semantic_entry = MachineId::new(913).unwrap(),
            ),
            (
                "semantic_entry_symbol",
                invalid(RelocationFreeObjectError::WrongSemanticEntrySymbol),
                |object| object.semantic_entry_symbol = ObjectLocalSymbolId::new(913).unwrap(),
            ),
            (
                "text_section.name",
                invalid(RelocationFreeObjectError::WrongTextSectionName),
                |object| object.text_section.name = "substituted.text".to_string(),
            ),
            (
                "text_section.alignment",
                invalid(RelocationFreeObjectError::WrongTextSectionAlignment),
                |object| object.text_section.alignment += 1,
            ),
            (
                "text_section.byte_count",
                invalid(RelocationFreeObjectError::TextSectionLengthMismatch),
                |object| object.text_section.byte_count += 1,
            ),
            ("text_section.bytes", artifact(), |object| {
                object.text_section.bytes[0] ^= 1
            }),
            (
                "symbols (dropped)",
                invalid(RelocationFreeObjectError::EmptySymbolTable),
                |object| {
                    object.symbols.pop();
                },
            ),
            (
                "symbols (duplicated)",
                invalid(RelocationFreeObjectError::NonCanonicalSymbolId),
                |object| object.symbols.push(object.symbols[0].clone()),
            ),
            (
                "symbols[0].symbol",
                invalid(RelocationFreeObjectError::NonCanonicalSymbolId),
                |object| object.symbols[0].symbol = ObjectLocalSymbolId::new(913).unwrap(),
            ),
            (
                "symbols[0].source_function_index",
                invalid(RelocationFreeObjectError::NonCanonicalSourceFunctionIndex),
                |object| object.symbols[0].source_function_index = 1,
            ),
            (
                "symbols[0].machine",
                invalid(RelocationFreeObjectError::NonCanonicalSymbolName),
                |object| object.symbols[0].machine = MachineId::new(913).unwrap(),
            ),
            (
                "symbols[0].name",
                invalid(RelocationFreeObjectError::NonCanonicalSymbolName),
                |object| object.symbols[0].name = "substituted_symbol".to_string(),
            ),
            (
                "symbols[0].section_offset",
                invalid(RelocationFreeObjectError::NonDenseSymbolInterval),
                |object| object.symbols[0].section_offset = 1,
            ),
            (
                "symbols[0].byte_count",
                invalid(RelocationFreeObjectError::SymbolOutsideTextSection),
                |object| object.symbols[0].byte_count += 1,
            ),
            (
                "symbols[0].role",
                invalid(RelocationFreeObjectError::WrongSemanticEntryRole),
                |object| object.symbols[0].role = RelocationFreeObjectSymbolRole::PrivateFunctionV1,
            ),
            (
                "relocation_record_count",
                invalid(RelocationFreeObjectError::RelocationsPresent),
                |object| object.relocation_record_count = 1,
            ),
        ];
        for (field, expected, mutate) in object_mutations {
            assert_object_field(&mut staged, &original_object, field, expected, mutate);
        }
        // A substituted target's rejection site is derived from the declared
        // policy matrix in the validator's own order: undeclared pair or
        // non-64-bit pointer contract, then section name, then alignment. A
        // same-policy substitution survives canonical admission and the
        // source join must reject the retained source target.
        let expected_for_target =
            |candidate: NativeTarget| match object_file::object_target_policy(candidate) {
                None => invalid(RelocationFreeObjectError::NonCanonicalTarget),
                Some(policy) if original_object.text_section.name != policy.text_section_name => {
                    invalid(RelocationFreeObjectError::WrongTextSectionName)
                }
                Some(policy)
                    if original_object.text_section.alignment != policy.text_section_alignment =>
                {
                    invalid(RelocationFreeObjectError::WrongTextSectionAlignment)
                }
                Some(_) => artifact(),
            };
        let mut architecture_target = original_object.target;
        architecture_target.architecture = other_architecture;
        let mut format_target = original_object.target;
        format_target.object_format = other_format;
        assert_object_field(
            &mut staged,
            &original_object,
            "target.pointer_alignment",
            invalid(RelocationFreeObjectError::NonCanonicalTarget),
            |object| object.target.pointer_alignment = 4,
        );
        assert_object_field(
            &mut staged,
            &original_object,
            "target",
            expected_for_target(other_target),
            |object| object.target = other_target,
        );
        assert_object_field(
            &mut staged,
            &original_object,
            "target.architecture",
            expected_for_target(architecture_target),
            |object| object.target.architecture = other_architecture,
        );
        assert_object_field(
            &mut staged,
            &original_object,
            "target.object_format",
            expected_for_target(format_target),
            |object| object.target.object_format = other_format,
        );

        // The plan identity itself is recomputed over the content: a foreign
        // identity fails object validation before any source or container
        // join.
        let mut foreign = original_object.clone();
        foreign.identity =
            RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"foreign object identity");
        *staged.object_mut() = foreign;
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Err(RelocationFreeObjectContainerError::InvalidObject(
                RelocationFreeObjectError::StaleObjectIdentity,
            )),
            "{target:?}: replay must reject a foreign object identity",
        );
        *staged.object_mut() = original_object;

        // Closed wire axes: the single-variant stage, policy, requirement and
        // unavailable markers, unknown vocabulary/architecture/format tags, a
        // zero fuel schedule, zero machine and symbol identities, envelope
        // drift, and truncation all fail canonical decoding before any custody
        // decision. `pointer_size` and `pointer_alignment` accept every u64 on
        // 64-bit hosts, so their mutations above are representable
        // substitutions rather than wire axes; the TargetLayoutOverflow arm
        // remains 32-bit-only.
        let manifest_bytes = staged.manifest().record().encode();
        let manifest_offsets = manifest_wire_offsets(&manifest_bytes);
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.stage] = 9,
            FunctionFragmentObjectContainerManifestDecodeError::UnknownStage(9),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.vocabulary..manifest_offsets.vocabulary + 2]
                    .copy_from_slice(&108_u16.to_le_bytes())
            },
            FunctionFragmentObjectContainerManifestDecodeError::UnknownVocabulary(108),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.fuel_schedule..manifest_offsets.fuel_schedule + 4].fill(0)
            },
            FunctionFragmentObjectContainerManifestDecodeError::InvalidFuelSchedule,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.architecture] = 9,
            FunctionFragmentObjectContainerManifestDecodeError::UnknownArchitecture(9),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.object_format] = 9,
            FunctionFragmentObjectContainerManifestDecodeError::UnknownObjectFormat(9),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.semantic_entry..manifest_offsets.semantic_entry + 8].fill(0)
            },
            FunctionFragmentObjectContainerManifestDecodeError::InvalidSemanticEntry,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.semantic_entry_symbol
                    ..manifest_offsets.semantic_entry_symbol + 8]
                    .fill(0)
            },
            FunctionFragmentObjectContainerManifestDecodeError::InvalidSymbolId,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.symbol_policy] = 9,
            FunctionFragmentObjectContainerManifestDecodeError::UnknownSymbolPolicy,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.relocation_requirements] = 9,
            FunctionFragmentObjectContainerManifestDecodeError::UnknownRelocationRequirements,
        );
        for offset in manifest_offsets.unavailable {
            assert_manifest_decode_error(
                &manifest_bytes,
                |bytes| bytes[offset] = 9,
                FunctionFragmentObjectContainerManifestDecodeError::UnknownUnavailableStatus,
            );
        }
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[12] ^= 1,
            FunctionFragmentObjectContainerManifestDecodeError::IdentityMismatch,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.stage + 1] ^= 1,
            FunctionFragmentObjectContainerManifestDecodeError::IdentityMismatch,
        );
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(
                &manifest_bytes[..manifest_bytes.len() - 1],
            ),
            Err(FunctionFragmentObjectContainerManifestDecodeError::Truncated),
            "{target:?}: truncated manifest content must reject",
        );
        let mut trailing = manifest_bytes.clone();
        trailing.push(0);
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&trailing),
            Err(FunctionFragmentObjectContainerManifestDecodeError::TrailingBytes),
            "{target:?}: trailing manifest bytes must reject",
        );

        // The restored staged container still replays honestly after every
        // substitution above was rolled back.
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Ok(staged.custody()),
            "{target:?}: the restored staged container must replay after the matrix",
        );
    }
}
