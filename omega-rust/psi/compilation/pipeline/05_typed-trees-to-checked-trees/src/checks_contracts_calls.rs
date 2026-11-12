/*
    Call requirement checking: we prove each call's authored `requires` clauses
    at the call site from captured boolean premises, guard observations,
    predicate domains, transition guards and state preservation, and explain
    the missing fact when a requirement cannot be established.
*/

mod boolean_requirements {
    /*
        Prove a Boolean arrival requirement at one exact call occurrence.

        Each conjunct may use a different existing proof route. No child is installed
        as a premise: all queries keep the same substituted operands and live context.
    */
    use super::super::call_bounds::proves_in_context;
    use super::super::current_values::proves_call;
    use super::super::entailment::structural_call_requirement;
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle};

    use super::super::places::expression_is_boolean_place_like;
    use super::super::prover::{
        call_entry_contexts_prove_boolean_contract_expression, closed_boolean_value,
    };
    use super::{incoming_guard_proves_requires, transition_guard_proves_boolean_requirement};

    /*
        A collection-predicate requirement over a field of the caller's argument
        place is the same fact the place's data states in `requires`; it stands at
        every call because calls are consumption points for invariant windows.
    */
    fn standing_domain_fact_proves_requirement(
        program: &dyn ::validation::CheckingInput,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        expression: ExpressionHandle,
    ) -> bool {
        let Some((caller, caller_state, call_site, parameters)) =
            super::call_coordinates(program, state_flow, call_flow)
        else {
            return false;
        };
        let Some(callee) = program
            .machine_by_symbol(call_flow.target_symbol)
            .or_else(|| {
                validation::find_machine_by_entry_state(program, call_flow.target_symbol)
                    .map(|(machine, _)| machine)
            })
        else {
            return false;
        };
        validation::standing_facts_prove_requirement(
            program,
            caller,
            caller_state,
            callee.symbol,
            parameters,
            validation::call_site_argument_expressions(program, &call_site),
            expression,
        )
    }

    /*
        A call binding every parameter positionally substitutes the subject
        argument too (`Main::fill(main, 2, node)`): the instantiated
        judgment — the only channel that substitutes `self` — then decides
        the requires on the bound values alone. Covers every fully-bound
        call site, any callee state, guarded or not; receiver-form calls
        bind `self` implicitly and never reach full arity here.
    */
    fn explicit_subject_call_requirement_proven(
        program: &dyn ::validation::CheckingInput,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        expression: ExpressionHandle,
    ) -> bool {
        let Some((machine, state, site, callee_parameters)) =
            super::call_coordinates(program, state_flow, call_flow)
        else {
            return false;
        };
        let arguments = validation::call_site_argument_expressions(program, &site);
        if arguments.len() != callee_parameters.len() {
            return false;
        }
        validation::instantiated_value_call_requires_proven(
            program,
            machine,
            state,
            call_flow.statement_index,
            site,
            call_flow.target_symbol,
            callee_parameters,
            arguments,
            expression,
        )
    }

    pub(super) fn proves(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        entry_contexts: &[facts::FactContextHandle],
        expression: ExpressionHandle,
        incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        completed: &mut crate::checks::current_scalar_values::CompletedCallGuarantees,
    ) -> bool {
        let proven = closed_boolean_value(program, &facts.operators, expression) == Some(true)
            || super::super::call_bounds::proves(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                expression,
                None,
            )
            || call_entry_contexts_prove_boolean_contract_expression(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                expression,
                call_frames,
            )
            || proves_in_context(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                expression,
                call_frames,
            )
            || structural_call_requirement(
                program,
                facts,
                state_flow,
                call_flow,
                expression,
                None,
                call_frames,
            )
            || explicit_subject_call_requirement_proven(program, state_flow, call_flow, expression)
            || if !expression_is_boolean_place_like(program, expression) {
                incoming_guard_proves_requires(
                    program,
                    facts,
                    state_flow,
                    call_flow,
                    expression,
                    incoming_guards,
                    call_frames,
                )
            } else {
                false
            }
            || transition_guard_proves_boolean_requirement(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                expression,
                call_frames,
            )
            || super::contradictory_edge::proves(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                call_frames,
            )
            || standing_domain_fact_proves_requirement(program, state_flow, call_flow, expression)
            || proves_call(
                program,
                state_flow,
                call_flow,
                expression,
                call_frames,
                completed,
            );
        if proven {
            return true;
        }
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return false;
        };
        binary.operator == BinaryOperator::And
            && [binary.left, binary.right].into_iter().all(|conjunct| {
                proves(
                    program,
                    facts,
                    state_flow,
                    call_flow,
                    entry_contexts,
                    conjunct,
                    incoming_guards,
                    call_frames,
                    &mut *completed,
                )
            })
    }
}
mod captured_boolean {
    /*
        Source substitution is valid only while it denotes the captured operand.
        A copied constructor child can precede a write by a sibling or later actual.
        Such a predicate must use its retained capture; syntax-only proof routes may
        not reinterpret both observations as the invocation's final storage value.
    */
    use crate::checks::float_implications::contexts_prove_comparison;
    use crate::checks::operators::InvocationContexts;
    use crate::checks::operators::closed_operators_are_builtin;
    use crate::checks::operators::selected_float_boolean_requirement;
    use crate::published::facts::boolean_predicates::BoundPredicate;
    use crate::published::facts::boolean_predicates::bind_parameters;
    use crate::published::facts::contract_occurrences::append_expression_occurrences;
    use crate::published::facts::contract_occurrences::append_selector_occurrences;
    use crate::published::places::place_at;
    use crate::published::semantic::facts::contract_parameter_position;
    use crate::values::lower_callable_scalar_contract_predicate;
    use crate::values::lower_parameter_boolean_expression;
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use facts::{Fact, FactPayload};

    use super::super::call_values;

