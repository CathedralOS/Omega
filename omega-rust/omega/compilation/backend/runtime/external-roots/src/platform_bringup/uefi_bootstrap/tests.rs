//! UEFI bootstrap tests.

use super::{
    UefiApplicationBootstrapAdapterComposition, UefiApplicationBootstrapAdapterInvocationReadiness,
    UefiApplicationBootstrapSameStackBudgetPlan, UefiApplicationBootstrapSameStackDemandComponents,
    UefiApplicationFirmwareLedger, UefiApplicationPhysicalArrival, UefiBootServicesPhaseLease,
    UefiSystemTableOccurrenceProvenance, compose_uefi_application_bootstrap_adapter,
    join_lifecycle_scoped_uefi_system_table, join_uefi_application_physical_arrival,
    plan_uefi_application_bootstrap_same_stack_budget,
    plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter,
    prepare_uefi_application_bootstrap_adapter_invocation,
};
use crate::{
    ExternalRootDiagnostic, UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId,
    UefiFirmwareSessionId, UefiImageHandleOccurrenceId, UefiPhysicalInvocationId,
    UefiSystemTableOccurrenceId,
};
use calling_conventions::{CallSignature, CallingPolicy};
use calling_conventions::{
    MachineRegister, ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
    ServiceSchema,
};
use language_semantics::{CarryPolicy, DomainPredicateBody};
use program_entry_plan::{
    OptimizedProgramStorageSemanticCallingApplication,
    OptimizedProgramStorageSemanticEntryContract, ProgramEntryPhysicalContractPlan,
    ProgramEntrySourceReceiverSignature, ProgramStorageEntryRootRole,
};
use program_entry_plan::{
    ProgramEntrySourceExtentValueLayout, SelectedProgramEntrySourceSignature,
    SelectedProgramStorageEntryPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
    UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
    UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY,
    bind_optimized_program_storage_semantic_entry_contract,
    exact_uefi_x64_physical_boundary_entry_plan,
    exact_uefi_x64_physical_contract_package_source_digest,
};
use symbols::SymbolHandle;
use target::{
    ProgramEntryPhysicalContractPackage, UEFI_SYSTEM_TABLE_SIGNATURE,
    validate_uefi_system_table_occurrence,
};
use target::{
    TargetProfile, ValidatedUefiSystemTableHeaderIntegrity, plan_uefi_system_table_native_layout,
};

const REVISION: u32 = (2 << 16) | 100;

fn id<T>(value: u64, constructor: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(value).unwrap()
}

fn ledger<'a>(base: u64) -> UefiApplicationFirmwareLedger<'a> {
    UefiApplicationFirmwareLedger::new(
        id(
            base,
            UefiApplicationBootstrapLedgerId::from_normalized_identity,
        ),
        id(base + 1, UefiFirmwareSessionId::from_normalized_identity),
        id(base + 2, UefiPhysicalInvocationId::from_normalized_identity),
    )
    .unwrap()
}

fn inputs<'a>(
    ledger: &mut UefiApplicationFirmwareLedger<'a>,
    bytes: &'a [u8],
    occurrence_id: u64,
    lease_id: u64,
) -> (
    ValidatedUefiSystemTableHeaderIntegrity<'a>,
    UefiSystemTableOccurrenceProvenance<'a>,
    UefiBootServicesPhaseLease,
) {
    let integrity = validate_uefi_system_table_occurrence(
        plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
        bytes,
    )
    .unwrap();
    let provenance = ledger
        .admit_system_table_occurrence(
            id(
                occurrence_id,
                UefiSystemTableOccurrenceId::from_normalized_identity,
            ),
            integrity.table_bytes(),
        )
        .unwrap();
    let lease = ledger
        .acquire_boot_services_phase_lease(id(
            lease_id,
            UefiBootServicesPhaseLeaseId::from_normalized_identity,
        ))
        .unwrap();
    (integrity, provenance, lease)
}

fn valid_occurrence(header_size: usize) -> Vec<u8> {
    assert!(header_size >= 120);
    let mut bytes = vec![0; header_size];
    bytes[0..8].copy_from_slice(&UEFI_SYSTEM_TABLE_SIGNATURE.to_le_bytes());
    bytes[8..12].copy_from_slice(&REVISION.to_le_bytes());
    bytes[12..16].copy_from_slice(&(header_size as u32).to_le_bytes());
    for (offset, byte) in bytes[24..].iter_mut().enumerate() {
        *byte = (offset as u8).wrapping_mul(17).wrapping_add(3);
    }
    let crc = system_table_crc32(&bytes);
    bytes[16..20].copy_from_slice(&crc.to_le_bytes());
    bytes
}

