//! Wrapper object record, composition, and codec contracts. The fixtures
//! build the canonical receiver-free wrapper template directly: the ISA owner
//! admits only canonical requests, so this is the same template the
//! native-realization encoding stage selects for a two-Extent free entry.

use super::validation::{validate_object, validate_object_preserving_seal};
use super::{
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectRecordError, TEST_PLAN_IDENTITY_RECOMPUTATIONS,
    ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest, WRAPPER_SYMBOL_NAME,
    compose_optimized_program_storage_semantic_wrapper_object, construct_manifest,
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object_preserving_seal,
};
use isa_x86_64::{
    ValidatedX86_64SemanticUnitWrapperTemplate,
    canonical_x86_64_semantic_unit_wrapper_encoding_request,
    encode_x86_64_semantic_unit_wrapper_template,
};
use object_file::{
    ObjectLocalSymbolId, RelocationFreeFunctionSymbol, RelocationFreeObjectPlan,
    RelocationFreeObjectRelocationRequirements, RelocationFreeObjectSymbolLinkage,
    RelocationFreeObjectSymbolPolicy, RelocationFreeObjectSymbolRole,
    RelocationFreeObjectTextSection, SectionKind, canonical_private_machine_symbol_name,
    section_name,
};
use optimization_core::{
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

mod manifest_mutation_matrix;
mod object_mutation_matrix;

fn template() -> ValidatedX86_64SemanticUnitWrapperTemplate {
    encode_x86_64_semantic_unit_wrapper_template(
        canonical_x86_64_semantic_unit_wrapper_encoding_request(NativeTarget::uefi_x64()),
    )
    .unwrap()
}

fn custody(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    container: &super::OptimizedProgramStorageSemanticWrapperObjectContainer,
    manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) -> OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt::from_records(
        object, container, manifest,
    )
}

fn child() -> RelocationFreeObjectPlan {
    let machine = MachineId::new(7).unwrap();
    let symbol = ObjectLocalSymbolId::new(1).unwrap();
    let mut child = RelocationFreeObjectPlan {
        identity: RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"pending"),
        source_text_section:
            optimization_core::TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                b"text",
            ),
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
        selections: optimization_core::OptimizationSelectionIdentity::from_bytes([6; 32]),
        target: NativeTarget::uefi_x64(),
        text_section: RelocationFreeObjectTextSection {
            name: section_name(NativeTarget::uefi_x64(), SectionKind::Text),
            alignment: 1,
            byte_count: 1,
            bytes: vec![0xc3],
        },
        symbol_policy: RelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
        symbols: vec![RelocationFreeFunctionSymbol {
            symbol,
            source_function_index: 0,
            machine,
            name: canonical_private_machine_symbol_name(machine),
            section_offset: 0,
            byte_count: 1,
            linkage: RelocationFreeObjectSymbolLinkage::ObjectLocalV1,
            role: RelocationFreeObjectSymbolRole::SemanticEntryV1,
        }],
        semantic_entry: machine,
        semantic_entry_symbol: symbol,
        normalized_imports: vec![],
        unresolved_normalized_foreign_calls: vec![],
        relocation_record_count: 0,
        relocation_requirements:
            RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    child.identity = child.recomputed_identity().unwrap();
    child
}

fn composed() -> OptimizedProgramStorageSemanticWrapperObjectPlan {
    compose_optimized_program_storage_semantic_wrapper_object(
        [5; 32],
        OptimizedObjectArtifactIdentity::from_canonical_bytes(b"artifact"),
        OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(b"manifest"),
        RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"container"),
        &child(),
        &template(),
    )
    .unwrap()
}

#[test]
fn composition_prefixes_resolved_wrapper_and_shifts_terminal_symbols() {
    let object = composed();
    assert_eq!(object.text_bytes.len(), 91);
    assert_eq!(object.text_bytes[90], 0xc3);
    assert_eq!(object.symbols.len(), 2);
    assert_eq!(object.symbols[0].machine, None);
    assert_eq!(object.symbols[0].name, WRAPPER_SYMBOL_NAME);
    assert_eq!(object.symbols[1].section_offset, 90);
    assert_eq!(object.continuation_symbol, object.symbols[1].symbol);
    assert_eq!(object.call_resolution.continuation_section_offset, 90);
    assert_eq!(object.call_resolution.displacement, 5);
    assert_eq!(object.relocation_record_count, 0);
}

#[test]
fn object_and_manifest_codecs_reject_identity_drift() {
    let object = composed();
    let container =
        encode_optimized_program_storage_semantic_wrapper_object(&object, &template()).unwrap();
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&container.bytes).unwrap(),
        object
    );
    let manifest = construct_manifest(&object, &container).unwrap();
    assert_eq!(
        OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&manifest.encode()).unwrap(),
        manifest
    );
    let mut corrupt = container.bytes.clone();
    let last = corrupt.last_mut().unwrap();
    *last ^= 1;
    assert!(decode_optimized_program_storage_semantic_wrapper_object(&corrupt).is_err());
}