    /*
        `None` permits the ordinary source-substitution proof routes. `Some` is a
        capture-owned judgment, including failure: no later fallback may replace it.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn judgment<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        guarantees: &mut crate::checks::contracts::prover::call_guarantees::StateGuarantees<'_, '_>,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        requirement: &Fact,
        incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
        frames: Option<&validation::CallFrameResolver<'_>>,
        whole_contract_cache: &mut super::WholeContractCache<'program>,
    ) -> Option<bool> {
        let state = guarantees.caller();
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
            let contract_call = facts.proof.contract_call_at(&(
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ))?;
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
                    guarantees,
                    call,
                    contexts,
                    contract,
                    reference.fact,
                    contract_call.target_state_symbol,
                    expression,
                    incoming_guards,
                    frames,
                    whole_contract_cache,
                ) {
                    return Some(false);
                }
            }
            Some(seen)
        })();
        if proven == Some(true)
            || (proven != Some(false)
                && captured_float_requirement(program, facts, state, call, expression)
                    == Some(true))
        {
            return Some(true);
        }
        if source_substitution_preserves_captures(program, facts, state, call, requirement)
            == Some(true)
        {
            None
        } else {
            Some(false)
        }
    }

    /*
        A selected semantic equation describes this completed operand, not a replay
        of its runtime call. Closed terms and complete Integer carrier images can
        establish a Boolean consequence without claiming the Float value has
        substitution identity. Unknown projections remain unknown: they cannot read
        current mutable storage or inherit a different operand's classification.
    */
    fn captured_float_requirement(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<bool> {
        let (machine, target_state) = validation::find_state(program, call.target_symbol)
            .map(|(machine, state)| (machine.symbol, state.symbol))
            .or_else(|| {
                let machine = program.machine_by_symbol(call.target_symbol)?;
                Some((
                    machine.symbol,
                    program.machine_states(machine).first()?.symbol,
                ))
            })
            .unwrap_or((call.target_symbol, symbols::SymbolHandle::invalid()));
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        let arguments = call_values::captured_argument_expressions(program, facts, state, call)?;
        let contexts = InvocationContexts::from_call(facts, call, &arguments);
        if closed_operators_are_builtin(program, &facts.operators, expression)
            && super::super::prover::evaluate_for_atoms(
                program,
                expression,
                &mut |_| None,
                &|_, _| true,
                &mut |leaf| {
                    contexts_prove_comparison(
                        program,
                        &facts.semantic,
                        &facts.operators,
                        &contexts,
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
        selected_float_boolean_requirement(
            program,
            facts,
            machine,
            target_state,
            expression,
            parameters,
            &arguments,
            state.machine_symbol,
            state.state_symbol,
        )
    }

    /*
        Reuse the ordinary Boolean evaluator with evidence-owned atoms. Scalar
        relations keep their paired capture epochs; semantic place predicates ask
        their existing owner using only the selected subject's contexts.
    */
    #[allow(clippy::too_many_arguments)]
    fn captured_contract_proven<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        guarantees: &mut crate::checks::contracts::prover::call_guarantees::StateGuarantees<'_, '_>,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        contract: &checked_trees::ContractProofFact,
        contract_handle: arena::Handle<checked_trees::ContractProofFact>,
        target: symbols::SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
        incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
        frames: Option<&validation::CallFrameResolver<'_>>,
        whole_contract_cache: &mut super::WholeContractCache<'program>,
    ) -> bool {
        let state = guarantees.caller();
        if let typed_trees::domain::ProofFact::Expression(required) =
            program.proof_facts.get(contract.fact)
        {
            let whole_proven = if let Some(&(_, cached)) = whole_contract_cache
                .answers
                .iter()
                .find(|&&(handle, _)| handle == contract_handle)
            {
                cached
            } else {
                let result = super::super::prover::call_guarantees::proves(
                    program, facts, guarantees, call, contexts, *required, frames,
                ) || proves_current_call(
                    whole_contract_cache,
                    program,
                    state,
                    call,
                    *required,
                    frames,
                    guarantees.completed_calls(),
                ) || super::super::entailment::structural_call_requirement(
                    program,
                    facts,
                    state,
                    call,
                    *required,
                    Some(contract_handle),
                    frames,
                );
                whole_contract_cache.answers.push((contract_handle, result));
                result
            };
            if whole_proven {
                return true;
            }
        }
        captured_expression_proven(
            program,
            facts,
            state,
            call,
            contexts,
            contract,
            target,
            expression,
            incoming_guards,
            frames,
            guarantees.completed_calls(),
            whole_contract_cache,
        )
    }

    // A mixed contract can need a different proof owner for each clause. Reuse
    // the caller's completed values without substituting a recast reference's source.
    #[allow(clippy::too_many_arguments)]
    fn proves_current_call<'program>(
        cache: &super::WholeContractCache<'program>,
        program: &'program dyn ::validation::CheckingInput,
        state: &FlowStateFact,
        call: &FlowCallFact,
        expression: typed_trees::expression::ExpressionHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
        completed: &mut crate::checks::current_scalar_values::CompletedCallGuarantees,
    ) -> bool {
        cache
            .current_values
            .get_or_init(|| {
                super::super::current_values::prepare_call(program, state, call, frames, completed)
            })
            .as_ref()
            .is_some_and(|values| {
                super::super::current_values::proves_prepared_call(values, state, call, expression)
            })
    }

    /*
        Compose expression-dependent evidence after the whole-contract attempt.
        Children retain the exact same captured context and fallback order.
    */
    #[allow(clippy::too_many_arguments)]
    fn captured_expression_proven<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        contract: &checked_trees::ContractProofFact,
        target: symbols::SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
        incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
        frames: Option<&validation::CallFrameResolver<'_>>,
        completed: &mut crate::checks::current_scalar_values::CompletedCallGuarantees,
        whole_contract_cache: &mut super::WholeContractCache<'program>,
    ) -> bool {
        // One clause may need its saved value while another can safely read an
        // unchanged scalar. Check source identity separately for each clause;
        // success here is never stored as a whole-contract answer.
        captured_value_expression_proven(
            program,
            facts,
            state,
            call,
            contexts,
            contract,
            target,
            expression,
            incoming_guards,
            frames,
            &mut *completed,
            whole_contract_cache,
        ) || (expression_substitution_preserves_captures(
            program,
            facts,
            state,
            call,
            expression,
            contract.fact,
        ) == Some(true)
            && super::boolean_requirements::proves(
                program,
                facts,
                state,
                call,
                contexts,
                expression,
                incoming_guards,
                frames,
                completed,
            ))
    }

    #[allow(clippy::too_many_arguments)]
    fn captured_value_expression_proven<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        contract: &checked_trees::ContractProofFact,
        target: symbols::SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
        incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
        frames: Option<&validation::CallFrameResolver<'_>>,
        completed: &mut crate::checks::current_scalar_values::CompletedCallGuarantees,
        whole_contract_cache: &mut super::WholeContractCache<'program>,
    ) -> bool {
        if call_values::proves_captured_arithmetic_requirement(
            program, facts, state, call, expression,
        ) {
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
        if let Some(binary) = program.expression_table.binary_node(expression) {
            use typed_trees::expression::BinaryOperator;
            let mut child = |expression| {
                proves_current_call(
                    whole_contract_cache,
                    program,
                    state,
                    call,
                    expression,
                    frames,
                    &mut *completed,
                ) || captured_expression_proven(
                    program,
                    facts,
                    state,
                    call,
                    contexts,
                    contract,
                    target,
                    expression,
                    incoming_guards,
                    frames,
                    &mut *completed,
                    whole_contract_cache,
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
        let complete_arguments =
            call_values::captured_argument_expressions(program, facts, state, call)
                .zip(validation::call_target_parameters(program, target))
                .is_some_and(|(arguments, parameters)| arguments.len() == parameters.len());
        if complete_arguments
            && super::super::argument_intervals::proves(program, expression, &|parameter| {
                let name = program.expression_table.name_path_node(parameter)?;
                if !program.expression_table.name_path_is_single_member(name) {
                    return None;
                }
                let declared = validation::canonical_place_from_expression(program, parameter)?;
                let actual = actual_for_formal_place(
                    program, facts, state, call, contract, target, &declared,
                )?;
                program
                    .primitive_type_reference(actual.parameter.type_reference)
                    .filter(|primitive| primitive.accepts_integer_literal())?;
                actual.argument
            })
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
                        let declared =
                            validation::canonical_place_from_expression(program, subject)?;
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
                                &facts.flow.contexts,
                                &facts.flow.control,
                                &facts.semantic,
                                value.operand.row(),
                                value.type_reference,
                                value.producer_invocation(call.entry_constraints),
                            );
                            if crate::flow::operand_binds_live_view(program, value.type_reference)
                                && value.operand.referents.is_empty()
                                && !matches!(
                                    value.operand.completion,
                                    checked_trees::FlowOperandCompletion::Copied { .. }
                                )
                            {
                                return None;
                            }
                            if !predicate
                                .proven_for_capture(program, facts, state, call, &contexts, &value)
                            {
                                let projection = call_case_projection(
                                    program,
                                    facts,
                                    state,
                                    call,
                                    contract,
                                    target,
                                    &declared.segments,
                                    frames,
                                )?;
                                if !predicate.proven_for_call_projection(
                                    program,
                                    facts,
                                    state,
                                    call,
                                    &contexts,
                                    &value,
                                    &declared.segments,
                                    &projection,
                                ) {
                                    return None;
                                }
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
        let position =
            contract_parameter_position(program, &facts.proof, contract, target, symbol).ok()??;
        call_values::actual_at(program, state, call, position, declared.segments.clone())
    }

    /*
        The retained contract row supplies its authored telescope. Instantiating its
        types never substitutes spelling, body parameters, or another satisfied row
        for its declaration symbols.
    */
    fn bound_contract_predicate(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contract: &checked_trees::ContractProofFact,
        target: symbols::SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<BoundPredicate> {
        let (owner, declared, parameters) = contract_parameters(program, facts, contract, target)?;
        let predicate = lower_parameter_boolean_expression(
            program,
            &facts.operators,
            owner,
            &parameters,
            expression,
            &facts.operators.exact_integer_casts,
        )
        .or_else(|| {
            let mut remaining = 256;
            lower_callable_scalar_contract_predicate(
                program,
                &facts.operators,
                owner,
                &parameters,
                typed_trees::types::TypeReferenceHandle::invalid(),
                expression,
                false,
                &mut remaining,
            )
        })?;
        bind_parameters(program, owner, declared, &parameters, None, predicate)
    }

    fn contract_parameters(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contract: &checked_trees::ContractProofFact,
        target: symbols::SymbolHandle,
    ) -> Option<(
        symbols::SymbolHandle,
        symbols::SymbolHandle,
        Vec<typed_trees::signature::StateParameter>,
    )> {
        use checked_trees::ContractProofFactOwner;
        let (owner, declared) = if let Some(scope) = contract.inherited_scope {
            let scope = facts.proof.inherited_contract_scopes.get(scope);
            (scope.requirement, scope.requirement)
        } else {
            match contract.owner {
                ContractProofFactOwner::Machine { machine_symbol } => {
                    (machine_symbol, machine_symbol)
                }
                ContractProofFactOwner::MachineState {
                    machine_symbol,
                    state_symbol,
                } => (machine_symbol, state_symbol),
                ContractProofFactOwner::StateSignature { state_symbol, .. }
                | ContractProofFactOwner::CallSignature { state_symbol, .. } => {
                    (state_symbol, state_symbol)
                }
                _ => return None,
            }
        };
        let actual = validation::call_target_parameters(program, target)?;
        let declared_parameters =
            if let ContractProofFactOwner::CallSignature { .. } = contract.owner {
                let signature = crate::published::proof::contracts::call_signatures::signature(
                    &facts.proof,
                    contract.owner,
                )?;
                program.state_signature_parameters(signature)
            } else {
                validation::call_target_parameters(program, declared)?
            };
        let parameters = declared_parameters
            .iter()
            .map(|parameter| {
                let position = contract_parameter_position(
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
        Some((owner, declared, parameters))
    }

    // Keep the saved collection's identity and observation point. Check the
    // reads used by each index expression before replacing the callee's
    // parameters with the captured caller arguments.
    fn call_case_projection(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contract: &checked_trees::ContractProofFact,
        target: symbols::SymbolHandle,
        path: &[facts::PlaceSegment],
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<crate::checks::current_scalar_values::CallArgumentProjection> {
        // Report the first unavailable input without repeating a proof query.
        macro_rules! projection_input {
            ($stage:literal, $value:expr) => {
                match $value {
                    Some(value) => value,
                    None => {
                        if validation::trace_flags::check_sites() {
                            eprintln!(
                                "CHECKSITE call-case-projection stage={} machine={:?} state={:?} statement={} call={} target={:?}",
                                $stage, state.machine_symbol, state.state_symbol,
                                call.statement_index, call.call_ordinal, target,
                            );
                        }
                        return None;
                    }
                }
            };
        }
        let (_, _, parameters) = projection_input!(
            "contract-telescope",
            contract_parameters(program, facts, contract, target)
        );
        let actuals = projection_input!(
            "argument-roster",
            call_values::captured_argument_expressions(program, facts, state, call)
        );
        let invocation = projection_input!(
            "invocation-operand",
            facts
                .flow
                .control
                .operands
                .span(call.operands)
                .and_then(|rows| rows.last())
        );
        let mut arguments = Vec::with_capacity(parameters.len());
        for parameter in &parameters {
            let position = projection_input!(
                "formal-position",
                contract_parameter_position(
                    program,
                    &facts.proof,
                    contract,
                    target,
                    parameter.symbol,
                )
                .ok()
                .flatten()
            );
            arguments.push(*projection_input!("actual-position", actuals.get(position)));
        }
        let mut reads = Vec::new();
        for segment in path {
            if let facts::PlaceSegment::Index { expression } = segment {
                append_expression_occurrences(program, *expression, &mut reads);
            }
        }
        for read in reads {
            let declared = projection_input!(
                "selector-read-place",
                validation::canonical_place_from_expression(program, read)
            );
            let symbol = projection_input!("selector-read-root", declared.root.as_symbol());
            let Some(position) = projection_input!(
                "selector-formal-position",
                contract_parameter_position(program, &facts.proof, contract, target, symbol).ok()
            ) else {
                continue;
            };
            let actual = projection_input!(
                "selector-actual",
                call_values::actual_at(program, state, call, position, declared.segments)
            );
            let captures = projection_input!(
                "selector-captures",
                call_values::captured_values(program, facts, state, call, &actual)
            );
            projection_input!(
                "nonempty-selector-captures",
                (!captures.is_empty()).then_some(())
            );
            for capture in captures {
                projection_input!(
                    "selector-source-preservation",
                    (capture_preserves_source(
                        program,
                        facts,
                        state,
                        call,
                        capture.observation.row(),
                        capture.observation_type,
                        &capture.observation_projection.remaining,
                        invocation,
                    ) == Some(true))
                    .then_some(())
                );
            }
        }
        let (machine, caller) = projection_input!(
            "caller-point",
            program.state_flow_site(state.machine_symbol, state.state_symbol)
        );
        let site = projection_input!(
            "call-site",
            validation::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )
        );
        let transition = validation::transition_call_target(
            program,
            machine,
            caller,
            call.statement_index,
            call.call_ordinal,
        )
        .filter(|target| target.is_valid() && site.completes_transition(program, *target))
        .unwrap_or_else(typed_trees::statement::TransitionTargetHandle::invalid);
        let mut values = projection_input!(
            "caller-scalar-inputs",
            crate::checks::current_scalar_values::CurrentScalarValues::at(
                program,
                machine,
                caller,
                call.statement_index,
                transition,
                frames,
            )
        );
        let snapshot = program.storage_graph().and_then(|graph| {
            validation::completed_storage::versions::invocation_snapshot(
                program,
                graph,
                caller.symbol,
                call.statement_index,
                call.call_ordinal,
            )
        });
        values.retire_unpreserved(snapshot);
        let (callee, entry) = projection_input!(
            "callee-point",
            validation::find_state(program, target)
                .or_else(|| validation::find_machine_head(program, target))
        );
        Some(projection_input!(
            "bound-index-recipe",
            values.call_argument_projection(path, callee, entry, &parameters, &arguments)
        ))
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
        expression_substitution_preserves_captures(
            program,
            facts,
            state,
            call,
            expression,
            source_fact,
        )
    }

    fn expression_substitution_preserves_captures(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        expression: typed_trees::expression::ExpressionHandle,
        source_fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> Option<bool> {
        let contract_call = facts.proof.contract_call_at(&(
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        ))?;
        let mut occurrences = Vec::new();
        append_expression_occurrences(program, expression, &mut occurrences);
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
                let selected = contract_parameter_position(
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
                    value.observation.row(),
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

    /*
        Whole constructed observations cover their already-evaluated children.
        Stored observations select their already-produced RHS captures; no
        initializer evaluation can manufacture a replacement epoch.
    */
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
        if remaining.is_empty() && !capture.constructed_children().is_empty() {
            for child in facts
                .flow
                .control
                .operand_children
                .span(capture.constructed_children())?
            {
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
        if source_read_changes_interpretation(program, state, capture, remaining) == Some(true) {
            return Some(false);
        }
        let source_reference = program
            .state_flow_site(state.machine_symbol, capture.state_symbol)
            .and_then(|(machine, state)| {
                validation::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    capture.expression,
                )
            });
        if crate::flow::operand_binds_live_view(program, reference)
            || source_reference
                .is_some_and(|source| crate::flow::operand_binds_live_view(program, source))
        {
            return reference_construction_preserves_source(
                program, facts, state, capture, remaining, invocation,
            );
        }
        if !computed_values_have_source_identity(program, facts, capture.expression) {
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
        append_expression_occurrences(program, capture.expression, &mut observations);
        for segment in remaining {
            if let facts::PlaceSegment::Index { expression } = segment {
                append_expression_occurrences(program, *expression, &mut observations);
            }
        }
        for observation in observations {
            let mut place = place_at(
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
                facts.flow.view(),
                capture,
                invocation,
                &place,
            ) {
                return Some(false);
            }
        }
        Some(true)
    }

    /*
        One shared source-coordinate walk distinguishes captured reference/descriptor
        bindings and address selectors from their invocation-live referent contents.
        A later payload write may change content facts, but cannot donate a new
        selector or descriptor epoch to an already completed operand.
    */
    fn reference_construction_preserves_source(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        capture: &checked_trees::FlowOperandFact,
        remaining: &[facts::PlaceSegment],
        invocation: &checked_trees::FlowOperandFact,
    ) -> Option<bool> {
        let (machine, captured_state) =
            program.state_flow_site(state.machine_symbol, capture.state_symbol)?;
        let statement = program
            .statement_table
            .statement_at(captured_state, capture.statement_index)?;
        let mut owned_frames = None;
        let frames = crate::flow::shared_call_frames_or(None, program, &mut owned_frames)?;
        let sources = frames.reference_expression_sources_before_statement(
            machine,
            statement,
            capture.expression,
        )?;
        let expected = validation::expression_result_type_reference(
            program,
            machine,
            captured_state,
            capture.expression,
        )
        .and_then(|reference| {
            validation::scalar_observation_meaning(program, reference).map(|meaning| meaning.0)
        });
        if sources.iter().any(|source| {
            if let Some(expected) = expected {
                !source
                    .scalar_storage_coordinate(program, captured_state)
                    .is_some_and(|scalar| {
                        scalar.preserves_scalar_value
                            && scalar.source_primitive == expected
                            && scalar.view_primitive == expected
                    })
            } else {
                !source.recasts.is_empty() || source.representation.is_some()
            }
        }) {
            return Some(false);
        }
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
        append_selector_occurrences(program, capture.expression, &mut selectors);
        for segment in remaining {
            if let facts::PlaceSegment::Index { expression } = segment {
                append_expression_occurrences(program, *expression, &mut selectors);
            }
        }
        for selector in selectors {
            let place = place_at(
                program,
                state.state_symbol,
                capture.statement_index,
                selector,
            )?;
            if !crate::flow::operand_observations_agree(
                program,
                facts.flow.view(),
                capture,
                invocation,
                &place,
            ) {
                return Some(false);
            }
        }
        for place in
            super::super::capture_coordinates::borrowed_bindings(program, capture, remaining)?
        {
            if !crate::flow::operand_bindings_agree(
                program,
                facts.flow.view(),
                capture,
                invocation,
                &place,
            ) {
                return Some(false);
            }
        }

        Some(true)
    }

    /*
        An owned scalar capture can originate in an implicit read through a view.
        Its backing may be stable while the read's numeric interpretation differs.
    */
    fn source_read_changes_interpretation(
        program: &dyn ::validation::CheckingInput,
        state: &FlowStateFact,
        capture: &checked_trees::FlowOperandFact,
        remaining: &[facts::PlaceSegment],
    ) -> Option<bool> {
        if !matches!(
            program.expression_table.expression(capture.expression),
            typed_trees::expression::ExpressionNode::Name(_)
                | typed_trees::expression::ExpressionNode::Member(_)
                | typed_trees::expression::ExpressionNode::Indexed(_)
        ) {
            return Some(false);
        }
        let (machine, captured_state) =
            program.state_flow_site(state.machine_symbol, capture.state_symbol)?;
        let mut place = validation::canonical_place_from_expression(program, capture.expression)?;
        place.extend_segments(remaining);
        let mut owned_frames = None;
        let frames = crate::flow::shared_call_frames_or(None, program, &mut owned_frames)?;
        frames.scalar_read_changes_interpretation(
            machine,
            captured_state,
            capture.expression,
            place.root.as_symbol()?,
            &place.segments,
        )
    }

    /*
        One exact returned reference to parameter storage. The callee projects a
        loan; it does not produce a value whose epoch replaces the referent.
    */
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
            && place
                .segments
                .iter()
                .all(crate::flow::fixed_selection_segment)
            && loan_call_does_not_write(program, state_symbol, expression)
    }

    /*
        The returned loan contributes no write of its own. A callee that stores
        through the referent before returning it changes the facts substitution
        would otherwise read, and an opaque write frame is not an empty one.
    */
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

    /*
        Runtime invocations are distinct observations unless the existing checked
        denotation owner establishes value-only normal returns. An empty write
        frame alone does not make a call repeatable. This stage already holds
        fact-call projections, so those calls need no second identity derivation.
    */
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
}
mod contradictory_edge {
    /*
        Discharge an arrival requirement on the arm no execution can reach.

        A conditional-entry assembly body normalizes its `jz`/`jnz` into one `When`
        transition plus the unguarded continuation that follows it, and each
        destination label's `requires` becomes an arrival obligation on the arm that
        delivers it. When the state's own premises already refute the guard -- `cmp
        eax, eax` always sets ZF, so the fallthrough cannot run -- the arm is
        infeasible: `A ∧ ¬G` has no model, so every requirement follows, including
        the literal `false`.

        A label keeps its meaning. This discharges a requirement only where the edge
        that delivers it cannot be taken, so an interior `jmp` to the same label is
        untouched and still owes that label's requirement in full.
    */
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use facts::ProgramPoint;
    use typed_trees::statement::{StatementNode, TransitionGuardNode};

    /*
        The guard this call's statement is the refuted arm of -- the negation of the
        immediately preceding `When` transition. Only the continuation of a dispatch
        the previous statement opens qualifies, so the guard was evaluated on this
        edge with no statement between it and the arrival.
    */
    fn refuted_edge_guard(
        program: &dyn ::validation::CheckingInput,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        let (_, state) = validation::machine_state_by_symbols(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        )?;
        let statements = program.statement_table.statements(state.statement_nodes);
        let StatementNode::Transition(continuation) = statements.get(call_flow.statement_index)?
        else {
            return None;
        };
        if !matches!(continuation.guard, TransitionGuardNode::Always) {
            return None;
        }
        let index = call_flow.statement_index.checked_sub(1)?;
        let StatementNode::Transition(selected) = statements.get(index)? else {
            return None;
        };
        let TransitionGuardNode::When(guard) = selected.guard else {
            return None;
        };
        (guard.is_valid() && !selected.continuation.is_valid()).then_some(guard)
    }

    /*
        `A ∧ ¬G` has no model: the premises this call already proves from refute
        the guard of the arm it arrives on, so the arrival cannot happen.
    */
    pub(super) fn proves(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        entry_contexts: &[facts::FactContextHandle],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some(guard) = refuted_edge_guard(program, state_flow, call_flow) else {
            return false;
        };
        if !super::super::guard_operands::premise_reads_survive_arm_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            guard,
            call_frames,
        ) {
            return false;
        }
        let Some(expression) = crate::published::facts::boolean_predicates::lower_current_guard(
            program,
            &facts.operators,
            state_flow.state_symbol,
            call_flow.statement_index,
            guard,
        ) else {
            return false;
        };
        let Some(refuted) = crate::published::facts::boolean_predicates::bind_current_expression(
            program,
            state_flow.state_symbol,
            expression,
            false,
            |operand| {
                crate::published::facts::boolean_predicates::declared_observation(
                    program,
                    state_flow.state_symbol,
                    call_flow.statement_index,
                    operand,
                )
            },
        ) else {
            return false;
        };
        let point = ProgramPoint::Statement {
            machine_symbol: state_flow.machine_symbol,
            state_symbol: state_flow.state_symbol,
            statement_index: call_flow.statement_index,
        };
        let mut premises = crate::published::facts::boolean_predicates::retained(
            program,
            &facts.operators,
            &facts.semantic,
            entry_contexts,
            point,
        );
        if premises.is_empty() {
            return false;
        }
        premises.push(refuted);
        crate::published::facts::boolean_predicates::unsatisfiable(&premises)
    }
}
mod guard_observations {
    /*
        A taken guard may read a scalar copy made before the guard. Normalization
        and authored locals have the same retained store evidence. Relate that copy
        to its source only while both the destination and every source/selector observation
        survive through this exact invocation. This does not replay an initializer
        or a callee, and a write followed by restoration still breaks the relation.
    */
    use super::captured_boolean::computed_values_have_source_identity;
    use crate::checks::operators::closed_operators_are_builtin;
    use crate::published::facts::contract_occurrences::append_expression_occurrences;
    use crate::published::places::place_at;
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

    pub(super) fn conjunct_matches(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        guard: ExpressionHandle,
        required: ExpressionHandle,
    ) -> bool {
        let Some((_, _, site, parameters)) = super::call_coordinates(program, state, call) else {
            return false;
        };
        let arguments = validation::call_site_argument_expressions(program, &site);
        let aliases = stable_copies(program, facts, state, call);
        Comparison {
            program,
            facts,
            state,
            call,
            parameters,
            arguments,
            aliases: &aliases,
        }
        .conjunct(guard, required)
    }

    fn stable_copies(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
    ) -> Vec<(validation::CanonicalPlace, ExpressionHandle)> {
        let Some(invocation) = facts
            .flow
            .control
            .operands
            .span(call.operands)
            .and_then(|values| values.last())
        else {
            return Vec::new();
        };
        let Some(statements) = facts.flow.control.statements.span(state.statements) else {
            return Vec::new();
        };
        let mut aliases = Vec::new();
        let mut seen = Vec::new();
        for statement in statements
            .iter()
            .rev()
            .filter(|statement| statement.statement_index < call.statement_index)
        {
            if !facts.semantic.places.is_valid(statement.stored_destination)
                || !facts.flow.control.operands.is_valid(statement.stored_value)
            {
                continue;
            }
            let Some(destination) = crate::flow::canonical_place_from_semantic_place(
                program,
                &facts.semantic,
                facts.semantic.places.get(statement.stored_destination),
            ) else {
                continue;
            };
            if seen
                .iter()
                .any(|prior| same_place(program, prior, &destination))
            {
                continue;
            }
            seen.push(destination.clone());
            let stored = facts.flow.control.operands.get(statement.stored_value);
            if program
                .primitive_type_reference(stored.type_reference)
                .is_none()
                || !computed_values_have_source_identity(program, facts, stored.expression)
            {
                continue;
            }
            if !statements
                .iter()
                .filter(|later| {
                    later.statement_index > statement.statement_index
                        && later.statement_index < call.statement_index
                })
                .all(|later| {
                    crate::flow::entry_origin_is_stable(
                        program,
                        &facts.flow.control,
                        later.completed_content_writes,
                        later.completed_content_unknown_write,
                        &destination,
                    )
                })
                || !crate::flow::operand_entry_origin_is_stable(
                    program,
                    facts.flow.view(),
                    invocation,
                    &destination,
                )
            {
                continue;
            }
            let mut reads = Vec::new();
            append_expression_occurrences(program, stored.expression, &mut reads);
            let mut selectors_preserve_identity = true;
            for segment in &destination.segments {
                if let facts::PlaceSegment::Index { expression } = segment {
                    selectors_preserve_identity &=
                        computed_values_have_source_identity(program, facts, *expression);
                    append_expression_occurrences(program, *expression, &mut reads);
                }
            }
            if selectors_preserve_identity
                && reads.iter().all(|read| {
                    place_at(program, state.state_symbol, stored.statement_index, *read)
                        .is_some_and(|place| {
                            crate::flow::operand_observations_agree(
                                program,
                                facts.flow.view(),
                                stored,
                                invocation,
                                &place,
                            )
                        })
                })
            {
                aliases.push((destination, stored.expression));
            }
        }
        aliases
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Namespace {
        Caller,
        Callee,
    }

    struct Comparison<'a> {
        program: &'a dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
        parameters: &'a [typed_trees::signature::StateParameter],
        arguments: &'a [ExpressionHandle],
        aliases: &'a [(validation::CanonicalPlace, ExpressionHandle)],
    }

    impl Comparison<'_> {
        fn conjunct(&self, guard: ExpressionHandle, required: ExpressionHandle) -> bool {
            if !closed_operators_are_builtin(self.program, &self.facts.operators, guard) {
                return false;
            }
            if self.equivalent(guard, required, Namespace::Callee, 0) {
                return true;
            }
            let Some(binary) = self.program.expression_table.binary_node(guard) else {
                return false;
            };
            match binary.operator {
                BinaryOperator::And => {
                    self.conjunct(binary.left, required) || self.conjunct(binary.right, required)
                }
                BinaryOperator::Equal
                    if matches!(
                        self.program.expression_table.expression(binary.right),
                        ExpressionNode::Boolean(true)
                    ) =>
                {
                    self.conjunct(binary.left, required)
                }
                _ => false,
            }
        }

        fn copied_source(&self, expression: ExpressionHandle) -> Option<ExpressionHandle> {
            if !matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ) {
                return None;
            }
            let place = place_at(
                self.program,
                self.state.state_symbol,
                self.call.statement_index,
                expression,
            )?;
            self.aliases
                .iter()
                .find(|(destination, _)| same_place(self.program, &place, destination))
                .map(|(_, source)| *source)
        }

        fn equivalent(
            &self,
            guard: ExpressionHandle,
            required: ExpressionHandle,
            namespace: Namespace,
            depth: usize,
        ) -> bool {
            if depth >= 64 {
                return false;
            }
            if namespace == Namespace::Callee
                && let ExpressionNode::Name(path) =
                    self.program.expression_table.expression(required)
                && self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1
                && let Some(position) = self.parameters.iter().position(|parameter| {
                    path.head_symbol == parameter.symbol || path.symbol == parameter.symbol
                })
            {
                let Some(mut argument) = self.arguments.get(position).copied() else {
                    return false;
                };
                if matches!(
                    self.program
                        .type_reference_table
                        .type_reference(self.parameters[position].type_reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                ) && let ExpressionNode::Borrow(borrow) =
                    self.program.expression_table.expression(argument)
                {
                    argument = borrow.target;
                }
                return self.equivalent(guard, argument, Namespace::Caller, depth + 1);
            }
            if let (Some(left), Some(right)) = (
                self.closed_integer(guard, Namespace::Caller),
                self.closed_integer(required, namespace),
            ) {
                return left == right;
            }
            let recurse = |left, right| self.equivalent(left, right, namespace, depth + 1);
            let same_observation = match (
                self.program.expression_table.expression(guard),
                self.program.expression_table.expression(required),
            ) {
                (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                    left.resolved_symbol().is_some()
                        && left.resolved_symbol() == right.resolved_symbol()
                        && left.symbol == right.symbol
                        && self
                            .program
                            .expression_table
                            .name_path_members(left.members)
                            == self
                                .program
                                .expression_table
                                .name_path_members(right.members)
                }
                (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                    left.member_symbol.is_valid()
                        && left.member_symbol == right.member_symbol
                        && left.case_variant == right.case_variant
                        && recurse(left.receiver, right.receiver)
                }
                (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
                    recurse(left.collection, right.collection) && recurse(left.index, right.index)
                }
                (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left
                    .value_bignum()
                    .is_some_and(|value| Some(value) == right.value_bignum()),
                (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
                (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
                (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
                    left.operator == right.operator
                        && closed_operators_are_builtin(self.program, &self.facts.operators, guard)
                        && closed_operators_are_builtin(
                            self.program,
                            &self.facts.operators,
                            required,
                        )
                        && recurse(left.left, right.left)
                        && recurse(left.right, right.right)
                }
                (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
                    left.operator == right.operator
                        && closed_operators_are_builtin(self.program, &self.facts.operators, guard)
                        && closed_operators_are_builtin(
                            self.program,
                            &self.facts.operators,
                            required,
                        )
                        && recurse(left.operand, right.operand)
                }
                _ => false,
            };
            same_observation
                || self
                    .copied_source(guard)
                    .is_some_and(|source| self.equivalent(source, required, namespace, depth + 1))
                || (namespace == Namespace::Caller
                    && self
                        .copied_source(required)
                        .is_some_and(|source| self.equivalent(guard, source, namespace, depth + 1)))
        }

        /*
            Fold selected closed integer values, including fixed extents. No live
            inputs are supplied, so a stored value or dynamic count stays unknown.
        */
        fn closed_integer(
            &self,
            expression: ExpressionHandle,
            namespace: Namespace,
        ) -> Option<numerics::bignum::BigInt> {
            if let ExpressionNode::Integer(literal) =
                self.program.expression_table.expression(expression)
            {
                return literal.value_bignum();
            }
            let (machine, state) = match namespace {
                Namespace::Caller => validation::machine_state_by_symbols(
                    self.program,
                    self.state.machine_symbol,
                    self.state.state_symbol,
                )?,
                Namespace::Callee => validation::find_state(self.program, self.call.target_symbol)
                    .or_else(|| {
                        validation::find_machine_head(self.program, self.call.target_symbol)
                    })?,
            };
            let primitive = if validation::collection_length_receiver(
                self.program,
                machine,
                Some(state),
                expression,
            )
            .is_some()
            {
                typed_trees::types::PrimitiveType::U64
            } else {
                let reference = validation::expression_result_type_reference(
                    self.program,
                    machine,
                    state,
                    expression,
                )?;
                self.program.primitive_type_reference(reference)?
            };
            if !primitive.is_fixed_integer() {
                return None;
            }
            let value = crate::values::lower_unit_scalar_argument(
                self.program,
                &self.facts.operators,
                state,
                if namespace == Namespace::Caller {
                    self.call.statement_index
                } else {
                    0
                },
                expression,
                primitive,
            )?;
            let facts::ScalarValue::Integer(value) =
                crate::values::evaluate_checked_scalar(&value, &mut |_| None)?
            else {
                return None;
            };
            Some(value)
        }
    }

    fn same_place(
        program: &dyn ::validation::CheckingInput,
        first: &validation::CanonicalPlace,
        second: &validation::CanonicalPlace,
    ) -> bool {
        first.root == second.root
            && first.segments.len() == second.segments.len()
            && first
                .segments
                .iter()
                .zip(&second.segments)
                .all(|(left, right)| {
                    crate::flow::canonical_place_segments_equal(program, *left, *right)
                })
    }
}
mod labels {
    /*
        Identifier rewriting and state-parameter label instantiation for the
        remaining transition-guard diagnostics and contract queries.
    */
    use language_core::is_self_receiver;
    use typed_trees::expression::ExpressionNode;

    /*
        One scanner event: an unqualified-or-qualified identifier run, or one
        non-identifier gap character.
    */
    pub(super) enum LabelToken<'a> {
        Identifier { text: &'a str, qualified: bool },
        Gap(char),
    }

    /*
        Walk `label` left to right, yielding each `_`/alphabetic-start identifier
        (its byte start, text, and whether a `.`/`:` immediately precedes it) and
        every gap character between identifiers — the shared scanner both label
        rewrites run.
    */
    pub(super) fn for_each_label_token(label: &str, mut visit: impl FnMut(LabelToken<'_>)) {
        let mut cursor = 0usize;
        while cursor < label.len() {
            let Some(character) = label[cursor..].chars().next() else {
                break;
            };
            if character == '_' || character.is_alphabetic() {
                let start = cursor;
                cursor += character.len_utf8();
                while cursor < label.len() {
                    let Some(next) = label[cursor..].chars().next() else {
                        break;
                    };
                    if next == '_' || next.is_alphanumeric() {
                        cursor += next.len_utf8();
                    } else {
                        break;
                    }
                }
                let identifier = &label[start..cursor];
                let qualified =
                    start > 0 && matches!(label.as_bytes().get(start - 1), Some(b'.' | b':'));
                visit(LabelToken::Identifier {
                    text: identifier,
                    qualified,
                });
            } else {
                visit(LabelToken::Gap(character));
                cursor += character.len_utf8();
            }
        }
    }

    pub(super) fn unqualified_label_identifiers(label: &str) -> Vec<String> {
        let mut names = Vec::new();
        for_each_label_token(label, |token| {
            if let LabelToken::Identifier {
                text: identifier,
                qualified,
                ..
            } = token
                && !qualified
                && !is_self_receiver(identifier)
                && !names.iter().any(|name: &String| name == identifier)
            {
                names.push(identifier.to_owned());
            }
        });
        names
    }

    pub(super) fn assignment_target_mentions_name(
        program: &dyn ::validation::CheckingInput,
        target: typed_trees::expression::ExpressionHandle,
        name: &str,
    ) -> bool {
        if !target.is_valid() {
            return false;
        }
        match program.expression_table.expression(target) {
            ExpressionNode::Name(_) => program.expression_table.display_name(target) == name,
            ExpressionNode::Member(member) => {
                assignment_target_mentions_name(program, member.receiver, name)
            }
            ExpressionNode::Borrow(inner) => {
                assignment_target_mentions_name(program, inner.target, name)
            }
            ExpressionNode::Indexed(indexed) => {
                assignment_target_mentions_name(program, indexed.collection, name)
            }
            _ => false,
        }
    }

    /*
        Rebind a contract label already instantiated in `state` through the
        immediate incoming transition arguments. Contract comparison is currently
        label-based, so substitute only whole, unqualified parameter identifiers:
        a field token in `self.count` must not be mistaken for the state parameter
        `count`.
    */
    pub(super) fn instantiate_state_parameter_label(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        arguments: arena::HandleSpan<typed_trees::expression::ExpressionHandle>,
        label: &str,
    ) -> String {
        let arguments = program.statement_table.expression_handles(arguments);
        let mut replacements: Vec<(&str, String)> = Vec::new();
        let mut argument_index = 0usize;
        for parameter in program.state_parameters(state) {
            let Some(argument) = arguments.get(argument_index) else {
                break;
            };
            replacements.push((
                parameter.name.as_str(),
                program.expression_table.display_name(*argument),
            ));
            argument_index = argument_index.saturating_add(1);
        }
        replace_unqualified_identifiers(label, &replacements)
    }

    fn replace_unqualified_identifiers(label: &str, replacements: &[(&str, String)]) -> String {
        let mut result = String::with_capacity(label.len());
        for_each_label_token(label, |token| match token {
            LabelToken::Identifier {
                text: identifier,
                qualified,
                ..
            } => {
                if !qualified
                    && let Some((_, replacement)) =
                        replacements.iter().find(|(name, _)| *name == identifier)
                {
                    result.push_str(replacement);
                } else {
                    result.push_str(identifier);
                }
            }
            LabelToken::Gap(character) => result.push(character),
        });
        result
    }
}
mod predicate_domains {
    /*
        Predicate-only domain vocabulary: predicates, labels, intervals, and
        the requirement subject a fact constrains.
    */
    use super::super::labels::instantiate_domain_expression_label;
    use checked_trees::CheckFacts;
    use facts::Fact;
    use language_semantics::SemanticDomainId;
    use numerics::bignum::BigInt;
    use symbols::SymbolHandle;
    use typed_trees::domain::DomainDefinition;
    use typed_trees::domain::ProofFact;
    use typed_trees::domain::domain_subject_reference;
    use typed_trees::domain::index_parameters;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::DomainConstraint;
    use validation::exact_declared_domain_interval;

    /*
        The proof-fact expressions of a PREDICATE-ONLY domain, or `None` when the
        domain is not predicate-only.

        [Domains](wiki/spec/language/domains.md#declaration-and-membership) settles
        the rule: "Predicates alone establish predicate-only membership", and a
        ROUTED domain is the case that "additionally needs exact authorized
        provenance". Routed and aliased domains retain that obligation; an indexed
        theory must already have a complete applied view. Only a theory whose whole content is
        predicates over `self` reduces, and then EVERY predicate must be
        established, never a subset.
    */
    pub(super) fn predicate_only_domain_predicates<'program>(
        program: &'program dyn ::validation::CheckingInput,
        domain: &'program DomainDefinition,
    ) -> Option<Vec<(&'program DomainDefinition, ExpressionHandle)>> {
        fn visit<'program>(
            program: &'program dyn ::validation::CheckingInput,
            domain: &'program DomainDefinition,
            active: &mut Vec<(SymbolHandle, SemanticDomainId)>,
        ) -> Option<Vec<(&'program DomainDefinition, ExpressionHandle)>> {
            let instance = (domain.symbol, domain.semantic_id);
            if active.contains(&instance)
                || domain.alias.is_some()
                || !domain.index_arguments.is_empty()
                || !domain.establishment_routes.is_empty()
                || !index_parameters(program, domain).is_empty()
            {
                return None;
            }
            let declared = program.proof_facts(domain);
            if declared.is_empty() {
                return None;
            }
            active.push(instance);
            let mut predicates = Vec::new();
            for fact in declared {
                match fact {
                    ProofFact::Expression(expression) => {
                        predicates.push((domain, *expression));
                    }
                    ProofFact::Membership(membership)
                        if domain_subject_reference(program, domain, membership.value) =>
                    {
                        let parent = program.domain_definition_for_instance(
                            membership.domain_symbol,
                            membership.semantic_domain,
                        )?;
                        predicates.extend(visit(program, parent, active)?);
                    }
                    _ => return None,
                }
            }
            active.pop();
            Some(predicates)
        }
        visit(program, domain, &mut Vec::new())
    }

    /*
        The predicates a PREDICATE-ONLY domain membership reduces to, each rendered
        at the subject, or `None` when the domain is not predicate-only.

        The rendering is `instantiate_domain_expression_label`, the same
        substitution `contracts::domains` already runs in the MEMBERSHIP ->
        PREDICATE direction. This is that rendering read the other way.
    */
    pub(super) fn predicate_only_domain_labels(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        fact: &Fact,
    ) -> Option<Vec<String>> {
        let (_, _, domain_symbol, identity) = fact.payload.domain_membership()?;
        let place_handle = fact.place.as_place()?;
        let subject_label = facts.semantic.place_label(program, place_handle);
        let domain = program.domain_definition_for_instance(domain_symbol, identity)?;
        Some(
            predicate_only_domain_predicates(program, domain)?
                .iter()
                .map(|(owner, expression)| {
                    instantiate_domain_expression_label(program, owner, *expression, &subject_label)
                })
                .collect(),
        )
    }

    /*
        The closed interval a PREDICATE-ONLY domain requires of its subject, paired
        with that subject's label, or `None` when the domain states no interval.

        Establishment direction, so the interval must be EXACT: every predicate a
        recognized bound on `self`, or containing the subject inside it would grant
        membership while a predicate it cannot see -- `self != 7` beside
        `self <= 15` -- stays unproven.
    */
    pub(super) fn predicate_only_domain_interval(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        fact: &Fact,
    ) -> Option<(String, BigInt, BigInt)> {
        let (_, _, domain_symbol, semantic_domain) = fact.payload.domain_membership()?;
        let place_handle = fact.place.as_place()?;
        let domain = program.domain_definition(domain_symbol)?;
        if domain.alias.is_some()
            || !domain.index_arguments.is_empty()
            || !domain.establishment_routes.is_empty()
            || !index_parameters(program, domain).is_empty()
        {
            return None;
        }
        let constraint = DomainConstraint {
            symbol: domain_symbol,
            semantic_id: semantic_domain,
            ..Default::default()
        };
        let (minimum, maximum) = exact_declared_domain_interval(program, &constraint)?;
        Some((
            facts.semantic.place_label(program, place_handle),
            minimum.unwrap_or_else(|| BigInt::from_i64(i64::MIN)),
            maximum.unwrap_or_else(|| BigInt::from_i64(i64::MAX)),
        ))
    }
}
mod state_preservation {
    /*
        Caller-state preservation: which fields, label names, and predicate
        premises survive the statements between an incoming guard and this call.
    */
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use facts::{PlaceRoot, PlaceSegment};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::StatementNode;

    use super::super::assembly::expression_reads_overlapping_place;
    use crate::published::labels::symbol_name;
    use crate::published::places::place_at;

    /*
        The guard match above compares display labels, so it stands only while
        every unqualified name the instantiated requirement spells still refers to
        the caller value the guard was evaluated against. Rebinding such a name
        before the call -- `limit = 0`, a shadowing `let`, or a mutation through a
        call's write frame (recorded as a pre-call invalidation) -- makes the label
        quote a stale premise, and this route must refuse; the remaining provers
        may still establish the fact from current evidence. Qualified `self.field`
        operands are covered by the field walk above, so `self` itself is skipped.

        `reads` are the caller argument expressions the requirement instance
        reads. A prior call write under a spelled root stales the premise only
        when it may overlap one of those reads: popping `main.a` leaves a guard on
        `main.b.count` intact. A write whose root no read shares cannot be placed
        against the label and stays conservative.
    */
    pub(super) fn caller_state_preserves_label_names(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        state: &typed_trees::state::State,
        required_label: &str,
        reads: &[ExpressionHandle],
    ) -> bool {
        let names = super::labels::unqualified_label_identifiers(required_label);
        if names.is_empty() {
            return true;
        }
        let rebound_before_call = crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .take(call_flow.statement_index)
            .any(|statement| match statement {
                StatementNode::Assignment(assignment) => names.iter().any(|name| {
                    super::labels::assignment_target_mentions_name(program, assignment.target, name)
                }),
                StatementNode::LocalData(local) => names
                    .iter()
                    .any(|name| local.name.as_str() == name.as_str()),
                _ => false,
            });
        if rebound_before_call {
            return false;
        }
        !facts
            .flow
            .state_call_prior_invalidations(state_flow, call_flow)
            .any(|invalidation| {
                let Some(symbol) = invalidation.mutated_root.as_symbol() else {
                    return false;
                };
                if !names
                    .iter()
                    .any(|name| name.as_str() == symbol_name(program, symbol))
                {
                    return false;
                }
                let written = crate::checks::places::place_or_root_in(
                    &facts.flow.invalidations.segments,
                    invalidation.mutated_root,
                    invalidation.mutated_segments,
                );
                let read_places = reads
                    .iter()
                    .filter_map(|read| {
                        place_at(
                            program,
                            state_flow.state_symbol,
                            call_flow.statement_index,
                            *read,
                        )
                    })
                    .collect::<Vec<_>>();
                !read_places.iter().any(|place| place.root == written.root)
                    || reads.iter().any(|read| {
                        expression_reads_overlapping_place(
                            program,
                            state_flow.state_symbol,
                            call_flow.statement_index,
                            *read,
                            &written,
                        )
                    })
            })
    }

    pub(super) fn collect_expression_self_fields(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
        fields: &mut Vec<typed_trees::name::Identifier>,
    ) {
        if !expression.is_valid() {
            return;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                fields.push(member.member.clone());
                collect_expression_self_fields(program, member.receiver, fields);
            }
            ExpressionNode::Binary(binary) => {
                collect_expression_self_fields(program, binary.left, fields);
                collect_expression_self_fields(program, binary.right, fields);
            }
            ExpressionNode::Borrow(inner) => {
                collect_expression_self_fields(program, inner.target, fields)
            }
            ExpressionNode::Cast(cast) => {
                collect_expression_self_fields(program, cast.value, fields)
            }
            _ => {}
        }
    }

    pub(super) fn caller_state_preserves_field(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        field: &typed_trees::name::Identifier,
    ) -> bool {
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(assignment) = statement.as_assignment()
                && assignment_target_mentions_field(program, assignment.target, field)
            {
                return false;
            }
        }
        true
    }

    /*
        The field walk above sees ASSIGNMENTS only: `self.clear()` is a call whose
        write lands in the state's prior-invalidation list instead. A mutation
        rooted at this machine's own `self` storage and naming one of the fields
        the premise reads stales that premise. Writes through a non-`self` root
        stay with `caller_state_preserves_label_names`, which matches the root's
        caller-visible name.
    */
    pub(super) fn caller_state_preserves_self_fields_against_call_writes(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        fields: &[typed_trees::name::Identifier],
    ) -> bool {
        !facts
            .flow
            .state_call_prior_invalidations(state_flow, call_flow)
            .any(|invalidation| {
                if (invalidation.mutated_root) != PlaceRoot::Symbol(state_flow.machine_symbol) {
                    return false;
                }
                let segments = facts
                    .flow
                    .invalidations
                    .segments
                    .span_or_empty(invalidation.mutated_segments);
                segments.is_empty()
                    || segments.iter().any(|segment| match segment {
                        PlaceSegment::Field { symbol } => fields
                            .iter()
                            .any(|field| field.as_str() == symbol_name(program, *symbol).as_str()),
                        _ => true,
                    })
            })
    }

    fn assignment_target_mentions_field(
        program: &dyn ::validation::CheckingInput,
        target: typed_trees::expression::ExpressionHandle,
        field: &typed_trees::name::Identifier,
    ) -> bool {
        if !target.is_valid() {
            return false;
        }
        match program.expression_table.expression(target) {
            ExpressionNode::Member(member) => {
                member.member.as_str() == field.as_str()
                    || assignment_target_mentions_field(program, member.receiver, field)
            }
            ExpressionNode::Borrow(inner) => {
                assignment_target_mentions_field(program, inner.target, field)
            }
            ExpressionNode::Indexed(indexed) => {
                assignment_target_mentions_field(program, indexed.collection, field)
            }
            _ => false,
        }
    }
}
mod transition_guards {
    /*
        Transition-guard requirement proofs: the selected transition call,
        guard conjunct matching, and guarded-value preservation a call
        requirement may read through.
    */
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use facts::FactPayload;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::StatementNode;

    use crate::published::labels::semantic_fact_requirement_label;

    /*
        Recover the premise of this selected edge once, for both complete facts
        and their Boolean children. A computed transition subject is a captured
        predicate, not merely its generated truth-test binding. Its reads must
        survive both evaluation of the predicate and the arm's operands before any
        consumer can use that premise; splitting a conjunction changes no epoch.
    */
    fn selected_transition_premise<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        call_frames: Option<&validation::CallFrameResolver<'program>>,
    ) -> Option<(
        validation::CallSite<'program>,
        &'program [typed_trees::signature::StateParameter],
        Vec<typed_trees::expression::ExpressionHandle>,
    )> {
        let (machine, caller_state, call_site, target_parameters) =
            super::call_coordinates(program, state_flow, call_flow)?;
        let Some(StatementNode::Transition(transition)) = program
            .statement_table
            .statement_at(caller_state, call_flow.statement_index)
        else {
            return None;
        };
        let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let selects_this_call = match &call_site {
            validation::CallSite::TransitionNamed { .. } => {
                validation::transition_call_target(
                    program,
                    machine,
                    caller_state,
                    call_flow.statement_index,
                    call_flow.call_ordinal,
                ) == Some(transition.target)
            }
            validation::CallSite::Expression { expression, .. } => {
                matches!(
                    program.statement_table.transition_target(transition.target),
                    typed_trees::statement::TransitionTargetNode::Value(value) if value == expression
                )
            }
            validation::CallSite::Statement(_) => false,
        };
        if !selects_this_call || !guard_preserves_values(machine, guard, call_frames) {
            return None;
        }
        let captured = crate::flow::saved_guard_observations(
            program,
            machine,
            caller_state,
            call_flow.statement_index,
            guard,
            true,
            &[],
            call_frames,
        );
        let premises = std::iter::once(guard)
            .chain(captured.into_iter().filter_map(|observation| {
                let premise = observation.current_expression;
                if !premise.is_valid() {
                    return None;
                }
                (observation.value
                    && super::super::guard_operands::premise_reads_survive_arm_operand_writes(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        premise,
                        call_frames,
                    ))
                .then_some(premise)
            }))
            .collect();
        Some((call_site, target_parameters, premises))
    }

    /*
        Bind the actual selected edge for a conditional proof goal. Its reads
        must survive the guard and every arm operand. Recovered subpredicates do
        not replace this guard: their truth is conditional on this exact selection.
    */
    pub(in crate::checks::contracts) fn selected_guard_predicate(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<crate::published::facts::boolean_predicates::BoundPredicate> {
        let (_, _, guards) =
            selected_transition_premise(program, facts, state_flow, call_flow, call_frames)?;
        let guard = *guards.first()?;
        let (machine, _, _, _) = super::call_coordinates(program, state_flow, call_flow)?;
        if !guard_preserves_values(machine, guard, call_frames)
            || !super::super::guard_operands::premise_reads_survive_arm_operand_writes(
                program,
                facts,
                state_flow,
                call_flow,
                guard,
                call_frames,
            )
        {
            return None;
        }
        let expression = crate::published::facts::boolean_predicates::lower_current_guard(
            program,
            &facts.operators,
            state_flow.state_symbol,
            call_flow.statement_index,
            guard,
        )?;
        crate::published::facts::boolean_predicates::bind_current_expression(
            program,
            state_flow.state_symbol,
            expression,
            true,
            |operand| {
                crate::published::facts::boolean_predicates::declared_observation(
                    program,
                    state_flow.state_symbol,
                    call_flow.statement_index,
                    operand,
                )
            },
        )
    }

    /*
        Query one original Boolean conjunct through the same selected-arm and
        operand-currentness gates as the complete contract fact.
    */
    pub(in crate::checks::contracts) fn transition_guard_proves_boolean_requirement(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        entry_contexts: &[facts::FactContextHandle],
        expression: typed_trees::expression::ExpressionHandle,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some((call_site, target_parameters, guards)) =
            selected_transition_premise(program, facts, state_flow, call_flow, call_frames)
        else {
            return false;
        };
        let required_label = super::super::labels::instantiate_call_contract_expression_label(
            program,
            state_flow.state_symbol,
            call_flow.statement_index,
            &call_site,
            target_parameters,
            expression,
        );
        (guards.iter().any(|guard| {
            guard_conjunct_matches(program, *guard, &required_label)
                || super::guard_observations::conjunct_matches(
                    program, facts, state_flow, call_flow, *guard, expression,
                )
                || super::super::call_bounds::proves(
                    program,
                    facts,
                    state_flow,
                    call_flow,
                    &[],
                    expression,
                    Some(*guard),
                )
                || decomposed_guard_premise_proves(
                    program,
                    facts,
                    state_flow,
                    call_flow,
                    entry_contexts,
                    *guard,
                    expression,
                    &required_label,
                    call_frames,
                )
        }) || case_requirement_matches(program, state_flow, &guards, &required_label))
            && super::super::guard_operands::requirement_reads_survive_earlier_operand_writes(
                program,
                facts,
                state_flow,
                call_flow,
                validation::call_site_argument_expressions(program, &call_site),
                target_parameters,
                &super::super::guard_operands::boolean_requirement_mentions(program, expression),
                call_frames,
            )
    }

    /*
        A named transition is itself an incoming edge. Its taken-arm guard may
        establish the target state's arrival requirement after positional
        substitution (`value > 0` becomes `self.value > 0`). Ordinary call entry
        contexts are statement-entry facts and therefore deliberately do not assume
        that guard; discharge it explicitly for the transition target only, and
        only while the arm's own operands, evaluated after the guard and before
        the jump, leave the guard's storage unwritten (`guard_operands`).
    */
    pub(super) fn transition_guard_proves_requires(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        fact: &facts::Fact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some((call_site, target_parameters, matching_guards)) =
            selected_transition_premise(program, facts, state_flow, call_flow, call_frames)
        else {
            return false;
        };
        let matches_guard = |label: &str| {
            matching_guards
                .iter()
                .any(|guard| guard_conjunct_matches(program, *guard, label))
        };
        let required_label = match fact.payload {
            FactPayload::ContractBooleanExpression { expression, .. } => {
                super::super::labels::instantiate_call_contract_expression_label(
                    program,
                    state_flow.state_symbol,
                    call_flow.statement_index,
                    &call_site,
                    target_parameters,
                    expression,
                )
            }
            FactPayload::ContractDomainMembership { .. } | FactPayload::DomainMembership { .. } => {
                semantic_fact_requirement_label(program, &facts.semantic, fact)
            }
            _ => return false,
        };
        let copied_guard_establishes = match fact.payload {
            FactPayload::ContractBooleanExpression { expression, .. } => {
                matching_guards.iter().any(|matching_guard| {
                    super::guard_observations::conjunct_matches(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        *matching_guard,
                        expression,
                    )
                })
            }
            _ => false,
        };
        let guard_establishes = matches_guard(&required_label)
            || copied_guard_establishes
            || case_requirement_matches(program, state_flow, &matching_guards, &required_label)
            || super::predicate_domains::predicate_only_domain_labels(program, facts, fact)
                .is_some_and(|labels| labels.iter().all(|label| matches_guard(label)))
            || super::predicate_domains::predicate_only_domain_interval(program, facts, fact)
                .is_some_and(|(subject_label, required_low, required_high)| {
                    matching_guards.iter().any(|guard| {
                        super::super::intervals::guard_interval_for_label(
                            program,
                            *guard,
                            &subject_label,
                        )
                        .is_some_and(|interval| {
                            super::super::intervals::establishes_closed_requirement(
                                &interval,
                                &required_low,
                                &required_high,
                            )
                        })
                    })
                });
        if !guard_establishes {
            return false;
        }
        super::super::guard_operands::requirement_reads_survive_earlier_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            validation::call_site_argument_expressions(program, &call_site),
            target_parameters,
            &super::super::guard_operands::fact_requirement_mentions(program, facts, fact),
            call_frames,
        )
    }

    /*
        A selected `subject in Data::Case` guard establishes that case's data
        `requires` conjuncts. Membership is the guard; the scalar obligations are
        the premises a callee `requires` may quote. Projection renames the
        predicate subject onto the dispatched value and the data's value
        parameters onto that value's type arguments, which is the vocabulary the
        instantiated callee contract already uses.
    */
    fn case_requirement_matches(
        program: &dyn ::validation::CheckingInput,
        state_flow: &FlowStateFact,
        guards: &[typed_trees::expression::ExpressionHandle],
        required_label: &str,
    ) -> bool {
        let Some(machine) = program.machine(state_flow.machine_symbol) else {
            return false;
        };
        let Some(state) = program.state_in_machine(machine, state_flow.state_symbol) else {
            return false;
        };
        guards.iter().any(|guard| {
            projected_case_requirement_labels(program, machine, state, *guard)
                .iter()
                .any(|label| {
                    label == required_label
                        || label.split(" && ").any(|part| part == required_label)
                })
        })
    }

    fn projected_case_requirement_labels(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        guard: typed_trees::expression::ExpressionHandle,
    ) -> Vec<String> {
        let Some(comparison) = program.expression_table.binary_node(guard) else {
            return Vec::new();
        };
        let Some(case) = validation::applied_case_membership_variant(
            program,
            machine,
            Some(state),
            guard,
            comparison,
        ) else {
            return Vec::new();
        };
        let mut subject = comparison.left;
        for _ in 0..8 {
            match program.expression_table.expression(subject) {
                ExpressionNode::Borrow(borrow) => subject = borrow.target,
                ExpressionNode::Name(path) => {
                    let initial = program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .find_map(|statement| match statement {
                            StatementNode::LocalData(local) if local.symbol == path.symbol => {
                                Some(local.initial_value)
                            }
                            _ => None,
                        });
                    let Some(initial) = initial else {
                        break;
                    };
                    subject = initial;
                }
                _ => break,
            }
        }
        let Some(reference) =
            validation::expression_result_type_reference(program, machine, state, subject)
        else {
            return Vec::new();
        };
        let reference = program
            .type_reference_table
            .peeled_type_reference(reference);
        let typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = program.type_reference_table.type_reference(reference)
        else {
            return Vec::new();
        };
        let Some(definition) = program.data_definition_by_symbol(*base_symbol) else {
            return Vec::new();
        };
        let parameters = program.declaration_type_parameters(*base_symbol);
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        let mut parameter_names = Vec::new();
        for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
            if !matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Value { .. }
                    | typed_trees::data::TypeParameterKind::Const { .. }
            ) {
                continue;
            }
            let name = match program.type_reference_table.type_reference(*argument) {
                typed_trees::types::TypeReferenceNode::ConstExpression(expression) => {
                    program.expression_table.display_name(*expression)
                }
                _ => continue,
            };
            parameter_names.push((parameter.symbol, name));
        }
        let subject_name = program.expression_table.display_name(subject);
        let requirements = validation::case_predicate_expressions(program, definition, case);
        let rendered = requirements
            .iter()
            .map(|requirement| {
                render_projected_requirement(
                    program,
                    definition.predicate_subject_symbol,
                    &subject_name,
                    &parameter_names,
                    *requirement,
                )
            })
            .collect::<Vec<_>>();
        if rendered.len() <= 1 {
            return rendered;
        }
        let mut labels = vec![rendered.join(" && ")];
        labels.extend(rendered);
        labels
    }

    fn render_projected_requirement(
        program: &dyn ::validation::CheckingInput,
        predicate_subject: symbols::SymbolHandle,
        subject_name: &str,
        parameters: &[(symbols::SymbolHandle, String)],
        expression: typed_trees::expression::ExpressionHandle,
    ) -> String {
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => format!(
                "{} {} {}",
                render_projected_requirement(
                    program,
                    predicate_subject,
                    subject_name,
                    parameters,
                    binary.left,
                ),
                binary.operator.display_name(),
                render_projected_requirement(
                    program,
                    predicate_subject,
                    subject_name,
                    parameters,
                    binary.right,
                ),
            ),
            ExpressionNode::Call(call) if !call.receiver.is_valid() => {
                let arguments = program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| {
                        render_projected_requirement(
                            program,
                            predicate_subject,
                            subject_name,
                            parameters,
                            *argument,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({arguments})", call.target.as_str())
            }
            ExpressionNode::Name(path) => {
                if path.symbol == predicate_subject {
                    return subject_name.to_string();
                }
                if let Some((_, name)) =
                    parameters.iter().find(|(symbol, _)| *symbol == path.symbol)
                {
                    return name.clone();
                }
                program.expression_table.display_name(expression)
            }
            ExpressionNode::Member(member) => {
                let receiver = match program.expression_table.expression(member.receiver) {
                    ExpressionNode::Name(path) if path.symbol == predicate_subject => {
                        subject_name.to_string()
                    }
                    _ => render_projected_requirement(
                        program,
                        predicate_subject,
                        subject_name,
                        parameters,
                        member.receiver,
                    ),
                };
                format!("{receiver}.{}", member.member.as_str())
            }
            _ => program.expression_table.display_name(expression),
        }
    }

    /*
        A later guard conjunct can overwrite an earlier premise. Operand write
        checks begin after the complete guard, so guard-derived proofs additionally
        require this complete, known-empty expression frame.
    */
    pub(super) fn guard_preserves_values<'program>(
        machine: &'program typed_trees::machine::Machine,
        guard: typed_trees::expression::ExpressionHandle,
        call_frames: Option<&validation::CallFrameResolver<'program>>,
    ) -> bool {
        call_frames.is_some_and(|frames| {
            frames
                .expression_write_frame(machine, guard)
                .into_complete_paths()
                .is_some_and(|writes| writes.is_empty())
        })
    }

    pub(super) fn guard_conjunct_matches(
        program: &dyn ::validation::CheckingInput,
        guard: typed_trees::expression::ExpressionHandle,
        required_label: &str,
    ) -> bool {
        if program.expression_table.display_name(guard) == required_label {
            return true;
        }
        let Some(binary) = program.expression_table.binary_node(guard) else {
            return false;
        };
        match binary.operator {
            typed_trees::expression::BinaryOperator::And => {
                guard_conjunct_matches(program, binary.left, required_label)
                    || guard_conjunct_matches(program, binary.right, required_label)
            }
            typed_trees::expression::BinaryOperator::Equal
                if matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
            {
                guard_conjunct_matches(program, binary.left, required_label)
            }
            _ => false,
        }
    }

    /*
        The negated-arm read of `guard_conjunct_matches`: whether `!guard` -- the
        evidence a continuation edge carries for a comparison or `||` guard that
        evaluated false -- spells `required_label`. `!(a || b)` keeps each refuted
        operand as its own conjunct of evidence; `!(a && b)` refutes an
        unspecified operand and contributes nothing. Comparisons match through the
        label their complement operator spells (`!(v >= n)` reads `v < n`). The
        caller owes the guard's builtin-operator gate separately.
    */
    pub(super) fn negated_guard_conjunct_matches(
        program: &dyn ::validation::CheckingInput,
        guard: typed_trees::expression::ExpressionHandle,
        required_label: &str,
    ) -> bool {
        let Some(binary) = program.expression_table.binary_node(guard) else {
            return false;
        };
        match binary.operator {
            typed_trees::expression::BinaryOperator::Or => {
                negated_guard_conjunct_matches(program, binary.left, required_label)
                    || negated_guard_conjunct_matches(program, binary.right, required_label)
            }
            typed_trees::expression::BinaryOperator::Equal
                if matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
            {
                negated_guard_conjunct_matches(program, binary.left, required_label)
            }
            _ => binary.operator.complement().is_some_and(|complement| {
                format!(
                    "{} {} {}",
                    program.expression_table.display_name(binary.left),
                    complement.display_name(),
                    program.expression_table.display_name(binary.right)
                ) == required_label
            }),
        }
    }

    /*
        A premise conjunct may be a defined Boolean name rather than a spelled
        predicate. Bound-Boolean transport publishes `premise <-> X` as the clause
        pair `premise || !X` and `!premise || X` in the state's semantic contexts;
        under the selected edge's premise the refuted disjunct dies and the
        survivor stands as premise evidence. The assembly `zf` scratch recovers
        `destination == 0` this way once normalization threads the flag through a
        generated parameter, where no statement-level capture exists.
    */
    #[allow(clippy::too_many_arguments)]
    fn decomposed_guard_premise_proves(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        guard: ExpressionHandle,
        required: ExpressionHandle,
        required_label: &str,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let establishes = |premise: ExpressionHandle| {
            super::super::guard_operands::premise_reads_survive_arm_operand_writes(
                program,
                facts,
                state_flow,
                call_flow,
                premise,
                call_frames,
            ) && (guard_conjunct_matches(program, premise, required_label)
                || super::guard_observations::conjunct_matches(
                    program, facts, state_flow, call_flow, premise, required,
                ))
        };
        let mut conjuncts = Vec::new();
        premise_conjuncts(program, guard, &mut conjuncts);
        conjuncts
            .iter()
            .filter(|conjunct| premise_names_defined_boolean(program, **conjunct))
            .any(|conjunct| {
                contexts.iter().any(|context| {
                    facts
                        .semantic
                        .context_view(facts.semantic.contexts.get(*context))
                        .facts()
                        .any(|fact| {
                            let clause = match fact.payload {
                                FactPayload::BooleanExpression(expression)
                                | FactPayload::ContractBooleanExpression { expression, .. } => {
                                    expression
                                }
                                _ => return false,
                            };
                            let Some(binary) = program.expression_table.binary_node(clause) else {
                                return false;
                            };
                            binary.operator == typed_trees::expression::BinaryOperator::Or
                                && [(binary.left, binary.right), (binary.right, binary.left)]
                                    .into_iter()
                                    .any(|(refuted, surviving)| {
                                        expressions_are_logical_negations(
                                            program, refuted, *conjunct,
                                        ) && establishes(surviving)
                                    })
                        })
                })
            })
    }

    /*
        Whether a premise conjunct names a carried Boolean binding — a bare name,
        `!name`, or `name == <bool>` — the only spellings a transported
        definition clause can refute.
    */
    fn premise_names_defined_boolean(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => true,
            ExpressionNode::Unary(unary) => {
                unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
                    && matches!(
                        program.expression_table.expression(unary.operand),
                        ExpressionNode::Name(_)
                    )
            }
            ExpressionNode::Binary(binary) => {
                binary.operator == typed_trees::expression::BinaryOperator::Equal
                    && matches!(
                        program.expression_table.expression(binary.left),
                        ExpressionNode::Name(_)
                    )
                    && matches!(
                        program.expression_table.expression(binary.right),
                        ExpressionNode::Boolean(_)
                    )
            }
            _ => false,
        }
    }

    /*
        The same conjunct decomposition `guard_conjunct_matches` applies: `&&`
        splits and `x == true` unwraps to `x`.
    */
    fn premise_conjuncts(
        program: &dyn ::validation::CheckingInput,
        guard: ExpressionHandle,
        conjuncts: &mut Vec<ExpressionHandle>,
    ) {
        let Some(binary) = program.expression_table.binary_node(guard) else {
            conjuncts.push(guard);
            return;
        };
        match binary.operator {
            typed_trees::expression::BinaryOperator::And => {
                premise_conjuncts(program, binary.left, conjuncts);
                premise_conjuncts(program, binary.right, conjuncts);
            }
            typed_trees::expression::BinaryOperator::Equal
                if matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
            {
                premise_conjuncts(program, binary.left, conjuncts);
            }
            _ => conjuncts.push(guard),
        }
    }

    /*
        Whether `refuted` spells the logical negation of `guard` in either checked
        shape — `!guard`, `guard == false`, or `false == guard` — and in either
        direction. Spelling identity is the same comparison the conjunct matchers
        already apply to premise labels.
    */
    fn expressions_are_logical_negations(
        program: &dyn ::validation::CheckingInput,
        refuted: ExpressionHandle,
        guard: ExpressionHandle,
    ) -> bool {
        let negated = |expression| match program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary)
                if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
            {
                Some(unary.operand)
            }
            ExpressionNode::Binary(binary)
                if binary.operator == typed_trees::expression::BinaryOperator::Equal =>
            {
                if matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(false)
                ) {
                    Some(binary.left)
                } else {
                    matches!(
                        program.expression_table.expression(binary.left),
                        ExpressionNode::Boolean(false)
                    )
                    .then_some(binary.right)
                }
            }
            _ => None,
        };
        let same = |left: ExpressionHandle, right: ExpressionHandle| {
            program.expression_table.display_name(left)
                == program.expression_table.display_name(right)
        };
        negated(refuted).is_some_and(|inner| same(inner, guard))
            || negated(guard).is_some_and(|inner| same(inner, refuted))
    }
}
use labels::instantiate_state_parameter_label;
use predicate_domains::predicate_only_domain_predicates;
use state_preservation::{
    caller_state_preserves_field, caller_state_preserves_label_names,
    caller_state_preserves_self_fields_against_call_writes, collect_expression_self_fields,
};
use transition_guards::{
    guard_conjunct_matches, negated_guard_conjunct_matches, transition_guard_proves_requires,
};
pub(super) use transition_guards::{
    selected_guard_predicate, transition_guard_proves_boolean_requirement,
};

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use diagnostics::Diagnostic;
use facts::{FactPayload, FactPlace};
use symbols::SymbolHandle;

use super::prover::semantic_contexts_prove_contract_fact;
use crate::published::labels::{
    call_target_label, joined_place_label, machine_name, semantic_fact_requirement_label,
    symbol_name,
};

/*
    Whole-contract answers may depend on a completed alternative's exact
    contexts. Share repeated questions only within that same captured premise
    roster; never reuse one arm's positive result to qualify another arm.
*/
struct WholeContractCache<'program> {
    contexts: Vec<facts::FactContextHandle>,
    answers: Vec<(arena::Handle<checked_trees::ContractProofFact>, bool)>,
    current_values: std::cell::OnceCell<
        Option<crate::checks::current_scalar_values::CurrentScalarValues<'program>>,
    >,
}

pub(super) fn check_call_requires(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    guarantees: &mut crate::checks::contracts::prover::call_guarantees::StateGuarantees<'_, '_>,
    call_flow: &FlowCallFact,
    nominal_requirements: &super::nominal_inputs::DeclaredFieldRequirements,
    incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    constant_obligations: &mut Vec<checked_trees::ConstantPredicateObligation>,
    recipe_obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
    value_qualifications: &mut arena::Arena<checked_trees::CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let state_flow = guarantees.caller();
    let entry_contexts: Vec<_> = facts
        .flow
        .state_call_entry_semantic_contexts(
            state_flow,
            call_flow.statement_index,
            call_flow.call_ordinal,
            call_flow.target_symbol,
            call_flow.receiver_symbol,
        )
        .collect();
    {
        super::contextual_cleanup::check(
            program,
            facts,
            state_flow,
            call_flow,
            &entry_contexts,
            call_frames,
            diagnostics,
        );
    };
    {
        super::nominal_inputs::check(
            program,
            facts,
            state_flow,
            call_flow,
            nominal_requirements,
            diagnostics,
        );
    };
    {
        check_lender_domain_restoration(program, facts, state_flow, call_flow, diagnostics);
    };
    let mut whole_contract_caches = Vec::<WholeContractCache>::new();
    for requires_context in facts
        .flow
        .semantic_constraint_contexts(call_flow.requires_constraints)
    {
        let context = facts.semantic.contexts.get(requires_context);
        for fact in facts.semantic.context_view(context).facts() {
            let membership = super::call_values::membership_evidence(
                program, facts, state_flow, call_flow, fact,
            );
            let mut proves_case = |case: Option<&super::call_values::MembershipCase>| {
                let requirement_contexts =
                    case.map_or(entry_contexts.as_slice(), |case| case.contexts.as_slice());
                let cache_index = whole_contract_caches
                    .iter()
                    .position(|cache| cache.contexts.as_slice() == requirement_contexts)
                    .unwrap_or_else(|| {
                        whole_contract_caches.push(WholeContractCache {
                            contexts: requirement_contexts.to_vec(),
                            answers: Vec::new(),
                            current_values: std::cell::OnceCell::new(),
                        });
                        whole_contract_caches.len() - 1
                    });
                let captured_judgment = captured_boolean::judgment(
                    program,
                    facts,
                    guarantees,
                    call_flow,
                    requirement_contexts,
                    fact,
                    incoming_guards,
                    call_frames,
                    &mut whole_contract_caches[cache_index],
                );
                let satisfied = {
                    selected_evidence_proves_requirement(facts, state_flow, call_flow, fact)
                        || captured_judgment.unwrap_or_else(|| match fact.payload {
                            FactPayload::ContractBooleanExpression { expression, .. } => {
                                boolean_requirements::proves(
                                    program,
                                    facts,
                                    state_flow,
                                    call_flow,
                                    requirement_contexts,
                                    expression,
                                    incoming_guards,
                                    call_frames,
                                    guarantees.completed_calls(),
                                )
                            }
                            _ => {
                                semantic_contexts_prove_contract_fact(
                                    program,
                                    facts,
                                    facts::ProgramPoint::Statement {
                                        machine_symbol: state_flow.machine_symbol,
                                        state_symbol: state_flow.state_symbol,
                                        statement_index: call_flow.statement_index,
                                    },
                                    requirement_contexts,
                                    fact,
                                ) || transparent_requirement_proves(
                                    program, facts, state_flow, call_flow, fact,
                                )
                            }
                        })
                };
                let satisfied = satisfied
                    || (captured_judgment.is_none()
                        && proven_predicates_grant_domain(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            requirement_contexts,
                            fact,
                            call_frames,
                        ))
                    || (captured_judgment.is_none()
                        && !super::prover::indexed_membership(program, fact.payload)
                        && (transition_guard_proves_requires(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            fact,
                            call_frames,
                        ) || string_literal_grants_domain(
                            program,
                            &facts.semantic,
                            fact.payload,
                            fact.place,
                        ) || value_call_return_domain_grants(
                            program,
                            &facts.semantic,
                            fact.payload,
                            fact.place,
                        ) || qualified_argument_grants_domain(
                            program,
                            facts,
                            requirement_contexts,
                            fact,
                            membership
                                .as_ref()
                                .and_then(|evidence| evidence.whole_argument),
                        ) || subslice_grants_domain(
                            program,
                            facts,
                            requirement_contexts,
                            fact.payload,
                            fact.place,
                        ) || parameter_domain_grants(
                            program,
                            facts,
                            state_flow,
                            fact.payload,
                            fact.place,
                        ) || super::reference_domains::proves(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            requirement_contexts,
                            fact,
                            case.and_then(|case| case.referents.as_deref()),
                            call_frames,
                        )));

                let satisfied = satisfied
                    || crate::checks::termination::profile_requirement_covered(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        fact,
                        call_frames,
                    );
                satisfied
                    && super::prover::contract_membership_place_is_accessible(
                        program,
                        &facts.semantic,
                        requirement_contexts,
                        fact,
                    )
            };
            let satisfied = match membership.as_ref() {
                None => proves_case(None),
                Some(evidence) => {
                    !evidence.cases.is_empty()
                        && evidence.cases.iter().all(|case| proves_case(Some(case)))
                }
            };
            if satisfied
                && let Some(expression) = membership
                    .as_ref()
                    .and_then(|evidence| evidence.whole_argument)
                && let Some((_, _, domain_symbol, semantic_domain)) =
                    fact.payload.domain_membership()
                && let Ok(statement_index) = u32::try_from(call_flow.statement_index)
            {
                value_qualifications.append(checked_trees::CheckedValueQualification {
                    machine: state_flow.machine_symbol,
                    state: state_flow.state_symbol,
                    statement_index,
                    expression,
                    semantic_domain,
                    domain_symbol,
                    structural_value: checked_trees::CheckedStructuralValueHandle::invalid(),
                });
            }
            if !satisfied {
                if let FactPayload::ContractBooleanExpression { expression, .. } = fact.payload
                    && super::prover::has_builtin_operators(program, &facts.operators, expression)
                    && let Some(parameters) =
                        validation::call_target_parameters(program, call_flow.target_symbol)
                    && let Some(arguments) = super::call_values::captured_argument_expressions(
                        program, facts, state_flow, call_flow,
                    )
                    && parameters.len() == arguments.len()
                    && let Some(obligations) = super::argument_intervals::recipe_requirements(
                        program,
                        state_flow.machine_symbol,
                        program
                            .machine_by_own_or_state_symbol(call_flow.target_symbol)
                            .map_or(call_flow.target_symbol, |machine| machine.symbol),
                        expression,
                        &|subject| {
                            let name = program.expression_table.name_path_node(subject)?;
                            if !program.expression_table.name_path_is_single_member(name) {
                                return None;
                            }
                            let position = parameters
                                .iter()
                                .position(|parameter| parameter.symbol == name.symbol)?;
                            program
                                .primitive_type_reference(parameters[position].type_reference)
                                .filter(|primitive| primitive.accepts_integer_literal())?;
                            arguments.get(position).copied()
                        },
                        &format!(
                            "requires contract for call {} from {} state {} statement {}",
                            call_target_label(program, call_flow.target_symbol),
                            machine_name(program, state_flow.machine_symbol),
                            symbol_name(program, state_flow.state_symbol),
                            call_flow.statement_index,
                        ),
                    )
                {
                    recipe_obligations.extend(obligations);
                    continue;
                }
                if let FactPayload::ContractBooleanExpression { expression, .. } = fact.payload
                    && let Some((target_machine, target_state)) =
                        validation::find_state(program, call_flow.target_symbol)
                    && let Some(obligation) = checked_trees::ConstantPredicateObligation::retain(
                        program,
                        expression,
                        target_machine.symbol,
                        state_flow.machine_symbol,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        format!(
                            "requires contract for call {} from {}: {}",
                            call_target_label(program, call_flow.target_symbol),
                            machine_name(program, state_flow.machine_symbol),
                            semantic_fact_requirement_label(program, &facts.semantic, fact)
                        ),
                        &|expression| {
                            validation::has_builtin_binary_expression_meaning(
                                program,
                                target_machine,
                                Some(target_state),
                                expression,
                            )
                        },
                    )
                {
                    constant_obligations.push(obligation);
                    continue;
                }
                let requirement = present_captured_parameter(&semantic_fact_requirement_label(
                    program,
                    &facts.semantic,
                    fact,
                ));
                let authored_requirement = match fact.payload {
                    FactPayload::ContractBooleanExpression { expression, .. } => {
                        authored_requirement_label(program, state_flow, call_flow, expression)
                            .filter(|authored| authored != &requirement)
                    }
                    _ => None,
                };
                let detail = match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. }
                    | FactPayload::DomainMembership { domain_symbol, .. } => {
                        let Some(place) = fact.place.as_place() else {
                            unreachable!("contract domain membership already handled above")
                        };
                        explain_domain_requirement_failure(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            place,
                            domain_symbol,
                        )
                    }
                    FactPayload::ContractBooleanExpression { expression, .. } => {
                        explain_missing_boolean_fact(program, expression)
                    }
                    _ => None,
                };
                let diagnostic = Diagnostic::error(format!(
                    "cannot prove requires contract for call {} from {}: {}{} in state `{}`{}",
                    call_target_label(program, call_flow.target_symbol),
                    machine_name(program, state_flow.machine_symbol),
                    requirement,
                    detail
                        .map(|message| format!(" ({message})"))
                        .unwrap_or_default(),
                    symbol_name(program, state_flow.state_symbol),
                    authored_requirement
                        .map(|authored| format!("; authored requirement `{authored}`"))
                        .unwrap_or_default()
                ));
                diagnostics.push(match call_source_span(program, state_flow, call_flow) {
                    Some(span) => diagnostic.with_source_span(span),
                    None => diagnostic,
                });
            }
        }
    }
}

