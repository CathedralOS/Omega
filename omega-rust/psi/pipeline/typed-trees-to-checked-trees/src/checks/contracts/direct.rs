use facts::FactPlace;
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::SymbolHandle;

use super::labels::{ContractTargetParameters, instantiate_call_contract_expression_label};
use super::places::{expression_is_boolean_place_like, expression_place_matches};
use crate::labels::semantic_boolean_fact_label;

mod guard_values;
mod match_patterns;

pub(super) fn direct_context_proves_boolean_expression(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let required_label = program.expression_table.display_name(expression);

    semantic.context_view(context).facts().any(|fact| {
        if let Some((subject, pattern, matched)) = fact.payload.match_pattern_comparison(program) {
            return match_patterns::proves(
                program,
                subject,
                pattern,
                matched,
                expression,
                true,
                &|value| value,
            );
        }
        if let facts::FactPayload::BooleanValue {
            expression: guard,
            value,
        } = fact.payload
        {
            return guard_values::proves(program, guard, value, expression, true);
        }
        let candidate_label = semantic_boolean_fact_label(program, semantic, fact).or_else(|| {
            semantic
                .proposition_fact_label(program, fact)
                .and_then(|label| label.strip_prefix("boolean:").map(str::to_owned))
        });
        let Some(candidate_label) = candidate_label else {
            return false;
        };

        candidate_label == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                    if expression_place_matches(program, semantic, expression, candidate_place)))
    })
}

pub(super) fn direct_context_proves_instantiated_boolean_expression(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    contexts: &[facts::FactContextHandle],
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::semantic::calls::CallSite<'_>,
    target_state: &(impl ContractTargetParameters + ?Sized),
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let parameters = target_state.contract_parameters(program);
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, call_site);
    let substitute = |expression| {
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(expression)
        else {
            return expression;
        };
        if !path.symbol.is_valid()
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return expression;
        }
        if parameters
            .iter()
            .any(|parameter| parameter.is_self && parameter.symbol == path.symbol)
        {
            return match call_site {
                crate::semantic::calls::CallSite::Expression { call, .. } => call.receiver,
                _ => expression,
            };
        }
        parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .position(|parameter| parameter.symbol == path.symbol)
            .and_then(|position| arguments.get(position).copied())
            .unwrap_or(expression)
    };
    if semantic.context_view(context).facts().any(|fact| {
        fact.payload
            .match_pattern_comparison(program)
            .is_some_and(|(subject, pattern, matched)| {
                match_patterns::proves(
                    program,
                    subject,
                    pattern,
                    matched,
                    expression,
                    true,
                    &substitute,
                )
            })
    }) {
        return true;
    }
    if instantiated_live_value_proves(
        program,
        semantic,
        context,
        contexts,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    ) {
        return true;
    }
    let required_label = instantiate_call_contract_expression_label(
        program,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    );

    semantic.context_view(context).facts().any(|fact| {
        let candidate_label = semantic_boolean_fact_label(program, semantic, fact).or_else(|| {
            semantic
                .proposition_fact_label(program, fact)
                .and_then(|label| label.strip_prefix("boolean:").map(str::to_owned))
        });
        let Some(candidate_label) = candidate_label else {
            return false;
        };

        candidate_label == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                    if instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        expression,
                ) == semantic.place_label(program, candidate_place)))
    })
}

#[allow(clippy::too_many_arguments)]
fn instantiated_live_value_proves(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    contexts: &[facts::FactContextHandle],
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::semantic::calls::CallSite<'_>,
    target: &(impl ContractTargetParameters + ?Sized),
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    use super::prover::{ScalarValue, evaluate_scalar};
    use typed_trees::expression::ExpressionNode;

    let parameters = target.contract_parameters(program);
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, call_site);
    evaluate_scalar(program, expression, &mut |formal| {
        let argument = match program.expression_table.expression(formal) {
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1 =>
            {
                if parameters
                    .iter()
                    .any(|parameter| parameter.is_self && parameter.symbol == path.symbol)
                {
                    match call_site {
                        crate::semantic::calls::CallSite::Expression { call, .. } => call.receiver,
                        _ => return None,
                    }
                } else {
                    let position = parameters
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .position(|parameter| parameter.symbol == path.symbol)?;
                    *arguments.get(position)?
                }
            }
            // A member or indexed projection of an explicit formal (`room.count`,
            // `table[i]`) names the actual's storage, not a value the call
            // expression computes. Substitute the formal's root for the
            // argument's referent place and read the projection there.
            _ => {
                return projected_formal_leaf_value(
                    program,
                    semantic,
                    contexts,
                    caller_state_symbol,
                    statement_index,
                    parameters,
                    arguments,
                    formal,
                );
            }
        };
        evaluate_scalar(program, argument, &mut |leaf| {
            // Local declarations are not current values. Read only assignment
            // snapshots which survived the scheduled argument effects. Live
            // scalar facts are spread across the entry contexts, so reads run
            // over their union just like `assigned_values`' domain prover.
            if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                caller_state_symbol,
                statement_index,
                leaf,
            ) && let Some(value) = super::prover::scalar_value_at_place(
                program,
                semantic,
                contexts
                    .iter()
                    .map(|context| semantic.contexts.get(*context)),
                &place,
            ) {
                return Some(value);
            }
            semantic.context_view(context).facts().find_map(|fact| {
                let facts::FactPayload::BooleanValue {
                    expression: guard,
                    value,
                } = fact.payload
                else {
                    return None;
                };
                [true, false]
                    .into_iter()
                    .find(|required| guard_values::proves(program, guard, value, leaf, *required))
                    .map(ScalarValue::Boolean)
            })
        })
    }) == Some(ScalarValue::Boolean(true))
}

