use super::{
    access_plan, extent_rights, field_key, reach, uart_access_plan, uart_access_source,
    uart_extent, uart_layout,
};
use crate::plan_policy::access_plan_validation::validate_entry_geometry;
use crate::{
    AccessExposure, AccessFieldEntry, AccessFieldKey, AccessOperation, AccessPlan,
    AtomicAccessOperation, AtomicCapability, AtomicPermissions, AtomicTransferRule, BorrowPolarity,
    BoundaryReach, BoundaryServiceReachId, EffectiveSupplyKind, ExternalCapability, ExternalRead,
    FieldAccess, LogicalFieldFragment, ObservationModel, PeerWritability, PlacementAdmissionId,
    PlacementPlan, RelativeEffectFootprint, ResourceProfile, ResourceProfileGrant,
    ResourceProfileReceiptId, ResourceRegion, StableCapability, TransferRule, admit_placement,
    place, validate_access_plan, validate_placement_plan,
};
use language_core::atomic::AtomicOrderingPlan;
use language_core::atomic::MemoryOrdering;
use layout_plans::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};

#[test]
fn compact_fnv_plan_inventory_is_explicitly_non_authoritative() {
    let source = include_str!("../plan_policy/normalized_identities.rs");
    assert_eq!(
        source.matches("0xcbf29ce484222325u64").count(),
        3,
        "every compact FNV plan family must remain in the reviewed local inventory"
    );
    for name in [
        "non_authoritative_access_plan_compatibility_fingerprint",
        "non_authoritative_placement_compatibility_fingerprint",
        "non_authoritative_resource_profile_compatibility_fingerprint",
    ] {
        assert!(
            source.contains(name),
            "compact plan fingerprints must advertise their non-authoritative role: {name}"
        );
    }
    assert!(
        source.contains("omega.placement-plan.authoritative.v1\\0") && source.contains("Sha256"),
        "provider-content interpretation must retain a domain-separated strong commitment"
    );
    assert!(
        source.contains("omega.access-field-layout.authoritative.v1\\0")
            && include_str!("../access_plan.rs")
                .contains("layout_commitment: AccessLayoutCommitment"),
        "access field keys must retain a domain-separated exact-layout commitment"
    );
}

#[test]
fn stored_integer_geometry_uses_the_exact_encoded_width() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "value".into(),
            member_identity: None,
            placement: LayoutPlacementReport::IntegerAt {
                offset: 3,
                stored_width: 16,
                interpretation: IntegerInterpretation::Signed,
            },
        }],
        offsets: None,
        size: Some(8),
        align: 1,
    };
    let (offset, logical, effect) = validate_entry_geometry("value", 16, &layout, 8)
        .expect("the access transfer matches the stored integer width");
    assert_eq!(offset, 3);
    assert_eq!(logical.fragments[0].layout_bit_offset, 24);
    assert_eq!(logical.fragments[0].width_bits, 16);
    assert_eq!(effect.length_bytes, 2);
    let error = validate_entry_geometry("value", 32, &layout, 8)
        .expect_err("semantic carrier width is not the stored transfer width");
    assert!(
        error
            .0
            .contains("32-bit transfer over a 16-bit stored integer")
    );
}

#[test]
fn inaccessible_seed_has_exact_canonical_schema_cardinality() {
    let layout = uart_layout();
    let plan = AccessPlan::inaccessible(&layout).expect("inaccessible plan");
    assert_eq!(plan.entries().len(), 3);
    assert_eq!(
        plan.entries()
            .iter()
            .map(AccessFieldEntry::field)
            .collect::<Vec<_>>(),
        vec!["control", "status", "transmit"]
    );
    assert!(
        plan.entries()
            .iter()
            .all(|entry| entry.access() == &FieldAccess::Inaccessible)
    );
    let validated = validate_access_plan(plan, &layout).expect("all-inaccessible plan");
    assert!(validated.field_descriptors().is_empty());
    assert!(
        validated
            .authorize(
                field_key(&validated, "status"),
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                AccessOperation::Read,
            )
            .is_err()
    );
}