/*
    A captured generic parameter is a member of a saved subject. The authored
    requirement names that parameter. The capture carrier is not part of it.
*/
fn present_captured_parameter(label: &str) -> String {
    let marker = "<saved subject #";
    let mut rest = label;
    let mut presented = String::new();
    while let Some(start) = rest.find(marker) {
        presented.push_str(&rest[..start]);
        let after_marker = &rest[start + marker.len()..];
        let Some(close) = after_marker.find('>') else {
            presented.push_str(&rest[start..]);
            return presented;
        };
        let identity = &after_marker[..close];
        let tail = &after_marker[close + 1..];
        if identity
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b':')
            && let Some(stripped) = tail.strip_prefix('.')
        {
            rest = stripped;
            continue;
        }
        presented.push_str(&rest[start..start + marker.len() + close + 1]);
        rest = tail;
    }
    presented.push_str(rest);
    presented
}

/*
    The authored invocation a requires diagnostic should point at: the call
    expression the traversal captured, else the authored span of the statement
    at the call's coordinate (a call statement or named transition, which have
    no expression handle). Compiler-generated occurrences keep the default
    span, so they yield `None` rather than an invented location.
*/
fn call_source_span(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
) -> Option<source::SourceSpan> {
    if call_flow.authored_expression.is_valid() {
        return program
            .expression_table
            .try_source_span(call_flow.authored_expression)
            .filter(|span| *span != source::SourceSpan::default());
    }
    let (_, state) = validation::find_state(program, state_flow.state_symbol)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(call_flow.statement_index)?;
    validation::statement_source_span(program, statement)
}

