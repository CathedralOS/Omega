use crate::plan_policy::access_fields_validation::validate_entry_geometry;
use crate::test_support::*;
use crate::*;
use layout_plans::test_support::*;
use layout_plans::*;
#[test]
fn stored_integer_geometry_uses_the_exact_encoded_width() {
    let layout = layout_report(
        vec![layout_field_entry(
            "value",
            LayoutPlacementReport::IntegerAt {
                offset: 3,
                stored_width: 16,
                interpretation: IntegerInterpretation::Signed,
            },
        )],
        None,
        Some(8),
        1,
    );
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
    let mut plan = AccessFields::inaccessible(&layout).expect("canonical access seed");
    let compact_identity = plan.layout_report_fingerprint;
    plan.retained_layout.entries[0].placement = LayoutPlacementReport::At { offset: 4 };
    assert_eq!(
        plan.layout_report_fingerprint, compact_identity,
        "the simulated carrier drift deliberately leaves its compact identity unchanged"
    );

    let error = validate_access_fields(plan, &layout)
        .expect_err("structural carrier drift must reject before access-plan sealing");
    assert!(
        error.0.contains("different validated layout"),
        "{}",
        error.0
    );
}
#[test]
fn field_keys_reject_cross_layout_and_out_of_cardinality_use() {
    let layout = uart_layout();
    let mut plan = AccessFields::inaccessible(&layout).expect("UART seed");
    let mut alternate_layout = layout.clone();
    alternate_layout.schema_report_fingerprint = 2;
    let mut alternate =
        AccessFields::inaccessible(&alternate_layout).expect("alternate schema seed");
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
            stable_word32_read(AccessExposure::Exported),
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
            stable_word32_read(AccessExposure::Exported),
        )
        .expect_err("out-of-cardinality key must reject");
    assert!(error.0.contains("outside the schema cardinality"));
}
