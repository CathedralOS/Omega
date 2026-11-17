//! Bodyless headers retain the qualifications generated for their exact formals.

use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionNode;

#[test]
fn a_bodyless_boundary_keeps_its_qualified_formal_in_the_complete_header() {
    let program = super::qualified_operator_home_tests::checked(&[(
        "header.omg",
        "domain u32::Tagged; boundary machine transfer(input: u32 in Tagged) -> u32 in Tagged;",
    )])
    .expect("an ordinary qualified boundary header types");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "transfer")
        .unwrap();
    assert!(!machine.body_is_present);
    let entries = program.machine_states(machine);
    let [entry] = entries.as_array().expect("one actual bodyless entry");
    assert!(entry.contracts.is_empty());
    let (owner, signature) = program
        .bodyless_declaration(machine.symbol)
        .unwrap()
        .unwrap();
    assert_eq!(owner, machine.symbol);
    assert_eq!(signature.symbol, machine.symbol);
    assert_eq!(signature.contracts, machine.contracts);
    let parameters = program.state_parameters.span(signature.parameters).unwrap();
    let [parameter] = parameters.as_array().expect("one actual source formal");
    let memberships = program
        .signature_contracts
        .span(signature.contracts)
        .unwrap()
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Membership(membership) => Some(membership),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(memberships.len(), 1);
    let ExpressionNode::Name(subject) = program.expression_table.expression(memberships[0].value)
    else {
        panic!("the original qualified input remains the membership subject");
    };
    assert_eq!(subject.symbol, parameter.symbol);
}

#[test]
fn a_bodyful_machine_keeps_its_qualified_formal_requirement_with_its_state() {
    let program = super::qualified_operator_home_tests::checked(&[(
        "body.omg",
        "domain u32::Tagged; machine value(input: u32 in Tagged) -> u32 in Tagged { input }",
    )])
    .expect("an ordinary qualified machine types");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    assert!(machine.body_is_present);
    assert!(
        program
            .machine_states(machine)
            .iter()
            .flat_map(|state| program.signature_contracts.span_or_empty(state.contracts))
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| matches!(fact, ProofFact::Membership(_)))
    );
    assert!(
        !program
            .signature_contracts
            .span_or_empty(machine.contracts)
            .iter()
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| matches!(fact, ProofFact::Membership(_)))
    );
}