fn system_table_crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for (index, byte) in bytes.iter().copied().enumerate() {
        let byte = if (16..20).contains(&index) { 0 } else { byte };
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let low_bit_mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & low_bit_mask);
        }
    }
    !crc
}

fn physical_contract(
    requirement_identity: &str,
    mutate_plan: impl FnOnce(&mut calling_conventions::BoundaryEntryPlan),
) -> ProgramEntryPhysicalContractPlan {
    let expected = exact_uefi_x64_physical_boundary_entry_plan();
    let mut plan = expected.plan().clone();
    mutate_plan(&mut plan);
    ProgramEntryPhysicalContractPlan::new(
        TargetProfile::UefiX64.program_entry_slot(),
        requirement_identity.into(),
        ProgramEntryPhysicalContractPackage::UefiX64,
        exact_uefi_x64_physical_contract_package_source_digest(),
        0xfeed,
        vec![
            UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY.into(),
            UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY.into(),
        ],
        UEFI_X64_STATUS_TYPE_IDENTITY.into(),
        expected.contract_report_fingerprint(),
        plan,
    )
    .unwrap()
}

fn exact_physical_contract() -> ProgramEntryPhysicalContractPlan {
    physical_contract(UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, |_| {})
}

fn semantic_entry_contract(
    physical_contract: ProgramEntryPhysicalContractPlan,
) -> OptimizedProgramStorageSemanticEntryContract {
    const REQUIREMENT: &str = "ProgramStorageEntry::enter#uefi-bootstrap";
    const IMAGE_TYPE: &str = "Extent in Granted#image";
    const STORAGE_TYPE: &str = "Extent in Granted#initial-storage";
    const EXTENT_CARRIER: &str = "named(name(Extent))";
    const GRANTED_DOMAIN: &str = "Extent::Granted";

    let extent = ValueShape::integer(16, 8);
    let field = ValueShape::integer(8, 8);
    let semantic = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![extent, extent],
            result: None,
        },
    )
    .unwrap();
    // Test-local calling-plan application identity, distinct from the raw
    // validated plan's own report fingerprint and commitment digest.
    let application = OptimizedProgramStorageSemanticCallingApplication::new(
        &semantic,
        0xE55C_CC11_1901_A005,
        BoundaryCallingPlanCommitment::from_digest([0xE5; 32]),
    );
    let claim = |parameter_index| ServiceEntryClaim {
        parameter_index,
        carrier_identity: EXTENT_CARRIER.into(),
        domain: GRANTED_DOMAIN.into(),
        predicate_body: DomainPredicateBody::Present,
        effective_carry: CarryPolicy::STRICT,
        authority_flow: ServiceEntryAuthorityFlow::Accepts,
    };
    let method = ServiceMethod {
        name: "enter".into(),
        requirement_owner: "ProgramStorageEntry".into(),
        requirement_identity: REQUIREMENT.into(),
        parameter_count: 2,
        parameter_type_identities: vec![IMAGE_TYPE.into(), STORAGE_TYPE.into()],
        entry_claims: vec![claim(0), claim(1)],
        calling_plan_report_fingerprint: Some(application.report_fingerprint()),
        calling_plan_commitment: Some(application.commitment()),
        ..Default::default()
    };
    let slot = TargetProfile::UefiX64.program_entry_slot();
    let selected = SelectedProgramStorageEntryPlan::from_target_slot(
        slot,
        ServiceSchema {
            trait_name: slot.boundary_schema.unwrap().into(),
            methods: vec![method],
            ..Default::default()
        },
        REQUIREMENT.into(),
    )
    .unwrap()
    .with_physical_contract(physical_contract)
    .unwrap();
    let extent_layout = |base| {
        ProgramEntrySourceExtentValueLayout::from_checked_record(
            SymbolHandle::from_arena_index(base),
            SymbolHandle::from_arena_index(base + 1),
            0,
            field,
            SymbolHandle::from_arena_index(base + 2),
            8,
            field,
            extent,
        )
        .unwrap()
    };
    let source = SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        slot,
        SymbolHandle::from_arena_index(1),
        SymbolHandle::from_arena_index(2),
        "Bootstrap::continue".into(),
        "continue".into(),
        "Bootstrap::continue#uefi".into(),
        ProgramEntrySourceReceiverSignature::Free,
        vec![
            SelectedProgramEntrySourceSignature::visible_parameter(
                ProgramStorageEntryRootRole::Image,
                0,
                IMAGE_TYPE.into(),
                extent,
                extent_layout(10),
                false,
                false,
            ),
            SelectedProgramEntrySourceSignature::visible_parameter(
                ProgramStorageEntryRootRole::InitialStorage,
                1,
                STORAGE_TYPE.into(),
                extent,
                extent_layout(20),
                false,
                false,
            ),
        ],
    )
    .unwrap();
    bind_optimized_program_storage_semantic_entry_contract(
        target::NativeTarget::uefi_x64(),
        &selected,
        &source,
        &application,
    )
    .unwrap()
}