/// Read a callee projection's value on the actual referent. `room.count` under
/// `requires room.count < bound` resolves to the argument's storage plus the
/// retained member suffix (`rooms[i].count`); the live snapshot at that exact
/// place is the only evidence this route accepts, and a runtime index segment
/// narrows to its currently recorded element or the read refuses.
#[allow(clippy::too_many_arguments)]
fn projected_formal_leaf_value(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    contexts: &[facts::FactContextHandle],
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    parameters: &[typed_trees::signature::StateParameter],
    arguments: &[typed_trees::expression::ExpressionHandle],
    leaf: typed_trees::expression::ExpressionHandle,
) -> Option<facts::ScalarValue> {
    use typed_trees::expression::ExpressionNode;

    // `room.exits.len` selects no storage: `len` on a fixed array is the
    // declared type's literal extent, so the receiver only has to be the
    // same checked projection of an explicit formal. A declared `len` field,
    // a slice's runtime length, or an unresolved extent keeps the storage
    // read below (or refuses there) — this route owns no other evidence.
    if let ExpressionNode::Member(member) = program.expression_table.expression(leaf)
        && member.case_variant.is_none()
        && !member.member_symbol.is_valid()
        && CollectionMeasure::from_authored_spelling(member.member.as_str())
            == Some(CollectionMeasure::Length)
    {
        return projected_collection_extent(program, parameters, member.receiver);
    }
    let mut required = projected_leaf_place(program, leaf)?;
    let facts::PlaceRoot::Symbol(formal) = required.root else {
        return None;
    };
    // `self`'s projections stay with the receiver's own substitution owner;
    // only explicit formals join the positional argument roster.
    let position = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.symbol == formal)?;
    let argument = *arguments.get(position)?;
    let mut actual = crate::flow::canonical_place_from_expression_in_state(
        program,
        caller_state_symbol,
        statement_index,
        argument,
    )?;
    actual.segments.append(&mut required.segments);
    for segment in &mut actual.segments {
        let facts::PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        // A stored selector narrows to the fixed element it currently names.
        // The read below rejects anything the live evidence cannot pin down,
        // and mutation invalidation has already retired a stale selector.
        let Some(selector) = crate::flow::canonical_place_from_expression_in_state(
            program,
            caller_state_symbol,
            statement_index,
            expression,
        ) else {
            continue;
        };
        let Some(facts::ScalarValue::Integer(value)) = super::prover::scalar_value_at_place(
            program,
            semantic,
            contexts
                .iter()
                .map(|context| semantic.contexts.get(*context)),
            &selector,
        ) else {
            continue;
        };
        if let Some(index) = value.to_u64().and_then(|value| usize::try_from(value).ok()) {
            *segment = facts::PlaceSegment::FixedIndex { index };
        }
    }
    super::prover::scalar_value_at_place(
        program,
        semantic,
        contexts
            .iter()
            .map(|context| semantic.contexts.get(*context)),
        &actual,
    )
}

/// The static extent a `.len` member names: `room.exits.len` under
/// `requires room.exit_count < room.exits.len` is `[Exit; 4]`'s literal
/// length, carried by the declared type rather than by any live snapshot.
/// The receiver keeps the projected-place contract — an explicit formal
/// root reached through checked member hops — and only a fixed array's
/// literal extent resolves; a slice or const-parameter length stays runtime
/// evidence this route does not own.
fn projected_collection_extent(
    program: &typed_trees::TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
    receiver: typed_trees::expression::ExpressionHandle,
) -> Option<facts::ScalarValue> {
    let required = projected_leaf_place(program, receiver)?;
    let facts::PlaceRoot::Symbol(formal) = required.root else {
        return None;
    };
    if !parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .any(|parameter| parameter.symbol == formal)
    {
        return None;
    }
    let reference = crate::flow::expression_place_type_reference(program, receiver, &[])?;
    let length = crate::checks::ranges::fixed_array_type_length(program, reference)?;
    Some(facts::ScalarValue::Integer(
        numerics::bignum::BigInt::from_u64(u64::try_from(length).ok()?),
    ))
}

/// `field_actuals::checked_place`'s contract for a requires leaf: the receiver
/// chain must stay acyclic and every retained member selection must still be
/// the member its expression resolves to, or the place it yields would read
/// evidence the source did not name. Kept local because the sibling owner
/// lives inside `prover`'s private module.
fn projected_leaf_place(
    program: &typed_trees::TypedTrees,
    leaf: typed_trees::expression::ExpressionHandle,
) -> Option<crate::flow::CanonicalPlace> {
    use typed_trees::expression::ExpressionNode;

    let mut current = leaf;
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
        if let ExpressionNode::Member(member) = program.expression_table.expression(expression)
            && (!member.member_symbol.is_valid()
                || crate::flow::effective_member_symbol(program, member.receiver, member)
                    != member.member_symbol)
        {
            return None;
        }
    }
    let place = crate::flow::canonical_place_from_expression(program, leaf)?;
    (!place
        .segments
        .iter()
        .any(|segment| crate::flow::place_segment_has_unresolved_identity(*segment)))
    .then_some(place)
}
