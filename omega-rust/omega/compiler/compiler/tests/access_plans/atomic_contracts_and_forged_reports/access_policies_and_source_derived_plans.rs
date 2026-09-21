use crate::{POLICY_SOURCE, extent_identity, provider_issuance, write_program};
use access_plans::{
    AtomicCapability, AtomicPermissions, AtomicTransferRule, BoundaryReach, EffectiveSupplyKind,
    ExternalCapability, ExternalRead, ExternalReadBehavior, FieldAccess, PeerWritability,
    PlacementAdmissionId, ResourceProfile, ResourceProfileGrant, ResourceProfileReceiptId,
    ResourceRegion, SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId,
    SchemaDeviceCorrespondenceGrant, StableCapability, StableDeviceInstanceId, TransferRule,
    admit_owned_placement, admit_placement, adopt_owned_stable,
    bind_schema_correspondence_to_placement,
};
use build_time_evaluation::{compute_access_plan, compute_layout_plan};
use compiler::{CheckedCompileRequest, compile_to_checked};
use extents::{
    AddressSpaceId, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLineageId,
    ExtentProvenanceId, ExtentRightId, ExtentRights, ExtentRootGrant, MappingEraId,
    ResidentClaimId,
};

#[test]
fn source_access_policy_requires_one_decision_per_schema_field() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
data Missing {
    entries: [AccessFieldEntry; 32];
}
machine Missing::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(&schema);
    transition { _ -> truncate(plan) }
    state truncate(plan: AccessPlan) -> AccessPlan {
        let mut partial: AccessPlan = plan;
        partial.field_count = 1;
        partial
    }
}
data Main {}
"#,
    );
    let main = write_program("missing-access-slot", &source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("invalid policy source should compile");
    let layout = compute_layout_plan(&checked.typed, "UartLayout::plan", "Registers", None)
        .expect("layout should validate");
    let error = compute_access_plan(&checked.typed, "Missing::plan", "Registers", &layout)
        .expect_err("a partial source access plan must reject");
    assert!(
        error.contains("requires exactly 5 decisions"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn aggregate_fields_admit_only_inaccessible_access_decisions() {
    let main = write_program(
        "aggregate-access",
        r#"
use omega::language::core::layout;

data Samples { values: [u16; 3]; }
data ArrayLayout { entries: [FieldEntry; 64]; }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    Plan { entries: owned_entries, entry_count: 1,
           size_fixed: 6, size_is_dynamic: false, align: 2 }
}

data ArrayAccess {}
machine ArrayAccess::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(&schema);
    plan.with(
        schema.fields[0].key,
        FieldAccess::Stable {
            read: true,
            write: false,
            exposure: Exposure::BindingPrivate,
        },
    )
}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("access policy source should type");
    let layout = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples", None)
        .expect("the aggregate At layout should validate");
    let error = compute_access_plan(&checked.typed, "ArrayAccess::plan", "Samples", &layout)
        .expect_err("aggregate Stable access must remain outside this layout slice");
    assert!(error.contains("admits only Inaccessible for aggregate fields"));
}

#[test]
fn inaccessible_seed_rejects_foreign_replacement_keys() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
data Forged {}
machine Forged::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(&schema);
    transition { _ -> replace(plan) }
    state replace(plan: AccessPlan) -> AccessPlan {
        plan.with(
            999,
            FieldAccess::Stable {
                read: true,
                write: false,
                exposure: Exposure::Exported
            }
        )
    }
}
data Main {}
"#,
    );
    let main = write_program("forged-access-key", &source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("forged policy source should compile");
    let layout = compute_layout_plan(&checked.typed, "UartLayout::plan", "Registers", None)
        .expect("layout should validate");
    let error = compute_access_plan(&checked.typed, "Forged::plan", "Registers", &layout)
        .expect_err("foreign schema keys must not be ignored by the seed replacement helper");
    assert!(
        error.contains("access field_count is 6") && error.contains("requires exactly 5 decisions"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn access_evaluation_rejects_a_forged_layout_report() {
    let main = write_program("forged-layout-report", POLICY_SOURCE);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("source policy should compile");
    let mut layout = compute_layout_plan(&checked.typed, "UartLayout::plan", "Registers", None)
        .expect("layout should validate");
    layout.offsets = Some(vec![0, 4, 6, 8, 17]);

    let error = compute_access_plan(&checked.typed, "UartAccess::plan", "Registers", &layout)
        .expect_err("access evaluation must revalidate its supposedly validated layout input");
    assert!(
        error.contains("is not the canonical validated layout"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn source_derived_stable_plan_binds_owned_content_and_preserves_retry_custody() {
    let main = write_program(
        "source-stable-owned-adoption",
        r#"
use omega::language::core::layout;

pub data Word {
    word: u32;
}

pub data HomePlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine HomePlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let mut owned_entries: [FieldEntry; 64];
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: owned_entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::Stable {
                read: true,
                write: true,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

pub data ShiftedPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine ShiftedPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let mut owned_entries: [FieldEntry; 64];
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 }
    };
    PlacementPlan {
        layout: Plan {
            entries: owned_entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::Stable {
                read: true,
                write: true,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plans(
    home: &Placed<HomePlacement, Word>,
    shifted: &Placed<ShiftedPlacement, Word>
) {}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("both source-derived Stable placements should reach checked custody");
    let home = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "HomePlacement")
        .expect("home checked placement row");
    let shifted = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "ShiftedPlacement")
        .expect("shifted checked placement row");
    assert_ne!(home.policy_symbol, shifted.policy_symbol);
    assert_ne!(home.placement.identity(), shifted.placement.identity());
    assert_eq!(home.placement.layout().size, Some(8));
    assert_eq!(shifted.placement.layout().size, Some(8));

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        401,
        ExtentRightId::from_normalized_identity,
    )]);
    let (extent, content) = ExtentRootGrant::from_admitted_provider(
        provider_issuance(25),
        extent_identity(402, ExtentLineageId::from_normalized_identity),
        extent_identity(403, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(404, ExtentProvenanceId::from_normalized_identity),
        extent_identity(405, MappingEraId::from_normalized_identity),
    )
    .mint_provider_existing_content(
        0x8000,
        8,
        home.placement.content_interpretation(),
        extent_identity(406, ResidentClaimId::from_normalized_identity),
        extent_identity(
            407,
            ExtentContentValidityReceiptId::from_normalized_identity,
        ),
        extent_identity(408, ExtentContentCustodyReceiptId::from_normalized_identity),
    )
    .expect("provider-owned existing Stable content");
    let extent_snapshot = (
        extent.origin(),
        extent.lineage_root(),
        extent.base(),
        extent.length(),
        extent.address_space(),
        extent.rights().clone(),
        extent.provenance(),
        extent.era(),
    );
    let content_snapshot = (
        content.origin(),
        content.lineage_root(),
        content.base(),
        content.length(),
        content.address_space(),
        content.provenance(),
        content.era(),
        content.interpretation(),
        content.resident_claim(),
        content.validity_receipt(),
        content.custody_receipt(),
    );
    let profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(409).expect("profile receipt"),
        &extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("provider profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 8,
            peer: PeerWritability::Exclusive,
            stable: StableCapability::ReadWrite,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted Stable profile");
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(410).expect("placement admission");
    let mismatched = admit_owned_placement(admission_id, extent, &shifted.placement, &profile)
        .expect("shifted placement is independently geometry/resource compatible");
    let rejection = adopt_owned_stable(mismatched, content)
        .expect_err("provider content must bind the exact checked source placement");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("interpretation commitment does not match the admitted placement")
    );
    let (returned_admission, returned_content, _) = rejection.into_parts();
    assert_eq!(returned_admission.identity(), admission_id);
    assert_eq!(returned_admission.placement_plan(), &shifted.placement);
    assert_eq!(
        (
            returned_admission.extent().origin(),
            returned_admission.extent().lineage_root(),
            returned_admission.extent().base(),
            returned_admission.extent().length(),
            returned_admission.extent().address_space(),
            returned_admission.extent().rights().clone(),
            returned_admission.extent().provenance(),
            returned_admission.extent().era(),
        ),
        extent_snapshot,
    );
    assert_eq!(
        (
            returned_content.origin(),
            returned_content.lineage_root(),
            returned_content.base(),
            returned_content.length(),
            returned_content.address_space(),
            returned_content.provenance(),
            returned_content.era(),
            returned_content.interpretation(),
            returned_content.resident_claim(),
            returned_content.validity_receipt(),
            returned_content.custody_receipt(),
        ),
        content_snapshot,
    );

    let returned_extent = returned_admission.withdraw();
    let corrected = admit_owned_placement(admission_id, returned_extent, &home.placement, &profile)
        .expect("the returned exact extent supports corrected admission");
    let dormant = adopt_owned_stable(corrected, returned_content)
        .expect("returned content supports exact source-plan retry");
    assert_eq!(dormant.admission(), admission_id);
    assert_eq!(dormant.placement_plan(), &home.placement);
    assert_eq!(
        (
            dormant.extent().origin(),
            dormant.extent().lineage_root(),
            dormant.extent().base(),
            dormant.extent().length(),
            dormant.extent().address_space(),
            dormant.extent().rights().clone(),
            dormant.extent().provenance(),
            dormant.extent().era(),
        ),
        extent_snapshot,
    );
    assert_eq!(dormant.resident_claim(), content_snapshot.8);
    assert_eq!(dormant.validity_receipt(), content_snapshot.9);
    assert_eq!(dormant.custody_receipt(), content_snapshot.10);
}

#[test]
fn source_derived_external_plan_binds_correspondence_and_preserves_retry_custody() {
    let main = write_program(
        "source-external-correspondence",
        r#"
use omega::language::core::layout;

pub data Register {
    value: u32;
}

pub data HomePlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine HomePlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let mut owned_entries: [FieldEntry; 64];
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: owned_entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::External {
                read: ExternalRead::Read,
                write: true,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

pub data ShiftedPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine ShiftedPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let mut owned_entries: [FieldEntry; 64];
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 }
    };
    PlacementPlan {
        layout: Plan {
            entries: owned_entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::External {
                read: ExternalRead::Read,
                write: true,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plans(
    home: &Placed<HomePlacement, Register>,
    shifted: &Placed<ShiftedPlacement, Register>
) {}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("both source-derived External placements should reach checked custody");
    let home = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "HomePlacement")
        .expect("home checked placement row");
    let shifted = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "ShiftedPlacement")
        .expect("shifted checked placement row");
    assert_ne!(home.policy_symbol, shifted.policy_symbol);
    assert_ne!(home.placement.identity(), shifted.placement.identity());
    assert_eq!(home.placement.layout().size, Some(8));
    assert_eq!(shifted.placement.layout().size, Some(8));
    assert!(matches!(
        home.placement
            .access()
            .plan()
            .entries()
            .first()
            .expect("home External field")
            .access(),
        FieldAccess::External {
            read: ExternalRead::Read,
            write: true,
            ..
        }
    ));

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        411,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        provider_issuance(26),
        extent_identity(412, ExtentLineageId::from_normalized_identity),
        extent_identity(413, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(414, ExtentProvenanceId::from_normalized_identity),
        extent_identity(415, MappingEraId::from_normalized_identity),
    )
    .mint(0x9000, 8)
    .expect("provider External extent");
    let loan_snapshot = (
        extent.origin(),
        extent.lineage_root(),
        extent.base(),
        extent.length(),
        extent.address_space(),
        extent.rights().clone(),
        extent.provenance(),
        extent.era(),
    );
    let external_profile = ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 8,
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
            reach: BoundaryReach::default(),
        }],
    };
    let exact_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(416).expect("exact profile receipt"),
        &extent,
        rights.clone(),
        BoundaryReach::default(),
    )
    .expect("exact provider profile grant")
    .admit(external_profile.clone())
    .expect("exact admitted External profile");
    let alternate_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(417).expect("alternate profile receipt"),
        &extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("alternate provider profile grant")
    .admit(external_profile)
    .expect("alternate admitted External profile");

    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(418)
        .expect("correspondence provider");
    let device =
        StableDeviceInstanceId::from_normalized_identity(419).expect("stable device identity");
    let source =
        SchemaCorrespondenceSourceId::from_normalized_identity(420).expect("correspondence source");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        source,
        &home.placement,
        exact_profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&home.placement, &exact_profile)
    .expect("source-derived correspondence admission");
    let correspondence_snapshot = (
        correspondence.provider(),
        correspondence.device(),
        correspondence.source(),
        correspondence.placement(),
        correspondence.profile_receipt(),
    );
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(421).expect("placement admission");

    let loan = extent.loan(0, 8).expect("shared External loan");
    let wrong_plan = admit_placement(admission_id, loan, &shifted.placement, &exact_profile)
        .expect("shifted source plan is independently resource-compatible");
    let rejection = bind_schema_correspondence_to_placement(wrong_plan, correspondence)
        .expect_err("correspondence must reject a different retained source plan");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (wrong_plan, correspondence, _) = rejection.into_parts();
    assert_eq!(wrong_plan.identity(), admission_id);
    assert_eq!(wrong_plan.profile_receipt(), exact_profile.receipt());
    assert_eq!(
        (
            correspondence.provider(),
            correspondence.device(),
            correspondence.source(),
            correspondence.placement(),
            correspondence.profile_receipt(),
        ),
        correspondence_snapshot,
    );
    let loan = wrong_plan.withdraw();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );

    let wrong_profile = admit_placement(admission_id, loan, &home.placement, &alternate_profile)
        .expect("alternate profile is independently resource-compatible");
    let rejection = bind_schema_correspondence_to_placement(wrong_profile, correspondence)
        .expect_err("correspondence must reject a different admitted profile receipt");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("resource-profile receipt")
    );
    let (wrong_profile, correspondence, _) = rejection.into_parts();
    assert_eq!(wrong_profile.identity(), admission_id);
    assert_eq!(wrong_profile.profile_receipt(), alternate_profile.receipt());
    assert_eq!(
        (
            correspondence.provider(),
            correspondence.device(),
            correspondence.source(),
            correspondence.placement(),
            correspondence.profile_receipt(),
        ),
        correspondence_snapshot,
    );
    let loan = wrong_profile.withdraw();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );

    let corrected = admit_placement(admission_id, loan, &home.placement, &exact_profile)
        .expect("returned loan supports exact source-plan/profile retry");
    let bound = bind_schema_correspondence_to_placement(corrected, correspondence)
        .expect("returned correspondence supports exact retry");
    assert_eq!(bound.admission(), admission_id);
    assert_eq!(
        (
            bound.correspondence().provider(),
            bound.correspondence().device(),
            bound.correspondence().source(),
            bound.correspondence().placement(),
            bound.correspondence().profile_receipt(),
        ),
        correspondence_snapshot,
    );
    let (loan, correspondence) = bound.withdraw();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );
    assert_eq!(
        (
            correspondence.provider(),
            correspondence.device(),
            correspondence.source(),
            correspondence.placement(),
            correspondence.profile_receipt(),
        ),
        correspondence_snapshot,
    );
}