/*
    A transparent proposition requirement holds when the calling state's own
    premises prove its unfolded definition under the call's argument terms.
    Those premises describe state entry, so any earlier write in the state
    withdraws the route.
*/
fn transparent_requirement_proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fact: &facts::Fact,
) -> bool {
    let FactPayload::ContractPropositionApplication { fact: source, .. } = fact.payload else {
        return false;
    };
    let Some(application) = program.proof_facts.get(source).as_proposition() else {
        return false;
    };
    let Some((caller, caller_state)) = validation::find_state(program, state_flow.state_symbol)
        .filter(|(machine, _)| machine.symbol == state_flow.machine_symbol)
    else {
        return false;
    };
    let Some((callee, callee_state)) = validation::find_state(program, call_flow.target_symbol)
    else {
        return false;
    };
    facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
        .next()
        .is_none()
        && validation::transparent_call_requirement_entailed(
            program,
            caller,
            caller_state,
            callee,
            callee_state,
            facts
                .flow
                .control
                .call_arguments
                .span_or_empty(call_flow.authored_arguments),
            application,
        )
}

/*
    ch8 construction-grant: a string literal grants a domain `D` -- satisfying a
    `requires <arg> in D` membership without a validating boundary call -- iff
    `D`'s sole fact is a recognized comptime byte-predicate over `self`
    (`valid_utf8`/`no_nul`/`ascii_only`) AND that predicate holds for the
    literal's compile-time bytes. Reuses the shared comptime byte-predicate
    primitives (`super::grants`); the subject must be the literal itself (an
    expression-rooted place with no field/index segments), not a derived place.
    A subslice `base[a..b]` satisfies a `requires <arg> in D` domain obligation
    when D's byte-predicate fact is SUBSLICE-PRESERVING (such as
    `no_nul`/`ascii_only`) and the `base` is itself provably in a domain implying
    D. Sound because a contiguous subslice's bytes are a subset of the whole's, so
    any per-byte character class the whole satisfies, the subslice does too. This
    is the subslice analog of the concat-domain grant (`value_proves_domain`);
    `base`'s membership is matched against the entry-context domain facts exactly
    as the concat/param discharge does. (Correctly EXCLUDES `valid_utf8` — a
    subslice can cut a multi-byte scalar — and `non_empty` — `base[a..a]` is empty.)
*/
fn subslice_grants_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    entry_contexts: &[facts::FactContextHandle],
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let Some((_, _, domain_symbol, _)) = payload.domain_membership() else {
        return false;
    };
    let Some(place_handle) = place.as_place() else {
        return false;
    };
    let Some(subject) = crate::flow::canonical_place_from_semantic_place(
        program,
        &facts.semantic,
        facts.semantic.places.get(place_handle),
    ) else {
        return false;
    };
    super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        entry_contexts,
        &subject,
        domain_symbol,
    )
}

