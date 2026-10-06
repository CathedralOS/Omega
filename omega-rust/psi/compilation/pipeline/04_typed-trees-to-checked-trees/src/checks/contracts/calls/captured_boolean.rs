//! Source substitution is valid only while it denotes the captured operand.
//! A copied constructor child can precede a write by a sibling or later actual.
//! Such a predicate must use its retained capture; syntax-only proof routes may
//! not reinterpret both observations as the invocation's final storage value.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::{Fact, FactPayload, PlaceRoot};
use typed_trees::TypedTrees;

use super::super::call_values;

/// `None` permits the ordinary source-substitution proof routes. `Some` is a
/// capture-owned judgment, including failure: no later fallback may replace it.
pub(super) fn judgment(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    requirement: &Fact,
) -> Option<bool> {
    let FactPayload::ContractBooleanExpression { expression, .. } = requirement.payload else {
        return None;
    };
    if source_substitution_preserves_captures(program, facts, state, call, requirement)
        == Some(true)
    {
        return None;
    }
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
            ) {
                return Some(false);
            }
        }
        Some(seen)
    })();
    Some(proven.unwrap_or(false))
}

/// Reuse the ordinary Boolean evaluator with evidence-owned atoms. Scalar
/// relations keep their paired capture epochs; semantic place predicates ask
/// their existing owner using only the selected subject's contexts.
#[allow(clippy::too_many_arguments)]
fn captured_contract_proven(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    contract: &checked_trees::ContractProofFact,
    target: symbols::SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let prove_scalar = |expression| {
        let predicate = bound_contract_predicate(program, facts, contract, target, expression)?;
        call_values::proves_bound_predicate(
            program,
            facts,
            state,
            call,
            contexts,
            &predicate,
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
    if prove_scalar(expression) == Some(true) {
        return true;
    }
    if !super::super::prover::has_builtin_operators(program, &facts.operators, expression) {
        return false;
    }
    matches!(
        super::super::prover::evaluate_with_atoms(
            program,
            expression,
            &mut |_| None,
            &|_, _| true,
            &mut |expression| {
                if let Some((predicate, subject)) =
                    super::super::prover::place_predicate(program, expression)
                {
                    let declared = crate::flow::canonical_place_from_expression(program, subject)?;
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
                        let mut place = crate::flow::canonical_place_from_expression_in_state(
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
    program: &'program TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contract: &checked_trees::ContractProofFact,
    target: symbols::SymbolHandle,
    declared: &crate::flow::CanonicalPlace,
) -> Option<call_values::Actual<'program>> {
    let PlaceRoot::Symbol(symbol) = declared.root else {
        return None;
    };
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
    program: &TypedTrees,
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
    let actual = crate::semantic::calls::call_target_parameters(program, target)?;
    let parameters = crate::semantic::calls::call_target_parameters(program, declared)?
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
    program: &TypedTrees,
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
        let declared = crate::flow::canonical_place_from_expression(program, occurrence)?;
        let PlaceRoot::Symbol(symbol) = declared.root else {
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
                value.operand,
                value.type_reference,
                &value.projection.remaining,
                invocation?,
            )? {
                return Some(false);
            }
        }
    }
    Some(true)
}

/// Whole constructed observations cover their already-evaluated children.
/// Stored aggregate observations retain the single existing storage capture;
/// this never follows an initializer to manufacture child epochs.
fn capture_preserves_source(
    program: &TypedTrees,
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
        // A borrowed child denotes the invocation-live referent.
        return Some(true);
    }
    // A whole capture rooted in its own expression is a closed temporary: the
    // children branch above already consumed every projectable capture, so an
    // empty path here names a value with no caller storage behind it.
    // Substitution names that expression, not a place the caller still reads,
    // so no later operand can write what the capture observed and there is no
    // source to re-observe. `copied_guarantees::observation_survives` states
    // the same rule for expression-rooted sources. This judgment also holds no
    // place fact about such a value, so claiming it would refuse every callee
    // guarantee rather than decide one.
    if remaining.is_empty()
        && matches!(
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.state_symbol,
                call.statement_index,
                capture.expression,
            )
            .map(|place| place.root),
            Some(PlaceRoot::Expression(_))
        )
    {
        return Some(true);
    }
    if !computed_values_have_source_identity(program, facts, capture.expression) {
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
        let mut place = crate::flow::canonical_place_from_expression_in_state(
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

/// Runtime invocations are distinct observations unless the existing checked
/// denotation owner establishes value-only normal returns. An empty write
/// frame alone does not make a call repeatable. This stage already holds
/// fact-call projections, so those calls need no second identity derivation.
fn computed_values_have_source_identity(
    program: &TypedTrees,
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
