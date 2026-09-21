//! Fixtures shared by the access-plan tests: UART layouts, plans, extents,
//! profiles and placements with fixed seeds.

mod atomic_specialization;
mod device_operations;
mod external_specialization;
mod owned_external_correspondence;
mod placed_views;
mod placement_admission;
mod plan_validation;
mod provider_content;
mod resident_custody;
mod resource_profiles;
mod schema_correspondence;
mod stable_specialization;

use crate::placements::placement_authority::PlacementAuthorityRef;
use crate::{
    AccessExposure, AccessFieldEntry, AccessFieldKey, AccessOperation, AccessPlan,
    AdmittedResourceProfile, AtomicAccessOperation, AtomicCapability, AtomicPermissions,
    AtomicTransferRule, AuthorizedFieldAccess, BorrowPolarity, BoundaryReach,
    BoundaryServiceReachId, DeviceOperation, DeviceOperationCoordinates,
    DeviceOperationProviderPlanId, DeviceOperationRequirement, DeviceOperationRequirementId,
    DeviceOrderingScopeId, DeviceOrderingScopeOccurrence, DeviceOrderingScopeOccurrenceId,
    DormantOwnedAtomicResident, EffectFootprint, EffectiveFieldSupply, EffectiveSupplyKind,
    EstablishedOwnedPlacement, ExternalCapability, ExternalRead, ExternalReadBehavior, FieldAccess,
    FieldAccessDescriptor, LogicalFieldExtent, ObservationModel, PeerWritability,
    PlacedOccurrenceId, PlacementAdmission, PlacementAdmissionId, PlacementPlan, PlacementPlanId,
    PlacementRejection, PrimitiveAccessRequest, ProviderAssertedDeviceOperationClaim,
    ResourceProfile, ResourceProfileGrant, ResourceProfileReceiptId, ResourceRegion,
    SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId, SchemaDeviceCorrespondenceGrant,
    SchemaDeviceCorrespondenceReceiptContext, StableCapability, StableDeviceInstanceId,
    TransferRule, ValidatedAccessPlan, ValidatedPlacementPlan, admit_owned_placement,
    admit_placement, adopt_owned_atomic, adopt_owned_stable, validate_access_plan,
    validate_placement_plan,
};
use extents::Extent;
use extents::ExtentLoan;
use extents::ProviderExistingContentGrant;
use extents::ResidentClaimId;
use extents::{
    AddressSpaceId, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLineageId,
    ExtentProvenanceId, ExtentRights, MappedRangeReceiptContext, MappingEraId, MappingGrant,
    MappingGrantId, MappingId, MappingSourceMode, PeerWriteRevocationObligations,
    TranslationActivationReceipt, TranslationInstallObligations,
    TranslationReleaseObligations, map_owned,
};
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};

fn reach() -> BoundaryServiceReachId {
    BoundaryServiceReachId::from_normalized_identity(7).expect("normalized reach")
}

fn uart_reach() -> BoundaryReach {
    BoundaryReach::from_services([reach()])
}

fn uart_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "status".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "transmit".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 4 },
            },
            LayoutFieldEntryReport {
                field: "control".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 8,
                    container_width: 32,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                },
            },
        ],
        offsets: None,
        size: Some(12),
        align: 4,
    }
}

fn access_plan(layout: &LayoutPlanReport, decisions: &[(&str, FieldAccess)]) -> AccessPlan {
    let mut plan = AccessPlan::inaccessible(layout).expect("inaccessible seed");
    for (field, access) in decisions {
        let key = plan
            .entries()
            .iter()
            .find(|entry| entry.field() == *field)
            .map(AccessFieldEntry::key)
            .expect("schema field key");
        plan.set(key, access.clone())
            .expect("replace field decision");
    }
    plan
}

