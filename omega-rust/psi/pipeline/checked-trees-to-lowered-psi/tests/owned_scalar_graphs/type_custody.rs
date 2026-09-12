//! Consistent forged catalogs must still agree with the typed source layout.

use checked_trees::types::PrimitiveType;
use checked_trees::{CheckedTrees, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape};
use semantic_vocabulary::{BoundedIntegerType, IntegerSign, IntegerType, IntegerValue};

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
    let error = terminal_production::TerminalProductionRequest::new(checked, "enter")
        .produce_artifact()
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
fn consistently_forged_bounded_fields_reject_range_erasure_and_carrier_changes() {
    let (original, identity) = fixture("data Envelope { limit: u64; spare: u64 [3..=5]; }");
    for replacement in [
        CheckedUnitStructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                IntegerValue::Unsigned(2),
                IntegerValue::Unsigned(6),
            )
            .unwrap(),
        ),
        CheckedUnitStructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                IntegerValue::Unsigned(4),
                IntegerValue::Unsigned(4),
            )
            .unwrap(),
        ),
        CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64),
        CheckedUnitStructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                IntegerValue::Unsigned(3),
                IntegerValue::Unsigned(5),
            )
            .unwrap(),
        ),
        CheckedUnitStructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                IntegerValue::Signed(3),
                IntegerValue::Signed(5),
            )
            .unwrap(),
        ),
    ] {
        let mut checked = original.clone();
        corrupt(&mut checked, &identity, |shape| {
            let CheckedUnitStructuralTypeShape::Record { fields } = shape else {
                panic!("bounded source record")
            };
            fields[1].field_type = replacement.clone();
        });
        reject(&checked);
    }
}

#[test]
fn source_range_drift_rejects_an_unchanged_bounded_catalog() {
    use checked_trees::data::DataMember;
    use checked_trees::expression::ExpressionNode;
    use checked_trees::types::{TypeConstraintNode, TypeReferenceNode};

    let (mut checked, _) = fixture("data Envelope { limit: u64; spare: u64 [3..=5]; }");
    let envelope = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Envelope")
        .unwrap();
    let DataMember::Field(field) = &checked.data_members(envelope)[1] else {
        panic!("bounded source field")
    };
    let TypeReferenceNode::Constrained { constraints, .. } = checked
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("source range shell")
    };
    let maximum = checked
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .find_map(|constraint| match constraint {
            TypeConstraintNode::Range { maximum, .. } => Some(*maximum),
            _ => None,
        })
        .unwrap();
    let ExpressionNode::Integer(value) = checked.typed.expression_table.expression_mut(maximum)
    else {
        panic!("literal source upper bound")
    };
    *value = numerics::literals::IntegerLiteral::from_value(6);
    reject(&checked);
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
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("equivalent intersected source ranges preserve the retained catalog");
}
