use super::*;
use psi_extents::{
    AddressSpaceId, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId,
    ExtentProvenanceId, ExtentRights,
};
use psi_layout_plans::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};

#[test]
fn stored_integer_geometry_uses_the_exact_encoded_width() {
    let layout = LayoutPlanReport {
        schema_identity: 1,
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

fn reach() -> BoundaryServiceReachId {
    BoundaryServiceReachId::from_normalized_identity(7).expect("normalized reach")
}

fn uart_reach() -> BoundaryReach {
    BoundaryReach::from_services([reach()])
}

fn uart_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_identity: 1,
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
        schema_identity: 0x44,
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
        schema_identity: 0x45,
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
        schema_identity: 0x46,
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
    let compact_identity = plan.layout_fingerprint;
    plan.retained_layout.entries[0].placement = LayoutPlacementReport::At { offset: 4 };
    assert_eq!(
        plan.layout_fingerprint, compact_identity,
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
        schema_identity: 1,
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
        schema_identity: 0xdead,
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
        schema_identity: 1,
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
        schema_identity: 0xce01,
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
        schema_identity: 0xa70,
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
        schema_identity: 1,
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
    alternate_layout.schema_identity = 2;
    let alternate = AccessPlan::inaccessible(&alternate_layout).expect("alternate schema seed");
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
                layout_fingerprint: plan.layout_fingerprint(),
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

fn extent_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn provider_issuance(seed: u64) -> psi_extents::ExtentProviderIssuance {
    let base = seed * 16;
    psi_extents::ExtentProviderIssuance::from_normalized_identities([
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
    ExtentRights::from_normalized_identities(identities.iter().copied().map(|identity| {
        extent_id(
            identity,
            psi_extents::ExtentRightId::from_normalized_identity,
        )
    }))
}

fn uart_extent(base: u64, length: u64) -> psi_extents::Extent {
    uart_extent_with_lineage(base, length, 1)
}

fn uart_extent_with_lineage(base: u64, length: u64, lineage: u64) -> psi_extents::Extent {
    uart_extent_with_root(base, length, 1, lineage)
}

fn uart_extent_with_root(
    base: u64,
    length: u64,
    provider: u64,
    lineage: u64,
) -> psi_extents::Extent {
    uart_root_grant(provider, lineage)
        .mint(base, length)
        .expect("UART extent")
}

fn uart_root_grant(provider: u64, lineage: u64) -> psi_extents::ExtentRootGrant {
    uart_root_grant_with_mapping(provider, lineage, 5, 6)
}

fn uart_root_grant_with_mapping(
    provider: u64,
    lineage: u64,
    provenance: u64,
    era: u64,
) -> psi_extents::ExtentRootGrant {
    psi_extents::ExtentRootGrant::from_admitted_provider(
        provider_issuance(provider),
        extent_id(
            lineage,
            psi_extents::ExtentLineageId::from_normalized_identity,
        ),
        extent_id(2, AddressSpaceId::from_normalized_identity),
        extent_rights(&[3, 4]),
        extent_id(provenance, ExtentProvenanceId::from_normalized_identity),
        extent_id(era, psi_extents::MappingEraId::from_normalized_identity),
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
        schema_identity: 0x5ab1e,
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
        schema_identity: 0xe17e_7a4e,
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
    let layout = LayoutPlanReport {
        schema_identity: 0xa70_1c,
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
                    operations: all_atomic_operations(),
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        layout,
        reach: BoundaryReach::default(),
    })
    .expect("all-family Atomic word placement")
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
        PlacementAuthorityRef::EstablishedOwned(established) => (
            "established-owned",
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
            extent_id(
                plan.identity().normalized_identity(),
                psi_extents::ExtentContentInterpretationId::from_normalized_identity,
            ),
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

#[test]
fn provider_correspondence_admits_against_exact_plan_and_profile_without_storage_join() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x7180, 12, 236);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(237)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(238).expect("stable device");
    let revision = RuntimeDeviceRevisionEvidence::from_admitted_provider(
        RuntimeDeviceRevisionObservationId::from_normalized_identity(239)
            .expect("revision observation"),
        DeviceRevisionPredicateId::from_normalized_identity(240).expect("revision predicate"),
        provider,
        device,
        profile.receipt(),
        3,
    );
    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(241).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        Some(revision),
    )
    .expect("provider correspondence grant");

    let mut colliding = plan.clone();
    colliding.layout.schema_identity ^= 1;
    assert_eq!(colliding.identity(), plan.identity());
    assert_ne!(colliding.layout(), plan.layout());
    let rejection = grant
        .admit(&colliding, &profile)
        .expect_err("compact placement identity cannot substitute exact plan structure");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("exact validated placement")
    );
    let (grant, _) = rejection.into_parts();

    let admitted = grant
        .admit(&plan, &profile)
        .expect("exact plan/profile correspondence admission");
    assert_eq!(admitted.placement(), plan.identity());
    assert_eq!(admitted.profile_receipt(), profile.receipt());
    assert_eq!(admitted.provider(), provider);
    assert_eq!(admitted.device(), device);
    assert_eq!(
        admitted
            .revision()
            .expect("runtime revision evidence")
            .observed_revision(),
        3
    );
}

#[test]
fn correspondence_binding_replays_placement_and_returns_both_inputs_for_retry() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x7190, 12, 242);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(243).expect("placement admission");
    let mut admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(244)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(245).expect("stable device");
    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(246).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant");
    let correspondence = grant
        .admit(&plan, &profile)
        .expect("schema correspondence admission");

    admission.placement_plan.layout.schema_identity ^= 1;
    assert_eq!(admission.placement_plan.identity(), plan.identity());
    let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect_err("same compact identity cannot hide placement structure drift");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (mut admission, mut correspondence, _) = rejection.into_parts();
    admission.placement_plan.layout.schema_identity = plan.layout().schema_identity;

    correspondence.replace_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect_err("placement identity drift must reject");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (mut admission, mut correspondence, _) = rejection.into_parts();
    assert_eq!(admission.identity(), admission_id);
    correspondence.replace_placement_for_test(plan.identity());

    admission.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect_err("admission receipt drift must reject");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (mut admission, correspondence, _) = rejection.into_parts();
    admission.profile_receipt = profile.receipt();

    let bound = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("repaired inputs remain valid for retry");
    assert_eq!(bound.admission(), admission_id);
    assert_eq!(bound.correspondence().provider(), provider);
    assert_eq!(bound.correspondence().device(), device);
    let (loan, correspondence) = bound.withdraw();
    assert_eq!(loan.base(), 0x7190);
    assert_eq!(loan.length(), 12);
    assert_eq!(correspondence.placement(), plan.identity());
}

#[test]
fn corresponded_view_establishment_replays_both_inputs_and_preserves_retry() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x71a0, 12, 247);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(248).expect("placement admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(249)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(250).expect("stable device");
    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(251).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant");
    let correspondence = grant
        .admit(&plan, &profile)
        .expect("schema correspondence admission");
    let mut bound = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding");

    bound.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = bound
        .establish_view()
        .expect_err("establishment must independently replay correspondence");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (mut bound, _) = rejection.into_parts();
    assert_eq!(bound.admission(), admission_id);
    bound.replace_correspondence_placement_for_test(plan.identity());

    let view = bound
        .establish_view()
        .expect("repaired bound carrier remains valid for retry");
    assert_eq!(view.admission(), admission_id);
    assert_eq!(view.base(), 0x71a0);
    assert_eq!(view.length(), 12);
    assert_eq!(view.correspondence().provider(), provider);
    assert_eq!(view.correspondence().device(), device);
    assert_eq!(view.correspondence().placement(), plan.identity());
}

#[test]
fn corresponded_view_retirement_replays_both_authorities_and_returns_exact_inputs() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x71a8, 12, 259);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(260).expect("placement admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("borrowed placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(261)
        .expect("correspondence provider");
    let device =
        StableDeviceInstanceId::from_normalized_identity(262).expect("stable device instance");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(263).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");

    let drifted_receipt =
        ResourceProfileReceiptId::from_normalized_identity(264).expect("drifted receipt");
    view.replace_view_profile_receipt_for_test(drifted_receipt);
    view.replace_correspondence_profile_receipt_for_test(drifted_receipt);
    let rejection = view
        .retire()
        .expect_err("coordinated copied receipt drift must reject retirement");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("admitted resource-profile receipt")
    );
    let (mut view, _) = rejection.into_parts();
    view.replace_view_profile_receipt_for_test(profile.receipt());
    view.replace_correspondence_profile_receipt_for_test(profile.receipt());

    view.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = view
        .retire()
        .expect_err("physical correspondence drift must reject retirement");
    assert!(rejection.diagnostic().0.contains("exact placement"));
    let (mut view, _) = rejection.into_parts();
    view.replace_correspondence_placement_for_test(plan.identity());

    let (loan, correspondence) = view
        .retire()
        .expect("repaired view remains valid for retirement retry");
    assert_eq!(loan.origin(), origin);
    assert_eq!(loan.lineage_root(), lineage);
    assert_eq!(loan.base(), 0x71a8);
    assert_eq!(loan.length(), 12);
    assert_eq!(loan.polarity(), LoanPolarity::Shared);
    assert_eq!(correspondence.provider(), provider);
    assert_eq!(correspondence.device(), device);
    assert_eq!(correspondence.placement(), plan.identity());
    assert_eq!(correspondence.profile_receipt(), profile.receipt());
}

#[test]
fn corresponded_view_retains_and_replays_evidence_through_primitive_specialization() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x71b0, 12, 252);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(253).expect("placement admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("borrowed placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(254)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(255).expect("stable device");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(256).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");
    let status = field_key(plan.access(), "status");

    view.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = view
        .project(status)
        .expect_err("projection must replay retained correspondence");
    assert!(rejection.0.contains("schema/device correspondence"));
    view.replace_correspondence_placement_for_test(plan.identity());

    let projection = view
        .project(status)
        .expect("repaired correspondence remains available for retry");
    assert_eq!(
        projection
            .correspondence()
            .expect("corresponded projection")
            .provider(),
        provider
    );
    let access = projection.read().expect("External status read");
    assert_eq!(
        access
            .correspondence()
            .expect("corresponded authorized access")
            .device(),
        device
    );
    let request = access.into_primitive_request();
    assert_eq!(
        request
            .correspondence()
            .expect("corresponded primitive request")
            .placement(),
        plan.identity()
    );
    let exact_request = primitive_request_snapshot(&request);
    let external = request
        .into_external_primitive_access()
        .expect("External specialization replays correspondence");
    assert_eq!(
        primitive_request_snapshot(external.primitive_request()),
        exact_request,
        "outward specialization must retain the exact sealed request"
    );
    assert_eq!(
        external
            .correspondence()
            .expect("correspondence reaches outward specialization")
            .device(),
        device
    );
    let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(259)
            .expect("alternate correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(260).expect("alternate stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(261)
            .expect("alternate datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("alternate provider correspondence grant")
    .admit(&plan, &profile)
    .expect("alternate schema correspondence admission");
    let mut corresponded = external
        .into_corresponded_external_access()
        .expect("provider/device preflight requires retained correspondence");
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        primitive_request_snapshot(corresponded.external_access().primitive_request()),
        exact_request
    );
    let retained_correspondence =
        corresponded.replace_correspondence_for_test(&alternate_correspondence);
    let rejection = corresponded
        .validate_for_provider_lowering()
        .expect_err("a distinct correspondence carrier cannot replace retained authority");
    assert!(
        rejection
            .0
            .contains("different schema/device correspondence")
    );
    corresponded.replace_correspondence_for_test(retained_correspondence);
    corresponded
        .validate_for_provider_lowering()
        .expect("restoring the exact correspondence carrier permits retry");

    corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = corresponded
        .validate_for_provider_lowering()
        .expect_err("provider/device preflight must replay the retained placement");
    assert!(rejection.0.contains("copied plan"));
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        corresponded.external_access().primitive_request().plan(),
        PlacementPlanId(plan.identity().0 ^ 1),
        "borrowed request inspection reflects the still-retained drifted carrier"
    );
    corresponded.replace_request_plan_for_test(plan.identity());
    corresponded
        .validate_for_provider_lowering()
        .expect("repaired outward carrier remains available for retry");
    assert_eq!(
        primitive_request_snapshot(corresponded.external_access().primitive_request()),
        exact_request
    );
    let request = corresponded.into_external_access().into_primitive_request();
    assert_eq!(
        request
            .correspondence()
            .expect("retained evidence")
            .provider(),
        provider
    );

    let ordinary_extent = uart_extent_with_lineage(0x71c0, 12, 257);
    let ordinary_loan = ordinary_extent.loan(0, 12).expect("ordinary shared loan");
    let ordinary = place(
        admit_uart(258, ordinary_loan, &plan, &uart_reach()).expect("ordinary placement admission"),
    )
    .expect("ordinary view establishment");
    let ordinary_projection = ordinary.project(status).expect("ordinary projection");
    assert!(ordinary_projection.correspondence().is_none());
    let ordinary_request = ordinary_projection
        .read()
        .expect("ordinary External read")
        .into_primitive_request();
    let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
    let rejection = ordinary_request
        .into_external_primitive_access()
        .expect("ordinary External specialization remains valid")
        .into_corresponded_external_access()
        .expect_err("device/provider preflight must reject correspondence-free storage");
    assert!(rejection.diagnostic().0.contains("requires admitted"));
    let (ordinary_external, _) = rejection.into_parts();
    assert_eq!(
        primitive_request_snapshot(ordinary_external.primitive_request()),
        ordinary_snapshot,
        "rejection must return the exact already-specialized External request"
    );
    ordinary_external
        .validate_for_lowering()
        .expect("returned correspondence-free request remains valid for another consumer");
}