#[test]
fn wrapper_cannot_be_reclassified_as_a_machine_symbol() {
    let mut object = composed();
    object.symbols[0].machine = Some(MachineId::new(99).unwrap());
    object.identity = object.recomputed_identity().unwrap();
    assert_eq!(
        validate_object(&object, &template()),
        Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::InvalidObject)
    );
}

#[test]
fn manifest_replay_detects_drift() {
    let object = composed();
    let container =
        encode_optimized_program_storage_semantic_wrapper_object(&object, &template()).unwrap();
    let mut validated = ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest::construct(
        &object, &container,
    )
    .unwrap();
    validated.record_mut().relocation_record_count = 1;
    assert!(
        OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&validated.record().encode())
            .is_err()
    );
}

#[test]
fn retained_object_identity_rejects_text_drift() {
    let mut object = composed();
    object.text_bytes[90] ^= 1;
    assert_eq!(
        validate_object(&object, &template()),
        Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::InvalidObject)
    );
}

/// The native-realization stage's object join and its trailing validator drive
/// exactly these record calls — composition (through its `construct_object`),
/// the preserving-seal encode, manifest and custody construction, then the
/// validator's recomposed `expected`, retained-object shape/template check,
/// container decode, and re-encode — on inputs this module can fabricate (the
/// settlement/source replay joins are stage records, not plan-identity work). Each produced
/// plan value — joined, replayed, wire-decoded — is sealed exactly once.
#[test]
fn staging_route_seals_each_produced_plan_exactly_once() {
    TEST_PLAN_IDENTITY_RECOMPUTATIONS.with(|count| count.set(0));

    // `stage_validated_...`: composition seals the joined plan, the
    // sealed-encoding join emits the container without reserializing it.
    let object = composed();
    let container = encode_optimized_program_storage_semantic_wrapper_object_preserving_seal(
        &object,
        &template(),
    )
    .unwrap();
    let manifest = construct_manifest(&object, &container).unwrap();
    let receipt = custody(&object, &container, &manifest);

    // `validate_optimized_...`: the honest replay recomposes and seals a fresh
    // `expected`, the retained plan's checks trust its assigned seal, the
    // container decode re-derives identity from the wire, and the re-encode
    // trusts the replayed seal.
    let expected = composed();
    validate_object_preserving_seal(&object, &template()).unwrap();
    assert_eq!(object, expected);
    let decoded =
        decode_optimized_program_storage_semantic_wrapper_object(&container.bytes).unwrap();
    let replayed_container =
        encode_optimized_program_storage_semantic_wrapper_object_preserving_seal(
            &expected,
            &template(),
        )
        .unwrap();
    assert_eq!(decoded, expected);
    assert_eq!(container, replayed_container);
    assert_eq!(receipt, custody(&expected, &replayed_container, &manifest));

    assert_eq!(
        TEST_PLAN_IDENTITY_RECOMPUTATIONS.with(|count| count.get()),
        3,
        "joined, replayed, and decoded plans are each sealed once; \
         the stage previously reserialized an unchanged plan at encode, \
         at retained-object validation, in the decode shape gate after the \
         wire check, and at the replayed re-encode",
    );
}

/// The preserving-seal joins used by the staging route trust the
/// construction-time seal — a stale identity proves they did not reserialize
/// it — while the public encode boundary keeps the full recompute and the
/// wire decode still rejects the substituted seal.
#[test]
fn preserving_seal_joins_trust_the_assigned_seal_while_boundaries_still_check_it() {
    let object = composed();
    let mut stale = object.clone();
    stale.identity =
        OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(b"stale-object");

    assert_eq!(
        encode_optimized_program_storage_semantic_wrapper_object(&stale, &template()),
        Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::InvalidObject),
        "the standalone encode boundary still recomputes the plan identity",
    );
    let container = encode_optimized_program_storage_semantic_wrapper_object_preserving_seal(
        &stale,
        &template(),
    )
    .expect("the just-sealed encode trusts the assigned seal");
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&container.bytes),
        Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch),
        "the wire boundary still rejects a substituted plan identity",
    );

    // The validator's `staged.object != expected` join rejects a stale seal
    // against the freshly recomposed plan with the same `InvalidObject` the
    // full recompute produced.
    let expected = composed();
    assert_ne!(stale, expected);
    assert_eq!(
        validate_object_preserving_seal(&stale, &template()),
        Ok(()),
        "shape and template checks are unaffected by the stale seal",
    );
}
