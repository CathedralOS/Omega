use super::{
    access_plan, admit_uart, extent_rights, field_key, reach, uart_access_source, uart_extent,
    uart_layout, uart_placement_plan, uart_reach,
};
use crate::{
    AccessExposure, AccessOperation, AtomicAccessOperation, AtomicCapability, AtomicPermissions,
    AtomicTransferRule, BorrowPolarity, BoundaryReach, EffectFootprint, EffectiveSupplyKind,
    ExternalCapability, ExternalRead, ExternalReadBehavior, FieldAccess, ObservationModel,
    PeerWritability, PlacementAdmissionId, PlacementPlan, ResourceProfile, ResourceProfileGrant,
    ResourceProfileReceiptId, ResourceRegion, StableCapability, TransferRule, admit_placement,
    place, validate_access_plan, validate_placement_plan,
};
use language_core::atomic::MemoryOrdering;
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};

#[test]
fn placed_view_derives_access_from_extent_provenance_and_actual_borrow() {
    let plan = uart_placement_plan();

    let mut shared_extent = uart_extent(0x1000, 64);
    let shared_loan = shared_extent.loan(0, 12).expect("shared UART loan");
    let admission = admit_uart(8, shared_loan, &plan, &uart_reach()).expect("admitted shared view");
    let mut shared_view = place(admission).expect("shared placed-view establishment");
    {
        let status = shared_view
            .project(field_key(plan.access(), "status"))
            .expect("pure status projection");
        assert_eq!(status.primitive_address(), 0x1000);
        assert_eq!(status.observation(), ObservationModel::External);
        let read = status.read().expect("shared read");
        assert_eq!(read.access().current_borrow(), BorrowPolarity::Shared);
        assert_eq!(read.access().source_loan(), BorrowPolarity::Shared);
        let request = read.into_primitive_request();
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(
            request.admission(),
            PlacementAdmissionId::from_normalized_identity(8).expect("admission")
        );
        assert_eq!(
            request.profile_receipt(),
            ResourceProfileReceiptId::from_normalized_identity(7).expect("profile receipt")
        );
        assert_eq!(
            request.effective_supply().kind(),
            EffectiveSupplyKind::External
        );
        assert_eq!(request.effective_supply().alignment_bytes(), 4);
        assert_eq!(request.primitive_address(), 0x1000);
        assert_eq!(request.field(), "status");
        assert_eq!(request.transfer_width_bits(), 32);
        assert_eq!(
            request.effect_footprint(),
            EffectFootprint {
                address: 0x1000,
                length_bytes: 4,
            }
        );
        assert_eq!(request.observation(), ObservationModel::External);
        assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
        assert_eq!(request.source_loan(), BorrowPolarity::Shared);
        assert_eq!(request.operation(), AccessOperation::Read);
        assert_eq!(request.resident_claim(), None);
        assert_eq!(request.placed_occurrence(), None);
        assert!(request.reach().contains(reach()));
    }
    {
        let mut transmit = shared_view
            .project(field_key(plan.access(), "transmit"))
            .expect("pure shared transmit projection");
        assert!(
            transmit.write().is_err(),
            "write accessor requires an exclusive current view borrow"
        );
    }
    {
        let mut transmit = shared_view
            .project_mut(field_key(plan.access(), "transmit"))
            .expect("pure exclusive transmit projection");
        assert!(
            transmit.write().is_err(),
            "exclusive reborrow cannot upgrade a shared source loan"
        );
    }

    let exclusive_loan = shared_extent.loan_mut(4, 12).expect("exclusive UART loan");
    let admission =
        admit_uart(9, exclusive_loan, &plan, &uart_reach()).expect("admitted exclusive view");
    let mut exclusive_view = place(admission).expect("exclusive placed-view establishment");
    {
        let mut transmit = exclusive_view
            .project(field_key(plan.access(), "transmit"))
            .expect("pure shared transmit projection");
        assert!(
            transmit.write().is_err(),
            "ordinary write requires an exclusive current view borrow"
        );
    }
    {
        let mut transmit = exclusive_view
            .project_mut(field_key(plan.access(), "transmit"))
            .expect("pure exclusive transmit projection");
        let write = transmit.write().expect("exclusive write");
        assert_eq!(write.primitive_address(), 0x1008);
        assert_eq!(write.access().current_borrow(), BorrowPolarity::Exclusive);
        assert_eq!(write.access().source_loan(), BorrowPolarity::Exclusive);
    }
}