#[test]
fn borrowed_admission_withdraws_the_exact_shared_loan() {
    let plan = uart_placement_plan();
    let mut extent = uart_extent_with_lineage(0x7200, 32, 76);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let address_space = extent.address_space();
    let rights = extent.rights().clone();
    let provenance = extent.provenance();
    let era = extent.era();
    let loan = extent.loan(4, 12).expect("shared placement loan");
    let profile = uart_resource_profile(&loan, &uart_reach());

    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(77).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("borrowed shared placement admission");
    let returned = admission.withdraw();

    assert_eq!(returned.base(), 0x7204);
    assert_eq!(returned.length(), 12);
    assert_eq!(returned.polarity(), LoanPolarity::Shared);
    assert_eq!(returned.origin(), origin);
    assert_eq!(returned.lineage_root(), lineage);
    assert_eq!(returned.address_space(), address_space);
    assert_eq!(returned.rights(), &rights);
    assert_eq!(returned.provenance(), provenance);
    assert_eq!(returned.era(), era);

    drop(returned);
    drop(
        extent
            .loan_mut(0, 32)
            .expect("dropping the returned loan restores exclusive parent access"),
    );
}

#[test]
fn borrowed_admission_withdraws_the_exact_exclusive_loan() {
    let plan = uart_placement_plan();
    let mut extent = uart_extent_with_lineage(0x7300, 32, 78);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let address_space = extent.address_space();
    let rights = extent.rights().clone();
    let provenance = extent.provenance();
    let era = extent.era();
    let loan = extent.loan_mut(8, 12).expect("exclusive placement loan");
    let profile = uart_resource_profile(&loan, &uart_reach());

    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(79).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("borrowed exclusive placement admission");
    let returned = admission.withdraw();

    assert_eq!(returned.base(), 0x7308);
    assert_eq!(returned.length(), 12);
    assert_eq!(returned.polarity(), LoanPolarity::Exclusive);
    assert_eq!(returned.origin(), origin);
    assert_eq!(returned.lineage_root(), lineage);
    assert_eq!(returned.address_space(), address_space);
    assert_eq!(returned.rights(), &rights);
    assert_eq!(returned.provenance(), provenance);
    assert_eq!(returned.era(), era);

    drop(returned);
    drop(
        extent
            .loan(0, 32)
            .expect("dropping the returned loan restores shared parent access"),
    );
}

#[test]
fn owned_admission_retains_and_withdraws_the_exact_extent() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x7000, 12, 72);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let profile = uart_resource_profile_for_extent(&extent, &uart_reach());

    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(73).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned whole-range placement admission");
    assert_eq!(admission.identity().normalized_identity(), 73);
    assert_eq!(admission.extent().base(), 0x7000);
    assert_eq!(admission.extent().length(), 12);
    assert_eq!(admission.extent().origin(), origin);
    assert_eq!(admission.extent().lineage_root(), lineage);
    assert_eq!(admission.placement_plan().identity(), plan.identity());

    let returned = admission.withdraw();
    assert_eq!(returned.base(), 0x7000);
    assert_eq!(returned.length(), 12);
    assert_eq!(returned.origin(), origin);
    assert_eq!(returned.lineage_root(), lineage);
}

#[test]
fn owned_admission_rejection_returns_the_exact_extent() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x7100, 8, 74);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let profile = uart_resource_profile_for_extent(&extent, &uart_reach());

    let rejection = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(75).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect_err("the complete placement must fit the owned extent");
    assert!(rejection.diagnostic().0.contains("exceeds"));
    let (returned, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("exceeds"));
    assert_eq!(returned.base(), 0x7100);
    assert_eq!(returned.length(), 8);
    assert_eq!(returned.origin(), origin);
    assert_eq!(returned.lineage_root(), lineage);
}

#[test]
fn provider_existing_content_establishes_owned_stable_placement() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xa000, 4, 92, 93);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let address_space = extent.address_space();
    let provenance = extent.provenance();
    let era = extent.era();
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(95).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");

    let dormant =
        adopt_owned_stable(admission, content).expect("provider-evidenced Stable adoption");
    assert_eq!(dormant.admission().normalized_identity(), 95);
    assert_eq!(dormant.placement_plan().identity(), plan.identity());
    assert_eq!(dormant.extent().base(), 0xa000);
    assert_eq!(dormant.extent().length(), 4);
    assert_eq!(dormant.extent().origin(), origin);
    assert_eq!(dormant.extent().lineage_root(), lineage);
    assert_eq!(dormant.extent().address_space(), address_space);
    assert_eq!(dormant.extent().provenance(), provenance);
    assert_eq!(dormant.extent().era(), era);
    assert_eq!(dormant.profile_receipt().normalized_identity(), 91);
    assert_eq!(dormant.resident_claim().normalized_identity(), 95);
    assert_eq!(dormant.validity_receipt().normalized_identity(), 93);
    assert_eq!(dormant.custody_receipt().normalized_identity(), 94);

    let established = dormant
        .view(PlacedOccurrenceId::from_normalized_identity(96).expect("placed occurrence"))
        .expect("owned resident-view establishment");
    assert_eq!(established.admission().normalized_identity(), 95);
    assert_eq!(established.placement_plan().identity(), plan.identity());
    assert_eq!(established.extent().base(), 0xa000);
    assert_eq!(established.extent().length(), 4);
    assert_eq!(established.resident_claim().normalized_identity(), 95);
    assert_eq!(established.occurrence().normalized_identity(), 96);
    assert_eq!(established.validity_receipt().normalized_identity(), 93);
    assert_eq!(established.custody_receipt().normalized_identity(), 94);
}

#[test]
fn stable_adoption_replays_profile_and_returns_both_inputs_for_retry() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xad80, 4, 191, 192);
    let extent_origin = extent.origin();
    let extent_lineage = extent.lineage_root();
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(195).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");

    let coincident = uart_extent_with_lineage(0xad80, 4, 196);
    let wrong_profile = stable_word_profile(&coincident);
    let OwnedPlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: _,
        resources,
        extent,
    } = admission;
    let corrupt = OwnedPlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: wrong_profile,
        resources,
        extent,
    };

    let rejection = adopt_owned_stable(corrupt, content)
        .expect_err("Stable adoption must replay admitted profile root facts");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("replay the admitted resource profile"),
        "{}",
        rejection.diagnostic()
    );
    let (returned, content, _) = rejection.into_parts();
    assert_eq!(returned.extent().origin(), extent_origin);
    assert_eq!(returned.extent().lineage_root(), extent_lineage);
    assert_eq!(content.resident_claim().normalized_identity(), 194);
    assert_eq!(content.validity_receipt().normalized_identity(), 192);
    assert_eq!(content.custody_receipt().normalized_identity(), 193);

    let OwnedPlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: _,
        resources,
        extent,
    } = returned;
    let repaired = OwnedPlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile,
        resources,
        extent,
    };
    let dormant = adopt_owned_stable(repaired, content)
        .expect("returned admission and content remain valid for corrected retry");
    assert_eq!(dormant.admission().normalized_identity(), 195);
    assert_eq!(dormant.resident_claim().normalized_identity(), 194);
}

#[test]
fn owned_resident_lifecycle_replays_full_provider_content_grant() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xad90, 4, 204, 205);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(208).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");
    let mut dormant = adopt_owned_stable(admission, content).expect("provider resident adoption");
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();

    let (replacement_extent, _replacement_content) =
        provider_existing_content(&plan, 0xad90, 4, 209, 210);
    let replacement_profile = stable_word_profile(&replacement_extent);
    let replacement_admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(213).expect("replacement admission"),
        replacement_extent,
        &plan,
        &replacement_profile,
    )
    .expect("coincident replacement placement");
    let retained_admission = std::mem::replace(&mut dormant.admission, replacement_admission);

    let occurrence = PlacedOccurrenceId::from_normalized_identity(214).expect("placed occurrence");
    let rejection = dormant
        .view(occurrence)
        .expect_err("resident view must replay the complete provider content grant");
    assert!(rejection.diagnostic().0.contains("provider content grant"));
    let (mut dormant, returned_occurrence, _) = rejection.into_parts();
    assert_eq!(returned_occurrence, occurrence);
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    let replacement_admission = std::mem::replace(&mut dormant.admission, retained_admission);

    let mut established = dormant
        .view(returned_occurrence)
        .expect("repaired dormant carrier supports corrected view");
    let retained_admission = std::mem::replace(&mut established.admission, replacement_admission);
    let rejection = established
        .retire_resident()
        .expect_err("resident retirement must replay the complete provider content grant");
    assert!(rejection.diagnostic().0.contains("provider content grant"));
    let (mut established, _) = rejection.into_parts();
    assert_eq!(established.occurrence(), occurrence);
    assert_eq!(established.resident_claim(), claim);
    assert_eq!(established.validity_receipt(), validity);
    assert_eq!(established.custody_receipt(), custody);
    established.admission = retained_admission;

    let dormant = established
        .retire_resident()
        .expect("returned active carrier supports corrected retirement");
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.admission().normalized_identity(), 208);
}

