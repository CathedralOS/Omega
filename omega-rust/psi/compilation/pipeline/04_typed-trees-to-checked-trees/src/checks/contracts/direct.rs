use facts::FactPlace;
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::SymbolHandle;

use super::labels::{ContractTargetParameters, instantiate_call_contract_expression_label};
use super::places::{expression_is_boolean_place_like, expression_place_matches};
use crate::labels::semantic_boolean_fact_label;

mod guard_values;
mod match_patterns;
pub(super) use guard_values::proves_with as guard_value_proves_with;

pub(super) fn direct_context_proves_boolean_expression(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    context: &facts::FactContext,
    contexts: &[facts::FactContextHandle],
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &validation::CallSite<'_>,
    target_state: &(impl ContractTargetParameters + ?Sized),
    target_state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let semantic = &facts.semantic;
    let parameters = target_state.contract_parameters(program);
    let arguments = validation::call_site_argument_expressions(program, call_site);
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
        typed_trees::signature::state_parameter_position(parameters, path.symbol)
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

    let denotations = guard_values::denotations::Denotations {
        program,
        facts,
        caller_state_symbol,
        target_state_symbol,
        statement_index,
        call_frames,
        arguments,
    };
    semantic.context_view(context).facts().any(|fact| {
        let supplied = match fact.payload {
            facts::FactPayload::BooleanValue { expression, value } => Some((expression, value)),
            facts::FactPayload::BooleanExpression(expression) => Some((expression, true)),
            facts::FactPayload::ContractBooleanExpression {
                expression,
                instantiated,
                ..
            } if !instantiated.is_valid() => Some((expression, true)),
            // Instantiated guarantees retain the producer's declaration
            // namespace and cannot be read as current caller expressions.
            _ => None,
        };
        if let Some((guard, value)) = supplied {
            let proven = guard_values::proves_with(
                program,
                guard,
                value,
                expression,
                true,
                &|left, right| denotations.equal(left, right),
            );
            // A boolean value fact is decided by its guard denotation alone.
            // Expression facts also carry the label route below: a state's own
            // `requires i < view.len` re-established for a call with the same
            // operands is proven by its label even where the denotation
            // comparison has no match, so a failed match must not hide it.
            if proven || matches!(fact.payload, facts::FactPayload::BooleanValue { .. }) {
                return proven;
            }
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
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    contexts: &[facts::FactContextHandle],
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &validation::CallSite<'_>,
    target: &(impl ContractTargetParameters + ?Sized),
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    use super::prover::{ScalarValue, evaluate_scalar};
    use typed_trees::expression::ExpressionNode;

    let parameters = target.contract_parameters(program);
    let arguments = validation::call_site_argument_expressions(program, call_site);
    evaluate_scalar(
        program,
        expression,
        &mut |formal| {
            let argument = match program.expression_table.expression(formal) {
                ExpressionNode::Name(path)
                    if path.symbol.is_valid()
                        && program
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            == 1 =>
                {
                    let position =
                        typed_trees::signature::state_parameter_position(parameters, path.symbol)?;
                    *arguments.get(position)?
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
            evaluate_scalar(
                program,
                argument,
                &mut |leaf| {
                    // Local declarations are not current values. Read only assignment
                    // snapshots which survived the scheduled argument effects. Live
                    // scalar facts are spread across the entry contexts, so reads run
                    // over their union just like `assigned_values`' domain prover.
                    if let Some(place) = validation::canonical_place_from_expression_in_state(
                        program,
                        caller_state_symbol,
                        statement_index,
                        leaf,
                    ) && let Some(value) = scalar_value_at_observation(
                        program,
                        semantic,
                        contexts,
                        caller_state_symbol,
                        statement_index,
                        validation::expression_result_type_reference(
                            program,
                            program.machine_holding_state(caller_state_symbol)?,
                            program.state_by_symbol(caller_state_symbol)?,
                            leaf,
                        )?,
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
                            .find(|required| {
                                guard_values::proves(program, guard, value, leaf, *required)
                            })
                            .map(ScalarValue::Boolean)
                    })
                },
                &|_, _| true,
            )
        },
        &|_, _| true,
    ) == Some(ScalarValue::Boolean(true))
}

/// Read a callee projection's value on the actual referent. `room.count` under
/// `requires room.count < bound` resolves to the argument's storage plus the
/// retained member suffix (`rooms[i].count`); the live snapshot at that exact
/// place is the only evidence this route accepts, and a runtime index segment
/// narrows to its currently recorded element or the read refuses.
#[allow(clippy::too_many_arguments)]
fn projected_formal_leaf_value(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    contexts: &[facts::FactContextHandle],
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    parameters: &[typed_trees::signature::StateParameter],
    arguments: &[typed_trees::expression::ExpressionHandle],
    leaf: typed_trees::expression::ExpressionHandle,
) -> Option<facts::ScalarValue> {
    // `room.exits.len` selects no storage: `len` on a fixed array is the
    // declared type's literal extent, so the receiver only has to be the
    // same checked projection of an explicit formal. A declared `len` field,
    // a slice's runtime length, or an unresolved extent keeps the storage
    // read below (or refuses there) — this route owns no other evidence.
    if let Some(member) = program.expression_table.member_node(leaf)
        && member.case_variant.is_none()
        && !member.member_symbol.is_valid()
        && CollectionMeasure::from_authored_spelling(member.member.as_str())
            == Some(CollectionMeasure::Length)
    {
        return projected_collection_extent(program, parameters, member.receiver);
    }
    let mut required = crate::flow::projected_leaf_place(program, leaf)?;
    let formal = required.root.as_symbol()?;
    // `self`'s projections stay with the receiver's own substitution owner;
    // only explicit formals join the positional argument roster.
    let position = typed_trees::signature::state_parameter_position(parameters, formal)?;
    let argument = *arguments.get(position)?;
    let observed_type = validation::project_type_reference_from_segments(
        program,
        parameters[position].type_reference,
        &required.segments,
    )?;
    let mut actual = validation::canonical_place_from_expression_in_state(
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
        let Some(selector) = validation::canonical_place_from_expression_in_state(
            program,
            caller_state_symbol,
            statement_index,
            expression,
        ) else {
            continue;
        };
        let Some(facts::ScalarValue::Integer(value)) = scalar_value_at_observation(
            program,
            semantic,
            contexts,
            caller_state_symbol,
            statement_index,
            validation::expression_result_type_reference(
                program,
                program.machine_holding_state(caller_state_symbol)?,
                program.state_by_symbol(caller_state_symbol)?,
                expression,
            )?,
            &selector,
        ) else {
            continue;
        };
        if let Some(index) = value.to_u64().and_then(|value| usize::try_from(value).ok()) {
            *segment = facts::PlaceSegment::FixedIndex { index };
        }
    }
    scalar_value_at_observation(
        program,
        semantic,
        contexts,
        caller_state_symbol,
        statement_index,
        observed_type,
        &actual,
    )
}

/// Backing coordinates do not establish the scalar observation's interpretation.
/// A recast needs its own checked value transform; matching address or width
/// cannot import the backing carrier's live scalar facts.
fn scalar_value_at_observation(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    contexts: &[facts::FactContextHandle],
    state: SymbolHandle,
    statement_index: usize,
    observed_type: typed_trees::types::TypeReferenceHandle,
    place: &validation::CanonicalPlace,
) -> Option<facts::ScalarValue> {
    let stored_type =
        validation::canonical_place_type_reference(program, state, statement_index, place)?;
    let referee = |reference| {
        validation::reference_shell(program, reference).map_or(reference, |shell| shell.referee)
    };
    if program.normalized_type_identity(referee(stored_type))
        != program.normalized_type_identity(referee(observed_type))
    {
        return None;
    }
    let mut place = place.clone();
    crate::flow::narrow_pinned_index_segments(
        program,
        semantic,
        state,
        statement_index,
        contexts,
        &mut place.segments,
    );
    super::prover::scalar_value_at_place(
        program,
        semantic,
        contexts
            .iter()
            .map(|context| semantic.contexts.get(*context)),
        &place,
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
    program: &dyn ::validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    receiver: typed_trees::expression::ExpressionHandle,
) -> Option<facts::ScalarValue> {
    let required = crate::flow::projected_leaf_place(program, receiver)?;
    let formal = required.root.as_symbol()?;
    if !parameters
        .iter()
        .any(|parameter| parameter.symbol == formal)
    {
        return None;
    }
    let reference = validation::expression_place_type_reference(program, receiver, &[])?;
    let length = crate::checks::ranges::fixed_array_type_length(program, reference)?;
    Some(facts::ScalarValue::Integer(
        numerics::bignum::BigInt::from_u64(u64::try_from(length).ok()?),
    ))
}
