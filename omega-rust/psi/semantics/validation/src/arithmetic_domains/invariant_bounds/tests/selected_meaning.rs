use super::*;
use typed_trees::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceRowIdentity, MachineSpecialization,
};

#[test]
fn nested_arithmetic_retains_selected_operator_and_comparison_meaning() {
    let source = r#"
        trait SelectedArithmetic {
            operator + add(left: Self, right: Self) -> Self;
            operator < before(left: Self, right: Self) -> bool;
        }
        Chosen: u64 satisfies SelectedArithmetic {
            machine add(left: u64, right: u64) -> u64 { 100u64 }
            machine before(left: u64, right: u64) -> bool { false }
        }
        machine arithmetic(input: u64) -> u64 { (input % 5) + 1 }
        machine comparison(input: u64) -> bool { (input % 5) < 3 }
    "#;
    for selected in [false, true] {
        let mut program = typed(source);
        let conformance = program.conformances()[0].clone();
        let rows = program
            .closed_conformance_rows(&conformance)
            .expect("closed selected conformance")
            .iter()
            .map(|row| ClosedConformanceRowIdentity {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
            })
            .collect::<Vec<_>>();
        for (name, spelling) in [
            ("arithmetic", OperatorSpelling::Add),
            ("comparison", OperatorSpelling::Less),
        ] {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .expect("query machine")
                .clone();
            let state = program.machine_states(&machine)[0].clone();
            let input_type = program.state_parameters(&state)[0].type_reference;
            if selected {
                // Query-level selected application, not publication evidence:
                // this lookup reads the subject and rows, not commitments.
                let application = ClosedConformanceApplication {
                    declaration: conformance.symbol,
                    subject_identity: Some(program.display_type_reference(input_type)),
                    trait_definition: conformance.trait_symbol,
                    rows: rows.clone(),
                    ..Default::default()
                };
                program.machine_specializations.push(MachineSpecialization {
                    template: machine.symbol,
                    instance: machine.symbol,
                    conformance_arguments: vec![conformance.symbol],
                    conformance_applications: vec![application],
                    ..Default::default()
                });
            }
            assert_eq!(
                typed_trees::operator::selected_trait_operator_meanings(
                    &program,
                    machine.symbol,
                    spelling,
                    &[Some(input_type), None],
                )
                .len(),
                usize::from(selected),
                "the fixture must select exactly the requested meaning"
            );
            let typed_trees::statement::StatementNode::Expression(expression) =
                program.statement_table.statements(state.statement_nodes)[0]
            else {
                panic!("computed expression");
            };
            if spelling == OperatorSpelling::Add {
                assert_eq!(
                    immutable_integer_expression_bounds(&program, &machine, &state, expression),
                    (!selected).then_some((1, 5)),
                );
            } else {
                assert_eq!(
                    builtin_comparison_intervals(&program, &machine, &state, expression),
                    (!selected).then_some((
                        Interval {
                            low: Some(0),
                            high: Some(4)
                        },
                        Interval::constant(3),
                    )),
                );
            }
        }
    }
}