#[test]
fn owned_resident_view_and_retirement_preserve_claim_and_rotate_occurrence() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xa080, 4, 97, 98);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(101).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");
    let mut dormant = adopt_owned_stable(admission, content).expect("provider resident adoption");
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();
    assert_eq!(claim.normalized_identity(), 100);

    let first_occurrence =
        PlacedOccurrenceId::from_normalized_identity(102).expect("first occurrence");
    let coincident = uart_extent_with_lineage(0xa080, 4, 199);
    dormant.admission.profile = stable_word_profile(&coincident);
    let rejection = dormant
        .view(first_occurrence)
        .expect_err("owned resident view must replay retained placement authority");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("could not replay the retained placement authority"),
        "{}",
        rejection.diagnostic()
    );
    let (mut dormant, returned_occurrence, _) = rejection.into_parts();
    assert_eq!(returned_occurrence, first_occurrence);
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().base(), 0xa080);
    dormant.admission.profile = profile;
    let mut first = dormant
        .view(returned_occurrence)
        .expect("first owned resident-view establishment");
    assert_eq!(first.resident_claim(), claim);
    assert_eq!(first.occurrence(), first_occurrence);
    {
        let projection = first
            .project(field_key(plan.access(), "word"))
            .expect("resident field projection");
        assert_eq!(projection.resident_claim(), Some(claim));
        assert_eq!(projection.placed_occurrence(), Some(first_occurrence));
        let access = projection.read().expect("resident Stable read");
        assert_eq!(access.resident_claim(), Some(claim));
        assert_eq!(access.placed_occurrence(), Some(first_occurrence));
        let request = access.into_primitive_request();
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(first_occurrence));
    }

    let retained_profile = first.admission.profile.clone();
    let coincident = uart_extent_with_lineage(0xa080, 4, 200);
    first.admission.profile = stable_word_profile(&coincident);
    let rejection = first
        .retire_resident()
        .expect_err("resident retirement must replay retained placement authority");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("could not replay the retained placement authority"),
        "{}",
        rejection.diagnostic()
    );
    let (mut first, _) = rejection.into_parts();
    assert_eq!(first.resident_claim(), claim);
    assert_eq!(first.occurrence(), first_occurrence);
    assert_eq!(first.validity_receipt(), validity);
    assert_eq!(first.custody_receipt(), custody);
    first.admission.profile = retained_profile;
    let dormant = first
        .retire_resident()
        .expect("returned active resident supports corrected retirement");
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().base(), 0xa080);
    assert_eq!(dormant.placement_plan().identity(), plan.identity());

    let second_occurrence =
        PlacedOccurrenceId::from_normalized_identity(103).expect("second occurrence");
    let second = dormant
        .view(second_occurrence)
        .expect("second owned resident-view establishment");
    assert_eq!(second.resident_claim(), claim);
    assert_eq!(second.occurrence(), second_occurrence);
    assert_ne!(second.occurrence(), first_occurrence);
}

#[test]
fn borrowed_resident_views_retain_claim_receipts_and_exact_loan_polarity() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xa100, 4, 104, 105);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(108).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");
    let mut dormant = adopt_owned_stable(admission, content).expect("provider resident adoption");
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();
    let retained_profile = profile.clone();

    let shared_occurrence =
        PlacedOccurrenceId::from_normalized_identity(109).expect("shared occurrence");
    let coincident = uart_extent_with_lineage(0xa100, 4, 201);
    dormant.admission.profile = stable_word_profile(&coincident);
    let diagnostic = dormant
        .borrow_view(shared_occurrence)
        .expect_err("shared resident view must replay retained placement authority");
    assert!(diagnostic.0.contains("shared-view establishment"));
    assert!(diagnostic.0.contains("retained placement authority"));
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().base(), 0xa100);
    dormant.admission.profile = retained_profile.clone();
    {
        let mut borrowed = dormant
            .borrow_view(shared_occurrence)
            .expect("shared resident loan");
        assert_eq!(borrowed.base(), 0xa100);
        assert_eq!(borrowed.length(), 4);
        assert_eq!(borrowed.loan_polarity(), LoanPolarity::Shared);
        assert_eq!(borrowed.resident_claim(), claim);
        assert_eq!(borrowed.occurrence(), shared_occurrence);
        assert_eq!(borrowed.validity_receipt(), validity);
        assert_eq!(borrowed.custody_receipt(), custody);

        let projection = borrowed
            .project(field_key(plan.access(), "word"))
            .expect("shared resident field projection");
        let request = projection
            .read()
            .expect("shared resident read")
            .into_primitive_request();
        assert_eq!(request.source_loan(), BorrowPolarity::Shared);
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(shared_occurrence));
        assert_eq!(
            primitive_request_snapshot(&request).authority_kind,
            "borrowed-resident"
        );
        drop(request);

        let mut projection = borrowed
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive projection borrow over shared resident loan");
        let diagnostic = projection
            .write()
            .expect_err("shared resident loan cannot authorize a write");
        assert!(diagnostic.0.contains("Shared source loan"));

        let coincident = uart_extent_with_lineage(0xa100, 4, 203);
        let wrong_profile = stable_word_profile(&coincident);
        let correct_profile = borrowed.replace_profile_for_test(wrong_profile);
        let rejection = borrowed
            .retire()
            .expect_err("shared resident retirement must replay exact loan authority");
        assert!(rejection.diagnostic().0.contains("retirement"));
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("retained placement authority")
        );
        let (mut borrowed, _) = rejection.into_parts();
        assert_eq!(borrowed.resident_claim(), claim);
        assert_eq!(borrowed.occurrence(), shared_occurrence);
        assert_eq!(borrowed.validity_receipt(), validity);
        assert_eq!(borrowed.custody_receipt(), custody);
        borrowed.replace_profile_for_test(correct_profile);

        let (_coincident_extent, coincident_content) =
            provider_existing_content(&plan, 0xa100, 4, 215, 216);
        let correct_content = borrowed.replace_content_for_test(&coincident_content);
        let diagnostic = borrowed
            .project(field_key(plan.access(), "word"))
            .expect_err("borrowed projection must replay the exact resident content grant");
        assert!(diagnostic.0.contains("resident content grant"));
        let rejection = borrowed
            .retire()
            .expect_err("shared retirement must replay the exact borrowed content grant");
        assert!(rejection.diagnostic().0.contains("provider content grant"));
        let (mut borrowed, _) = rejection.into_parts();
        borrowed.replace_content_for_test(correct_content);
        assert_eq!(borrowed.resident_claim(), claim);
        assert_eq!(borrowed.validity_receipt(), validity);
        assert_eq!(borrowed.custody_receipt(), custody);
        borrowed
            .retire()
            .expect("returned shared resident carrier supports corrected retirement");
    }
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);

    let exclusive_occurrence =
        PlacedOccurrenceId::from_normalized_identity(110).expect("exclusive occurrence");
    let coincident = uart_extent_with_lineage(0xa100, 4, 202);
    dormant.admission.profile = stable_word_profile(&coincident);
    let diagnostic = dormant
        .borrow_view_mut(exclusive_occurrence)
        .expect_err("exclusive resident view must replay retained placement authority");
    assert!(diagnostic.0.contains("exclusive-view establishment"));
    assert!(diagnostic.0.contains("retained placement authority"));
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().base(), 0xa100);
    dormant.admission.profile = retained_profile;
    {
        let mut borrowed = dormant
            .borrow_view_mut(exclusive_occurrence)
            .expect("exclusive resident loan");
        assert_eq!(borrowed.loan_polarity(), LoanPolarity::Exclusive);
        let mut projection = borrowed
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive resident field projection");
        let request = projection
            .write()
            .expect("exclusive resident write")
            .into_primitive_request();
        assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(exclusive_occurrence));
        drop(request);
        borrowed
            .retire()
            .expect("exclusive resident retirement replays exact loan authority");
    }

    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.extent().base(), 0xa100);
    let owned_occurrence =
        PlacedOccurrenceId::from_normalized_identity(111).expect("owned occurrence");
    let owned = dormant
        .view(owned_occurrence)
        .expect("owned resident-view establishment");
    assert_eq!(owned.resident_claim(), claim);
    assert_eq!(owned.occurrence(), owned_occurrence);
}

#[test]
fn established_owned_stable_shared_projection_seals_a_read_request() {
    let (plan, established) = established_stable_word(0xa400, 112, 113, 115);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("Stable shared read")
        .into_primitive_request();

    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 115);
    assert_eq!(request.profile_receipt().normalized_identity(), 91);
    assert_eq!(
        request.effective_supply().kind(),
        EffectiveSupplyKind::Stable
    );
    assert_eq!(request.primitive_address(), 0xa400);
    assert_eq!(request.field(), "word");
    assert_eq!(request.observation(), ObservationModel::Stable);
    assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
    assert_eq!(request.operation(), AccessOperation::Read);
}

#[test]
fn established_owned_stable_exclusive_projection_seals_a_write_request() {
    let (plan, mut established) = established_stable_word(0xa500, 116, 117, 119);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .write()
        .expect("Stable exclusive write")
        .into_primitive_request();

    assert_eq!(request.primitive_address(), 0xa500);
    assert_eq!(request.observation(), ObservationModel::Stable);
    assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
    assert_eq!(request.operation(), AccessOperation::Write);
}

#[test]
fn established_owned_stable_shared_projection_rejects_write() {
    let (plan, established) = established_stable_word(0xa600, 120, 121, 123);
    let mut projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");

    let rejection = projection
        .write()
        .expect_err("shared current borrow must not authorize Stable write");
    assert!(rejection.0.contains("Shared current borrow"));
    assert_eq!(established.validity_receipt().normalized_identity(), 121);
    assert_eq!(established.custody_receipt().normalized_identity(), 122);
}

#[test]
fn established_owned_read_specializes_for_stable_primitive_lowering() {
    let (plan, established) = established_stable_word(0xa700, 124, 125, 127);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("Stable read")
        .into_primitive_request();
    let stable = request
        .into_stable_primitive_access()
        .expect("Stable read specialization");

    assert_eq!(stable.operation(), StablePrimitiveOperation::Read);
    assert_eq!(stable.primitive_address(), 0xa700);
    assert_eq!(stable.transfer_width_bits(), 32);
    assert_eq!(stable.effect_footprint().address(), 0xa700);
    assert_eq!(stable.effect_footprint().length_bytes(), 4);
    assert_eq!(stable.logical_extent().fragments().len(), 1);
    let request = stable.into_primitive_request();
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 127);
    assert_eq!(request.profile_receipt().normalized_identity(), 91);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
}

#[test]
fn stable_primitive_lowering_replays_authority_without_consuming_retry() {
    let (plan, established) = established_stable_word(0xa740, 224, 225, 227);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("Stable read")
        .into_primitive_request();
    let mut stable = request
        .into_stable_primitive_access()
        .expect("Stable read specialization");
    let expected = primitive_request_snapshot(&stable.request);

    stable.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let diagnostic = stable
        .validate_for_lowering()
        .expect_err("outward preflight must reject copied receipt drift");
    assert!(diagnostic.0.contains("retained placement authority"));
    stable.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(91).expect("profile receipt");

    stable.operation = StablePrimitiveOperation::Write;
    let diagnostic = stable
        .validate_for_lowering()
        .expect_err("outward preflight must reject specialization drift");
    assert!(diagnostic.0.contains("retained specialization"));
    stable.operation = StablePrimitiveOperation::Read;

    stable
        .validate_for_lowering()
        .expect("corrected carrier must remain valid for retry");
    assert_eq!(primitive_request_snapshot(&stable.request), expected);
    assert_eq!(stable.operation(), StablePrimitiveOperation::Read);
}