/*
    A SHARED (`&`, non-`mut`) parameter's DECLARED domain is invariant for the
    state's lifetime: any interleaved call receives that parameter as a shared
    borrow (or not at all) and cannot mutate its bytes -- Omega's
    shared-XOR-mutable borrow discipline keeps any aliased backing frozen while
    the shared borrow is live -- so the parameter's declared domain still holds
    EVEN after an interleaved `&mut self` call whose conservative fact
    invalidation dropped the flow-tracked `<param> in <declared>` fact (e.g. an
    empty-mutation-summary helper that reduces to the blunt "wipe every context"
    path). A `requires <param> in D` obligation is therefore satisfied whenever
    the parameter's declared domain implies `D`. This is the same trust basis as
    a declared param/return domain at a use site (`value_call_return_domain_grants`);
    the subject must be the PARAMETER ITSELF (a symbol-rooted place with no
    derived segments -- a derived place `param.field`/`param[i]` may carry a
    different domain). Restricted to a non-mutable, non-self parameter: a
    `&mut`-borrowed parameter's bytes CAN change, so its domain is not invariant.
*/
fn parameter_domain_grants(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let Some((_, _, domain_symbol, _)) = payload.domain_membership() else {
        return false;
    };
    let Some(place_handle) = place.as_place() else {
        return false;
    };
    let resolved = *facts.semantic.places.get(place_handle);
    if !facts
        .semantic
        .place_segments
        .span_or_empty(resolved.segments)
        .is_empty()
    {
        return false;
    }
    let Some(root_symbol) = resolved.root.as_symbol() else {
        return false;
    };
    let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
        return false;
    };
    let Some(parameter) = program.state_parameter_by_symbol(state, root_symbol) else {
        return false;
    };
    if parameter.is_mutable {
        return false;
    }
    validation::field_domains::domain_constraint_symbols(program, parameter.type_reference)
        .into_iter()
        .any(|param_domain| {
            facts.semantic.domain_implies(param_domain, domain_symbol)
                || validation::field_domains::domain_membership_implies(
                    program,
                    param_domain,
                    domain_symbol,
                )
        })
}

