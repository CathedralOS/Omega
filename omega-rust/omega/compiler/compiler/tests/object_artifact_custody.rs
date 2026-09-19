//! Optimized-object artifact custody mutation coverage.
//!
//! `object_file::stage_validated_optimized_object_artifact` seals one
//! source-free [`OptimizedObjectArtifactRecord`], its
//! [`OptimizedObjectArtifactManifest`], and the in-memory custody receipt over
//! a retained canonical terminal artifact and relocation-free object
//! container. Independent replay (`validate_optimized_object_artifact`)
//! re-derives all three from that retained evidence rather than trusting the
//! presented records, so this family is exercised here — beneath the real
//! pipeline that produces the staged custody — instead of inside `object-file`
//! where no honest staged fixture exists.
//!
//! Every representable field of each record is mutated independently. A field
//! that still encodes canonically under an honestly recomputed containing
//! identity must be rejected by replay; a field whose value is closed by the
//! representation (single-variant stage/unavailable markers) is mutated on the
//! wire and must fail decoding before any custody decision. Stale containing
//! identities fail the identity check inside canonical decoding.

use object_file::{
    OptimizedObjectArtifactError, OptimizedObjectArtifactManifest,
    OptimizedObjectArtifactManifestDecodeError, OptimizedObjectArtifactRecord,
    OptimizedObjectArtifactRecordDecodeError, StagedValidatedOptimizedObjectArtifact,
    validate_optimized_object_artifact,
};
use optimization_core::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifestIdentity, PostAllocationOptimizationManifestIdentity,
    PrePhysicalOptimizationManifestIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ProofBundle,
    SemanticFingerprint, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};

/// One machine returning one integer constant: the smallest module that still
/// reaches a sealed object artifact, so every mutation below exercises real
/// retained custody rather than a fabricated record.
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
                crash_routes: Vec::new(),
                erased_scalar_formals: Vec::new(),
            },
        }],
    }
}

/// Stage the minimal module through the real optimized pipeline, fragment
/// emission, object container, and the artifact-custody join for `target`.
/// The returned staged artifact replays cleanly before any mutation.
fn staged_artifact(target: NativeTarget) -> StagedValidatedOptimizedObjectArtifact {
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
    let source = object_file::stage_optimized_relocation_free_object_container(placed)
        .expect("object container");
    let optimization =
        terminal_codec::build_identity_optimization_execution_record(&module, &proof)
            .expect("identity optimization record");
    let terminal =
        terminal_codec::CanonicalTerminalArtifact::from_parts(&module, &proof, &optimization, None)
            .expect("canonical terminal artifact");
    let staged = object_file::stage_validated_optimized_object_artifact(terminal, source)
        .expect("validated object artifact");
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Ok(staged.custody()),
        "{target:?}: honest staged artifact must replay before mutation",
    );
    staged
}

/// Recompute the mutated artifact record's containing identity, require the
/// envelope to stay canonical, then require independent replay to reject the
/// substitution. `staged` is restored afterward so later mutations stay
/// independent of this one.
fn assert_artifact_field(
    staged: &mut StagedValidatedOptimizedObjectArtifact,
    original: &OptimizedObjectArtifactRecord,
    field: &str,
    mutate: impl Fn(&mut OptimizedObjectArtifactRecord),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&mutated.encode()),
        Ok(mutated.clone()),
        "reauthenticated artifact field {field} must remain a canonical envelope",
    );
    *staged.artifact_mut() = mutated;
    assert_eq!(
        validate_optimized_object_artifact(staged),
        Err(OptimizedObjectArtifactError::ArtifactMismatch),
        "independent replay must reject substituted artifact field {field}",
    );
    *staged.artifact_mut() = original.clone();
}

/// Same contract for the manifest record: the mutated field still encodes
/// under its recomputed manifest identity, and replay rejects it.
fn assert_manifest_field(
    staged: &mut StagedValidatedOptimizedObjectArtifact,
    original: &OptimizedObjectArtifactManifest,
    field: &str,
    mutate: impl Fn(&mut OptimizedObjectArtifactManifest),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&mutated.encode()),
        Ok(mutated.clone()),
        "reauthenticated manifest field {field} must remain a canonical envelope",
    );
    *staged.manifest_mut().record_mut() = mutated;
    assert_eq!(
        validate_optimized_object_artifact(staged),
        Err(OptimizedObjectArtifactError::ManifestMismatch),
        "independent replay must reject substituted manifest field {field}",
    );
    *staged.manifest_mut().record_mut() = original.clone();
}