#[test]
fn numbered_field_rename_does_not_change_access_identity() {
    let mut original_layout = LayoutPlanReport {
        schema_report_fingerprint: 0x44,
        entries: vec![LayoutFieldEntryReport {
            field: "word".into(),
            member_identity: Some(7),
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 4,
    };
    let original = validate_access_plan(
        access_plan(
            &original_layout,
            &[(
                "word",
                FieldAccess::Stable {
                    transfer_width_bits: 32,
                    read: true,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &original_layout,
    )
    .expect("original numbered plan");

    original_layout.entries[0].field = "renamed_word".into();
    let renamed = validate_access_plan(
        access_plan(
            &original_layout,
            &[(
                "renamed_word",
                FieldAccess::Stable {
                    transfer_width_bits: 32,
                    read: true,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &original_layout,
    )
    .expect("renamed numbered plan");
    assert_eq!(original.identity(), renamed.identity());
}

#[test]
fn inaccessible_plan_rejects_one_name_for_multiple_field_identities() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 0x45,
        entries: vec![
            LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: Some(7),
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: Some(8),
                placement: LayoutPlacementReport::At { offset: 4 },
            },
        ],
        offsets: Some(vec![0, 4]),
        size: Some(8),
        align: 4,
    };

    let error = AccessPlan::inaccessible(&layout)
        .expect_err("one presentation name cannot select two stable field identities");
    assert!(
        error.0.contains(
            "layout field `word` identifies both stable member identity #7 and stable member identity #8"
        ),
        "{}",
        error.0
    );
}

#[test]
fn access_validation_replays_retained_layout_structure_not_only_fingerprint() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 0x46,
        entries: vec![LayoutFieldEntryReport {
            field: "word".into(),
            member_identity: Some(7),
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(8),
        align: 8,
    };
    let mut plan = AccessPlan::inaccessible(&layout).expect("canonical access seed");
    let compact_identity = plan.layout_report_fingerprint;
    plan.retained_layout.entries[0].placement = LayoutPlacementReport::At { offset: 4 };
    assert_eq!(
        plan.layout_report_fingerprint, compact_identity,
        "the simulated carrier drift deliberately leaves its compact identity unchanged"
    );

    let error = validate_access_plan(plan, &layout)
        .expect_err("structural carrier drift must reject before access-plan sealing");
    assert!(
        error.0.contains("different validated layout"),
        "{}",
        error.0
    );
}

#[test]
fn access_identity_covers_operation_width_and_exposure() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "word".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(8),
        align: 8,
    };
    let validate = |access: FieldAccess| {
        validate_access_plan(access_plan(&layout, &[("word", access)]), &layout)
            .expect("identity test plan")
            .identity()
    };
    let stable_read = FieldAccess::Stable {
        transfer_width_bits: 32,
        read: true,
        write: false,
        exposure: AccessExposure::Exported,
    };
    let mut stable_write = stable_read.clone();
    let FieldAccess::Stable { read, write, .. } = &mut stable_write else {
        unreachable!()
    };
    *read = false;
    *write = true;
    let mut wider = stable_read.clone();
    let FieldAccess::Stable {
        transfer_width_bits,
        ..
    } = &mut wider
    else {
        unreachable!()
    };
    *transfer_width_bits = 64;
    let mut private = stable_read.clone();
    let FieldAccess::Stable { exposure, .. } = &mut private else {
        unreachable!()
    };
    *exposure = AccessExposure::BindingPrivate;
    let external = FieldAccess::External {
        transfer_width_bits: 32,
        read: ExternalRead::Read,
        write: false,
        exposure: AccessExposure::Exported,
    };

    let identities = [
        validate(stable_read),
        validate(stable_write),
        validate(wider),
        validate(private),
        validate(external),
    ];
    for (index, identity) in identities.iter().enumerate() {
        assert!(
            identities[index + 1..]
                .iter()
                .all(|other| other != identity),
            "every semantic policy change must alter normalized identity"
        );
    }
}

#[test]
fn placement_identity_owns_normalized_reach() {
    let layout = uart_layout();
    let access = uart_access_source(&layout);
    let uart = validate_placement_plan(PlacementPlan {
        layout: layout.clone(),
        access: access.clone(),
        reach: BoundaryReach::from_services([reach(), reach()]),
    })
    .expect("UART placement");
    let alternate_reach =
        BoundaryServiceReachId::from_normalized_identity(8).expect("alternate reach");
    let alternate = validate_placement_plan(PlacementPlan {
        layout,
        access,
        reach: BoundaryReach::from_services([alternate_reach]),
    })
    .expect("alternate placement reach");
    assert_eq!(
        uart.reach().services().len(),
        1,
        "reach is a normalized set"
    );
    assert_eq!(uart.access().identity(), alternate.access().identity());
    assert_ne!(uart.identity(), alternate.identity());
}

#[test]
fn uart_access_plan_validates_geometry_and_borrow_polarity() {
    let plan = uart_access_plan();

    let status = plan
        .authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Shared,
            BorrowPolarity::Shared,
            AccessOperation::Read,
        )
        .expect("shared snapshot read");
    assert_eq!(status.descriptor().field(), "status");
    assert_eq!(status.descriptor().container_byte_offset(), 0);
    assert_eq!(status.descriptor().transfer_width_bits(), 32);
    assert_eq!(
        status.descriptor().observation(),
        ObservationModel::External
    );
    assert_eq!(status.current_borrow(), BorrowPolarity::Shared);
    assert_eq!(status.source_loan(), BorrowPolarity::Shared);
    assert_eq!(status.operation(), AccessOperation::Read);
    assert_eq!(plan.field_descriptors().len(), 3);
    let control = plan
        .field_descriptor(field_key(&plan, "control"))
        .expect("control descriptor");
    assert_eq!(control.container_byte_offset(), 8);
    assert_eq!(
        control.logical_extent().fragments(),
        &[LogicalFieldFragment {
            layout_bit_offset: 64,
            source_bit_offset: 0,
            width_bits: 8,
        }]
    );
    assert_eq!(
        control.effect_footprint(),
        RelativeEffectFootprint {
            byte_offset: 8,
            length_bytes: 4,
        },
        "a narrow logical bitfield retains its whole transfer container"
    );
    assert!(
        plan.authorize(
            field_key(&plan, "transmit"),
            BorrowPolarity::Shared,
            BorrowPolarity::Exclusive,
            AccessOperation::Write,
        )
        .is_err()
    );
    plan.authorize(
        field_key(&plan, "transmit"),
        BorrowPolarity::Exclusive,
        BorrowPolarity::Exclusive,
        AccessOperation::Write,
    )
    .expect("exclusive whole write");
    assert!(
        plan.authorize(
            field_key(&plan, "control"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::CompoundMutation,
        )
        .is_err(),
        "external storage never derives compound mutation"
    );
}

#[test]
fn stable_compound_mutation_is_derived_from_permissions_and_borrow() {
    let layout = uart_layout();
    let plan = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "status",
                FieldAccess::Stable {
                    transfer_width_bits: 32,
                    read: true,
                    write: true,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &layout,
    )
    .expect("stable read-write plan");
    plan.authorize(
        field_key(&plan, "status"),
        BorrowPolarity::Exclusive,
        BorrowPolarity::Exclusive,
        AccessOperation::CompoundMutation,
    )
    .expect("exclusive stable read-write access derives compound mutation");
    assert!(
        plan.authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Shared,
            BorrowPolarity::Exclusive,
            AccessOperation::CompoundMutation,
        )
        .is_err()
    );
    assert!(
        plan.authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Shared,
            AccessOperation::CompoundMutation,
        )
        .is_err(),
        "an exclusive current borrow cannot upgrade a shared source loan"
    );

    let plan = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "status",
                FieldAccess::Stable {
                    transfer_width_bits: 32,
                    read: true,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &layout,
    )
    .expect("stable read-only plan");
    assert!(
        plan.authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::CompoundMutation,
        )
        .is_err()
    );
}

#[test]
fn destructive_external_read_does_not_derive_readable() {
    let layout = uart_layout();
    let plan = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "status",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Take,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &layout,
    )
    .expect("destructive external plan");
    assert!(
        plan.authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Shared,
            BorrowPolarity::Exclusive,
            AccessOperation::Read,
        )
        .is_err()
    );
    assert!(
        plan.authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Shared,
            BorrowPolarity::Exclusive,
            AccessOperation::Take,
        )
        .is_err()
    );
    plan.authorize(
        field_key(&plan, "status"),
        BorrowPolarity::Exclusive,
        BorrowPolarity::Exclusive,
        AccessOperation::Take,
    )
    .expect("destructive read requires exclusive access");
}

