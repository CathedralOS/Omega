use super::{entry, split_layout};
use crate::post_handoff_writer::apply_post_handoff_writes_atomically;
use crate::{
    ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, EntryStubId, IntegerInterpretation,
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport, MachineRegimeId,
    MaterializationAction, MaterializationContext, MaterializationWrite,
    POST_HANDOFF_WRITER_CONTEXT_ABI_V1, PlacementAddressRange, PlacementConstraints,
    PlacementPhase, PlacementSite, PostHandoffWriterPlan, PostHandoffWriterSource,
    PostHandoffWriterStep, RelocationTarget, StoredIntegerFit, SymbolicFieldValue,
    SymbolicMaterializationPlan, derive_symbolic_materialization,
    post_handoff_writer_context_byte_len,
};

#[test]
fn reusable_writer_fragment_separates_static_geometry_from_invocation_evidence() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("symbolic materialization");
    let writer = plan
        .derive_post_handoff_writer()
        .expect("post-handoff writer");
    let lowering = writer
        .lower_reusable_fragment()
        .expect("address-free reusable fragment");

    assert_eq!(
        lowering.fragment.context_abi(),
        POST_HANDOFF_WRITER_CONTEXT_ABI_V1
    );
    assert_eq!(lowering.fragment.source_slot_count(), 1);
    assert_eq!(lowering.sources.len(), 1);
    assert_eq!(lowering.sources[0].target, entry());
    assert_eq!(
        lowering.sources[0].source,
        PostHandoffWriterSource::Resolve(entry())
    );
    assert!(
        lowering
            .fragment
            .steps()
            .iter()
            .all(|step| step.source_slot == 0),
        "all three fragments of one symbolic target share one private slot"
    );
    assert_eq!(post_handoff_writer_context_byte_len(1), Some(16));

    let replacement = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("replacement entry"),
    );
    let mut rebound = writer.clone();
    for step in &mut rebound.steps {
        step.write.target = replacement;
        step.source = PostHandoffWriterSource::Resolve(replacement);
    }
    rebound.placement =
        PlacementConstraints::new(None, 16, PlacementPhase::PostHandoff, None, None)
            .expect("stronger invocation placement");
    let rebound = rebound
        .lower_reusable_fragment()
        .expect("same reusable geometry");

    assert_eq!(
        rebound.fragment.report_fingerprint(),
        lowering.fragment.report_fingerprint(),
        "target identity and concrete placement are invocation evidence"
    );
    assert_eq!(rebound.fragment, lowering.fragment);
    assert_ne!(rebound.sources, lowering.sources);
    assert_ne!(rebound.placement, lowering.placement);
}

#[test]
fn reusable_writer_fragment_rejects_inconsistent_values_for_one_target() {
    let write = MaterializationWrite {
        field: "address".into(),
        target: entry(),
        container_byte_offset: 0,
        container_width_bits: 64,
        destination_lsb: 0,
        source_lsb: 0,
        width: 32,
        stored_integer_fit: None,
    };
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![
            PostHandoffWriterStep {
                write: write.clone(),
                source: PostHandoffWriterSource::Resolved(1),
            },
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    destination_lsb: 32,
                    source_lsb: 32,
                    ..write
                },
                source: PostHandoffWriterSource::Resolved(2),
            },
        ],
    };

    let error = writer
        .lower_reusable_fragment()
        .expect_err("one symbolic source cannot change between fragments");
    assert!(error.0.contains("inconsistent invocation values"));
    let error = writer
        .validate(
            8,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
        )
        .expect_err("direct execution validates the same source invariant");
    assert!(error.0.contains("inconsistent invocation values"));
}

#[test]
fn invocation_source_validation_rejects_preresolved_value_substitution() {
    let target = entry();
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolved(0x1234),
        }],
    };
    let invocation = writer
        .lower_reusable_fragment()
        .expect("valid pre-resolved writer invocation");
    invocation
        .validate_source_values(&[0x1234])
        .expect("the exact retained pre-resolved value remains valid");

    let error = invocation
        .validate_source_values(&[0x5678])
        .expect_err("source values cannot substitute pre-resolved invocation evidence");
    assert!(error.0.contains("source slot 0"), "{}", error.0);
    assert!(error.0.contains("0x5678"), "{}", error.0);
    assert!(error.0.contains("0x1234"), "{}", error.0);
}