fn exact_semantic_entry_contract() -> OptimizedProgramStorageSemanticEntryContract {
    semantic_entry_contract(exact_physical_contract())
}

fn authenticated_adapter_budget(
    readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
) -> UefiApplicationBootstrapSameStackBudgetPlan {
    plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter(
        readiness,
        4 * 1024,
        crate::tests::generated_program_storage_adapter_live_frame_demand(),
        96 * 1024,
        16 * 1024,
    )
    .expect("generated wrapper live-frame evidence fits the UEFI guarantee")
}

fn adapter_readiness<'a>(
    ledger: &mut UefiApplicationFirmwareLedger<'a>,
    bytes: &'a [u8],
    base: u64,
) -> UefiApplicationBootstrapAdapterInvocationReadiness<'a> {
    let image_handle = ledger
        .admit_image_handle_occurrence(id(
            base,
            UefiImageHandleOccurrenceId::from_normalized_identity,
        ))
        .unwrap();
    let (integrity, provenance, lease) = inputs(ledger, bytes, base + 1, base + 2);
    let system_table =
        join_lifecycle_scoped_uefi_system_table(ledger, integrity, provenance, lease).unwrap();
    let arrival = join_uefi_application_physical_arrival(
        ledger,
        image_handle,
        system_table,
        exact_physical_contract(),
    )
    .unwrap();
    prepare_uefi_application_bootstrap_adapter_invocation(ledger, arrival).unwrap()
}

fn item_block<'a>(source: &'a str, declaration: &str) -> &'a str {
    let start = source.find(declaration).expect("source declaration");
    let body = &source[start..];
    let mut depth = 0_u32;
    let mut opened = false;
    for (index, character) in body.char_indices() {
        match character {
            '{' => {
                opened = true;
                depth += 1;
            }
            '}' if opened => {
                depth -= 1;
                if depth == 0 {
                    return &body[..=index];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated source declaration {declaration}")
}

fn public_method_names(block: &str) -> Vec<&str> {
    block
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with("pub ") {
                return None;
            }
            let function = line.find("fn ")?;
            line[function + 3..].split('(').next()
        })
        .collect()
}

#[test]
fn joins_exact_occurrence_and_live_phase_without_pointer_projection() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(10);
    let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 13, 14);
    let scoped =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();

    assert_eq!(scoped.layout().profile(), TargetProfile::UefiX64);
    assert_eq!(
        scoped.layout().entry_slot(),
        TargetProfile::UefiX64.program_entry_slot()
    );
    assert_eq!(scoped.revision(), REVISION);
    assert_eq!(scoped.header_size(), 120);
    assert_eq!(scoped.physical_invocation(), ledger.physical_invocation());
    assert_eq!(scoped.firmware_session(), ledger.firmware_session());

    let released = ledger
        .release_lifecycle_scoped_system_table(scoped)
        .unwrap();
    assert_eq!(released.ledger, ledger.ledger_id());
    ledger.begin_firmware_return().unwrap();
    assert!(
        ledger
            .acquire_boot_services_phase_lease(id(
                15,
                UefiBootServicesPhaseLeaseId::from_normalized_identity
            ))
            .is_err()
    );
}

