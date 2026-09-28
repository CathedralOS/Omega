use super::{
    POST_HANDOFF_WRITER_CONTEXT_ABI_V1, PostHandoffWriterPlan, PostHandoffWriterSource,
    PostHandoffWriterStep, derive_post_handoff_writer, post_handoff_writer_context_byte_len,
};
use layout_plans::{
    ByteOrder, ConsumptionInstant, EntryStubId, IntegerInterpretation, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport, MaterializationAction, MaterializationContext,
    MaterializationWrite, PlacementConstraints, PlacementPhase, PlacementSite, RelocationTarget,
    SymbolicFieldValue, SymbolicMaterializationPlan, derive_symbolic_materialization,
};

fn entry() -> RelocationTarget {
    RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("nonzero identity"),
    )
}

fn split_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "address".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 16,
                },
            },
            LayoutFieldEntryReport {
                field: "address".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 2,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 16,
                    width: 16,
                },
            },
            LayoutFieldEntryReport {
                field: "address".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 8,
                    container_width: 64,
                    destination_lsb: 0,
                    source_lsb: 32,
                    width: 32,
                },
            },
        ],
        offsets: None,
        size: Some(16),
        align: 1,
    }
}

fn post_handoff_context() -> MaterializationContext {
    MaterializationContext {
        consumption: ConsumptionInstant::AfterOmegaHandoff,
        byte_order: ByteOrder::LittleEndian,
        native_pointer_relocation_bits: None,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
    }
}

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
    let writer = derive_post_handoff_writer(&plan).expect("post-handoff writer");
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
fn empty_post_handoff_writer_rejects_derivation_and_lowering() {
    let symbolic = SymbolicMaterializationPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        actions: Vec::new(),
    };
    let error = derive_post_handoff_writer(&symbolic)
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
        derive_post_handoff_writer(&plan)
            .expect_err("loader relocation is not a writer instruction")
            .0
            .contains("loader-native")
    );
}

#[test]
fn numbered_symbolic_materialization_rejoins_renamed_fields_by_identity() {
    let mut layout = split_layout();
    for entry in &mut layout.entries {
        entry.field = "legacy_address".into();
        entry.member_identity = Some(7);
    }
    let context = post_handoff_context();
    let symbolic =
        SymbolicFieldValue::new_numbered("handler", 7, 64, entry()).expect("numbered target");
    let plan =
        derive_symbolic_materialization(&layout, std::slice::from_ref(&symbolic), context, |_| {
            None
        })
        .expect("stable identity should rejoin renamed symbolic fragments");
    assert_eq!(plan.actions.len(), 3);
    assert!(plan.actions.iter().all(|action| {
        matches!(action, MaterializationAction::RuntimeWriter(write) if write.field == "handler")
    }));

    let current_fragment = derive_post_handoff_writer(&plan)
        .expect("renamed symbolic plan derives a writer")
        .lower_reusable_fragment()
        .expect("renamed symbolic writer lowers");
    let legacy = SymbolicFieldValue::new_numbered("legacy_address", 7, 64, entry())
        .expect("legacy numbered target");
    let legacy_fragment = derive_symbolic_materialization(&layout, &[legacy], context, |_| None)
        .expect("matching presentation spelling also derives");
    let legacy_fragment = derive_post_handoff_writer(&legacy_fragment)
        .expect("legacy symbolic plan derives a writer")
        .lower_reusable_fragment()
        .expect("legacy symbolic writer lowers");
    assert_eq!(
        current_fragment.fragment.report_fingerprint(),
        legacy_fragment.fragment.report_fingerprint(),
        "presentation spelling must not change generated writer identity"
    );
}

#[test]
fn stored_integer_fit_binds_post_handoff_fragment_evidence() {
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
    let plan = derive_symbolic_materialization(
        &layout,
        std::slice::from_ref(&symbolic),
        post_handoff_context(),
        |_| None,
    )
    .expect("a post-handoff resolver can discharge stored-integer fit");
    assert!(matches!(
        plan.actions.as_slice(),
        [MaterializationAction::RuntimeWriter(write)]
            if write.stored_integer_fit.is_some()
    ));
    let writer =
        derive_post_handoff_writer(&plan).expect("stored-integer runtime action derives a writer");
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
}
