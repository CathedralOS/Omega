//! Computed field assignments retain their authored scalar call operands.

use checked_trees::CheckedScalarComputationKind;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[test]
fn computed_field_rhs_rejects_same_typed_call_operand_substitution() {
    let source = r#"
        data Main { first: u16; second: u16; }
        machine identity(value: u16) -> u16 { value }
        machine Main::main(&mut self, first: u16, second: u16) {
            self.first = identity(first);
            self.second = identity(second);
        }
    "#;
    let mut checked = crate::front_end::checked_program(source);
    let _ = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("unmodified field RHS operands publish")
    .into_artifact();

    let plans = &mut checked.facts.values.scalar_computations;
    let calls = plans
        .nodes
        .iter()
        .filter_map(|(handle, node)| {
            let CheckedScalarComputationKind::Call { arguments, .. } = &node.kind else {
                return None;
            };
            Some((handle, *arguments))
        })
        .collect::<Vec<_>>();
    let [(first_call, first_arguments), (_, second_arguments)] = calls.as_slice() else {
        panic!("fixture retains exactly two scalar calls")
    };
    let [first_operand] = plans.operands.span(*first_arguments).unwrap() else {
        panic!("first identity call has one operand")
    };
    let [second_operand] = plans.operands.span(*second_arguments).unwrap() else {
        panic!("second identity call has one operand")
    };
    assert_ne!(first_operand, second_operand);
    assert_eq!(
        plans.nodes.get(*first_operand).primitive_type,
        plans.nodes.get(*second_operand).primitive_type
    );

    // Substitute only the one-operand roster. Callee, source occurrence,
    // root, authored expression, result type, and destination remain unchanged.
    let CheckedScalarComputationKind::Call { arguments, .. } =
        &mut plans.nodes.get_mut(*first_call).kind
    else {
        unreachable!()
    };
    *arguments = *second_arguments;
    let result = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .map(|produced| produced.into_artifact());
    assert!(
        result.is_err(),
        "same-typed operand substitution must reject"
    );
}