#[test]
fn invocation_structure_replay_rejects_tamper_and_preserves_retry() {
    let target = entry();
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    };
    let invocation = writer
        .lower_reusable_fragment()
        .expect("valid reusable invocation");
    invocation
        .validate_structure()
        .expect("lowering produces canonical structure");

    let mut wrong_abi = invocation.clone();
    wrong_abi.fragment.context_abi ^= 1;
    assert!(
        wrong_abi
            .validate_structure()
            .expect_err("context ABI drift must reject")
            .0
            .contains("context ABI")
    );

    let mut missing_slot = invocation.clone();
    missing_slot.fragment.steps[0].source_slot = 1;
    assert!(
        missing_slot
            .validate_structure()
            .expect_err("missing source slot must reject")
            .0
            .contains("missing source slot")
    );

    let mut zero_alignment = invocation.clone();
    zero_alignment.placement.alignment = 0;
    assert!(
        zero_alignment
            .validate_structure()
            .expect_err("zero alignment must reject")
            .0
            .contains("alignment")
    );

    let mut wrong_fingerprint = invocation.clone();
    wrong_fingerprint.fragment.report_fingerprint ^= 1;
    assert!(
        wrong_fingerprint
            .validate_structure()
            .expect_err("fragment fingerprint drift must reject")
            .0
            .contains("fingerprint")
    );

    invocation
        .validate_source_values(&[0x1234])
        .expect("the untouched invocation remains valid for retry");
}

#[test]
fn empty_post_handoff_writer_rejects_every_execution_path() {
    let symbolic = SymbolicMaterializationPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        actions: Vec::new(),
    };
    let error = symbolic
        .derive_post_handoff_writer()
        .expect_err("empty symbolic actions cannot claim a generated writer");
    assert!(error.0.contains("at least one fragment"), "{}", error.0);

    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: Vec::new(),
    };
    let error = writer
        .lower_reusable_fragment()
        .expect_err("empty direct plans cannot claim reusable lowering");
    assert!(error.0.contains("at least one fragment"), "{}", error.0);

    let mut bytes = [0xa5; 8];
    let mut resolutions = 0;
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |_| {
                resolutions += 1;
                Some(0)
            },
        )
        .expect_err("empty direct execution cannot report writer success");
    assert!(error.0.contains("at least one fragment"), "{}", error.0);
    assert_eq!(resolutions, 0);
    assert_eq!(bytes, [0xa5; 8]);
}

#[test]
fn writer_stops_resolving_after_one_target_fails_value_validation() {
    let first = entry();
    let second = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("second entry identity"),
    );
    let writer = PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "narrow".into(),
                    target: first,
                    container_byte_offset: 0,
                    container_width_bits: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                    stored_integer_fit: Some(StoredIntegerFit {
                        source_width_bits: 64,
                        stored_width_bits: 8,
                        interpretation: IntegerInterpretation::Unsigned,
                    }),
                },
                source: PostHandoffWriterSource::Resolve(first),
            },
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "later".into(),
                    target: second,
                    container_byte_offset: 8,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source: PostHandoffWriterSource::Resolve(second),
            },
        ],
    };
    let mut bytes = [0xa5; 16];
    let mut resolutions = Vec::new();
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |target| {
                resolutions.push(target);
                Some(if target == first { 0x100 } else { 0 })
            },
        )
        .expect_err("the first target does not fit its retained stored width");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(resolutions, vec![first]);
    assert_eq!(bytes, [0xa5; 16]);
}