#[test]
fn source_derived_atomic_plan_rejects_underpowered_profile_and_preserves_retry_custody() {
    let main = write_program(
        "source-atomic-profile-retry",
        r#"
use omega::language::core::layout;

pub data Counter {
    value: u32;
}

pub data AtomicPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine AtomicPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let mut owned_entries: [FieldEntry; 64];
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: owned_entries,
            entry_count: 1,
            size_fixed: 4,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::Atomic {
                operations: AtomicOperations {
                    load: true,
                    store: false,
                    fetch_add: true,
                    fetch_sub: false,
                    fetch_xor: false,
                    fetch_or: false,
                    fetch_and: false,
                    swap: false,
                    compare_exchange: false,
                    compare_exchange_once: false,
                    try_exchange: false,
                    try_exchange_once: false
                },
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plan(counter: &Placed<AtomicPlacement, Counter>) {}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("source-derived Atomic placement should reach checked custody");
    let retained = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "AtomicPlacement")
        .expect("checked Atomic placement row");
    let atomic_access = retained
        .placement
        .access()
        .plan()
        .entries()
        .first()
        .expect("retained Atomic field")
        .access();
    assert!(matches!(
        atomic_access,
        FieldAccess::Atomic { operations, .. }
            if operations.load && operations.fetch_add && !operations.store
    ));
    assert_eq!(retained.placement.layout().size, Some(4));

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        422,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        provider_issuance(27),
        extent_identity(423, ExtentLineageId::from_normalized_identity),
        extent_identity(424, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(425, ExtentProvenanceId::from_normalized_identity),
        extent_identity(426, MappingEraId::from_normalized_identity),
    )
    .mint(0xa000, 4)
    .expect("provider Atomic extent");
    let loan_snapshot = (
        extent.origin(),
        extent.lineage_root(),
        extent.base(),
        extent.length(),
        extent.address_space(),
        extent.rights().clone(),
        extent.provenance(),
        extent.era(),
    );
    let underpowered_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(427)
            .expect("underpowered profile receipt"),
        &extent,
        rights.clone(),
        BoundaryReach::default(),
    )
    .expect("underpowered provider profile grant")
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
                        ..AtomicPermissions::default()
                    },
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted load-only Atomic profile");
    let exact_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(428).expect("exact profile receipt"),
        &extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("exact provider profile grant")
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
                        fetch_add: true,
                        ..AtomicPermissions::default()
                    },
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted exact Atomic profile");

    let admission_id =
        PlacementAdmissionId::from_normalized_identity(429).expect("Atomic placement admission");
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let rejection = admit_placement(
        admission_id,
        loan,
        &retained.placement,
        &underpowered_profile,
    )
    .expect_err("load-only supply must not satisfy source-requested fetch-add");
    assert!(
        rejection.diagnostic().0.contains("value")
            && rejection.diagnostic().0.contains("operation families"),
        "unexpected Atomic profile diagnostic: {}",
        rejection.diagnostic().0,
    );
    let (loan, _) = rejection.into_parts();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );

    let admission = admit_placement(admission_id, loan, &retained.placement, &exact_profile)
        .expect("returned loan supports exact Atomic plan/profile retry");
    assert_eq!(admission.identity(), admission_id);
    assert_eq!(admission.profile_receipt(), exact_profile.receipt());
    assert_eq!(
        admission.resources().placement(),
        retained.placement.identity()
    );
    assert_eq!(
        admission.resources().profile(),
        exact_profile.profile().identity(),
    );
    let fields = admission.resources().fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind(), EffectiveSupplyKind::Atomic);
    assert_eq!(fields[0].offset(), 0);
    assert_eq!(fields[0].width_bits(), 32);
    assert_eq!(fields[0].alignment_bytes(), 4);

    let loan = admission.withdraw();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );
}

