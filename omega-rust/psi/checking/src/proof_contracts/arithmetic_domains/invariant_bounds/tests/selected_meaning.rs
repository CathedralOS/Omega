use super::super::OperatorSpelling;
use typed_trees::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceRowIdentity, MachineSpecialization,
};

#[test]
fn closed_primitive_range_bounds_do_not_erase_selected_trait_meaning() {
    let mut program = crate::front_end::typed_program(
        "trait SelectedArithmetic { operator + add(left: Self, right: Self) -> Self; }
         Chosen: u64 satisfies SelectedArithmetic {
             machine add(left: u64, right: u64) -> u64 { 100u64 }
         }
         machine arithmetic() -> u64 { 7u64 + 1 }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "arithmetic")
        .expect("closed arithmetic owner")
        .clone();
    let state = program.machine_states(&machine)[0].clone();
    let typed_trees::statement::StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("closed arithmetic expression");
    };
    assert_eq!(
        crate::closed_integer_range_bound(&program, expression).and_then(|value| value.to_u64()),
        Some(8),
    );
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
        .collect();
    // This query mutation establishes the competing selected meaning; it is
    // not a claim that the program carries checked publication evidence.
    program.machine_specializations.push(MachineSpecialization {
        template: machine.symbol,
        instance: machine.symbol,
        conformance_arguments: vec![conformance.symbol],
        conformance_applications: vec![ClosedConformanceApplication {
            declaration: conformance.symbol,
            subject_identity: Some(program.display_type_reference(state.return_type)),
            trait_definition: conformance.trait_symbol,
            rows,
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(
        typed_trees::operator::selected_trait_operator_meanings(
            &program,
            machine.symbol,
            OperatorSpelling::Add,
            &[Some(state.return_type), None],
        )
        .len(),
        1,
    );
    assert!(crate::closed_integer_range_bound(&program, expression).is_none());
}