#[test]
fn same_stack_budget_replays_target_guarantee_and_all_four_contributors() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(150);
    let readiness = adapter_readiness(&mut ledger, &bytes, 153);
    let components = UefiApplicationBootstrapSameStackDemandComponents::new(
        4 * 1024,
        8 * 1024,
        96 * 1024,
        16 * 1024,
    );
    let plan = plan_uefi_application_bootstrap_same_stack_budget(&readiness, components)
        .expect("four-term UEFI stack demand fits the target guarantee");

    assert_eq!(plan.ledger_id(), ledger.ledger_id());
    assert_eq!(plan.firmware_session(), ledger.firmware_session());
    assert_eq!(plan.physical_invocation(), ledger.physical_invocation());
    assert_eq!(
        plan.image_handle_occurrence(),
        readiness.image_handle_occurrence()
    );
    assert_eq!(
        plan.system_table_occurrence(),
        readiness.system_table_occurrence()
    );
    assert_eq!(
        plan.phase_lease_id(),
        readiness.arrival.system_table.phase_lease_id()
    );
    assert!(plan.matches_exact_adapter_readiness(&readiness));
    assert_eq!(plan.components(), components);
    assert_eq!(plan.required_entry_stack_bytes(), 124 * 1024);
    assert_eq!(plan.remaining_entry_stack_bytes(), 4 * 1024);
    assert_eq!(
        plan.target_entry_stack_guarantee()
            .guaranteed_available_bytes(),
        128 * 1024,
    );
    assert_eq!(plan.target_entry_stack_guarantee().required_alignment(), 16,);
    assert_eq!(
        plan.physical_calling_plan_commitment(),
        readiness.physical_calling_plan_commitment(),
    );

    let exact_boundary = plan_uefi_application_bootstrap_same_stack_budget(
        &readiness,
        UefiApplicationBootstrapSameStackDemandComponents::new(
            4 * 1024,
            8 * 1024,
            100 * 1024,
            16 * 1024,
        ),
    )
    .expect("the exact target-guarantee boundary is admitted");
    assert_eq!(exact_boundary.required_entry_stack_bytes(), 128 * 1024);
    assert_eq!(exact_boundary.remaining_entry_stack_bytes(), 0);

    let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
    let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn adapter_composition_retains_exact_physical_semantic_and_stack_custody() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(210);
    let readiness = adapter_readiness(&mut ledger, &bytes, 213);
    let budget = authenticated_adapter_budget(&readiness);
    let adapter = compose_uefi_application_bootstrap_adapter(
        readiness,
        budget,
        exact_semantic_entry_contract(),
    )
    .unwrap();

    assert_eq!(adapter.ledger_id(), ledger.ledger_id());
    assert_eq!(adapter.physical_invocation(), ledger.physical_invocation());
    assert_eq!(
        adapter.physical_requirement_identity(),
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY,
    );
    assert_eq!(
        adapter.semantic_requirement_identity(),
        "ProgramStorageEntry::enter#uefi-bootstrap",
    );
    assert_ne!(
        adapter.physical_requirement_identity(),
        adapter.semantic_requirement_identity(),
    );
    assert_eq!(
        adapter.same_stack_budget().required_entry_stack_bytes(),
        4 * 1024 + 72 + 96 * 1024 + 16 * 1024,
    );
    assert_ne!(
        adapter.semantic_source_signature_identity().bytes(),
        [0; 32]
    );
    assert_ne!(adapter.semantic_calling_plan_commitment(), &[0; 32]);
    assert_ne!(
        adapter.semantic_calling_plan_commitment(),
        &exact_uefi_x64_physical_boundary_entry_plan().contract_commitment_digest(),
    );

    let UefiApplicationBootstrapAdapterComposition { readiness, .. } = adapter;
    let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
    let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn adapter_composition_rejects_missing_or_substituted_generated_frame_evidence() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(215);
    let readiness = adapter_readiness(&mut ledger, &bytes, 218);
    let numeric_only = plan_uefi_application_bootstrap_same_stack_budget(
        &readiness,
        UefiApplicationBootstrapSameStackDemandComponents::new(4 * 1024, 72, 96 * 1024, 16 * 1024),
    )
    .unwrap();
    let error = compose_uefi_application_bootstrap_adapter(
        readiness,
        numeric_only,
        exact_semantic_entry_contract(),
    )
    .expect_err("a numerically equal live-frame assertion is not generated evidence");
    assert!(
        error
            .diagnostic()
            .0
            .contains("lacks exact generated live-frame evidence")
    );
    let (readiness, _, _, _) = error.into_parts();

    let substituted = crate::tests::generated_program_storage_adapter_live_frame_demand()
        .with_semantic_boundary_commitment_for_test([0x5a; 32]);
    let budget = plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter(
        &readiness,
        4 * 1024,
        substituted,
        96 * 1024,
        16 * 1024,
    )
    .unwrap();
    let error = compose_uefi_application_bootstrap_adapter(
        readiness,
        budget,
        exact_semantic_entry_contract(),
    )
    .expect_err("generated evidence for another semantic ABI must reject");
    assert!(
        error
            .diagnostic()
            .0
            .contains("semantic and physical ABI identities are not distinct and exact")
    );
    let (readiness, _, _, _) = error.into_parts();
    let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
    let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn adapter_composition_rejects_cross_ledger_stack_plan_and_returns_retry_inputs() {
    let first_bytes = valid_occurrence(120);
    let second_bytes = valid_occurrence(120);
    let mut first_ledger = ledger(220);
    let mut second_ledger = ledger(220);
    let first_readiness = adapter_readiness(&mut first_ledger, &first_bytes, 223);
    let second_readiness = adapter_readiness(&mut second_ledger, &second_bytes, 223);
    let first_budget = authenticated_adapter_budget(&first_readiness);

    let error = compose_uefi_application_bootstrap_adapter(
        second_readiness,
        first_budget,
        exact_semantic_entry_contract(),
    )
    .unwrap_err();
    assert!(
        error
            .diagnostic()
            .0
            .contains("different physical readiness")
    );
    let (second_readiness, first_budget, semantic_entry, _) = error.into_parts();
    assert!(!first_budget.matches_exact_adapter_readiness(&second_readiness));

    let UefiApplicationBootstrapAdapterInvocationReadiness {
        arrival: second_arrival,
        ..
    } = second_readiness;
    let UefiApplicationPhysicalArrival {
        system_table: second_table,
        ..
    } = second_arrival;
    second_ledger
        .release_lifecycle_scoped_system_table(second_table)
        .unwrap();

    let adapter =
        compose_uefi_application_bootstrap_adapter(first_readiness, first_budget, semantic_entry)
            .unwrap();
    let UefiApplicationBootstrapAdapterComposition { readiness, .. } = adapter;
    let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
    let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
    first_ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn adapter_composition_rejects_noncanonical_physical_contract_in_semantic_entry() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(230);
    let readiness = adapter_readiness(&mut ledger, &bytes, 233);
    let budget = authenticated_adapter_budget(&readiness);
    let semantic_entry = semantic_entry_contract(physical_contract(
        "UefiPhysicalEntry::enter#lookalike",
        |_| {},
    ));

    let error =
        compose_uefi_application_bootstrap_adapter(readiness, budget, semantic_entry).unwrap_err();
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact physical arrival contract"),
    );
    let (readiness, budget, _, _) = error.into_parts();
    assert!(budget.matches_exact_adapter_readiness(&readiness));
    let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
    let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn same_stack_budget_rejects_public_coordinate_substitution_across_private_ledgers() {
    let first_bytes = valid_occurrence(120);
    let second_bytes = valid_occurrence(120);
    let mut first_ledger = ledger(180);
    let mut second_ledger = ledger(180);
    let first_readiness = adapter_readiness(&mut first_ledger, &first_bytes, 183);
    let second_readiness = adapter_readiness(&mut second_ledger, &second_bytes, 183);
    assert_eq!(first_readiness.ledger_id(), second_readiness.ledger_id());
    assert_eq!(
        first_readiness.firmware_session(),
        second_readiness.firmware_session()
    );
    assert_eq!(
        first_readiness.physical_invocation(),
        second_readiness.physical_invocation()
    );
    assert_eq!(
        first_readiness.image_handle_occurrence(),
        second_readiness.image_handle_occurrence()
    );
    assert_eq!(
        first_readiness.system_table_occurrence(),
        second_readiness.system_table_occurrence()
    );

    let plan = plan_uefi_application_bootstrap_same_stack_budget(
        &first_readiness,
        UefiApplicationBootstrapSameStackDemandComponents::new(
            4 * 1024,
            8 * 1024,
            96 * 1024,
            16 * 1024,
        ),
    )
    .unwrap();
    assert!(plan.matches_exact_adapter_readiness(&first_readiness));
    assert!(!plan.matches_exact_adapter_readiness(&second_readiness));

    let UefiApplicationBootstrapAdapterInvocationReadiness {
        arrival: first_arrival,
        ..
    } = first_readiness;
    let UefiApplicationPhysicalArrival {
        system_table: first_table,
        ..
    } = first_arrival;
    first_ledger
        .release_lifecycle_scoped_system_table(first_table)
        .unwrap();
    let UefiApplicationBootstrapAdapterInvocationReadiness {
        arrival: second_arrival,
        ..
    } = second_readiness;
    let UefiApplicationPhysicalArrival {
        system_table: second_table,
        ..
    } = second_arrival;
    second_ledger
        .release_lifecycle_scoped_system_table(second_table)
        .unwrap();
}