#[test]
fn placed_projection_exposes_only_granular_authorized_events() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 95,
        entries: ["stable", "fifo", "counter", "hidden"]
            .into_iter()
            .enumerate()
            .map(|(index, field)| LayoutFieldEntryReport {
                field: field.into(),
                member_identity: None,
                placement: LayoutPlacementReport::At {
                    offset: u64::try_from(index).expect("field index") * 4,
                },
            })
            .collect(),
        offsets: Some(vec![0, 4, 8, 12]),
        size: Some(16),
        align: 4,
    };
    let placement = validate_placement_plan(PlacementPlan {
        access: access_plan(
            &layout,
            &[
                (
                    "stable",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: true,
                        exposure: AccessExposure::Exported,
                    },
                ),
                (
                    "fifo",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Take,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                ),
                (
                    "counter",
                    FieldAccess::Atomic {
                        transfer_width_bits: 32,
                        operations: AtomicPermissions {
                            load: true,
                            fetch_add: true,
                            ..AtomicPermissions::default()
                        },
                        exposure: AccessExposure::Exported,
                    },
                ),
            ],
        ),
        layout,
        reach: BoundaryReach::default(),
    })
    .expect("heterogeneous placement");
    let mut extent = uart_extent(0x5000, 16);
    let resources = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(51).expect("profile receipt"),
        &extent,
        extent_rights(&[3]),
        BoundaryReach::default(),
    )
    .expect("profile grant")
    .admit(ResourceProfile {
        regions: vec![
            ResourceRegion {
                offset: 0,
                length: 4,
                peer: PeerWritability::Exclusive,
                stable: StableCapability::ReadWrite,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            },
            ResourceRegion {
                offset: 4,
                length: 4,
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
            },
            ResourceRegion {
                offset: 8,
                length: 4,
                peer: PeerWritability::Exclusive,
                stable: StableCapability::None,
                external: ExternalCapability::None,
                atomic: AtomicCapability::Access {
                    transfers: vec![AtomicTransferRule {
                        transfer: TransferRule {
                            width_bits: 32,
                            alignment_bytes: 4,
                        },
                        operations: AtomicPermissions {
                            load: true,
                            fetch_add: true,
                            ..AtomicPermissions::default()
                        },
                    }],
                },
                reach: BoundaryReach::default(),
            },
        ],
    })
    .expect("heterogeneous profile");
    let loan = extent.loan_mut(0, 16).expect("exclusive placed loan");
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(52).expect("admission"),
        loan,
        &placement,
        &resources,
    )
    .expect("heterogeneous placement admission");
    let mut view = place(admission).expect("heterogeneous placed-view establishment");

    {
        let mut stable = view
            .project_mut(field_key(placement.access(), "stable"))
            .expect("stable projection");
        assert_eq!(
            stable.read().expect("stable read").access().operation(),
            AccessOperation::Read
        );
        assert_eq!(
            stable.write().expect("stable write").access().operation(),
            AccessOperation::Write
        );
        assert_eq!(
            stable
                .compound_mutation()
                .expect("stable compound mutation")
                .access()
                .operation(),
            AccessOperation::CompoundMutation
        );
    }
    {
        let mut fifo = view
            .project_mut(field_key(placement.access(), "fifo"))
            .expect("destructive projection");
        assert!(
            fifo.read().is_err(),
            "destructive observation must not derive Readable"
        );
        assert_eq!(
            fifo.take().expect("destructive take").access().operation(),
            AccessOperation::Take
        );
    }
    {
        let counter = view
            .project(field_key(placement.access(), "counter"))
            .expect("atomic projection");
        assert_eq!(
            counter
                .atomic_load(MemoryOrdering::Receive)
                .expect("atomic load")
                .access()
                .operation(),
            AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Receive))
        );
        assert_eq!(
            counter
                .atomic_fetch_add(MemoryOrdering::ReceivePublish)
                .expect("atomic fetch-add")
                .access()
                .operation(),
            AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(
                MemoryOrdering::ReceivePublish
            ))
        );
        assert!(
            counter
                .atomic_fetch_sub(MemoryOrdering::ReceivePublish)
                .is_err(),
            "unlisted atomic families must remain absent"
        );
        assert!(
            counter.atomic_load(MemoryOrdering::Publish).is_err(),
            "operation-specific ordering legality remains sealed"
        );
    }
    assert!(
        view.project(field_key(placement.access(), "hidden"))
            .is_err(),
        "an inaccessible field must not project to an accessor"
    );
}