#[test]
fn narrow_external_write_rejects_before_admission() {
    let layout = uart_layout();
    let error = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "control",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Read,
                    write: true,
                    exposure: AccessExposure::BindingPrivate,
                },
            )],
        ),
        &layout,
    )
    .expect_err("a narrow External write would require a generic RMW");
    assert!(
        error.0.contains("complete admitted container"),
        "diagnostic must explain the whole-transfer requirement: {error}"
    );
}

#[test]
fn destructive_access_requires_one_whole_snapshot_accessor() {
    let layout = uart_layout();
    let error = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "control",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Take,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &layout,
    )
    .expect_err("a narrow field cannot independently consume its container");
    assert!(
        error
            .0
            .contains("only part of its 4-byte transfer container")
    );

    let aliased_layout = LayoutPlanReport {
        schema_report_fingerprint: 0xdead,
        entries: vec![
            LayoutFieldEntryReport {
                field: "snapshot".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "status".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            },
        ],
        offsets: Some(vec![0, 0]),
        size: Some(4),
        align: 4,
    };
    let error = validate_access_plan(
        access_plan(
            &aliased_layout,
            &[
                (
                    "snapshot",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Take,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                ),
                (
                    "status",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                ),
            ],
        ),
        &aliased_layout,
    )
    .expect_err("one destructive unit cannot expose a second field accessor");
    assert!(error.0.contains("one whole-snapshot take"));
}

