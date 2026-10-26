use facts::FactPlace;
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::SymbolHandle;

use super::labels::instantiate_call_contract_expression_label;
use super::places::{expression_is_boolean_place_like, expression_place_matches};
use crate::labels::semantic_boolean_fact_label;

mod guard_values;
mod match_patterns;
pub(super) use guard_values::proves_with as guard_value_proves_with;

#[derive(Clone, Copy)]
pub(super) struct InvocationScope<'program, 'proof> {
    pub program: &'program dyn ::validation::CheckingInput,
    pub facts: &'proof checked_trees::CheckFacts,
    pub call_flow: &'proof checked_trees::FlowCallFact,
    pub contexts: &'proof [facts::FactContextHandle],
    pub caller_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_site: &'proof validation::CallSite<'program>,
    pub parameters: &'program [typed_trees::signature::StateParameter],
    pub target_state_symbol: SymbolHandle,
}

/// One invocation owns the preparation shared by its recursive Boolean clauses
/// and entry-context alternatives. All inputs are borrowed immutably for this
/// proof: neither a later call nor a changed context selection can reuse these
/// observations. Context-specific Boolean evidence stays with each alternative.
pub(super) struct InvocationProof<'program, 'proof> {
    scope: InvocationScope<'program, 'proof>,
    assignments: Option<Vec<crate::values::AssignmentObservation>>,
    scalars: Vec<(validation::CanonicalPlace, Option<facts::ScalarValue>)>,
    labels: Vec<(typed_trees::expression::ExpressionHandle, String)>,
}

impl<'program, 'proof> InvocationProof<'program, 'proof> {
    pub(super) fn new(scope: InvocationScope<'program, 'proof>) -> Self {
        Self {
            scope,
            assignments: None,
            scalars: Vec::new(),
            labels: Vec::new(),
        }
    }

    pub(super) fn scope(&self) -> InvocationScope<'program, 'proof> {
        self.scope
    }

    fn scalar_value_at_place(
        &mut self,
        subject: &validation::CanonicalPlace,
    ) -> Option<facts::ScalarValue> {
        let InvocationScope {
            program,
            facts,
            contexts,
            ..
        } = self.scope;
        let semantic = &facts.semantic;
        if let Some(payload) = crate::values::literal_assignment_payload(program, subject) {
            return crate::values::scalar_value_from_payloads(program, semantic, [payload]);
        }
        if let Some((_, value)) = self.scalars.iter().find(|(place, _)| place == subject) {
            return value.clone();
        }
        let assignments = self.assignments.get_or_insert_with(|| {
            contexts
                .iter()
                .flat_map(|context| {
                    crate::values::prepare_assignment_observations(
                        program,
                        semantic,
                        semantic.contexts.get(*context),
                    )
                })
                .collect()
        });
        let value = crate::values::scalar_value_from_payloads(
            program,
            semantic,
            assignments
                .iter()
                .filter(|observation| {
                    crate::flow::canonical_places_agree(program, &observation.place, subject)
                })
                .map(|observation| observation.payload),
        );
        // Cache only the exact query. Semantic place matching admits different
        // spellings of a static index, but need not produce identical match
        // sets for those spellings. The shared reducer still sees every matching
        // payload, including unknown and conflicting observations.
        self.scalars.push((subject.clone(), value.clone()));
        value
    }

    pub(super) fn required_label(
        &mut self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> &str {
        let position = self
            .labels
            .iter()
            .position(|(handle, _)| *handle == expression);
        let position = position.unwrap_or_else(|| {
            let scope = self.scope;
            let label = instantiate_call_contract_expression_label(
                scope.program,
                scope.caller_state_symbol,
                scope.statement_index,
                scope.call_site,
                scope.parameters,
                expression,
            );
            self.labels.push((expression, label));
            self.labels.len() - 1
        });
        &self.labels[position].1
    }
}

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
    proof: &mut InvocationProof<'_, '_>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    context: &facts::FactContext,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let InvocationScope {
        program,
        facts,
        caller_state_symbol,
        statement_index,
        call_site,
        parameters,
        target_state_symbol,
        ..
    } = proof.scope();
    let semantic = &facts.semantic;
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
    if instantiated_live_value_proves(proof, context, expression) {
        return true;
    }
    let required_label = proof.required_label(expression);

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
                    if required_label == semantic.place_label(program, candidate_place)))
    })
}