/*
    The `domain_symbol` and no-segment expression root a membership `place`
    names — the shared peel both grant checks apply before their payload
    tests.
*/
fn expression_rooted_domain_parts(
    semantic: &facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> Option<(
    symbols::SymbolHandle,
    typed_trees::expression::ExpressionHandle,
)> {
    let (_, _, domain_symbol, _) = payload.domain_membership()?;
    let place_handle = place.as_place()?;
    let resolved = semantic.places.get(place_handle);
    if !resolved.segments.is_empty() {
        return None;
    }
    let expression = resolved.root.as_expression()?;
    Some((domain_symbol, expression))
}

fn string_literal_grants_domain(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let Some((domain_symbol, expression)) =
        expression_rooted_domain_parts(semantic, payload, place)
    else {
        return false;
    };
    validation::field_domains::string_literal_expression_grants_domain(
        program,
        expression,
        domain_symbol,
    )
}

/*
    #66 return-domain forwarding: a `requires <arg> in D` obligation whose argument
    is a VALUE CALL (`self.direction_command(direction)`) is satisfied when the
    callee's DECLARED return type carries a domain implying `D`. This is the same
    trust basis as a declared param domain at a call site (the signature's domain
    is trusted at use sites; scalar result domains are enforced independently at
    ordinary exits in exits/result_domains.rs) -- and the call-argument analog
    of the field write that trusts a declared return domain (checks/contracts/writes.rs
    `value_call_return_domain_implies`). The subject must be the call expression
    itself (an expression-rooted place with no field/index segments).
*/
fn value_call_return_domain_grants(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let Some((domain_symbol, expression)) =
        expression_rooted_domain_parts(semantic, payload, place)
    else {
        return false;
    };
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let Some(target) = program.state_by_symbol(call.target_symbol) else {
        return false;
    };
    if !target.return_type.is_valid() {
        return false;
    }
    validation::field_domains::predicate_domain_constraint_symbols(program, target.return_type)
        .into_iter()
        .any(|return_domain| {
            semantic.domain_implies(return_domain, domain_symbol)
                || validation::field_domains::domain_membership_implies(
                    program,
                    return_domain,
                    domain_symbol,
                )
        })
}

/*
    Every requirement route's static coordinates for one call occurrence: the
    caller's own machine and state, the exact invocation site inside it, and
    the callee's declared parameters. The routes used to spell this lookup
    chain themselves, one gate at a time, so one resolver keeps them agreeing
    about which lookups must succeed before a proof may run.
*/
fn call_coordinates<'p>(
    program: &'p dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
) -> Option<(
    &'p typed_trees::machine::Machine,
    &'p typed_trees::state::State,
    validation::CallSite<'p>,
    &'p [typed_trees::signature::StateParameter],
)> {
    let (machine, state) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    )?;
    let call_site = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    )?;
    let target_parameters = validation::call_target_parameters(program, call_flow.target_symbol)?;
    Some((machine, state, call_site, target_parameters))
}