#[test]
fn external_compound_mutation_rejects() {
    let layout = uart_layout();
    let plan = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "status",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Read,
                    write: true,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &layout,
    )
    .expect("external read-write access is valid");
    let error = plan
        .authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::CompoundMutation,
        )
        .expect_err("external access must never derive compound mutation");
    assert!(error.0.contains("does not permit"));
}

#[test]
fn empty_access_cases_reject_in_favor_of_inaccessible() {
    let layout = uart_layout();
    for access in [
        FieldAccess::Stable {
            transfer_width_bits: 32,
            read: false,
            write: false,
            exposure: AccessExposure::Exported,
        },
        FieldAccess::External {
            transfer_width_bits: 32,
            read: ExternalRead::None,
            write: false,
            exposure: AccessExposure::Exported,
        },
        FieldAccess::Atomic {
            transfer_width_bits: 32,
            operations: AtomicPermissions::default(),
            exposure: AccessExposure::Exported,
        },
    ] {
        let error = validate_access_plan(access_plan(&layout, &[("status", access)]), &layout)
            .expect_err("empty access case must reject");
        assert!(error.0.contains("Inaccessible"));
    }
}

#[test]
fn atomic_shared_page_exposes_only_atomic_mutation() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "head".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 4,
    };
    let plan = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "head",
                FieldAccess::Atomic {
                    transfer_width_bits: 32,
                    operations: AtomicPermissions {
                        load: true,
                        store: true,
                        fetch_add: true,
                        compare_exchange: true,
                        ..AtomicPermissions::default()
                    },
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &layout,
    )
    .expect("atomic IPC plan");

    let mut alternate_source = plan.plan().clone();
    let FieldAccess::Atomic {
        operations: alternate_permissions,
        ..
    } = &mut alternate_source.entries[0].access
    else {
        panic!("atomic field decision")
    };
    alternate_permissions.fetch_add = false;
    alternate_permissions.fetch_sub = true;
    let alternate = validate_access_plan(alternate_source, &layout).expect("alternate atomic plan");
    assert_ne!(
        plan.identity(),
        alternate.identity(),
        "distinct atomic operation families must alter normalized identity"
    );

    let store = AccessOperation::Atomic(AtomicAccessOperation::Store(MemoryOrdering::Publish));
    plan.authorize(
        field_key(&plan, "head"),
        BorrowPolarity::Shared,
        BorrowPolarity::Shared,
        store,
    )
    .expect("shared mutation is explicitly atomic");
    plan.authorize(
        field_key(&plan, "head"),
        BorrowPolarity::Shared,
        BorrowPolarity::Shared,
        AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(
            MemoryOrdering::ReceivePublish,
        )),
    )
    .expect("admitted fetch-add");
    assert!(
        plan.authorize(
            field_key(&plan, "head"),
            BorrowPolarity::Shared,
            BorrowPolarity::Shared,
            AccessOperation::Atomic(AtomicAccessOperation::FetchSub(
                MemoryOrdering::ReceivePublish
            )),
        )
        .is_err(),
        "one admitted fetch family does not imply another"
    );
    let invalid_load =
        AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Publish));
    let error = plan
        .authorize(
            field_key(&plan, "head"),
            BorrowPolarity::Shared,
            BorrowPolarity::Shared,
            invalid_load,
        )
        .expect_err("Publish cannot order an atomic load");
    assert!(error.0.contains("invalid ordering"));
    assert!(
        plan.authorize(
            field_key(&plan, "head"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::Write,
        )
        .is_err()
    );

    let placement = validate_placement_plan(PlacementPlan {
        layout: layout.clone(),
        access: plan.plan().clone(),
        reach: BoundaryReach::default(),
    })
    .expect("atomic placement plan");
    let extent = uart_extent(0x2000, 4);
    let loan = extent.loan(0, 4).expect("shared atomic loan");
    let required_rights = extent_rights(&[3]);
    let resources = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(11).expect("profile receipt"),
        &extent,
        required_rights,
        BoundaryReach::default(),
    )
    .expect("atomic profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
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
                        store: true,
                        fetch_add: true,
                        compare_exchange: true,
                        ..AtomicPermissions::default()
                    },
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted atomic profile");
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(10).expect("atomic admission");
    let admission = admit_placement(admission_id, loan, &placement, &resources)
        .expect("admitted atomic placement");
    let view = place(admission).expect("atomic placed-view establishment");
    let head = view
        .project(field_key(placement.access(), "head"))
        .expect("pure atomic projection");
    let request = head
        .atomic_compare_exchange(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
        .expect("authorized compare-exchange")
        .into_primitive_request();
    assert_eq!(request.plan(), placement.identity());
    assert_eq!(request.admission(), admission_id);
    assert_eq!(
        request.profile_receipt(),
        ResourceProfileReceiptId::from_normalized_identity(11).expect("profile receipt")
    );
    assert_eq!(
        request.effective_supply().kind(),
        EffectiveSupplyKind::Atomic
    );
    assert_eq!(request.effective_supply().alignment_bytes(), 4);
    assert_eq!(request.primitive_address(), 0x2000);
    assert_eq!(request.field(), "head");
    assert_eq!(request.transfer_width_bits(), 32);
    assert_eq!(request.observation(), ObservationModel::Atomic);
    assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
    assert_eq!(request.source_loan(), BorrowPolarity::Shared);
    assert_eq!(
        request.operation(),
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchange {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        })
    );
    assert_eq!(request.reach(), &BoundaryReach::default());
}

