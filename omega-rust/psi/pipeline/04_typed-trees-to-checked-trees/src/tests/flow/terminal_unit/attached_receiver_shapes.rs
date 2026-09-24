//! Which attached receiver shapes the composed Unit builder plans.
//!
//! The product lexer keeps `[Token; 16384]`, a fixed array of a copy sum
//! with payload cases and common fields, inside the entry receiver. Terminal
//! already spells that as `FixedArray` over `Sum`/`Mixed`, so the Unit type
//! builder admits it beside the copy-record and primitive element rules.
use super::CheckedUnitStructuralTypeShape;
use crate::tests::flow::terminal_unit::checked_with_service;
use crate::tests::flow::terminal_unit::machine_named;

fn entry_with_part(part: &str) -> String {
    format!(
        r#"
pub boundary trait Host {{
    machine exit(code: i32);
}}
{part}
pub data Root {{ host: Binding<Host>; part: Part; }}
machine Root::run(&mut self) reaches Host {{
    transition {{ _ -> done() }}
    state done(&mut self) {{ self.host.exit(0); }}
}}
"#
    )
}

/// The receiver plans as one composed Unit machine with no omission row.
fn composed_plan_count(part: &str) -> (usize, Option<checked_trees::CheckedUnitPlanOmissionStage>) {
    let checked = checked_with_service(&entry_with_part(part));
    let root = machine_named(&checked, "run");
    let plans = &checked.facts.flow.terminal_unit_effects;
    (
        plans
            .composed_machines
            .iter()
            .filter(|plan| plan.machine == root)
            .count(),
        plans.omission_for_machine(root).map(|row| row.stage),
    )
}

#[test]
fn material_receiver_shapes_plan_as_composed_unit_machines() {
    for (name, part) in [
        ("bool", "pub data Part { flag: bool; }"),
        ("ranged", "pub data Part { count: u64 [0..=16384]; }"),
        ("trapping", "pub data Part { count: u64 in Trapping; }"),
        ("bytes", "pub data Part { bytes: [u8; 65536] in Trapping; }"),
        (
            "copy sum",
            "pub data Kind [copy] { case A; case B; }\npub data Part { kind: Kind; }",
        ),
        (
            "copy payload sum",
            "pub data Kind [copy] { case A; case B(value: u64 in Trapping); }\npub data Part { kind: Kind; }",
        ),
        (
            "copy record array",
            "pub data Span [copy] { start: u64 in Trapping; end: u64 in Trapping; }\npub data Part { spans: [Span; 16] in Trapping; }",
        ),
        (
            "nested record",
            "pub data Inner { count: u64 in Trapping; }\npub data Part { inner: Inner; }",
        ),
        (
            "copy payload sum array",
            "pub data Kind [copy] { case A; case B(value: u64 in Trapping); }\npub data Part { kinds: [Kind; 16] in Trapping; }",
        ),
        (
            "copy mixed sum array",
            "pub data Kind [copy] { case A; case B; }\npub data Token [copy] { start: u64 in Trapping; end: u64 in Trapping; case Word; case Keyword(kind: Kind); }\npub data Part { tokens: [Token; 16] in Trapping; count: u64 [0..=16]; }",
        ),
    ] {
        assert_eq!(
            composed_plan_count(part),
            (1, None),
            "{name}: the attached receiver keeps its composed Unit plan"
        );
    }
}

#[test]
fn fixed_array_of_a_copy_sum_is_shaped_beneath_the_receiver() {
    let checked = checked_with_service(&entry_with_part(
        "pub data Kind [copy] { case A; case B(value: u64 in Trapping); }\npub data Part { kinds: [Kind; 16] in Trapping; }",
    ));
    let types = &checked.facts.flow.terminal_unit_effects.structural_types;
    let array = types
        .iter()
        .find_map(|plan| match &plan.shape {
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } if *length == 16 => Some(element_type_identity.clone()),
            _ => None,
        })
        .expect("the receiver's fixed array is a Unit structural type");
    assert!(
        types.iter().any(|plan| plan.identity == array
            && matches!(&plan.shape, CheckedUnitStructuralTypeShape::Sum { cases } if cases.len() == 2)),
        "the array element is the copy sum itself, not a projection of it"
    );
}

// Elements that the element rules never admitted stay outside the array
// shape: an affine sum with an owned payload is not a material element.
#[test]
fn fixed_array_of_an_affine_payload_sum_stays_unshaped() {
    assert!(matches!(
        composed_plan_count(
            "pub data Owned { count: u64 in Trapping; }\npub data Slot { case Empty; case Live(owned: Owned); }\npub data Part { slots: [Slot; 4] in Trapping; }"
        ),
        (
            0,
            Some(checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { .. })
        )
    ));
}
