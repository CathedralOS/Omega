//! Consistent forged catalogs must still agree with the typed source layout.

use checked_trees::{CheckedTrees, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape};
use semantic_vocabulary::IntegerValue;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

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
    let error = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect_err("source type custody must reject a consistently forged catalog")
    .into_parts()
    .0;
    assert!(
        format!("{error:?}")
            .contains("owned structural catalog differs from its typed declaration"),
        "the source rejoin must witness this corruption: {error:?}"
    );
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

#[test]
fn bounded_fields_preserve_signed_unsigned_expression_and_substituted_ranges() {
    for (declarations, owner_field, minimum, maximum) in [
        (
            "data Envelope { limit: u64; spare: i8 [-128..=-1]; }",
            None,
            IntegerValue::Signed(-128),
            IntegerValue::Signed(-1),
        ),
        (
            "data Envelope { limit: u64; spare: u64 [3..=5]; }",
            None,
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
        ),
        (
            "data Envelope { limit: u64; spare: u64 [0..18446744073709551616]; }",
            None,
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(u128::from(u64::MAX)),
        ),
        (
            "data Envelope { limit: u64; spare: u64 [18446744073709551615..=18446744073709551615]; }",
            None,
            IntegerValue::Unsigned(u128::from(u64::MAX)),
            IntegerValue::Unsigned(u128::from(u64::MAX)),
        ),
        (
            "data Cell<T> { spare: T; }
             data Envelope { limit: u64; wrapped: Cell<u64 [0..18446744073709551616]>; }",
            Some("wrapped"),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(u128::from(u64::MAX)),
        ),
        (
            "data Envelope { limit: u64; spare: i16 [0 - 3..=10 * 2]; }",
            None,
            IntegerValue::Signed(-3),
            IntegerValue::Signed(20),
        ),
        (
            "data Envelope { limit: u64; spare: u8 [0..=300]; }",
            None,
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(255),
        ),
        (
            "data Cell<T> { spare: T; }
             data Envelope { limit: u64; wrapped: Cell<i32 [12..=100]>; }",
            Some("wrapped"),
            IntegerValue::Signed(12),
            IntegerValue::Signed(100),
        ),
    ] {
        let (checked, identity) = fixture(declarations);
        let identity = owner_field
            .map(|field| nested_type(&checked, &identity, field))
            .unwrap_or(identity);
        let plan = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter()
            .find(|plan| plan.identity == identity)
            .unwrap();
        let CheckedUnitStructuralTypeShape::Record { fields } = &plan.shape else {
            panic!("bounded source record")
        };
        let field = fields
            .iter()
            .find(|field| field.identity == "spare")
            .unwrap();
        let CheckedUnitStructuralFieldType::BoundedInteger(integer) = field.field_type else {
            panic!("source range must survive publication: {declarations}")
        };
        assert_eq!((integer.minimum(), integer.maximum()), (minimum, maximum));
    }
}

#[test]
fn nested_source_range_shells_reconstruct_their_intersection() {
    use checked_trees::data::DataMember;
    use checked_trees::types::TypeReferenceNode;

    let (mut checked, _) = fixture(
        "data Envelope { limit: u64; spare: i32 [12..=100]; }
         data Wider { value: i32 [0..=255]; }",
    );
    let wider = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Wider")
        .unwrap();
    let DataMember::Field(field) = &checked.data_members(wider)[0] else {
        panic!("wider source field")
    };
    let TypeReferenceNode::Constrained { constraints, .. } = *checked
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("wider source range shell")
    };
    let (handle, reference) = checked
        .typed
        .data_members
        .iter()
        .find_map(|(handle, member)| match member {
            DataMember::Field(field) if field.name.as_str() == "spare" => {
                Some((handle, field.type_reference))
            }
            _ => None,
        })
        .unwrap();
    // Source syntax has one range slot; typed substitution can retain nested
    // shells. The wider shell must not replace the narrower source restriction.
    let reference = checked
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: reference,
            constraints,
        });
    let DataMember::Field(field) = checked.typed.data_members.get_mut(handle) else {
        panic!("bounded source field")
    };
    field.type_reference = reference;
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("equivalent intersected source ranges preserve the retained catalog")
    .into_artifact();
}