#[test]
fn provider_stable_preflight_requires_and_retains_exact_correspondence() {
    let plan = stable_word_placement();
    let extent = uart_extent_with_lineage(0xa780, 4, 272);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan(0, 4).expect("shared Stable loan");
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(273).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("Stable placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(274)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(275).expect("stable device");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(276).expect("provider provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");
    let word = view
        .project(field_key(plan.access(), "word"))
        .expect("Stable word projection");
    let request = word.read().expect("Stable read").into_primitive_request();
    let expected = primitive_request_snapshot(&request);
    let stable = request
        .into_stable_primitive_access()
        .expect("Stable read specialization");

    let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(277)
            .expect("alternate correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(278).expect("alternate stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(279)
            .expect("alternate provider provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("alternate provider correspondence grant")
    .admit(&plan, &profile)
    .expect("alternate schema correspondence admission");
    let mut corresponded = stable
        .into_corresponded_stable_access()
        .expect("provider/device Stable preflight requires retained correspondence");
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        corresponded.stable_access().operation(),
        StablePrimitiveOperation::Read
    );
    assert_eq!(
        primitive_request_snapshot(corresponded.stable_access().primitive_request()),
        expected
    );

    let retained_correspondence =
        corresponded.replace_correspondence_for_test(&alternate_correspondence);
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("a distinct correspondence carrier cannot replace retained authority");
    assert!(
        diagnostic
            .0
            .contains("different schema/device correspondence")
    );
    corresponded.replace_correspondence_for_test(retained_correspondence);

    corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("provider/device Stable preflight must replay placement authority");
    assert!(diagnostic.0.contains("copied plan"));
    corresponded.replace_request_plan_for_test(plan.identity());
    corresponded
        .validate_for_provider_lowering()
        .expect("restored exact carrier remains available for retry");
    assert_eq!(
        primitive_request_snapshot(corresponded.into_stable_access().primitive_request()),
        expected
    );

    let ordinary_extent = uart_extent_with_lineage(0xa790, 4, 280);
    let ordinary_profile = stable_word_profile(&ordinary_extent);
    let ordinary_loan = ordinary_extent
        .loan(0, 4)
        .expect("ordinary shared Stable loan");
    let ordinary = place(
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(281).expect("ordinary admission"),
            ordinary_loan,
            &plan,
            &ordinary_profile,
        )
        .expect("ordinary Stable placement admission"),
    )
    .expect("ordinary Stable view establishment");
    let ordinary_projection = ordinary
        .project(field_key(plan.access(), "word"))
        .expect("ordinary Stable projection");
    let ordinary_request = ordinary_projection
        .read()
        .expect("ordinary Stable read")
        .into_primitive_request();
    let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
    let rejection = ordinary_request
        .into_stable_primitive_access()
        .expect("ordinary Stable specialization remains valid")
        .into_corresponded_stable_access()
        .expect_err("provider/device preflight rejects correspondence-free Stable storage");
    assert!(rejection.diagnostic().0.contains("requires admitted"));
    let (ordinary_stable, _) = rejection.into_parts();
    assert_eq!(
        primitive_request_snapshot(ordinary_stable.primitive_request()),
        ordinary_snapshot,
        "rejection returns the exact already-specialized Stable request"
    );
    ordinary_stable
        .validate_for_lowering()
        .expect("returned correspondence-free Stable request remains usable elsewhere");
}

#[test]
fn established_owned_write_specializes_for_stable_primitive_lowering() {
    let (plan, mut established) = established_stable_word(0xa800, 128, 129, 131);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .write()
        .expect("Stable write")
        .into_primitive_request();
    let stable = request
        .into_stable_primitive_access()
        .expect("Stable write specialization");

    assert_eq!(stable.operation(), StablePrimitiveOperation::Write);
    assert_eq!(stable.primitive_address(), 0xa800);
    let request = stable.into_primitive_request();
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 131);
    assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
}

#[test]
fn established_owned_compound_mutation_specializes_with_exact_custody() {
    let (plan, mut established) = established_stable_word(0xad00, 160, 161, 163);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .compound_mutation()
        .expect("authorized Stable compound mutation")
        .into_primitive_request();
    let before = primitive_request_snapshot(&request);
    let compound = request
        .into_stable_compound_mutation_access()
        .expect("Stable compound specialization");

    assert_eq!(compound.primitive_address(), 0xad00);
    assert_eq!(compound.transfer_width_bits(), 32);
    assert_eq!(compound.logical_extent().fragments().len(), 1);
    assert_eq!(compound.effect_footprint().address(), 0xad00);
    assert_eq!(compound.effect_footprint().length_bytes(), 4);
    let request = compound.into_primitive_request();
    assert_eq!(primitive_request_snapshot(&request), before);
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 163);
    assert_eq!(request.effective_supply().key(), request.key);
    assert_eq!(request.effective_supply().width_bits(), 32);
    assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
    assert_eq!(request.operation(), AccessOperation::CompoundMutation);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 161);
    assert_eq!(established.custody_receipt().normalized_identity(), 162);
}

#[test]
fn stable_compound_lowering_replays_authority_without_consuming_retry() {
    let (plan, mut established) = established_stable_word(0xad10, 228, 229, 231);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .compound_mutation()
        .expect("authorized Stable compound mutation")
        .into_primitive_request();
    let mut compound = request
        .into_stable_compound_mutation_access()
        .expect("Stable compound specialization");
    let expected = primitive_request_snapshot(&compound.request);

    compound.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let diagnostic = compound
        .validate_for_lowering()
        .expect_err("outward preflight must reject copied receipt drift");
    assert!(diagnostic.0.contains("retained placement authority"));
    compound.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(91).expect("profile receipt");

    compound.request.operation = AccessOperation::Write;
    let diagnostic = compound
        .validate_for_lowering()
        .expect_err("outward preflight must reject operation drift");
    assert!(diagnostic.0.contains("CompoundMutation"));
    compound.request.operation = AccessOperation::CompoundMutation;

    compound
        .validate_for_lowering()
        .expect("corrected carrier must remain valid for retry");
    assert_eq!(primitive_request_snapshot(&compound.request), expected);
}

#[test]
fn provider_stable_compound_preflight_requires_exact_correspondence() {
    let plan = stable_word_placement();
    let mut extent = uart_extent_with_lineage(0xad18, 4, 282);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan_mut(0, 4).expect("exclusive Stable loan");
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(283).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("Stable placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(284)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(285).expect("stable device");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(286).expect("provider provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");
    let mut word = view
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable word projection");
    let request = word
        .compound_mutation()
        .expect("Stable compound mutation")
        .into_primitive_request();
    let expected = primitive_request_snapshot(&request);
    let compound = request
        .into_stable_compound_mutation_access()
        .expect("Stable compound specialization");

    let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(287)
            .expect("alternate correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(288).expect("alternate stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(289)
            .expect("alternate provider provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("alternate provider correspondence grant")
    .admit(&plan, &profile)
    .expect("alternate schema correspondence admission");
    let mut corresponded = compound
        .into_corresponded_stable_compound_access()
        .expect("provider/device compound preflight requires retained correspondence");
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        primitive_request_snapshot(corresponded.compound_access().primitive_request()),
        expected
    );

    let retained_correspondence =
        corresponded.replace_correspondence_for_test(&alternate_correspondence);
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("a distinct correspondence carrier cannot replace retained authority");
    assert!(
        diagnostic
            .0
            .contains("different schema/device correspondence")
    );
    corresponded.replace_correspondence_for_test(retained_correspondence);

    corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("provider/device compound preflight must replay placement authority");
    assert!(diagnostic.0.contains("copied plan"));
    corresponded.replace_request_plan_for_test(plan.identity());
    corresponded
        .validate_for_provider_lowering()
        .expect("restored exact carrier remains available for retry");
    assert_eq!(
        primitive_request_snapshot(corresponded.into_compound_access().primitive_request()),
        expected
    );

    let mut ordinary_extent = uart_extent_with_lineage(0xad28, 4, 290);
    let ordinary_profile = stable_word_profile(&ordinary_extent);
    let ordinary_loan = ordinary_extent
        .loan_mut(0, 4)
        .expect("ordinary exclusive Stable loan");
    let mut ordinary = place(
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(291).expect("ordinary admission"),
            ordinary_loan,
            &plan,
            &ordinary_profile,
        )
        .expect("ordinary Stable placement admission"),
    )
    .expect("ordinary Stable view establishment");
    let mut ordinary_projection = ordinary
        .project_mut(field_key(plan.access(), "word"))
        .expect("ordinary exclusive Stable projection");
    let ordinary_request = ordinary_projection
        .compound_mutation()
        .expect("ordinary Stable compound mutation")
        .into_primitive_request();
    let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
    let rejection = ordinary_request
        .into_stable_compound_mutation_access()
        .expect("ordinary compound specialization remains valid")
        .into_corresponded_stable_compound_access()
        .expect_err("provider/device preflight rejects correspondence-free Stable storage");
    assert!(rejection.diagnostic().0.contains("requires admitted"));
    let (ordinary_compound, _) = rejection.into_parts();
    assert_eq!(
        primitive_request_snapshot(ordinary_compound.primitive_request()),
        ordinary_snapshot,
        "rejection returns the exact already-specialized compound request"
    );
    ordinary_compound
        .validate_for_lowering()
        .expect("returned correspondence-free compound request remains usable elsewhere");
}

#[test]
fn placed_field_authorization_replays_projection_authority_and_allows_retry() {
    let (plan, established) = established_stable_word(0xad20, 164, 165, 167);
    let mut projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");

    projection.plan.0 ^= 1;
    let diagnostic = projection
        .read()
        .expect_err("authorization must reject copied placement identity drift");
    assert!(diagnostic.0.contains("placed field authorization"));
    assert!(diagnostic.0.contains("retained authority"));
    projection.plan = plan.identity();

    projection.supply.offset = 4;
    let diagnostic = projection
        .read()
        .expect_err("authorization must reject copied supply-row drift");
    assert!(diagnostic.0.contains("replayed resource row"));
    projection.supply.offset = 0;

    projection.primitive_address += 4;
    let diagnostic = projection
        .read()
        .expect_err("authorization must reject copied primitive-address drift");
    assert!(
        diagnostic
            .0
            .contains("reproduce the projected primitive address")
    );
    projection.primitive_address -= 4;

    let request = projection
        .read()
        .expect("repaired projection remains authorizable")
        .into_primitive_request();
    let stable = request
        .into_stable_primitive_access()
        .expect("repaired projection remains valid through specialization");
    assert_eq!(stable.primitive_address(), 0xad20);
    let request = stable.into_primitive_request();
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 167);
}

#[test]
fn stable_primitive_specialization_replays_exact_supply_row_and_returns_custody() {
    let (plan, established) = established_stable_word(0xad40, 168, 169, 171);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.effective_supply.key.slot ^= 1;
    request = expect_exact_stable_primitive_rejection(request, "supply key and width");
    request.effective_supply.key = request.key;

    request.effective_supply.field.push_str("_drift");
    request = expect_exact_stable_primitive_rejection(request, "field identity");
    request.effective_supply.field = request.field.clone();

    request.effective_supply.width_bits = 64;
    request = expect_exact_stable_primitive_rejection(request, "supply key and width");
    request.effective_supply.width_bits = request.transfer_width_bits;

    request.effective_supply.offset = 4;
    request = expect_exact_stable_primitive_rejection(request, "supply offset");
    request.effective_supply.offset = 0;

    request.effective_supply.alignment_bytes = 0;
    request = expect_exact_stable_primitive_rejection(request, "supply alignment");
    request.effective_supply.alignment_bytes = 4;

    request.primitive_address += 4;
    let request = expect_exact_stable_primitive_rejection(request, "supply offset");
    assert_eq!(request.admission().normalized_identity(), 171);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 169);
    assert_eq!(established.custody_receipt().normalized_identity(), 170);
}

#[test]
fn placed_authorization_and_specialization_replay_resident_content_grant() {
    let (plan, established) = established_stable_word(0xad50, 220, 221, 223);

    let (replacement_extent, replacement_content) =
        provider_existing_content(&plan, 0xad50, 4, 224, 225);
    let replacement_profile = stable_word_profile(&replacement_extent);
    let replacement_admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(223).expect("matching admission"),
        replacement_extent,
        &plan,
        &replacement_profile,
    )
    .expect("matching replacement placement");
    let replacement_dormant = adopt_owned_stable(replacement_admission, replacement_content)
        .expect("replacement resident adoption");
    let mut corrupt = replacement_dormant
        .view(PlacedOccurrenceId::from_normalized_identity(10_223).expect("matching occurrence"))
        .expect("replacement resident view");
    let (_unrelated_extent, unrelated_content) =
        provider_existing_content(&plan, 0xad50, 4, 228, 229);
    corrupt.content = unrelated_content;

    let mut projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    projection._authority = PlacementAuthorityRef::EstablishedOwned(&corrupt);
    projection.resident_claim = Some(corrupt.resident_claim());
    projection.placed_occurrence = Some(corrupt.occurrence());
    let diagnostic = projection
        .read()
        .expect_err("authorization must replay resident content beyond copied identities");
    assert!(diagnostic.0.contains("resident content grant"));

    projection._authority = PlacementAuthorityRef::EstablishedOwned(&established);
    projection.resident_claim = Some(established.resident_claim());
    projection.placed_occurrence = Some(established.occurrence());
    let mut request = projection
        .read()
        .expect("repaired projection remains authorizable")
        .into_primitive_request();
    request._authority = PlacementAuthorityRef::EstablishedOwned(&corrupt);
    request.resident_claim = Some(corrupt.resident_claim());
    request.placed_occurrence = Some(corrupt.occurrence());
    request = expect_exact_stable_primitive_rejection(request, "resident content grant");

    request._authority = PlacementAuthorityRef::EstablishedOwned(&established);
    request.resident_claim = Some(established.resident_claim());
    request.placed_occurrence = Some(established.occurrence());
    let stable = request
        .into_stable_primitive_access()
        .expect("repaired resident content authority supports specialization");
    assert_eq!(stable.primitive_address(), 0xad50);
}

