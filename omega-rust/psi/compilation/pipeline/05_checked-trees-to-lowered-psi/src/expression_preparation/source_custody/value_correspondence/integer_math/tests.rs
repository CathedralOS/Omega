use super::super::{ExpressionHandle, PrimitiveType};
use checked_trees::domain::ProofFact;
use checked_trees::signature::SignatureContractKind;
use checked_trees::{
    CheckedBooleanExpression, CheckedCallScalarArgument, CheckedIntegerMathTerm,
    CheckedScalarExpression, ClosedScalarContractValue,
};

fn fixture() -> (
    checked_trees::CheckedTrees,
    symbols::SymbolHandle,
    ExpressionHandle,
    CheckedBooleanExpression,
) {
    let checked = crate::front_end::checked_program(
        "machine read(input: u64, other: u64) -> u64
         ensures embed(result) == embed(input) + 0 { input }",
    );
    let machine = &checked.machines()[0];
    let state = checked.machine_states(machine)[0].symbol;
    let clause = checked
        .machine_contracts(machine)
        .iter()
        .find(|clause| clause.kind == SignatureContractKind::Ensures)
        .unwrap();
    let [ProofFact::Expression(source)] = checked.proof_facts.span_or_empty(clause.facts) else {
        panic!("one authored guarantee");
    };
    let Some(ClosedScalarContractValue::Predicate(predicate)) = &checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .unwrap()
        .closed_scalar_values
        .ensures()[0]
    else {
        panic!("mathematical guarantee retained");
    };
    let source = *source;
    let predicate = predicate.clone();
    (checked, state, source, predicate)
}

fn replay(
    checked: &checked_trees::CheckedTrees,
    state: symbols::SymbolHandle,
    source: ExpressionHandle,
    predicate: CheckedBooleanExpression,
) -> Result<(), crate::LoweringError> {
    let retained = CheckedScalarExpression::Boolean(Box::new(predicate));
    crate::expression_preparation::source_custody::validate_normal_result_read_expression(
        checked, state, source, &retained,
    )?;
    super::super::validate(
        checked,
        state,
        0,
        source,
        PrimitiveType::Bool,
        &CheckedCallScalarArgument::Pure(retained),
    )
}

#[test]
fn mathematical_predicate_source_replay_retains_operators_carriers_and_result_identity() {
    let (checked, state, source, predicate) = fixture();
    replay(&checked, state, source, predicate.clone()).unwrap();
    for mutation in 0..7 {
        let mut changed = predicate.clone();
        let CheckedBooleanExpression::IntegerMathComparison { left, right, .. } = &mut changed
        else {
            panic!("math comparison");
        };
        match mutation {
            0 => {
                let CheckedIntegerMathTerm::Embed { operand, .. } = left.terms.get_mut(left.root)
                else {
                    panic!("result embed");
                };
                *operand = CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type: PrimitiveType::U64,
                };
            }
            1 => {
                let CheckedIntegerMathTerm::Embed { primitive_type, .. } =
                    left.terms.get_mut(left.root)
                else {
                    panic!("result embed");
                };
                *primitive_type = PrimitiveType::U32;
            }
            2 => {
                let CheckedIntegerMathTerm::Add {
                    left,
                    right: literal,
                } = *right.terms.get(right.root)
                else {
                    panic!("math addition");
                };
                *right.terms.get_mut(right.root) = CheckedIntegerMathTerm::Multiply {
                    left,
                    right: literal,
                };
            }
            3 => {
                let CheckedIntegerMathTerm::Add { right: literal, .. } =
                    *right.terms.get(right.root)
                else {
                    panic!("math addition");
                };
                *right.terms.get_mut(literal) = CheckedIntegerMathTerm::Literal(
                    semantic_vocabulary::IntegerMathLiteral::new(false, 1).unwrap(),
                );
            }
            4 => right.root = Default::default(),
            5 => {
                *right.terms.get_mut(right.root) = CheckedIntegerMathTerm::Add {
                    left: right.root,
                    right: right.root,
                };
            }
            _ => {
                let CheckedIntegerMathTerm::Add { left: input, .. } = *right.terms.get(right.root)
                else {
                    panic!("math addition");
                };
                let CheckedIntegerMathTerm::Embed { operand, .. } = right.terms.get_mut(input)
                else {
                    panic!("input embed");
                };
                *operand = CheckedScalarExpression::Parameter {
                    position: 1,
                    primitive_type: PrimitiveType::U64,
                };
            }
        }
        assert!(
            replay(&checked, state, source, changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn mathematical_predicate_source_replay_rejects_a_result_from_another_contract() {
    let (mut checked, state, source, predicate) = fixture();
    let result = checked
        .expression_table
        .iter_expressions()
        .find_map(|(handle, _)| {
            validation::reserved_result_owner(&checked.typed, handle).map(|_| handle)
        })
        .unwrap();
    let checked_trees::expression::ExpressionNode::Name(mut path) =
        *checked.expression_table.expression(result)
    else {
        panic!("reserved result source");
    };
    // This source name no longer denotes this machine's reserved result.
    path.symbol = state;
    path.head_symbol = state;
    *checked.typed.expression_table.expression_mut(result) =
        checked_trees::expression::ExpressionNode::Name(path);
    assert!(replay(&checked, state, source, predicate).is_err());
}

#[test]
fn mathematical_predicate_source_replay_keeps_inequality_polarity() {
    let (mut checked, state, source, predicate) = fixture();
    let checked_trees::expression::ExpressionNode::Binary(binary) =
        checked.typed.expression_table.expression_mut(source)
    else {
        panic!("authored equality");
    };
    binary.operator = checked_trees::expression::BinaryOperator::NotEqual;
    // This is correspondence only: the changed source still owes its proof.
    // != must retain the explicit Boolean negation of mathematical equality.
    replay(
        &checked,
        state,
        source,
        CheckedBooleanExpression::Not(Box::new(predicate.clone())),
    )
    .unwrap();
    assert!(replay(&checked, state, source, predicate).is_err());
}
