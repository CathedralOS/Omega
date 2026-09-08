//! The fixed scalar dependency set whose duplicated state slots must stay equal.
//! Unrelated payload copies retain independent values, even in numeric carriers.

use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::TableRangeExpression;

pub(super) fn required_symbols(
    program: &TypedTrees,
    machine: &Machine,
    range: &TableRangeExpression,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
) -> Option<Vec<SymbolHandle>> {
    let root = program.machine_states(machine).first()?;
    let mut symbols = Vec::new();
    let mut expressions = vec![range.start, range.end];
    match measure {
        RankingRangeMeasure::Single(subject)
        | RankingRangeMeasure::SliceLength(subject)
        | RankingRangeMeasure::Field { subject, .. } => expressions.push(subject),
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => expressions.extend([lower, upper]),
    }
    if matches!(premises, RankingRangePremises::EntryInvariant) {
        for contract in program
            .machine_contracts(machine)
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Expression(expression) = fact {
                    expressions.push(*expression);
                }
            }
        }
        for parameter in program.state_parameters(root).iter().filter(|parameter| {
            !parameter.is_self
                && exact_integer_parameter(program, parameter.type_reference).is_some()
        }) {
            let mut reference = parameter.type_reference;
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(reference)
            {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Range { minimum, maximum } = constraint {
                        if !symbols.contains(&parameter.symbol) {
                            symbols.push(parameter.symbol);
                        }
                        expressions.extend([*minimum, *maximum]);
                    }
                }
                reference = *base_type;
            }
        }
    }
    for expression in expressions {
        collect(program, expression, &mut symbols, 0)?;
    }
    symbols.retain(|symbol| {
        program.state_parameters(root).iter().any(|parameter| {
            parameter.symbol == *symbol
                && !parameter.is_self
                && exact_integer_parameter(program, parameter.type_reference).is_some()
        })
    });
    Some(symbols)
}

fn collect(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbols: &mut Vec<SymbolHandle>,
    depth: usize,
) -> Option<()> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        // Literal spellings have no state dependency. Formation and selected
        // meaning separately decide whether a decimal can land as an integer.
        ExpressionNode::Integer(_) | ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => {}
        ExpressionNode::Name(name) if name.symbol.is_valid() && name.head_symbol == name.symbol => {
            if !symbols.contains(&name.symbol) {
                symbols.push(name.symbol);
            }
        }
        ExpressionNode::Atomic(atomic) => collect(program, atomic.value, symbols, depth + 1)?,
        ExpressionNode::Unary(unary) => collect(program, unary.operand, symbols, depth + 1)?,
        ExpressionNode::Member(member) => collect(program, member.receiver, symbols, depth + 1)?,
        ExpressionNode::Binary(binary) => {
            collect(program, binary.left, symbols, depth + 1)?;
            collect(program, binary.right, symbols, depth + 1)?;
        }
        _ => return None,
    }
    Some(())
}