/*
    Diagnostics retain the caller's operand spelling beside the canonical
    backing label. Normalizing a loan to its backing is useful to the prover,
    but must not hide which authored view owes the current-content obligation.
    This rendering supplies no evidence and never participates in proof.
*/
fn authored_requirement_label(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    call: &FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<String> {
    use typed_trees::proposition::ProofSubstitutions;
    let (_, _, site, parameters) = call_coordinates(program, state, call)?;
    let arguments = validation::call_site_argument_expressions(program, &site);
    if arguments.len() != parameters.len() {
        return None;
    }
    let substitutions: Vec<_> = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, &argument)| {
            let argument = match program.expression_table.expression(argument) {
                typed_trees::expression::ExpressionNode::Borrow(borrow)
                    if validation::reference_shell(program, parameter.type_reference).is_some() =>
                {
                    borrow.target
                }
                _ => argument,
            };
            (
                parameter.symbol,
                parameter.name.to_string(),
                program.render_proof_expression(argument, ProofSubstitutions::None),
            )
        })
        .collect();
    Some(
        program
            .render_proof_expression(expression, ProofSubstitutions::ByParameter(&substitutions)),
    )
}

fn proven_predicates_grant_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let facts::FactPayload::ContractDomainMembership {
        value,
        domain_symbol,
        semantic_domain,
        ..
    } = fact.payload
    else {
        return false;
    };
    let Some(domain) = program.domain_definition_for_instance(domain_symbol, semantic_domain)
    else {
        return false;
    };
    let Some(predicates) = predicate_only_domain_predicates(program, domain) else {
        return false;
    };
    let Some(subject) = validation::canonical_place_from_expression(program, value) else {
        return false;
    };
    let Some(goals) = crate::published::facts::boolean_predicates::domain_predicates_for_subject(
        program,
        &facts.operators,
        domain,
        &subject,
    ) else {
        return false;
    };
    goals.len() == predicates.len()
        && goals.iter().all(|goal| {
            super::call_values::proves_bound_predicate(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                goal,
                call_frames,
                &|observation| {
                    super::call_values::observation_actual(
                        program,
                        state_flow,
                        call_flow,
                        observation,
                    )
                },
            )
        })
}

/*
    Clear "needs fact X here" guidance for a proof-backed operator/contract that
    is missing a required boolean fact (for example an index bound or a
    domain-sensitive operator precondition). The caller has not established the
    fact in the entry context, so name exactly what must hold before the call.
*/
fn explain_missing_boolean_fact(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<String> {
    let fact = program.expression_table.display_name(expression);
    if fact.is_empty() {
        return None;
    }
    Some(format!(
        "needs fact `{fact}` here; establish it before the call via a prior \
         contract guarantee, domain membership, or guard"
    ))
}

fn explain_domain_requirement_failure(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    required_place: facts::PlaceHandle,
    required_domain: SymbolHandle,
) -> Option<String> {
    let mut detail = None;
    for invalidation in facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
    {
        let fact = facts.semantic.facts.get(invalidation.fact);
        let (fact_domain, fact_place) = match fact.payload {
            facts::domain_membership_pattern!(domain_symbol) => {
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                (domain_symbol, place)
            }
            _ => continue,
        };

        if (!facts.semantic.domain_implies(fact_domain, required_domain)
            && !validation::field_domains::domain_membership_implies(
                program,
                fact_domain,
                required_domain,
            ))
            || !facts
                .semantic
                .places_match(program, fact_place, required_place)
        {
            continue;
        }

        let fact_place = facts.semantic.places.get(fact_place);
        let dependency_segments = facts
            .flow
            .invalidations
            .segments
            .span_or_empty(invalidation.dependency_segments);
        let invalidated =
            joined_place_label(program, &facts.semantic, fact_place, dependency_segments);
        let mutated = facts::canonical_place_label_from_parts(
            program,
            invalidation.mutated_root,
            facts
                .flow
                .invalidations
                .segments
                .span_or_empty(invalidation.mutated_segments),
        );
        detail = Some(format!(
            "invalidated by prior mutation of {mutated}; {invalidated} is part of {}",
            symbol_name(program, required_domain)
        ));
    }

    detail
}

/*
    A caller-state incoming guard (non-negated, dominating, rewrite-fenced by
    the ranges walk-back) whose spelling matches the requires expression --
    exactly (`self.a <= self.b`), as an `&&` conjunct, or through the
    multi-arm `(subject) == true` desugar. The caller state itself must also
    preserve every field the expression names (whole-state: any assignment
    mentioning one, or any call statement, defeats the route) and every
    unqualified operand name the instantiated requirement spells (a rebinding
    such as `limit = 0` leaves the label match quoting a stale premise), and
    the jump's own earlier operands must leave the requirement's reads
    unwritten (`guard_operands`).
*/
fn incoming_guard_proves_requires(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((_machine, state, call_site, target_parameters)) =
        call_coordinates(program, state_flow, call_flow)
    else {
        return false;
    };
    let required_label = super::labels::instantiate_call_contract_expression_label(
        program,
        state_flow.state_symbol,
        call_flow.statement_index,
        &call_site,
        target_parameters,
        expression,
    );
    let guard_matches = incoming
        .iter()
        .filter(|guard| guard.applies_at(state_flow.state_symbol))
        .any(|guard| {
            if guard.is_negated() {
                super::prover::has_builtin_operators(program, &facts.operators, guard.guard())
                    && (negated_guard_conjunct_matches(program, guard.guard(), &required_label)
                        || guard.direct_arguments().is_some_and(|arguments| {
                            let instantiated = instantiate_state_parameter_label(
                                program,
                                state,
                                arguments,
                                &required_label,
                            );
                            negated_guard_conjunct_matches(program, guard.guard(), &instantiated)
                        }))
            } else {
                guard_conjunct_matches(program, guard.guard(), &required_label)
                    || guard.direct_arguments().is_some_and(|arguments| {
                        let instantiated = instantiate_state_parameter_label(
                            program,
                            state,
                            arguments,
                            &required_label,
                        );
                        guard_conjunct_matches(program, guard.guard(), &instantiated)
                    })
            }
        });
    if !guard_matches {
        return false;
    }
    let mut fields: Vec<typed_trees::name::Identifier> = Vec::new();
    collect_expression_self_fields(program, expression, &mut fields);
    let arguments = validation::call_site_argument_expressions(program, &call_site);
    let mentioned = super::guard_operands::boolean_requirement_mentions(program, expression);
    fields
        .iter()
        .all(|field| caller_state_preserves_field(program, state, field))
        && caller_state_preserves_self_fields_against_call_writes(
            program, facts, state_flow, call_flow, &fields,
        )
        && caller_state_preserves_label_names(
            program,
            facts,
            state_flow,
            call_flow,
            state,
            &required_label,
            &super::guard_operands::requirement_argument_reads(
                arguments,
                target_parameters,
                &mentioned,
            ),
        )
        && super::guard_operands::requirement_reads_survive_earlier_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            arguments,
            target_parameters,
            &mentioned,
            call_frames,
        )
}

/*
    A selected Type witness already inhabited this exact requires lane after
    completed-operand substitution in `evidence`. Re-rendering its subject as a
    current storage label would discard the call snapshot and reject preserved
    copies (or accept a later write). This consumes only the bound lane of this
    exact call; it imports no unconditional proposition or runtime authority.
*/
fn selected_evidence_proves_requirement(
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &facts::Fact,
) -> bool {
    let FactPayload::ContractPropositionApplication { fact, .. } = requirement.payload else {
        return false;
    };
    facts.proof.contract_calls.iter().any(|(_, candidate)| {
        candidate.caller_machine_symbol == state.machine_symbol
            && candidate.caller_state_symbol == state.state_symbol
            && candidate.statement_index == call.statement_index
            && candidate.call_ordinal == call.call_ordinal
            && candidate.target_state_symbol == call.target_symbol
            && facts
                .proof
                .contract_evidence_arguments
                .span_or_empty(candidate.evidence_arguments)
                .iter()
                .any(|binding| {
                    facts.proof.outcome_specific_arms.iter().any(|(_, arm)| {
                        arm.rows
                            .iter()
                            .any(|row| row.selected_term == Some(binding.source))
                    }) && facts
                        .proof
                        .contract_fact_refs
                        .span_or_empty(candidate.requires)
                        .iter()
                        .any(|reference| {
                            let contract = facts.proof.contract_facts.get(reference.fact);
                            contract.fact == fact
                                && contract.evidence_term == Some(binding.parameter)
                        })
                })
    })
}