#[test]
fn compare_exchange_permissions_keep_both_axes_distinct() {
    let decisive_ordering = AtomicAccessOperation::CompareExchange {
        success: MemoryOrdering::ReceivePublish,
        failure: MemoryOrdering::Receive,
    }
    .ordering_plan();
    let once_ordering = AtomicAccessOperation::CompareExchangeOnce {
        success: MemoryOrdering::ReceivePublish,
        failure: MemoryOrdering::Receive,
    }
    .ordering_plan();
    assert!(matches!(
        decisive_ordering,
        AtomicOrderingPlan::CompareExchange { .. }
    ));
    assert!(matches!(
        once_ordering,
        AtomicOrderingPlan::CompareExchangeOnce { .. }
    ));
    assert_ne!(decisive_ordering, once_ordering);

    let permissions = [
        AtomicPermissions {
            compare_exchange: true,
            ..AtomicPermissions::default()
        },
        AtomicPermissions {
            compare_exchange_once: true,
            ..AtomicPermissions::default()
        },
        AtomicPermissions {
            try_exchange: true,
            ..AtomicPermissions::default()
        },
        AtomicPermissions {
            try_exchange_once: true,
            ..AtomicPermissions::default()
        },
    ];
    for (provided_index, provided) in permissions.iter().copied().enumerate() {
        assert!(provided.any());
        assert!(provided.contains(provided));
        for (required_index, required) in permissions.iter().copied().enumerate() {
            if provided_index != required_index {
                assert!(
                    !provided.contains(required),
                    "compare-exchange permission row {provided_index} must not cover row {required_index}"
                );
            }
        }
    }

    let layout = LayoutPlanReport {
        schema_report_fingerprint: 0xce01,
        entries: vec![LayoutFieldEntryReport {
            field: "word".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 4,
    };
    let once_only = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "word",
                FieldAccess::Atomic {
                    transfer_width_bits: 32,
                    operations: AtomicPermissions {
                        compare_exchange_once: true,
                        ..AtomicPermissions::default()
                    },
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &layout,
    )
    .expect("single-attempt compare-exchange access plan");
    let key = field_key(&once_only, "word");
    once_only
        .authorize(
            key,
            BorrowPolarity::Shared,
            BorrowPolarity::Shared,
            AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            }),
        )
        .expect("the exact single-attempt family is independently admitted");
    let decisive = once_only
        .authorize(
            key,
            BorrowPolarity::Shared,
            BorrowPolarity::Shared,
            AccessOperation::Atomic(AtomicAccessOperation::CompareExchange {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            }),
        )
        .expect_err("single-attempt permission must not admit decisive exchange");
    assert!(decisive.0.contains("does not permit"));
    let invalid_ordering = once_only
        .authorize(
            key,
            BorrowPolarity::Shared,
            BorrowPolarity::Shared,
            AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce {
                success: MemoryOrdering::Receive,
                failure: MemoryOrdering::GlobalOrder,
            }),
        )
        .expect_err("single-attempt exchange uses the exact compare-exchange ordering law");
    assert!(invalid_ordering.0.contains("invalid ordering"));
}

