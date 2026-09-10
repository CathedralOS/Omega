//! Arithmetic-policy storage keeps the complete source type while projecting
//! only its integer payload shape. Other qualifications cannot borrow this gate.

use super::*;

fn field_program(spelling: &str) -> (TypedTrees, TypeReferenceHandle) {
    let source = format!("data Other {{ value: u64; }} data Carrier {{ values: {spelling}; }}");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let carrier = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Carrier")
        .unwrap();
    let [DataMember::Field(field)] = program.data_members(carrier) else {
        panic!("one array field");
    };
    let reference = field.type_reference;
    (program, reference)
}

#[test]
fn policy_only_integer_array_shapes_preserve_complete_array_identity() {
    for spelling in [
        "[u64 in Wrapping; 16]",
        "[i32 in Saturating; 3]",
        "[u8 in Trapping; 2]",
        "[[u16 in Wrapping; 2]; 3]",
    ] {
        let (program, reference) = field_program(spelling);
        let mut collector = ShapeCollector::new(&program);
        let identity = collector.add_type(reference, &[], &[]).expect(spelling);
        assert_eq!(
            identity,
            program
                .normalized_type_identity_with_binders_and_substitutions(reference, &[], &[])
                .into_string()
        );
        let shape = &collector.types[&identity].shape;
        assert!(matches!(
            shape,
            CheckedUnitStructuralTypeShape::FixedArray { .. }
        ));
    }
    for spelling in ["[u64 [0..=15]; 16]", "[u64 [0..=15] in Wrapping; 16]"] {
        let (program, reference) = field_program(spelling);
        assert!(
            ShapeCollector::new(&program)
                .add_type(reference, &[], &[])
                .is_none(),
            "{spelling}"
        );
    }
}

#[test]
fn policy_element_requires_live_exact_builtin_carrier_and_complete_singleton_constraint() {
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::types::TypeConstraintNode;
    let (program, array) = field_program("[u64 in Wrapping; 16]");
    let TypeReferenceNode::FixedArray {
        element_type: element,
        ..
    } = *program.type_reference_table.type_reference(array)
    else {
        panic!("array");
    };
    let TypeReferenceNode::Constrained {
        base_type: base,
        constraints,
    } = *program.type_reference_table.type_reference(element)
    else {
        panic!("qualified element");
    };
    let other = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .unwrap()
        .symbol;
    for corruption in 0..8 {
        let mut changed = program.clone();
        match corruption {
            0 => changed.type_reference_table.substitute_node(
                element,
                TypeReferenceNode::Constrained {
                    base_type: TypeReferenceHandle::from_parts(
                        base.arena_index(),
                        base.generation() + 1,
                    ),
                    constraints,
                },
            ),
            1 => changed.type_reference_table.substitute_node(
                element,
                TypeReferenceNode::Constrained {
                    base_type: element,
                    constraints,
                },
            ),
            2 => {
                let duplicate = changed.type_reference_table.insert_constraints([
                    TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Wrapping),
                    TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Wrapping),
                ]);
                changed.type_reference_table.substitute_node(
                    element,
                    TypeReferenceNode::Constrained {
                        base_type: base,
                        constraints: duplicate,
                    },
                );
            }
            3 => changed.type_reference_table.substitute_node(
                element,
                TypeReferenceNode::Constrained {
                    base_type: base,
                    constraints: arena::HandleSpan::empty(),
                },
            ),
            4 => {
                let TypeReferenceNode::Named { name, .. } =
                    changed.type_reference_table.type_reference(base).clone()
                else {
                    panic!("builtin");
                };
                changed.type_reference_table.substitute_node(
                    base,
                    TypeReferenceNode::Named {
                        symbol: other,
                        name,
                    },
                );
            }
            5 => {
                let TypeReferenceNode::Named { symbol, .. } =
                    changed.type_reference_table.type_reference(base).clone()
                else {
                    panic!("builtin");
                };
                changed.type_reference_table.substitute_node(
                    base,
                    TypeReferenceNode::Named {
                        symbol,
                        name: typed_trees::name::Identifier::generated("f64"),
                    },
                );
            }
            6 => {
                let named =
                    changed
                        .type_reference_table
                        .insert_constraints([TypeConstraintNode::Named(
                            typed_trees::name::Identifier::generated("Tag"),
                        )]);
                changed.type_reference_table.substitute_node(
                    element,
                    TypeReferenceNode::Constrained {
                        base_type: base,
                        constraints: named,
                    },
                );
            }
            7 => changed.type_reference_table.substitute_node(
                element,
                TypeReferenceNode::Constrained {
                    base_type: base,
                    constraints: arena::HandleSpan::from_parts(
                        arena::Handle::from_parts(
                            constraints.start().arena_index(),
                            constraints.start().generation() + 1,
                        ),
                        constraints.count(),
                    ),
                },
            ),
            _ => unreachable!(),
        }
        assert!(
            !ShapeCollector::new(&changed).is_unrestricted_nonatomic_primitive(element),
            "corruption {corruption}"
        );
    }
}
