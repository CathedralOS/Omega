use super::{
    access_plan, extent_rights, reach, uart_extent, uart_extent_with_lineage,
    uart_extent_with_root, uart_layout, uart_placement_plan, uart_reach,
    uart_resource_profile_data,
};
use crate::{
    AccessExposure, AccessOperation, AtomicAccessOperation, AtomicCapability, AtomicPermissions,
    AtomicTransferRule, BoundaryReach, BoundaryServiceReachId, EffectFootprint,
    EffectiveSupplyKind, ExternalCapability, ExternalRead, ExternalReadBehavior, FieldAccess,
    PeerWritability, PlacementAdmissionId, PlacementPlan, ResourceProfile, ResourceProfileGrant,
    ResourceProfileReceiptId, ResourceRegion, StableCapability, TransferRule, admit_placement,
    effect_footprints_conflict, place, validate_placement_plan, validate_placement_resources,
    validate_resource_profile,
};
use language_core::atomic::MemoryOrdering;
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};

#[test]
fn resource_profiles_normalize_disjoint_regions_and_restrict_subranges() {
    let alternate_reach =
        BoundaryServiceReachId::from_normalized_identity(8).expect("alternate reach");
    let broad_reach = BoundaryReach::from_services([reach(), alternate_reach]);
    let stable = ResourceRegion {
        offset: 0,
        length: 4,
        peer: PeerWritability::Exclusive,
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
                    peer: PeerWritability::Exclusive,
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
                    peer: PeerWritability::Exclusive,
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
                peer: PeerWritability::Exclusive,
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
                peer: PeerWritability::Exclusive,
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
                peer: PeerWritability::Exclusive,
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
        schema_report_fingerprint: 93,
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
        schema_report_fingerprint: 94,
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
            peer: PeerWritability::Exclusive,
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
        schema_report_fingerprint: 91,
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
                peer: PeerWritability::Exclusive,
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
        schema_report_fingerprint: 92,
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
            peer: PeerWritability::Exclusive,
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

/// A region a hostile writable peer can still rewrite cannot honestly
/// claim `Stable` supply: zero-copy stable placement would read bytes the
/// peer may change after validation. `External` and `Atomic` supply stay
/// honest — every authorized access is one exact-width event under
/// mutation — and adjacent regions never merge across a peer boundary.
#[test]
fn hostile_shared_regions_never_supply_stable() {
    let hostile_stable = validate_resource_profile(
        ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 8,
                peer: PeerWritability::HostileShared,
                stable: StableCapability::Read,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        },
        8,
    )
    .expect_err("stable supply over hostile-shared memory is incoherent");
    assert!(
        hostile_stable.0.contains("hostile-shared"),
        "the rejection names the violated rule: {hostile_stable}"
    );

    let transfers = vec![TransferRule {
        width_bits: 32,
        alignment_bytes: 4,
    }];
    let hostile_external = validate_resource_profile(
        ResourceProfile {
            regions: vec![
                ResourceRegion {
                    offset: 0,
                    length: 4,
                    peer: PeerWritability::HostileShared,
                    stable: StableCapability::None,
                    external: ExternalCapability::Access {
                        read: ExternalReadBehavior::Repeatable,
                        write: true,
                        transfers: transfers.clone(),
                    },
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                },
                ResourceRegion {
                    offset: 4,
                    length: 4,
                    peer: PeerWritability::Exclusive,
                    stable: StableCapability::Read,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                },
            ],
        },
        8,
    )
    .expect("hostile external supply is coherent beside exclusive stable");
    assert_eq!(
        hostile_external.regions().len(),
        2,
        "adjacent regions never merge across a peer boundary"
    );

    // Restriction retains the peer claim, and the claim participates in the
    // normalized profile identity.
    let child = hostile_external
        .restrict(0, 4, &uart_reach())
        .expect("restrict retains the hostile peer claim");
    assert_eq!(child.regions()[0].peer, PeerWritability::HostileShared);

    let profile_with = |peer: PeerWritability| {
        validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 4,
                    peer,
                    stable: StableCapability::None,
                    external: ExternalCapability::Access {
                        read: ExternalReadBehavior::Repeatable,
                        write: true,
                        transfers: transfers.clone(),
                    },
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                }],
            },
            4,
        )
        .expect("peer claim alone cannot invalidate a profile")
    };
    assert_ne!(
        profile_with(PeerWritability::Exclusive).identity(),
        profile_with(PeerWritability::HostileShared).identity(),
        "the peer claim participates in normalized identity"
    );
}