#[test]
fn overlapping_atomic_fields_cannot_select_mixed_widths() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 0xa70,
        entries: vec![
            LayoutFieldEntryReport {
                field: "wide".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "upper".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 4 },
            },
        ],
        offsets: Some(vec![0, 4]),
        size: Some(8),
        align: 8,
    };
    let atomic_load = |transfer_width_bits| FieldAccess::Atomic {
        transfer_width_bits,
        operations: AtomicPermissions {
            load: true,
            ..AtomicPermissions::default()
        },
        exposure: AccessExposure::Exported,
    };
    let error = validate_access_plan(
        access_plan(
            &layout,
            &[("wide", atomic_load(64)), ("upper", atomic_load(32))],
        ),
        &layout,
    )
    .expect_err("one active placement cannot mix overlapping atomic widths");
    assert!(
        error.0.contains("overlapping transfer containers") && error.0.contains("mix widths"),
        "diagnostic must identify both the overlap and granularity conflict: {error}"
    );
}

#[test]
fn multi_container_fragments_are_not_one_access() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "entry".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 32,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 16,
                },
            },
            LayoutFieldEntryReport {
                field: "entry".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 4,
                    container_width: 32,
                    destination_lsb: 0,
                    source_lsb: 16,
                    width: 16,
                },
            },
        ],
        offsets: None,
        size: Some(8),
        align: 4,
    };
    let error = validate_access_plan(
        access_plan(
            &layout,
            &[(
                "entry",
                FieldAccess::Stable {
                    transfer_width_bits: 32,
                    read: true,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        &layout,
    )
    .expect_err("one token cannot hide two primitive accesses");
    assert!(error.0.contains("multiple containers"));
}

#[test]
fn field_keys_reject_cross_layout_and_out_of_cardinality_use() {
    let layout = uart_layout();
    let mut plan = AccessPlan::inaccessible(&layout).expect("UART seed");
    let mut alternate_layout = layout.clone();
    alternate_layout.schema_report_fingerprint = 2;
    let mut alternate = AccessPlan::inaccessible(&alternate_layout).expect("alternate schema seed");
    alternate.layout_report_fingerprint = plan.layout_report_fingerprint;
    for entry in &mut alternate.entries {
        entry.key.layout_report_fingerprint = plan.layout_report_fingerprint;
    }
    assert_eq!(
        plan.layout_report_fingerprint, alternate.layout_report_fingerprint,
        "the adversarial key substitution holds the compact report coordinate equal"
    );
    assert_ne!(plan.layout_commitment, alternate.layout_commitment);
    let error = plan
        .set(
            alternate.key_at(0).expect("alternate key"),
            FieldAccess::Stable {
                transfer_width_bits: 32,
                read: true,
                write: false,
                exposure: AccessExposure::Exported,
            },
        )
        .expect_err("cross-layout key must reject");
    assert!(error.0.contains("different validated layout"));

    let error = plan
        .set(
            AccessFieldKey {
                layout_report_fingerprint: plan.layout_report_fingerprint(),
                layout_commitment: plan.layout_commitment,
                slot: u32::MAX,
            },
            FieldAccess::Stable {
                transfer_width_bits: 32,
                read: true,
                write: false,
                exposure: AccessExposure::Exported,
            },
        )
        .expect_err("out-of-cardinality key must reject");
    assert!(error.0.contains("outside the schema cardinality"));
}
