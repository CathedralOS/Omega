use super::*;
use checked_trees::{CheckFacts, ContractProofFact, ContractProofFactOwner};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{CallExpression, Expression, ExpressionNode},
    machine::Machine,
    name::Identifier,
    state::State,
};

#[test]
fn contract_owner_queries_keep_lazy_setup_and_exact_fact_namespaces() {
    let mut symbols = SymbolTableBuilder::default();
    let machine_symbol = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Caller"));
    let state_symbol = symbols
        .insert_children(
            machine_symbol,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        )
        .handle_at(0);
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let boolean = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let call = program
        .expression_table
        .insert_tree(&Expression::Call(Box::new(CallExpression {
            authored_call_selection: None,
            authored_call_binding: None,
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver: None,
            target_symbol: state_symbol,
            target: Identifier::from("entry"),
            arguments: Vec::new().into(),
            evidence_arguments: Vec::new().into(),
            operational_acknowledgement: Default::default(),
        })));
    let mut machine = Machine {
        symbol: machine_symbol,
        ..Default::default()
    };
    program.push_machine_state(
        &mut machine,
        State {
            symbol: state_symbol,
            name: Identifier::from("entry"),
            ..Default::default()
        },
    );
    program.push_machine(machine);
    let proof = program.proof_facts.append(ProofFact::Expression(call));
    let mut facts = CheckFacts::default();
    facts.proof.contract_facts.append(ContractProofFact {
        owner: ContractProofFactOwner::Machine { machine_symbol },
        fact: proof,
        ..Default::default()
    });
    let queries = ContractOwnerQueries::new(&program, &facts);
    assert!(queries.owners.get().is_none());
    assert_eq!(queries.derive_checked_nominal_call_target(boolean), None);
    assert!(queries.owners.get().is_none());
    assert_eq!(
        queries.derive_checked_nominal_call_target(call),
        Some(state_symbol)
    );
    let prepared = std::ptr::from_ref(queries.owners.get().unwrap());
    for _ in 0..3 {
        assert_eq!(
            queries.derive_checked_nominal_call_target(call),
            typed_trees_to_checked_trees::derive_checked_nominal_call_target(
                &program, &facts, call, None
            )
        );
        assert_eq!(std::ptr::from_ref(queries.owners.get().unwrap()), prepared);
    }
    // The same typed handles with a different supplied contract arena have no
    // checked owner. No program-only or another request's facts may fill it in.
    let other_facts = CheckFacts::default();
    let other = ContractOwnerQueries::new(&program, &other_facts);
    assert!(other.owners.get().is_none());
    assert_eq!(other.derive_checked_nominal_call_target(call), None);
    assert_eq!(
        queries.derive_checked_nominal_call_target(call),
        Some(state_symbol)
    );
}