#[test]
fn stable_primitive_specialization_replays_descriptor_geometry_and_authorization() {
    let (plan, established) = established_stable_word(0xad60, 172, 173, 175);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.logical_extent.fragments[0].source_bit_offset ^= 1;
    request = expect_exact_stable_primitive_rejection(request, "field descriptor");
    request.logical_extent = request.descriptor.logical_extent.clone();

    request.effect_footprint.address += 4;
    request = expect_exact_stable_primitive_rejection(request, "effect footprint");
    request.effect_footprint.address = request.primitive_address;

    request.effect_footprint.length_bytes = 8;
    request = expect_exact_stable_primitive_rejection(request, "effect footprint");
    request.effect_footprint.length_bytes = request.descriptor.effect_footprint.length_bytes;

    request.operation = AccessOperation::Write;
    let request = expect_exact_stable_primitive_rejection(request, "does not permit Write");
    assert_eq!(request.admission().normalized_identity(), 175);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 173);
    assert_eq!(established.custody_receipt().normalized_identity(), 174);
}

#[test]
fn stable_primitive_specialization_replays_exact_placement_authority() {
    let plan = stable_word_placement();
    let extent = uart_extent_with_lineage(0xad70, 4, 176);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan(0, 4).expect("shared Stable loan");
    let admission_id = PlacementAdmissionId::from_normalized_identity(177).expect("admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed Stable admission");
    let view = place(admission).expect("Stable placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.plan.0 ^= 1;
    request = expect_exact_stable_primitive_rejection(request, "placement authority");
    request.plan = plan.identity();

    request.profile_receipt = ResourceProfileReceiptId::from_normalized_identity(
        request.profile_receipt.normalized_identity() ^ 1,
    )
    .expect("tampered nonzero profile receipt");
    request = expect_exact_stable_primitive_rejection(request, "placement authority");
    request.profile_receipt = profile.receipt();

    request.admission.0 ^= 1;
    request = expect_exact_stable_primitive_rejection(request, "placement authority");
    request.admission = admission_id;

    request.reach = BoundaryReach::from_services([
        BoundaryServiceReachId::from_normalized_identity(178).expect("reach"),
    ]);
    request = expect_exact_stable_primitive_rejection(request, "placement authority");
    request.reach = plan.reach().clone();

    request.source_loan = BorrowPolarity::Exclusive;
    request = expect_exact_stable_primitive_rejection(request, "source-loan");
    request.source_loan = BorrowPolarity::Shared;

    request.resident_claim =
        Some(ResidentClaimId::from_normalized_identity(179).expect("spurious resident claim"));
    request = expect_exact_stable_primitive_rejection(request, "resident identities");
    request.resident_claim = None;

    request.descriptor.field.push_str("_drift");
    request.field.push_str("_drift");
    request.effective_supply.field.push_str("_drift");
    let request = expect_exact_stable_primitive_rejection(request, "resource row");
    assert_eq!(request.admission(), admission_id);
}

#[test]
fn stable_primitive_specialization_rejects_coherent_authorization_rewrite() {
    let plan = stable_word_placement();
    let mut extent = uart_extent_with_lineage(0xad74, 4, 186);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan_mut(0, 4).expect("exclusive Stable loan");
    let admission_id = PlacementAdmissionId::from_normalized_identity(187).expect("admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed Stable admission");
    let view = place(admission).expect("Stable placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "word"))
        .expect("shared projection over exclusive source loan");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.current_borrow = BorrowPolarity::Exclusive;
    request.operation = AccessOperation::Write;
    let request = expect_exact_stable_primitive_rejection(request, "field authorization");
    assert_eq!(request.admission(), admission_id);
    assert_eq!(
        request.authorization.current_borrow(),
        BorrowPolarity::Shared
    );
    assert_eq!(request.authorization.operation(), AccessOperation::Read);
}

#[test]
fn borrowed_view_establishment_replays_profile_and_returns_admission_for_retry() {
    let plan = stable_word_placement();
    let extent = uart_extent_with_lineage(0xad7c, 4, 188);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan(0, 4).expect("shared Stable loan");
    let admission_id = PlacementAdmissionId::from_normalized_identity(189).expect("admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed Stable admission");

    let coincident = uart_extent_with_lineage(0xad7c, 4, 190);
    let wrong_profile = stable_word_profile(&coincident);
    assert_eq!(wrong_profile.receipt(), profile.receipt());
    let PlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: _,
        resources,
        loan,
    } = admission;
    let corrupt = PlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: wrong_profile,
        resources,
        loan,
    };
    let rejection = place(corrupt)
        .expect_err("borrowed view establishment must replay admitted profile root facts");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("could not replay the admitted resource profile"),
        "{}",
        rejection.diagnostic()
    );
    let (returned, _) = rejection.into_parts();
    assert_eq!(returned.identity(), admission_id);
    assert_eq!(returned.profile_receipt(), profile.receipt());
    let PlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: _,
        resources,
        loan,
    } = returned;
    let repaired = PlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile,
        resources,
        loan,
    };
    let mut view = place(repaired).expect("returned admission supports corrected retry");
    let retained_profile = view.profile.clone();
    let coincident = uart_extent_with_lineage(0xad7c, 4, 203);
    view.profile = stable_word_profile(&coincident);
    let diagnostic = view
        .project(field_key(plan.access(), "word"))
        .expect_err("field projection must replay retained placement authority");
    assert!(diagnostic.0.contains("field projection"));
    assert!(diagnostic.0.contains("retained placement authority"));
    assert_eq!(view.admission(), admission_id);
    assert_eq!(view.base(), 0xad7c);
    assert_eq!(view.length(), 4);
    view.profile = retained_profile;
    let projection = view
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();
    let stable = request
        .into_stable_primitive_access()
        .expect("repaired view remains valid through specialization");
    let request = stable.into_primitive_request();
    assert_eq!(request.admission(), admission_id);
    assert_eq!(request.profile_receipt(), profile_receipt);
}

#[test]
fn borrowed_view_retirement_replays_authority_and_returns_exact_loan() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0xad88, 12, 265);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(266).expect("placement admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed placement admission");
    let mut view = place(admission).expect("borrowed view establishment");
    let exact_resources = view.resources.clone();

    view.resources.fields[0].offset ^= 4;
    let rejection = view
        .retire()
        .expect_err("retirement must reject drifted resource compatibility");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("resource compatibility differs")
    );
    let (mut view, _) = rejection.into_parts();
    assert_eq!(view.admission(), admission_id);
    view.resources = exact_resources;

    let loan = view
        .retire()
        .expect("repaired view remains valid for retirement retry");
    assert_eq!(loan.origin(), origin);
    assert_eq!(loan.lineage_root(), lineage);
    assert_eq!(loan.base(), 0xad88);
    assert_eq!(loan.length(), 12);
    assert_eq!(loan.polarity(), LoanPolarity::Shared);
}

#[test]
fn established_primitive_specialization_replays_resident_identities() {
    let (plan, established) = established_stable_word(0xad78, 180, 181, 183);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared established projection");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.resident_claim =
        Some(ResidentClaimId::from_normalized_identity(184).expect("drifting resident claim"));
    request = expect_exact_stable_primitive_rejection(request, "resident identities");
    request.resident_claim = Some(established.resident_claim());

    request.placed_occurrence =
        Some(PlacedOccurrenceId::from_normalized_identity(185).expect("drifting occurrence"));
    let request = expect_exact_stable_primitive_rejection(request, "resident identities");
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 181);
    assert_eq!(established.custody_receipt().normalized_identity(), 182);
}

#[test]
fn stable_compound_specialization_fails_closed_and_returns_exact_request() {
    let (plan, mut established) = established_stable_word(0xad80, 164, 165, 167);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let mut request = projection
        .compound_mutation()
        .expect("authorized Stable compound mutation")
        .into_primitive_request();

    request.observation = ObservationModel::External;
    request = expect_exact_stable_compound_rejection(request, "Stable observation");
    request.observation = ObservationModel::Stable;

    request.effective_supply.kind = EffectiveSupplyKind::External;
    request = expect_exact_stable_compound_rejection(request, "Stable supply");
    request.effective_supply.kind = EffectiveSupplyKind::Stable;

    request.key.slot ^= 1;
    request = expect_exact_stable_compound_rejection(request, "supply key and width");
    request.key = request.effective_supply.key;

    request.effective_supply.width_bits = 64;
    request = expect_exact_stable_compound_rejection(request, "supply key and width");
    request.effective_supply.width_bits = request.transfer_width_bits;

    request.current_borrow = BorrowPolarity::Shared;
    request = expect_exact_stable_compound_rejection(request, "exclusive current and source");
    request.current_borrow = BorrowPolarity::Exclusive;

    request.source_loan = BorrowPolarity::Shared;
    request = expect_exact_stable_compound_rejection(request, "exclusive current and source");
    request.source_loan = BorrowPolarity::Exclusive;

    request.operation = AccessOperation::Write;
    let request = expect_exact_stable_compound_rejection(request, "sealed CompoundMutation event");
    assert_eq!(request.admission().normalized_identity(), 167);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 165);
    assert_eq!(established.custody_receipt().normalized_identity(), 166);
}

#[test]
fn external_primitive_specialization_accepts_each_operation_and_supply_kind() {
    let plan = uart_placement_plan();
    let read_extent = uart_extent_with_lineage(0xae00, 12, 143);
    let read_loan = read_extent.loan(0, 12).expect("shared UART loan");
    let read_admission =
        admit_uart(144, read_loan, &plan, &uart_reach()).expect("External UART admission");
    let read_view = place(read_admission).expect("External read-view establishment");
    let read_projection = read_view
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    let read_request = read_projection
        .read()
        .expect("repeatable External read")
        .into_primitive_request();
    let read = read_request
        .into_external_primitive_access()
        .expect("External read specialization");
    assert_eq!(read.operation(), ExternalPrimitiveOperation::Read);
    assert_eq!(read.primitive_address(), 0xae00);
    assert_eq!(read.transfer_width_bits(), 32);
    assert_eq!(read.effect_footprint().address(), 0xae00);
    assert_eq!(read.effect_footprint().length_bytes(), 4);
    assert_eq!(read.logical_extent().fragments().len(), 1);
    let read_request = read.into_primitive_request();
    assert_eq!(
        read_request.effective_supply().kind(),
        EffectiveSupplyKind::External
    );

    let mut write_extent = uart_extent_with_lineage(0xaf00, 12, 145);
    let write_loan = write_extent.loan_mut(0, 12).expect("exclusive UART loan");
    let stable_resources = stable_uart_resource_profile(&write_loan, &uart_reach());
    let write_admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(146).expect("admission"),
        write_loan,
        &plan,
        &stable_resources,
    )
    .expect("Stable-backed External UART admission");
    let mut write_view = place(write_admission).expect("External write-view establishment");
    let mut write_projection = write_view
        .project_mut(field_key(plan.access(), "transmit"))
        .expect("External transmit projection");
    let write_request = write_projection
        .write()
        .expect("whole External write")
        .into_primitive_request();
    let write = write_request
        .into_external_primitive_access()
        .expect("conservatively Stable-backed External write specialization");
    assert_eq!(write.operation(), ExternalPrimitiveOperation::Write);
    assert_eq!(write.primitive_address(), 0xaf04);
    let write_request = write.into_primitive_request();
    assert_eq!(
        write_request.effective_supply().kind(),
        EffectiveSupplyKind::Stable
    );
    assert_eq!(write_request.observation(), ObservationModel::External);
    assert_eq!(write_request.operation(), AccessOperation::Write);
    assert_eq!(write_request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(write_request.source_loan(), BorrowPolarity::Exclusive);

    let take_plan = destructive_word_placement();
    let mut take_extent = uart_extent_with_lineage(0xb000, 4, 147);
    let take_loan = take_extent.loan_mut(0, 4).expect("exclusive FIFO loan");
    let take_resources = destructive_word_profile(&take_loan);
    let take_admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(148).expect("admission"),
        take_loan,
        &take_plan,
        &take_resources,
    )
    .expect("destructive External admission");
    let mut take_view = place(take_admission).expect("External take-view establishment");
    let mut take_projection = take_view
        .project_mut(field_key(take_plan.access(), "fifo"))
        .expect("destructive External projection");
    let take_request = take_projection
        .take()
        .expect("destructive External read")
        .into_primitive_request();
    let take = take_request
        .into_external_primitive_access()
        .expect("External take specialization");
    assert_eq!(take.operation(), ExternalPrimitiveOperation::Take);
    assert_eq!(take.primitive_address(), 0xb000);
    let take_request = take.into_primitive_request();
    assert_eq!(
        take_request.effective_supply().kind(),
        EffectiveSupplyKind::External
    );
    assert_eq!(take_request.operation(), AccessOperation::Take);
    assert_eq!(take_request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(take_request.source_loan(), BorrowPolarity::Exclusive);
}