fn assert_record_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: OptimizedObjectArtifactRecordDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&encoded),
        Err(expected),
    );
}

fn assert_manifest_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: OptimizedObjectArtifactManifestDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&encoded),
        Err(expected),
    );
}

/// Structural offsets into the encoded artifact record for the debug-absent
/// fixture layout: 8-byte magic, 4-byte version, 32-byte identity, then
/// content — `psi_artifact` (32), `psi` marker (2) and fingerprint (32),
/// `obligation_ledger` (32), `proof_bundle` (32), one `debug_section` presence
/// byte, `selections` (32), `target` (1+1+8+8), `semantic_entry` (8), six
/// manifest identities (192), `object` (32), `object_container` (32), and
/// `statistics` (32).
struct RecordWireOffsets {
    vocabulary: usize,
    optional_tag: usize,
    architecture: usize,
    object_format: usize,
    semantic_entry: usize,
}

fn record_wire_offsets(encoded: &[u8]) -> RecordWireOffsets {
    let content = 8 + 4 + 32;
    let vocabulary = content + 32;
    let optional_tag = vocabulary + 2 + 32 + 32 + 32;
    let architecture = optional_tag + 1 + 32;
    let object_format = architecture + 1;
    let semantic_entry = object_format + 1 + 8 + 8;
    assert_eq!(encoded.len(), semantic_entry + 8 + 6 * 32 + 32 + 32 + 32);
    RecordWireOffsets {
        vocabulary,
        optional_tag,
        architecture,
        object_format,
        semantic_entry,
    }
}

/// Encoded-manifest layout: the same 44-byte envelope, then a one-byte stage
/// tag, `artifact` (32), `psi_artifact` (32), `psi` (34), `selections` (32),
/// `target` (18), `semantic_entry` (8), three identities (96), `statistics`
/// (32), and four single-byte unavailable markers.
struct ManifestWireOffsets {
    stage: usize,
    vocabulary: usize,
    architecture: usize,
    object_format: usize,
    semantic_entry: usize,
    unavailable: [usize; 4],
}

fn manifest_wire_offsets(encoded: &[u8]) -> ManifestWireOffsets {
    let stage = 8 + 4 + 32;
    let vocabulary = stage + 1 + 32 + 32;
    let architecture = vocabulary + 2 + 32 + 32;
    let object_format = architecture + 1;
    let semantic_entry = object_format + 1 + 8 + 8;
    let unavailable_start = semantic_entry + 8 + 32 + 32 + 32 + 32;
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
        architecture,
        object_format,
        semantic_entry,
        unavailable,
    }
}