#[test]
fn writer_rejects_invalid_preresolved_value_before_any_resolution() {
    let dynamic = entry();
    let pre_resolved = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("pre-resolved entry identity"),
    );
    let writer = PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "dynamic".into(),
                    target: dynamic,
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source: PostHandoffWriterSource::Resolve(dynamic),
            },
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "narrow".into(),
                    target: pre_resolved,
                    container_byte_offset: 8,
                    container_width_bits: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                    stored_integer_fit: Some(StoredIntegerFit {
                        source_width_bits: 64,
                        stored_width_bits: 8,
                        interpretation: IntegerInterpretation::Unsigned,
                    }),
                },
                source: PostHandoffWriterSource::Resolved(0x100),
            },
        ],
    };

    let error = writer
        .lower_reusable_fragment()
        .expect_err("lowering must reject known-invalid invocation evidence");
    assert!(error.0.contains("does not fit"), "{}", error.0);

    let mut bytes = [0xa5; 16];
    let mut resolutions = 0;
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |_| {
                resolutions += 1;
                Some(0)
            },
        )
        .expect_err("known-invalid invocation evidence must reject during static preflight");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(resolutions, 0);
    assert_eq!(bytes, [0xa5; 16]);
}

#[test]
fn writer_rejects_a_resolved_source_that_does_not_match_its_write_target() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("writer plan");
    let mut writer = plan
        .derive_post_handoff_writer()
        .expect("runtime actions form a writer program");
    let substituted = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("second entry identity"),
    );
    writer.steps[0].source = PostHandoffWriterSource::Resolve(substituted);

    let mut bytes = [0xa5_u8; 16];
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |_| panic!("mismatched writer target must reject before resolution"),
        )
        .expect_err("writer source substitution must reject");
    assert!(error.0.contains("does not match write target"));
    assert_eq!(bytes, [0xa5; 16]);
}

#[test]
fn writer_validates_every_step_before_direct_destination_writes() {
    let valid = MaterializationWrite {
        field: "valid".into(),
        target: entry(),
        container_byte_offset: 0,
        container_width_bits: 64,
        destination_lsb: 0,
        source_lsb: 0,
        width: 64,
        stored_integer_fit: None,
    };
    let invalid = MaterializationWrite {
        field: "outside".into(),
        container_byte_offset: 16,
        ..valid.clone()
    };
    let writer = PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![
            PostHandoffWriterStep {
                write: valid,
                source: PostHandoffWriterSource::Resolve(entry()),
            },
            PostHandoffWriterStep {
                write: invalid,
                source: PostHandoffWriterSource::Resolve(entry()),
            },
        ],
    };
    let mut bytes = [0xa5_u8; 16];
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |_| Some(0x1122_3344_5566_7788),
        )
        .expect_err("invalid later step must reject before direct writes begin");
    assert!(error.0.contains("outside"));
    assert_eq!(bytes, [0xa5; 16]);
}

#[test]
fn writer_application_stages_late_failure_and_retry_atomically() {
    let first = PostHandoffWriterStep {
        write: MaterializationWrite {
            field: "first".into(),
            target: entry(),
            container_byte_offset: 0,
            container_width_bits: 64,
            destination_lsb: 0,
            source_lsb: 0,
            width: 64,
            stored_integer_fit: None,
        },
        source: PostHandoffWriterSource::Resolve(entry()),
    };
    let outside = PostHandoffWriterStep {
        write: MaterializationWrite {
            field: "outside".into(),
            container_byte_offset: 16,
            ..first.write.clone()
        },
        source: first.source,
    };
    let values = [0x1122_3344_5566_7788, 0x99aa_bbcc_ddee_ff00];
    let mut bytes = [0xa5_u8; 16];

    let error = apply_post_handoff_writes_atomically(
        &mut bytes,
        ByteOrder::LittleEndian,
        &[first.clone(), outside.clone()],
        &values,
    )
    .expect_err("late application failure must reject the staged image");
    assert!(error.0.contains("outside"));
    assert_eq!(bytes, [0xa5; 16]);

    let mut repaired = outside;
    repaired.write.container_byte_offset = 8;
    apply_post_handoff_writes_atomically(
        &mut bytes,
        ByteOrder::LittleEndian,
        &[first, repaired],
        &values,
    )
    .expect("repaired staged image commits once");
    assert_eq!(
        bytes,
        [
            0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00, 0xff, 0xee, 0xdd, 0xcc, 0xbb,
            0xaa, 0x99,
        ]
    );
}