#[test]
fn external_primitive_lowering_replays_authority_without_observing_storage() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0xb080, 12, 232);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let admission = admit_uart(233, loan, &plan, &uart_reach()).expect("External UART admission");
    let view = place(admission).expect("External read-view establishment");
    let projection = view
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    let request = projection
        .read()
        .expect("repeatable External read")
        .into_primitive_request();
    let mut external = request
        .into_external_primitive_access()
        .expect("External read specialization");
    let expected = primitive_request_snapshot(&external.request);
    let profile_receipt = external.request.profile_receipt;

    external.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let diagnostic = external
        .validate_for_lowering()
        .expect_err("outward preflight must reject copied receipt drift");
    assert!(diagnostic.0.contains("retained placement authority"));
    external.request.profile_receipt = profile_receipt;

    external.operation = ExternalPrimitiveOperation::Write;
    let diagnostic = external
        .validate_for_lowering()
        .expect_err("outward preflight must reject specialization drift");
    assert!(diagnostic.0.contains("retained specialization"));
    external.operation = ExternalPrimitiveOperation::Read;

    external
        .validate_for_lowering()
        .expect("corrected carrier must remain valid for retry");
    assert_eq!(primitive_request_snapshot(&external.request), expected);
    assert_eq!(external.operation(), ExternalPrimitiveOperation::Read);
}

#[test]
fn external_specialization_rejection_returns_the_exact_sealed_request() {
    let (plan, established) = established_stable_word(0xb100, 149, 150, 152);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("Stable read")
        .into_primitive_request();
    let before = primitive_request_snapshot(&request);

    let rejection = request
        .into_external_primitive_access()
        .expect_err("Stable observation must not enter External lowering");
    assert!(rejection.diagnostic().0.contains("External observation"));
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("External observation"));
    assert_eq!(primitive_request_snapshot(&request), before);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 150);
    assert_eq!(established.custody_receipt().normalized_identity(), 151);
}

#[test]
fn external_specialization_fails_closed_without_losing_corrupt_request_custody() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0xb200, 12, 153);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let admission = admit_uart(154, loan, &plan, &uart_reach()).expect("External UART admission");
    let view = place(admission).expect("External placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    let mut request = projection
        .read()
        .expect("repeatable External read")
        .into_primitive_request();

    request.effective_supply.field.push_str("_drift");
    let field_drift = primitive_request_snapshot(&request);
    let rejection = request
        .into_external_primitive_access()
        .expect_err("drifting supply field must not enter External lowering");
    assert!(rejection.diagnostic().0.contains("field identity"));
    let (mut request, _) = rejection.into_parts();
    assert_eq!(primitive_request_snapshot(&request), field_drift);
    request.effective_supply.field = request.field.clone();

    request.effective_supply.kind = EffectiveSupplyKind::Atomic;
    let atomic_supply = primitive_request_snapshot(&request);
    let rejection = request
        .into_external_primitive_access()
        .expect_err("Atomic supply must not enter External lowering");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("External supply, or conservative Stable supply")
    );
    let (mut request, _) = rejection.into_parts();
    assert_eq!(primitive_request_snapshot(&request), atomic_supply);

    request.effective_supply.kind = EffectiveSupplyKind::Stable;
    request.operation = AccessOperation::Take;
    let stable_take = primitive_request_snapshot(&request);
    let rejection = request
        .into_external_primitive_access()
        .expect_err("Stable supply cannot satisfy a destructive External take");
    assert!(rejection.diagnostic().0.contains("for Read or Write"));
    let (mut request, _) = rejection.into_parts();
    assert_eq!(primitive_request_snapshot(&request), stable_take);

    request.effective_supply.kind = EffectiveSupplyKind::External;
    request.operation = AccessOperation::CompoundMutation;
    let compound = primitive_request_snapshot(&request);
    let rejection = request
        .into_external_primitive_access()
        .expect_err("compound mutation must not enter External lowering");
    assert!(rejection.diagnostic().0.contains("Read, Take, or Write"));
    let (request, _) = rejection.into_parts();
    assert_eq!(primitive_request_snapshot(&request), compound);
}

#[test]
fn atomic_primitive_specialization_retains_all_ten_families_and_orderings() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc000, 4, 156);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let resources = atomic_word_profile(&loan);
    let admission_id = PlacementAdmissionId::from_normalized_identity(157).expect("admission");
    let admission = admit_placement(admission_id, loan, &plan, &resources)
        .expect("all-family Atomic admission");
    let view = place(admission).expect("Atomic placed-view establishment");
    let head = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic head projection");

    let requests = [
        (
            head.atomic_load(MemoryOrdering::Receive)
                .expect("Atomic load")
                .into_primitive_request(),
            AtomicAccessOperation::Load(MemoryOrdering::Receive),
        ),
        (
            head.atomic_store(MemoryOrdering::Publish)
                .expect("Atomic store")
                .into_primitive_request(),
            AtomicAccessOperation::Store(MemoryOrdering::Publish),
        ),
        (
            head.atomic_fetch_add(MemoryOrdering::ReceivePublish)
                .expect("Atomic fetch-add")
                .into_primitive_request(),
            AtomicAccessOperation::FetchAdd(MemoryOrdering::ReceivePublish),
        ),
        (
            head.atomic_fetch_sub(MemoryOrdering::NoOrdering)
                .expect("Atomic fetch-sub")
                .into_primitive_request(),
            AtomicAccessOperation::FetchSub(MemoryOrdering::NoOrdering),
        ),
        (
            head.atomic_fetch_xor(MemoryOrdering::GlobalOrder)
                .expect("Atomic fetch-xor")
                .into_primitive_request(),
            AtomicAccessOperation::FetchXor(MemoryOrdering::GlobalOrder),
        ),
        (
            head.atomic_fetch_or(MemoryOrdering::Receive)
                .expect("Atomic fetch-or")
                .into_primitive_request(),
            AtomicAccessOperation::FetchOr(MemoryOrdering::Receive),
        ),
        (
            head.atomic_fetch_and(MemoryOrdering::Publish)
                .expect("Atomic fetch-and")
                .into_primitive_request(),
            AtomicAccessOperation::FetchAnd(MemoryOrdering::Publish),
        ),
        (
            head.atomic_swap(MemoryOrdering::GlobalOrder)
                .expect("Atomic swap")
                .into_primitive_request(),
            AtomicAccessOperation::Swap(MemoryOrdering::GlobalOrder),
        ),
        (
            head.atomic_compare_exchange(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
                .expect("Atomic compare-exchange")
                .into_primitive_request(),
            AtomicAccessOperation::CompareExchange {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
        ),
        (
            head.atomic_compare_exchange_once(
                MemoryOrdering::ReceivePublish,
                MemoryOrdering::Receive,
            )
            .expect("Atomic single-attempt compare-exchange")
            .into_primitive_request(),
            AtomicAccessOperation::CompareExchangeOnce {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
        ),
    ];
    for (request, operation) in requests {
        assert_atomic_specialization(request, operation, plan.identity(), admission_id);
    }
}

#[test]
fn atomic_primitive_lowering_replays_authority_and_ordering_without_attempt() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc080, 4, 234);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let resources = atomic_word_profile(&loan);
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(235).expect("admission"),
        loan,
        &plan,
        &resources,
    )
    .expect("all-family Atomic admission");
    let view = place(admission).expect("Atomic placed-view establishment");
    let head = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic head projection");
    let request = head
        .atomic_compare_exchange_once(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
        .expect("Atomic single-attempt compare-exchange")
        .into_primitive_request();
    let mut atomic = request
        .into_atomic_primitive_access()
        .expect("Atomic single-attempt compare-exchange specialization");
    let expected = primitive_request_snapshot(&atomic.request);
    let profile_receipt = atomic.request.profile_receipt;

    atomic.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let diagnostic = atomic
        .validate_for_lowering()
        .expect_err("outward preflight must reject copied receipt drift");
    assert!(diagnostic.0.contains("retained placement authority"));
    atomic.request.profile_receipt = profile_receipt;

    atomic.request.operation =
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce {
            success: MemoryOrdering::Receive,
            failure: MemoryOrdering::GlobalOrder,
        });
    let diagnostic = atomic
        .validate_for_lowering()
        .expect_err("outward preflight must reject invalid ordering drift");
    assert!(diagnostic.0.contains("invalid ordering plan"));
    atomic.request.operation =
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        });

    atomic.operation = AtomicAccessOperation::CompareExchange {
        success: MemoryOrdering::ReceivePublish,
        failure: MemoryOrdering::Receive,
    };
    let diagnostic = atomic
        .validate_for_lowering()
        .expect_err("outward preflight must reject specialization drift");
    assert!(diagnostic.0.contains("retained specialization"));
    atomic.operation = AtomicAccessOperation::CompareExchangeOnce {
        success: MemoryOrdering::ReceivePublish,
        failure: MemoryOrdering::Receive,
    };

    atomic
        .validate_for_lowering()
        .expect("corrected carrier must remain valid for retry");
    assert_eq!(primitive_request_snapshot(&atomic.request), expected);
    assert_eq!(
        atomic.operation(),
        AtomicAccessOperation::CompareExchangeOnce {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        }
    );
}

#[test]
fn provider_atomic_preflight_requires_and_retains_exact_correspondence() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc0c0, 4, 262);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let profile = atomic_word_profile(&loan);
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(263).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("Atomic placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(264)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(265).expect("stable device");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(266).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");
    let head = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic head projection");
    let request = head
        .atomic_fetch_add(MemoryOrdering::ReceivePublish)
        .expect("Atomic fetch-add")
        .into_primitive_request();
    let expected = primitive_request_snapshot(&request);
    let atomic = request
        .into_atomic_primitive_access()
        .expect("Atomic fetch-add specialization");

    let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(267)
            .expect("alternate correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(268).expect("alternate stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(269)
            .expect("alternate datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("alternate provider correspondence grant")
    .admit(&plan, &profile)
    .expect("alternate schema correspondence admission");
    let mut corresponded = atomic
        .into_corresponded_atomic_access()
        .expect("provider/device Atomic preflight requires retained correspondence");
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        corresponded.atomic_access().operation(),
        AtomicAccessOperation::FetchAdd(MemoryOrdering::ReceivePublish)
    );
    assert_eq!(
        primitive_request_snapshot(corresponded.atomic_access().primitive_request()),
        expected
    );

    let retained_correspondence =
        corresponded.replace_correspondence_for_test(&alternate_correspondence);
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("a distinct correspondence carrier cannot replace retained authority");
    assert!(
        diagnostic
            .0
            .contains("different schema/device correspondence")
    );
    corresponded.replace_correspondence_for_test(retained_correspondence);

    corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("provider/device Atomic preflight must replay placement authority");
    assert!(diagnostic.0.contains("copied plan"));
    corresponded.replace_request_plan_for_test(plan.identity());
    corresponded
        .validate_for_provider_lowering()
        .expect("restored exact carrier remains available for retry");
    assert_eq!(
        primitive_request_snapshot(corresponded.into_atomic_access().primitive_request()),
        expected
    );

    let ordinary_extent = uart_extent_with_lineage(0xc0d0, 4, 270);
    let ordinary_loan = ordinary_extent
        .loan(0, 4)
        .expect("ordinary shared Atomic loan");
    let ordinary_profile = atomic_word_profile(&ordinary_loan);
    let ordinary = place(
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(271).expect("ordinary admission"),
            ordinary_loan,
            &plan,
            &ordinary_profile,
        )
        .expect("ordinary Atomic placement admission"),
    )
    .expect("ordinary Atomic view establishment");
    let ordinary_projection = ordinary
        .project(field_key(plan.access(), "head"))
        .expect("ordinary Atomic projection");
    let ordinary_request = ordinary_projection
        .atomic_load(MemoryOrdering::Receive)
        .expect("ordinary Atomic load")
        .into_primitive_request();
    let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
    let rejection = ordinary_request
        .into_atomic_primitive_access()
        .expect("ordinary Atomic specialization remains valid")
        .into_corresponded_atomic_access()
        .expect_err("provider/device preflight rejects correspondence-free atomic storage");
    assert!(rejection.diagnostic().0.contains("requires admitted"));
    let (ordinary_atomic, _) = rejection.into_parts();
    assert_eq!(
        primitive_request_snapshot(ordinary_atomic.primitive_request()),
        ordinary_snapshot,
        "rejection returns the exact already-specialized Atomic request"
    );
    ordinary_atomic
        .validate_for_lowering()
        .expect("returned correspondence-free Atomic request remains usable elsewhere");
}