#[test]
fn optimized_object_artifact_custody_rejects_every_one_field_substitution() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut staged = staged_artifact(target);
        let other_architecture = match staged.artifact().target.architecture {
            Architecture::X86_64 => Architecture::Aarch64,
            Architecture::Aarch64 => Architecture::X86_64,
        };
        let other_format = match staged.artifact().target.object_format {
            ObjectFormat::Elf => ObjectFormat::MachO,
            ObjectFormat::MachO => ObjectFormat::Coff,
            ObjectFormat::Coff => ObjectFormat::Elf,
        };

        // Every representable artifact-record field mutates independently: the
        // recomputed identity keeps the envelope canonical, and replay rejects
        // the substitution against the retained terminal and object evidence.
        let original_record = staged.artifact().clone();
        assert!(
            original_record.debug_section.is_none(),
            "the wire offsets below assume the debug-absent layout",
        );
        let record_mutations: [(&str, fn(&mut OptimizedObjectArtifactRecord)); 19] = [
            ("psi_artifact", |record| record.psi_artifact = [0xa1; 32]),
            ("psi.program_fingerprint", |record| {
                record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
            }),
            ("obligation_ledger", |record| {
                record.obligation_ledger = [0xb0; 32]
            }),
            ("proof_bundle", |record| record.proof_bundle = [0xb1; 32]),
            ("debug_section", |record| {
                record.debug_section = Some([0xd6; 32])
            }),
            ("selections", |record| {
                record.selections = OptimizationSelectionIdentity::from_bytes([0x5e; 32])
            }),
            ("semantic_entry", |record| {
                record.semantic_entry = MachineId::new(913).unwrap()
            }),
            ("pre_physical_manifest", |record| {
                record.pre_physical_manifest =
                    PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(
                        b"substituted pre-physical manifest",
                    )
            }),
            ("post_allocation_manifest", |record| {
                record.post_allocation_manifest =
                    PostAllocationOptimizationManifestIdentity::from_canonical_bytes(
                        b"substituted post-allocation manifest",
                    )
            }),
            ("function_relative_manifest", |record| {
                record.function_relative_manifest =
                    FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                        b"substituted function-relative manifest",
                    )
            }),
            ("function_fragment_manifest", |record| {
                record.function_fragment_manifest =
                    FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(
                        b"substituted function-fragment manifest",
                    )
            }),
            ("text_section_manifest", |record| {
                record.text_section_manifest =
                    FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(
                        b"substituted text-section manifest",
                    )
            }),
            ("object_container_manifest", |record| {
                record.object_container_manifest =
                    FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(
                        b"substituted object-container manifest",
                    )
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
            ("statistics.text_bytes", |record| {
                record.statistics.text_bytes += 1
            }),
            ("statistics.object_container_bytes", |record| {
                record.statistics.object_container_bytes += 1
            }),
            ("statistics.function_symbols", |record| {
                record.statistics.function_symbols += 1
            }),
            ("statistics.relocation_records", |record| {
                record.statistics.relocation_records += 1
            }),
        ];
        for (field, mutate) in record_mutations {
            assert_artifact_field(&mut staged, &original_record, field, mutate);
        }
        assert_artifact_field(
            &mut staged,
            &original_record,
            "target.architecture",
            |record| record.target.architecture = other_architecture,
        );
        assert_artifact_field(
            &mut staged,
            &original_record,
            "target.object_format",
            |record| record.target.object_format = other_format,
        );
        assert_artifact_field(
            &mut staged,
            &original_record,
            "target.pointer_size",
            |record| record.target.pointer_size = 4,
        );
        assert_artifact_field(
            &mut staged,
            &original_record,
            "target.pointer_alignment",
            |record| record.target.pointer_alignment = 4,
        );

        // The containing identity itself is sealed over the content: a foreign
        // identity, or honest content drift carried under the stale identity,
        // fails the identity check inside canonical decoding and at replay.
        let mut stale = original_record.clone();
        stale.identity =
            OptimizedObjectArtifactIdentity::from_canonical_bytes(b"foreign artifact identity");
        assert_eq!(
            OptimizedObjectArtifactRecord::decode(&stale.encode()),
            Err(OptimizedObjectArtifactRecordDecodeError::IdentityMismatch),
            "{target:?}: foreign artifact identity must fail canonical decoding",
        );
        *staged.artifact_mut() = stale;
        assert_eq!(
            validate_optimized_object_artifact(&staged),
            Err(OptimizedObjectArtifactError::ArtifactMismatch),
            "{target:?}: replay must reject a foreign artifact identity",
        );
        let mut stale = original_record.clone();
        stale.statistics.text_bytes += 1;
        assert_eq!(
            OptimizedObjectArtifactRecord::decode(&stale.encode()),
            Err(OptimizedObjectArtifactRecordDecodeError::IdentityMismatch),
            "{target:?}: drifted content under a stale identity must fail decoding",
        );
        *staged.artifact_mut() = stale;
        assert_eq!(
            validate_optimized_object_artifact(&staged),
            Err(OptimizedObjectArtifactError::ArtifactMismatch),
            "{target:?}: replay must reject drifted content under a stale identity",
        );
        *staged.artifact_mut() = original_record.clone();

        // Every representable manifest field follows the same contract under
        // its own recomputed identity.
        let original_manifest = staged.manifest().record().clone();
        let manifest_mutations: [(&str, fn(&mut OptimizedObjectArtifactManifest)); 12] = [
            ("artifact", |record| {
                record.artifact =
                    OptimizedObjectArtifactIdentity::from_canonical_bytes(b"substituted artifact")
            }),
            ("psi_artifact", |record| record.psi_artifact = [0xa1; 32]),
            ("psi.program_fingerprint", |record| {
                record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
            }),
            ("selections", |record| {
                record.selections = OptimizationSelectionIdentity::from_bytes([0x5e; 32])
            }),
            ("semantic_entry", |record| {
                record.semantic_entry = MachineId::new(913).unwrap()
            }),
            ("object_container_manifest", |record| {
                record.object_container_manifest =
                    FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(
                        b"substituted object-container manifest",
                    )
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
            ("statistics.text_bytes", |record| {
                record.statistics.text_bytes += 1
            }),
            ("statistics.object_container_bytes", |record| {
                record.statistics.object_container_bytes += 1
            }),
            ("statistics.function_symbols", |record| {
                record.statistics.function_symbols += 1
            }),
            ("statistics.relocation_records", |record| {
                record.statistics.relocation_records += 1
            }),
        ];
        for (field, mutate) in manifest_mutations {
            assert_manifest_field(&mut staged, &original_manifest, field, mutate);
        }
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

        let mut stale = original_manifest.clone();
        stale.identity = OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(
            b"foreign manifest identity",
        );
        assert_eq!(
            OptimizedObjectArtifactManifest::decode(&stale.encode()),
            Err(OptimizedObjectArtifactManifestDecodeError::IdentityMismatch),
            "{target:?}: foreign manifest identity must fail canonical decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            validate_optimized_object_artifact(&staged),
            Err(OptimizedObjectArtifactError::ManifestMismatch),
            "{target:?}: replay must reject a foreign manifest identity",
        );
        let mut stale = original_manifest.clone();
        stale.statistics.function_symbols += 1;
        assert_eq!(
            OptimizedObjectArtifactManifest::decode(&stale.encode()),
            Err(OptimizedObjectArtifactManifestDecodeError::IdentityMismatch),
            "{target:?}: drifted content under a stale identity must fail decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            validate_optimized_object_artifact(&staged),
            Err(OptimizedObjectArtifactError::ManifestMismatch),
            "{target:?}: replay must reject drifted manifest content under a stale identity",
        );
        *staged.manifest_mut().record_mut() = original_manifest.clone();

        // The custody receipt fields have no independent wire form; each is
        // substituted in place and replay rejects every one.
        let receipt_mutations: [(
            &str,
            fn(&mut StagedValidatedOptimizedObjectArtifact),
        ); 6] = [
            (
                "psi_artifact",
                StagedValidatedOptimizedObjectArtifact::corrupt_custody_psi_artifact_for_test,
            ),
            (
                "object_container_manifest",
                StagedValidatedOptimizedObjectArtifact::corrupt_custody_object_container_manifest_for_test,
            ),
            (
                "object",
                StagedValidatedOptimizedObjectArtifact::corrupt_custody_object_for_test,
            ),
            (
                "object_container",
                StagedValidatedOptimizedObjectArtifact::corrupt_custody_object_container_for_test,
            ),
            (
                "artifact",
                StagedValidatedOptimizedObjectArtifact::corrupt_custody_artifact_for_test,
            ),
            (
                "manifest",
                StagedValidatedOptimizedObjectArtifact::corrupt_custody_manifest_for_test,
            ),
        ];
        for (field, corrupt) in receipt_mutations {
            let mut substituted = staged_artifact(target);
            corrupt(&mut substituted);
            assert_eq!(
                validate_optimized_object_artifact(&substituted),
                Err(OptimizedObjectArtifactError::ReceiptMismatch),
                "{target:?}: replay must reject substituted custody receipt field {field}",
            );
        }

        // Closed wire axes: single-variant stage/unavailable markers, unknown
        // vocabulary/architecture/format tags, a zero machine identity, an
        // unknown optional-section tag, envelope drift, and truncation all fail
        // canonical decoding before any custody decision. `pointer_size` and
        // `pointer_alignment` accept every u64 on 64-bit hosts, so their
        // mutations above are representable substitutions rather than wire
        // axes; the TargetLayoutOverflow arm remains 32-bit-only.
        let record_bytes = staged.artifact().encode();
        let record_offsets = record_wire_offsets(&record_bytes);
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[0] ^= 1,
            OptimizedObjectArtifactRecordDecodeError::WrongMagic,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[8..12].copy_from_slice(&99_u32.to_le_bytes()),
            OptimizedObjectArtifactRecordDecodeError::UnsupportedVersion(99),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[12] ^= 1,
            OptimizedObjectArtifactRecordDecodeError::IdentityMismatch,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[44] ^= 1,
            OptimizedObjectArtifactRecordDecodeError::IdentityMismatch,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| {
                bytes[record_offsets.vocabulary..record_offsets.vocabulary + 2]
                    .copy_from_slice(&108_u16.to_le_bytes())
            },
            OptimizedObjectArtifactRecordDecodeError::UnknownVocabulary(108),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.optional_tag] = 2,
            OptimizedObjectArtifactRecordDecodeError::UnknownOptionalTag(2),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.architecture] = 9,
            OptimizedObjectArtifactRecordDecodeError::UnknownArchitecture(9),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.object_format] = 9,
            OptimizedObjectArtifactRecordDecodeError::UnknownObjectFormat(9),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.semantic_entry..record_offsets.semantic_entry + 8].fill(0),
            OptimizedObjectArtifactRecordDecodeError::InvalidMachine,
        );
        let mut trailing = record_bytes.clone();
        trailing.push(0);
        assert_eq!(
            OptimizedObjectArtifactRecord::decode(&trailing),
            Err(OptimizedObjectArtifactRecordDecodeError::TrailingBytes),
            "{target:?}: trailing artifact bytes must reject",
        );
        assert_eq!(
            OptimizedObjectArtifactRecord::decode(&record_bytes[..record_bytes.len() - 1]),
            Err(OptimizedObjectArtifactRecordDecodeError::Truncated),
            "{target:?}: truncated artifact content must reject",
        );

        let manifest_bytes = staged.manifest().record().encode();
        let manifest_offsets = manifest_wire_offsets(&manifest_bytes);
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[0] ^= 1,
            OptimizedObjectArtifactManifestDecodeError::WrongMagic,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[8..12].copy_from_slice(&99_u32.to_le_bytes()),
            OptimizedObjectArtifactManifestDecodeError::UnsupportedVersion(99),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[12] ^= 1,
            OptimizedObjectArtifactManifestDecodeError::IdentityMismatch,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.stage + 1] ^= 1,
            OptimizedObjectArtifactManifestDecodeError::IdentityMismatch,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.stage] = 9,
            OptimizedObjectArtifactManifestDecodeError::UnknownStage(9),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.vocabulary..manifest_offsets.vocabulary + 2]
                    .copy_from_slice(&108_u16.to_le_bytes())
            },
            OptimizedObjectArtifactManifestDecodeError::Artifact(
                OptimizedObjectArtifactRecordDecodeError::UnknownVocabulary(108),
            ),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.architecture] = 9,
            OptimizedObjectArtifactManifestDecodeError::Artifact(
                OptimizedObjectArtifactRecordDecodeError::UnknownArchitecture(9),
            ),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.object_format] = 9,
            OptimizedObjectArtifactManifestDecodeError::Artifact(
                OptimizedObjectArtifactRecordDecodeError::UnknownObjectFormat(9),
            ),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.semantic_entry..manifest_offsets.semantic_entry + 8].fill(0)
            },
            OptimizedObjectArtifactManifestDecodeError::Artifact(
                OptimizedObjectArtifactRecordDecodeError::InvalidMachine,
            ),
        );
        for offset in manifest_offsets.unavailable {
            assert_manifest_decode_error(
                &manifest_bytes,
                |bytes| bytes[offset] = 9,
                OptimizedObjectArtifactManifestDecodeError::UnknownUnavailableStatus,
            );
        }
        let mut trailing = manifest_bytes.clone();
        trailing.push(0);
        assert_eq!(
            OptimizedObjectArtifactManifest::decode(&trailing),
            Err(OptimizedObjectArtifactManifestDecodeError::TrailingBytes),
            "{target:?}: trailing manifest bytes must reject",
        );
        assert_eq!(
            OptimizedObjectArtifactManifest::decode(&manifest_bytes[..manifest_bytes.len() - 1]),
            Err(OptimizedObjectArtifactManifestDecodeError::Artifact(
                OptimizedObjectArtifactRecordDecodeError::Truncated,
            )),
            "{target:?}: truncated manifest content must reject",
        );

        // The restored staged artifact still replays honestly after every
        // substitution above was rolled back.
        assert_eq!(
            validate_optimized_object_artifact(&staged),
            Ok(staged.custody()),
            "{target:?}: the restored staged artifact must replay after the matrix",
        );
    }
}
