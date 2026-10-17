//! Source substitution is valid only while it denotes the captured operand.
//! A copied constructor child can precede a write by a sibling or later actual.
//! Such a predicate must use its retained capture; syntax-only proof routes may
//! not reinterpret both observations as the invocation's final storage value.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::{Fact, FactPayload};

use super::super::call_values;

/// `None` permits the ordinary source-substitution proof routes. `Some` is a
/// capture-owned judgment, including failure: no later fallback may replace it.
pub(super) fn judgment(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    requirement: &Fact,
    expression_intervals: &[validation::ExpressionInterval],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<bool> {
    let FactPayload::ContractBooleanExpression { expression, .. } = requirement.payload else {
        return None;
    };
    let proven = (|| {
        let FactPayload::ContractBooleanExpression {
            fact: source_fact, ..
        } = requirement.payload
        else {
            return None;
        };
        let contract_call = crate::flow::proof_contract_call(
            &facts.proof,
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let mut seen = false;
        for reference in facts
            .proof
            .contract_fact_refs
            .span(contract_call.requires)?
        {
            let contract = facts.proof.contract_facts.get(reference.fact);
            if contract.fact != source_fact {
                continue;
            }
            seen = true;
            if !captured_contract_proven(
                program,
                facts,
                state,
                call,
                contexts,
                contract,
                contract_call.target_state_symbol,
                expression,
                expression_intervals,
                frames,
            ) {
                return Some(false);
            }
        }
        Some(seen)
    })();
    if proven == Some(true)
        || captured_float_requirement(program, facts, state, call, expression) == Some(true)
    {
        return Some(true);
    }
    // Ordinary proof routes observe the authored actual, not its initializer.
    // A retained value-origin proof may discharge the requirement first; a
    // failed origin proof cannot weaken the original operand's epoch fence.
    if source_substitution_preserves_captures(program, facts, state, call, requirement)
        == Some(true)
    {
        None
    } else {
        Some(false)
    }
}

/// A selected semantic equation describes this completed operand, not a replay
/// of its runtime call. Closed terms and complete Integer carrier images can
/// establish a Boolean consequence without claiming the Float value has
/// substitution identity. Unknown projections remain unknown: they cannot read
/// current mutable storage or inherit a different operand's classification.
fn captured_float_requirement(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<bool> {
    let (machine, target_state) =
        validation::find_state(program, call.target_symbol).or_else(|| {
            let machine = program.machine_by_symbol(call.target_symbol)?;
            Some((machine, program.machine_states(machine).first()?))
        })?;
    let parameters = validation::call_target_parameters(program, call.target_symbol)?;
    let arguments = call_values::captured_argument_expressions(program, facts, state, call)?;
    let contexts = crate::checks::operators::InvocationContexts::from_call(facts, call, &arguments);
    // Boolean composition keeps each leaf in its own captured operand epoch.
    // The IEEE query publishes only proved positive atoms: failure is unknown,
    // never the false polarity of a NaN-permitting comparison.
    if crate::checks::operators::closed_operators_are_builtin(program, &facts.operators, expression)
        && super::super::prover::evaluate_for_atoms(
            program,
            expression,
            &mut |_| None,
            &|_, _| true,
            &mut |leaf| {
                crate::checks::float_implications::contexts_prove_comparison(
                    program,
                    &facts.semantic,
                    &facts.operators,
                    &contexts.for_expressions(program, parameters, [leaf]),
                    parameters,
                    &arguments,
                    leaf,
                )
                .then_some(true)
            },
        ) == Some(super::super::prover::ScalarValue::Boolean(true))
    {
        return Some(true);
    }
    crate::checks::operators::selected_float_boolean_requirement(
        program,
        facts,
        machine.symbol,
        target_state.symbol,
        expression,
        parameters,
        &arguments,
        state.machine_symbol,
        state.state_symbol,
    )
}

/// Reuse the ordinary Boolean evaluator with evidence-owned atoms. Scalar
/// relations keep their paired capture epochs; semantic place predicates ask
/// their existing owner using only the selected subject's contexts.
#[allow(clippy::too_many_arguments)]
fn captured_contract_proven(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    contract: &checked_trees::ContractProofFact,
    target: symbols::SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    expression_intervals: &[validation::ExpressionInterval],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    // The exact normal-call prefix owns completed nested operands and their
    // public guarantees. It must run before the source-replay fence: a call
    // result need not be repeatable to establish this occurrence's requirement.
    // Named and statement destinations use the same captured-operand prefix;
    // they must reach it before the replay fence just like expression calls.
    // Use the retained authored fact, not a replay of its substituted syntax.
    // The current roster retains descriptor measures and endpoint snapshots;
    // its whole-operand write fence declines when an earlier operand could
    // replace them. It is therefore a capture proof, not a source fallback.
    if let typed_trees::domain::ProofFact::Expression(required) =
        program.proof_facts.get(contract.fact)
        && (super::super::current_values::proves_call(program, state, call, *required, frames)
            || super::super::entailment::structural_call_requirement(
                program, facts, state, call, *required, frames,
            ))
    {
        return true;
    }
    if call_values::proves_captured_arithmetic_requirement(program, facts, state, call, expression)
    {
        return true;
    }
    let prove_scalar = |expression| {
        let predicate = bound_contract_predicate(program, facts, contract, target, expression)?;
        call_values::proves_bound_predicate(
            program,
            facts,
            state,
            call,
            contexts,
            &predicate,
            frames,
            &|observation| {
                actual_for_formal_place(
                    program,
                    facts,
                    state,
                    call,
                    contract,
                    target,
                    &observation.place,
                )
            },
        )
        .then_some(true)
    };
    if prove_scalar(expression) == Some(true)
        || captured_float_requirement(program, facts, state, call, expression) == Some(true)
    {
        return true;
    }
    // Captured facts compose before the source-substitution fence. Each child
    // retains this same contract and operand roster; an IEEE result predicate
    // and an integer bound need not be understood by one arithmetic theory.
    // These logical connectives have no overloadable spelling.
    if let Some(binary) = program.expression_table.binary_node(expression) {
        use typed_trees::expression::BinaryOperator;
        let child = |expression| {
            captured_contract_proven(
                program,
                facts,
                state,
                call,
                contexts,
                contract,
                target,
                expression,
                expression_intervals,
                frames,
            )
        };
        match binary.operator {
            BinaryOperator::And => return child(binary.left) && child(binary.right),
            BinaryOperator::Or => return child(binary.left) || child(binary.right),
            _ => {}
        }
    }
    if !super::super::prover::has_builtin_operators(program, &facts.operators, expression) {
        return false;
    }
    // The arithmetic walk observed every evaluation of this exact actual.
    // Its retained hull can describe an effectful call's completed result;
    // it does not establish that replaying that call has the same value.
    let complete_arguments =
        call_values::captured_argument_expressions(program, facts, state, call)
            .zip(validation::call_target_parameters(program, target))
            .is_some_and(|(arguments, parameters)| arguments.len() == parameters.len());
    if complete_arguments
        && super::super::argument_intervals::proves(
            program,
            expression,
            expression_intervals,
            &|parameter| {
                let name = program.expression_table.name_path_node(parameter)?;
                if !name.symbol.is_valid()
                    || name.symbol != name.head_symbol
                    || program
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        != 1
                {
                    return None;
                }
                let declared = validation::canonical_place_from_expression(program, parameter)?;
                let actual = actual_for_formal_place(
                    program, facts, state, call, contract, target, &declared,
                )?;
                // A value formal receives the completed scalar snapshot.
                // A reference formal instead observes live referee contents;
                // its argument-time hull cannot certify later invocation data.
                program
                    .primitive_type_reference(actual.parameter.type_reference)
                    .filter(|primitive| primitive.accepts_integer_literal())?;
                actual.argument
            },
        )
    {
        return true;
    }
    matches!(
        super::super::prover::evaluate_for_atoms(
            program,
            expression,
            &mut |_| None,
            &|_, _| true,
            &mut |expression| {
                if let Some((predicate, subject)) =
                    super::super::prover::place_predicate(program, expression)
                {
                    let declared = validation::canonical_place_from_expression(program, subject)?;
                    let actual = actual_for_formal_place(
                        program, facts, state, call, contract, target, &declared,
                    )?;
                    let values =
                        call_values::captured_values(program, facts, state, call, &actual)?;
                    if values.is_empty() {
                        return None;
                    }
                    for value in values {
                        let contexts = crate::flow::operand_contexts(
                            program,
                            &facts.flow,
                            &facts.semantic,
                            value.operand,
                            value.type_reference,
                            call.entry_constraints,
                        );
                        let mut place = validation::canonical_place_from_expression_in_state(
                            program,
                            state.state_symbol,
                            call.statement_index,
                            value.projection.expression,
                        )?;
                        place.extend_segments(&value.projection.remaining);
                        if !predicate.proven(program, facts, state, call, &contexts, &place) {
                            return None;
                        }
                    }
                    return Some(true);
                }
                prove_scalar(expression)
            },
        ),
        Some(facts::ScalarValue::Boolean(true))
    )
}

fn actual_for_formal_place<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contract: &checked_trees::ContractProofFact,
    target: symbols::SymbolHandle,
    declared: &validation::CanonicalPlace,
) -> Option<call_values::Actual<'program>> {
    let symbol = declared.root.as_symbol()?;
    let position = crate::semantic::facts::contract_parameter_position(
        program,
        &facts.proof,
        contract,
        target,
        symbol,
    )
    .ok()??;
    call_values::actual_at(program, state, call, position, declared.segments.clone())
}

/// The retained contract row supplies its authored telescope. Instantiating its
/// types never substitutes spelling, body parameters, or another satisfied row
/// for its declaration symbols.
fn bound_contract_predicate(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contract: &checked_trees::ContractProofFact,
    target: symbols::SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<crate::facts::boolean_predicates::BoundPredicate> {
    use checked_trees::ContractProofFactOwner;
    let (owner, declared) = if let Some(scope) = contract.inherited_scope {
        let scope = facts.proof.inherited_contract_scopes.get(scope);
        (scope.requirement, scope.requirement)
    } else {
        match contract.owner {
            ContractProofFactOwner::Machine { machine_symbol } => (machine_symbol, machine_symbol),
            ContractProofFactOwner::MachineState {
                machine_symbol,
                state_symbol,
            } => (machine_symbol, state_symbol),
            ContractProofFactOwner::StateSignature { state_symbol, .. } => {
                (state_symbol, state_symbol)
            }
            _ => return None,
        }
    };
    let actual = validation::call_target_parameters(program, target)?;
    let parameters = validation::call_target_parameters(program, declared)?
        .iter()
        .map(|parameter| {
            let position = crate::semantic::facts::contract_parameter_position(
                program,
                &facts.proof,
                contract,
                target,
                parameter.symbol,
            )
            .ok()??;
            let mut parameter = parameter.clone();
            parameter.type_reference = actual.get(position)?.type_reference;
            Some(parameter)
        })
        .collect::<Option<Vec<_>>>()?;
    let predicate = crate::values::lower_parameter_boolean_expression(
        program,
        &facts.operators,
        owner,
        &parameters,
        expression,
        &facts.operators.exact_integer_casts,
    )?;
    crate::facts::boolean_predicates::bind_parameters(
        program,
        owner,
        declared,
        &parameters,
        predicate,
    )
}

fn source_substitution_preserves_captures(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &Fact,
) -> Option<bool> {
    let FactPayload::ContractBooleanExpression {
        expression,
        fact: source_fact,
        ..
    } = requirement.payload
    else {
        return None;
    };
    let contract_call = crate::flow::proof_contract_call(
        &facts.proof,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let mut occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        expression,
        &mut occurrences,
    );
    let invocation = facts.flow.control.operands.span(call.operands)?.last();
    for occurrence in occurrences {
        let declared = validation::canonical_place_from_expression(program, occurrence)?;
        let Some(symbol) = declared.root.as_symbol() else {
            continue;
        };
        let mut position = None;
        let mut found = false;
        for reference in facts
            .proof
            .contract_fact_refs
            .span(contract_call.requires)?
        {
            let contract = facts.proof.contract_facts.get(reference.fact);
            if contract.fact != source_fact {
                continue;
            }
            let selected = crate::semantic::facts::contract_parameter_position(
                program,
                &facts.proof,
                contract,
                contract_call.target_state_symbol,
                symbol,
            )
            .ok()?;
            if found && position != selected {
                return None;
            }
            position = selected;
            found = true;
        }
        if !found {
            return None;
        }
        let Some(position) = position else { continue };
        let actual = call_values::actual_at(program, state, call, position, declared.segments)?;
        let values = call_values::captured_values(program, facts, state, call, &actual)?;
        if values.is_empty() {
            return None;
        }
        for value in values {
            if !capture_preserves_source(
                program,
                facts,
                state,
                call,
                value.observation,
                value.observation_type,
                &value.observation_projection.remaining,
                invocation?,
            )? {
                return Some(false);
            }
        }
    }
    Some(true)
}

/// Whole constructed observations cover their already-evaluated children.
/// Stored observations select their already-produced RHS captures; no
/// initializer evaluation can manufacture a replacement epoch.
fn capture_preserves_source(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    capture: &checked_trees::FlowOperandFact,
    reference: typed_trees::types::TypeReferenceHandle,
    remaining: &[facts::PlaceSegment],
    invocation: &checked_trees::FlowOperandFact,
) -> Option<bool> {
    if remaining.is_empty() && !capture.children.is_empty() {
        for child in facts.flow.control.operand_children.span(capture.children)? {
            if !facts.flow.control.operands.is_valid(child.value) {
                return None;
            }
            if !capture_preserves_source(
                program,
                facts,
                state,
                call,
                facts.flow.control.operands.get(child.value),
                child.type_reference,
                &[],
                invocation,
            )? {
                return Some(false);
            }
        }
        return Some(true);
    }
    if crate::flow::operand_binds_live_view(program, reference) {
        // Referent contents are invocation-live; construction selected its
        // address and descriptor geometry earlier. Authored source substitution
        // is valid only when those independent binding/selector epochs agree.
        return reference_construction_preserves_source(
            program, facts, state, capture, remaining, invocation,
        );
    }
    // Expression-rooted temporaries can still depend on earlier caller reads.
    // Their actual dependencies below, not root category, establish whether
    // source substitution preserves the completed value.
    if !computed_values_have_source_identity(program, facts, capture.expression) {
        // A returned loan is the storage it names, not a fresh computed value.
        // Field facts on that storage stay subject to ordinary substitution.
        if exact_returned_loan(
            program,
            state.state_symbol,
            call.statement_index,
            capture.expression,
        ) {
            return Some(true);
        }
        return Some(false);
    }
    let mut observations = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        capture.expression,
        &mut observations,
    );
    for segment in remaining {
        if let facts::PlaceSegment::Index { expression } = segment {
            crate::facts::contract_occurrences::append_expression_occurrences(
                program,
                *expression,
                &mut observations,
            );
        }
    }
    for observation in observations {
        let mut place = validation::canonical_place_from_expression_in_state(
            program,
            state.state_symbol,
            call.statement_index,
            observation,
        )?;
        if observation == capture.expression {
            place.extend_segments(remaining);
        }
        if !crate::flow::operand_observations_agree(
            program,
            &facts.flow,
            capture,
            invocation,
            &place,
        ) {
            return Some(false);
        }
    }
    Some(true)
}

/// One shared source-coordinate walk distinguishes captured reference/descriptor
/// bindings and address selectors from their invocation-live referent contents.
/// A later payload write may change content facts, but cannot donate a new
/// selector or descriptor epoch to an already completed operand.
fn reference_construction_preserves_source(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: &checked_trees::FlowOperandFact,
    remaining: &[facts::PlaceSegment],
    invocation: &checked_trees::FlowOperandFact,
) -> Option<bool> {
    if !computed_values_have_source_identity(program, facts, capture.expression) {
        if exact_returned_loan(
            program,
            state.state_symbol,
            capture.statement_index,
            capture.expression,
        ) {
            return Some(true);
        }
        return Some(false);
    }
    let mut selectors = Vec::new();
    crate::facts::contract_occurrences::append_selector_occurrences(
        program,
        capture.expression,
        &mut selectors,
    );
    for segment in remaining {
        if let facts::PlaceSegment::Index { expression } = segment {
            crate::facts::contract_occurrences::append_expression_occurrences(
                program,
                *expression,
                &mut selectors,
            );
        }
    }
    for selector in selectors {
        let place = validation::canonical_place_from_expression_in_state(
            program,
            state.state_symbol,
            capture.statement_index,
            selector,
        )?;
        if !crate::flow::operand_observations_agree(
            program,
            &facts.flow,
            capture,
            invocation,
            &place,
        ) {
            return Some(false);
        }
    }
    for place in super::super::capture_coordinates::borrowed_bindings(program, capture, remaining)?
    {
        if !crate::flow::operand_bindings_agree(program, &facts.flow, capture, invocation, &place) {
            return Some(false);
        }
    }

    Some(true)
}

/// One exact returned reference to parameter storage. The callee projects a
/// loan; it does not produce a value whose epoch replaces the referent.
fn exact_returned_loan(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(places) = crate::flow::reference_expression_storage_places_including_shared(
        program,
        state_symbol,
        statement_index,
        expression,
        None,
    ) else {
        return false;
    };
    let [place] = places.as_slice() else {
        return false;
    };
    place
        .root
        .as_symbol()
        .is_some_and(|symbol| symbol.is_valid())
        && place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
            )
        })
        && loan_call_does_not_write(program, state_symbol, expression)
}

/// The returned loan contributes no write of its own. A callee that stores
/// through the referent before returning it changes the facts substitution
/// would otherwise read, and an opaque write frame is not an empty one.
fn loan_call_does_not_write(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let mut owned = None;
    let Some(frames) = crate::flow::shared_call_frames_or(None, program, &mut owned) else {
        return false;
    };
    let Some((machine, _)) = validation::find_state(program, state_symbol) else {
        return false;
    };
    matches!(
        frames.expression_may_write_paths(machine, expression),
        Some(paths) if paths.is_empty()
    )
}

/// Runtime invocations are distinct observations unless the existing checked
/// denotation owner establishes value-only normal returns. An empty write
/// frame alone does not make a call repeatable. This stage already holds
/// fact-call projections, so those calls need no second identity derivation.
pub(super) fn computed_values_have_source_identity(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let plans = validation::operand_call_plans(program, None);
    validation::expression_values_have_call_identity(
        program,
        expression,
        &plans.0,
        &plans.1,
        &|expression| {
            facts
                .fact_call_projections
                .iter()
                .any(|projection| projection.call_expression == expression)
        },
    )
}