#[test]
fn atomic_specialization_fails_closed_and_returns_exact_request() {
    let plan = atomic_word_placement();
    let extent = uart_extent_with_lineage(0xc100, 4, 158);
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let resources = atomic_word_profile(&loan);
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(159).expect("admission"),
        loan,
        &plan,
        &resources,
    )
    .expect("all-family Atomic admission");
    let view = place(admission).expect("Atomic placed-view establishment");
    let head = view
        .project(field_key(plan.access(), "head"))
        .expect("Atomic head projection");
    let mut request = head
        .atomic_load(MemoryOrdering::NoOrdering)
        .expect("Atomic load")
        .into_primitive_request();

    request.observation = ObservationModel::Stable;
    request = expect_exact_atomic_rejection(request, "Atomic observation");
    request.observation = ObservationModel::Atomic;

    request.effective_supply.kind = EffectiveSupplyKind::External;
    request = expect_exact_atomic_rejection(request, "Atomic supply");
    request.effective_supply.kind = EffectiveSupplyKind::Atomic;

    request.key.slot ^= 1;
    request = expect_exact_atomic_rejection(request, "supply key and width");
    request.key = request.effective_supply.key;

    request.effective_supply.width_bits = 64;
    request = expect_exact_atomic_rejection(request, "supply key and width");
    request.effective_supply.width_bits = request.transfer_width_bits;

    request.operation = AccessOperation::Read;
    request = expect_exact_atomic_rejection(request, "sealed Atomic operation");

    request.operation =
        AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Publish));
    request = expect_exact_atomic_rejection(request, "invalid ordering plan");
    request.operation =
        AccessOperation::Atomic(AtomicAccessOperation::Store(MemoryOrdering::Receive));
    request = expect_exact_atomic_rejection(request, "invalid ordering plan");
    request.operation = AccessOperation::Atomic(AtomicAccessOperation::CompareExchange {
        success: MemoryOrdering::Receive,
        failure: MemoryOrdering::GlobalOrder,
    });
    request = expect_exact_atomic_rejection(request, "invalid ordering plan");
    request.operation = AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce {
        success: MemoryOrdering::Receive,
        failure: MemoryOrdering::GlobalOrder,
    });
    let request = expect_exact_atomic_rejection(request, "invalid ordering plan");
    assert_eq!(request.admission().normalized_identity(), 159);
}

#[test]
fn external_request_rejects_stable_specialization_and_returns_exact_request() {
    let plan = uart_placement_plan();
    let extent = uart_extent(0xb000, 12);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let admission = admit_uart(132, loan, &plan, &uart_reach()).expect("admitted shared UART view");
    let view = place(admission).expect("shared UART placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    let request = projection
        .read()
        .expect("External status read")
        .into_primitive_request();

    let rejection = request
        .into_stable_primitive_access()
        .expect_err("External observation must not enter Stable lowering");
    assert!(rejection.diagnostic().0.contains("Stable observation"));
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("Stable observation"));
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 132);
    assert_eq!(request.primitive_address(), 0xb000);
    assert_eq!(request.observation(), ObservationModel::External);
    assert_eq!(request.operation(), AccessOperation::Read);
    assert_eq!(request.source_loan(), BorrowPolarity::Shared);
}

#[test]
fn compound_request_rejects_stable_specialization_and_returns_custody() {
    let (plan, mut established) = established_stable_word(0xb100, 133, 134, 136);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .compound_mutation()
        .expect("authorized Stable compound mutation")
        .into_primitive_request();

    let rejection = request
        .into_stable_primitive_access()
        .expect_err("compound mutation needs its distinct bounded lowering");
    assert!(rejection.diagnostic().0.contains("Read or Write"));
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("Read or Write"));
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 136);
    assert_eq!(request.operation(), AccessOperation::CompoundMutation);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 134);
    assert_eq!(established.custody_receipt().normalized_identity(), 135);
}

#[test]
fn provider_existing_content_cannot_replay_across_extent_roots() {
    let plan = stable_word_placement();
    let (_source_extent, content) = provider_existing_content(&plan, 0xa100, 4, 96, 97);
    let coincident = uart_extent_with_lineage(0xa100, 4, 98);
    let returned_origin = coincident.origin();
    let returned_lineage = coincident.lineage_root();
    let profile = stable_word_profile(&coincident);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(99).expect("admission"),
        coincident,
        &plan,
        &profile,
    )
    .expect("coincident root admission");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("existing-content authority must not replay across roots");
    assert!(rejection.diagnostic().0.contains("lineage"));
    let (admission, content, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("lineage"));
    assert_eq!(content.lineage_root().normalized_identity(), 96);
    assert_eq!(content.resident_claim().normalized_identity(), 99);
    let returned = admission.withdraw();
    assert_eq!(returned.origin(), returned_origin);
    assert_eq!(returned.lineage_root(), returned_lineage);
}

#[test]
fn provider_existing_content_cannot_replay_after_mapping_era_drift() {
    let plan = stable_word_placement();
    let (_source_extent, content) = provider_existing_content(&plan, 0xa180, 4, 108, 109);
    let drifted = uart_root_grant_with_mapping(1, 108, 5, 110)
        .mint(0xa180, 4)
        .expect("same-root geometry in a later mapping era");
    assert_eq!(content.origin(), drifted.origin());
    assert_eq!(content.lineage_root(), drifted.lineage_root());
    assert_eq!(
        (content.base(), content.length()),
        (drifted.base(), drifted.length())
    );
    assert_eq!(content.address_space(), drifted.address_space());
    assert_eq!(content.provenance(), drifted.provenance());
    assert_ne!(content.era(), drifted.era());
    let profile = stable_word_profile(&drifted);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(111).expect("admission"),
        drifted,
        &plan,
        &profile,
    )
    .expect("drifted mapping admission");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("existing-content authority must not replay after mapping-era drift");
    assert!(rejection.diagnostic().0.contains("mapping era"));
    let (admission, content, _) = rejection.into_parts();
    assert_eq!(content.era().normalized_identity(), 6);
    assert_eq!(admission.withdraw().era().normalized_identity(), 110);
}

#[test]
fn provider_existing_content_must_name_the_actual_placement() {
    let plan = stable_word_placement();
    let (extent, content) = uart_root_grant(1, 100)
        .mint_provider_existing_content(
            0xa200,
            4,
            extent_id(
                plan.identity().normalized_identity() + 1,
                psi_extents::ExtentContentInterpretationId::from_normalized_identity,
            ),
            extent_id(104, ResidentClaimId::from_normalized_identity),
            extent_id(
                101,
                ExtentContentValidityReceiptId::from_normalized_identity,
            ),
            extent_id(102, ExtentContentCustodyReceiptId::from_normalized_identity),
        )
        .expect("provider existing-content extent");
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(103).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("provider interpretation must match the actual admitted placement");
    assert!(rejection.diagnostic().0.contains("interpretation"));
}

#[test]
fn provider_content_does_not_turn_external_placement_into_stable_adoption() {
    let plan = uart_placement_plan();
    let (extent, content) = provider_existing_content(&plan, 0xa300, 12, 104, 105);
    let profile = uart_resource_profile_for_extent(&extent, &uart_reach());
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(107).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned External admission");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("External observation needs its distinct adopt route");
    assert!(
        rejection.diagnostic().0.contains("External")
            && rejection.diagnostic().0.contains("Stable adoption")
    );
}

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
        schema_identity: 95,
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
                stable: StableCapability::ReadWrite,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            },
            ResourceRegion {
                offset: 4,
                length: 4,
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
    let alternate = validate_placement_plan(PlacementPlan {
        access: uart_access_source(&alternate_layout),
        layout: alternate_layout,
        reach: uart_reach(),
    })
    .expect("fresh plan over non-overlapping alternate geometry");
    assert_ne!(plan.access().identity(), alternate.access().identity());
    assert_ne!(plan.identity(), alternate.identity());
    assert_ne!(
        plan.access().layout_fingerprint(),
        alternate.access().layout_fingerprint(),
        "layout geometry is part of access-policy identity"
    );
}

#[test]
fn resource_profiles_normalize_disjoint_regions_and_restrict_subranges() {
    let alternate_reach =
        BoundaryServiceReachId::from_normalized_identity(8).expect("alternate reach");
    let broad_reach = BoundaryReach::from_services([reach(), alternate_reach]);
    let stable = ResourceRegion {
        offset: 0,
        length: 4,
        stable: StableCapability::ReadWrite,
        external: ExternalCapability::None,
        atomic: AtomicCapability::None,
        reach: broad_reach.clone(),
    };
    let profile = validate_resource_profile(
        ResourceProfile {
            regions: vec![
                ResourceRegion {
                    offset: 4,
                    ..stable.clone()
                },
                stable.clone(),
                ResourceRegion {
                    offset: 8,
                    length: 8,
                    stable: StableCapability::None,
                    external: ExternalCapability::Access {
                        read: ExternalReadBehavior::Repeatable,
                        write: false,
                        transfers: vec![TransferRule {
                            width_bits: 32,
                            alignment_bytes: 4,
                        }],
                    },
                    atomic: AtomicCapability::None,
                    reach: broad_reach,
                },
            ],
        },
        16,
    )
    .expect("disjoint profile");
    assert_eq!(
        profile.regions().len(),
        2,
        "adjacent identical regions normalize into one interval"
    );
    assert_eq!(profile.regions()[0].offset, 0);
    assert_eq!(profile.regions()[0].length, 8);

    let child = profile
        .restrict(4, 8, &uart_reach())
        .expect("subrange restriction");
    assert_eq!(child.length(), 8);
    assert_eq!(child.regions().len(), 2);
    assert_eq!(
        (child.regions()[0].offset, child.regions()[0].length),
        (0, 4)
    );
    assert_eq!(
        (child.regions()[1].offset, child.regions()[1].length),
        (4, 4)
    );
    assert!(
        child
            .regions()
            .iter()
            .all(|region| { region.reach.services().len() == 1 && region.reach.contains(reach()) })
    );

    let overlap = validate_resource_profile(
        ResourceProfile {
            regions: vec![
                stable,
                ResourceRegion {
                    offset: 2,
                    length: 4,
                    stable: StableCapability::Read,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::None,
                    reach: BoundaryReach::default(),
                },
            ],
        },
        8,
    )
    .expect_err("overlapping resource regions must reject");
    assert!(overlap.0.contains("overlap"));
}