#[test]
fn source_derived_take_plan_rejects_repeatable_profile_and_preserves_retry_custody() {
    let main = write_program(
        "source-take-profile-retry",
        r#"
use omega::language::core::layout;

pub data Fifo {
    sample: u32;
}

pub data DestructivePlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine DestructivePlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let mut owned_entries: [FieldEntry; 64];
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: owned_entries,
            entry_count: 1,
            size_fixed: 4,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::External {
                read: ExternalRead::Take,
                write: false,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plan(fifo: &mut Placed<DestructivePlacement, Fifo>) {}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("source-derived destructive External placement should reach checked custody");
    let retained = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "DestructivePlacement")
        .expect("checked destructive External placement row");
    assert!(matches!(
        retained
            .placement
            .access()
            .plan()
            .entries()
            .first()
            .expect("retained destructive External field")
            .access(),
        FieldAccess::External {
            read: ExternalRead::Take,
            write: false,
            ..
        }
    ));
    assert_eq!(retained.placement.layout().size, Some(4));

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        430,
        ExtentRightId::from_normalized_identity,
    )]);
    let mut extent = ExtentRootGrant::from_admitted_provider(
        provider_issuance(28),
        extent_identity(431, ExtentLineageId::from_normalized_identity),
        extent_identity(432, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(433, ExtentProvenanceId::from_normalized_identity),
        extent_identity(434, MappingEraId::from_normalized_identity),
    )
    .mint(0xb000, 4)
    .expect("provider destructive External extent");
    let repeatable_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(435)
            .expect("Repeatable profile receipt"),
        &extent,
        rights.clone(),
        BoundaryReach::default(),
    )
    .expect("Repeatable provider profile grant")
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
    .expect("admitted Repeatable External profile");
    let destructive_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(436)
            .expect("Destructive profile receipt"),
        &extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("Destructive provider profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
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
        }],
    })
    .expect("admitted Destructive External profile");

    let admission_id = PlacementAdmissionId::from_normalized_identity(437)
        .expect("destructive External placement admission");
    let loan = extent
        .loan_mut(0, 4)
        .expect("exclusive destructive External loan");
    let loan_snapshot = (
        loan.polarity(),
        loan.origin(),
        loan.lineage_root(),
        loan.base(),
        loan.length(),
        loan.address_space(),
        loan.rights().clone(),
        loan.provenance(),
        loan.era(),
    );
    let rejection = admit_placement(admission_id, loan, &retained.placement, &repeatable_profile)
        .expect_err("Repeatable supply must not satisfy source-requested destructive Take");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("field `sample` requests incompatible External 32-bit read=Take write=false"),
        "unexpected destructive External diagnostic: {}",
        rejection.diagnostic().0,
    );
    let (loan, _) = rejection.into_parts();
    assert_eq!(
        (
            loan.polarity(),
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );

    let admission = admit_placement(
        admission_id,
        loan,
        &retained.placement,
        &destructive_profile,
    )
    .expect("returned loan supports exact destructive External plan/profile retry");
    assert_eq!(admission.identity(), admission_id);
    assert_eq!(admission.profile_receipt(), destructive_profile.receipt());
    assert_eq!(
        admission.resources().placement(),
        retained.placement.identity()
    );
    assert_eq!(
        admission.resources().profile(),
        destructive_profile.profile().identity(),
    );
    let fields = admission.resources().fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind(), EffectiveSupplyKind::External);
    assert_eq!(fields[0].offset(), 0);
    assert_eq!(fields[0].width_bits(), 32);
    assert_eq!(fields[0].alignment_bytes(), 4);

    let loan = admission.withdraw();
    assert_eq!(
        (
            loan.polarity(),
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );
}