fn field_key(plan: &ValidatedAccessPlan, field: &str) -> AccessFieldKey {
    plan.plan()
        .entries()
        .iter()
        .find(|entry| entry.field() == field)
        .map(AccessFieldEntry::key)
        .expect("validated schema field key")
}

fn uart_access_source(layout: &LayoutPlanReport) -> AccessPlan {
    access_plan(
        layout,
        &[
            (
                "status",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Read,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            ),
            (
                "transmit",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::None,
                    write: true,
                    exposure: AccessExposure::Exported,
                },
            ),
            (
                "control",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Read,
                    write: false,
                    exposure: AccessExposure::BindingPrivate,
                },
            ),
        ],
    )
}

fn uart_access_plan() -> ValidatedAccessPlan {
    let layout = uart_layout();
    let plan = uart_access_source(&layout);
    validate_access_plan(plan, &layout).expect("UART plan")
}

fn uart_placement_plan() -> ValidatedPlacementPlan {
    let layout = uart_layout();
    validate_placement_plan(PlacementPlan {
        access: uart_access_source(&layout),
        layout,
        reach: uart_reach(),
    })
    .expect("UART placement plan")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, extents::ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn provider_issuance(seed: u64) -> extents::ExtentProviderIssuance {
    let base = seed * 16;
    extents::ExtentProviderIssuance::from_normalized_identities([
        base + 1,
        base + 2,
        base + 3,
        base + 4,
        base + 5,
        base + 6,
        base + 7,
        base + 8,
        base + 9,
        base + 10,
        base + 11,
        base + 12,
        base + 13,
    ])
    .expect("normalized provider issuance")
}

fn extent_rights(identities: &[u64]) -> ExtentRights {
    ExtentRights::from_normalized_identities(
        identities
            .iter()
            .copied()
            .map(|identity| extent_id(identity, extents::ExtentRightId::from_normalized_identity)),
    )
}

fn uart_extent(base: u64, length: u64) -> extents::Extent {
    uart_extent_with_lineage(base, length, 1)
}

fn uart_extent_with_lineage(base: u64, length: u64, lineage: u64) -> extents::Extent {
    uart_extent_with_root(base, length, 1, lineage)
}

fn uart_extent_with_root(base: u64, length: u64, provider: u64, lineage: u64) -> extents::Extent {
    uart_root_grant(provider, lineage)
        .mint(base, length)
        .expect("UART extent")
}

fn uart_root_grant(provider: u64, lineage: u64) -> extents::ExtentRootGrant {
    uart_root_grant_with_mapping(provider, lineage, 5, 6)
}

fn uart_root_grant_with_mapping(
    provider: u64,
    lineage: u64,
    provenance: u64,
    era: u64,
) -> extents::ExtentRootGrant {
    extents::ExtentRootGrant::from_admitted_provider(
        provider_issuance(provider),
        extent_id(lineage, extents::ExtentLineageId::from_normalized_identity),
        extent_id(2, AddressSpaceId::from_normalized_identity),
        extent_rights(&[3, 4]),
        extent_id(provenance, ExtentProvenanceId::from_normalized_identity),
        extent_id(era, extents::MappingEraId::from_normalized_identity),
    )
}

fn uart_resource_profile(loan: &ExtentLoan<'_>, reach: &BoundaryReach) -> AdmittedResourceProfile {
    ResourceProfileGrant::from_admitted_provider_loan(
        ResourceProfileReceiptId::from_normalized_identity(7).expect("profile receipt"),
        loan,
        extent_rights(&[3]),
        reach.clone(),
    )
    .expect("UART resource-profile grant")
    .admit(uart_resource_profile_data(loan.length(), reach))
    .expect("admitted UART resource profile")
}

fn uart_resource_profile_for_extent(
    extent: &Extent,
    reach: &BoundaryReach,
) -> AdmittedResourceProfile {
    ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(71).expect("profile receipt"),
        extent,
        extent_rights(&[3]),
        reach.clone(),
    )
    .expect("UART resource-profile grant")
    .admit(uart_resource_profile_data(extent.length(), reach))
    .expect("admitted UART resource profile")
}