#[test]
fn same_stack_budget_rejects_every_omitted_contributor_and_overflow() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(160);
    let readiness = adapter_readiness(&mut ledger, &bytes, 163);
    let exact = [4 * 1024, 8 * 1024, 96 * 1024, 16 * 1024];
    for omitted in 0..exact.len() {
        let mut values = exact;
        values[omitted] = 0;
        let error = plan_uefi_application_bootstrap_same_stack_budget(
            &readiness,
            UefiApplicationBootstrapSameStackDemandComponents::new(
                values[0], values[1], values[2], values[3],
            ),
        )
        .unwrap_err();
        assert!(error.0.contains("omitted"));
    }

    let overflow = plan_uefi_application_bootstrap_same_stack_budget(
        &readiness,
        UefiApplicationBootstrapSameStackDemandComponents::new(u64::MAX, 1, 1, 1),
    )
    .unwrap_err();
    assert!(overflow.0.contains("overflowed"));

    let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
    let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn same_stack_budget_rejects_demand_above_the_target_guarantee() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(170);
    let readiness = adapter_readiness(&mut ledger, &bytes, 173);
    let error = plan_uefi_application_bootstrap_same_stack_budget(
        &readiness,
        UefiApplicationBootstrapSameStackDemandComponents::new(
            4 * 1024,
            8 * 1024,
            112 * 1024,
            16 * 1024,
        ),
    )
    .unwrap_err();
    assert!(error.0.contains("guarantees only 131072 bytes"));

    let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
    let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn joins_both_physical_inputs_under_the_exact_non_authorizing_contract() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(70);
    let image_handle = ledger
        .admit_image_handle_occurrence(id(
            73,
            UefiImageHandleOccurrenceId::from_normalized_identity,
        ))
        .unwrap();
    let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 74, 75);
    let system_table =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
    let arrival = join_uefi_application_physical_arrival(
        &ledger,
        image_handle,
        system_table,
        exact_physical_contract(),
    )
    .unwrap();

    assert_eq!(arrival.ledger_id(), ledger.ledger_id());
    assert_eq!(arrival.firmware_session(), ledger.firmware_session());
    assert_eq!(arrival.physical_invocation(), ledger.physical_invocation());
    assert_eq!(
        arrival.physical_contract().requirement_identity(),
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY
    );

    let UefiApplicationPhysicalArrival {
        image_handle: _,
        system_table,
        physical_contract: _,
    } = arrival;
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn physical_arrival_public_surface_has_no_handle_or_storage_projection() {
    // Module-internal field visibility is no public projection.
    let source = [
        include_str!("firmware_ledger.rs"),
        include_str!("system_table_lifecycle.rs"),
        include_str!("physical_arrival.rs"),
        include_str!("same_stack_budget.rs"),
        include_str!("adapter_composition.rs"),
    ]
    .concat()
    .replace("pub(super) ", "");
    let source = source.as_str();
    let handle = item_block(source, "pub struct UefiImageHandleProvenance");
    let compact_handle = handle
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert_eq!(
        compact_handle,
        "pubstructUefiImageHandleProvenance{authority:u64,ledger:UefiApplicationBootstrapLedgerId,session:UefiFirmwareSessionId,invocation:UefiPhysicalInvocationId,occurrence:UefiImageHandleOccurrenceId,opaque_handle:Option<NonZeroU64>,}"
    );
    let compact_source = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(!compact_source.contains("implUefiImageHandleProvenance{"));
    assert_eq!(
        compact_source
            .matches("forUefiImageHandleProvenance{")
            .count(),
        1,
        "image-handle provenance must implement only report-only Debug",
    );
    assert!(compact_source.contains("implstd::fmt::DebugforUefiImageHandleProvenance{"));

    let arrival = item_block(
        source,
        "pub struct UefiApplicationPhysicalArrival<'occurrence>",
    );
    let compact_arrival = arrival
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert_eq!(
        compact_arrival,
        "pubstructUefiApplicationPhysicalArrival<'occurrence>{image_handle:UefiImageHandleProvenance,system_table:LifecycleScopedUefiSystemTable<'occurrence>,physical_contract:ProgramEntryPhysicalContractPlan,}"
    );
    let arrival_impl = item_block(source, "impl UefiApplicationPhysicalArrival<'_>");
    assert_eq!(
        public_method_names(arrival_impl),
        [
            "ledger_id",
            "firmware_session",
            "physical_invocation",
            "image_handle_occurrence",
            "system_table_occurrence",
            "physical_contract",
        ]
    );
    assert_eq!(
        compact_source
            .matches("forUefiApplicationPhysicalArrival<'_>{")
            .count(),
        1,
        "physical-arrival custody must implement only report-only Debug",
    );

    let readiness = item_block(
        source,
        "pub struct UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>",
    );
    let compact_readiness = readiness
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert_eq!(
        compact_readiness,
        "pubstructUefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>{arrival:UefiApplicationPhysicalArrival<'occurrence>,physical_calling_plan_commitment:[u8;32],}"
    );
    let readiness_impl = item_block(
        source,
        "impl UefiApplicationBootstrapAdapterInvocationReadiness<'_>",
    );
    assert_eq!(
        public_method_names(readiness_impl),
        [
            "ledger_id",
            "physical_invocation",
            "firmware_session",
            "image_handle_occurrence",
            "system_table_occurrence",
            "physical_requirement_identity",
            "physical_calling_plan_commitment",
        ]
    );
    assert_eq!(
        compact_source
            .matches("forUefiApplicationBootstrapAdapterInvocationReadiness<'_>{")
            .count(),
        1,
        "adapter readiness must implement only report-only Debug",
    );

    let composition = item_block(
        source,
        "pub struct UefiApplicationBootstrapAdapterComposition<'occurrence>",
    );
    let compact_composition = composition
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert_eq!(
        compact_composition,
        "pubstructUefiApplicationBootstrapAdapterComposition<'occurrence>{readiness:UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,same_stack_budget:UefiApplicationBootstrapSameStackBudgetPlan,semantic_entry:OptimizedProgramStorageSemanticEntryContract,semantic_calling_plan_commitment:[u8;32],}"
    );
    let composition_impl = item_block(
        source,
        "impl UefiApplicationBootstrapAdapterComposition<'_>",
    );
    assert_eq!(
        public_method_names(composition_impl),
        [
            "ledger_id",
            "physical_invocation",
            "physical_requirement_identity",
            "semantic_requirement_identity",
            "semantic_source_signature_identity",
            "same_stack_budget",
            "semantic_calling_plan_commitment",
        ]
    );
    assert_eq!(
        compact_source
            .matches("forUefiApplicationBootstrapAdapterComposition<'_>{")
            .count(),
        1,
        "adapter composition must implement only report-only Debug",
    );

    for forbidden in [
        "psi_extents::Extent",
        "pub fn raw_",
        "pub const fn raw_",
        "pub fn address",
        "pub const fn address",
        "impl From<UefiImageHandleProvenance",
        "impl Into<UefiImageHandleProvenance",
        "impl From<UefiApplicationPhysicalArrival",
        "impl Into<UefiApplicationPhysicalArrival",
        "impl From<UefiApplicationBootstrapAdapterComposition",
        "impl Into<UefiApplicationBootstrapAdapterComposition",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden UEFI physical-arrival API appeared: {forbidden}"
        );
    }
}