fn instantiated_live_value_proves(
    proof: &mut InvocationProof<'_, '_>,
    context: &facts::FactContext,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    use super::prover::{ScalarValue, evaluate_scalar};
    use typed_trees::expression::ExpressionNode;

    let InvocationScope {
        program,
        facts,
        caller_state_symbol,
        statement_index,
        call_site,
        parameters,
        ..
    } = proof.scope();
    let semantic = &facts.semantic;
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
                    return projected_formal_leaf_value(proof, arguments, formal);
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
                        proof,
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
fn projected_formal_leaf_value(
    proof: &mut InvocationProof<'_, '_>,
    arguments: &[typed_trees::expression::ExpressionHandle],
    leaf: typed_trees::expression::ExpressionHandle,
) -> Option<facts::ScalarValue> {
    let InvocationScope {
        program,
        facts,
        call_flow,
        contexts,
        caller_state_symbol,
        statement_index,
        parameters,
        ..
    } = proof.scope();
    let semantic = &facts.semantic;
    // A collection measure observes the receiver's exact current descriptor.
    // Keep substitution and selector narrowing identical to ordinary scalar
    // reads; a declared extent and a surviving value snapshot are distinct
    // sources of evidence, and neither replays a local initializer.
    let length_receiver = program
        .expression_table
        .member_node(leaf)
        .filter(|member| {
            member.case_variant.is_none()
                && !member.member_symbol.is_valid()
                && CollectionMeasure::from_authored_spelling(member.member.as_str())
                    == Some(CollectionMeasure::Length)
        })
        .map(|member| member.receiver);
    let leaf = length_receiver.unwrap_or(leaf);
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
    let actual_prefix_length = actual.segments.len();
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
            proof,
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
    if length_receiver.is_some() {
        if let Some(length) = validation::fixed_array_type_length(program, observed_type) {
            return Some(facts::ScalarValue::Integer(
                numerics::bignum::BigInt::from_u64(u64::try_from(length).ok()?),
            ));
        }
        if !observation_carriers_agree(
            program,
            caller_state_symbol,
            statement_index,
            observed_type,
            &actual,
        ) {
            return None;
        }
        let capture = call_flow
            .argument_operands()
            .handle_at(u32::try_from(position).ok()?);
        let length = validation::completed_storage::descriptor_extent(
            program,
            &facts.flow.control,
            capture,
            &actual.segments[actual_prefix_length..],
            call_flow.invocation_storage,
        )
        .or_else(|| {
            let literal = crate::values::literal_at_place(
                program,
                semantic,
                contexts
                    .iter()
                    .map(|context| semantic.contexts.get(*context)),
                &actual,
            )?;
            u64::try_from(typed_trees::byte_predicates::literal_bytes(program, literal)?.len()).ok()
        })?;
        return Some(facts::ScalarValue::Integer(
            numerics::bignum::BigInt::from_u64(length),
        ));
    }
    scalar_value_at_observation(proof, observed_type, &actual)
}

/// Backing coordinates do not establish the scalar observation's interpretation.
/// A recast needs its own checked value transform; matching address or width
/// cannot import the backing carrier's live scalar facts.
fn scalar_value_at_observation(
    proof: &mut InvocationProof<'_, '_>,
    observed_type: typed_trees::types::TypeReferenceHandle,
    place: &validation::CanonicalPlace,
) -> Option<facts::ScalarValue> {
    let scope = proof.scope();
    if !observation_carriers_agree(
        scope.program,
        scope.caller_state_symbol,
        scope.statement_index,
        observed_type,
        place,
    ) {
        return None;
    }
    let mut place = place.clone();
    crate::flow::narrow_pinned_index_segments(
        scope.program,
        &scope.facts.semantic,
        scope.caller_state_symbol,
        scope.statement_index,
        scope.contexts,
        &mut place.segments,
    );
    proof.scalar_value_at_place(&place)
}

/// A stored observation retains its declaration's interpretation across the
/// exact actual/formal substitution. Equal addresses or widths do not suffice.
fn observation_carriers_agree(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    observed_type: typed_trees::types::TypeReferenceHandle,
    place: &validation::CanonicalPlace,
) -> bool {
    let Some(stored_type) =
        validation::canonical_place_type_reference(program, state, statement_index, place)
    else {
        return false;
    };
    let referee = |reference| {
        validation::reference_shell(program, reference).map_or(reference, |shell| shell.referee)
    };
    program.normalized_type_identity(referee(stored_type))
        == program.normalized_type_identity(referee(observed_type))
}
