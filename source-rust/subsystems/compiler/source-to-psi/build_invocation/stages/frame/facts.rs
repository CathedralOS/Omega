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
    facts: &[crate::ReturnedIntegerFact],
) -> Result<(), Vec<Diagnostic>> {
    use lowered_program_to_checked_program::checking::IntegerBound;
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
        let Some(&home) = fact
            .position()
            .checked_sub(1)
            .and_then(|i| homes.get(i as usize))
        else {
            return Err(vec![Diagnostic::error(
                "returned integer fact has no matching Build frame home",
            )]);
        };
        if !incoming.bindings.iter().any(|binding| binding.home == home) {
            continue;
        }
        let name = parameter(local(syntax, statements[home]));
        let span = name.source_span();
        let formal = read(syntax, name, false);
        let (operator, value) = match fact.bound() {
            IntegerBound::Equal(n) => (BinaryOperator::Equal, n),
            IntegerBound::AtLeast(n) => (BinaryOperator::GreaterOrEqual, n),
            IntegerBound::AtMost(n) => (BinaryOperator::LessOrEqual, n),
        };
        let (negative, magnitude) = match value {
            IntegerValue::Signed(n) => (n < 0, n.unsigned_abs()),
            IntegerValue::Unsigned(n) => (false, n),
        };
        let literal = numeric_literals::IntegerLiteral::from_parts(
            negative,
            numeric_literals::IntegerRadix::Decimal,
            &magnitude.to_string(),
        )
        .expect("integer magnitude has decimal digits");
        let right = syntax.expressions.insert(ExpressionNode::Integer(literal));
        syntax.expressions.set_source_span(right, span);
        let expression = syntax
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: formal,
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