#[test]
fn image_handle_provenance_is_issued_only_once() {
    let mut ledger = ledger(80);
    let _handle = ledger
        .admit_image_handle_occurrence(id(
            83,
            UefiImageHandleOccurrenceId::from_normalized_identity,
        ))
        .unwrap();
    let error = ledger
        .admit_image_handle_occurrence(id(
            84,
            UefiImageHandleOccurrenceId::from_normalized_identity,
        ))
        .unwrap_err();
    assert!(error.0.contains("already admitted"));
}

#[test]
fn foreign_image_handle_rejects_even_when_report_ids_match() {
    let bytes = valid_occurrence(120);
    let mut exact = ledger(90);
    let mut foreign = ledger(90);
    let image_handle = foreign
        .admit_image_handle_occurrence(id(
            93,
            UefiImageHandleOccurrenceId::from_normalized_identity,
        ))
        .unwrap();
    let (integrity, provenance, lease) = inputs(&mut exact, &bytes, 94, 95);
    let system_table =
        join_lifecycle_scoped_uefi_system_table(&exact, integrity, provenance, lease).unwrap();

    let error = join_uefi_application_physical_arrival(
        &exact,
        image_handle,
        system_table,
        exact_physical_contract(),
    )
    .unwrap_err();
    assert!(
        error
            .diagnostic()
            .0
            .contains("different physical invocation")
    );
    let (_image_handle, system_table, _contract, _) = error.into_parts();
    exact
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn drifted_physical_plan_rejects_and_returns_inputs_for_retry() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(100);
    let image_handle = ledger
        .admit_image_handle_occurrence(id(
            103,
            UefiImageHandleOccurrenceId::from_normalized_identity,
        ))
        .unwrap();
    let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 104, 105);
    let system_table =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
    let drifted = physical_contract(UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, |plan| {
        plan.call.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::X86R8,
            value_byte_offset: 0,
            byte_size: 8,
        };
    });

    let error =
        join_uefi_application_physical_arrival(&ledger, image_handle, system_table, drifted)
            .unwrap_err();
    assert!(error.diagnostic().0.contains("exact target requirement"));
    let (image_handle, system_table, _drifted, _) = error.into_parts();
    let arrival = join_uefi_application_physical_arrival(
        &ledger,
        image_handle,
        system_table,
        exact_physical_contract(),
    )
    .unwrap();
    let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn semantic_requirement_conflation_rejects() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(110);
    let image_handle = ledger
        .admit_image_handle_occurrence(id(
            113,
            UefiImageHandleOccurrenceId::from_normalized_identity,
        ))
        .unwrap();
    let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 114, 115);
    let system_table =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
    let conflated = physical_contract(
        "named-callable(path(ProgramStorageEntry::enter),parameters(),result-dispatch())",
        |_| {},
    );

    let error =
        join_uefi_application_physical_arrival(&ledger, image_handle, system_table, conflated)
            .unwrap_err();
    assert!(error.diagnostic().0.contains("exact target requirement"));
    let (_image_handle, system_table, _contract, _) = error.into_parts();
    ledger
        .release_lifecycle_scoped_system_table(system_table)
        .unwrap();
}

