//! Optimized ordinary-callable-entry custody mutation coverage.
//!
//! `native_artifact::stage_validated_optimized_ordinary_callable_entry` seals
//! one source-free [`OptimizedOrdinaryCallableEntryRecord`], its
//! [`OptimizedOrdinaryCallableEntryManifest`], and the in-memory custody
//! receipt over a retained validated object artifact. Independent replay
//! (`validate_optimized_ordinary_callable_entry`) re-derives all three from
//! that retained evidence rather than trusting the presented records, so this
//! family is exercised here — beneath the real pipeline that produces the
//! staged custody — instead of inside `native-artifact` where no honest staged
//! fixture exists.
//!
//! Every representable field of each record is mutated independently. A field
//! that still encodes canonically under an honestly recomputed containing
//! identity must be rejected by replay; a field whose value is closed by the
//! representation (single-variant stage/disposition/unavailable markers,
//! position-bound parameter ordinals, non-declared target pairs, qualified
//! result declarations) is mutated on the wire or at encoding and must fail
//! before any custody decision. Stale containing identities fail the identity
//! check inside canonical decoding.

use calling_conventions::{CallingPolicy, MachineRegister, ValueShape};
use machine_code::{
    WholeFunctionEntryAssumption, WholeFunctionExitContractIdentity, WholeFunctionExitPolicy,
};
use native_artifact::{
    OptimizedOrdinaryCallableEntryDecodeError, OptimizedOrdinaryCallableEntryError,
    OptimizedOrdinaryCallableEntryManifest, OptimizedOrdinaryCallableEntryManifestDecodeError,
    OptimizedOrdinaryCallableEntryRecord, StagedValidatedOptimizedOrdinaryCallableEntry,
    validate_optimized_ordinary_callable_entry,
};
use object_file::{ObjectLocalSymbolId, stage_validated_optimized_object_artifact};
use optimization_core::{
    OptimizationSelectionIdentity, OptimizationSelections, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifestIdentity, OptimizedOrdinaryCallableEntryManifestIdentity,
    OptimizedTerminalOrdinaryCallableEntryIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity,
};
use proof_admission::AdmissionProfile;
use register_homes::RegisterHomeIdentity;
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
};
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId,
    ScalarQualificationSetId, ScalarType, ValueId,
};
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ProofBundle,
    SemanticFingerprint, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};

/// One machine taking two `u64` parameters, mixing them, and returning the
/// result: the smallest module that still seals a callable entry with a
/// non-empty parameter roster, so every mutation below exercises real retained
/// custody rather than a fabricated record.
fn callable_module() -> TerminalModule {
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id: u64| ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: u64_type,
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
            parameters: vec![declaration(1), declaration(2)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(4)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(1).unwrap(),
                    result: OperationResult::Scalar(declaration(3)),
                    kind: OperationKind::IntegerBitwiseXor {
                        left: ValueId::new(1).unwrap(),
                        right: ValueId::new(2).unwrap(),
                    },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(2).unwrap(),
                    value: ValueId::new(3).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(1).unwrap(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
                crash_routes: Vec::new(),
            },
        }],
    }
}

/// Stage the parameterized module through the real optimized pipeline,
/// fragment emission, object container, and the artifact and callable-entry
/// custody joins for `target`. The returned staged entry replays cleanly
/// before any mutation.
fn staged_callable_entry(target: NativeTarget) -> StagedValidatedOptimizedOrdinaryCallableEntry {
    let module = callable_module();
    let proof = ProofBundle::default();
    let semantic = terminal_codec::encode_module(&module).expect("encode callable module");
    let proof_bytes =
        terminal_codec::encode_proof_section(&module, &proof).expect("encode callable proof");
    let optimized = native_realization::optimize_artifact_sections(
        &semantic,
        &proof_bytes,
        &AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
    )
    .expect("callable module optimizes");
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .expect("callable module reaches physical staging");
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
    let artifact = stage_validated_optimized_object_artifact(terminal, source)
        .expect("validated object artifact");
    let staged = native_artifact::stage_validated_optimized_ordinary_callable_entry(artifact)
        .expect("validated ordinary callable entry");
    assert_eq!(
        staged.entry().parameters.len(),
        2,
        "{target:?}: the fixture must carry both entry parameters",
    );
    assert_eq!(
        staged.entry().returns.len(),
        1,
        "{target:?}: the fixture must carry the single return",
    );
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Ok(staged.custody()),
        "{target:?}: honest staged entry must replay before mutation",
    );
    staged
}

