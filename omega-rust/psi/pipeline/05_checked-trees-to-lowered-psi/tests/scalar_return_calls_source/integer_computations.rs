use super::checked_arms;
use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn integer_application_operand_namespaces_reject_malformed_templates() {
    use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpression};
    use typed_trees::types::PrimitiveType;
    let source = r#"
        machine identity(input: u8 in Wrapping) -> u8 in Wrapping
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        { input }
        machine value(selected: bool, input: u8 in Wrapping) -> bool
        requires true == true
        ensures true == true
        {
            transition selected {
                true -> finish(selected && ((identity(input) + 1u8) == 0u8))
                false -> finish(false)
            }
            state finish(result: bool) -> bool { result }
        }
    "#;
    for combined in [false, true] {
        let checked = checked_arms(source, combined);
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("value"),
        )
        .unwrap();
        let (handle, _) = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .iter()
            .find(|(_, node)| {
                matches!(
                    &node.kind,
                    CheckedScalarComputationKind::Apply {
                        expression: CheckedScalarExpression::IntegerBinary { .. },
                        ..
                    }
                )
            })
            .unwrap();
        for mutation in 0..4 {
            let mut mutated = checked.clone();
            let node = mutated
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(handle);
            let CheckedScalarComputationKind::Apply {
                expression,
                operands,
                ..
            } = &mut node.kind
            else {
                unreachable!()
            };
            let CheckedScalarExpression::IntegerBinary { left, .. } = expression else {
                unreachable!()
            };
            match mutation {
                0 => {
                    **left = CheckedScalarExpression::Parameter {
                        position: 2,
                        primitive_type: PrimitiveType::U8,
                    }
                }
                1 => {
                    **left = CheckedScalarExpression::Parameter {
                        position: 0,
                        primitive_type: PrimitiveType::U16,
                    }
                }
                2 => *operands = arena::HandleSpan::empty(),
                3 => node.primitive_type = PrimitiveType::U16,
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &mutated,
                    TerminalMachineSelection::Name("value")
                )
                .is_err(),
                "mutation={mutation}, combined={combined}"
            );
        }
    }
}