/*
    An argument written `x as T in D` names its operand's storage place, while
    flow publishes the checked qualification's membership on the cast itself.
    That membership establishes the formal's requirement for this exact value.
*/
fn qualified_argument_grants_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[facts::FactContextHandle],
    requirement: &facts::Fact,
    argument: Option<typed_trees::expression::ExpressionHandle>,
) -> bool {
    let Some(argument) = argument else {
        return false;
    };
    let Some((_, _, domain_symbol, semantic_domain)) = requirement.payload.domain_membership()
    else {
        return false;
    };
    let qualified = matches!(
        program.expression_table.expression(argument),
        typed_trees::expression::ExpressionNode::Cast(cast) if !cast.semantic_domain.is_empty()
    );
    qualified
        && super::exact_scalar_membership(
            program,
            facts.view(),
            contexts,
            &validation::CanonicalPlace {
                root: facts::PlaceRoot::Expression(argument),
                segments: Vec::new(),
            },
            domain_symbol,
            semantic_domain,
        )
}

mod restoration_memberships {
    use facts::{FactPayload, FactPlace};
    use language_semantics::SemanticDomainId;
    use std::hash::{DefaultHasher, Hash, Hasher};
    use symbols::SymbolHandle;
    use validation::CanonicalPlace;

    struct Membership {
        place: CanonicalPlace,
        domain: SymbolHandle,
        instance: SemanticDomainId,
    }

    // Prepare the call's exit records once, so each lent field searches only its
    // own records. These are candidates; the original exact predicate decides.
    pub(super) struct Memberships {
        rows: Vec<Membership>,
        places: facts::places::PlaceBucketIndex,
    }

    fn key(place: &CanonicalPlace, domain: SymbolHandle) -> u64 {
        let mut hash = DefaultHasher::new();
        (place, domain).hash(&mut hash);
        hash.finish()
    }

    fn ordinary(
        place: FactPlace,
        payload: FactPayload,
    ) -> Option<(facts::PlaceHandle, SymbolHandle, SemanticDomainId)> {
        match (place, payload) {
            (
                FactPlace::Place(place),
                FactPayload::DomainMembership {
                    domain_symbol,
                    semantic_domain,
                    ..
                },
            ) => Some((place, domain_symbol, semantic_domain)),
            _ => None,
        }
    }

    impl Memberships {
        pub(super) fn prepare(
            program: &dyn validation::CheckingInput,
            semantic: &facts::FactPlan,
            contexts: &[facts::FactContextHandle],
        ) -> Self {
            let mut rows = Vec::new();
            for &handle in contexts {
                let context = semantic.contexts.get(handle);
                for fact in semantic.context_view(context).facts() {
                    let Some((place, domain, instance)) = ordinary(fact.place, fact.payload) else {
                        continue;
                    };
                    let Some(place) = crate::flow::canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(place),
                    ) else {
                        continue;
                    };
                    rows.push(Membership {
                        place,
                        domain,
                        instance,
                    });
                }
            }
            Self::from_rows(rows)
        }

        fn from_rows(rows: Vec<Membership>) -> Self {
            let places = facts::places::PlaceBucketIndex::new(
                rows.iter().map(|row| key(&row.place, row.domain)),
            );
            Self { rows, places }
        }

        fn candidates(
            &self,
            place: &CanonicalPlace,
            domain: SymbolHandle,
        ) -> impl Iterator<Item = &Membership> {
            self.places
                .candidates(key(place, domain))
                .map(|position| &self.rows[position])
        }

        pub(super) fn contains(
            &self,
            place: &CanonicalPlace,
            domain: SymbolHandle,
            instance: SemanticDomainId,
        ) -> bool {
            self.candidates(place, domain).any(|row| {
                row.domain == domain
                    && (!instance.is_valid() || row.instance == instance)
                    && row.place == *place
            })
        }
    }

    // The first declaration still supplies the obligation, even when another
    // row names a different instance of the same domain for that field.
    #[derive(Default)]
    pub(super) struct Obligations {
        seen: std::collections::HashSet<(CanonicalPlace, SymbolHandle)>,
    }

    impl Obligations {
        pub(super) fn first(&mut self, place: &CanonicalPlace, domain: SymbolHandle) -> bool {
            self.seen.insert((place.clone(), domain))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{Membership, Memberships, Obligations, key, ordinary};
        use facts::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
        use language_semantics::SemanticDomainId;
        use symbols::SymbolHandle;
        use validation::CanonicalPlace;

        #[test]
        fn exact_restoration_candidates_preserve_instances_and_first_obligations() {
            let domain = SymbolHandle::from_parts(7, 3);
            let place = CanonicalPlace {
                root: PlaceRoot::Symbol(SymbolHandle::from_parts(11, 2)),
                segments: vec![
                    PlaceSegment::Field {
                        symbol: SymbolHandle::from_parts(19, 1),
                    },
                    PlaceSegment::FixedIndex { index: 4 },
                ],
            };
            let other_field = CanonicalPlace {
                segments: vec![PlaceSegment::Field {
                    symbol: SymbolHandle::from_parts(20, 1),
                }],
                ..place.clone()
            };
            let other_root = CanonicalPlace {
                root: PlaceRoot::Symbol(SymbolHandle::from_parts(11, 3)),
                ..place.clone()
            };
            let other_domain = SymbolHandle::from_parts(7, 4);
            let rows = (0..1024)
                .map(|position| Membership {
                    place: match position % 4 {
                        0 => place.clone(),
                        1 => other_field.clone(),
                        2 => other_root.clone(),
                        _ => place.clone(),
                    },
                    domain: if position % 4 == 3 {
                        other_domain
                    } else {
                        domain
                    },
                    instance: if position % 7 == 0 {
                        SemanticDomainId::NULL
                    } else {
                        SemanticDomainId((position % 5 + 1) as u32)
                    },
                })
                .collect();
            let prepared = Memberships::from_rows(rows);
            let mut direct_scans = 0;
            let mut indexed_scans = 0;
            for requested in [&place, &other_field, &other_root] {
                for selected_domain in [domain, other_domain, SymbolHandle::from_parts(30, 1)] {
                    for instance in [
                        SemanticDomainId::NULL,
                        SemanticDomainId(1),
                        SemanticDomainId(5),
                        SemanticDomainId(99),
                    ] {
                        let matches = |row: &&Membership| {
                            row.domain == selected_domain
                                && (!instance.is_valid() || row.instance == instance)
                                && row.place == *requested
                        };
                        let direct = prepared
                            .rows
                            .iter()
                            .inspect(|_| direct_scans += 1)
                            .find(matches);
                        let indexed = prepared
                            .candidates(requested, selected_domain)
                            .inspect(|_| indexed_scans += 1)
                            .find(matches);
                        assert_eq!(
                            direct.map(|row| (row.instance, &row.place)),
                            indexed.map(|row| (row.instance, &row.place))
                        );
                        assert_eq!(
                            prepared.contains(requested, selected_domain, instance),
                            direct.is_some()
                        );
                    }
                }
            }
            eprintln!(
                "restoration membership table: {} prepared rows; direct {} visited rows; indexed {} visited rows",
                prepared.rows.len(),
                direct_scans,
                indexed_scans
            );
            assert!(indexed_scans < direct_scans);
            // Deliberately offer every row for each requested bucket. A root,
            // field, domain-generation or instance mismatch must still refuse.
            let absent_root = CanonicalPlace {
                root: PlaceRoot::Symbol(SymbolHandle::from_parts(11, 9)),
                ..place.clone()
            };
            let absent_field = CanonicalPlace {
                segments: vec![
                    PlaceSegment::Field {
                        symbol: SymbolHandle::from_parts(19, 1),
                    },
                    PlaceSegment::FixedIndex { index: 5 },
                ],
                ..place.clone()
            };
            let mut collided = Memberships {
                places: facts::places::PlaceBucketIndex::new(std::iter::empty()),
                rows: prepared.rows,
            };
            for (requested, selected_domain, instance, expected) in [
                (&place, domain, SemanticDomainId(1), true),
                (&place, domain, SemanticDomainId::NULL, true),
                (&place, domain, SemanticDomainId(99), false),
                (&absent_root, domain, SemanticDomainId(1), false),
                (&absent_field, domain, SemanticDomainId(1), false),
                (
                    &place,
                    SymbolHandle::from_parts(7, 9),
                    SemanticDomainId(1),
                    false,
                ),
            ] {
                collided.places = facts::places::PlaceBucketIndex::new(
                    collided
                        .rows
                        .iter()
                        .map(|_| key(requested, selected_domain)),
                );
                assert_eq!(
                    collided.contains(requested, selected_domain, instance),
                    expected
                );
            }
            let mut direct = Vec::new();
            let mut indexed = Obligations::default();
            for (path, selected_domain, instance) in [
                (&place, domain, 1),
                (&place, domain, 2),
                (&other_field, domain, 3),
                (&place, other_domain, 4),
                (&other_root, domain, 5),
            ] {
                let is_first = !direct.contains(&(path.clone(), selected_domain));
                if is_first {
                    direct.push((path.clone(), selected_domain));
                }
                assert_eq!(
                    indexed.first(path, selected_domain),
                    is_first,
                    "instance {instance} changes no dedup key"
                );
            }
            let raw_place = facts::PlaceHandle::from_parts(1, 1);
            let payload = FactPayload::DomainMembership {
                value: Default::default(),
                domain: Default::default(),
                domain_symbol: domain,
                semantic_domain: SemanticDomainId(1),
            };
            assert_eq!(
                ordinary(FactPlace::Place(raw_place), payload),
                Some((raw_place, domain, SemanticDomainId(1)))
            );
            let contract = FactPayload::ContractDomainMembership {
                kind: facts::ContractFactKind::Ensures,
                fact: Default::default(),
                value: Default::default(),
                domain: Default::default(),
                domain_symbol: domain,
                semantic_domain: SemanticDomainId(1),
            };
            assert_eq!(ordinary(FactPlace::Place(raw_place), contract), None);
            assert_eq!(ordinary(FactPlace::Unknown, payload), None);
        }
    }
}

/*
    A local declared with a domain qualification (`let mut bytes: [u8; 2] in Ascii`)
    retains that obligation across exclusive loans (`&mut`, `&write`).
    If a callee mutates the lent storage without an `ensures` clause restoring
    the declared domain, the call statement's exit contexts cannot prove the
    declared domain for that storage, rendering the call inadmissible.
*/
fn check_lender_domain_restoration(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parameters) = validation::call_target_parameters(program, call_flow.target_symbol)
    else {
        return;
    };
    if !parameters.iter().any(|p| {
        validation::reference_shell(program, p.type_reference)
            .is_some_and(|shell| shell.access.is_exclusive())
    }) {
        return;
    }
    let Some(site) = validation::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return;
    };
    let arguments = validation::call_site_argument_expressions(program, &site);
    let receiver_offset = usize::from(
        matches!(&site, validation::CallSite::Expression { call, .. } if call.receiver.is_valid()),
    );
    let Some(state) = validation::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return;
    };
    let exit_contexts: Vec<_> = {
        let call_contexts: Vec<_> = facts
            .flow
            .contexts
            .semantic_context_refs
            .span_or_empty(call_flow.exit_semantic_contexts)
            .iter()
            .map(|c| c.context)
            .collect();
        if !call_contexts.is_empty() {
            call_contexts
        } else {
            facts
                .flow
                .state_statement(state_flow, call_flow.statement_index)
                .map(|s| {
                    facts
                        .flow
                        .contexts
                        .semantic_context_refs
                        .span_or_empty(s.exit_semantic_contexts)
                        .iter()
                        .map(|c| c.context)
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    let mut checked_obligations = restoration_memberships::Obligations::default();
    let memberships = std::cell::OnceCell::new();
    for (position, parameter) in parameters.iter().enumerate() {
        if !validation::reference_shell(program, parameter.type_reference)
            .is_some_and(|shell| shell.access.is_exclusive())
        {
            continue;
        }
        let argument = arguments.get(position).copied();
        let operand_position = position + receiver_offset;
        let actual = super::call_values::Actual {
            parameter,
            argument,
            path: Vec::new(),
            operand_position,
        };
        let Some(captured_values) =
            super::call_values::captured_values(program, facts, state_flow, call_flow, &actual)
        else {
            continue;
        };
        for captured in captured_values {
            let mut places = Vec::new();
            if let Some(obs_places) =
                crate::flow::captured_observation_places(program, facts.flow.view(), &captured)
            {
                places.extend(obs_places);
            }
            if let Some(val_places) =
                crate::flow::captured_value_places(program, facts.flow.view(), &captured)
            {
                places.extend(val_places);
            }
            for place in places {
                let Some(symbol) = place.storage.root.as_symbol() else {
                    continue;
                };
                let Some(local_type) = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .find_map(|s| match s {
                        typed_trees::statement::StatementNode::LocalData(l)
                            if l.symbol == symbol =>
                        {
                            Some(l.type_reference)
                        }
                        _ => None,
                    })
                else {
                    continue;
                };
                for row in validation::field_domains::declared_type_domain_rows(program, local_type)
                {
                    if row.transfer {
                        continue;
                    }
                    let overlaps = row.path.starts_with(&place.storage.segments)
                        || place.storage.segments.starts_with(&row.path);
                    if !overlaps {
                        continue;
                    }
                    let domain_symbol = row.domain_symbol;
                    let semantic_domain = row.semantic_domain;
                    let required_place = validation::CanonicalPlace {
                        root: place.storage.root,
                        segments: row.path.clone(),
                    };
                    if !checked_obligations.first(&required_place, domain_symbol) {
                        continue;
                    }

                    let has_membership = memberships
                        .get_or_init(|| {
                            restoration_memberships::Memberships::prepare(
                                program,
                                &facts.semantic,
                                &exit_contexts,
                            )
                        })
                        .contains(&required_place, domain_symbol, semantic_domain);
                    let satisfied = has_membership
                        || if semantic_domain.is_valid() {
                            super::prover::prove_indexed_domain_at_place(
                                program,
                                &facts.semantic,
                                &exit_contexts,
                                &required_place,
                                domain_symbol,
                                semantic_domain,
                            )
                        } else {
                            super::prover::prove_domain_at_place(
                                program,
                                &facts.semantic,
                                &exit_contexts,
                                &required_place,
                                domain_symbol,
                            )
                        };
                    if !satisfied {
                        let diagnostic = Diagnostic::error(format!(
                            "cannot prove default-domain field requirement for call {} from {}::{}: parameter {} requires {}",
                            call_target_label(program, call_flow.target_symbol),
                            machine_name(program, state_flow.machine_symbol),
                            symbol_name(program, state_flow.state_symbol),
                            facts::canonical_place_label_from_parts(
                                program,
                                facts::PlaceRoot::Symbol(parameter.symbol),
                                &row.path,
                            ),
                            symbol_name(program, domain_symbol),
                        ));
                        diagnostics.push(match call_source_span(program, state_flow, call_flow) {
                            Some(span) => diagnostic.with_source_span(span),
                            None => diagnostic,
                        });
                    }
                }
            }
        }
    }
}
