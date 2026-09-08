//! Exact referent substitution for supplied Boolean field facts at call entry.

use facts::{FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

use crate::flow::CanonicalPlace;

#[cfg(test)]
mod tests;

/// None leaves unrelated computed-value proof paths unchanged. A recognized
/// place requirement returns Some(false) on mismatch and cannot use labels.
pub(super) fn proves(
    program: &TypedTrees,
    semantic: &FactPlan,
    context: &facts::FactContext,
    caller_state: SymbolHandle,
    statement_index: usize,
    call: &crate::CallSite<'_>,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<bool> {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_)
    ) {
        return None;
    }
    // Implicit receivers retain their existing receiver-substitution owner.
    // Establish this helper's explicit-formal scope before field preflight.
    match has_explicit_formal_root(program, parameters, expression) {
        Some(true) => {}
        Some(false) => return None,
        None => return Some(false),
    }
    let Some(mut required) = checked_place(program, expression) else {
        return Some(false);
    };
    let PlaceRoot::Symbol(formal) = required.root else {
        return None;
    };
    let position = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.symbol == formal)?;
    let Some(argument) = crate::call_site_argument_expressions(program, call)
        .get(position)
        .copied()
    else {
        return Some(false);
    };
    let Some(raw_actual) = checked_place(program, argument) else {
        return Some(false);
    };
    // Constructed/call-produced values retain their existing value evaluator;
    // this proof path is only substitution of an existing storage referent.
    if !matches!(raw_actual.root, PlaceRoot::Symbol(_)) {
        return None;
    }
    let Some(mut actual) = crate::flow::canonical_place_from_expression_in_state(
        program,
        caller_state,
        statement_index,
        argument,
    ) else {
        return Some(false);
    };
    actual.segments.append(&mut required.segments);
    Some(semantic.context_view(context).facts().any(|fact| {
        let candidate = match fact.payload {
            FactPayload::BooleanExpression(expression) => expression,
            FactPayload::ContractBooleanExpression { expression, .. } => expression,
            FactPayload::BooleanValue {
                expression,
                value: true,
            } => expression,
            _ => return false,
        };
        if !matches!(
            program.expression_table.expression(candidate),
            ExpressionNode::Member(_)
        ) {
            return false;
        }
        let Some(candidate) = checked_place(program, candidate) else {
            return false;
        };
        let FactPlace::Place(place) = fact.place else {
            return false;
        };
        // Call postconditions retain their original formal predicate while the
        // semantic substitution owner publishes its full actual FactPlace.
        // Only direct positive members can use that substituted subject; their
        // exact field suffix must remain part of the retained actual place.
        let substituted = fact.origin == FactOrigin::CallEnsures
            && matches!(fact.point, ProgramPoint::CallEnsures { .. });
        if if substituted {
            !actual.segments.ends_with(&candidate.segments)
        } else {
            candidate != actual
        } {
            return false;
        }
        // Only facts surviving the scheduled call-entry context may prove this
        // use, and the complete invalidation subject must match, not a suffix.
        crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(place),
        )
        .is_some_and(|place| place == actual)
    }))
}

fn has_explicit_formal_root(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<bool> {
    let mut current = expression;
    let mut visited = Vec::new();
    while program.expression_table.expression_is_valid(current) && !visited.contains(&current) {
        visited.push(current);
        current = match program.expression_table.expression(current) {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Borrow(borrow) => borrow.target,
            ExpressionNode::Name(name) => {
                if !name.head_symbol.is_valid() || !name.symbol.is_valid() {
                    return None;
                }
                return Some(parameters.iter().any(|parameter| {
                    !parameter.is_self
                        && (parameter.symbol == name.head_symbol || parameter.symbol == name.symbol)
                }));
            }
            _ => return Some(false),
        };
    }
    None
}

fn checked_place(program: &TypedTrees, expression: ExpressionHandle) -> Option<CanonicalPlace> {
    let mut current = expression;
    let mut visited = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(current) || visited.contains(&current) {
            return None;
        }
        visited.push(current);
        current = match program.expression_table.expression(current) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Name(name) => {
                if !name.symbol.is_valid()
                    || !name.head_symbol.is_valid()
                    || program
                        .expression_table
                        .name_path_member_symbols(name.member_symbols)
                        .first()
                        .is_some_and(|symbol| *symbol != name.head_symbol)
                    || matches!(
                        program.expression_table.name_path_members(name.members),
                        [_]
                    ) && name.symbol != name.head_symbol
                {
                    return None;
                }
                break;
            }
            _ => break,
        };
    }
    for expression in visited.into_iter().rev() {
        if let ExpressionNode::Member(member) = program.expression_table.expression(expression) {
            // Check only after establishing an acyclic receiver chain. The
            // existing owner cannot replace a conflicting retained selection.
            if !member.member_symbol.is_valid()
                || crate::flow::effective_member_symbol(program, member.receiver, member)
                    != member.member_symbol
            {
                return None;
            }
        }
    }
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    (!place
        .segments
        .iter()
        .any(|segment| crate::flow::place_segment_has_unresolved_identity(*segment)))
    .then_some(place)
}