/// Recompute the mutated entry record's containing identity, require the
/// envelope to stay canonical, then require independent replay to reject the
/// substitution. `staged` is restored afterward so later mutations stay
/// independent of this one.
fn assert_entry_field(
    staged: &mut StagedValidatedOptimizedOrdinaryCallableEntry,
    original: &OptimizedOrdinaryCallableEntryRecord,
    field: &str,
    mutate: impl Fn(&mut OptimizedOrdinaryCallableEntryRecord),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated
        .recomputed_identity()
        .expect("recomputed entry identity");
    let encoded = mutated.encode().expect("mutated entry still encodes");
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&encoded),
        Ok(mutated.clone()),
        "reauthenticated entry field {field} must remain a canonical envelope",
    );
    *staged.entry_mut() = mutated;
    assert_eq!(
        validate_optimized_ordinary_callable_entry(staged),
        Err(OptimizedOrdinaryCallableEntryError::RecordMismatch),
        "independent replay must reject substituted entry field {field}",
    );
    *staged.entry_mut() = original.clone();
}

/// Same contract for the manifest record: the mutated field still encodes
/// under its recomputed manifest identity, and replay rejects it.
fn assert_manifest_field(
    staged: &mut StagedValidatedOptimizedOrdinaryCallableEntry,
    original: &OptimizedOrdinaryCallableEntryManifest,
    field: &str,
    mutate: impl Fn(&mut OptimizedOrdinaryCallableEntryManifest),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&mutated.encode()),
        Ok(mutated.clone()),
        "reauthenticated manifest field {field} must remain a canonical envelope",
    );
    *staged.manifest_mut().record_mut() = mutated;
    assert_eq!(
        validate_optimized_ordinary_callable_entry(staged),
        Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch),
        "independent replay must reject substituted manifest field {field}",
    );
    *staged.manifest_mut().record_mut() = original.clone();
}

/// A field whose substituted value cannot live inside a canonical envelope:
/// the record still encodes, but canonical decoding rejects it before any
/// custody decision.
fn assert_entry_field_not_canonical(
    original: &OptimizedOrdinaryCallableEntryRecord,
    field: &str,
    expected: OptimizedOrdinaryCallableEntryDecodeError,
    mutate: impl Fn(&mut OptimizedOrdinaryCallableEntryRecord),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated
        .recomputed_identity()
        .expect("recomputed entry identity");
    let encoded = mutated.encode().expect("mutated entry still encodes");
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&encoded),
        Err(expected),
        "non-canonical entry field {field} must fail decoding",
    );
}

/// Same closed-field contract for the manifest record under its recomputed
/// identity: canonical decoding rejects the substitution before custody.
fn assert_manifest_field_not_canonical(
    original: &OptimizedOrdinaryCallableEntryManifest,
    field: &str,
    expected: OptimizedOrdinaryCallableEntryManifestDecodeError,
    mutate: impl Fn(&mut OptimizedOrdinaryCallableEntryManifest),
) {
    let mut mutated = original.clone();
    mutate(&mut mutated);
    mutated.identity = mutated.recomputed_identity();
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&mutated.encode()),
        Err(expected),
        "non-canonical manifest field {field} must fail decoding",
    );
}

fn assert_record_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: OptimizedOrdinaryCallableEntryDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&encoded),
        Err(expected),
    );
}

fn assert_manifest_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: OptimizedOrdinaryCallableEntryManifestDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&encoded),
        Err(expected),
    );
}
/// Structural offsets into the encoded entry record: 8-byte magic, 4-byte
/// version, 32-byte identity, then content — `source_artifact` (32),
/// `source_manifest` (32), `psi` marker (2) and fingerprint (32), `selections`
/// (32), `target` (1+1+8+8), `semantic_entry` (8), six identity fields (192),
/// `semantic_entry_symbol` (8), the counted symbol name, section offset (8),
/// byte count (8), `calling_policy` (1), the counted parameter rows, the
/// result declaration and placement, the counted return rows, `exit_policy`
/// (1), the closed hardening byte, `entry_assumption`, `stack_pointer` (2),
/// `stack_alignment` (2), `red_zone_bytes` (2), and the closed disposition
/// byte. Variable parts are walked from the staged record's real lengths.
struct RecordWireOffsets {
    vocabulary: usize,
    architecture: usize,
    object_format: usize,
    semantic_entry: usize,
    semantic_entry_symbol: usize,
    symbol_name: usize,
    calling_policy: usize,
    parameter_ordinal: usize,
    parameter_scalar: usize,
    parameter_register: usize,
    second_parameter_ordinal: usize,
    result_scalar: usize,
    return_edge: usize,
    exit_policy: usize,
    hardening: usize,
    entry_assumption: usize,
    disposition: usize,
}

