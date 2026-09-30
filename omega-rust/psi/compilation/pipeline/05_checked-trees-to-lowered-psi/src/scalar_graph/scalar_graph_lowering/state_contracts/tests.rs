use super::replay;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarStateGraph, CheckedTrees,
    ClosedScalarContractValue, ClosedScalarValueContractPlan,
};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::LandedIntegerType;

const STATE_REQUIRES: &str = r#"
    machine start(value: u32) -> u32 {
        transition { _ -> finish(value % 10) }
        state finish(pending: u32) requires pending <= 9 { pending }
    }
"#;

#[test]
fn state_requirements_replay_rejects_missing_and_forged_predicates() {
    let checked = crate::front_end::checked_program(STATE_REQUIRES);
    let (dispatch, state) = state_row(&checked, "start", 1);
    assert!(replay(&checked, dispatch, state).is_ok());
    for mutation in 0..6 {
        let mut changed = state.clone();
        mutate_requirements(&mut changed.requires, mutation);
        assert!(
            replay(&checked, dispatch, &changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn foreign_entry_requirements_replay_rejects_missing_and_forged_predicates() {
    let checked = crate::front_end::checked_program(
        r#"
        machine start(value: u32) -> u32 {
            transition { _ -> bounded(value % 10) }
        }
        machine bounded(pending: u32) -> u32 requires pending <= 9 { pending }
    "#,
    );
    let dispatch = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "start")
        .unwrap()
        .symbol;
    let (owner, state) = state_row(&checked, "bounded", 0);
    let state = state.clone();
    assert_eq!(replay(&checked, dispatch, &state).unwrap().len(), 1);
    for mutation in 0..6 {
        let mut changed = checked.clone();
        let contract = &mut changed
            .facts
            .contract_plans
            .machines
            .iter_mut()
            .find(|plan| plan.machine == owner)
            .unwrap()
            .closed_scalar_values;
        let mut requires = contract.requires().to_vec();
        mutate_requirements(&mut requires, mutation);
        *contract = ClosedScalarValueContractPlan::new(
            requires,
            contract.ensures().to_vec(),
            contract.has_crash_clauses(),
            contract.has_outcome_specific_clauses(),
        )
        .with_float_entry_ranges(contract.float_entry_ranges().map(<[_]>::to_vec))
        .with_integer_entry_ranges(contract.integer_entry_ranges().map(<[_]>::to_vec));
        assert!(
            replay(&changed, dispatch, &state).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn state_requirement_literal_replay_preserves_authored_nonexact_landing() {
    use checked_trees::domain::ProofFact;
    use checked_trees::expression::ExpressionNode;

    let mut checked = crate::front_end::checked_program(STATE_REQUIRES);
    let (dispatch, state) = state_row(&checked, "start", 1);
    let mut state = state.clone();
    let (_, source) =
        crate::expression_preparation::source_custody::authored_state(&checked, state.state)
            .unwrap();
    let clause = &checked.state_contracts(source)[0];
    let [ProofFact::Expression(expression)] = checked.proof_facts.span(clause.facts).unwrap()
    else {
        panic!("one authored expression")
    };
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(*expression) else {
        panic!("comparison")
    };
    let literal_source = binary.right;
    let literal = upper_bound(&mut state.requires);
    let mut landing = literal.landing().unwrap();
    landing.domain = ArithmeticDomain::Wrapping;
    *literal = literal.clone().with_landing(landing);
    let ExpressionNode::Integer(authored) = checked
        .typed
        .expression_table
        .expression_mut(literal_source)
    else {
        panic!("authored literal")
    };
    *authored = authored.clone().with_landing(landing);
    assert!(
        replay(&checked, dispatch, &state).is_ok(),
        "preserve source landing, not unconditional Exact"
    );
    let literal = upper_bound(&mut state.requires);
    landing.domain = ArithmeticDomain::Exact;
    *literal = literal.clone().with_landing(landing);
    assert!(replay(&checked, dispatch, &state).is_err());
}

fn state_row<'a>(
    checked: &'a CheckedTrees,
    name: &str,
    position: usize,
) -> (symbols::SymbolHandle, &'a CheckedScalarStateGraph) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("source machine")
        .symbol;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .expect("scalar graph");
    (machine, &graph.states[position])
}

fn mutate_requirements(clauses: &mut Vec<Option<ClosedScalarContractValue>>, mutation: usize) {
    match mutation {
        0 => clauses.clear(),
        1 => clauses[0] = None,
        2 => {
            clauses[0] = Some(ClosedScalarContractValue::Predicate(
                CheckedBooleanExpression::Constant(true),
            ))
        }
        3 => clauses.push(clauses[0].clone()),
        4 | 5 => {
            let literal = upper_bound(clauses);
            let mut landing = literal.landing().expect("landed comparison bound");
            if mutation == 4 {
                landing.landed_type = LandedIntegerType::U64;
            } else {
                landing.domain = ArithmeticDomain::Wrapping;
            }
            *literal = literal.clone().with_landing(landing);
        }
        _ => unreachable!(),
    }
}

fn upper_bound(
    clauses: &mut [Option<ClosedScalarContractValue>],
) -> &mut numerics::literals::IntegerLiteral {
    let Some(ClosedScalarContractValue::Predicate(CheckedBooleanExpression::IntegerComparison {
        right,
        ..
    })) = &mut clauses[0]
    else {
        panic!("retained comparison")
    };
    let CheckedScalarExpression::IntegerLiteral { literal } = right.as_mut() else {
        panic!("retained literal")
    };
    literal
}
