//! The fixed scalar dependency set whose duplicated state slots must stay equal.
//! Unrelated payload copies retain independent values, even in numeric carriers.
//!
//! The same collection also names the premise carriers: every root formal whose
//! arrival value some rank-range judgment may read. The prefix readers protect
//! exactly those paths against intervening writes, so a mutable formal outside
//! the set may be stored to before an edge or a call site without invalidating
//! the entry-relative ranking.
use super::{
    ExpressionHandle, ExpressionNode, Machine, RankPremiseInput, RankPremiseObservation,
    RankingRangeMeasure, RankingRangePremises, integer_parameter,
};
use symbols::SymbolHandle;
use typed_trees::expression::TableRangeExpression;
use typed_trees::state::State;

pub(super) fn required_symbols(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    range: Option<&TableRangeExpression>,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
) -> Option<Vec<SymbolHandle>> {
    let root = program.machine_states(machine).first()?;
    let mut symbols = premise_symbols(
        program,
        machine,
        range,
        measure,
        matches!(premises, RankingRangePremises::EntryInvariant),
    )?
    .into_iter()
    .map(|input| input.parameter)
    .collect::<Vec<_>>();
    symbols.dedup();
    symbols.retain(|symbol| {
        program.state_parameters(root).iter().any(|parameter| {
            parameter.symbol == *symbol
                && integer_parameter(program, parameter.type_reference).is_some()
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
/// Each formal carries how much of it the judgment reads: a collection named
/// only through `.len` contributes its extent, which element stores preserve.
pub(super) fn premise_symbols(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    range: Option<&TableRangeExpression>,
    measure: RankingRangeMeasure,
    entry_facts: bool,
) -> Option<Vec<RankPremiseInput>> {
    let root = program.machine_states(machine).first()?;
    let mut constrained = Vec::new();
    let mut expressions = range.map_or_else(Vec::new, |range| vec![range.start, range.end]);
    super::push_measure_subjects(measure, &mut expressions);
    if entry_facts {
        expressions.extend(super::required_contract_expressions(program, machine));
        super::constrained_parameter_bound_expressions(
            program,
            root,
            &mut expressions,
            Some(&mut constrained),
        );
    }
    let mut inputs = Vec::new();
    for parameter in constrained {
        record(
            &mut inputs,
            parameter,
            Vec::new(),
            RankPremiseObservation::Value,
        );
    }
    for expression in expressions {
        collect(program, machine, root, expression, &mut inputs, 0)?;
    }
    inputs.retain(|input| {
        program
            .state_parameter_by_symbol(root, input.parameter)
            .is_some()
    });
    Some(inputs)
}

/// A whole-formal reading covers every extent beneath it and never weakens;
/// extents at distinct paths are distinct carriers.
fn record(
    inputs: &mut Vec<RankPremiseInput>,
    parameter: SymbolHandle,
    path: Vec<facts::PlaceSegment>,
    observation: RankPremiseObservation,
) {
    if observation == RankPremiseObservation::Value {
        inputs.retain(|input| input.parameter != parameter);
    } else if inputs.iter().any(|input| {
        input.parameter == parameter
            && (input.observation == RankPremiseObservation::Value || input.path == path)
    }) {
        return;
    }
    inputs.push(RankPremiseInput {
        parameter,
        path,
        observation,
    });
}

fn collect(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    root: &State,
    expression: ExpressionHandle,
    inputs: &mut Vec<RankPremiseInput>,
    depth: usize,
) -> Option<()> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    match node {
        // Literal spellings have no state dependency. Formation and selected
        // meaning separately decide whether a decimal can land as an integer.
        ExpressionNode::Integer(_) | ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => {}
        ExpressionNode::Name(name) if name.symbol.is_valid() && name.head_symbol == name.symbol => {
            record(
                inputs,
                name.symbol,
                Vec::new(),
                RankPremiseObservation::Value,
            );
        }
        ExpressionNode::Atomic(atomic) => {
            collect(program, machine, root, atomic.value, inputs, depth + 1)?;
        }
        ExpressionNode::Unary(unary) => {
            collect(program, machine, root, unary.operand, inputs, depth + 1)?;
        }
        ExpressionNode::Member(member) => {
            // `.len` on a collection place reads that collection's extent and
            // nothing else of the formal it sits in.
            let extent_of = crate::value_custody::places::collection_length_receiver(
                program,
                machine,
                Some(root),
                expression,
            )
            .and_then(|receiver| {
                let place = crate::canonical_place_from_expression_in_state(
                    program,
                    root.symbol,
                    usize::MAX,
                    receiver,
                )?;
                Some((place.root.as_symbol()?, place.segments))
            });
            match extent_of {
                Some((parameter, path)) => {
                    record(inputs, parameter, path, RankPremiseObservation::Length);
                }
                None => collect(program, machine, root, member.receiver, inputs, depth + 1)?,
            }
        }
        ExpressionNode::Binary(binary) => {
            collect(program, machine, root, binary.left, inputs, depth + 1)?;
            collect(program, machine, root, binary.right, inputs, depth + 1)?;
        }
        // A fact can spell carriers through any expression shape a contract
        // admits -- predicate calls (`sorted(values) == true`), borrows,
        // casts, indexing, literals. Their formals still sit in the subtrees;
        // a node this arm does not interpret contributes its children, never a
        // refusal, so one opaque operator cannot void the whole premise set.
        node => {
            let mut missing = false;
            crate::expression_traversal::children(program, node, |child| {
                if child.is_valid()
                    && collect(program, machine, root, child, inputs, depth + 1).is_none()
                {
                    missing = true;
                }
            });
            if missing {
                return None;
            }
        }
    }
    Some(())
}