#[test]
fn fixed_entry_constant_folds_split_little_endian_bytes() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        },
        |_| Some(0x1122_3344_5566_7788),
    )
    .expect("fixed-address fragments constant-fold");
    let mut bytes = [0_u8; 16];
    plan.materialize_resolved_into(&mut bytes)
        .expect("all writes resolved");

    assert_eq!(&bytes[0..4], &[0x88, 0x77, 0x66, 0x55]);
    assert_eq!(&bytes[8..12], &[0x44, 0x33, 0x22, 0x11]);
}

#[test]
fn symbolic_stored_integer_fit_is_enforced_before_each_consumption_phase() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "address".into(),
            member_identity: None,
            placement: LayoutPlacementReport::IntegerAt {
                offset: 0,
                stored_width: 32,
                interpretation: IntegerInterpretation::Unsigned,
            },
        }],
        offsets: None,
        size: Some(4),
        align: 4,
    };
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let context = MaterializationContext {
        consumption: ConsumptionInstant::BeforeOmegaEntry,
        byte_order: ByteOrder::LittleEndian,
        native_pointer_relocation_bits: Some(64),
        placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
    };
    let plan =
        derive_symbolic_materialization(&layout, std::slice::from_ref(&symbolic), context, |_| {
            Some(0x1234_5678)
        })
        .expect("a resolved fitting symbolic value supplies a concrete fit proof");
    let mut bytes = [0xa5_u8; 4];

    let mut tampered = plan.clone();
    let MaterializationAction::ResolvedWrite { source_value, .. } = &mut tampered.actions[0] else {
        panic!("resolved derivation must retain a resolved write")
    };
    *source_value = 1_u64 << 32;
    let error = tampered
        .materialize_resolved_into(&mut bytes)
        .expect_err("resolved execution must replay retained stored-integer fit evidence");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(
        bytes, [0xa5; 4],
        "tampered resolved values reject before destination mutation"
    );

    plan.materialize_resolved_into(&mut bytes)
        .expect("the resolved value should use the exact stored width");
    assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);

    let error =
        derive_symbolic_materialization(&layout, std::slice::from_ref(&symbolic), context, |_| {
            Some(1_u64 << 32)
        })
        .expect_err("an out-of-range symbolic value must reject");
    assert!(error.0.contains("does not fit"), "{}", error.0);

    let post_handoff = derive_symbolic_materialization(
        &layout,
        std::slice::from_ref(&symbolic),
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("a post-handoff resolver can discharge stored-integer fit");
    assert!(matches!(
        post_handoff.actions.as_slice(),
        [MaterializationAction::RuntimeWriter(write)]
            if write.stored_integer_fit.is_some()
    ));
    let writer = post_handoff
        .derive_post_handoff_writer()
        .expect("stored-integer runtime action derives a writer");
    let invocation = writer
        .lower_reusable_fragment()
        .expect("stored-integer fit remains invocation evidence");
    assert_eq!(invocation.fit_constraints().len(), 1);
    invocation
        .validate_structure()
        .expect("stored-integer fit binds exact generated geometry");
    let mut drifted_fit = invocation.clone();
    drifted_fit.fit_constraints[0].source_slot = 1;
    let error = drifted_fit
        .validate_structure()
        .expect_err("fit evidence cannot move to a missing source slot");
    assert!(error.0.contains("does not bind"), "{}", error.0);
    invocation
        .validate_structure()
        .expect("fit rejection leaves the original invocation reusable");

    let site = PlacementSite {
        base_address: 0,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let mut bytes = [0xa5_u8; 4];
    writer
        .execute(&mut bytes, site, |_| Some(0x1234_5678))
        .expect("resolved post-handoff value fits stored width");
    assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);

    bytes.fill(0xa5);
    let error = writer
        .execute(&mut bytes, site, |_| Some(1_u64 << 32))
        .expect_err("post-handoff resolution must reject an out-of-range value");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(bytes, [0xa5; 4], "fit rejection must precede every write");

    let error = derive_symbolic_materialization(&layout, &[symbolic], context, |_| None)
        .expect_err("a loader cannot defer stored-integer fit to Omega");
    assert!(error.0.contains("before Omega entry"), "{}", error.0);
}

#[test]
fn unresolved_loader_consumed_fragments_reject() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let error = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        },
        |_| None,
    )
    .expect_err("a loader cannot apply split pointer relocations");

    assert!(error.0.contains("before Omega entry"));
}