#[test]
fn resource_compatibility_joins_observation_operations_width_and_reach() {
    let plan = uart_placement_plan();
    let stable_profile = validate_resource_profile(
        ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 12,
                stable: StableCapability::ReadWrite,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: uart_reach(),
            }],
        },
        12,
    )
    .expect("stable profile");
    let compatibility = validate_placement_resources(&plan, &stable_profile)
        .expect("stable supply may conservatively satisfy external demand");
    assert!(
        compatibility
            .fields()
            .iter()
            .all(|field| field.kind() == EffectiveSupplyKind::Stable)
    );
    assert_eq!(compatibility.base_congruence().modulus(), 4);
    assert_eq!(compatibility.base_congruence().residue(), 0);

    let read_only_external = validate_resource_profile(
        ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 12,
                stable: StableCapability::None,
                external: ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: false,
                    transfers: vec![TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    }],
                },
                atomic: AtomicCapability::None,
                reach: uart_reach(),
            }],
        },
        12,
    )
    .expect("read-only external profile");
    let error = validate_placement_resources(&plan, &read_only_external)
        .expect_err("read-only external supply cannot satisfy UART writes");
    assert!(
        error.0.contains("transmit") && error.0.contains("incompatible External"),
        "canonical field order reports the first unsupported UART write: {error}"
    );

    let wrong_width = validate_resource_profile(
        ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 12,
                stable: StableCapability::None,
                external: ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: true,
                    transfers: vec![TransferRule {
                        width_bits: 64,
                        alignment_bytes: 8,
                    }],
                },
                atomic: AtomicCapability::None,
                reach: uart_reach(),
            }],
        },
        12,
    )
    .expect("wrong-width profile remains structurally valid");
    let error = validate_placement_resources(&plan, &wrong_width)
        .expect_err("transfer width must match exactly");
    assert!(
        error.0.contains("control") && error.0.contains("32-bit"),
        "canonical field order reports the first width mismatch: {error}"
    );

    let stable_demand = validate_placement_plan(PlacementPlan {
        layout: uart_layout(),
        access: access_plan(
            &uart_layout(),
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
        reach: uart_reach(),
    })
    .expect("stable demand");
    let error = validate_placement_resources(&stable_demand, &read_only_external)
        .expect_err("external supply cannot satisfy Stable demand");
    assert!(error.0.contains("requests Stable"));

    let destructive_demand = validate_placement_plan(PlacementPlan {
        layout: uart_layout(),
        access: access_plan(
            &uart_layout(),
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
        reach: uart_reach(),
    })
    .expect("destructive external demand");
    let error = validate_placement_resources(&destructive_demand, &read_only_external)
        .expect_err("repeatable reads cannot satisfy destructive observation");
    assert!(
        error.0.contains("status") && error.0.contains("Take"),
        "observation mismatch must name the destructive demand: {error}"
    );

    let atomic_layout = LayoutPlanReport {
        schema_identity: 93,
        entries: vec![LayoutFieldEntryReport {
            field: "head".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 4,
    };
    let atomic_demand = validate_placement_plan(PlacementPlan {
        access: access_plan(
            &atomic_layout,
            &[(
                "head",
                FieldAccess::Atomic {
                    transfer_width_bits: 32,
                    operations: AtomicPermissions {
                        load: true,
                        fetch_add: true,
                        ..AtomicPermissions::default()
                    },
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        layout: atomic_layout,
        reach: BoundaryReach::default(),
    })
    .expect("atomic demand");
    let load_only = validate_resource_profile(
        ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 4,
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
                            ..AtomicPermissions::default()
                        },
                    }],
                },
                reach: BoundaryReach::default(),
            }],
        },
        4,
    )
    .expect("load-only atomic profile");
    let error = validate_placement_resources(&atomic_demand, &load_only)
        .expect_err("atomic operation demand must be an exact supply subset");
    assert!(
        error.0.contains("head") && error.0.contains("operation families"),
        "atomic mismatch must name the field and operation family: {error}"
    );
}

#[test]
fn subrange_loan_rebases_profile_and_preserves_denied_bytes() {
    let layout = LayoutPlanReport {
        schema_identity: 94,
        entries: vec![LayoutFieldEntryReport {
            field: "word".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 4,
    };
    let placement = validate_placement_plan(PlacementPlan {
        access: access_plan(
            &layout,
            &[(
                "word",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Read,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        layout,
        reach: BoundaryReach::default(),
    })
    .expect("subrange placement");
    let extent = uart_extent(0x4000, 16);
    let profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(41).expect("profile receipt"),
        &extent,
        extent_rights(&[3]),
        BoundaryReach::default(),
    )
    .expect("profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 4,
            length: 4,
            stable: StableCapability::None,
            external: ExternalCapability::Access {
                read: ExternalReadBehavior::Repeatable,
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
    .expect("sparse admitted profile");

    {
        let loan = extent.loan(4, 4).expect("covered subrange loan");
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(42).expect("admission"),
            loan,
            &placement,
            &profile,
        )
        .expect("resource region must rebase to the subrange loan");
        assert_eq!(admission.resources().fields()[0].offset(), 0);
        let view = place(admission).expect("split placed-view establishment");
        assert_eq!(view.base(), 0x4004);
    }

    let loan = extent.loan(0, 4).expect("uncovered subrange loan");
    let rejection = admit_placement(
        PlacementAdmissionId::from_normalized_identity(43).expect("admission"),
        loan,
        &placement,
        &profile,
    )
    .expect_err("profile restriction must not fill uncovered parent bytes");
    assert!(
        rejection.diagnostic().0.contains("not covered"),
        "uncovered subrange rejection must report missing supply"
    );
    drop(rejection);

    let partition = extent
        .partition_owned(4, 4)
        .expect("owned subrange partition");
    {
        let loan = partition
            .selected()
            .loan(0, 4)
            .expect("selected split loan");
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(44).expect("admission"),
            loan,
            &placement,
            &profile,
        )
        .expect("a conserved split must retain its root profile binding");
        let view = place(admission).expect("split placed-view establishment");
        assert_eq!(view.base(), 0x4004);
    }
    let restored = partition.rejoin();
    assert_eq!(restored.base(), 0x4000);
    assert_eq!(restored.length(), 16);
}

#[test]
fn admitted_profile_rejects_coincident_independent_extent_root() {
    let plan = uart_placement_plan();
    let admitted_root = uart_extent_with_lineage(0x6000, 12, 61);
    let profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(62).expect("profile receipt"),
        &admitted_root,
        extent_rights(&[3]),
        uart_reach(),
    )
    .expect("root-bound profile grant")
    .admit(uart_resource_profile_data(12, &uart_reach()))
    .expect("admitted root-bound profile");

    let foreign_origin = uart_extent_with_root(0x6000, 12, 2, 61);
    assert_ne!(foreign_origin.origin(), admitted_root.origin());
    assert_eq!(foreign_origin.lineage_root(), admitted_root.lineage_root());
    let loan = foreign_origin
        .loan(0, 12)
        .expect("coincident foreign-origin loan");
    let rejection = admit_placement(
        PlacementAdmissionId::from_normalized_identity(63).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect_err("coincident geometry and lineage must not replay another origin's profile");
    assert!(
        rejection.diagnostic().0.contains("sealed root origin"),
        "cross-origin replay must identify the sealed-origin mismatch"
    );

    let coincident_root = uart_extent_with_lineage(0x6000, 12, 63);
    assert_eq!(coincident_root.origin(), admitted_root.origin());
    assert_ne!(coincident_root.lineage_root(), admitted_root.lineage_root());
    let loan = coincident_root
        .loan(0, 12)
        .expect("coincident independent loan");
    let rejection = admit_placement(
        PlacementAdmissionId::from_normalized_identity(64).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect_err("coincident geometry and provenance must not replay another root's profile");
    assert!(
        rejection.diagnostic().0.contains("root lineage"),
        "cross-root replay must identify the root-lineage mismatch"
    );
}

#[test]
fn transfer_alignment_derives_build_time_and_runtime_base_checks() {
    let conflicting_layout = LayoutPlanReport {
        schema_identity: 91,
        entries: vec![
            LayoutFieldEntryReport {
                field: "left".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "right".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 2 },
            },
        ],
        offsets: Some(vec![0, 2]),
        size: Some(8),
        align: 2,
    };
    let conflicting = validate_placement_plan(PlacementPlan {
        access: access_plan(
            &conflicting_layout,
            &[
                (
                    "left",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                ),
                (
                    "right",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                ),
            ],
        ),
        layout: conflicting_layout,
        reach: BoundaryReach::default(),
    })
    .expect("relative geometry is structurally valid");
    let profile = validate_resource_profile(
        ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 8,
                stable: StableCapability::None,
                external: ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: false,
                    transfers: vec![TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    }],
                },
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        },
        8,
    )
    .expect("alignment profile");
    let error = validate_placement_resources(&conflicting, &profile)
        .expect_err("inconsistent field congruences must reject before admission");
    assert!(
        error.0.contains("right") && error.0.contains("offset 2") && error.0.contains("conflicts")
    );

    let layout = LayoutPlanReport {
        schema_identity: 92,
        entries: vec![LayoutFieldEntryReport {
            field: "word".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(4),
        align: 1,
    };
    let placement = validate_placement_plan(PlacementPlan {
        access: access_plan(
            &layout,
            &[(
                "word",
                FieldAccess::External {
                    transfer_width_bits: 32,
                    read: ExternalRead::Read,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )],
        ),
        layout,
        reach: BoundaryReach::default(),
    })
    .expect("single-field placement");
    let extent = uart_extent(0x1002, 4);
    let loan = extent.loan(0, 4).expect("misaligned loan");
    let resources = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(22).expect("profile receipt"),
        &extent,
        extent_rights(&[3]),
        BoundaryReach::default(),
    )
    .expect("profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 4,
            stable: StableCapability::None,
            external: ExternalCapability::Access {
                read: ExternalReadBehavior::Repeatable,
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
    .expect("admitted profile");
    let rejection = admit_placement(
        PlacementAdmissionId::from_normalized_identity(23).expect("admission"),
        loan,
        &placement,
        &resources,
    )
    .expect_err("actual base must discharge the derived congruence");
    assert!(rejection.diagnostic().0.contains("base mod 4 must equal 0"));
}

#[test]
fn admitted_profile_binds_rights_provenance_era_and_returns_rejected_loan() {
    let plan = uart_placement_plan();
    let extent = uart_extent(0x3000, 12);
    let profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(31).expect("profile receipt"),
        &extent,
        extent_rights(&[4]),
        uart_reach(),
    )
    .expect("profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 12,
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
            reach: uart_reach(),
        }],
    })
    .expect("admitted profile");
    let extent = extent
        .attenuate(extent_rights(&[3]))
        .expect("attenuated extent");
    let loan = extent.loan(0, 12).expect("UART loan");
    let rejection = admit_placement(
        PlacementAdmissionId::from_normalized_identity(32).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect_err("attenuated loan cannot recover profile-bound rights");
    assert!(rejection.diagnostic().0.contains("lacks rights"));
    let (returned, _) = rejection.into_parts();
    assert_eq!(returned.base(), 0x3000);
    assert_eq!(returned.length(), 12);
}

#[test]
fn effect_conflicts_use_whole_transfer_containers() {
    let word = EffectFootprint {
        address: 0x1000,
        length_bytes: 4,
    };
    let overlapping_half = EffectFootprint {
        address: 0x1002,
        length_bytes: 2,
    };
    let next_word = EffectFootprint {
        address: 0x1004,
        length_bytes: 4,
    };

    assert!(!effect_footprints_conflict(
        word,
        AccessOperation::Read,
        word,
        AccessOperation::Read,
    ));
    assert!(effect_footprints_conflict(
        word,
        AccessOperation::Read,
        word,
        AccessOperation::Take,
    ));
    assert!(effect_footprints_conflict(
        word,
        AccessOperation::CompoundMutation,
        word,
        AccessOperation::Read,
    ));

    let atomic_load = AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Receive));
    let atomic_store =
        AccessOperation::Atomic(AtomicAccessOperation::Store(MemoryOrdering::Publish));
    assert!(!effect_footprints_conflict(
        word,
        atomic_load,
        word,
        atomic_store,
    ));
    assert!(effect_footprints_conflict(
        word,
        atomic_load,
        overlapping_half,
        atomic_store,
    ));
    assert!(!effect_footprints_conflict(
        word,
        AccessOperation::Write,
        next_word,
        AccessOperation::Write,
    ));
}
