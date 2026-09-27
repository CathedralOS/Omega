//! The fixed scalar dependency set whose duplicated state slots must stay equal.
//! Unrelated payload copies retain independent values, even in numeric carriers.
//!
//! The same collection also names the premise carriers: every root formal whose
//! arrival value some rank-range judgment may read. The prefix readers protect
//! exactly those paths against intervening writes, so a mutable formal outside
//! the set may be stored to before an edge or a call site without invalidating
//! the entry-relative ranking.
use super::{
    ExpressionHandle, ExpressionNode, Machine, ProofFact, RankingRangeMeasure,
    RankingRangePremises, SignatureContractKind, TypeConstraintNode, TypeReferenceNode, TypedTrees,
    exact_integer_parameter,
};
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
    let mut symbols = premise_symbols(
        program,
        machine,
        Some(range),
        measure,
        matches!(premises, RankingRangePremises::EntryInvariant),
    )?;
    symbols.retain(|symbol| {
        program.state_parameters(root).iter().any(|parameter| {
            parameter.symbol == *symbol
                && !parameter.is_self
                && exact_integer_parameter(program, parameter.type_reference).is_some()
        })
    });
    Some(symbols)
}

/// Every root formal, of any carrier, named by the range endpoints, the
/// produced-rank subjects, and -- when `entry_facts` -- the requires facts and
/// the range-constrained integer formals. Constrained formals are included
/// even though a store into them is type-enforced: a telescoped slot may carry
/// that role under a wider declared type, so its root fact only holds while the
/// slot still denotes the arrival value. An absent range contributes no
/// endpoint; the caller decides whether the whole judgment is then supported.
pub(super) fn premise_symbols(
    program: &TypedTrees,
    machine: &Machine,
    range: Option<&TableRangeExpression>,
    measure: RankingRangeMeasure,
    entry_facts: bool,
) -> Option<Vec<SymbolHandle>> {
    let root = program.machine_states(machine).first()?;
    let mut symbols = Vec::new();
    let mut expressions = range.map_or_else(Vec::new, |range| vec![range.start, range.end]);
    match measure {
        RankingRangeMeasure::Single(subject)
        | RankingRangeMeasure::Computed { subject, .. }
        | RankingRangeMeasure::SliceLength(subject)
        | RankingRangeMeasure::Field { subject, .. } => expressions.push(subject),
        RankingRangeMeasure::Distance { lower, upper }
        | RankingRangeMeasure::IncreasingTo {
            subject: lower,
            limit: upper,
        } => expressions.extend([lower, upper]),
    }
    if entry_facts {
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
                    if let TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } = constraint
                    {
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
        program
            .state_parameters(root)
            .iter()
            .any(|parameter| parameter.symbol == *symbol && !parameter.is_self)
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