#[test]
fn accepts_crc_covered_forward_compatible_suffix() {
    let bytes = valid_occurrence(136);
    let mut ledger = ledger(20);
    let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 23, 24);
    let scoped =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
    assert_eq!(scoped.header_size(), 136);
}

#[test]
fn equal_contents_in_a_different_allocation_reject_and_return_all_inputs() {
    let bytes = valid_occurrence(120);
    let copy = bytes.clone();
    let mut ledger = ledger(30);
    let integrity = validate_uefi_system_table_occurrence(
        plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
        &copy,
    )
    .unwrap();
    let provenance = ledger
        .admit_system_table_occurrence(
            id(33, UefiSystemTableOccurrenceId::from_normalized_identity),
            &bytes,
        )
        .unwrap();
    let lease = ledger
        .acquire_boot_services_phase_lease(id(
            34,
            UefiBootServicesPhaseLeaseId::from_normalized_identity,
        ))
        .unwrap();

    let error =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap_err();
    assert!(error.diagnostic().0.contains("exact same byte range"));
    let (_wrong_integrity, provenance, lease, _) = error.into_parts();
    let integrity = validate_uefi_system_table_occurrence(
        plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
        &bytes,
    )
    .unwrap();
    let scoped =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
    ledger
        .release_lifecycle_scoped_system_table(scoped)
        .unwrap();
}

