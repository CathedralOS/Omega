use super::{entry, split_layout};
use crate::{
    ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, IntegerInterpretation,
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport, MachineRegimeId,
    MaterializationAction, MaterializationContext, PlacementAddressRange, PlacementConstraints,
    PlacementPhase, PlacementSite, SymbolicFieldValue, derive_symbolic_materialization,
};

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