fn record_wire_offsets(
    encoded: &[u8],
    record: &OptimizedOrdinaryCallableEntryRecord,
) -> RecordWireOffsets {
    let scalar_len = |scalar: ScalarType| match scalar {
        ScalarType::Boolean => 1,
        ScalarType::Integer(_) => 5,
        ScalarType::IeeeFloat(_) => 2,
    };
    let mut cursor = 8 + 4 + 32 + 32 + 32;
    let vocabulary = cursor;
    cursor += 2 + 32 + 32; // fingerprint + selections
    let architecture = cursor;
    let object_format = architecture + 1;
    cursor = object_format + 1 + 8 + 8;
    let semantic_entry = cursor;
    cursor += 8 + 6 * 32;
    let semantic_entry_symbol = cursor;
    cursor += 8 + 8;
    let symbol_name = cursor;
    cursor += record.semantic_entry_symbol_name.len() + 8 + 8;
    let calling_policy = cursor;
    cursor += 1 + 8;
    let parameter_ordinal = cursor;
    let parameter_scalar = parameter_ordinal + 8 + 8;
    let parameter_register =
        parameter_scalar + scalar_len(record.parameters[0].scalar_type) + 5 + 4 + 2;
    cursor += 8
        + 8
        + scalar_len(record.parameters[0].scalar_type)
        + 5
        + 4
        + 2
        + 2
        + 2
        + 2
        + 8
        + 2 * record.parameters[0].storage_units.len();
    let second_parameter_ordinal = cursor;
    cursor += 8
        + 8
        + scalar_len(record.parameters[1].scalar_type)
        + 5
        + 4
        + 2
        + 2
        + 2
        + 2
        + 8
        + 2 * record.parameters[1].storage_units.len();
    let result_scalar = cursor + 8;
    cursor += 8
        + scalar_len(record.result.declaration.scalar_type)
        + 5
        + 2
        + 2
        + 8
        + 2 * record.result.storage_units.len();
    cursor += 8;
    let return_edge = cursor;
    for returned in &record.returns {
        cursor += 8 + 8 + 4 + 4 + 2 + 8 + 2 * returned.storage_units.len();
    }
    let exit_policy = cursor;
    let hardening = exit_policy + 1;
    let entry_assumption = hardening + 1;
    cursor = entry_assumption
        + match record.entry_assumption {
            WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => 1,
            WholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. } => 3,
        }
        + 2
        + 2
        + 2;
    let disposition = cursor;
    cursor += 1;
    assert_eq!(
        cursor,
        encoded.len(),
        "the walked record layout must consume the whole envelope"
    );
    RecordWireOffsets {
        vocabulary,
        architecture,
        object_format,
        semantic_entry,
        semantic_entry_symbol,
        symbol_name,
        calling_policy,
        parameter_ordinal,
        parameter_scalar,
        parameter_register,
        second_parameter_ordinal,
        result_scalar,
        return_edge,
        exit_policy,
        hardening,
        entry_assumption,
        disposition,
    }
}

/// Encoded-manifest layout: the same 44-byte envelope, then a one-byte stage
/// tag, `entry`, `source_artifact`, `source_manifest` (96), `psi` (34),
/// `selections` (32), `target` (18), `semantic_entry` (8),
/// `semantic_entry_symbol` (8), `exit_contract` (32), `parameter_count` (8),
/// `return_count` (8), the closed disposition byte, and five single-byte
/// unavailable markers.
struct ManifestWireOffsets {
    stage: usize,
    vocabulary: usize,
    architecture: usize,
    object_format: usize,
    semantic_entry: usize,
    semantic_entry_symbol: usize,
    disposition: usize,
    unavailable: [usize; 5],
}

fn manifest_wire_offsets(encoded: &[u8]) -> ManifestWireOffsets {
    let stage = 8 + 4 + 32;
    let vocabulary = stage + 1 + 32 + 32 + 32;
    let architecture = vocabulary + 2 + 32 + 32;
    let object_format = architecture + 1;
    let semantic_entry = object_format + 1 + 8 + 8;
    let semantic_entry_symbol = semantic_entry + 8;
    let disposition = semantic_entry_symbol + 8 + 32 + 8 + 8;
    let unavailable = [
        disposition + 1,
        disposition + 2,
        disposition + 3,
        disposition + 4,
        disposition + 5,
    ];
    assert_eq!(encoded.len(), unavailable[4] + 1);
    ManifestWireOffsets {
        stage,
        vocabulary,
        architecture,
        object_format,
        semantic_entry,
        semantic_entry_symbol,
        disposition,
        unavailable,
    }
}

/// The alternate architecture keeping the `(architecture, object_format)`
/// pair declared: `Elf` accepts both architectures, `MachO` only `Aarch64`,
/// and `Coff` only `X86_64`.
fn representable_architecture(target: NativeTarget) -> Option<Architecture> {
    let other = match target.architecture {
        Architecture::X86_64 => Architecture::Aarch64,
        Architecture::Aarch64 => Architecture::X86_64,
    };
    matches!(
        (other, target.object_format),
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::Coff)
            | (
                Architecture::Aarch64,
                ObjectFormat::Elf | ObjectFormat::MachO
            )
    )
    .then_some(other)
}

