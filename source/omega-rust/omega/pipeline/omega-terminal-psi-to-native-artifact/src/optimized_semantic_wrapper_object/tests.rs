use super::*;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use omega_effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
};
use omega_object_file::{
    RelocationFreeFunctionSymbol, RelocationFreeObjectRelocationRequirements,
    RelocationFreeObjectSymbolLinkage, RelocationFreeObjectSymbolPolicy,
    RelocationFreeObjectTextSection,
};
use omega_program_entry_plan::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceExtentValueLayout,
    ProgramEntrySourceReceiverSignature, ProgramStorageEntryRootRole,
    SelectedProgramEntrySourceSignature, SelectedProgramStorageEntryPlan,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use psi_core::FuelScheduleIdentity;
use psi_language_semantics::{CarryPolicy, DomainPredicateBody};
use psi_symbols::SymbolHandle;

const EXTENT_SHAPE: ValueShape = ValueShape::integer(16, 8);
const WORD_SHAPE: ValueShape = ValueShape::integer(8, 8);

fn extent_layout(base: u32) -> ProgramEntrySourceExtentValueLayout {
    ProgramEntrySourceExtentValueLayout::from_checked_record(
        SymbolHandle::from_arena_index(base),
        SymbolHandle::from_arena_index(base + 1),
        0,
        WORD_SHAPE,
        SymbolHandle::from_arena_index(base + 2),
        8,
        WORD_SHAPE,
        EXTENT_SHAPE,
    )
    .unwrap()
}

fn encoding() -> StagedOptimizedProgramStorageSemanticWrapperEncoding {
    let slot = omega_target::TargetProfile::UefiX64.program_entry_slot();
    let semantic = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![EXTENT_SHAPE, EXTENT_SHAPE],
            result: None,
        },
    )
    .unwrap();
    let claim = |parameter_index| ServiceEntryClaim {
        parameter_index,
        carrier_identity: "named(name(Extent))".into(),
        domain: "Extent::Granted".into(),
        predicate_body: DomainPredicateBody::Present,
        effective_carry: CarryPolicy::STRICT,
        authority_flow: ServiceEntryAuthorityFlow::Accepts,
    };
    let storage = SelectedProgramStorageEntryPlan::from_target_slot(
        slot,
        ServiceSchema {
            trait_name: slot.boundary_schema.unwrap().into(),
            methods: vec![ServiceMethod {
                name: "enter".into(),
                requirement_owner: "ProgramStorageEntry".into(),
                requirement_identity: "ProgramStorageEntry::enter#object".into(),
                parameter_count: 2,
                parameter_type_identities: vec!["ImageExtent".into(), "StorageExtent".into()],
                entry_claims: vec![claim(0), claim(1)],
                calling_plan_fingerprint: Some(semantic.contract_fingerprint()),
                ..Default::default()
            }],
            ..Default::default()
        },
        "ProgramStorageEntry::enter#object".into(),
    )
    .unwrap();
    let pointer = ValueShape::integer(8, 8);
    let physical = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![pointer, pointer],
            result: Some(pointer),
        },
    )
    .unwrap();
    let storage = storage
        .with_physical_contract(
            ProgramEntryPhysicalContractPlan::new(
                slot,
                "UefiPhysicalEntry::enter#object".into(),
                omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
                omega_program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                    omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
                    b"optimized-semantic-wrapper-object-test-package-source",
                ),
                1,
                vec!["EfiImageHandle".into(), "&EfiSystemTable".into()],
                "EfiStatus".into(),
                physical.contract_fingerprint(),
                physical.plan().clone(),
            )
            .unwrap(),
        )
        .unwrap();
    let source = SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        slot,
        SymbolHandle::from_arena_index(1),
        SymbolHandle::from_arena_index(2),
        "Boot::launch".into(),
        "launch".into(),
        "Boot::launch#object".into(),
        ProgramEntrySourceReceiverSignature::Free,
        vec![
            SelectedProgramEntrySourceSignature::visible_parameter(
                ProgramStorageEntryRootRole::Image,
                0,
                "ImageExtent".into(),
                EXTENT_SHAPE,
                extent_layout(10),
                false,
                false,
            ),
            SelectedProgramEntrySourceSignature::visible_parameter(
                ProgramStorageEntryRootRole::InitialStorage,
                1,
                "StorageExtent".into(),
                EXTENT_SHAPE,
                extent_layout(20),
                false,
                false,
            ),
        ],
    )
    .unwrap();
    let contract = bind_optimized_program_storage_semantic_entry_contract(
        NativeTarget::uefi_x64(),
        &storage,
        &source,
        semantic.plan(),
    )
    .unwrap();
    crate::select_optimized_program_storage_semantic_wrapper_encoding(
        plan_optimized_program_storage_semantic_wrapper(contract).unwrap(),
    )
    .unwrap()
}

fn child() -> RelocationFreeObjectPlan {
    let machine = MachineId::new(7).unwrap();
    let symbol = ObjectLocalSymbolId::new(1).unwrap();
    let mut child = RelocationFreeObjectPlan {
        identity: RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"pending"),
        source_text_section:
            omega_optimization_core::TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                b"text",
            ),
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
        selections: omega_optimization_core::OptimizationSelectionIdentity::from_bytes([6; 32]),
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
        relocation_record_count: 0,
        relocation_requirements:
            RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    child.identity = child.recomputed_identity().unwrap();
    child
}

fn composed() -> OptimizedProgramStorageSemanticWrapperObjectPlan {
    compose_object(
        [5; 32],
        OptimizedObjectArtifactIdentity::from_canonical_bytes(b"artifact"),
        OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(b"manifest"),
        RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"container"),
        &child(),
        &encoding(),
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
    let container = encode_optimized_program_storage_semantic_wrapper_object(&object).unwrap();
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
        validate_object(&object),
        Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject)
    );
}

#[test]
fn manifest_replay_detects_drift() {
    let object = composed();
    let container = encode_optimized_program_storage_semantic_wrapper_object(&object).unwrap();
    let mut validated = ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
        record: construct_manifest(&object, &container).unwrap(),
    };
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
        validate_object(&object),
        Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject)
    );
}