#[test]
fn placed_view_rejects_unqualified_extent_or_unadmitted_reach() {
    let plan = uart_placement_plan();
    let short = uart_extent(0x1000, 8);
    let short_loan = short.loan(0, 8).expect("short loan");
    let rejection =
        admit_uart(8, short_loan, &plan, &uart_reach()).expect_err("layout must fit extent loan");
    assert!(rejection.diagnostic().0.contains("exceeds"));
    let (returned_loan, _) = rejection.into_parts();
    assert_eq!(
        returned_loan.length(),
        8,
        "rejection returns the exact loan"
    );

    let extent = uart_extent(0x1000, 64);
    let loan = extent.loan(0, 12).expect("UART loan");
    let rejection = admit_uart(9, loan, &plan, &BoundaryReach::default())
        .expect_err("service reach must agree with provenance admission");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("does not supply the placement's complete boundary reach")
    );
}

#[test]
fn access_keys_and_placement_identity_bind_exact_layout_geometry() {
    let plan = uart_placement_plan();
    let mut alternate_layout = uart_layout();
    alternate_layout
        .entries
        .iter_mut()
        .find(|entry| entry.field == "status")
        .expect("status layout entry")
        .placement = LayoutPlacementReport::At { offset: 12 };
    alternate_layout.size = Some(16);
    let error = validate_access_plan(plan.access().plan().clone(), &alternate_layout)
        .expect_err("plan keys bind their exact layout");
    assert!(error.0.contains("different validated layout"));
    let mut alternate = validate_placement_plan(PlacementPlan {
        access: uart_access_source(&alternate_layout),
        layout: alternate_layout,
        reach: uart_reach(),
    })
    .expect("fresh plan over non-overlapping alternate geometry");
    assert_ne!(plan.access().identity(), alternate.access().identity());
    assert_ne!(plan.identity(), alternate.identity());
    assert_ne!(
        plan.access().layout_report_fingerprint(),
        alternate.access().layout_report_fingerprint(),
        "layout geometry is part of access-policy identity"
    );

    alternate.access.layout_report_fingerprint = plan.access.layout_report_fingerprint;
    alternate.access.plan.layout_report_fingerprint = plan.access.layout_report_fingerprint;
    for entry in &mut alternate.access.plan.entries {
        entry.key.layout_report_fingerprint = plan.access.layout_report_fingerprint;
    }
    for descriptor in &mut alternate.access.fields {
        descriptor.key.layout_report_fingerprint = plan.access.layout_report_fingerprint;
    }
    let substituted_key = field_key(alternate.access(), "status");
    assert_eq!(
        substituted_key.layout_report_fingerprint,
        field_key(plan.access(), "status").layout_report_fingerprint,
        "the adversarial substitution holds the compact key coordinate equal"
    );
    assert_ne!(
        substituted_key.layout_commitment,
        field_key(plan.access(), "status").layout_commitment,
        "the exact layout commitment must still distinguish the keys"
    );
    assert!(plan.access().field(substituted_key).is_none());
    assert!(plan.access().field_descriptor(substituted_key).is_none());
    assert!(
        plan.access()
            .authorize(
                substituted_key,
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                AccessOperation::Read,
            )
            .is_err(),
        "compact-equal key substitution must not authorize an exact foreign layout"
    );

    let extent = uart_extent(0x1000, 12);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let admission = admit_uart(108, loan, &plan, &uart_reach()).expect("admitted UART view");
    let view = place(admission).expect("placed UART view");
    assert!(
        view.project(substituted_key).is_err(),
        "placed projection must reject a compact-equal foreign layout key"
    );
}