/// An alternate object format keeping the pair declared for the target's
/// architecture.
fn representable_format(target: NativeTarget) -> ObjectFormat {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => ObjectFormat::Coff,
        (Architecture::X86_64, _) => ObjectFormat::Elf,
        (Architecture::Aarch64, ObjectFormat::MachO) => ObjectFormat::Elf,
        (Architecture::Aarch64, _) => ObjectFormat::MachO,
    }
}

/// An object format the target's architecture does not declare: `MachO` is
/// never an `X86_64` format and `Coff` is never an `Aarch64` format.
fn undeclared_format(target: NativeTarget) -> ObjectFormat {
    match target.architecture {
        Architecture::X86_64 => ObjectFormat::MachO,
        Architecture::Aarch64 => ObjectFormat::Coff,
    }
}
#[test]
fn optimized_ordinary_callable_entry_custody_rejects_every_one_field_substitution() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut staged = staged_callable_entry(target);
        let other_policy = match staged.entry().calling_policy {
            CallingPolicy::SystemVAMD64 => CallingPolicy::MicrosoftX64,
            _ => CallingPolicy::SystemVAMD64,
        };
        let other_exit_policy = match staged.entry().exit_policy {
            WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1 => {
                WholeFunctionExitPolicy::MicrosoftX64CanonicalFixedFrameV1
            }
            _ => WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1,
        };
        let other_assumption = match staged.entry().entry_assumption {
            WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => {
                WholeFunctionEntryAssumption::CallerLinkRegisterV1 {
                    link_register: RegisterViewId(0x7ffe),
                }
            }
            WholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. } => {
                WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
            }
        };
        let u32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());

        // Every representable entry-record field mutates independently: the
        // recomputed identity keeps the envelope canonical, and replay rejects
        // the substitution against the retained object-artifact custody.
        let original_record = staged.entry().clone();
        let record_mutations: [(&str, fn(&mut OptimizedOrdinaryCallableEntryRecord)); 18] = [
            ("source_artifact", |record| {
                record.source_artifact = OptimizedObjectArtifactIdentity::from_canonical_bytes(
                    b"substituted source artifact",
                )
            }),
            ("source_manifest", |record| {
                record.source_manifest =
                    OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(
                        b"substituted source manifest",
                    )
            }),
            ("psi.program_fingerprint", |record| {
                record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
            }),
            ("selections", |record| {
                record.selections = OptimizationSelectionIdentity::from_bytes([0x5e; 32])
            }),
            ("semantic_entry", |record| {
                record.semantic_entry = MachineId::new(913).unwrap()
            }),
            ("selected", |record| {
                record.selected =
                    SelectedInstructionPlanIdentity::from_canonical_bytes(b"substituted selected")
            }),
            ("register_homes", |record| {
                record.register_homes = RegisterHomeIdentity::from_bytes([0x72; 32])
            }),
            ("physical_register_model", |record| {
                record.physical_register_model =
                    PhysicalRegisterModelIdentity::from_bytes([0x70; 32])
            }),
            ("exit_contract", |record| {
                record.exit_contract = WholeFunctionExitContractIdentity::from_bytes([0xe7; 32])
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
            ("semantic_entry_symbol", |record| {
                record.semantic_entry_symbol = ObjectLocalSymbolId::new(0x5157).unwrap()
            }),
            ("semantic_entry_symbol_name", |record| {
                record.semantic_entry_symbol_name = "forged.entry".to_string()
            }),
            ("semantic_entry_section_offset", |record| {
                record.semantic_entry_section_offset += 8
            }),
            ("semantic_entry_byte_count", |record| {
                record.semantic_entry_byte_count += 8
            }),
            ("stack_pointer", |record| {
                record.stack_pointer = RegisterViewId(record.stack_pointer.0 ^ 0x4000)
            }),
            ("stack_alignment", |record| {
                record.stack_alignment = record.stack_alignment.wrapping_add(8)
            }),
            ("red_zone_bytes", |record| {
                record.red_zone_bytes = record.red_zone_bytes.wrapping_add(1)
            }),
        ];
        for (field, mutate) in record_mutations {
            assert_entry_field(&mut staged, &original_record, field, mutate);
        }
        // Target-dependent substitutions capture the chosen alternates, so
        // they run beside the plain matrix instead of inside it.
        assert_entry_field(&mut staged, &original_record, "calling_policy", |record| {
            record.calling_policy = other_policy
        });
        assert_entry_field(&mut staged, &original_record, "exit_policy", |record| {
            record.exit_policy = other_exit_policy
        });
        assert_entry_field(
            &mut staged,
            &original_record,
            "entry_assumption",
            |record| record.entry_assumption = other_assumption,
        );

        // The target stays canonical only while the substituted field keeps a
        // declared (architecture, object_format) pair and 8-byte pointers.
        if let Some(other_architecture) = representable_architecture(target) {
            assert_entry_field(
                &mut staged,
                &original_record,
                "target.architecture",
                |record| record.target.architecture = other_architecture,
            );
        } else {
            assert_entry_field_not_canonical(
                &original_record,
                "target.architecture",
                OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
                |record| {
                    record.target.architecture = match record.target.architecture {
                        Architecture::X86_64 => Architecture::Aarch64,
                        Architecture::Aarch64 => Architecture::X86_64,
                    }
                },
            );
        }
        assert_entry_field(
            &mut staged,
            &original_record,
            "target.object_format",
            |record| record.target.object_format = representable_format(target),
        );
        assert_entry_field_not_canonical(
            &original_record,
            "target.(undeclared pair)",
            OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
            |record| record.target.object_format = undeclared_format(target),
        );
        assert_entry_field_not_canonical(
            &original_record,
            "target.pointer_size",
            OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
            |record| record.target.pointer_size = 4,
        );
        assert_entry_field_not_canonical(
            &original_record,
            "target.pointer_alignment",
            OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
            |record| record.target.pointer_alignment = 4,
        );

        // Each parameter row's representable fields substitute independently;
        // ordinals are position-bound, so an ordinal substitution or a raw
        // dropped/duplicated row cannot keep a canonical envelope.
        for index in 0..original_record.parameters.len() {
            let parameter_mutations: [(&str, fn(&mut OptimizedOrdinaryCallableEntryRecord, usize));
                8] = [
                ("value", |record, index| {
                    record.parameters[index].value = ValueId::new(0x51 + index as u64).unwrap()
                }),
                ("shape", |record, index| {
                    record.parameters[index].shape = ValueShape::integer(4, 4)
                }),
                ("virtual_register", |record, index| {
                    record.parameters[index].virtual_register =
                        VirtualRegisterId(u32::MAX - 1 - index as u32)
                }),
                ("class", |record, index| {
                    record.parameters[index].class =
                        RegisterClassId(record.parameters[index].class.0 ^ 0x4000)
                }),
                ("abi_register", |record, index| {
                    record.parameters[index].abi_register = match record.target.architecture {
                        Architecture::X86_64 => MachineRegister::X86Rbx,
                        Architecture::Aarch64 => MachineRegister::Aarch64X(15),
                    }
                }),
                ("fixed_view", |record, index| {
                    record.parameters[index].fixed_view =
                        RegisterViewId(record.parameters[index].fixed_view.0 ^ 0x4000)
                }),
                ("assigned_view", |record, index| {
                    record.parameters[index].assigned_view =
                        RegisterViewId(record.parameters[index].assigned_view.0 ^ 0x4000)
                }),
                ("storage_units", |record, index| {
                    record.parameters[index]
                        .storage_units
                        .push(RegisterUnitId(0x7f00 + index as u16))
                }),
            ];
            for (field, mutate) in parameter_mutations {
                let field_name = format!("parameters[{index}].{field}");
                assert_entry_field(&mut staged, &original_record, &field_name, |record| {
                    mutate(record, index)
                });
            }
            let field_name = format!("parameters[{index}].scalar_type");
            assert_entry_field(&mut staged, &original_record, &field_name, |record| {
                record.parameters[index].scalar_type = u32_type
            });
        }
        assert_entry_field(
            &mut staged,
            &original_record,
            "parameters (reordered)",
            |record| {
                record.parameters.reverse();
                for (ordinal, parameter) in record.parameters.iter_mut().enumerate() {
                    parameter.ordinal = ordinal as u64;
                }
            },
        );
        assert_entry_field_not_canonical(
            &original_record,
            "parameters[0].ordinal",
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
            |record| record.parameters[0].ordinal = 7,
        );
        assert_entry_field_not_canonical(
            &original_record,
            "parameters (dropped row)",
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
            |record| {
                record.parameters.remove(0);
            },
        );
        assert_entry_field_not_canonical(
            &original_record,
            "parameters (duplicated row)",
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
            |record| {
                let row = record.parameters[0].clone();
                record.parameters.push(row);
            },
        );

        // The result declaration and placement substitute independently; a
        // qualified result cannot even encode — the admission rule that keeps
        // installed interfaces bare rejects it before the wire.
        let result_mutations: [(&str, fn(&mut OptimizedOrdinaryCallableEntryRecord)); 5] = [
            ("result.declaration.id", |record| {
                record.result.declaration.id = ValueId::new(0x51).unwrap()
            }),
            ("result.shape", |record| {
                record.result.shape = ValueShape::integer(4, 4)
            }),
            ("result.abi_register", |record| {
                record.result.abi_register = match record.target.architecture {
                    Architecture::X86_64 => MachineRegister::X86Rbx,
                    Architecture::Aarch64 => MachineRegister::Aarch64X(15),
                }
            }),
            ("result.view", |record| {
                record.result.view = RegisterViewId(record.result.view.0 ^ 0x4000)
            }),
            ("result.storage_units", |record| {
                record.result.storage_units.push(RegisterUnitId(0x7f10))
            }),
        ];
        for (field, mutate) in result_mutations {
            assert_entry_field(&mut staged, &original_record, field, mutate);
        }
        assert_entry_field(
            &mut staged,
            &original_record,
            "result.declaration.scalar_type",
            |record| record.result.declaration.scalar_type = u32_type,
        );
        let mut qualified = original_record.clone();
        qualified.result.declaration.qualifications = ScalarQualificationSetId::new(7);
        assert_eq!(
            qualified.recomputed_identity(),
            Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature),
            "{target:?}: a qualified result declaration must fail the entry qualification check",
        );
        assert_eq!(
            qualified.encode().map(|_| ()),
            Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature),
            "{target:?}: a qualified result declaration must fail canonical encoding",
        );

        // Each return row's representable fields substitute independently; the
        // roster itself can shrink or grow and still encode, but replay
        // recomputes the exact return evidence from the retained custody.
        let return_mutations: [(&str, fn(&mut OptimizedOrdinaryCallableEntryRecord)); 6] = [
            ("returns[0].edge", |record| {
                record.returns[0].edge = EdgeId::new(0x51).unwrap()
            }),
            ("returns[0].value", |record| {
                record.returns[0].value = ValueId::new(0x51).unwrap()
            }),
            ("returns[0].selected_instruction", |record| {
                record.returns[0].selected_instruction = SelectedInstructionId(u32::MAX - 1)
            }),
            ("returns[0].virtual_register", |record| {
                record.returns[0].virtual_register = VirtualRegisterId(u32::MAX - 1)
            }),
            ("returns[0].view", |record| {
                record.returns[0].view = RegisterViewId(record.returns[0].view.0 ^ 0x4000)
            }),
            ("returns[0].storage_units", |record| {
                record.returns[0].storage_units.push(RegisterUnitId(0x7f20))
            }),
        ];
        for (field, mutate) in return_mutations {
            assert_entry_field(&mut staged, &original_record, field, mutate);
        }
        assert_entry_field(
            &mut staged,
            &original_record,
            "returns (dropped row)",
            |record| {
                record.returns.clear();
            },
        );
        assert_entry_field(
            &mut staged,
            &original_record,
            "returns (duplicated row)",
            |record| {
                let row = record.returns[0].clone();
                record.returns.push(row);
            },
        );

        // The containing identity itself is sealed over the content: a foreign
        // identity, or honest content drift carried under the stale identity,
        // fails the identity check inside canonical decoding and at replay.
        let mut stale = original_record.clone();
        stale.identity = OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(
            b"foreign entry identity",
        );
        assert_eq!(
            OptimizedOrdinaryCallableEntryRecord::decode(&stale.encode().unwrap()),
            Err(OptimizedOrdinaryCallableEntryDecodeError::IdentityMismatch),
            "{target:?}: foreign entry identity must fail canonical decoding",
        );
        *staged.entry_mut() = stale;
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Err(OptimizedOrdinaryCallableEntryError::RecordMismatch),
            "{target:?}: replay must reject a foreign entry identity",
        );
        let mut stale = original_record.clone();
        stale.semantic_entry_byte_count += 8;
        assert_eq!(
            OptimizedOrdinaryCallableEntryRecord::decode(&stale.encode().unwrap()),
            Err(OptimizedOrdinaryCallableEntryDecodeError::IdentityMismatch),
            "{target:?}: drifted content under a stale identity must fail decoding",
        );
        *staged.entry_mut() = stale;
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Err(OptimizedOrdinaryCallableEntryError::RecordMismatch),
            "{target:?}: replay must reject drifted content under a stale identity",
        );
        *staged.entry_mut() = original_record.clone();

        // Every representable manifest field follows the same contract under
        // its own recomputed identity.
        let original_manifest = staged.manifest().record().clone();
        let manifest_mutations: [(&str, fn(&mut OptimizedOrdinaryCallableEntryManifest)); 10] = [
            ("entry", |record| {
                record.entry = OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(
                    b"substituted entry",
                )
            }),
            ("source_artifact", |record| {
                record.source_artifact = OptimizedObjectArtifactIdentity::from_canonical_bytes(
                    b"substituted source artifact",
                )
            }),
            ("source_manifest", |record| {
                record.source_manifest =
                    OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(
                        b"substituted source manifest",
                    )
            }),
            ("psi.program_fingerprint", |record| {
                record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
            }),
            ("selections", |record| {
                record.selections = OptimizationSelectionIdentity::from_bytes([0x5e; 32])
            }),
            ("semantic_entry", |record| {
                record.semantic_entry = MachineId::new(913).unwrap()
            }),
            ("semantic_entry_symbol", |record| {
                record.semantic_entry_symbol = ObjectLocalSymbolId::new(0x5157).unwrap()
            }),
            ("exit_contract", |record| {
                record.exit_contract = WholeFunctionExitContractIdentity::from_bytes([0xe7; 32])
            }),
            ("parameter_count", |record| record.parameter_count += 1),
            ("return_count", |record| record.return_count += 1),
        ];
        for (field, mutate) in manifest_mutations {
            assert_manifest_field(&mut staged, &original_manifest, field, mutate);
        }
        assert_manifest_field(
            &mut staged,
            &original_manifest,
            "target.object_format",
            |record| record.target.object_format = representable_format(target),
        );
        if let Some(other_architecture) = representable_architecture(target) {
            assert_manifest_field(
                &mut staged,
                &original_manifest,
                "target.architecture",
                |record| record.target.architecture = other_architecture,
            );
        } else {
            assert_manifest_field_not_canonical(
                &original_manifest,
                "target.architecture",
                OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                    OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
                ),
                |record| {
                    record.target.architecture = match record.target.architecture {
                        Architecture::X86_64 => Architecture::Aarch64,
                        Architecture::Aarch64 => Architecture::X86_64,
                    }
                },
            );
        }
        assert_manifest_field_not_canonical(
            &original_manifest,
            "target.(undeclared pair)",
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
            ),
            |record| record.target.object_format = undeclared_format(target),
        );
        assert_manifest_field_not_canonical(
            &original_manifest,
            "target.pointer_size",
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
            ),
            |record| record.target.pointer_size = 4,
        );
        assert_manifest_field_not_canonical(
            &original_manifest,
            "target.pointer_alignment",
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
            ),
            |record| record.target.pointer_alignment = 4,
        );

        let mut stale = original_manifest.clone();
        stale.identity = OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(
            b"foreign entry manifest identity",
        );
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&stale.encode()),
            Err(OptimizedOrdinaryCallableEntryManifestDecodeError::IdentityMismatch),
            "{target:?}: foreign manifest identity must fail canonical decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch),
            "{target:?}: replay must reject a foreign manifest identity",
        );
        let mut stale = original_manifest.clone();
        stale.parameter_count += 1;
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&stale.encode()),
            Err(OptimizedOrdinaryCallableEntryManifestDecodeError::IdentityMismatch),
            "{target:?}: drifted content under a stale identity must fail decoding",
        );
        *staged.manifest_mut().record_mut() = stale;
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch),
            "{target:?}: replay must reject drifted manifest content under a stale identity",
        );
        *staged.manifest_mut().record_mut() = original_manifest.clone();

        // The custody receipt fields have no independent wire form; each is
        // substituted in place and replay rejects every one.
        let receipt_mutations: [(
            &str,
            fn(&mut StagedValidatedOptimizedOrdinaryCallableEntry),
        ); 3] = [
            (
                "source_artifact",
                StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_source_artifact_for_test,
            ),
            (
                "entry",
                StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_entry_for_test,
            ),
            (
                "manifest",
                StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_manifest_for_test,
            ),
        ];
        for (field, corrupt) in receipt_mutations {
            let mut substituted = staged_callable_entry(target);
            corrupt(&mut substituted);
            assert_eq!(
                validate_optimized_ordinary_callable_entry(&substituted),
                Err(OptimizedOrdinaryCallableEntryError::ReceiptMismatch),
                "{target:?}: replay must reject substituted custody receipt field {field}",
            );
        }

        // Closed wire axes: single-variant stage/hardening/disposition markers,
        // unavailable-status bytes, unknown vocabulary/architecture/format/
        // policy/scalar/register tags, reserved zero identities, non-UTF-8
        // names, position-bound parameter ordinals, and retired exit-policy
        // tags all fail canonical decoding before any custody decision.
        let record_bytes = staged.entry().encode().expect("encode entry record");
        let record_offsets = record_wire_offsets(&record_bytes, staged.entry());
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[0] ^= 1,
            OptimizedOrdinaryCallableEntryDecodeError::WrongMagic,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[8..12].copy_from_slice(&99_u32.to_le_bytes()),
            OptimizedOrdinaryCallableEntryDecodeError::UnsupportedVersion(99),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[12] ^= 1,
            OptimizedOrdinaryCallableEntryDecodeError::IdentityMismatch,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[44] ^= 1,
            OptimizedOrdinaryCallableEntryDecodeError::IdentityMismatch,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| {
                bytes[record_offsets.vocabulary..record_offsets.vocabulary + 2]
                    .copy_from_slice(&108_u16.to_le_bytes())
            },
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.architecture] = 9,
            OptimizedOrdinaryCallableEntryDecodeError::UnknownArchitecture(9),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.object_format] = 9,
            OptimizedOrdinaryCallableEntryDecodeError::UnknownObjectFormat(9),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.semantic_entry..record_offsets.semantic_entry + 8].fill(0),
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| {
                bytes
                    [record_offsets.semantic_entry_symbol..record_offsets.semantic_entry_symbol + 8]
                    .fill(0)
            },
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.symbol_name] = 0xff,
            OptimizedOrdinaryCallableEntryDecodeError::InvalidUtf8,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.calling_policy] = 9,
            OptimizedOrdinaryCallableEntryDecodeError::UnknownCallingPolicy(9),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.parameter_ordinal] = 7,
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.second_parameter_ordinal] = 7,
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.parameter_scalar] = 9,
            OptimizedOrdinaryCallableEntryDecodeError::InvalidScalarType,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.parameter_scalar + 2] = 9,
            OptimizedOrdinaryCallableEntryDecodeError::InvalidIntegerType,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.parameter_register] = 99,
            OptimizedOrdinaryCallableEntryDecodeError::UnknownRegister(99),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.result_scalar] = 9,
            OptimizedOrdinaryCallableEntryDecodeError::InvalidScalarType,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.return_edge..record_offsets.return_edge + 8].fill(0),
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.exit_policy] = 5,
            OptimizedOrdinaryCallableEntryDecodeError::UnknownExitPolicy(5),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.hardening] = 2,
            OptimizedOrdinaryCallableEntryDecodeError::UnknownHardening(2),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.entry_assumption] = 9,
            OptimizedOrdinaryCallableEntryDecodeError::UnknownEntryAssumption(9),
        );
        assert_record_decode_error(
            &record_bytes,
            |bytes| bytes[record_offsets.disposition] = 9,
            OptimizedOrdinaryCallableEntryDecodeError::UnknownDisposition(9),
        );
        let mut trailing = record_bytes.clone();
        trailing.push(0);
        assert_eq!(
            OptimizedOrdinaryCallableEntryRecord::decode(&trailing),
            Err(OptimizedOrdinaryCallableEntryDecodeError::TrailingBytes),
            "{target:?}: trailing entry bytes must reject",
        );
        assert_eq!(
            OptimizedOrdinaryCallableEntryRecord::decode(&record_bytes[..record_bytes.len() - 1]),
            Err(OptimizedOrdinaryCallableEntryDecodeError::Truncated),
            "{target:?}: truncated entry content must reject",
        );

        let manifest_bytes = staged.manifest().record().encode();
        let manifest_offsets = manifest_wire_offsets(&manifest_bytes);
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[0] ^= 1,
            OptimizedOrdinaryCallableEntryManifestDecodeError::WrongMagic,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[8..12].copy_from_slice(&99_u32.to_le_bytes()),
            OptimizedOrdinaryCallableEntryManifestDecodeError::UnsupportedVersion(99),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[12] ^= 1,
            OptimizedOrdinaryCallableEntryManifestDecodeError::IdentityMismatch,
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.stage] = 9,
            OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownStage(9),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.vocabulary..manifest_offsets.vocabulary + 2]
                    .copy_from_slice(&108_u16.to_le_bytes())
            },
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
            ),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.architecture] = 9,
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::UnknownArchitecture(9),
            ),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.object_format] = 9,
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::UnknownObjectFormat(9),
            ),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.semantic_entry..manifest_offsets.semantic_entry + 8].fill(0)
            },
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
            ),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| {
                bytes[manifest_offsets.semantic_entry_symbol
                    ..manifest_offsets.semantic_entry_symbol + 8]
                    .fill(0)
            },
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
            ),
        );
        assert_manifest_decode_error(
            &manifest_bytes,
            |bytes| bytes[manifest_offsets.disposition] = 9,
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::UnknownDisposition(9),
            ),
        );
        for offset in manifest_offsets.unavailable {
            assert_manifest_decode_error(
                &manifest_bytes,
                |bytes| bytes[offset] = 9,
                OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownUnavailableStatus,
            );
        }
        let mut trailing = manifest_bytes.clone();
        trailing.push(0);
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&trailing),
            Err(OptimizedOrdinaryCallableEntryManifestDecodeError::TrailingBytes),
            "{target:?}: trailing manifest bytes must reject",
        );
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(
                &manifest_bytes[..manifest_bytes.len() - 1]
            ),
            Err(OptimizedOrdinaryCallableEntryManifestDecodeError::Truncated),
            "{target:?}: truncated manifest content must reject",
        );

        // The restored staged entry still replays honestly after every
        // substitution above was rolled back.
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Ok(staged.custody()),
            "{target:?}: the restored staged entry must replay after the matrix",
        );
    }
}