#[test]
fn same_allocation_with_a_different_range_rejects() {
    let mut bytes = valid_occurrence(120);
    bytes.extend_from_slice(&[0; 16]);
    let mut ledger = ledger(35);
    let integrity = validate_uefi_system_table_occurrence(
        plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
        &bytes,
    )
    .unwrap();
    assert_eq!(integrity.table_bytes().len(), 120);
    let provenance = ledger
        .admit_system_table_occurrence(
            id(38, UefiSystemTableOccurrenceId::from_normalized_identity),
            &bytes,
        )
        .unwrap();
    let lease = ledger
        .acquire_boot_services_phase_lease(id(
            39,
            UefiBootServicesPhaseLeaseId::from_normalized_identity,
        ))
        .unwrap();

    let error =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap_err();
    assert!(error.diagnostic().0.contains("exact same byte range"));
}

#[test]
fn copied_report_ids_from_a_foreign_ledger_reject_and_exact_ledger_accepts_retry() {
    let bytes = valid_occurrence(120);
    let mut exact = ledger(40);
    let foreign = ledger(40);
    let (integrity, provenance, lease) = inputs(&mut exact, &bytes, 43, 44);
    let error = join_lifecycle_scoped_uefi_system_table(&foreign, integrity, provenance, lease)
        .unwrap_err();
    assert!(
        error
            .diagnostic()
            .0
            .contains("different physical invocation")
    );
    let (integrity, provenance, lease, _) = error.into_parts();
    let scoped =
        join_lifecycle_scoped_uefi_system_table(&exact, integrity, provenance, lease).unwrap();
    exact.release_lifecycle_scoped_system_table(scoped).unwrap();
}

#[test]
fn failed_release_returns_the_complete_scoped_carrier() {
    let bytes = valid_occurrence(120);
    let mut exact = ledger(50);
    let mut foreign = ledger(50);
    let (integrity, provenance, lease) = inputs(&mut exact, &bytes, 53, 54);
    let scoped =
        join_lifecycle_scoped_uefi_system_table(&exact, integrity, provenance, lease).unwrap();

    let error = foreign
        .release_lifecycle_scoped_system_table(scoped)
        .unwrap_err();
    assert!(error.diagnostic().0.contains("different firmware ledger"));
    let (scoped, _) = error.into_parts();
    exact.release_lifecycle_scoped_system_table(scoped).unwrap();
}

#[test]
fn stale_or_spent_phase_lease_fails_closed() {
    let bytes = valid_occurrence(120);
    let mut ledger = ledger(60);
    let (integrity, provenance, mut lease) = inputs(&mut ledger, &bytes, 63, 64);
    lease.generation += 1;
    let error =
        join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap_err();
    assert!(error.diagnostic().0.contains("foreign, stale, spent"));
}