fn uart_resource_profile_data(length: u64, reach: &BoundaryReach) -> ResourceProfile {
    ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length,
            peer: PeerWritability::Exclusive,
            stable: StableCapability::None,
            external: ExternalCapability::Access {
                read: ExternalReadBehavior::Repeatable,
                write: true,
                transfers: vec![TransferRule {
                    width_bits: 32,
                    alignment_bytes: 4,
                }],
            },
            atomic: AtomicCapability::None,
            reach: reach.clone(),
        }],
    }
}

fn stable_word_placement() -> ValidatedPlacementPlan {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 0x5ab1e,
        entries: vec![LayoutFieldEntryReport {
            field: "word".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 4,
    };
    validate_placement_plan(PlacementPlan {
        access: access_plan(
            &layout,
            &[(
                "word",
                FieldAccess::Stable {
                    transfer_width_bits: 32,
                    read: true,
                    write: true,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        layout,
        reach: BoundaryReach::default(),
    })
    .expect("Stable word placement")
}

fn stable_word_profile(extent: &Extent) -> AdmittedResourceProfile {
    ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(91).expect("profile receipt"),
        extent,
        extent_rights(&[3]),
        BoundaryReach::default(),
    )
    .expect("Stable resource-profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: extent.length(),
            peer: PeerWritability::Exclusive,
            stable: StableCapability::ReadWrite,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted Stable resource profile")
}

fn stable_uart_resource_profile(
    loan: &ExtentLoan<'_>,
    reach: &BoundaryReach,
) -> AdmittedResourceProfile {
    ResourceProfileGrant::from_admitted_provider_loan(
        ResourceProfileReceiptId::from_normalized_identity(141).expect("profile receipt"),
        loan,
        extent_rights(&[3]),
        reach.clone(),
    )
    .expect("Stable UART resource-profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: loan.length(),
            peer: PeerWritability::Exclusive,
            stable: StableCapability::ReadWrite,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: reach.clone(),
        }],
    })
    .expect("admitted Stable UART resource profile")
}

fn destructive_word_placement() -> ValidatedPlacementPlan {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 0xe17e_7a4e,
        entries: vec![LayoutFieldEntryReport {
            field: "fifo".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 4,
    };
    validate_placement_plan(PlacementPlan {
        access: access_plan(
            &layout,
            &[(
                "fifo",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Take,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        layout,
        reach: BoundaryReach::default(),
    })
    .expect("destructive External word placement")
}

fn destructive_word_profile(loan: &ExtentLoan<'_>) -> AdmittedResourceProfile {
    ResourceProfileGrant::from_admitted_provider_loan(
        ResourceProfileReceiptId::from_normalized_identity(142).expect("profile receipt"),
        loan,
        extent_rights(&[3]),
        BoundaryReach::default(),
    )
    .expect("destructive External resource-profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: loan.length(),
            peer: PeerWritability::Exclusive,
            stable: StableCapability::None,
            external: ExternalCapability::Access {
                read: ExternalReadBehavior::Destructive,
                write: false,
                transfers: vec![TransferRule {
                    width_bits: 32,
                    alignment_bytes: 4,
                }],
            },
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted destructive External resource profile")
}

const fn all_atomic_operations() -> AtomicPermissions {
    AtomicPermissions {
        load: true,
        store: true,
        fetch_add: true,
        fetch_sub: true,
        fetch_xor: true,
        fetch_or: true,
        fetch_and: true,
        swap: true,
        compare_exchange: true,
        compare_exchange_once: true,
        try_exchange: true,
        try_exchange_once: true,
    }
}

fn atomic_word_placement() -> ValidatedPlacementPlan {
    atomic_word_placement_with_operations(0x000a_701c, all_atomic_operations())
}

fn atomic_word_placement_with_operations(
    schema_report_fingerprint: u64,
    operations: AtomicPermissions,
) -> ValidatedPlacementPlan {
    let layout = LayoutPlanReport {
        schema_report_fingerprint,
        entries: vec![LayoutFieldEntryReport {
            field: "head".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 4,
    };
    validate_placement_plan(PlacementPlan {
        access: access_plan(
            &layout,
            &[(
                "head",
                FieldAccess::Atomic {
                    transfer_width_bits: 32,
                    operations,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        layout,
        reach: BoundaryReach::default(),
    })
    .expect("Atomic word placement")
}

fn atomic_word_profile(loan: &ExtentLoan<'_>) -> AdmittedResourceProfile {
    ResourceProfileGrant::from_admitted_provider_loan(
        ResourceProfileReceiptId::from_normalized_identity(155).expect("profile receipt"),
        loan,
        extent_rights(&[3]),
        BoundaryReach::default(),
    )
    .expect("Atomic resource-profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: loan.length(),
            peer: PeerWritability::Exclusive,
            stable: StableCapability::None,
            external: ExternalCapability::None,
            atomic: AtomicCapability::Access {
                transfers: vec![AtomicTransferRule {
                    transfer: TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    },
                    operations: all_atomic_operations(),
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted all-family Atomic resource profile")
}

fn dormant_atomic_word(
    plan: &ValidatedPlacementPlan,
    base: u64,
    lineage: u64,
    validity: u64,
    admission: u64,
) -> DormantOwnedAtomicResident {
    let (extent, content) = provider_existing_content(plan, base, 4, lineage, validity);
    let profile = {
        let loan = extent.loan(0, 4).expect("Atomic profile loan");
        atomic_word_profile(&loan)
    };
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(admission).expect("Atomic admission"),
        extent,
        plan,
        &profile,
    )
    .expect("owned Atomic admission");
    adopt_owned_atomic(admission, content).expect("provider-backed Atomic resident")
}

#[derive(Debug, PartialEq, Eq)]
struct PrimitiveRequestSnapshot {
    plan: PlacementPlanId,
    profile_receipt: ResourceProfileReceiptId,
    effective_supply: EffectiveFieldSupply,
    admission: PlacementAdmissionId,
    primitive_address: u64,
    key: AccessFieldKey,
    field: String,
    transfer_width_bits: u16,
    logical_extent: LogicalFieldExtent,
    effect_footprint: EffectFootprint,
    observation: ObservationModel,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    operation: AccessOperation,
    reach: BoundaryReach,
    resident_claim: Option<ResidentClaimId>,
    placed_occurrence: Option<PlacedOccurrenceId>,
    descriptor: FieldAccessDescriptor,
    authorization: AuthorizedFieldAccess,
    authority_kind: &'static str,
    authority_identity: *const (),
}

fn primitive_request_snapshot(
    request: &PrimitiveAccessRequest<'_, '_>,
) -> PrimitiveRequestSnapshot {
    let (authority_kind, authority_identity) = match request._authority {
        PlacementAuthorityRef::Borrowed(view) => {
            ("borrowed", std::ptr::from_ref(view).cast::<()>())
        }
        PlacementAuthorityRef::CorrespondedBorrowed(view) => (
            "corresponded-borrowed",
            std::ptr::from_ref(view).cast::<()>(),
        ),
        PlacementAuthorityRef::BorrowedResident(established) => (
            "borrowed-resident",
            std::ptr::from_ref(established).cast::<()>(),
        ),
        PlacementAuthorityRef::BorrowedAtomicResident(established) => (
            "borrowed-atomic-resident",
            std::ptr::from_ref(established).cast::<()>(),
        ),
        PlacementAuthorityRef::EstablishedOwned(established) => (
            "established-owned",
            std::ptr::from_ref(established).cast::<()>(),
        ),
        PlacementAuthorityRef::EstablishedOwnedAtomic(established) => (
            "established-owned-atomic",
            std::ptr::from_ref(established).cast::<()>(),
        ),
        PlacementAuthorityRef::OwnedCorrespondedExternal(established) => (
            "owned-corresponded-external",
            std::ptr::from_ref(established).cast::<()>(),
        ),
    };
    PrimitiveRequestSnapshot {
        plan: request.plan,
        profile_receipt: request.profile_receipt,
        effective_supply: request.effective_supply.clone(),
        admission: request.admission,
        primitive_address: request.primitive_address,
        key: request.key,
        field: request.field.clone(),
        transfer_width_bits: request.transfer_width_bits,
        logical_extent: request.logical_extent.clone(),
        effect_footprint: request.effect_footprint,
        observation: request.observation,
        current_borrow: request.current_borrow,
        source_loan: request.source_loan,
        operation: request.operation,
        reach: request.reach.clone(),
        resident_claim: request.resident_claim,
        placed_occurrence: request.placed_occurrence,
        descriptor: request.descriptor.clone(),
        authorization: request.authorization.clone(),
        authority_kind,
        authority_identity,
    }
}

fn assert_atomic_specialization(
    request: PrimitiveAccessRequest<'_, '_>,
    expected: AtomicAccessOperation,
    plan: PlacementPlanId,
    admission: PlacementAdmissionId,
) {
    let atomic = request
        .into_atomic_primitive_access()
        .expect("Atomic primitive specialization");
    assert_eq!(atomic.operation(), expected);
    assert_eq!(atomic.ordering_plan(), expected.ordering_plan());
    assert_eq!(atomic.primitive_address(), 0xc000);
    assert_eq!(atomic.transfer_width_bits(), 32);
    assert_eq!(atomic.logical_extent().fragments().len(), 1);
    assert_eq!(atomic.effect_footprint().address(), 0xc000);
    assert_eq!(atomic.effect_footprint().length_bytes(), 4);

    let request = atomic.into_primitive_request();
    assert_eq!(request.plan(), plan);
    assert_eq!(request.admission(), admission);
    assert_eq!(request.profile_receipt().normalized_identity(), 155);
    assert_eq!(
        request.effective_supply().kind(),
        EffectiveSupplyKind::Atomic
    );
    assert_eq!(request.effective_supply().key(), request.key);
    assert_eq!(request.effective_supply().width_bits(), 32);
    assert_eq!(request.effective_supply().alignment_bytes(), 4);
    assert_eq!(request.observation(), ObservationModel::Atomic);
    assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
    assert_eq!(request.source_loan(), BorrowPolarity::Shared);
    assert_eq!(request.operation(), AccessOperation::Atomic(expected));
}

fn expect_exact_atomic_rejection<'view, 'extent>(
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic_fragment: &str,
) -> PrimitiveAccessRequest<'view, 'extent> {
    let before = primitive_request_snapshot(&request);
    let rejection = request
        .into_atomic_primitive_access()
        .expect_err("corrupt request must fail Atomic specialization");
    assert!(
        rejection.diagnostic().0.contains(diagnostic_fragment),
        "unexpected Atomic rejection: {}",
        rejection.diagnostic()
    );
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains(diagnostic_fragment));
    assert_eq!(primitive_request_snapshot(&request), before);
    request
}

fn expect_exact_stable_primitive_rejection<'view, 'extent>(
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic_fragment: &str,
) -> PrimitiveAccessRequest<'view, 'extent> {
    let before = primitive_request_snapshot(&request);
    let rejection = request
        .into_stable_primitive_access()
        .expect_err("corrupt request must fail Stable primitive specialization");
    assert!(
        rejection.diagnostic().0.contains(diagnostic_fragment),
        "unexpected Stable primitive rejection: {}",
        rejection.diagnostic()
    );
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains(diagnostic_fragment));
    assert_eq!(primitive_request_snapshot(&request), before);
    request
}

fn expect_exact_stable_compound_rejection<'view, 'extent>(
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic_fragment: &str,
) -> PrimitiveAccessRequest<'view, 'extent> {
    let before = primitive_request_snapshot(&request);
    let rejection = request
        .into_stable_compound_mutation_access()
        .expect_err("corrupt request must fail Stable compound specialization");
    assert!(
        rejection.diagnostic().0.contains(diagnostic_fragment),
        "unexpected Stable compound rejection: {}",
        rejection.diagnostic()
    );
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains(diagnostic_fragment));
    assert_eq!(primitive_request_snapshot(&request), before);
    request
}

fn provider_existing_content(
    plan: &ValidatedPlacementPlan,
    base: u64,
    length: u64,
    lineage: u64,
    receipt_seed: u64,
) -> (Extent, ProviderExistingContentGrant) {
    uart_root_grant(1, lineage)
        .mint_provider_existing_content(
            base,
            length,
            plan.content_interpretation(),
            extent_id(receipt_seed + 2, ResidentClaimId::from_normalized_identity),
            extent_id(
                receipt_seed,
                ExtentContentValidityReceiptId::from_normalized_identity,
            ),
            extent_id(
                receipt_seed + 1,
                ExtentContentCustodyReceiptId::from_normalized_identity,
            ),
        )
        .expect("provider existing-content extent")
}

fn established_stable_word(
    base: u64,
    lineage: u64,
    receipt_seed: u64,
    admission_identity: u64,
) -> (ValidatedPlacementPlan, EstablishedOwnedPlacement) {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, base, 4, lineage, receipt_seed);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(admission_identity).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");
    let dormant =
        adopt_owned_stable(admission, content).expect("provider-evidenced Stable adoption");
    let established = dormant
        .view(
            PlacedOccurrenceId::from_normalized_identity(admission_identity + 10_000)
                .expect("placed occurrence"),
        )
        .expect("owned resident-view establishment");
    (plan, established)
}

fn admit_uart<'extent>(
    identity: u64,
    loan: ExtentLoan<'extent>,
    plan: &ValidatedPlacementPlan,
    permitted_reach: &BoundaryReach,
) -> Result<PlacementAdmission<'extent>, PlacementRejection<'extent>> {
    let resources = uart_resource_profile(&loan, permitted_reach);
    admit_placement(
        PlacementAdmissionId::from_normalized_identity(identity).expect("placement admission"),
        loan,
        plan,
        &resources,
    )
}

fn device_requirement_mapped_range(offset: u64, length: u64) -> MappedRangeReceiptContext {
    let source = extents::ExtentRootGrant::from_admitted_provider(
        provider_issuance(801),
        extent_id(802, ExtentLineageId::from_normalized_identity),
        extent_id(803, AddressSpaceId::from_normalized_identity),
        extent_rights(&[804]),
        extent_id(805, ExtentProvenanceId::from_normalized_identity),
        extent_id(806, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 0x1000)
    .expect("device source extent");
    let destination = extents::ExtentRootGrant::from_admitted_provider(
        provider_issuance(807),
        extent_id(808, ExtentLineageId::from_normalized_identity),
        extent_id(809, AddressSpaceId::from_normalized_identity),
        extent_rights(&[810]),
        extent_id(811, ExtentProvenanceId::from_normalized_identity),
        extent_id(812, MappingEraId::from_normalized_identity),
    )
    .mint(0x8000, 0x1000)
    .expect("device destination extent");
    let grant = MappingGrant::from_admitted_provider(
        extent_id(813, MappingGrantId::from_normalized_identity),
        MappingSourceMode::Owned,
        source.address_space(),
        destination.address_space(),
        source.rights().clone(),
        destination.rights().clone(),
        destination.rights().clone(),
        extent_id(814, ExtentProvenanceId::from_normalized_identity),
        extent_id(815, MappingEraId::from_normalized_identity),
        TranslationInstallObligations::default(),
        TranslationReleaseObligations::default(),
        PeerWriteRevocationObligations::default(),
    );
    let pending = map_owned(
        source,
        destination,
        extent_id(816, MappingId::from_normalized_identity),
        &grant,
    )
    .expect("device mapping candidate");
    let receipt =
        TranslationActivationReceipt::from_admitted_provider(&pending.receipt_context(), true, []);
    pending
        .complete(receipt)
        .expect("active device mapping")
        .range_receipt_context(offset, length)
        .expect("exact mapped device range")
}

fn device_requirement_correspondence(
    provider_identity: u64,
) -> SchemaDeviceCorrespondenceReceiptContext {
    let placement = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x9000, 12, 817);
    let profile = uart_resource_profile_for_extent(&extent, &uart_reach());
    SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(provider_identity)
            .expect("device correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(818).expect("stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(819)
            .expect("device correspondence source"),
        &placement,
        profile.receipt(),
        None,
    )
    .expect("device correspondence grant")
    .admit(&placement, &profile)
    .expect("admitted device correspondence")
    .receipt_context()
}

fn device_scope_occurrence(ordering_scope: u64, occurrence: u64) -> DeviceOrderingScopeOccurrence {
    DeviceOrderingScopeOccurrence::from_provider_assertion(
        DeviceOrderingScopeId::from_normalized_identity(ordering_scope)
            .expect("device ordering scope capability"),
        DeviceOrderingScopeOccurrenceId::from_normalized_identity(occurrence)
            .expect("device ordering-scope occurrence"),
    )
}

fn device_claim(
    provider_plan: u64,
    requirement: &DeviceOperationRequirement,
    occurrence: u64,
) -> ProviderAssertedDeviceOperationClaim {
    ProviderAssertedDeviceOperationClaim::from_provider_assertion(
        DeviceOperationProviderPlanId::from_normalized_identity(provider_plan)
            .expect("device provider plan"),
        requirement,
        device_scope_occurrence(
            requirement.ordering_scope().normalized_identity(),
            occurrence,
        ),
    )
    .expect("scope occurrence covers the demanded scope capability")
}

fn device_coordinates(operation: DeviceOperation, range_offset: u64) -> DeviceOperationCoordinates {
    let primary = || device_requirement_mapped_range(range_offset, 0x80);
    let secondary = || device_requirement_mapped_range(range_offset + 0x100, 0x40);
    match operation {
        DeviceOperation::DmaPublication => DeviceOperationCoordinates::DmaPublication {
            data: primary(),
            descriptor: secondary(),
        },
        DeviceOperation::DeviceAcquisition => DeviceOperationCoordinates::DeviceAcquisition {
            request: primary(),
            completion: secondary(),
        },
        DeviceOperation::CacheMaintenance => DeviceOperationCoordinates::CacheMaintenance {
            maintained: primary(),
        },
        DeviceOperation::MmioNotification => DeviceOperationCoordinates::MmioNotification {
            doorbell: primary(),
            request: secondary(),
        },
        DeviceOperation::PostedWriteCompletion => {
            DeviceOperationCoordinates::PostedWriteCompletion {
                request: primary(),
                completion: secondary(),
            }
        }
    }
}

fn device_requirement(
    identity: u64,
    operation: DeviceOperation,
    range_offset: u64,
    correspondence_provider: u64,
    ordering_scope: u64,
) -> DeviceOperationRequirement {
    DeviceOperationRequirement::new(
        DeviceOperationRequirementId::from_normalized_identity(identity)
            .expect("device requirement identity"),
        device_coordinates(operation, range_offset),
        device_requirement_correspondence(correspondence_provider),
        DeviceOrderingScopeId::from_normalized_identity(ordering_scope)
            .expect("device ordering scope"),
    )
}
