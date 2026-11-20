//! The resumed entry asks for facts only about the declarations actually carried
//! at this cut. These are ordinary argument requirements in the published program.

use super::{Cut, local, parameter, read};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::statement::StatementHandle;

pub(super) fn require(
    syntax: &mut SyntaxTrees,
    machine: &mut syntax_trees::item::Machine,
    statements: &[StatementHandle],
    homes: &[usize],
    incoming: &Cut,
    facts: &[crate::ReturnedScalarFact],
) -> Result<(), Vec<Diagnostic>> {
    use lowered_program_to_checked_program::checking::{ReturnedComparison, ReturnedOperand};
    use semantic_vocabulary::IntegerValue;
    use syntax_trees::expression::{BinaryOperator, TableBinaryExpression};
    use syntax_trees::item::{CapabilityContract, CapabilityContractKind, ProofFact};
    if facts.is_empty() || incoming.bindings.is_empty() {
        return Ok(());
    }
    let mut contracts = syntax
        .items
        .capability_contracts(machine.contracts)
        .to_vec();
    for fact in facts {
        let mut span = None;
        let mut expression_for = |operand| -> Result<_, Vec<Diagnostic>> {
            let node = match operand {
                ReturnedOperand::Parameter(position) => {
                    let home = position
                        .checked_sub(1)
                        .and_then(|i| homes.get(i as usize))
                        .copied()
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "returned fact has no matching Build frame home",
                            )]
                        })?;
                    if !incoming.bindings.iter().any(|binding| binding.home == home) {
                        return Ok(None);
                    }
                    let name = parameter(local(syntax, statements[home]));
                    span = Some(name.source_span());
                    return Ok(Some(read(syntax, name, false)));
                }
                ReturnedOperand::Boolean(value) => ExpressionNode::Boolean(value),
                ReturnedOperand::Integer(value) => {
                    let (negative, magnitude) = match value {
                        IntegerValue::Signed(n) => (n < 0, n.unsigned_abs()),
                        IntegerValue::Unsigned(n) => (false, n),
                    };
                    ExpressionNode::Integer(
                        numeric_literals::IntegerLiteral::from_parts(
                            negative,
                            numeric_literals::IntegerRadix::Decimal,
                            &magnitude.to_string(),
                        )
                        .expect("integer magnitude has decimal digits"),
                    )
                }
            };
            Ok(Some(syntax.expressions.insert(node)))
        };
        let [a, b] = fact.operands();
        let Some(left) = expression_for(a)? else {
            continue;
        };
        let Some(right) = expression_for(b)? else {
            continue;
        };
        let span = span.expect("a returned fact names a frame parameter");
        for (node, operand) in [(left, a), (right, b)] {
            if !matches!(operand, ReturnedOperand::Parameter(_)) {
                syntax.expressions.set_source_span(node, span);
            }
        }
        let operator = match fact.comparison() {
            ReturnedComparison::Equal => BinaryOperator::Equal,
            ReturnedComparison::LessThan => BinaryOperator::Less,
            ReturnedComparison::LessOrEqual => BinaryOperator::LessOrEqual,
        };
        let expression = syntax
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator,
                right,
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
            }));
        syntax.expressions.set_source_span(expression, span);
        let fact = syntax
            .items
            .append_proof_fact(ProofFact::Expression(expression));
        contracts.push(CapabilityContract {
            kind: CapabilityContractKind::Requires,
            facts: HandleSpan::from_parts(fact, 1),
            ..Default::default()
        });
    }
    machine.contracts = HandleSpan::empty();
    for contract in contracts {
        machine
            .contracts
            .push_contiguous(syntax.items.append_capability_contract(contract));
    }
    Ok(())
}