#[test]
fn whole_pointer_uses_loader_native_relocation() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "entry".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 8 },
        }],
        offsets: Some(vec![8]),
        size: Some(16),
        align: 8,
    };
    let symbolic = SymbolicFieldValue::new("entry", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &layout,
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        },
        |_| None,
    )
    .expect("whole-pointer native relocation is available");

    assert!(matches!(
        plan.actions.as_slice(),
        [MaterializationAction::NativePointerRelocation {
            destination_byte_offset: 8,
            width_bits: 64,
            ..
        }]
    ));
    assert!(
        plan.derive_post_handoff_writer()
            .expect_err("loader relocation is not a writer instruction")
            .0
            .contains("loader-native")
    );
}

#[test]
fn unresolved_action_cannot_partially_materialize() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("writer plan");
    let mut bytes = [0xa5_u8; 16];
    assert!(plan.materialize_resolved_into(&mut bytes).is_err());
    assert_eq!(bytes, [0xa5; 16]);

    let writer = plan
        .derive_post_handoff_writer()
        .expect("runtime actions form a writer program");
    let mut missing = [0xa5_u8; 16];
    assert!(
        writer
            .execute(
                &mut missing,
                PlacementSite {
                    base_address: 0,
                    phase: PlacementPhase::PostHandoff,
                    machine_regime: None,
                    installation_scope: None,
                },
                |_| None,
            )
            .is_err()
    );
    assert_eq!(missing, [0xa5; 16]);
}

#[test]
fn placement_constraints_join_layout_alignment_and_validate_all_axes() {
    let regime = MachineRegimeId::from_normalized_identity(11).expect("machine regime");
    let scope =
        ArtifactInstallationScopeId::from_normalized_identity(12).expect("installation scope");
    let constraints = PlacementConstraints::new(
        Some(PlacementAddressRange::new(0x1000, 0x10_0000).expect("low-memory range")),
        4096,
        PlacementPhase::PostHandoff,
        Some(regime),
        Some(scope),
    )
    .expect("placement constraints");
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let mut layout = split_layout();
    layout.align = 16;
    let plan = derive_symbolic_materialization(
        &layout,
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: constraints,
        },
        |_| None,
    )
    .expect("constrained post-handoff plan");

    assert_eq!(plan.placement.alignment(), 4096);
    plan.placement
        .validate_site(
            plan.byte_len,
            PlacementSite {
                base_address: 0x8000,
                phase: PlacementPhase::PostHandoff,
                machine_regime: Some(regime),
                installation_scope: Some(scope),
            },
        )
        .expect("all concrete placement facts match");

    let wrong_phase = PlacementSite {
        base_address: 0x8000,
        phase: PlacementPhase::Load,
        machine_regime: Some(regime),
        installation_scope: Some(scope),
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, wrong_phase)
            .expect_err("phase is part of the normalized constraint")
            .0
            .contains("phase")
    );
    let writer = plan
        .derive_post_handoff_writer()
        .expect("runtime actions form a writer program");
    let mut unchanged = [0xa5_u8; 16];
    assert!(
        writer
            .execute(&mut unchanged, wrong_phase, |_| Some(1))
            .is_err()
    );
    assert_eq!(unchanged, [0xa5; 16]);

    let misaligned = PlacementSite {
        base_address: 0x8001,
        phase: PlacementPhase::PostHandoff,
        ..wrong_phase
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, misaligned)
            .expect_err("layout and policy alignment are mandatory")
            .0
            .contains("aligned")
    );

    let wrong_regime = PlacementSite {
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        ..wrong_phase
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, wrong_regime)
            .expect_err("machine regime is required")
            .0
            .contains("regime")
    );

    let wrong_scope = PlacementSite {
        machine_regime: Some(regime),
        installation_scope: None,
        ..wrong_regime
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, wrong_scope)
            .expect_err("installation scope is required")
            .0
            .contains("scope")
    );

    let outside_range = PlacementSite {
        base_address: 0x10_0000,
        installation_scope: Some(scope),
        ..wrong_scope
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, outside_range)
            .expect_err("complete placement must fit the range")
            .0
            .contains("outside")
    );
}
