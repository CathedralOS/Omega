//! Ensures-clause shape queries over a called signature's contracts: which
//! fact expressions carry the clause, and how a `parameter <OP> <literal>`
//! conjunct resolves against one call's arguments. Both the ranges walk in
//! typed-to-checked preparation and the proof obligation collector run this
//! same recognition, so the shape lives here once.

use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};

/// Every `ProofFact::Expression` under `signature`'s `ensures` contracts, in
/// declaration order.
pub fn ensures_contract_expressions<'program>(
    program: &'program dyn crate::CheckingInput,
    signature: &'program StateSignature,
) -> impl Iterator<Item = ExpressionHandle> + 'program {
    program
        .signature_contracts
        .span_or_empty(signature.contracts)
        .iter()
        .filter(|contract| matches!(contract.kind, SignatureContractKind::Ensures))
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
}

/// Collect each `parameter <OP> <literal>` comparison inside `conjunct`,
/// descending `And` nodes, as `(place, operator, literal)` rows: `place` is
/// the display name of the `&mut` argument bound to the signature parameter
/// named on the comparison's left, `literal` the integer on its right.
/// `parameters` are the called signature's state parameters and `arguments`
/// the call's argument expressions, so row position follows the signature.
pub fn collect_parameter_bound_comparisons(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    conjunct: ExpressionHandle,
    comparisons: &mut Vec<(String, BinaryOperator, i64)>,
) {
    let Some(comparison) = program.expression_table.binary_node(conjunct) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        collect_parameter_bound_comparisons(
            program,
            parameters,
            arguments,
            comparison.left,
            comparisons,
        );
        collect_parameter_bound_comparisons(
            program,
            parameters,
            arguments,
            comparison.right,
            comparisons,
        );
        return;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(comparison.left) else {
        return;
    };
    let [parameter_name] = program.expression_table.name_path_members(path.members) else {
        return;
    };
    let ExpressionNode::Integer(literal) = program.expression_table.expression(comparison.right)
    else {
        return;
    };
    let Some(value) = literal.value_i64() else {
        return;
    };
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.name.as_str() == parameter_name.as_str())
    else {
        return;
    };
    let Some(argument) = arguments.get(position).copied() else {
        return;
    };
    let Some(place) = program.expression_table.borrow_expression(argument) else {
        return;
    };
    comparisons.push((
        program.expression_table.display_name(place.target),
        comparison.operator,
        value,
    ));
}
