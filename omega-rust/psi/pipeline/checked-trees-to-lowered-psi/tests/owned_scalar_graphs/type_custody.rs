//! Consistent forged catalogs must still agree with the typed source layout.

use checked_trees::types::PrimitiveType;
use checked_trees::{CheckedTrees, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape};

use super::support;

fn fixture(declarations: &str) -> (CheckedTrees, String) {
    let source = format!(
        "{declarations}
        machine reset(value: &mut u64) -> u64 {{ value = 0; 0 }}
        machine enter(value: Envelope) -> u64 {{
            let mut scratch: u64 = 1;
            let cleared: u64 = reset(&mut scratch);
            value.limit
        }}"
    );
    let (checked, _, _, _) = support::publish(&source, "enter");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let identity = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .find(|graph| graph.machine == machine.symbol)
        .unwrap()
        .states[0]
        .structural_parameters[0]
        .type_identity
        .clone();
    (checked, identity)
}

fn nested_type(checked: &CheckedTrees, owner: &str, field_identity: &str) -> String {
    let plan = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .structural_types
        .iter()
        .find(|plan| plan.identity == owner)
        .unwrap();
    let CheckedUnitStructuralTypeShape::Record { fields } = &plan.shape else {
        panic!("record source")
    };
    let field = fields
        .iter()
        .find(|field| field.identity == field_identity)
        .unwrap();
    let CheckedUnitStructuralFieldType::Structural { type_identity } = &field.field_type else {
        panic!("nested source type")
    };
    type_identity.clone()
}

/// Change every retained copy so disagreement between catalog owners cannot
/// stand in for independent correspondence with the typed declaration.
fn corrupt(
    checked: &mut CheckedTrees,
    identity: &str,
    mut mutation: impl FnMut(&mut CheckedUnitStructuralTypeShape),
) {
    let flow = &mut checked.facts.flow;
    let mut changed = 0usize;
    for catalog in [
        &mut flow.terminal_scalar_graphs.structural_types,
        &mut flow.terminal_unit_effects.structural_types,
        &mut flow.terminal_structural_returns.structural_types,
        &mut flow.terminal_boundary_scalar_returns.structural_types,
        &mut flow.terminal_structural_scalar_returns.structural_types,
    ] {
        for plan in catalog.iter_mut().filter(|plan| plan.identity == identity) {
            mutation(&mut plan.shape);
            changed += 1;
        }
    }
    assert!(changed > 0, "mutated an existing selected type");
}

fn reject(checked: &CheckedTrees) {
    let error = terminal_production::produce_terminal_artifact(checked, "enter")
        .expect_err("source type custody must reject a consistently forged catalog");
    assert!(
        format!("{error:?}")
            .contains("owned structural catalog differs from its typed declaration"),
        "the source rejoin must witness this corruption: {error:?}"
    );
}

#[test]
fn consistent_field_order_name_type_count_and_relevance_corruption_reject() {
    let (original, identity) = fixture("data Envelope { limit: u64; spare: u64; }");
    for mutation in 0..5 {
        let mut checked = original.clone();
        corrupt(&mut checked, &identity, |shape| {
            let CheckedUnitStructuralTypeShape::Record { fields } = shape else {
                panic!("record")
            };
            match mutation {
                0 => fields.swap(0, 1),
                1 => fields[1].identity = "renamed".into(),
                2 => {
                    fields[1].field_type =
                        CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U32)
                }
                3 => {
                    fields.pop();
                }
                _ => fields[1].relevance = terminal_psi::BindingRelevance::Erased,
            }
        });
        reject(&checked);
    }
}

#[test]
fn numbered_field_identity_is_rejoined_even_when_the_field_is_unread() {
    let (mut checked, identity) = fixture("data Envelope { #7 limit: u64; #8 spare: u64; }");
    corrupt(&mut checked, &identity, |shape| {
        let CheckedUnitStructuralTypeShape::Record { fields } = shape else {
            panic!("record")
        };
        assert_eq!(fields[1].identity, "#8");
        fields[1].identity = "#9".into();
    });
    reject(&checked);
}

#[test]
fn nested_generic_and_array_source_shapes_rejoin_their_complete_closure() {
    let (original, identity) = fixture(
        "
        data Cell<T> { first: T; second: T; }
        data Child { first: u64; second: u64; }
        data Envelope { limit: u64; wrapped: Cell<u64>; children: [Child; 2]; }
    ",
    );
    let wrapped = nested_type(&original, &identity, "wrapped");
    let children = nested_type(&original, &identity, "children");
    let CheckedUnitStructuralTypeShape::FixedArray {
        element_type_identity,
        ..
    } = &original
        .facts
        .flow
        .terminal_scalar_graphs
        .structural_types
        .iter()
        .find(|plan| plan.identity == children)
        .unwrap()
        .shape
    else {
        panic!("array")
    };
    for identity in [&wrapped, element_type_identity] {
        let mut checked = original.clone();
        corrupt(&mut checked, identity, |shape| {
            let CheckedUnitStructuralTypeShape::Record { fields } = shape else {
                panic!("child record")
            };
            fields.swap(0, 1);
        });
        reject(&checked);
    }
    let mut checked = original.clone();
    corrupt(&mut checked, &children, |shape| {
        let CheckedUnitStructuralTypeShape::FixedArray { length, .. } = shape else {
            panic!("array")
        };
        *length += 1;
    });
    reject(&checked);
    let mut checked = original.clone();
    corrupt(&mut checked, &children, |shape| {
        let CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity,
            ..
        } = shape
        else {
            panic!("array")
        };
        *element_type_identity = wrapped.clone();
    });
    reject(&checked);
}

#[test]
fn sum_and_mixed_shapes_rejoin_case_order_identity_and_payloads() {
    let (original, identity) = fixture(
        "
        data Message { case #1 Empty; case #2 Data(#1 first: u64, #2 second: u64); }
        data Tagged { marker: u64; case Empty; case Data(first: u64, second: u64); }
        data Envelope { limit: u64; message: Message; tagged: Tagged; }
    ",
    );
    for field in ["message", "tagged"] {
        let identity = nested_type(&original, &identity, field);
        for mutation in 0..3 {
            let mut checked = original.clone();
            corrupt(&mut checked, &identity, |shape| {
                let cases = match shape {
                    CheckedUnitStructuralTypeShape::Sum { cases }
                    | CheckedUnitStructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => panic!("sum or mixed"),
                };
                match mutation {
                    0 => cases.swap(0, 1),
                    1 => cases[0].identity = "other_case".into(),
                    _ => cases[1].fields.swap(0, 1),
                }
            });
            reject(&checked);
        }
    }
}
