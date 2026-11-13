/*
    Content-bearing conservation and returned-custody checks.

    A borrow alone never supplies an owned claim. Checked extraction through
    an exclusive loan can detach one while owing restoration of its source.
    Body custody and qualification checking establish those actual transfers;
    declaration-only crossings must instead justify supply from their contract.
    Retained custody
    may nevertheless remain lifetime-bound to one explicit shared input loan.
    This consumer admits only that exact whole-parameter/whole-result shape and
    otherwise requires owned custody. Compatibility keys on the retained
    compiler-owned algebra identity, never carrier or operation names.

    `call_results.rs` checks provisional result qualifications after ownership
    reconstruction, including non-content provenance when no content plan exists.
    `partition_wrappers.rs` composes
    returned partition invocations into wrappers, `content_paths.rs` maps
    content segments, contracts and projections onto fact paths, and
    `retained_custody.rs` decides lifetime-bound borrow custody of retained
    content.
*/

mod body_conservation {
    /*
        Bind a completed body equation to its exact entry and returned carrier claims.

        A numeric theorem and a custody transaction are separate judgments. This
        binding retains the former without granting the latter: an entry claim may
        already have moved, and two inputs with adjacent geometry may have unrelated
        backing. Consumers must never treat these rows as establishment receipts.
        In particular, a returned raw linear carrier already owns its nominal claim;
        derived qualification must attach authority to that occurrence, not mint it
        again. The shared outgoing capture owns result discovery, including nested
        fields and local moves, rather than a constructor-shape recognizer here.
    */
    use super::content_paths::{
        ClaimBindings, content_segments_to_fact_path, entry_claim_identity,
    };
    use super::partition_wrappers::ordinary_returns;
    use crate::checks::multiplicity::observe_body_inputs;
    use crate::checks::multiplicity::publish_body_inputs;
    use crate::checks::multiplicity::{
        CompletedClaimLocation, LinearPlace, ReturnedClaimSource, completed_owned_claims,
    };
    use crate::flow::SelectionConditions;
    use crate::flow::state_exit_parameter_origins;
    use checked_trees::CheckFacts;
    use checked_trees::facts::ContentBodyConservationFact;
    use language_semantics::content::{
        ContentConservationOwnerKind, ContentConservationTerm as ConservationTerm,
        ContentPlaceRoot, ContentPlaceVersion,
    };

    mod terminal_demands {
        /*
        A terminal callee must prove the caller's exact returned-content demand.

        Transport substitutes only unchanged, typed incoming values captured by the
        actual invocation. The resulting equation is still proved from the callee's
        own entry contract at each raw return; no caller fact or custody is imported.
        */

        use std::collections::{BTreeMap, HashSet};

        use arena::Handle;
        use checked_trees::{CheckFactsRef, FlowOperandFact, FlowStateOutgoingFact};
        use language_semantics::content::{
            ContentConservationEquation, ContentConservationPlan, ContentConservationTerm,
            ContentPlaceRoot, conservation_report_fingerprint,
        };
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::TypeReferenceHandle;
        use validation::completed_storage::{StoredObservation, StoredOperand};

        /// Shared across topology discovery and completed-value correspondence. An
        /// expanding cycle declines transport rather than manufacturing a finite law.
        const TRANSPORT_WORK: usize = 65_536;

        pub(super) fn at_state(
            program: &dyn validation::CheckingInput,
            facts: CheckFactsRef<'_>,
            source: &ContentConservationPlan,
            machine: &Machine,
            state: &State,
        ) -> Vec<ContentConservationPlan> {
            if source.owner == machine.symbol
                || super::owned_return_sides(source.equation.left(), source.equation.right())
                    .is_none()
            {
                return Vec::new();
            }
            let Some(owner) = program.machine_by_symbol(source.owner) else {
                return Vec::new();
            };
            let Some(entry) = program.machine_states(owner).first() else {
                return Vec::new();
            };
            let mut pending = Vec::new();
            for initial in program.machine_states(owner) {
                if (initial.symbol == source.callable || entry.symbol == source.callable)
                    && let Some(plan) =
                        super::plan_at_state(program, facts, initial, source.clone())
                {
                    pending.push(plan);
                }
            }
            let mut remaining = TRANSPORT_WORK;
            let mut visited = BTreeMap::<_, Vec<ContentConservationPlan>>::new();
            let mut demands = Vec::new();
            while let Some(plan) = pending.pop() {
                if !spend(&mut remaining, 1) {
                    break;
                }
                let Some((current_machine, current_state)) =
                    validation::find_state(program, plan.callable)
                else {
                    continue;
                };
                let known = visited
                    .entry((identity(current_state.symbol), plan.report_fingerprint))
                    .or_default();
                if !spend(&mut remaining, 1 + known.len()) {
                    break;
                }
                // A report hash narrows this query-local comparison, never decides
                // semantic identity or discards a distinct substitution on collision.
                if known.contains(&plan) {
                    continue;
                }
                known.push(plan.clone());
                if plan.owner == machine.symbol && plan.callable == state.symbol {
                    if plan != *source {
                        demands.push(plan);
                    }
                    continue;
                }
                let Some(flow) = facts
                    .flow
                    .control
                    .unique_state(current_machine.symbol, current_state.symbol)
                else {
                    continue;
                };
                for outgoing in facts.flow.control.outgoing.span_or_empty(flow.outgoing) {
                    if !spend(&mut remaining, 1) {
                        return demands;
                    }
                    if outgoing.cannot_return_normally {
                        continue;
                    }
                    if outgoing.transition_target.is_valid() {
                        let Some(statement) = program
                            .statement_table
                            .statement_at(current_state, outgoing.statement_index)
                            .and_then(|statement| statement.as_transition())
                        else {
                            continue;
                        };
                        if statement.exit != typed_trees::statement::TransitionExit::Ordinary {
                            continue;
                        }
                        for invocation in validation::completing_transition_invocations(
                            program,
                            outgoing.transition_target,
                        ) {
                            let transferred = transfer(
                                program,
                                facts,
                                current_state,
                                outgoing,
                                invocation.target,
                                invocation.expression,
                                invocation.arguments,
                                &plan,
                                &mut remaining,
                            );
                            if let Ok(next) = transferred {
                                pending.push(next);
                            }
                        }
                    } else if let Some(alternatives) =
                        validation::completed_storage::terminal_alternatives(
                            facts.flow.control,
                            outgoing.returned_value,
                        )
                    {
                        for returned in alternatives {
                            let capture = facts.flow.control.operands.get(returned);
                            if let Some(invocation) =
                                validation::completing_call_transfer(program, capture.expression)
                            {
                                let transferred = transfer(
                                    program,
                                    facts,
                                    current_state,
                                    outgoing,
                                    invocation.target,
                                    invocation.expression,
                                    invocation.arguments,
                                    &plan,
                                    &mut remaining,
                                );
                                if let Ok(next) = transferred {
                                    pending.push(next);
                                }
                            }
                        }
                    }
                }
            }
            demands
        }

        #[allow(clippy::too_many_arguments)]
        fn transfer(
            program: &dyn validation::CheckingInput,
            facts: CheckFactsRef<'_>,
            state: &State,
            outgoing: &FlowStateOutgoingFact,
            target: SymbolHandle,
            expression: ExpressionHandle,
            arguments: &[ExpressionHandle],
            plan: &ContentConservationPlan,
            remaining: &mut usize,
        ) -> Result<ContentConservationPlan, &'static str> {
            if !spend(remaining, 1) {
                return Err("terminal demand transport work exhausted");
            }
            let next_state = validation::call_target_state(program, target)
                .ok_or("terminal demand has no concrete target state")?;
            let next_machine = program
                .machine_holding_state(next_state.symbol)
                .ok_or("terminal demand target has no declaration owner")?;
            if next_machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !same_type(program, state.return_type, next_state.return_type)
            {
                return Err(
                    "terminal demand target is not a checked body with the same result carrier",
                );
            }
            let flow = facts
                .flow
                .control
                .unique_state(plan.owner, state.symbol)
                .ok_or("terminal demand source has no unique state flow")?;
            if !spend(remaining, flow.calls.len()) {
                return Err("terminal demand call-roster work exhausted");
            }
            let mut calls = facts
                .flow
                .control
                .calls
                .span(flow.calls).ok_or("terminal demand call roster is unavailable")?
                .iter()
                .filter(|call| {
                    call.statement_index == outgoing.statement_index
                        && call.authored_target == target
                        && call.authored_expression == expression
                        && !call.authored_receiver.is_valid()
                        && call.authored_source_custody_valid
                        && facts
                            .flow
                            .control
                            .call_arguments
                            .span(call.authored_arguments)
                            == Some(arguments)
                        && facts
                            .flow
                            .control
                            .operands
                            .span(call.argument_operands())
                            .is_some_and(|operands| {
                                operands.iter().all(|operand| {
                                    !outgoing.transition_target.is_valid()
                                        || operand.evaluation
                                            == checked_trees::FlowPermissionEvaluation::TransitionTarget(
                                                outgoing.transition_target,
                                            )
                                })
                            })
                });
            let call = calls.next().ok_or(
                "terminal demand has no exact source call, argument roster and evaluation lane",
            )?;
            if calls.next().is_some() {
                return Err("terminal demand source call is ambiguous");
            }
            let parameters = program.state_parameters(next_state);
            if parameters.len() != call.argument_operands().len() {
                return Err("terminal demand argument count does not match its target telescope");
            }
            let mut bindings = Vec::new();
            for (parameter, operand) in parameters.iter().zip(call.argument_operands().handles()) {
                if !spend(remaining, 1) {
                    return Err("terminal demand argument transport work exhausted");
                }
                if parameter.is_mutable || parameter.is_const {
                    continue;
                }
                if let Some(source) = incoming_parameter(
                    program,
                    facts,
                    state,
                    operand,
                    parameter.type_reference,
                    remaining,
                ) {
                    bindings.push((source, parameter));
                }
            }
            let mut plan = plan.clone();
            let mut left = plan.equation.left().clone();
            let mut right = plan.equation.right().clone();
            rebase(&mut left, parameters, &bindings)
                .ok_or("terminal demand left side has no exact unchanged incoming binding")?;
            rebase(&mut right, parameters, &bindings)
                .ok_or("terminal demand right side has no exact unchanged incoming binding")?;
            plan.owner = next_machine.symbol;
            plan.callable = next_state.symbol;
            plan.equation = ContentConservationEquation::new(left, right);
            plan.report_fingerprint =
                conservation_report_fingerprint(&plan.algebra, &plan.equation);
            Ok(plan)
        }

        /// Every predecessor must be the same unchanged complete entry value. A local
        /// read may follow its retained store, but never a declaration initializer.
        fn incoming_parameter(
            program: &dyn validation::CheckingInput,
            facts: CheckFactsRef<'_>,
            state: &State,
            operand: Handle<FlowOperandFact>,
            reference: TypeReferenceHandle,
            remaining: &mut usize,
        ) -> Option<SymbolHandle> {
            // Inline storage preserves descriptors, not the contents they borrow.
            // Without a complete empty loan frontier, the parent's entry content
            // cannot become the child's entry snapshot through this value relation.
            if !validation::borrow_contracts::borrow_carrying_owner_paths(program, reference)?
                .is_empty()
            {
                return None;
            }
            let mut pending = vec![(operand, Vec::new(), false)];
            let mut active = HashSet::new();
            let mut completed = HashSet::new();
            let mut incoming = None;
            while let Some((operand, projection, leaving)) = pending.pop() {
                if !spend(remaining, 1 + projection.len())
                    || !facts.flow.control.operands.is_valid(operand)
                {
                    return None;
                }
                let key = (
                    (operand.arena_index(), operand.generation()),
                    projection.clone(),
                );
                if leaving {
                    active.remove(&key);
                    completed.insert(key);
                    continue;
                }
                if completed.contains(&key) {
                    continue;
                }
                if !active.insert(key.clone()) {
                    return None;
                }
                let capture = facts.flow.control.operands.get(operand);
                let actual_type = validation::project_type_reference_from_segments(
                    program,
                    capture.type_reference,
                    &projection,
                )?;
                if capture.state_symbol != state.symbol
                    || !same_type(program, actual_type, reference)
                    || validation::reference_shell(program, actual_type).is_some()
                {
                    return None;
                }
                let source = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    capture.statement_index,
                    capture.expression,
                )
                .map(|mut source| {
                    source.segments.extend_from_slice(&projection);
                    source
                });
                if let Some(source) = source
                    && let Some(StoredOperand::Incoming(source)) =
                        validation::completed_storage::sources_at_snapshot(
                            program,
                            facts.flow.control,
                            capture.storage_snapshot,
                            &source,
                            StoredObservation::InlineContents,
                        )
                    && source.segments.is_empty()
                    && let facts::PlaceRoot::Symbol(symbol) = source.root
                    && let Some(parameter) = program.state_parameter_by_symbol(state, symbol)
                    && !parameter.is_mutable
                    && !parameter.is_const
                    && same_type(program, parameter.type_reference, reference)
                {
                    if incoming.is_some_and(|previous| previous != symbol) {
                        return None;
                    }
                    incoming = Some(symbol);
                    active.remove(&key);
                    completed.insert(key);
                    continue;
                }
                let predecessors = crate::flow::completed_values::predecessors(
                    program,
                    facts,
                    operand,
                    &projection,
                )?;
                if predecessors.is_empty() {
                    return None;
                }
                pending.push((operand, projection, true));
                pending.extend(
                    predecessors
                        .into_iter()
                        .map(|(operand, projection)| (operand, projection, false)),
                );
            }
            incoming
        }

        fn rebase(
            term: &mut ContentConservationTerm,
            parameters: &[typed_trees::signature::StateParameter],
            bindings: &[(SymbolHandle, &typed_trees::signature::StateParameter)],
        ) -> Option<()> {
            match term {
                ContentConservationTerm::Separate(children) => {
                    for child in children {
                        rebase(child, parameters, bindings)?;
                    }
                }
                ContentConservationTerm::Projection { subject, .. } => {
                    if let ContentPlaceRoot::Parameter { symbol, .. } = subject.root {
                        let mut targets = bindings
                            .iter()
                            .filter_map(|(source, target)| (*source == symbol).then_some(*target));
                        let target = targets.next()?;
                        if targets.any(|other| other.symbol != target.symbol) {
                            return None;
                        }
                        let position = parameters
                            .iter()
                            .position(|parameter| parameter.symbol == target.symbol)?;
                        subject.root = ContentPlaceRoot::Parameter {
                            position: u32::try_from(position).ok()?,
                            symbol: target.symbol,
                            name: target.name.as_str().to_owned(),
                        };
                    }
                }
            }
            Some(())
        }

        fn identity(symbol: SymbolHandle) -> (u32, u32) {
            (symbol.arena_index(), symbol.generation())
        }

        fn same_type(
            program: &dyn validation::CheckingInput,
            left: TypeReferenceHandle,
            right: TypeReferenceHandle,
        ) -> bool {
            left.is_valid()
                && right.is_valid()
                && program.normalized_type_identity(left) == program.normalized_type_identity(right)
        }

        fn spend(remaining: &mut usize, work: usize) -> bool {
            let Some(next) = remaining.checked_sub(work) else {
                return false;
            };
            *remaining = next;
            true
        }
    }

    pub(crate) fn observe_body_completion(
        program: &dyn validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        evaluation: checked_trees::FlowPermissionEvaluation,
        places: &[LinearPlace],
        events: &[checked_trees::FlowPermissionEventFact],
        evaluation_events: &[checked_trees::FlowPermissionEventFact],
        location: CompletedClaimLocation,
        candidates: &[ConservationCandidate],
    ) -> Vec<arena::Handle<ContentBodyConservationFact>> {
        if facts.qualifications.content.conservation_plans.is_empty() {
            return Vec::new();
        }
        let mut observed = Vec::new();
        // The immutable site preparation already transported terminal demands.
        // Reuse those plans instead of rediscovering the call graph at every
        // completed operand during the sequential custody walk.
        let mut plans = Vec::new();
        for candidate in candidates {
            if !plans.iter().any(|(plan_index, plan)| {
                *plan_index == candidate.plan_index && *plan == &candidate.plan
            }) {
                plans.push((candidate.plan_index, &candidate.plan));
            }
        }
        for (plan_index, plan) in plans {
            if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
                continue;
            }
            let Some((inputs, outputs)) =
                owned_return_sides(plan.equation.left(), plan.equation.right())
            else {
                continue;
            };
            for returned_at in ordinary_returns(program, facts.flow.control.outgoing.iter(), state)
            {
                let outgoing = facts.flow.control.outgoing.get(returned_at);
                let outgoing_evaluation = if outgoing.statement_index != statement_index {
                    checked_trees::FlowPermissionEvaluation::Statement
                } else if outgoing.transition_target.is_valid() {
                    checked_trees::FlowPermissionEvaluation::TransitionTarget(
                        outgoing.transition_target,
                    )
                } else {
                    checked_trees::FlowPermissionEvaluation::Statement
                };
                if !outgoing.returned_value.is_valid() || outgoing_evaluation != evaluation {
                    continue;
                }
                for completed_value in
                    completed_producers(program, facts.view(), state, outgoing.returned_value)
                {
                    let captured = facts.flow.control.operands.get(completed_value);
                    if captured.statement_index != statement_index
                        || captured.expression != expression
                    {
                        continue;
                    }
                    let prepared = candidates.iter().find(|candidate| {
                        candidate.plan_index == plan_index
                            && candidate.plan == *plan
                            && candidate.returned_at == returned_at
                            && candidate.completed_value == completed_value
                    });
                    let admitted = prepared.map_or_else(
                        || {
                            has_conservation_candidate(
                                program,
                                facts.view(),
                                machine,
                                state,
                                completed_value,
                                plan,
                            )
                        },
                        |candidate| candidate.admitted,
                    );
                    if !admitted {
                        continue;
                    }
                    let mut conditions = SelectionConditions::new();
                    let returned = completed_owned_claims(
                        program,
                        facts.view(),
                        completed_value,
                        location,
                        places,
                        evaluation_events,
                        &mut conditions,
                    );
                    let Ok(returned) = returned else {
                        continue;
                    };
                    let mut input_bindings = ClaimBindings::default();
                    let mut output_bindings = ClaimBindings::default();
                    if input_bindings
                        .extend_term(inputs, |subject| {
                            let ContentPlaceRoot::Parameter { symbol, .. } = subject.root else {
                                return None;
                            };
                            let path = content_segments_to_fact_path(&subject.segments)?;
                            entry_claim_identity(
                                facts.view(),
                                state.symbol,
                                symbol,
                                &path,
                                events.iter(),
                            )
                        })
                        .is_none()
                        || output_bindings
                            .extend_term(outputs, |subject| {
                                let path = content_segments_to_fact_path(&subject.segments)?;
                                let mut matching =
                                    returned.iter().filter(|claim| claim.path == path);
                                let first = matching.next()?;
                                let ReturnedClaimSource::Live(identity) = first.source else {
                                    return None;
                                };
                                let mut coverage = first.condition;
                                for claim in matching {
                                    if claim.source != first.source {
                                        return None;
                                    }
                                    coverage = conditions.any(coverage, claim.condition);
                                }
                                conditions
                                    .covers(facts.flow.control, coverage)
                                    .then_some(identity)
                            })
                            .is_none()
                    {
                        continue;
                    }
                    if !output_bindings.rows().all(|(binding, path)| {
                        prepared
                            .and_then(|candidate| candidate.proven_domain(binding.domain, path))
                            .unwrap_or_else(|| {
                                proves_domain(
                                    program,
                                    machine,
                                    state,
                                    (statement_index, expression),
                                    binding.domain,
                                    path,
                                )
                            })
                    }) {
                        continue;
                    }
                    let inputs = input_bindings.publish(facts);
                    let outputs = output_bindings.publish(facts);
                    let conservation = ContentBodyConservationFact {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        returned_at,
                        completed_value,
                        evaluation,
                        plan_index,
                        body_plan_index: facts.qualifications.content.body_conservation_plans.len(),
                        inputs,
                        outputs,
                        available_inputs: Default::default(),
                    };
                    let available =
                        observe_body_inputs(program, facts.view(), &conservation, places);
                    let Ok(available) = available else {
                        continue;
                    };
                    facts
                        .qualifications
                        .content
                        .body_conservation_plans
                        .push(plan.clone());
                    let conservation = facts
                        .qualifications
                        .content
                        .body_conservations
                        .insert(conservation);
                    publish_body_inputs(facts, conservation, available);
                    observed.push(conservation);
                }
            }
        }
        observed
    }

    /*
        The proof answers one conservation site needs: whether its completed value
        is a conservation candidate and, if so, which result-field domains it
        proves. Both are proof searches over the program, flow control rows and
        content plans, none of which the permission walk writes, so the walk
        prepares every site of a state on the checking pool before its sequential
        custody replay reaches the completions that ask.
    */
    #[derive(Debug, Clone)]
    pub(crate) struct ConservationCandidate {
        plan_index: usize,
        plan: language_semantics::content::ContentConservationPlan,
        returned_at: arena::Handle<checked_trees::FlowStateOutgoingFact>,
        completed_value: arena::Handle<checked_trees::FlowOperandFact>,
        admitted: bool,
        /*
            Each output projection's domain and claim path with its answer; empty
            for a site that is not admitted, which never asks.
        */
        proven_domains: Vec<(symbols::SymbolHandle, Vec<facts::PlaceSegment>, bool)>,
    }

    impl ConservationCandidate {
        fn proven_domain(
            &self,
            domain: symbols::SymbolHandle,
            path: &[facts::PlaceSegment],
        ) -> Option<bool> {
            self.proven_domains
                .iter()
                .find(|(proven, proven_path, _)| *proven == domain && proven_path == path)
                .map(|(_, _, answer)| *answer)
        }
    }

    /*
        The conservation sites one state's completions can ask about: every
        returned producer of every machine conservation plan the state owns, with
        the plan's output projections. Enumerating them is cheap; answering them
        is a proof search per site.
    */
    #[derive(Default)]
    pub(crate) struct ConservationSites {
        plans: Vec<(
            language_semantics::content::ContentConservationPlan,
            Vec<(symbols::SymbolHandle, Vec<facts::PlaceSegment>)>,
        )>,
        /* `(plan slot, plan index, returned_at, completed_value)`. */
        sites: Vec<(
            usize,
            usize,
            arena::Handle<checked_trees::FlowStateOutgoingFact>,
            arena::Handle<checked_trees::FlowOperandFact>,
        )>,
        /* Site positions grouped by completed value, in first-appearance order. */
        groups: Vec<Vec<usize>>,
    }

    impl ConservationSites {
        /* How many completed values the sites ask about. */
        pub(crate) fn groups(&self) -> usize {
            self.groups.len()
        }

        /*
            Answer every site of one completed value: each candidate query and,
            when admitted, its domains, as `(site position, answer)`. Every plan
            and domain asks about the same completed value, so one shared proof
            walk answers them together; a question it does not prove is asked
            alone, exactly as before.
        */
        pub(crate) fn answer_group(
            &self,
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            group: usize,
        ) -> Vec<(usize, ConservationCandidate)> {
            let sites = &self.groups[group];
            let completed_value = self.sites[sites[0]].3;
            let captured = facts.flow.control.operands.get(completed_value);
            let plans = sites
                .iter()
                .map(|&site| &self.plans[self.sites[site].0].0)
                .collect::<Vec<_>>();
            let mut domains: Vec<(symbols::SymbolHandle, &[facts::PlaceSegment])> = Vec::new();
            for &site in sites {
                for (domain, path) in &self.plans[self.sites[site].0].1 {
                    if !domains.iter().any(|(known, known_path)| {
                        known == domain && *known_path == path.as_slice()
                    }) {
                        domains.push((*domain, path));
                    }
                }
            }
            let (shared_candidates, shared_domains) =
                validation::completed_value_conservation_answers(
                    program,
                    machine,
                    state,
                    &validation::CompletedConservationQueries {
                        statement_index: captured.statement_index,
                        expression: captured.expression,
                        reference: captured.type_reference,
                        plans: &plans,
                        projections: &facts.qualifications.content.plans,
                        domains: &domains,
                    },
                );
            let site = (captured.statement_index, captured.expression);
            sites
                .iter()
                .zip(shared_candidates)
                .map(|(&position, shared)| {
                    let (plan, plan_index, returned_at, completed_value) = self.sites[position];
                    let (plan, output_projections) = &self.plans[plan];
                    let admitted = shared
                        || has_conservation_candidate(
                            program,
                            facts.view(),
                            machine,
                            state,
                            completed_value,
                            plan,
                        );
                    let proven_domains = if admitted {
                        output_projections
                            .iter()
                            .map(|(domain, path)| {
                                let shared = domains
                                    .iter()
                                    .position(|(known, known_path)| {
                                        known == domain && *known_path == path.as_slice()
                                    })
                                    .is_some_and(|known| shared_domains[known]);
                                let answer = shared
                                    || proves_domain(program, machine, state, site, *domain, path);
                                (*domain, path.clone(), answer)
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    (
                        position,
                        ConservationCandidate {
                            plan_index,
                            plan: plan.clone(),
                            returned_at,
                            completed_value,
                            admitted,
                            proven_domains,
                        },
                    )
                })
                .collect()
        }
    }

    /* Enumerate the sites `observe_body_completion` can reach in this state. */
    pub(crate) fn conservation_sites(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> ConservationSites {
        let mut sites = ConservationSites::default();
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            return sites;
        }
        for (plan_index, plan) in state_conservation_plans(program, facts.view(), machine, state) {
            let Some(plan) = plan_at_state(program, facts.view(), state, plan) else {
                continue;
            };
            let Some((_, outputs)) =
                owned_return_sides(plan.equation.left(), plan.equation.right())
            else {
                continue;
            };
            let mut output_projections = Vec::new();
            let mut pending = vec![outputs];
            while let Some(term) = pending.pop() {
                match term {
                    ConservationTerm::Projection {
                        domain, subject, ..
                    } => {
                        if let Some(path) = content_segments_to_fact_path(&subject.segments) {
                            output_projections.push((*domain, path));
                        }
                    }
                    ConservationTerm::Separate(children) => pending.extend(children.iter()),
                }
            }
            let slot = sites.plans.len();
            for returned_at in ordinary_returns(program, facts.flow.control.outgoing.iter(), state)
            {
                let outgoing = facts.flow.control.outgoing.get(returned_at);
                if !outgoing.returned_value.is_valid() {
                    continue;
                }
                for completed_value in
                    completed_producers(program, facts.view(), state, outgoing.returned_value)
                {
                    sites
                        .sites
                        .push((slot, plan_index, returned_at, completed_value));
                }
            }
            sites.plans.push((plan, output_projections));
        }
        for position in 0..sites.sites.len() {
            let completed_value = sites.sites[position].3;
            match sites
                .groups
                .iter()
                .position(|group| sites.sites[group[0]].3 == completed_value)
            {
                Some(group) => sites.groups[group].push(position),
                None => sites.groups.push(vec![position]),
            }
        }
        sites
    }

    /// Declared plans and exact terminal callers' demands are proof obligations,
    /// never assumptions about the value a body returns.
    fn state_conservation_plans(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Vec<(usize, language_semantics::content::ContentConservationPlan)> {
        let entry = program
            .machine_states(machine)
            .first()
            .map(|state| state.symbol);
        let mut plans = facts
            .qualifications
            .content
            .conservation_plans
            .iter()
            .enumerate()
            .filter(|(_, plan)| {
                plan.owner_kind == ContentConservationOwnerKind::Machine
                    && plan.owner == machine.symbol
                    && (plan.callable == state.symbol || Some(plan.callable) == entry)
            })
            .map(|(position, plan)| (position, plan.clone()))
            .collect::<Vec<_>>();
        for (plan_index, plan) in facts
            .qualifications
            .content
            .conservation_plans
            .iter()
            .enumerate()
            .filter(|(_, plan)| plan.owner_kind == ContentConservationOwnerKind::Machine)
        {
            for demanded in terminal_demands::at_state(program, facts, plan, machine, state) {
                if !plans.iter().any(|(existing_index, existing)| {
                    *existing_index == plan_index && *existing == demanded
                }) {
                    plans.push((plan_index, demanded));
                }
            }
        }
        plans
    }

    /*
        Whether the value completed at `site` proves `domain` at `path`, in the
        same completed frame as its conservation equation.
    */
    fn proves_domain(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        (statement_index, expression): (usize, typed_trees::expression::ExpressionHandle),
        domain: symbols::SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> bool {
        validation::completed_value_proves_domain_predicate(
            program,
            machine,
            state,
            statement_index,
            expression,
            domain,
            typed_trees::expression::ExpressionHandle::invalid(),
            path,
        )
    }

    fn has_conservation_candidate(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        completed_value: arena::Handle<checked_trees::FlowOperandFact>,
        plan: &language_semantics::content::ContentConservationPlan,
    ) -> bool {
        let captured = facts.flow.control.operands.get(completed_value);
        validation::completed_value_has_content_conservation_candidate(
            program,
            machine,
            state,
            captured.statement_index,
            captured.expression,
            captured.type_reference,
            plan,
            facts.qualifications.content.plans,
        )
    }

    /*
        A machine guarantee may be consumed in another state only after the shared
        arrival analysis proves the exact parameter origin on every predecessor.
        Rebase its subjects, never import the guarantee as an entry assumption.
    */
    fn plan_at_state(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state: &typed_trees::state::State,
        mut plan: language_semantics::content::ContentConservationPlan,
    ) -> Option<language_semantics::content::ContentConservationPlan> {
        if plan.callable == state.symbol {
            return Some(plan);
        }
        let flow = facts.flow.control.state(plan.owner, state.symbol)?;
        let origins = state_exit_parameter_origins(facts, flow);
        fn rebase(
            term: &mut ConservationTerm,
            origins: &[&checked_trees::FlowExitParameterOrigin],
            parameters: &[typed_trees::signature::StateParameter],
        ) -> Option<()> {
            match term {
                ConservationTerm::Separate(children) => {
                    for child in children {
                        rebase(child, origins, parameters)?;
                    }
                }
                ConservationTerm::Projection { subject, .. } => {
                    if let ContentPlaceRoot::Parameter {
                        position,
                        symbol,
                        name,
                    } = &mut subject.root
                    {
                        let mut matching = origins
                            .iter()
                            .filter(|origin| origin.entry_parameter == *symbol);
                        let target = matching.next()?.state_parameter;
                        if !target.is_valid()
                            || matching.any(|origin| origin.state_parameter != target)
                        {
                            return None;
                        }
                        let (ordinal, parameter) = parameters
                            .iter()
                            .enumerate()
                            .find(|(_, parameter)| parameter.symbol == target)?;
                        *position = u32::try_from(ordinal).ok()?;
                        *symbol = target;
                        *name = parameter.name.as_str().to_owned();
                    }
                }
            }
            Some(())
        }
        let mut left = plan.equation.left().clone();
        let mut right = plan.equation.right().clone();
        let parameters = program.state_parameters(state);
        rebase(&mut left, &origins, parameters)?;
        rebase(&mut right, &origins, parameters)?;
        plan.callable = state.symbol;
        plan.equation = language_semantics::content::ContentConservationEquation::new(left, right);
        plan.report_fingerprint = language_semantics::content::conservation_report_fingerprint(
            &plan.algebra,
            &plan.equation,
        );
        Some(plan)
    }

    /*
        Follow exact produced values through unchanged resident storage. This
        discovers where a return demand may be proved; each candidate still needs
        its own completion-time theorem and live custody. Never use retention
        profiles (which describe loans), copyability, or a declaration initializer
        as a substitute for the completed store. Alternatives stay distinct.
    */
    fn completed_producers(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state: &typed_trees::state::State,
        returned: arena::Handle<checked_trees::FlowOperandFact>,
    ) -> Vec<arena::Handle<checked_trees::FlowOperandFact>> {
        let mut pending = vec![(returned, Vec::new())];
        let mut visited = Vec::new();
        let mut completed = Vec::new();
        while let Some((handle, projection)) = pending.pop() {
            if !facts.flow.control.operands.is_valid(handle)
                || visited.contains(&(handle, projection.clone()))
            {
                continue;
            }
            visited.push((handle, projection.clone()));
            let capture = facts.flow.control.operands.get(handle);
            if capture.state_symbol != state.symbol {
                continue;
            }
            if projection.is_empty() && !completed.contains(&handle) {
                completed.push(handle);
            }
            if let Some(predecessors) =
                crate::flow::completed_values::predecessors(program, facts, handle, &projection)
            {
                pending.extend(predecessors);
            }
        }
        completed
    }

    fn side_has_role(term: &ConservationTerm, entry: bool) -> bool {
        match term {
            ConservationTerm::Projection { subject, .. } => {
                if entry {
                    subject.version == ContentPlaceVersion::Entry
                        && matches!(subject.root, ContentPlaceRoot::Parameter { .. })
                } else {
                    subject.version == ContentPlaceVersion::Current
                        && subject.root == ContentPlaceRoot::Result
                }
            }
            ConservationTerm::Separate(children) => {
                !children.is_empty() && children.iter().all(|child| side_has_role(child, entry))
            }
        }
    }

    /*
        Equation canonicalization may reverse the authored sides. Classify by
        exact root/revision, not spelling or serialized side order. Empty separate
        terms provide no owned input or returned occurrence.
    */
    pub(super) fn owned_return_sides<'term>(
        left: &'term ConservationTerm,
        right: &'term ConservationTerm,
    ) -> Option<(&'term ConservationTerm, &'term ConservationTerm)> {
        if side_has_role(left, true) && side_has_role(right, false) {
            Some((left, right))
        } else if side_has_role(right, true) && side_has_role(left, false) {
            Some((right, left))
        } else {
            None
        }
    }
}
mod body_proofs {
    /*
        Prepare numeric law dependencies from the exact callable's completed exits.

        The machine-header validator already retains its whole-body candidates.
        This reader covers state-local declarations through the same completed-value
        observation owner. Internal transfers cannot masquerade as completed result
        observations; their proof still belongs to partition composition. No entry
        projection is rebound to a later state's arrival version.
    */
    use checked_trees::CheckFacts;
    use language_semantics::content::ContentConservationOwnerKind;

    pub(super) fn completed_source(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        outgoing_rows: &super::partition_wrappers::OutgoingRows<'_>,
        source: &validation::ContentConservationSourcePlan,
    ) -> Option<Vec<validation::ContentConservationDependencies>> {
        if source.plan.owner_kind != ContentConservationOwnerKind::Machine {
            return None;
        }
        let machine = program.machine_by_symbol(source.plan.owner)?;
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !machine.body_is_present
        {
            return None;
        }
        let state = program.state_in_machine(machine, source.plan.callable)?;
        let outgoing = super::partition_wrappers::ordinary_returns(
            program,
            outgoing_rows.for_state(state.symbol).into_iter(),
            state,
        );
        if outgoing.is_empty() {
            return None;
        }
        let mut dependencies = Vec::new();
        for handle in outgoing {
            let returned = facts.flow.control.outgoing.get(handle);
            if returned.machine_symbol != machine.symbol
                || returned.state_symbol != state.symbol
                || !facts
                    .flow
                    .control
                    .operands
                    .is_valid(returned.returned_value)
            {
                return None;
            }
            let captured = facts.flow.control.operands.get(returned.returned_value);
            if captured.state_symbol != state.symbol
                || captured.statement_index != returned.statement_index
                || !captured.expression.is_valid()
                || !captured.type_reference.is_valid()
            {
                return None;
            }
            dependencies.extend(
                validation::completed_value_content_conservation_dependencies(
                    program,
                    machine,
                    state,
                    captured.statement_index,
                    captured.expression,
                    captured.type_reference,
                    &source.plan,
                    &facts.qualifications.content.plans,
                )?,
            );
        }
        Some(dependencies)
    }
}
mod call_ancestry {
    /*
        Discharge conditional call ancestry after all checked bodies have completed.

        A numeric callee guarantee cannot certify backing identity: a callee could
        return a freshly supplied root with the same geometry. Used replacement
        edges must rejoin the exact normal-result receipts and the callee's actual
        transformations, ending at the formal whose caller-owned claim was moved.
        Unused candidate equations grant nothing and need no ancestry judgment.
    */
    use checked_trees::facts::{
        ContentBodyClaimBinding as ClaimBinding, ContentClaimTransformation,
        ContentClaimTransformationSource as TransformationSource,
    };
    use checked_trees::{CheckFacts, FlowClaimJoinAlternativeSource, FlowClaimOutcomeSource};
    use language_semantics::{PermissionClaimIdentity, PermissionEventKind, PermissionEventSource};

    pub(super) fn check_used_calls(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) {
        let content = &facts.qualifications.content;
        let mut checked = Vec::new();
        let mut traversal = OriginTraversal::default();
        for (_, derived) in content.body_derivations.iter() {
            let equation = content.body_conservations.get(derived.conservation);
            let completed = facts.flow.control.operands.get(equation.completed_value);
            let frontier = content.descendant_frontier(
                equation.inputs,
                equation.machine_symbol,
                equation.state_symbol,
                completed.statement_index,
                equation.evaluation,
                derived.conservation,
            );
            let result = frontier.and_then(|frontier| {
                for (handle, _, _) in frontier.replacements {
                    if checked.contains(&handle) {
                        continue;
                    }
                    let transformation = content.claim_transformations.get(handle);
                    if matches!(transformation.source, TransformationSource::Call { .. }) {
                        authenticate_call(program, facts, transformation, &mut traversal)?;
                        checked.push(handle);
                    }
                }
                Ok(())
            });
            if let Err(reason) = result {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "cannot derive content custody in `{}`, state `{}` at statement {}: {reason}",
                    crate::published::labels::symbol_name(program, equation.machine_symbol),
                    crate::published::labels::symbol_name(program, equation.state_symbol),
                    completed.statement_index,
                )));
            }
        }
    }

    type ActiveClaim = (
        PermissionClaimIdentity,
        symbols::SymbolHandle,
        language_semantics::SemanticDomainId,
        symbols::SymbolHandle,
    );

    /*
        Completed origins belong to this fixed checked graph. Share them across
        reconvergent uses, while bounding a deeply nested or expanding source graph
        before it can exhaust the host stack or receiving work.
    */
    #[derive(Default)]
    struct OriginTraversal {
        active: Vec<ActiveClaim>,
        completed: Vec<(ActiveClaim, Vec<PermissionClaimIdentity>)>,
        visits: usize,
    }

    impl OriginTraversal {
        fn visit(&mut self) -> Result<(), &'static str> {
            if self.visits >= 65_536 {
                return Err("callee content lineage work budget exhausted");
            }
            self.visits += 1;
            Ok(())
        }
    }

    fn authenticate_call(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        transformation: &ContentClaimTransformation,
        traversal: &mut OriginTraversal,
    ) -> Result<(), &'static str> {
        for output in transformation.outputs.handles() {
            call_parents(
                program,
                facts,
                transformation,
                facts.qualifications.content.body_claim_bindings.get(output),
                traversal,
            )?;
        }
        Ok(())
    }

    fn call_parents(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        transformation: &ContentClaimTransformation,
        output: &ClaimBinding,
        traversal: &mut OriginTraversal,
    ) -> Result<Vec<PermissionClaimIdentity>, &'static str> {
        let TransformationSource::Call { call_ordinal, .. } = transformation.source else {
            return Err("ancestry edge is not a completed call");
        };
        let ownership = &facts.flow.ownership;
        let mut receipts = ownership.claim_join_receipts.iter().filter(|(_, receipt)| {
            receipt.machine_symbol == transformation.machine_symbol
                && receipt.state_symbol == transformation.state_symbol
                && receipt.statement_index == transformation.statement_index
                && receipt.call_ordinal == call_ordinal
                && receipt.claim_identity == output.claim_identity
        });
        let (_, receipt) = receipts
            .next()
            .ok_or("conserved call result has no exact normal-result receipt")?;
        if receipts.next().is_some() {
            return Err("conserved call result has ambiguous normal-result receipts");
        }
        let mut parents: Option<Vec<PermissionClaimIdentity>> = None;
        for alternative in ownership
            .claim_join_alternatives
            .span_or_empty(receipt.alternatives)
        {
            traversal.visit()?;
            let source = match alternative.source {
                FlowClaimJoinAlternativeSource::Claim { source, .. } => source,
                FlowClaimJoinAlternativeSource::Inactive
                | FlowClaimJoinAlternativeSource::ExcludedInput { .. } => continue,
            };
            let formals = match source {
                FlowClaimOutcomeSource::Input {
                    parameter_symbol,
                    segments,
                } => vec![(parameter_symbol, segments)],
                FlowClaimOutcomeSource::Established { claim_identity, .. } => {
                    let roots = qualified_roots(program, facts, claim_identity, output, traversal)?;
                    entry_formals(facts, &roots)?
                }
                FlowClaimOutcomeSource::Unknown => return Err("callee result lineage is unknown"),
            };
            let mut candidate = Vec::new();
            for (parameter, path) in formals {
                let mut inputs = facts
                    .qualifications
                    .content
                    .body_claim_bindings
                    .span_or_empty(transformation.inputs)
                    .iter()
                    .filter(|input| {
                        input.parameter_symbol == parameter
                            && ownership.segments.span_or_empty(input.segments)
                                == ownership.segments.span_or_empty(path)
                            && same_projection(input, output)
                    });
                let input = inputs
                    .next()
                    .ok_or("callee result lineage does not bind a conserved actual input")?;
                if inputs.next().is_some() {
                    return Err("callee result lineage has ambiguous conserved actual inputs");
                }
                crate::shared::collections::push_unique(&mut candidate, input.claim_identity);
            }
            if candidate.is_empty()
                || parents
                    .as_ref()
                    .is_some_and(|previous| !same_roots(previous, &candidate))
            {
                return Err("callee result alternatives have incompatible input rosters");
            }
            parents = Some(candidate);
        }
        parents.ok_or("conserved call result has no live normal-result alternative")
    }

    fn callable_entry_parameter(
        facts: &CheckFacts,
        entry: &checked_trees::FlowPermissionEventFact,
        parameter: symbols::SymbolHandle,
    ) -> Result<symbols::SymbolHandle, &'static str> {
        let flow = facts
            .flow
            .control
            .state(entry.machine_symbol, entry.state_symbol)
            .ok_or("callee lineage entry has no state flow")?;
        let origins = crate::flow::state_exit_parameter_origins(facts.view(), flow);
        let mut matching = origins
            .iter()
            .filter(|origin| origin.state_parameter == parameter);
        let Some(first) = matching.next() else {
            return Ok(parameter);
        };
        if matching.any(|origin| origin.entry_parameter != first.entry_parameter)
            || origins.iter().any(|origin| {
                origin.entry_parameter == first.entry_parameter
                    && origin.state_parameter != parameter
            })
        {
            return Err("callee lineage has ambiguous state-arrival origins");
        }
        Ok(first.entry_parameter)
    }

    fn entry_formals(
        facts: &CheckFacts,
        roots: &[PermissionClaimIdentity],
    ) -> Result<
        Vec<(
            symbols::SymbolHandle,
            arena::HandleSpan<facts::PlaceSegment>,
        )>,
        &'static str,
    > {
        let mut formals = Vec::new();
        for root in roots {
            let mut entries = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.claim_identity == *root
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                });
            let (_, entry) = entries.next().ok_or(
                "equal content does not relate a freshly established callee root to an input",
            )?;
            if entries.next().is_some() {
                return Err("callee lineage has ambiguous entry custody");
            }
            let facts::PlaceRoot::Symbol(parameter) = entry.root else {
                return Err("callee lineage entry is not a formal");
            };
            formals.push((
                callable_entry_parameter(facts, entry, parameter)?,
                entry.segments,
            ));
        }
        Ok(formals)
    }

    /// A completed call keeps its own result identity. Follow that exact receipt
    /// to the callee's checked roots, then substitute the arguments actually moved
    /// by this invocation. Equal content alone never supplies a parent identity.
    fn call_result_roots(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        receipt: &checked_trees::FlowClaimJoinReceipt,
        projection: &ClaimBinding,
        traversal: &mut OriginTraversal,
    ) -> Result<Vec<PermissionClaimIdentity>, &'static str> {
        let control = &facts.flow.control;
        let ownership = &facts.flow.ownership;
        let flow = control
            .unique_state(receipt.machine_symbol, receipt.state_symbol)
            .ok_or("normal-result lineage has no unique caller flow")?;
        let mut calls = control
            .calls
            .span_or_empty(flow.calls)
            .iter()
            .filter(|call| {
                call.statement_index == receipt.statement_index
                    && call.call_ordinal == receipt.call_ordinal
                    && call.authored_expression == receipt.expression
                    && validation::call_target_entry_symbol(program, call.target_symbol)
                        == receipt.target_symbol
                    && call.authored_source_custody_valid
            });
        let call = calls
            .next()
            .ok_or("normal-result lineage has no exact invocation")?;
        if calls.next().is_some() {
            return Err("normal-result lineage has ambiguous invocations");
        }
        let site = validation::find_call_site(
            program,
            receipt.machine_symbol,
            receipt.state_symbol,
            receipt.statement_index,
            receipt.call_ordinal,
        )
        .ok_or("normal-result lineage lost its source invocation")?;
        if !matches!(&site, validation::CallSite::Expression { expression, call: source }
            if *expression == receipt.expression && !source.requirement_application.is_valid())
            || site.static_machine_parameter().is_valid()
            || site.static_requirement_dispatch().is_some()
            || call.static_requirement_dispatch.is_some()
            || validation::call_target_entry_symbol(program, site.target_symbol(program))
                != receipt.target_symbol
            || control.call_arguments.span(call.authored_arguments)
                != Some(validation::call_site_argument_expressions(program, &site))
        {
            return Err("normal-result lineage cannot inspect a selected private contract");
        }
        let (declaration, _) =
            validation::authored_call_parameters(program, receipt.machine_symbol, &site)?;
        if program.machine_by_symbol(declaration).is_none() {
            return Err("normal-result lineage has no direct source body");
        }
        let target = validation::call_target_state(program, call.target_symbol)
            .ok_or("normal-result lineage has no concrete return target")?;
        if program
            .machine_holding_state(target.symbol)
            .is_none_or(|machine| {
                machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            })
        {
            return Err("normal-result lineage has no independently checked body");
        }
        let parameters = program.state_parameters(target);
        if parameters.len() != call.argument_operands().len() {
            return Err("normal-result lineage lost its exact argument telescope");
        }
        let caller = program
            .machine_by_symbol(receipt.machine_symbol)
            .ok_or("normal-result lineage lost its caller")?;
        let state = program
            .state_by_symbol(receipt.state_symbol)
            .ok_or("normal-result lineage lost its caller state")?;
        let evaluation = crate::flow::call_permission_evaluation(
            program,
            caller,
            state,
            receipt.statement_index,
            receipt.call_ordinal,
        );
        if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
            return Err("normal-result lineage has no exact evaluation");
        }
        let consumer = crate::checks::multiplicity::ClaimConsumption {
            machine: receipt.machine_symbol,
            state: receipt.state_symbol,
            statement_index: receipt.statement_index,
            evaluation,
            source: PermissionEventSource::Call {
                statement_index: receipt.statement_index,
                call_ordinal: receipt.call_ordinal,
                target_symbol: call.target_symbol,
            },
        };
        let mut parents: Option<Vec<PermissionClaimIdentity>> = None;
        for alternative in ownership
            .claim_join_alternatives
            .span_or_empty(receipt.alternatives)
        {
            traversal.visit()?;
            let source = match alternative.source {
                FlowClaimJoinAlternativeSource::Claim { source, .. } => source,
                FlowClaimJoinAlternativeSource::Inactive
                | FlowClaimJoinAlternativeSource::ExcludedInput { .. } => continue,
            };
            let formals = match source {
                FlowClaimOutcomeSource::Input {
                    parameter_symbol,
                    segments,
                } => {
                    vec![(parameter_symbol, segments)]
                }
                FlowClaimOutcomeSource::Established { claim_identity, .. } => entry_formals(
                    facts,
                    &qualified_roots(program, facts, claim_identity, projection, traversal)?,
                )?,
                FlowClaimOutcomeSource::Unknown => return Err("callee result lineage is unknown"),
            };
            let mut candidate = Vec::new();
            for (parameter, path) in formals {
                traversal.visit()?;
                let position =
                    typed_trees::signature::state_parameter_position(parameters, parameter)
                        .ok_or("callee lineage is not rooted in this invocation's formal")?;
                let operand = call.argument_operands().handle_at(
                    u32::try_from(position).map_err(|_| "callee argument position overflow")?,
                );
                let mut conditions = crate::flow::SelectionConditions::new();
                let claims = crate::checks::multiplicity::consumed_operand_claims(
                    program,
                    facts.view(),
                    consumer,
                    operand,
                    &mut conditions,
                )?;
                let mut coverage = conditions.never();
                let mut actual_roots: Option<Vec<PermissionClaimIdentity>> = None;
                for claim in claims
                    .iter()
                    .filter(|claim| claim.path == ownership.segments.span_or_empty(path))
                {
                    let crate::checks::multiplicity::ReturnedClaimSource::Live(identity) =
                        claim.source
                    else {
                        return Err("callee lineage input is not live on every actual alternative");
                    };
                    let roots = qualified_roots(program, facts, identity, projection, traversal)?;
                    if actual_roots
                        .as_ref()
                        .is_some_and(|prior| !same_roots(prior, &roots))
                    {
                        return Err("callee lineage actual alternatives have different parents");
                    }
                    actual_roots = Some(roots);
                    coverage = conditions.any(coverage, claim.condition);
                }
                if !conditions.covers(control, coverage) {
                    return Err("callee lineage input does not cover every actual alternative");
                }
                for root in actual_roots.ok_or("callee lineage actual input is absent")? {
                    crate::shared::collections::push_unique(&mut candidate, root);
                }
            }
            if candidate.is_empty()
                || parents
                    .as_ref()
                    .is_some_and(|prior| !same_roots(prior, &candidate))
            {
                return Err("callee normal returns have incompatible input rosters");
            }
            parents = Some(candidate);
        }
        parents.ok_or("normal-result lineage has no live return alternative")
    }

    fn qualified_roots(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        claim: PermissionClaimIdentity,
        projection: &ClaimBinding,
        traversal: &mut OriginTraversal,
    ) -> Result<Vec<PermissionClaimIdentity>, &'static str> {
        traversal.visit()?;
        let key = (
            claim,
            projection.domain,
            projection.semantic_domain,
            projection.projection_machine,
        );
        if let Some((_, roots)) = traversal
            .completed
            .iter()
            .find(|(completed, _)| *completed == key)
        {
            return Ok(roots.clone());
        }
        if traversal.active.contains(&key) {
            return Err("callee content lineage is circular");
        }
        if traversal.active.len() >= 128 {
            return Err("callee content lineage depth budget exhausted");
        }
        traversal.active.push(key);
        let content = &facts.qualifications.content;
        let result = (|| {
            let mut roots: Option<Vec<PermissionClaimIdentity>> = None;
            for (_, transformation) in
                content
                    .claim_transformations
                    .iter()
                    .filter(|(_, transformation)| {
                        content
                            .body_claim_bindings
                            .span_or_empty(transformation.outputs)
                            .iter()
                            .any(|output| {
                                output.claim_identity == claim
                                    && same_projection(output, projection)
                            })
                    })
            {
                traversal.visit()?;
                let parents = match transformation.source {
                    TransformationSource::Call { .. } => call_parents(
                        program,
                        facts,
                        transformation,
                        &ClaimBinding {
                            claim_identity: claim,
                            ..projection.clone()
                        },
                        traversal,
                    )?,
                    TransformationSource::Body { .. } => content
                        .body_claim_bindings
                        .span_or_empty(transformation.inputs)
                        .iter()
                        .map(|input| input.claim_identity)
                        .collect(),
                    TransformationSource::Unknown => {
                        return Err("callee lineage has an unknown producer");
                    }
                };
                if parents.is_empty() {
                    return Err("callee content transformation has no parent");
                }
                let mut candidate = Vec::new();
                for parent in parents {
                    let ancestors = if parent == claim {
                        vec![parent]
                    } else {
                        qualified_roots(program, facts, parent, projection, traversal)?
                    };
                    for ancestor in ancestors {
                        crate::shared::collections::push_unique(&mut candidate, ancestor);
                    }
                }
                if roots
                    .as_ref()
                    .is_some_and(|previous| !same_roots(previous, &candidate))
                {
                    return Err("callee content transformation has conflicting input rosters");
                }
                roots = Some(candidate);
            }
            if let Some(roots) = roots {
                return Ok(roots);
            }
            let mut receipts = facts
                .flow
                .ownership
                .claim_join_receipts
                .iter()
                .filter(|(_, receipt)| receipt.claim_identity == claim);
            let Some((_, receipt)) = receipts.next() else {
                return Ok(vec![claim]);
            };
            if receipts.next().is_some() {
                return Err("callee lineage has ambiguous normal-result receipts");
            }
            call_result_roots(program, facts, receipt, projection, traversal)
        })();
        traversal.active.pop();
        if let Ok(roots) = &result {
            traversal.completed.push((key, roots.clone()));
        }
        result
    }

    fn same_roots(left: &[PermissionClaimIdentity], right: &[PermissionClaimIdentity]) -> bool {
        left.len() == right.len() && left.iter().all(|claim| right.contains(claim))
    }

    fn same_projection(left: &ClaimBinding, right: &ClaimBinding) -> bool {
        left.domain == right.domain
            && left.semantic_domain == right.semantic_domain
            && left.projection_machine == right.projection_machine
    }
}
mod call_results {
    /*
        Replay provisional owned result qualifications against the exact invocation.

        Flow publishes normal-return promises before ownership checking. This gate
        joins their structural subjects to the completed ownership ledger; a result
        annotation or a compatible content algebra cannot substitute for that join.
    */
    use super::content_paths::content_segments_to_fact_path;
    use crate::checks::multiplicity::linear_claim_frontier;
    use crate::flow::expressions;
    use crate::published::facts::qualification_evidence::unwrapped_type_references_match;
    use crate::published::labels::symbol_name;
    use crate::published::places::place_at;
    use checked_trees::CheckFacts;
    use diagnostics::Diagnostic;
    use facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, PlaceSegment, ProgramPoint};
    use language_semantics::content::{
        ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionPlan,
        ContentStructuralPlace,
    };
    use language_semantics::{
        DomainEstablishmentRoute, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionEventSource, QualificationEvidenceOrigin,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, TableCallExpression};
    use typed_trees::signature::StateParameter;

    fn runtime_contract_at<'program>(
        program: &'program dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        ordinal: usize,
        site: &validation::CallSite<'_>,
    ) -> Option<&'program typed_trees::callable_values::RuntimeCallableContract> {
        let validation::CallContractSource::Runtime { contract, .. } =
            validation::call_contract_source(program, machine, site).ok()??
        else {
            return None;
        };
        let retained = facts
            .proof
            .contract_call_at(&(machine, state, statement, ordinal))?;
        crate::published::proof::contracts::call_signatures::source_is_current(
            program,
            &facts.proof,
            retained,
            site,
        )
        .then_some(contract)
    }

    mod parameter_qualifications {
        /*
            Exact out-parameter subjects for one completed invocation.

            Flow publishes one promise per qualified child, including fixed-array
            elements. Rebuilding the entire declaration and projecting every child for
            each promise makes this join quadratic. Prepare that immutable roster once
            and order its exact storage identities; no verdict or live premise is cached.
            Checked bodies owe these obligations at every normal exit. A bodyless call
            still needs the same readable-exclusive formal and exact entry membership:
            an annotation, issuer name or prepared subject cannot introduce authority.
        */
        use crate::checks::contracts::exclusive_referent_type;
        use crate::checks::contracts::is_readable_mutable_reference;
        use crate::checks::places::place_in;
        use crate::flow::RetainedMembership;
        use crate::flow::call_parameter_qualification_identities;
        use crate::flow::operand_binds_live_view;
        use crate::flow::project_captured_view_place;
        use crate::flow::replay_retained_memberships;
        use crate::published::places::place_at;
        use checked_trees::{CheckFacts, FlowCallFact};
        use facts::{FactPlace, PlaceRoot, PlaceSegment, ProgramPoint, QualificationEvidence};
        use language_semantics::{QualificationEvidenceOrigin, SemanticDomainId};
        use symbols::SymbolHandle;
        use typed_trees::signature::StateParameter;
        use validation::{CallSite, CanonicalPlace, CheckingInput};

        struct ParameterSubject {
            place: CanonicalPlace,
            domain: SymbolHandle,
            instance: SemanticDomainId,
            position: usize,
        }

        pub(super) struct ParameterQualificationJoin<'program> {
            program: &'program dyn CheckingInput,
            facts: &'program CheckFacts,
            target: SymbolHandle,
            parameters: &'program [StateParameter],
            call_flow: &'program FlowCallFact,
            checked_contract: bool,
            runtime_call: Option<typed_trees::callable_values::RuntimeCallBinding>,
            subjects: Vec<ParameterSubject>,
            retained: Vec<RetainedMembership>,
        }

        impl<'program> ParameterQualificationJoin<'program> {
            pub(super) fn prepare(
                program: &'program dyn CheckingInput,
                facts: &'program CheckFacts,
                site: CallSite<'_>,
                point: ProgramPoint,
            ) -> Option<Self> {
                let ProgramPoint::CallEnsures {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    call_ordinal,
                } = point
                else {
                    return None;
                };
                let target = match site {
                    CallSite::Expression { call, .. } => {
                        validation::call_target_symbol(program, call)
                    }
                    CallSite::Statement(call) => call.target_symbol,
                    CallSite::TransitionNamed { path, .. } => path.symbol,
                };
                let runtime_call = site.runtime_call();
                let runtime = if runtime_call.is_some() {
                    Some(super::runtime_contract_at(
                        program,
                        facts,
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        call_ordinal,
                        &site,
                    )?)
                } else {
                    None
                };
                let source =
                    validation::call_contract_source(program, machine_symbol, &site).ok()?;
                let selected = source
                    .as_ref()
                    .map(validation::CallContractSource::signature)
                    .or_else(|| match site {
                        CallSite::Expression { call, .. } => {
                            program.requirement_call_signature(machine_symbol, call)
                        }
                        _ => None,
                    });
                let parameters = selected
                    .map(|signature| program.state_signature_parameters(signature))
                    .or_else(|| validation::call_target_parameters(program, target))?;
                let arguments = validation::call_site_argument_expressions(program, &site);
                let state_flow = facts.flow.control.state(machine_symbol, state_symbol)?;
                let call_flow = facts
                    .flow
                    .control
                    .calls
                    .span(state_flow.calls)?
                    .iter()
                    .find(|call| {
                        call.statement_index == statement_index
                            && call.call_ordinal == call_ordinal
                            && call.runtime_call == runtime_call
                            && if runtime_call.is_some() {
                                !call.target_symbol.is_valid()
                            } else {
                                call.target_symbol
                                    == validation::call_target_entry_symbol(program, target)
                            }
                    })?;
                let operands = facts.flow.control.operands.span(call_flow.operands)?;
                let receiver = match site {
                    CallSite::Expression { call, .. } if call.receiver.is_valid() => {
                        Some(call.receiver)
                    }
                    _ => None,
                };
                if parameters.len() != arguments.len()
                    || !operands
                        .iter()
                        .map(|operand| operand.expression)
                        .eq(runtime_call
                            .map(|binding| binding.callee)
                            .into_iter()
                            .chain(receiver)
                            .chain(arguments.iter().copied()))
                {
                    return None;
                }
                let checked_contract = runtime.is_some()
                    || validation::call_target_has_checked_body_contract(program, target);
                let identities = call_parameter_qualification_identities(program, target, selected);
                let mut subjects = Vec::new();
                for (position, parameter) in parameters.iter().enumerate() {
                    let mut promised = identities
                        .iter()
                        .filter(|(argument_position, _, _)| *argument_position == position)
                        .map(|(_, domain, instance)| (Vec::new(), *domain, *instance))
                        .collect::<Vec<_>>();
                    let referent = exclusive_referent_type(program, parameter.type_reference);
                    if referent.is_valid() {
                        promised.extend(
                            validation::field_domains::declared_result_field_domain_paths(
                                program, referent,
                            ),
                        );
                    }
                    if promised.is_empty() {
                        continue;
                    }
                    let storage = if operand_binds_live_view(program, parameter.type_reference) {
                        let Some(operand) = operands.get(
                            position
                                + usize::from(runtime_call.is_some())
                                + usize::from(receiver.is_some()),
                        ) else {
                            continue;
                        };
                        let Some(referents) =
                            facts.flow.control.operand_referents.span(operand.referents)
                        else {
                            continue;
                        };
                        referents
                            .iter()
                            .filter_map(|referent| {
                                place_in(
                                    &facts.flow.control.operand_referent_segments,
                                    referent.root,
                                    referent.segments,
                                )
                            })
                            .collect::<Vec<_>>()
                    } else {
                        place_at(program, state_symbol, statement_index, arguments[position])
                            .into_iter()
                            .collect()
                    };
                    for storage in storage {
                        for (path, domain, instance) in &promised {
                            if let Some(place) = project_captured_view_place(storage.clone(), path)
                            {
                                subjects.push(ParameterSubject {
                                    place,
                                    domain: *domain,
                                    instance: *instance,
                                    position,
                                });
                            }
                        }
                    }
                }
                subjects.sort_unstable_by(|left, right| {
                    left.place
                        .compare_identity(&right.place)
                        .then_with(|| left.position.cmp(&right.position))
                });
                let retained = replay_retained_memberships(program, facts, point);
                Some(Self {
                    program,
                    facts,
                    target,
                    parameters,
                    call_flow,
                    checked_contract,
                    runtime_call,
                    subjects,
                    retained,
                })
            }

            pub(super) fn check(
                &self,
                evidence: &QualificationEvidence,
                root: PlaceRoot,
                path: &[PlaceSegment],
                domain: SymbolHandle,
                instance: SemanticDomainId,
            ) -> Option<()> {
                if evidence.origin != QualificationEvidenceOrigin::Propagated
                    || if self.runtime_call.is_some() {
                        evidence.source_symbol.is_valid()
                    } else {
                        evidence.source_symbol != self.target
                            && evidence.source_symbol != self.call_flow.target_symbol
                    }
                    || evidence.requirement_symbol.is_valid()
                {
                    return None;
                }
                let subject = CanonicalPlace {
                    root,
                    segments: path.to_vec(),
                };
                if self.retained.iter().any(|row| {
                    row.place == subject && row.domain == domain && row.instance == instance
                }) {
                    return Some(());
                }
                let start = self
                    .subjects
                    .partition_point(|row| row.place.compare_identity(&subject).is_lt());
                let position = self.subjects[start..]
                    .iter()
                    .take_while(|row| row.place == subject)
                    .find(|row| row.domain == domain && row.instance == instance)?
                    .position;
                if self.checked_contract {
                    return Some(());
                }
                let parameter = self.parameters.get(position)?;
                if !is_readable_mutable_reference(self.program, parameter.type_reference) {
                    return None;
                }
                self.facts
                    .flow
                    .semantic_constraint_contexts(self.call_flow.entry_constraints)
                    .any(|context| {
                        self.facts
                            .semantic
                            .context_view(self.facts.semantic.contexts.get(context))
                            .facts()
                            .any(|fact| {
                                fact.payload.domain_membership().is_some_and(
                                    |(_, _, symbol, identity)| {
                                        symbol == domain && identity == instance
                                    },
                                ) && matches!(fact.place, FactPlace::Place(place)
                            if self.facts.semantic.places.get(place).root == root
                                && self.facts.semantic.place_segments.span_or_empty(
                                    self.facts.semantic.places.get(place).segments) == path)
                            })
                    })
                    .then_some(())
            }
        }
    }
    mod retained_cell {
        /*
            Extraction returns the qualified value already owned by a resident or run.
            Identify its exact payload slot after the caller authenticates the core
            operation and its complete applied result type. Shape alone is not authority.
        */
        use facts::PlaceSegment;
        use typed_trees::data::DataMember;
        use typed_trees::types::TypeReferenceHandle;

        pub(super) fn is_extracted_value_path(
            program: &dyn validation::CheckingInput,
            result: TypeReferenceHandle,
            element: TypeReferenceHandle,
            path: &[PlaceSegment],
        ) -> bool {
            let Some(outcome) = program
                .data_application_definition(result)
                .or_else(|| program.closed_data_definition(result))
            else {
                return false;
            };
            let [DataMember::Variant(moved)] = program.data_members(outcome) else {
                return false;
            };
            let [_storage, value] = program.data_payload_fields(moved) else {
                return false;
            };
            let [
                PlaceSegment::Case { variant },
                PlaceSegment::Field { symbol },
                ..,
            ] = path
            else {
                return false;
            };
            *variant == moved.symbol
                && *symbol == value.symbol
                && program.normalized_type_identity(value.type_reference)
                    == program.normalized_type_identity(element)
        }
    }
    use parameter_qualifications::ParameterQualificationJoin;

    pub(super) struct Invocation<'program> {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement: usize,
        pub ordinal: usize,
        pub expression: ExpressionHandle,
        pub call: &'program TableCallExpression,
        /*
            The selected ownership walk's completed prefix, or its published ledger.
            A previous permission pass cannot supply evidence for this completion.
        */
        pub events: InvocationEvents<'program>,
    }

    pub(super) enum InvocationEvents<'events> {
        Published(&'events CheckFacts),
        Completed(&'events [checked_trees::FlowPermissionEventFact]),
    }

    impl InvocationEvents<'_> {
        fn iter(&self) -> impl Iterator<Item = &checked_trees::FlowPermissionEventFact> {
            let (published, completed) = match self {
                Self::Published(facts) => (Some(*facts), None),
                Self::Completed(events) => (None, Some(*events)),
            };
            published
                .into_iter()
                .flat_map(|facts| {
                    facts
                        .flow
                        .ownership
                        .permissions
                        .iter()
                        .map(|(_, event)| event)
                })
                .chain(completed.into_iter().flatten())
        }
    }

    impl Invocation<'_> {
        fn target_symbol(&self, program: &dyn ::validation::CheckingInput) -> SymbolHandle {
            validation::call_target_symbol(program, self.call)
        }

        fn result_type(
            &self,
            program: &dyn ::validation::CheckingInput,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            let (machine, state) =
                validation::machine_state_by_symbols(program, self.machine, self.state)?;
            validation::expression_result_type_reference(program, machine, state, self.expression)
        }

        pub(super) fn evaluation(
            &self,
            program: &dyn ::validation::CheckingInput,
        ) -> Option<checked_trees::FlowPermissionEvaluation> {
            let evaluation = crate::flow::call_permission_evaluation(
                program,
                program.machine_by_symbol(self.machine)?,
                program.state_by_symbol(self.state)?,
                self.statement,
                self.ordinal,
            );
            (evaluation != checked_trees::FlowPermissionEvaluation::Unknown).then_some(evaluation)
        }
    }

    pub(super) fn check_call_result_qualifications(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut parameter_join = None;
        for (_, fact) in facts.semantic.facts.iter() {
            let (FactOrigin::CallEnsures, FactPlace::Place(place)) = (fact.origin, fact.place)
            else {
                continue;
            };
            let FactPayload::DomainMembership {
                domain_symbol,
                semantic_domain,
                ..
            } = fact.payload
            else {
                continue;
            };
            let Some(domain) = typed_trees::domain::domain_by_symbol(program, domain_symbol)
                .filter(|_| {
                    validation::field_domains::domain_requires_provenance(program, domain_symbol)
                })
            else {
                continue;
            };
            let place = facts.semantic.places.get(place);
            let path = facts.semantic.place_segments.span_or_empty(place.segments);
            let mut primitive_error = None;
            let mut failure_reason = None;
            let valid = (|| {
                let ProgramPoint::CallEnsures {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    call_ordinal,
                } = fact.point
                else {
                    return None;
                };
                let site = validation::find_call_site(
                    program,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    call_ordinal,
                )?;
                let is_result_place = matches!(
                    (place.root, site),
                    (
                        PlaceRoot::Expression(expression),
                        validation::CallSite::Expression {
                            expression: actual,
                            ..
                        }
                    ) if expression == actual
                );
                if !is_result_place {
                    if !matches!(&parameter_join, Some((point, _)) if *point == fact.point) {
                        parameter_join = Some((
                            fact.point,
                            ParameterQualificationJoin::prepare(program, facts, site, fact.point),
                        ));
                    }
                    return parameter_join.as_ref()?.1.as_ref()?.check(
                        &fact.evidence,
                        place.root,
                        path,
                        domain_symbol,
                        semantic_domain,
                    );
                }
                let (PlaceRoot::Expression(expression), validation::CallSite::Expression { call, .. }) =
                    (place.root, site)
                else {
                    return None;
                };
                let target_symbol = validation::call_target_symbol(program, call);
                let source = match validation::call_contract_source(program, machine_symbol, &site) {
                    Ok(source) => source,
                    Err(reason) => { failure_reason = Some(reason); return None; }
                };
                let runtime = if site.runtime_call().is_some() {
                    Some(runtime_contract_at(program, facts, machine_symbol, state_symbol,
                        statement_index, call_ordinal, &site)?)
                } else { None };
                let public_contract = match source.as_ref() {
                    Some(validation::CallContractSource::Requirement { declaration, signature }) =>
                        Some((*declaration, *signature)),
                    _ => None,
                };
                let exact_source = if runtime.is_some() {
                    // The occurrence and retained row carry identity. This field has
                    // no nominal issuer for an indirect call.
                    !fact.evidence.source_symbol.is_valid()
                } else {
                    fact.evidence.source_symbol == target_symbol
                        || fact.evidence.source_symbol == validation::call_target_entry_symbol(program, target_symbol)
                        || public_contract.is_some_and(|(_, signature)| fact.evidence.source_symbol == signature.symbol)
                };
                if fact.evidence.origin != QualificationEvidenceOrigin::Propagated
                    || !exact_source
                    || fact.evidence.requirement_symbol.is_valid()
                {
                    failure_reason = Some("the published qualification does not identify this exact invocation");
                    return None;
                }
                let invocation = Invocation {
                    machine: machine_symbol,
                    state: state_symbol,
                    statement: statement_index,
                    ordinal: call_ordinal,
                    expression,
                    call,
                    events: InvocationEvents::Published(facts),
                };
                let selected = source.as_ref().map(validation::CallContractSource::signature)
                    .or_else(|| program.requirement_call_signature(machine_symbol, call));
                let declared = crate::flow::call_result_qualification_identities(
                    program,
                    target_symbol,
                    selected,
                    invocation.result_type(program)?,
                );
                if !declared.iter().any(|(declared_path, symbol, identity)| {
                    declared_path == path && *symbol == domain_symbol && *identity == semantic_domain
                }) {
                    failure_reason = Some("the selected result contract does not declare this exact qualified path and domain instance");
                    return None;
                }
                let projection = facts
                    .qualifications
                    .content
                    .for_semantic_domain(semantic_domain);
                if runtime.is_some()
                    || validation::call_target_has_checked_body_contract(program, target_symbol) {
                    return Some(());
                }
                if public_contract.is_some_and(|(declaration, _)| !declaration.is_boundary) {
                    return Some(());
                }

                match private_access_result_contract(program, &invocation, domain, semantic_domain, path) {
                    Ok(true) => {
                        if result_claim(program, facts.view(), &invocation, path).is_none() {
                            primitive_error = Some(Diagnostic::error(
                                "placement primitive result has no exact owned result claim",
                            ));
                            return None;
                        }
                        return Some(());
                    }
                    Ok(false) => {}
                    Err(error) => {
                        primitive_error = Some(error);
                        return None;
                    }
                }

                if (projection.is_none() || !path.is_empty())
                    && boundary_requirement_signature(program, target_symbol, domain).is_none()
                    && program.traits().iter().any(|owner| {
                        owner.is_boundary
                            && program
                                .trait_machine_signature_by_symbol(owner, target_symbol)
                                .is_some()
                    })
                {
                    return Some(());
                }

                let call_parameters = selected
                    .map(|signature| program.state_signature_parameters(signature))
                    .or_else(|| validation::call_target_parameters(program, target_symbol));

                if path.is_empty()
                    && match (call_parameters, projection) {
                        (Some(parameters), Some(projection)) => !consumes_qualified_input(
                            program,
                            facts,
                            &invocation,
                            parameters,
                            projection,
                        ),
                        (Some(_), None) => true,
                        (None, _) => false,
                    }
                    && (facts.proof.contract_facts.iter().any(|(_, contract)| {
                        let exact_call = facts.proof.contract_call_at(&(
                            invocation.machine,
                            invocation.state,
                            invocation.statement,
                            invocation.ordinal,
                        ));
                        let (owner_symbol, state_symbol, applied) = match contract.owner {
                            checked_trees::ContractProofFactOwner::CallSignature {
                                owner_symbol,
                                state_symbol,
                                ..
                            } => {
                                let Some(exact_call) = exact_call else {
                                    return false;
                                };
                                if !crate::published::proof::contracts::call_signatures::owns_contract(
                                    program,
                                    &facts.proof,
                                    exact_call,
                                    contract,
                                ) {
                                    return false;
                                }
                                (owner_symbol, state_symbol, true)
                            }
                            checked_trees::ContractProofFactOwner::StateSignature {
                                owner_symbol,
                                state_symbol,
                            } if !exact_call.is_some_and(|call| call.signature.is_valid()) => {
                                (owner_symbol, state_symbol, false)
                            }
                            _ => return false,
                        };
                        if state_symbol != target_symbol
                            || contract.kind != checked_trees::ContractProofFactKind::Ensures
                        {
                            return false;
                        }
                        let selected_fact = if applied {
                            Some(program.proof_facts.get(contract.fact))
                        } else {
                            invocation_contract_fact(
                                program,
                                selected,
                                owner_symbol,
                                state_symbol,
                                contract.fact,
                            )
                        };
                        let Some(membership) = selected_fact.and_then(|fact| fact.as_membership())
                        else {
                            return false;
                        };
                        if membership.domain_symbol != domain_symbol
                            || membership.semantic_domain != semantic_domain
                        {
                            return false;
                        }
                        authorized_ensures_signature(program, owner_symbol, state_symbol, contract.fact)
                    }) || result_type_issuance_route(program, target_symbol, selected, domain))
                {
                    return Some(());
                }

                if path
                    .iter()
                    .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
                    && case_payload_issuance_route(program, target_symbol, selected, domain, path)
                    && match (call_parameters, projection) {
                        (Some(parameters), Some(projection)) => !consumes_qualified_input(
                            program,
                            facts,
                            &invocation,
                            parameters,
                            projection,
                        ),
                        (Some(_), None) => true,
                        (None, _) => false,
                    }
                {
                    return Some(());
                }

                let Some(projection) = projection else {
                    failure_reason = Some("the qualified domain has no retained content projection for conservation");
                    return None;
                };
                if result_claim(program, facts.view(), &invocation, path).is_none() {
                    failure_reason = Some("the qualified result path has no exact owned result claim");
                    return None;
                }
                let Some(parameters) = call_parameters else {
                    failure_reason = Some("the exact invocation has no retained input signature");
                    return None;
                };
                if let Some(outcomes) = super::retained_custody::lifecycle_backing_transfers(
                    program, target_symbol, parameters, invocation.result_type(program)?, projection,
                ) {
                    let Some(outcome) = outcomes.iter().find(|outcome| outcome.iter().any(|transfer| transfer.output == *path)) else {
                        failure_reason = Some("the placement contract has no exact outcome for this result path");
                        return None;
                    };
                    let mut consumed = Vec::new();
                    let mut returned = Vec::new();
                    for transfer in outcome {
                        let Some(input) = input_claim(program, facts.view(), &invocation, parameters,
                            transfer.parameter, &transfer.input, projection) else {
                                failure_reason = Some("the placement outcome has no exact consumed backing claim");
                                return None;
                            };
                        let Some(output) = result_claim(program, facts.view(), &invocation, &transfer.output) else {
                            failure_reason = Some("the placement outcome has no exact returned backing claim");
                            return None;
                        };
                        if consumed.contains(&input) || returned.contains(&output) {
                            failure_reason = Some("the placement outcome duplicates an input or output backing claim");
                            return None;
                        }
                        consumed.push(input);
                        returned.push(output);
                    }
                    return Some(());
                }
                if facts
                    .qualifications
                    .content
                    .conservation_plans
                    .iter()
                    .any(|plan| {
                        plan.callable == target_symbol
                            && plan.algebra == projection.algebra
                            && [
                                (plan.equation.left(), plan.equation.right()),
                                (plan.equation.right(), plan.equation.left()),
                            ]
                            .into_iter()
                            .any(|(input, output)| {
                                replay_equation(
                                    program,
                                    facts,
                                    &invocation,
                                    parameters,
                                    projection,
                                    path,
                                    input,
                                    output,
                                )
                            })
                    })
                {
                    return Some(());
                }

                let outputs = declared
                    .iter()
                    .filter(|(_, _, identity)| *identity == semantic_domain)
                    .collect::<Vec<_>>();
                if outputs.is_empty()
                    || outputs.iter().enumerate().any(|(position, output)| {
                        outputs[..position].iter().any(|previous| {
                            !crate::checks::multiplicity::claim_paths_are_case_alternatives(
                                &previous.0,
                                &output.0,
                            )
                        })
                    })
                {
                    failure_reason = Some("the result has simultaneous qualified destinations without a conservation proof");
                    return None;
                }
                let mut inputs = Vec::new();
                for (position, parameter) in parameters.iter().enumerate() {
                    for path in qualified_input_paths(program, parameter.type_reference, projection) {
                        if let Some(identity) = input_claim(
                            program,
                            facts.view(),
                            &invocation,
                            parameters,
                            position,
                            &path,
                            projection,
                        ) {
                            inputs.push(identity);
                        }
                    }
                }
                if inputs.len() != 1 {
                    failure_reason = Some("the invocation does not transfer exactly one established input claim for this domain");
                    return None;
                }
                Some(())
            })()
            .is_some();
            if !valid {
                if let Some(error) = primitive_error {
                    diagnostics.push(error);
                    continue;
                }
                let location = match fact.point {
                    ProgramPoint::CallEnsures {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        call_ordinal,
                    } => format!(
                        " in `{}`, state `{}` (statement {statement_index}, call {call_ordinal})",
                        symbol_name(program, machine_symbol),
                        symbol_name(program, state_symbol),
                    ),
                    _ => String::new(),
                };
                let reason = failure_reason.unwrap_or("the exact invocation, authorized route or consumed qualified claims, and result correspondence are not proved");
                diagnostics.push(Diagnostic::error(format!(
                    "cannot establish call-result qualification `{}`{location}: {reason}",
                    domain.name,
                )));
            }
        }
    }

    /*
        Recognize only an authenticated closed lifecycle operation whose exact
        result subject and evaluated P/T plan survive preparation. An arbitrary
        bodyless exact-machine route is not a compiler implementation.
    */
    fn private_access_result_contract(
        program: &dyn ::validation::CheckingInput,
        invocation: &Invocation<'_>,
        domain: &typed_trees::domain::DomainDefinition,
        semantic_domain: language_semantics::SemanticDomainId,
        path: &[PlaceSegment],
    ) -> Result<bool, Diagnostic> {
        let callable = invocation.target_symbol(program);
        if let Some(declaration) =
            typed_trees::typed_trees::EmptyRunReleaseDeclaration::authenticate(program, callable)?
        {
            let state = program.state_by_symbol(declaration.state).ok_or_else(|| {
                Diagnostic::error("empty-run release lost its authenticated entry")
            })?;
            let result = invocation
                .result_type(program)
                .ok_or_else(|| Diagnostic::error("empty-run release lost its exact result type"))?;
            return Ok(path.is_empty()
                && program.normalized_type_identity(result)
                    == program.normalized_type_identity(state.return_type)
                && domain.establishment_routes.iter().any(|route| matches!(route,
                    DomainEstablishmentRoute::ExactMachine { machine } if *machine == declaration.template))
                && validation::field_domains::declared_domain_constraint_records(program, result)
                    .iter().any(|constraint| constraint.symbol == domain.symbol
                        && constraint.semantic_id == semantic_domain));
        }
        let Some(declaration) =
            typed_trees::typed_trees::AccessExecutionDeclaration::authenticate(program, callable)?
        else {
            return Ok(false);
        };
        let bindings = declaration.source_invocation_bindings(
            program,
            invocation.machine,
            invocation.call.authored_machine_application,
        )?;
        let state = program
            .state_by_symbol(declaration.state())
            .ok_or_else(|| {
                Diagnostic::error("placement primitive result lost its authenticated entry")
            })?;
        let result_type = invocation.result_type(program).ok_or_else(|| {
            Diagnostic::error("placement source contract lost its applied result type")
        })?;
        let expected = program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
            substitutions: &bindings,
            ..typed_trees::type_identity::TypeIdentityRequest::ordinary(state.return_type)
        });
        if expected != program.normalized_type_identity(result_type) {
            return Err(Diagnostic::error(
                "placement source result disagrees with its exact invocation-owned type operands",
            ));
        }
        let retained_cell = declaration.operation()
            == typed_trees::typed_trees::AccessLifecycleOperation::MoveOut
            && retained_cell::is_extracted_value_path(program, result_type, bindings[1].1, path);
        if !retained_cell
            && !domain.establishment_routes.iter().any(|route| {
                matches!(route, DomainEstablishmentRoute::ExactMachine { machine }
                    if *machine == declaration.template())
            })
        {
            return Ok(false);
        }
        let matches = path
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
            && qualified_leaf_carrier(program, result_type, path).is_some_and(|carrier| {
                validation::field_domains::declared_domain_constraint_records(program, carrier)
                    .into_iter()
                    .any(|constraint| {
                        constraint.symbol == domain.symbol
                            && constraint.semantic_id == semantic_domain
                            && typed_trees::domain::domain_accepts_carrier(
                                program,
                                domain,
                                program
                                    .type_reference_table
                                    .unconstrained_type_reference(carrier),
                                &constraint.arguments,
                            )
                    })
            });
        if !matches {
            return Err(Diagnostic::error(
                "placement primitive result does not name the exact declared case payload",
            ));
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    fn replay_equation(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        invocation: &Invocation<'_>,
        parameters: &[StateParameter],
        projection: &ContentProjectionPlan,
        required_path: &[PlaceSegment],
        input: &ContentConservationTerm,
        output: &ContentConservationTerm,
    ) -> bool {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        if !projection_places(input, projection, &mut inputs)
            || !projection_places(output, projection, &mut outputs)
            || inputs.is_empty()
            || outputs.is_empty()
        {
            return false;
        }
        let mut input_identities = Vec::new();
        for subject in inputs {
            let ContentPlaceRoot::Parameter {
                position, symbol, ..
            } = subject.root
            else {
                return false;
            };
            let Ok(position) = usize::try_from(position) else {
                return false;
            };
            let Some(parameter) = parameters.get(position) else {
                return false;
            };
            let Some(path) = content_segments_to_fact_path(&subject.segments) else {
                return false;
            };
            if subject.version != ContentPlaceVersion::Entry || parameter.symbol != symbol {
                return false;
            }
            let Some(identity) = input_claim(
                program,
                facts.view(),
                invocation,
                parameters,
                position,
                &path,
                projection,
            ) else {
                return false;
            };
            if input_identities.contains(&identity) {
                return false;
            }
            input_identities.push(identity);
        }
        let mut output_identities = Vec::new();
        let mut includes_required = false;
        for subject in outputs {
            if subject.root != ContentPlaceRoot::Result
                || subject.version != ContentPlaceVersion::Current
            {
                return false;
            }
            let Some(path) = content_segments_to_fact_path(&subject.segments) else {
                return false;
            };
            includes_required |= path == required_path;
            let Some(identity) = result_claim(program, facts.view(), invocation, &path) else {
                return false;
            };
            if output_identities.contains(&identity) {
                return false;
            }
            output_identities.push(identity);
        }
        includes_required
    }

    /*
        The signature `state_symbol` on the `owner_symbol` trait carries `ensures`
        authorization for `fact`, when the owner and its requirement exist.
        Match the invocation's payload while retaining the declaration's original
        clause as the authorization witness. Applying a telescope cannot turn a
        provider's stronger guarantee into a public issuer grant.
    */
    fn invocation_contract_fact<'program>(
        program: &'program dyn ::validation::CheckingInput,
        selected: Option<&typed_trees::signature::StateSignature>,
        owner: SymbolHandle,
        requirement: SymbolHandle,
        fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> Option<&'program typed_trees::domain::ProofFact> {
        let Some(selected) = selected else {
            return Some(program.proof_facts.get(fact));
        };
        let owner = program.trait_definition_by_symbol(owner)?;
        let declared = program.trait_machine_signature_by_symbol(owner, requirement)?;
        program.applied_signature_contract_fact(declared, selected, fact)
    }

    fn authorized_ensures_signature(
        program: &dyn ::validation::CheckingInput,
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> bool {
        program
            .trait_definition_by_symbol(owner_symbol)
            .and_then(|owner| program.trait_machine_signature_by_symbol(owner, state_symbol))
            .is_some_and(|signature| {
                ::validation::boundary_qualification_authorization(
                    program,
                    owner_symbol,
                    signature,
                    typed_trees::signature::SignatureContractKind::Ensures,
                    program.proof_facts.get(fact),
                )
                .is_some()
            })
    }

    /*
        The signature a boundary trait declares under `target_symbol`, present
        only when one of `domain`'s `established by` routes names that trait and
        requirement as the issuance witness.
    */
    fn boundary_requirement_signature<'a>(
        program: &'a dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
        domain: &typed_trees::domain::DomainDefinition,
    ) -> Option<&'a typed_trees::signature::StateSignature> {
        let owner = program.traits().iter().find(|owner| {
            owner.is_boundary
                && program
                    .trait_machine_signature_by_symbol(owner, target_symbol)
                    .is_some()
        })?;
        let signature = program.trait_machine_signature_by_symbol(owner, target_symbol)?;
        let authority_signature = program
            .closed_requirement_application(target_symbol)
            .filter(|application| application.declaring_trait == owner.symbol)
            .map_or(signature.symbol, |application| application.template);
        domain
            .establishment_routes
            .iter()
            .any(|route| {
                matches!(
                    route,
                    DomainEstablishmentRoute::BoundaryRequirement {
                        boundary_trait,
                        requirement,
                    } if *boundary_trait == owner.symbol && *requirement == authority_signature
                )
            })
            .then_some(signature)
    }

    /*
        The other authorized issuance spelling: a boundary requirement carries
        its grant on the constrained result type (`-> T in D`) instead of an
        `ensures` clause. When the domain's `established by` route names that
        exact requirement, the signature's own declared constraint is the
        issuance witness — the bare-result and freshness gates above still
        apply, so transferred custody cannot mint supply here either.
    */
    fn result_type_issuance_route(
        program: &dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
        selected: Option<&typed_trees::signature::StateSignature>,
        domain: &typed_trees::domain::DomainDefinition,
    ) -> bool {
        let Some(signature) = boundary_requirement_signature(program, target_symbol, domain) else {
            return false;
        };
        unwrapped_type_references_match(
            program,
            selected.unwrap_or(signature).return_type,
            domain.target_type,
        )
    }

    /*
        The case-scoped spelling of `result_type_issuance_route`: instead of the
        result itself being `T in D`, one case of the owned result carrier
        declares its payload `T in D` and the granted subject path selects exactly
        that payload. The `established by` route must name the called requirement,
        and the leaf field's declared carrier must be the domain's own target
        type — a payload that wears the constraint on an unrelated carrier cannot
        mint here.
    */
    fn case_payload_issuance_route(
        program: &dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
        selected: Option<&typed_trees::signature::StateSignature>,
        domain: &typed_trees::domain::DomainDefinition,
        path: &[PlaceSegment],
    ) -> bool {
        let Some(signature) = boundary_requirement_signature(program, target_symbol, domain) else {
            return false;
        };
        qualified_leaf_carrier(program, selected.unwrap_or(signature).return_type, path)
            .is_some_and(|carrier| {
                unwrapped_type_references_match(program, carrier, domain.target_type)
            })
    }

    /*
        The declared type of the field at `path` inside an owned result carrier:
        `Case` segments select the variant whose payload the following `Field`
        resolves in, `Field` descends record storage, `FixedIndex` descends an
        element. Anything else has no declared carrier to compare.
    */
    fn qualified_leaf_carrier(
        program: &dyn ::validation::CheckingInput,
        return_type: typed_trees::types::TypeReferenceHandle,
        path: &[PlaceSegment],
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let mut carrier = return_type;
        let mut variant_scope: Option<SymbolHandle> = None;
        for segment in path {
            match segment {
                PlaceSegment::Case { variant } => {
                    variant_scope = Some(*variant);
                }
                PlaceSegment::Field { symbol } => {
                    let reference = program
                        .type_reference_table
                        .unconstrained_type_reference(carrier);
                    let data = program
                        .data_application_definition(reference)
                        .or_else(|| program.closed_data_definition(reference))?;
                    let field = match variant_scope.take() {
                        Some(variant_symbol) => program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Variant(variant)
                                    if variant.symbol == variant_symbol =>
                                {
                                    Some(variant)
                                }
                                _ => None,
                            })
                            .and_then(|variant| {
                                program.data_payload_field_by_symbol(variant, *symbol)
                            })?,
                        None => {
                            program
                                .data_members(data)
                                .iter()
                                .find_map(|member| match member {
                                    typed_trees::data::DataMember::Field(field)
                                        if field.symbol == *symbol =>
                                    {
                                        Some(field)
                                    }
                                    _ => None,
                                })?
                        }
                    };
                    carrier = field.type_reference;
                }
                PlaceSegment::FixedIndex { .. } => {
                    let mut reference = carrier;
                    while let typed_trees::types::TypeReferenceNode::Constrained {
                        base_type, ..
                    } = program.type_reference_table.type_reference(reference)
                    {
                        reference = *base_type;
                    }
                    let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                        program.type_reference_table.type_reference(reference)
                    else {
                        return None;
                    };
                    carrier = *element_type;
                    variant_scope = None;
                }
                _ => return None,
            }
        }
        Some(carrier)
    }

    pub(super) fn projection_places<'term>(
        term: &'term ContentConservationTerm,
        projection: &ContentProjectionPlan,
        places: &mut Vec<&'term ContentStructuralPlace>,
    ) -> bool {
        match term {
            ContentConservationTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                projection_report_fingerprint,
                subject,
            } => {
                if *domain != projection.domain
                    || *semantic_domain != projection.semantic_domain
                    || *projection_machine != projection.machine
                    || *projection_report_fingerprint != projection.report_fingerprint
                {
                    return false;
                }
                places.push(subject);
                true
            }
            ContentConservationTerm::Separate(children) => children
                .iter()
                .all(|child| projection_places(child, projection, places)),
        }
    }

    /* The actual argument expression bound to one authored formal position. */
    fn input_argument(
        program: &dyn ::validation::CheckingInput,
        invocation: &Invocation<'_>,
        parameters: &[StateParameter],
        position: usize,
    ) -> Option<ExpressionHandle> {
        let arguments = expressions(program).expression_handles(invocation.call.arguments);
        parameters.get(position)?;
        arguments.get(position).copied()
    }

    /*
        Whether the call consumes a live owned claim carrying this exact content
        family: a pre-call membership fact on a place rooted at an actual
        argument, transferred into the call. Issuance must not be credited for
        such a call — its result is at most a transfer.
    */
    fn consumes_qualified_input(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        invocation: &Invocation<'_>,
        parameters: &[StateParameter],
        projection: &ContentProjectionPlan,
    ) -> bool {
        parameters.iter().enumerate().any(|(position, parameter)| {
            qualified_input_paths(program, parameter.type_reference, projection)
                .iter()
                .any(|path| {
                    input_claim(
                        program,
                        facts.view(),
                        invocation,
                        parameters,
                        position,
                        path,
                        projection,
                    )
                    .is_some()
                })
        })
    }

    /*
        Content can qualify an owned field inside a whole linear carrier. The
        ownership frontier intentionally stops at that carrier; the declaration
        roster supplies the nested subjects, not additional ownership claims.
        Keep distinct subjects distinct here: two qualified fields cannot satisfy
        the implicit single-input forwarding rule merely because one owner holds both.
    */
    fn qualified_input_paths(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        projection: &ContentProjectionPlan,
    ) -> Vec<Vec<PlaceSegment>> {
        let mut paths = linear_claim_frontier(program, reference)
            .into_iter()
            .map(|claim| claim.path)
            .collect::<Vec<_>>();
        for row in validation::field_domains::declared_type_domain_rows(program, reference) {
            if !row.transfer
                && row.domain_symbol == projection.domain
                && row.semantic_domain == projection.semantic_domain
                && owned_claim_path(program, reference, &row.path).is_some()
                && !paths.contains(&row.path)
            {
                paths.push(row.path);
            }
        }
        paths
    }

    /*
        Find the frontier claim that owns this structural subject without crossing
        a reference. Returning the containing identity does not split its custody:
        conservation binding still refuses to count that identity twice.
    */
    fn owned_claim_path(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        path: &[PlaceSegment],
    ) -> Option<Vec<PlaceSegment>> {
        qualified_leaf_carrier(program, reference, path)?;
        let mut owners = linear_claim_frontier(program, reference)
            .into_iter()
            .filter(|claim| path.starts_with(&claim.path));
        let owner = owners.next()?;
        owners.next().is_none().then_some(owner.path)
    }

    pub(super) fn input_claim(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        invocation: &Invocation<'_>,
        parameters: &[StateParameter],
        position: usize,
        path: &[PlaceSegment],
        projection: &ContentProjectionPlan,
    ) -> Option<PermissionClaimIdentity> {
        let parameter = parameters.get(position)?;
        let argument = input_argument(program, invocation, parameters, position)?;
        let state_flow = facts
            .flow
            .control
            .state(invocation.machine, invocation.state)?;
        let call_flow = crate::flow::call_flows(facts.flow.control, state_flow.calls)
            .iter()
            .find(|call| {
                call.statement_index == invocation.statement
                    && call.call_ordinal == invocation.ordinal
                    && call.target_symbol == invocation.target_symbol(program)
            })?;
        let (operands, receiver_offset) =
            crate::flow::captured_call_operands(program, facts.flow, state_flow, call_flow)?;
        operands.get(position + receiver_offset)?;
        let values = crate::flow::captured_operand_values(
            program,
            facts.flow,
            facts.semantic,
            state_flow,
            crate::flow::CapturedOperand::recorded(
                facts.flow.control,
                call_flow
                    .operands
                    .handle_at(u32::try_from(position + receiver_offset).ok()?),
            )?,
            parameter.type_reference,
            path,
        )?;
        let [value] = values.as_slice() else {
            return None;
        };
        let places = crate::flow::captured_observation_places(program, facts.flow, value)?;
        let [place] = places.as_slice() else {
            return None;
        };
        let actual = &place.projected;
        let contexts = crate::flow::operand_contexts(
            program,
            facts.flow.contexts,
            facts.flow.control,
            facts.semantic,
            value.observation.row(),
            value.observation_type,
            call_flow.entry_constraints,
        );
        let qualified = contexts.iter().any(|context| {
            facts.semantic.context_view(facts.semantic.contexts.get(*context)).facts().any(|fact| {
                matches!(fact.payload,
                    facts::domain_membership_pattern!(domain_symbol, semantic_domain)
                    if domain_symbol == projection.domain && semantic_domain == projection.semantic_domain)
                    && matches!(fact.place, FactPlace::Place(place)
                        if facts.semantic.places.get(place).root == actual.root
                            && facts.semantic.place_segments.span_or_empty(facts.semantic.places.get(place).segments) == actual.segments)
            })
        });
        if !qualified {
            return None;
        }
        let evaluation = invocation.evaluation(program)?;
        let mut current = place_at(program, invocation.state, invocation.statement, argument)?;
        current.extend_segments(path);
        let owner_path = owned_claim_path(program, parameter.type_reference, path)?;
        let projection_path = &path[owner_path.len()..];
        let mut owner = place_at(program, invocation.state, invocation.statement, argument)?;
        owner.extend_segments(&owner_path);
        let transferred = unique_claim(
            program,
            facts,
            invocation,
            evaluation,
            PermissionEventKind::Transfer,
            owner.root,
            &owner.segments,
        )?;
        if current == *actual {
            return Some(transferred);
        }
        let received_owner_path = value
            .observation_projection
            .remaining
            .strip_suffix(projection_path)?;
        invocation
            .events
            .iter()
            .any(|event| {
                event.machine_symbol == invocation.machine
                    && event.state_symbol == invocation.state
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.evaluation == evaluation
                    && event.root == owner.root
                    && facts.flow.ownership.segments.span_or_empty(event.segments) == owner.segments
                    && event.claim_identity == transferred
                    && validation::receiving_value(program, &facts.flow.ownership.segments, event)
                        == Some((
                            invocation.statement,
                            value.observation_projection.expression,
                            received_owner_path,
                        ))
            })
            .then_some(transferred)
    }

    pub(super) fn result_claim(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        invocation: &Invocation<'_>,
        path: &[PlaceSegment],
    ) -> Option<PermissionClaimIdentity> {
        let evaluation = invocation.evaluation(program)?;
        let owner_path = owned_claim_path(program, invocation.result_type(program)?, path)?;
        let call_source = PermissionEventSource::Call {
            statement_index: invocation.statement,
            call_ordinal: invocation.ordinal,
            target_symbol: invocation.target_symbol(program),
        };
        let statement_source = PermissionEventSource::Statement {
            statement_index: invocation.statement,
        };
        let mut identity = None;
        for event in invocation.events.iter() {
            if event.machine_symbol != invocation.machine
                || event.state_symbol != invocation.state
                || event.kind != PermissionEventKind::Establish
                || event.evaluation != evaluation
                || event.access != PermissionAccess::Owned
                || !event.obligation_live
                || event.claim_identity == PermissionClaimIdentity::Unknown
                || (event.source != call_source && event.source != statement_source)
            {
                continue;
            }
            if validation::receiving_value(program, &facts.flow.ownership.segments, event)
                != Some((
                    invocation.statement,
                    invocation.expression,
                    owner_path.as_slice(),
                ))
            {
                continue;
            }
            if identity.is_some_and(|previous| previous != event.claim_identity) {
                return None;
            }
            identity = Some(event.claim_identity);
        }
        identity
    }

    fn unique_claim(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        invocation: &Invocation<'_>,
        evaluation: checked_trees::FlowPermissionEvaluation,
        kind: PermissionEventKind,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> Option<PermissionClaimIdentity> {
        let source = PermissionEventSource::Call {
            statement_index: invocation.statement,
            call_ordinal: invocation.ordinal,
            target_symbol: invocation.target_symbol(program),
        };
        let mut identity = None;
        for event in invocation.events.iter() {
            if event.machine_symbol != invocation.machine
                || event.state_symbol != invocation.state
                || event.kind != kind
                || event.evaluation != evaluation
                || event.access != PermissionAccess::Owned
                || !event.obligation_live
                || event.root != root
                || facts.flow.ownership.segments.span_or_empty(event.segments) != path
                || event.source != source
                || event.claim_identity == PermissionClaimIdentity::Unknown
            {
                continue;
            }
            if identity.is_some_and(|previous| previous != event.claim_identity) {
                return None;
            }
            identity = Some(event.claim_identity);
        }
        identity
    }
}
mod call_transformations {
    /*
        Bind a completed conservation call to its caller-local claim occurrences.

        An authored callee equation is a conditional normal-return contract. It
        becomes a replacement edge only after the ownership walk has transferred
        every exact input and established every exact result. The callee still owes
        its body proof. These edges do not grant membership, consume storage, or
        identify independent roots; they let a later transaction follow the complete
        live descendants of an account it already owned.
    */
    use super::body_conservation::owned_return_sides;
    use super::call_results::{
        Invocation, InvocationEvents, input_claim, projection_places, result_claim,
    };
    use super::content_paths::{ClaimBindings, content_segments_to_fact_path};
    use crate::flow::call_flows;
    use crate::flow::captured_call_operands;
    use checked_trees::facts::{ContentClaimTransformation, ContentClaimTransformationSource};
    use checked_trees::{FlowPermissionEvaluation, FlowPermissionEventFact};
    use language_semantics::content::ContentPlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    pub(crate) fn observe_call_completion(
        program: &dyn validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
        evaluation: FlowPermissionEvaluation,
        events: &[FlowPermissionEventFact],
    ) {
        if facts.qualifications.content.conservation_plans.is_empty() {
            return;
        }
        let Some(state_flow) = facts.flow.control.state(machine, state) else {
            return;
        };
        let invocations = call_flows(facts.flow.control, state_flow.calls)
            .iter()
            .filter(|call| {
                call.statement_index == statement && call.authored_expression == expression
            })
            .map(|call| call.call_ordinal)
            .collect::<Vec<_>>();
        for ordinal in invocations {
            let Some(validation::CallSite::Expression {
                expression: actual,
                call,
            }) = validation::find_call_site(program, machine, state, statement, ordinal)
            else {
                continue;
            };
            if actual != expression {
                continue;
            }
            let invocation = Invocation {
                machine,
                state,
                statement,
                ordinal,
                expression,
                call,
                events: InvocationEvents::Completed(events),
            };
            if invocation.evaluation(program) != Some(evaluation) {
                continue;
            }
            let selected = program.requirement_call_signature(machine, call);
            let Some(parameters) = selected
                .map(|signature| program.state_signature_parameters(signature))
                .or_else(|| validation::call_target_parameters(program, call.target_symbol))
            else {
                continue;
            };
            let plans = facts
                .qualifications
                .content
                .conservation_plans
                .iter()
                .enumerate()
                .filter(|(_, plan)| {
                    plan.owner == call.target_symbol || plan.callable == call.target_symbol
                })
                .map(|(position, plan)| (position, plan.clone()))
                .collect::<Vec<_>>();
            for (plan_index, plan) in plans {
                if facts
                    .qualifications
                    .content
                    .claim_transformations
                    .iter()
                    .any(|(_, previous)| {
                        previous.machine_symbol == machine
                            && previous.state_symbol == state
                            && previous.statement_index == statement
                            && previous.evaluation == evaluation
                            && previous.source
                                == ContentClaimTransformationSource::Call {
                                    call_ordinal: ordinal,
                                    plan_index,
                                }
                    })
                {
                    continue;
                }
                let Some((input, output)) =
                    owned_return_sides(plan.equation.left(), plan.equation.right())
                else {
                    continue;
                };
                for projection in facts.qualifications.content.plans.clone() {
                    let mut input_subjects = Vec::new();
                    let mut output_subjects = Vec::new();
                    if !projection_places(input, &projection, &mut input_subjects)
                        || !projection_places(output, &projection, &mut output_subjects)
                    {
                        continue;
                    }
                    let bound = (|| {
                        let mut inputs = ClaimBindings::default();
                        let mut outputs = ClaimBindings::default();
                        for subject in input_subjects {
                            let ContentPlaceRoot::Parameter {
                                position, symbol, ..
                            } = subject.root
                            else {
                                return None;
                            };
                            let position = usize::try_from(position).ok()?;
                            if parameters.get(position)?.symbol != symbol {
                                return None;
                            }
                            let path = content_segments_to_fact_path(&subject.segments)?;
                            let identity = input_claim(
                                program,
                                facts.view(),
                                &invocation,
                                parameters,
                                position,
                                &path,
                                &projection,
                            )?;
                            let state_flow = facts.flow.control.state(machine, state)?;
                            let call_flow = call_flows(facts.flow.control, state_flow.calls)
                                .iter()
                                .find(|candidate| {
                                    candidate.statement_index == statement
                                        && candidate.call_ordinal == ordinal
                                })?;
                            let (_, receiver_offset) = captured_call_operands(
                                program,
                                facts.flow.view(),
                                state_flow,
                                call_flow,
                            )?;
                            let observation = call_flow
                                .operands
                                .handles()
                                .nth(position + receiver_offset)?;
                            inputs.push(
                                projection.domain,
                                projection.semantic_domain,
                                projection.machine,
                                subject,
                                path,
                                identity,
                                observation,
                            )?;
                        }
                        for subject in output_subjects {
                            let path = content_segments_to_fact_path(&subject.segments)?;
                            let identity = result_claim(program, facts.view(), &invocation, &path)?;
                            outputs.push(
                                projection.domain,
                                projection.semantic_domain,
                                projection.machine,
                                subject,
                                path,
                                identity,
                                arena::Handle::invalid(),
                            )?;
                        }
                        (!inputs.is_empty() && !outputs.is_empty()).then_some((inputs, outputs))
                    })();
                    let Some((inputs, outputs)) = bound else {
                        continue;
                    };
                    let inputs = inputs.publish(facts);
                    let outputs = outputs.publish(facts);
                    facts.qualifications.content.claim_transformations.insert(
                        ContentClaimTransformation {
                            machine_symbol: machine,
                            state_symbol: state,
                            statement_index: statement,
                            expression,
                            evaluation,
                            source: ContentClaimTransformationSource::Call {
                                call_ordinal: ordinal,
                                plan_index,
                            },
                            inputs,
                            outputs,
                        },
                    );
                }
            }
        }
    }
}
mod content_paths {
    /*
        Content segments, contract places and projection plans as fact paths.
    */
    use crate::flow::expressions;
    use arena::Handle;
    use arena::HandleSpan;
    use checked_trees::facts::ContentBodyClaimBinding;
    use checked_trees::{CheckFacts, FlowOperandFact, FlowPermissionEventFact, FlowStateFact};
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::content::{
        ContentCaseSegment, ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot,
        ContentPlaceSegment, ContentPlaceVersion, ContentProjectionPlan, ContentStructuralPlace,
    };
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
        SemanticDomainId,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::state::State;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
    use validation::CanonicalPlace;

    pub(crate) fn content_segments_to_fact_path(
        segments: &[ContentPlaceSegment],
    ) -> Option<Vec<PlaceSegment>> {
        segments
            .iter()
            .map(|segment| match segment {
                ContentPlaceSegment::Case(case) if case.symbol.is_valid() => {
                    Some(PlaceSegment::Case {
                        variant: case.symbol,
                    })
                }
                ContentPlaceSegment::Field(field) if field.symbol.is_valid() => {
                    Some(PlaceSegment::Field {
                        symbol: field.symbol,
                    })
                }
                ContentPlaceSegment::FixedIndex(index) => Some(PlaceSegment::FixedIndex {
                    index: usize::try_from(*index).ok()?,
                }),
                ContentPlaceSegment::Case(_) | ContentPlaceSegment::Field(_) => None,
            })
            .collect()
    }

    /*
        Pending body-claim bindings: one row per exact projection subject, refused
        when one claim identity would stand for two separated subjects, published
        only after its subject segments are interned. The joins that decide a claim
        identity stay with their owners; this owner records rows, never authority.
    */
    #[derive(Default)]
    pub(super) struct ClaimBindings {
        rows: Vec<PendingBinding>,
    }

    struct PendingBinding {
        binding: ContentBodyClaimBinding,
        path: Vec<PlaceSegment>,
    }

    impl ClaimBindings {
        /*
            Record one projection subject. An unknown identity names no claim, and
            one claim identity cannot stand for two separated subjects.
        */
        pub(super) fn push(
            &mut self,
            domain: SymbolHandle,
            semantic_domain: SemanticDomainId,
            machine: SymbolHandle,
            subject: &ContentStructuralPlace,
            path: Vec<PlaceSegment>,
            claim_identity: PermissionClaimIdentity,
            observation: Handle<FlowOperandFact>,
        ) -> Option<()> {
            if claim_identity == PermissionClaimIdentity::Unknown
                || self
                    .rows
                    .iter()
                    .any(|pending| pending.binding.claim_identity == claim_identity)
            {
                return None;
            }
            self.rows.push(PendingBinding {
                binding: ContentBodyClaimBinding {
                    claim_identity,
                    domain,
                    semantic_domain,
                    projection_machine: machine,
                    parameter_symbol: match subject.root {
                        ContentPlaceRoot::Parameter { symbol, .. } => symbol,
                        ContentPlaceRoot::Result => SymbolHandle::invalid(),
                    },
                    segments: Default::default(),
                    observation,
                },
                path,
            });
            Some(())
        }

        /*
            Walk one conservation term, binding every projection subject through
            `claim_at`, which resolves that subject's claim from its own evidence.
            A missing or ambiguous claim refuses the whole walk.
        */
        pub(super) fn extend_term(
            &mut self,
            term: &ContentConservationTerm,
            mut claim_at: impl FnMut(&ContentStructuralPlace) -> Option<PermissionClaimIdentity>,
        ) -> Option<()> {
            let mut pending = vec![term];
            while let Some(term) = pending.pop() {
                match term {
                    ContentConservationTerm::Projection {
                        domain,
                        semantic_domain,
                        projection_machine,
                        subject,
                        ..
                    } => {
                        let claim_identity = claim_at(subject)?;
                        self.push(
                            *domain,
                            *semantic_domain,
                            *projection_machine,
                            subject,
                            content_segments_to_fact_path(&subject.segments)?,
                            claim_identity,
                            Handle::invalid(),
                        )?;
                    }
                    ContentConservationTerm::Separate(children) => {
                        pending.extend(children.iter().rev());
                    }
                }
            }
            Some(())
        }

        pub(super) fn is_empty(&self) -> bool {
            self.rows.is_empty()
        }

        /* The recorded rows, still holding their uninterred subject segments. */
        pub(super) fn rows(
            &self,
        ) -> impl Iterator<Item = (&ContentBodyClaimBinding, &[PlaceSegment])> {
            self.rows
                .iter()
                .map(|pending| (&pending.binding, pending.path.as_slice()))
        }

        /* Publish the recorded rows, interning each subject's segments once. */
        pub(super) fn publish(
            self,
            facts: &mut checked_trees::CheckFactsWalk<'_>,
        ) -> HandleSpan<ContentBodyClaimBinding> {
            let rows = self
                .rows
                .into_iter()
                .map(|mut pending| {
                    pending.binding.segments =
                        facts.flow.ownership.segments.insert_many(pending.path);
                    pending.binding
                })
                .collect::<Vec<_>>();
            facts
                .qualifications
                .content
                .body_claim_bindings
                .insert_many(rows)
        }
    }

    pub(crate) fn projection_term(
        plan: &ContentProjectionPlan,
        subject: ContentStructuralPlace,
    ) -> ContentConservationTerm {
        ContentConservationTerm::Projection {
            domain: plan.domain,
            semantic_domain: plan.semantic_domain,
            projection_machine: plan.machine,
            projection_report_fingerprint: plan.report_fingerprint,
            subject,
        }
    }

    pub(crate) fn unique_entry_claim_identity(
        facts: &CheckFacts,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
        input_path: &[PlaceSegment],
    ) -> Option<PermissionClaimIdentity> {
        entry_claim_identity(
            facts.view(),
            state_symbol,
            parameter_symbol,
            input_path,
            facts
                .flow
                .ownership
                .permissions
                .iter()
                .map(|(_, event)| event),
        )
    }

    /*
        Recording and final consumers share the exact entry-claim join. Recording
        supplies its event prefix because the final permission arena is not yet
        published; neither consumer reconstructs identity from carrier geometry.
    */
    pub(super) fn entry_claim_identity<'event>(
        facts: checked_trees::CheckFactsRef<'_>,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
        input_path: &[PlaceSegment],
        events: impl Iterator<Item = &'event FlowPermissionEventFact>,
    ) -> Option<PermissionClaimIdentity> {
        let identities = crate::shared::collections::unique_in_order(
            events
                .filter(|event| {
                    event.state_symbol == state_symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.root == PlaceRoot::Symbol(parameter_symbol)
                        && facts.flow.ownership.segments.span_or_empty(event.segments) == input_path
                        && event.claim_identity != PermissionClaimIdentity::Unknown
                })
                .map(|event| event.claim_identity),
        );
        let [identity] = identities.as_slice() else {
            return None;
        };
        Some(*identity)
    }

    pub(crate) fn applicable_projection_plans<'facts>(
        program: &dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        machine: &Machine,
        state: &State,
        type_reference: TypeReferenceHandle,
        subject: &ContentStructuralPlace,
    ) -> Vec<&'facts ContentProjectionPlan> {
        let Some(carrier) = program.unwrapped_type_reference(type_reference) else {
            return Vec::new();
        };
        let carrier_identity = program.normalized_type_identity_text(carrier);
        facts
            .qualifications
            .content
            .plans
            .iter()
            .filter(|plan| {
                plan.carrier_identity == carrier_identity
                    && (type_has_domain(program, type_reference, plan.semantic_domain)
                        || contracts_establish_domain(
                            program,
                            machine,
                            state,
                            subject,
                            plan.domain,
                            plan.semantic_domain,
                        ))
            })
            .collect()
    }

    fn type_has_domain(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        domain: SemanticDomainId,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                type_has_domain(program, *referee, domain)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| {
                        matches!(constraint, TypeConstraintNode::Domain(candidate) if candidate.semantic_id == domain)
                    })
                    || type_has_domain(program, *base_type, domain)
            }
            _ => false,
        }
    }

    fn contracts_establish_domain(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        subject: &ContentStructuralPlace,
        domain: SymbolHandle,
        semantic_domain: SemanticDomainId,
    ) -> bool {
        if program
            .domain_definition(domain)
            .is_none_or(|definition| definition.semantic_id != semantic_domain)
        {
            return false;
        }
        let mut contracts = program.state_contracts(state).iter().collect::<Vec<_>>();
        if program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol)
        {
            contracts.extend(program.machine_contracts(machine));
        }
        contracts.into_iter().any(|contract| {
            let allowed = match (&subject.root, subject.version) {
                (ContentPlaceRoot::Parameter { .. }, ContentPlaceVersion::Entry) => {
                    contract.kind == SignatureContractKind::Requires
                }
                (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
                    contract.kind == SignatureContractKind::Ensures
                }
                _ => false,
            };
            allowed
                && program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| {
                        let Some(membership) = fact.as_membership() else {
                            return false;
                        };
                        membership.domain_symbol == domain
                            && contract_place_matches(program, membership.value, subject)
                    })
        })
    }

    fn contract_place_matches(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        expected: &ContentStructuralPlace,
    ) -> bool {
        let Some((root_name, root_symbol, segments)) =
            contract_structural_place(program, expression)
        else {
            return false;
        };
        let root_matches = match &expected.root {
            ContentPlaceRoot::Result => language_core::is_reserved_result(&root_name),
            ContentPlaceRoot::Parameter { symbol, name, .. } => {
                (*symbol == root_symbol && symbol.is_valid()) || *name == root_name
            }
        };
        root_matches && content_paths_match(&expected.segments, &segments)
    }

    fn contract_structural_place(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<(String, SymbolHandle, Vec<ContentPlaceSegment>)> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let names = program.expression_table.name_path_members(path.members);
                let root = names.first()?.as_str().to_owned();
                let symbols = expressions(program).name_path_member_symbols(path.member_symbols);
                let root_symbol = symbols.first().copied().unwrap_or(path.head_symbol);
                let segments = names.iter().enumerate().skip(1).fold(
                    Vec::new(),
                    |mut segments, (index, name)| {
                        ::validation::push_structural_field(
                            program,
                            &mut segments,
                            symbols
                                .get(index)
                                .copied()
                                .unwrap_or(SymbolHandle::invalid()),
                            name.as_str(),
                        );
                        segments
                    },
                );
                Some((root, root_symbol, segments))
            }
            ExpressionNode::Member(member) => {
                let (root, root_symbol, mut segments) =
                    contract_structural_place(program, member.receiver)?;
                ::validation::push_structural_field(
                    program,
                    &mut segments,
                    member.member_symbol,
                    member.member.as_str(),
                );
                Some((root, root_symbol, segments))
            }
            ExpressionNode::Indexed(indexed) => {
                let (root, root_symbol, mut segments) =
                    contract_structural_place(program, indexed.collection)?;
                let index = program.expression_table.integer_node(indexed.index)?;
                segments.push(ContentPlaceSegment::FixedIndex(index.value_u64()?));
                Some((root, root_symbol, segments))
            }
            ExpressionNode::Borrow(inner) => contract_structural_place(program, inner.target),
            _ => None,
        }
    }

    fn content_paths_match(left: &[ContentPlaceSegment], right: &[ContentPlaceSegment]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| match (left, right) {
                    (ContentPlaceSegment::Case(left), ContentPlaceSegment::Case(right)) => {
                        left.name == right.name
                            && (!left.symbol.is_valid()
                                || !right.symbol.is_valid()
                                || left.symbol == right.symbol)
                    }
                    (
                        ContentPlaceSegment::FixedIndex(left),
                        ContentPlaceSegment::FixedIndex(right),
                    ) => left == right,
                    (ContentPlaceSegment::Field(left), ContentPlaceSegment::Field(right)) => {
                        left.name == right.name
                            && (!left.symbol.is_valid()
                                || !right.symbol.is_valid()
                                || left.symbol == right.symbol)
                    }
                    _ => false,
                })
    }

    pub(crate) fn content_path(
        program: &dyn ::validation::CheckingInput,
        path: &[PlaceSegment],
    ) -> Option<Vec<ContentPlaceSegment>> {
        path.iter()
            .map(|segment| match segment {
                PlaceSegment::Case { variant } => {
                    Some(ContentPlaceSegment::Case(ContentCaseSegment {
                        symbol: *variant,
                        name: ::validation::data_variant_name(program, *variant)?.to_owned(),
                    }))
                }
                PlaceSegment::Field { symbol } => {
                    Some(ContentPlaceSegment::Field(ContentFieldSegment {
                        symbol: *symbol,
                        name: crate::checks::multiplicity::data_field_name(program, *symbol)?
                            .to_owned(),
                    }))
                }
                PlaceSegment::FixedIndex { index } => Some(ContentPlaceSegment::FixedIndex(
                    u64::try_from(*index).expect("fixed index fits u64"),
                )),
                PlaceSegment::FixedRange { .. }
                | PlaceSegment::OpaqueRange
                | PlaceSegment::Index { .. } => None,
            })
            .collect()
    }

    /*
        Carrier fields read by a normalized content projection. Content equality
        requires these observations, not every qualification or unrelated field in
        the enclosing owner. Constants have no storage dependency.
    */
    pub(crate) fn projection_observations(
        expression: &language_semantics::content::ContentProjectionExpression,
    ) -> Option<Vec<Vec<PlaceSegment>>> {
        use language_semantics::content::{ContentProjectionExpression, ContentScalarExpression};
        fn scalar(
            expression: &ContentScalarExpression,
            paths: &mut Vec<Vec<PlaceSegment>>,
        ) -> Option<()> {
            match expression {
                ContentScalarExpression::SubjectField(fields)
                | ContentScalarExpression::RuntimeScalarEmbedding(fields) => {
                    let path = fields
                        .iter()
                        .map(|field| {
                            field.symbol.is_valid().then_some(PlaceSegment::Field {
                                symbol: field.symbol,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    crate::shared::collections::push_unique(&mut *paths, path);
                }
                ContentScalarExpression::Natural(_) => {}
                ContentScalarExpression::Successor(inner) => scalar(inner, paths)?,
                ContentScalarExpression::Arithmetic { left, right, .. } => {
                    scalar(left, paths)?;
                    scalar(right, paths)?;
                }
            }
            Some(())
        }
        let mut paths = Vec::new();
        match expression {
            ContentProjectionExpression::CountedQuantity { magnitude } => {
                scalar(magnitude, &mut paths)?
            }
            ContentProjectionExpression::IntervalSet { members } => {
                for member in members {
                    scalar(member.start(), &mut paths)?;
                    scalar(member.end(), &mut paths)?;
                }
            }
        }
        Some(paths)
    }

    /*
        Compare one content dependency with its exact entry source. The ordinary
        origin query covers preceding statements; the capture's write prefix covers
        effects already performed in the current statement. Neither alone suffices.
    */
    pub(crate) fn preserves_entry_observation(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &Machine,
        state: &FlowStateFact,
        operand: &FlowOperandFact,
        observed: CanonicalPlace,
        expected: &CanonicalPlace,
    ) -> bool {
        crate::flow::operand_entry_origin_is_stable(program, facts.flow.view(), operand, &observed)
            && crate::flow::value_origin_before_statement(
                program,
                facts.flow.view(),
                machine,
                state,
                operand.statement_index,
                observed,
                None,
                crate::flow::ValueObservation::Contents,
            )
            .is_some_and(|origin| origin == *expected)
    }
}
mod derived_memberships {
    /*
        Completed Content memberships share the exact storage predecessor law.
    */
    pub(crate) use validation::completed_storage::completed_membership;
}
mod partition_wrappers {
    /*
        Returned partition invocations and the wrappers composed from them.
    */
    use crate::checks::content::content_paths::{
        content_path, content_segments_to_fact_path, projection_observations,
    };
    use crate::checks::multiplicity::ReturnedClaim;
    use crate::checks::multiplicity::ReturnedClaimSource::Live;
    use crate::checks::multiplicity::current_returned_claims;
    use crate::flow::{SelectionCondition, SelectionConditions};
    use crate::published::places::place_at;
    use checked_trees::{
        CheckFacts, ContentPartitionCompositionFact, ContentPartitionPlaceSubstitution,
        ContentPartitionResultRewrite,
    };
    use language_semantics::content::{
        ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
        ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentStructuralPlace,
        conservation_report_fingerprint,
    };
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};

    /*
        An executed value-producing call or a selected tail transfer, joined to
        one exact outgoing occurrence. Local bindings and constructors are already
        represented by the returned claim paths and need no separate source forms.
    */
    #[derive(Debug, Clone)]
    pub(crate) struct ReturnedPartitionInvocation {
        machine_symbol: SymbolHandle,
        evaluation: checked_trees::FlowPermissionEvaluation,
        statement_index: usize,
        call_ordinal: usize,
        pub(crate) target_symbol: SymbolHandle,
        returned_at: arena::Handle<checked_trees::FlowStateOutgoingFact>,
        /*
            The exact completed value this invocation returned: the outgoing
            edge's capture, or one alternative of its value join. An alternative
            is the arm's own completion, so the returned place and its custody are
            read from that completed value rather than from the join that carries
            every arm.
        */
        completed_value: arena::Handle<checked_trees::FlowOperandFact>,
        result: InvocationResult,
    }

    #[derive(Debug, Clone)]
    enum InvocationResult {
        Captured {
            expression: ExpressionHandle,
            claims: Vec<ReturnedClaim>,
        },
        TailTransfer,
    }

    struct PartitionCompositionEvidence<'a> {
        conditions: &'a mut SelectionConditions,
        condition: SelectionCondition,
        input_claim_identities: Vec<PermissionClaimIdentity>,
        input_claim_bindings: Vec<checked_trees::ContentPartitionInputClaimBinding>,
        result_rewrites: Vec<ContentPartitionResultRewrite>,
        substitutions: Vec<ContentPartitionPlaceSubstitution>,
        observed_entry_projection: bool,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct AvailablePartitionSource {
        pub(crate) plan: ContentConservationPlan,
        pub(crate) derivation_depth: u32,
    }

    /*
        Every raw outgoing row under its exact state key, in arena order. This
        reader belongs to one immutable composition request, not a state span or a
        theorem frontier. Missing owners and duplicate rows remain candidates.
    */
    pub(super) struct OutgoingRows<'facts> {
        outgoing: &'facts arena::PagedArenaSet<checked_trees::FlowStateOutgoingFact>,
        rows: checked_trees::KeyedRows<SymbolHandle, checked_trees::FlowStateOutgoingFact>,
    }

    impl<'facts> OutgoingRows<'facts> {
        pub(super) fn new(
            outgoing: &'facts arena::PagedArenaSet<checked_trees::FlowStateOutgoingFact>,
        ) -> Self {
            Self {
                outgoing,
                rows: Default::default(),
            }
        }

        pub(super) fn for_state(
            &self,
            state: SymbolHandle,
        ) -> Vec<(
            arena::Handle<checked_trees::FlowStateOutgoingFact>,
            &'facts checked_trees::FlowStateOutgoingFact,
        )> {
            self.rows.with_rows(
                self.outgoing,
                |row| row.state_symbol,
                &state,
                |rows| {
                    rows.iter()
                        .map(|&handle| (handle, self.outgoing.get(handle)))
                        .collect()
                },
            )
        }
    }

    /*
        The flow owner records ordinary outgoing edges even when no authored
        ensures clause exists. Keep that complete roster independently from the
        invocations that happen to have usable theorem evidence.
    */
    pub(crate) fn ordinary_returns<'facts>(
        program: &dyn ::validation::CheckingInput,
        rows: impl Iterator<
            Item = (
                arena::Handle<checked_trees::FlowStateOutgoingFact>,
                &'facts checked_trees::FlowStateOutgoingFact,
            ),
        >,
        state: &typed_trees::state::State,
    ) -> Vec<arena::Handle<checked_trees::FlowStateOutgoingFact>> {
        rows.filter_map(|(handle, outgoing)| {
            if outgoing.state_symbol != state.symbol || outgoing.cannot_return_normally {
                return None;
            }
            if outgoing.transition_target.is_valid() {
                let Some(StatementNode::Transition(transition)) = program
                    .statement_table
                    .statement_at(state, outgoing.statement_index)
                else {
                    return None;
                };
                if transition.exit != typed_trees::statement::TransitionExit::Ordinary {
                    return None;
                }
            }
            Some(handle)
        })
        .collect()
    }

    pub(super) fn returned_partition_invocations(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        outgoing_rows: &OutgoingRows<'_>,
        state: &typed_trees::state::State,
        conditions: &mut SelectionConditions,
    ) -> Vec<ReturnedPartitionInvocation> {
        let Some(state_flow) = facts.flow.control.state_by_symbol(state.symbol) else {
            return Vec::new();
        };
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            return Vec::new();
        };
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let mut invocations = Vec::new();
        for returned_at in ordinary_returns(
            program,
            outgoing_rows.for_state(state.symbol).into_iter(),
            state,
        ) {
            let outgoing = facts.flow.control.outgoing.get(returned_at);
            let claims = current_returned_claims(program, facts.view(), outgoing, conditions);
            let returned_operand = facts.flow.control.operands.get(outgoing.returned_value);
            let alternatives = match returned_operand.completion {
                checked_trees::FlowOperandCompletion::Selected(alternatives) => facts
                    .flow
                    .control
                    .operand_alternatives
                    .span_or_empty(alternatives),
                _ => &[],
            };
            for call in facts.flow.control.calls.span_or_empty(state_flow.calls) {
                let (Some(call_position), Some(return_position)) = (
                    order.position(call.statement_index),
                    order.position(outgoing.statement_index).or_else(|| {
                        (outgoing.statement_index
                            == crate::flow::state_statements(program, state.statement_nodes).len())
                        .then_some(order.indices().len())
                    }),
                ) else {
                    continue;
                };
                if call_position > return_position {
                    continue;
                }
                let evaluation = crate::flow::call_permission_evaluation(
                    program,
                    machine,
                    state,
                    call.statement_index,
                    call.call_ordinal,
                );
                if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
                    continue;
                }
                let Some(site) = validation::find_call_site(
                    program,
                    state_flow.machine_symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    continue;
                };
                let mut completed_value = outgoing.returned_value;
                let result = match site {
                    validation::CallSite::Expression { expression, .. } => {
                        let mut producers =
                            crate::flow::call_flows(&facts.flow.control, state_flow.calls)
                                .iter()
                                .filter(|producer| {
                                    producer.statement_index == call.statement_index
                                        && producer.authored_expression == expression
                                });
                        if !producers
                            .next()
                            .is_some_and(|producer| std::ptr::eq(producer, call))
                            || producers.next().is_some()
                        {
                            continue;
                        }
                        let Ok(claims) = &claims else {
                            continue;
                        };
                        if let Some(alternative) = alternatives.iter().find(|alternative| {
                            let completed = facts.flow.control.operands.get(alternative.value);
                            completed.expression == expression
                                && completed.type_reference.is_valid()
                        }) {
                            completed_value = alternative.value;
                        }
                        InvocationResult::Captured {
                            expression,
                            claims: claims.clone(),
                        }
                    }
                    validation::CallSite::TransitionNamed { path, .. }
                        if call.statement_index == outgoing.statement_index
                            && outgoing.transition_target.is_valid() =>
                    {
                        let TransitionTargetNode::Named { path: selected, .. } = program
                            .statement_table
                            .transition_target(outgoing.transition_target)
                        else {
                            continue;
                        };
                        if !std::ptr::eq(path, selected) {
                            continue;
                        }
                        InvocationResult::TailTransfer
                    }
                    _ => continue,
                };
                invocations.push(ReturnedPartitionInvocation {
                    machine_symbol: machine.symbol,
                    evaluation,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_symbol: call.target_symbol,
                    returned_at,
                    completed_value,
                    result,
                });
            }
        }
        invocations
    }

    pub(crate) fn equation_contains_partition(equation: &ContentConservationEquation) -> bool {
        term_contains_partition(equation.left()) || term_contains_partition(equation.right())
    }

    fn term_contains_partition(term: &ContentConservationTerm) -> bool {
        match term {
            ContentConservationTerm::Projection { .. } => false,
            ContentConservationTerm::Separate(_) => true,
        }
    }

    pub(crate) fn instantiate_partition_wrapper(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine_symbol: SymbolHandle,
        state: &typed_trees::state::State,
        invocation: &ReturnedPartitionInvocation,
        source: &ContentConservationPlan,
        source_derivation_depth: u32,
        conditions: &mut SelectionConditions,
    ) -> Option<(ContentPartitionCompositionFact, SelectionCondition)> {
        let target_parameters =
            validation::call_target_parameters(program, invocation.target_symbol)?;
        let call_ordinal = invocation.call_ordinal;
        let condition = conditions.always();
        let mut evidence = PartitionCompositionEvidence {
            conditions,
            condition,
            input_claim_identities: Vec::new(),
            input_claim_bindings: Vec::new(),
            result_rewrites: Vec::new(),
            substitutions: Vec::new(),
            observed_entry_projection: false,
        };
        let left = instantiate_partition_term(
            program,
            facts,
            state,
            invocation,
            target_parameters,
            source.equation.left(),
            &mut evidence,
        )?;
        let right = instantiate_partition_term(
            program,
            facts,
            state,
            invocation,
            target_parameters,
            source.equation.right(),
            &mut evidence,
        )?;
        if !evidence.observed_entry_projection {
            return None;
        }
        evidence.input_claim_bindings.sort_by_key(|binding| {
            (
                format!("{:?}", binding.claim_identity),
                format!("{:?}", binding.entry_place),
            )
        });
        evidence.input_claim_bindings.dedup();
        if evidence
            .input_claim_bindings
            .iter()
            .enumerate()
            .any(|(index, binding)| {
                evidence.input_claim_bindings[index + 1..]
                    .iter()
                    .any(|later| {
                        later.claim_identity == binding.claim_identity
                            && later.entry_place != binding.entry_place
                    })
            })
        {
            return None;
        }
        evidence.input_claim_identities = evidence
            .input_claim_bindings
            .iter()
            .map(|binding| binding.claim_identity)
            .collect();
        evidence.input_claim_identities.dedup();
        evidence.result_rewrites.sort_by_key(|rewrite| {
            (
                format!("{:?}", rewrite.source),
                format!("{:?}", rewrite.target),
                format!("{:?}", rewrite.claim_identity),
            )
        });
        evidence.result_rewrites.dedup();
        evidence.substitutions.sort_by_key(|substitution| {
            (
                format!("{:?}", substitution.source),
                format!("{:?}", substitution.target),
            )
        });
        evidence.substitutions.dedup();
        let equation = ContentConservationEquation::new(left, right);
        let report_fingerprint = conservation_report_fingerprint(&source.algebra, &equation);
        let plan = ContentConservationPlan {
            owner_kind: ContentConservationOwnerKind::Machine,
            owner: machine_symbol,
            callable: state.symbol,
            algebra: source.algebra.clone(),
            equation,
            report_fingerprint,
        };
        Some((
            ContentPartitionCompositionFact {
                machine_symbol,
                state_symbol: state.symbol,
                source_callable: source.callable,
                source_report_fingerprint: source.report_fingerprint,
                source_derivation_depth,
                source_plan: source.clone(),
                statement_index: invocation.statement_index,
                call_ordinal,
                returned_at: invocation.returned_at,
                input_claim_identities: evidence.input_claim_identities,
                input_claim_bindings: evidence.input_claim_bindings,
                result_rewrites: evidence.result_rewrites,
                substitutions: evidence.substitutions,
                plan,
            },
            evidence.condition,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn instantiate_partition_term(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller_state: &typed_trees::state::State,
        invocation: &ReturnedPartitionInvocation,
        target_parameters: &[StateParameter],
        term: &ContentConservationTerm,
        evidence: &mut PartitionCompositionEvidence,
    ) -> Option<ContentConservationTerm> {
        match term {
            ContentConservationTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                projection_report_fingerprint,
                subject,
            } => {
                let projection = facts
                    .qualifications
                    .content
                    .plans
                    .iter()
                    .find(|projection| {
                        projection.domain == *domain
                            && projection.semantic_domain == *semantic_domain
                            && projection.machine == *projection_machine
                    })?;
                let observations = projection_observations(&projection.expression)?;
                Some(ContentConservationTerm::Projection {
                    domain: *domain,
                    semantic_domain: *semantic_domain,
                    projection_machine: *projection_machine,
                    projection_report_fingerprint: *projection_report_fingerprint,
                    subject: instantiate_partition_subject(
                        program,
                        facts,
                        caller_state,
                        invocation,
                        target_parameters,
                        subject,
                        &observations,
                        evidence,
                    )?,
                })
            }
            ContentConservationTerm::Separate(children) => Some(ContentConservationTerm::separate(
                children
                    .iter()
                    .map(|child| {
                        instantiate_partition_term(
                            program,
                            facts,
                            caller_state,
                            invocation,
                            target_parameters,
                            child,
                            evidence,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
        }
    }

    /*
        Whether `event` belongs to `caller_state` and was emitted at this
        invocation site (the call edge itself or its containing statement).
    */
    fn permission_event_at_invocation(
        event: &checked_trees::FlowPermissionEventFact,
        caller_state_symbol: SymbolHandle,
        invocation: &ReturnedPartitionInvocation,
        call_ordinal: usize,
    ) -> bool {
        let source_matches = matches!(
            event.source,
            PermissionEventSource::Call {
                statement_index,
                call_ordinal: event_call_ordinal,
                target_symbol,
                ..
            } if statement_index == invocation.statement_index
                && event_call_ordinal == call_ordinal
                && target_symbol == invocation.target_symbol
        ) || event.source
            == PermissionEventSource::Statement {
                statement_index: invocation.statement_index,
            };
        event.machine_symbol == invocation.machine_symbol
            && event.state_symbol == caller_state_symbol
            && event.evaluation == invocation.evaluation
            && source_matches
    }

    #[allow(clippy::too_many_arguments)]
    fn instantiate_partition_subject(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller_state: &typed_trees::state::State,
        invocation: &ReturnedPartitionInvocation,
        target_parameters: &[StateParameter],
        subject: &ContentStructuralPlace,
        observations: &[Vec<facts::PlaceSegment>],
        evidence: &mut PartitionCompositionEvidence,
    ) -> Option<ContentStructuralPlace> {
        let target = match (&subject.root, subject.version) {
            (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
                instantiate_partition_result_subject(
                    program,
                    facts,
                    caller_state,
                    invocation,
                    subject,
                    observations,
                    evidence,
                )?
            }
            (
                ContentPlaceRoot::Parameter {
                    position, symbol, ..
                },
                ContentPlaceVersion::Entry,
            ) => {
                evidence.observed_entry_projection = true;
                let parameter = target_parameters
                    .get(usize::try_from(*position).ok()?)
                    .filter(|parameter| !symbol.is_valid() || parameter.symbol == *symbol)
                    .or_else(|| {
                        symbol
                            .is_valid()
                            .then(|| {
                                target_parameters
                                    .iter()
                                    .find(|parameter| parameter.symbol == *symbol)
                            })
                            .flatten()
                    })?;
                let (claim_identity, entry_place) = partition_entry_binding(
                    program,
                    facts,
                    caller_state,
                    invocation,
                    target_parameters,
                    parameter,
                    subject,
                    observations,
                )?;
                evidence.input_claim_bindings.push(
                    checked_trees::ContentPartitionInputClaimBinding {
                        claim_identity,
                        entry_place: entry_place.clone(),
                    },
                );
                entry_place
            }
            _ => return None,
        };
        if let Some(previous) = evidence
            .substitutions
            .iter()
            .find(|substitution| substitution.source == *subject)
        {
            if previous.target != target {
                return None;
            }
        } else {
            evidence
                .substitutions
                .push(ContentPartitionPlaceSubstitution {
                    source: subject.clone(),
                    target: target.clone(),
                });
        }
        Some(target)
    }

    /*
        Bind the actual captured value to its entry observation. The capture owner
        traces detached stores and constructions; the origin owner then proves that
        the reaching observation still has its entry contents. The permission join
        independently requires the same claim at this exact consuming occurrence.
    */
    fn partition_entry_binding(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        invocation: &ReturnedPartitionInvocation,
        parameters: &[StateParameter],
        parameter: &StateParameter,
        subject: &ContentStructuralPlace,
        observations: &[Vec<facts::PlaceSegment>],
    ) -> Option<(PermissionClaimIdentity, ContentStructuralPlace)> {
        let state_flow = facts.flow.control.state_by_symbol(state.symbol)?;
        let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
        let call = crate::flow::call_flows(&facts.flow.control, state_flow.calls)
            .iter()
            .find(|call| {
                call.statement_index == invocation.statement_index
                    && call.call_ordinal == invocation.call_ordinal
                    && call.target_symbol == invocation.target_symbol
            })?;
        let position =
            typed_trees::signature::state_parameter_position(parameters, parameter.symbol)?;
        facts
            .flow
            .control
            .operands
            .span_or_empty(call.argument_operands())
            .get(position)?;
        let operand = crate::flow::CapturedOperand::recorded(
            &facts.flow.control,
            call.argument_operands()
                .handle_at(u32::try_from(position).ok()?),
        )?;
        let path = content_segments_to_fact_path(&subject.segments)?;
        let values = crate::flow::captured_operand_values(
            program,
            facts.flow.view(),
            &facts.semantic,
            state_flow,
            operand,
            parameter.type_reference,
            &path,
        )?;
        let mut binding = None;
        for value in &values {
            let actuals =
                crate::flow::captured_observation_places(program, facts.flow.view(), value)?;
            if actuals.is_empty() {
                return None;
            }
            for actual in actuals {
                let mut transfers = facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .filter(|(_, event)| {
                        permission_event_at_invocation(
                            event,
                            state.symbol,
                            invocation,
                            invocation.call_ordinal,
                        ) && event.expression == value.observation.expression
                            && event.kind == PermissionEventKind::Transfer
                            && event.access == PermissionAccess::Owned
                            && event.obligation_live
                            && event.claim_identity != PermissionClaimIdentity::Unknown
                            && event.root == actual.projected.root
                            && facts.flow.ownership.segments.span_or_empty(event.segments)
                                == actual.projected.segments
                    })
                    .map(|(_, event)| event);
                let transfer = transfers.next()?;
                if transfers.next().is_some() {
                    return None;
                }
                let identity = transfer.claim_identity;
                let mut entries = facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .filter(|(_, event)| {
                        event.machine_symbol == state_flow.machine_symbol
                            && event.state_symbol == state.symbol
                            && event.source == PermissionEventSource::StateEntry
                            && event.kind == PermissionEventKind::Establish
                            && event.access == PermissionAccess::Owned
                            && event.obligation_live
                            && event.claim_identity == identity
                    })
                    .map(|(_, event)| event);
                let entry = entries.next()?;
                if entries.next().is_some() {
                    return None;
                }
                let root = entry.root.as_symbol()?;
                let (position, entry_parameter) = program
                    .state_parameters(state)
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == root)?;
                let entry_path = facts.flow.ownership.segments.span_or_empty(entry.segments);
                for observation in observations {
                    let mut selected = path.clone();
                    selected.extend_from_slice(observation);
                    let captured = crate::flow::captured_operand_values(
                        program,
                        facts.flow.view(),
                        &facts.semantic,
                        state_flow,
                        operand,
                        parameter.type_reference,
                        &selected,
                    )?;
                    if captured.is_empty() {
                        return None;
                    }
                    let mut expected = validation::CanonicalPlace {
                        root: entry.root,
                        segments: entry_path.to_vec(),
                    };
                    expected.segments.extend_from_slice(observation);
                    for captured in &captured {
                        let producers = crate::flow::captured_value_places(
                            program,
                            facts.flow.view(),
                            captured,
                        )?;
                        if producers.is_empty() {
                            return None;
                        }
                        for producer in producers {
                            if !super::content_paths::preserves_entry_observation(
                                program,
                                facts,
                                machine,
                                state_flow,
                                captured.operand.row(),
                                producer.projected,
                                &expected,
                            ) {
                                return None;
                            }
                        }
                    }
                }
                let candidate = (
                    identity,
                    ContentStructuralPlace {
                        version: ContentPlaceVersion::Entry,
                        root: ContentPlaceRoot::Parameter {
                            position: u32::try_from(position).ok()?,
                            symbol: entry_parameter.symbol,
                            name: entry_parameter.name.as_str().to_owned(),
                        },
                        segments: content_path(program, entry_path)?,
                    },
                );
                if binding
                    .as_ref()
                    .is_some_and(|previous| *previous != candidate)
                {
                    return None;
                }
                binding = Some(candidate);
            }
        }
        binding
    }

    fn instantiate_partition_result_subject(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller_state: &typed_trees::state::State,
        invocation: &ReturnedPartitionInvocation,
        subject: &ContentStructuralPlace,
        observations: &[Vec<facts::PlaceSegment>],
        evidence: &mut PartitionCompositionEvidence,
    ) -> Option<ContentStructuralPlace> {
        let InvocationResult::Captured { expression, claims } = &invocation.result else {
            return Some(subject.clone());
        };
        let source_path = content_segments_to_fact_path(&subject.segments)?;
        let outgoing = facts.flow.control.outgoing.get(invocation.returned_at);
        let returned_operand = facts.flow.control.operands.get(outgoing.returned_value);
        let joined_result = invocation.completed_value != outgoing.returned_value;
        let mut identities = Vec::new();
        let state_permissions = facts
            .flow
            .ownership
            .with_state_permissions(caller_state.symbol, |handles| handles.to_vec());
        for handle in state_permissions {
            let event = facts.flow.ownership.permissions.get(handle);
            if !permission_event_at_invocation(
                event,
                caller_state.symbol,
                invocation,
                invocation.call_ordinal,
            ) || event.kind != PermissionEventKind::Establish
                || event.access != PermissionAccess::Owned
                || !event.obligation_live
                || event.claim_identity == PermissionClaimIdentity::Unknown
            {
                continue;
            }
            let Some((statement, produced, path)) =
                validation::receiving_value(program, &facts.flow.ownership.segments, event)
            else {
                continue;
            };
            if statement == invocation.statement_index
                && (produced == *expression
                    || (joined_result && produced == returned_operand.expression))
                && path == source_path
                && !identities.contains(&event.claim_identity)
            {
                identities.push(event.claim_identity);
            }
        }
        let [claim_identity] = identities.as_slice() else {
            return None;
        };
        let outputs = claims
            .iter()
            .filter(|claim| claim.source == Live(*claim_identity))
            .collect::<Vec<_>>();
        let returned = *outputs.first()?;
        if outputs.iter().any(|claim| claim.path != returned.path) {
            return None;
        }
        let mut correspondence = evidence.conditions.never();
        for claim in outputs {
            correspondence = evidence.conditions.any(correspondence, claim.condition);
        }
        let state_flow = facts.flow.control.state_by_symbol(caller_state.symbol)?;
        let operand = crate::flow::CapturedOperand::recorded(
            &facts.flow.control,
            invocation.completed_value,
        )?;
        for observation in observations {
            let mut selected = returned.path.clone();
            selected.extend_from_slice(observation);
            let values = crate::flow::captured_operand_values(
                program,
                facts.flow.view(),
                &facts.semantic,
                state_flow,
                operand,
                operand.type_reference,
                &selected,
            )?;
            let mut expected = place_at(
                program,
                caller_state.symbol,
                invocation.statement_index,
                *expression,
            )?;
            expected.segments.extend_from_slice(&source_path);
            expected.segments.extend_from_slice(observation);
            if values.is_empty() {
                return None;
            }
            let mut preserved = evidence.conditions.never();
            let mut invalidated = evidence.conditions.never();
            for value in &values {
                let condition = evidence
                    .conditions
                    .choices(&facts.flow.control, &value.choices)?;
                let agrees = crate::flow::captured_value_places(program, facts.flow.view(), value)
                    .is_some_and(|places| {
                        !places.is_empty() && places.iter().all(|place| place.projected == expected)
                    });
                if agrees {
                    preserved = evidence.conditions.any(preserved, condition);
                } else {
                    invalidated = evidence.conditions.any(invalidated, condition);
                }
            }
            let not_invalidated = evidence.conditions.negate(&facts.flow.control, invalidated);
            let contents = evidence.conditions.all(preserved, not_invalidated);
            correspondence = evidence.conditions.all(correspondence, contents);
        }
        evidence.condition = evidence.conditions.all(evidence.condition, correspondence);
        if !evidence
            .conditions
            .possible(&facts.flow.control, evidence.condition)
        {
            return None;
        }

        let target = ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: content_path(program, &returned.path)?,
        };
        evidence
            .result_rewrites
            .push(ContentPartitionResultRewrite {
                claim_identity: *claim_identity,
                source: subject.clone(),
                target: target.clone(),
            });
        Some(target)
    }

    #[cfg(test)]
    mod outgoing_rows_tests {
        use super::OutgoingRows;
        use checked_trees::{FlowControlFacts, FlowStateFact, FlowStateOutgoingFact};
        use symbols::SymbolHandle;

        fn compare(rows: &OutgoingRows<'_>, control: &FlowControlFacts, state: SymbolHandle) {
            let expected: Vec<_> = control
                .outgoing
                .iter()
                .filter(|(_, row)| row.state_symbol == state)
                .collect();
            let actual = rows.for_state(state);
            assert_eq!(actual.len(), expected.len());
            for ((actual_handle, actual_row), (expected_handle, expected_row)) in
                actual.iter().zip(&expected)
            {
                assert_eq!(actual_handle, expected_handle);
                assert!(std::ptr::eq(*actual_row, *expected_row));
            }
        }

        #[test]
        fn outgoing_candidates_match_whole_arena_filter_with_exact_keys_and_order() {
            let machine = SymbolHandle::from_parts(11, 2);
            let other_machine = SymbolHandle::from_parts(11, 3);
            let state = SymbolHandle::from_parts(12, 4);
            let other_generation = SymbolHandle::from_parts(12, 5);
            let orphan = SymbolHandle::from_parts(13, 6);
            let missing = SymbolHandle::from_parts(14, 7);
            let mut control = FlowControlFacts::default();
            control.states.insert(FlowStateFact {
                machine_symbol: machine,
                state_symbol: state,
                ..Default::default()
            });
            let base = FlowStateOutgoingFact {
                machine_symbol: machine,
                state_symbol: state,
                ..Default::default()
            };
            for row in [
                base.clone(),
                FlowStateOutgoingFact {
                    state_symbol: other_generation,
                    ..base.clone()
                },
                FlowStateOutgoingFact {
                    machine_symbol: other_machine,
                    ..base.clone()
                },
                FlowStateOutgoingFact {
                    state_symbol: orphan,
                    ..base.clone()
                },
                base.clone(),
                FlowStateOutgoingFact {
                    transition_target: arena::Handle::from_parts(21, 8),
                    ..base.clone()
                },
                FlowStateOutgoingFact {
                    statement_index: usize::MAX,
                    ..base.clone()
                },
                FlowStateOutgoingFact {
                    state_symbol: SymbolHandle::invalid(),
                    ..base.clone()
                },
            ] {
                control.outgoing.insert(row);
            }
            assert!(control.state_by_symbol(orphan).is_none());
            {
                let rows = OutgoingRows::new(&control.outgoing);
                for _ in 0..3 {
                    for selected in [
                        state,
                        other_generation,
                        orphan,
                        missing,
                        SymbolHandle::invalid(),
                    ] {
                        compare(&rows, &control, selected);
                    }
                }
            }
            control.outgoing.insert(base);
            let rows = OutgoingRows::new(&control.outgoing);
            compare(&rows, &control, state);
            let mut other = FlowControlFacts::default();
            for (_, row) in control.outgoing.iter() {
                other.outgoing.insert(FlowStateOutgoingFact {
                    state_symbol: missing,
                    ..row.clone()
                });
            }
            let other_rows = OutgoingRows::new(&other.outgoing);
            compare(&other_rows, &other, state);
            compare(&other_rows, &other, missing);
            compare(&rows, &control, missing);
        }
    }
}
mod retained_custody {
    /*
        Retained content needs checked transfers or declaration-level supply.

        The root check handles unique source retention; `borrowed_fields` applies
        the loan rule beneath structural shells. Bodyless multi-output authority
        additionally needs the closed partition equation checked by
        `partitioned_results`. Both structural gates share declaration traversal in
        `structural_sources`; none publishes call-result authority from annotations.
        A checked body may detach owned content from exclusive borrowed storage,
        restore that storage, and return the detached claim. Its parameter modes
        alone cannot describe that transfer: body qualification, linear disposition,
        and restoration checks own the proof. Keep declaration-only source checks
        off that route, while still publishing exact lifetime-bound loan receipts.
    */
    mod borrowed_fields {
        /*
            A record shell cannot turn a loan into owned retained content.

            The root checker owns exact lifetime-bound retention and one-to-one source
            selection. This additional rejection checks beneath structural shells on
            both sides of the signature, including explicitly linear records. A loan
            applies only to its descendants: an unrelated reference field must not
            classify an owned sibling as borrowed.

            These rows are a source-availability check, not claim correspondences or
            conservation evidence. Finding an owned source grants nothing; partition
            geometry, exact outcomes, issuance and qualification transport still owe
            their independent checks. Consequently fixed arrays need one element-type
            visit, not one row per runtime element or an invented capacity count.
            Recursive expansion and unresolved extents remain explicit analysis limits,
            never evidence of an owned source.
        */
        use super::structural_sources::{
            ContentSource, ContentSourceExpansion, domain_sources, parameter_sources,
        };
        use super::{DomainApplication, domain_name};
        use crate::shared::collections::push_unique;
        use checked_trees::{CheckFacts, RetainedBorrowCustodyFact};
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::signature::{SignatureContract, StateParameter};
        use typed_trees::types::TypeReferenceHandle;

        #[allow(clippy::too_many_arguments)]
        pub(super) fn check_structural_borrow_sources(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            label: &str,
            callable: SymbolHandle,
            parameters: &[StateParameter],
            return_type: TypeReferenceHandle,
            contracts: &[&SignatureContract],
            root_result_domains: &[DomainApplication],
            retained_borrow_custodies: &[RetainedBorrowCustodyFact],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let ContentSourceExpansion {
                sources: mut results,
                complete: result_complete,
                ..
            } = domain_sources(program, return_type);
            for domain in root_result_domains {
                let source = ContentSource {
                    carrier: return_type,
                    applications: Vec::new(),
                    domain: *domain,
                    borrowed: super::direct_reference(program, return_type).is_some(),
                    nested: false,
                    path: Vec::new(),
                };
                push_unique(&mut results, source);
            }
            results.retain(|result| {
                !result.borrowed
                    && facts
                        .qualifications
                        .content
                        .for_semantic_domain(result.domain.semantic_domain)
                        .is_some()
                    && (result.nested
                        || !retained_borrow_custodies.iter().any(|custody| {
                            custody.callable == callable
                                && custody.retained_semantic_domain == result.domain.semantic_domain
                        }))
            });
            if results.is_empty() {
                return;
            }
            if !result_complete {
                diagnostics.push(unsupported_structure(label, "result"));
                return;
            }
            let sources = parameters
                .iter()
                .map(|parameter| {
                    let ContentSourceExpansion {
                        sources, complete, ..
                    } = parameter_sources(program, parameter, contracts);
                    if !complete {
                        diagnostics.push(unsupported_structure(label, parameter.name.as_str()));
                    }
                    (parameter, sources)
                })
                .collect::<Vec<_>>();

            for result in &results {
                let Some(result_plan) = facts
                    .qualifications
                    .content
                    .for_semantic_domain(result.domain.semantic_domain)
                else {
                    continue;
                };
                let mut owned_source = false;
                let mut nested_source = false;
                let mut borrowed_parameters = Vec::new();
                for (parameter, sources) in &sources {
                    for source in sources {
                        let Some(source_plan) = facts
                            .qualifications
                            .content
                            .for_semantic_domain(source.domain.semantic_domain)
                        else {
                            continue;
                        };
                        if source_plan.algebra != result_plan.algebra {
                            continue;
                        }
                        if source.borrowed
                            || !super::owned_source_can_supply(
                                program,
                                callable,
                                source.carrier,
                                &source.applications,
                                source_plan,
                                result.carrier,
                                &result.applications,
                                result_plan,
                            )
                        {
                            nested_source |= source.nested;
                            push_unique(&mut borrowed_parameters, parameter.name.as_str());
                        } else {
                            owned_source = true;
                        }
                    }
                }
                if owned_source
                    || borrowed_parameters.is_empty()
                    || (!result.nested && !nested_source)
                {
                    continue;
                }
                let borrowed = borrowed_parameters
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` returns content-bearing custody `{}` sourced only from borrowed parameter{} {borrowed} through a structural field; retained-after-return authority requires a consumed owned input",
                    domain_name(program, result.domain.symbol),
                    if borrowed_parameters.len() == 1 { "" } else { "s" },
                )));
            }
        }

        fn unsupported_structure(label: &str, subject: &str) -> Diagnostic {
            Diagnostic::error(format!(
                "callable `{label}` cannot check structural content custody for `{subject}`: recursive expansion or an unresolved array extent requires a finite structural source analysis",
            ))
        }
    }
    mod lifecycle_backing {
        /*
            Resolve a sealed lifecycle's backing transfers against its applied types.

            Geometry equality cannot conserve authority. Each normal outcome must return
            every input backing once, with the same content qualification. This prepares
            exact paths; call checking still joins those paths to consumed/live claims.
        */
        use super::structural_sources::{SourceSegment, domain_sources};
        use facts::PlaceSegment;
        use language_semantics::content::ContentProjectionPlan;
        use symbols::SymbolHandle;
        use typed_trees::signature::StateParameter;
        use typed_trees::typed_trees::AccessExecutionDeclaration;
        use typed_trees::types::TypeReferenceHandle;

        pub(in crate::checks::content) struct BackingTransfer {
            pub parameter: usize,
            pub input: Vec<PlaceSegment>,
            pub output: Vec<PlaceSegment>,
        }

        pub(in crate::checks::content) fn lifecycle_backing_transfers(
            program: &dyn validation::CheckingInput,
            callable: SymbolHandle,
            parameters: &[StateParameter],
            return_type: TypeReferenceHandle,
            projection: &ContentProjectionPlan,
        ) -> Option<Vec<Vec<BackingTransfer>>> {
            let declaration =
                AccessExecutionDeclaration::authenticate(program, callable).ok()??;
            let outcomes = declaration.backing_outcomes();
            if outcomes.is_empty() {
                return None;
            }
            let output = domain_sources(program, return_type);
            let inputs: Vec<_> = parameters
                .iter()
                .map(|parameter| domain_sources(program, parameter.type_reference))
                .collect();
            if !output.complete || inputs.iter().any(|input| !input.complete) {
                return None;
            }
            let same_domain = |source: &&super::structural_sources::ContentSource| {
                !source.borrowed
                    && source.domain.symbol == projection.domain
                    && source.domain.semantic_domain == projection.semantic_domain
            };
            let outputs: Vec<_> = output.sources.iter().filter(same_domain).collect();
            let inputs: Vec<_> = inputs
                .iter()
                .enumerate()
                .flat_map(|(position, input)| {
                    input
                        .sources
                        .iter()
                        .filter(same_domain)
                        .map(move |source| (position, source))
                })
                .collect();
            let mut covered = Vec::new();
            let mut resolved = Vec::with_capacity(outcomes.len());
            for outcome in outcomes {
                let mut transfers = Vec::with_capacity(outcome.transfers.len());
                let mut consumed = Vec::new();
                for transfer in outcome.transfers {
                    let mut matching_inputs =
                        inputs.iter().enumerate().filter(|(_, (position, source))| {
                            *position == transfer.parameter
                                && fields_match(program, &source.path, transfer.input)
                        });
                    let (input_index, (_, input)) = matching_inputs.next()?;
                    if matching_inputs.next().is_some() || consumed.contains(&input_index) {
                        return None;
                    }
                    consumed.push(input_index);
                    let mut matching_outputs = outputs.iter().enumerate().filter(|(_, source)| {
                        let Some((SourceSegment::Case(case), fields)) = source.path.split_first()
                        else {
                            return false;
                        };
                        program.symbols.name(*case) == outcome.case
                            && fields_match(program, fields, transfer.output)
                    });
                    let (output_index, output) = matching_outputs.next()?;
                    if matching_outputs.next().is_some() || covered.contains(&output_index) {
                        return None;
                    }
                    covered.push(output_index);
                    transfers.push(BackingTransfer {
                        parameter: transfer.parameter,
                        input: fact_path(&input.path)?,
                        output: fact_path(&output.path)?,
                    });
                }
                if consumed.len() != inputs.len() {
                    return None;
                }
                resolved.push(transfers);
            }
            (covered.len() == outputs.len()).then_some(resolved)
        }

        fn fields_match(
            program: &dyn validation::CheckingInput,
            path: &[SourceSegment],
            names: &str,
        ) -> bool {
            let mut names = names.split('.').filter(|name| !name.is_empty());
            path.iter().all(|segment| {
                matches!(segment, SourceSegment::Field(symbol) if names.next() == Some(program.symbols.name(*symbol)))
            }) && names.next().is_none()
        }

        fn fact_path(path: &[SourceSegment]) -> Option<Vec<PlaceSegment>> {
            path.iter()
                .map(|segment| match segment {
                    SourceSegment::Field(symbol) => Some(PlaceSegment::Field { symbol: *symbol }),
                    SourceSegment::Case(variant) => Some(PlaceSegment::Case { variant: *variant }),
                    _ => None,
                })
                .collect()
        }
    }
    mod partitioned_results {
        /*
            A bodyless partition promise must account for every returned authority.

            The borrow-source gate only establishes that owned input is available. It
            cannot justify turning that input into several claims. Before accepting such
            a boundary promise, its authored conservation equation must
            cover the exact owned output places and retain the same qualification and
            owner projection on consumed input places. Equal algebras alone cannot
            exchange authority between domains.

            This is declaration validation, not a call receipt or an issuance rule.
            Unique identity forwarding and checked-body composition retain their existing
            checks. Calls still owe live actuals, exact substitution, result correspondence
            and provider admission; these checks do not publish qualification facts.
        */
        use super::structural_sources::{
            ContentSource, ContentSourceExpansion, SourceSegment, domain_sources, parameter_sources,
        };
        use checked_trees::CheckFacts;
        use diagnostics::Diagnostic;
        use language_semantics::content::{
            ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
            ContentProjectionPlan, ContentStructuralPlace,
        };
        use symbols::SymbolHandle;
        use typed_trees::signature::{SignatureContract, StateParameter};
        use typed_trees::types::TypeReferenceHandle;

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn check_boundary_partition_results(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            label: &str,
            callable: SymbolHandle,
            parameters: &[StateParameter],
            return_type: TypeReferenceHandle,
            contracts: &[&SignatureContract],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let ContentSourceExpansion {
                sources, complete, ..
            } = domain_sources(program, return_type);
            let outputs: Vec<_> = sources
                .iter()
                .filter(|source| {
                    !source.borrowed
                        && source.nested
                        && facts
                            .qualifications
                            .content
                            .for_semantic_domain(source.domain.semantic_domain)
                            .is_some()
                        && program.domain_definitions().iter().any(|domain| {
                            domain.symbol == source.domain.symbol
                                && !domain.establishment_routes.is_empty()
                        })
                })
                .collect();
            if outputs.is_empty() {
                return;
            }
            let inputs: Vec<_> = parameters
                .iter()
                .map(|parameter| parameter_sources(program, parameter, contracts))
                .collect();
            if complete
                && inputs.iter().all(|input| {
                    input.frontier_is_closed()
                        && input.sources.iter().all(|source| {
                            source.borrowed
                                || facts
                                    .qualifications
                                    .content
                                    .for_semantic_domain(source.domain.semantic_domain)
                                    .is_none()
                        })
                })
            {
                return;
            }
            let partition_outputs: Vec<_> = outputs
                .iter()
                .copied()
                .filter(|source| {
                    occurrence_count(source).is_none_or(|count| count > 1)
                        || outputs.iter().any(|other| {
                            source.path != other.path
                                && coexist(&source.path, &other.path)
                                && facts
                                    .qualifications
                                    .content
                                    .for_semantic_domain(source.domain.semantic_domain)
                                    .zip(
                                        facts
                                            .qualifications
                                            .content
                                            .for_semantic_domain(other.domain.semantic_domain),
                                    )
                                    .is_some_and(|(left, right)| left.algebra == right.algebra)
                        })
                })
                .collect();
            if partition_outputs.is_empty() {
                return;
            }
            let lifecycle = typed_trees::typed_trees::AccessExecutionDeclaration::authenticate(
                program, callable,
            )
            .ok()
            .flatten();
            if complete
                && partition_outputs.iter().all(|output| {
                    if lifecycle.is_some_and(|operation| program.domain_definition(output.domain.symbol)
                        .is_some_and(|domain| domain.establishment_routes.iter().any(|route|
                            matches!(route, language_semantics::DomainEstablishmentRoute::ExactMachine { machine }
                                if *machine == operation.template())))) {
                        return true;
                    }
                    facts
                        .qualifications
                        .content
                        .for_semantic_domain(output.domain.semantic_domain)
                        .is_some_and(|projection| {
                            super::lifecycle_backing::lifecycle_backing_transfers(
                                program,
                                callable,
                                parameters,
                                return_type,
                                projection,
                            )
                            .is_some()
                        })
                })
            {
                return;
            }
            if partition_outputs.iter().any(|source| {
                outputs
                    .iter()
                    .any(|other| !coexist(&source.path, &other.path))
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` has a structural content result requiring outcome-specific partition conservation; checking alternative case frontiers is not implemented",
                )));
                return;
            }
            let valid = complete
                && partition_outputs.iter().all(|output| {
                    let Some(projection) = facts
                        .qualifications
                        .content
                        .for_semantic_domain(output.domain.semantic_domain)
                    else {
                        return false;
                    };
                    facts
                        .qualifications
                        .content
                        .conservation_plans
                        .iter()
                        .any(|plan| {
                            if plan.callable != callable || plan.algebra != projection.algebra {
                                return false;
                            }
                            [
                                (plan.equation.left(), plan.equation.right()),
                                (plan.equation.right(), plan.equation.left()),
                            ]
                            .into_iter()
                            .any(|(entry, result)| {
                                let mut entry_places = Vec::new();
                                let mut result_places = Vec::new();
                                collect_places(
                                    entry,
                                    projection,
                                    ContentPlaceVersion::Entry,
                                    &mut entry_places,
                                ) && collect_places(
                                    result,
                                    projection,
                                    ContentPlaceVersion::Current,
                                    &mut result_places,
                                ) && !entry_places.is_empty()
                                    && !result_places.is_empty()
                                    && distinct_coexisting_places(&entry_places)
                                    && distinct_coexisting_places(&result_places)
                                    && entry_places.iter().all(|place| {
                                        owned_input(parameters, &inputs, projection, place)
                                    })
                                    && result_places.iter().all(|place| {
                                        place.root == ContentPlaceRoot::Result
                                            && outputs.iter().any(|declared| {
                                                declared.domain.semantic_domain
                                                    == projection.semantic_domain
                                                    && path_matches(&declared.path, &place.segments)
                                            })
                                    })
                                    && outputs
                                        .iter()
                                        .filter(|declared| {
                                            declared.domain.semantic_domain
                                                == projection.semantic_domain
                                        })
                                        .all(|declared| {
                                            occurrence_count(declared)
                                                == Some(
                                                    result_places
                                                        .iter()
                                                        .filter(|place| {
                                                            path_matches(
                                                                &declared.path,
                                                                &place.segments,
                                                            )
                                                        })
                                                        .count(),
                                                )
                                        })
                            })
                        })
                });
            if !valid {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` has a structural content result without an exact conservation equation covering its owned qualified output paths from consumed inputs of the same qualification; result annotations and equal content algebras do not establish partitioned authority",
                )));
            }
        }

        fn occurrence_count(source: &ContentSource) -> Option<usize> {
            source
                .path
                .iter()
                .try_fold(1usize, |count, segment| match segment {
                    SourceSegment::Elements(length) => count.checked_mul((*length)?),
                    _ => Some(count),
                })
        }

        fn coexist(left: &[SourceSegment], right: &[SourceSegment]) -> bool {
            for (left, right) in left.iter().zip(right) {
                if left == right {
                    continue;
                }
                return !matches!(
                    (left, right),
                    (SourceSegment::Case(_), SourceSegment::Case(_))
                );
            }
            true
        }

        fn path_matches(path: &[SourceSegment], concrete: &[ContentPlaceSegment]) -> bool {
            path.len() == concrete.len()
                && path
                    .iter()
                    .zip(concrete)
                    .all(|(declared, actual)| match (declared, actual) {
                        (SourceSegment::Field(symbol), ContentPlaceSegment::Field(field)) => {
                            *symbol == field.symbol
                        }
                        (SourceSegment::Case(symbol), ContentPlaceSegment::Case(case)) => {
                            *symbol == case.symbol
                        }
                        (
                            SourceSegment::FixedIndex(declared),
                            ContentPlaceSegment::FixedIndex(actual),
                        ) => declared == actual,
                        (
                            SourceSegment::Elements(Some(length)),
                            ContentPlaceSegment::FixedIndex(element),
                        ) => usize::try_from(*element).is_ok_and(|element| element < *length),
                        _ => false,
                    })
        }

        fn collect_places<'term>(
            term: &'term ContentConservationTerm,
            projection: &ContentProjectionPlan,
            version: ContentPlaceVersion,
            places: &mut Vec<&'term ContentStructuralPlace>,
        ) -> bool {
            match term {
                ContentConservationTerm::Projection {
                    domain,
                    semantic_domain,
                    projection_machine,
                    subject,
                    ..
                } => {
                    if *domain != projection.domain
                        || *semantic_domain != projection.semantic_domain
                        || *projection_machine != projection.machine
                        || subject.version != version
                    {
                        return false;
                    }
                    places.push(subject);
                    true
                }
                ContentConservationTerm::Separate(children) => children
                    .iter()
                    .all(|child| collect_places(child, projection, version, places)),
            }
        }

        /*
            A separation frontier cannot count one subject twice or combine mutually
            exclusive cases of the same sum occurrence. Distinct parameters, fields and
            array elements remain independent; display names never identify a subject.
        */
        fn distinct_coexisting_places(places: &[&ContentStructuralPlace]) -> bool {
            places.iter().enumerate().all(|(position, place)| {
                places[..position].iter().all(|previous| {
                    if !same_root(&previous.root, &place.root) || previous.version != place.version
                    {
                        return true;
                    }
                    let first_difference = previous
                        .segments
                        .iter()
                        .zip(&place.segments)
                        .find(|(left, right)| !same_segment(left, right));
                    match first_difference {
                        Some((ContentPlaceSegment::Case(_), ContentPlaceSegment::Case(_))) => false,
                        Some(_) => true,
                        None => previous.segments.len() != place.segments.len(),
                    }
                })
            })
        }

        fn same_root(left: &ContentPlaceRoot, right: &ContentPlaceRoot) -> bool {
            match (left, right) {
                (ContentPlaceRoot::Result, ContentPlaceRoot::Result) => true,
                (
                    ContentPlaceRoot::Parameter {
                        position: left_position,
                        symbol: left_symbol,
                        ..
                    },
                    ContentPlaceRoot::Parameter {
                        position: right_position,
                        symbol: right_symbol,
                        ..
                    },
                ) => left_position == right_position && left_symbol == right_symbol,
                _ => false,
            }
        }

        fn same_segment(left: &ContentPlaceSegment, right: &ContentPlaceSegment) -> bool {
            match (left, right) {
                (ContentPlaceSegment::Field(left), ContentPlaceSegment::Field(right)) => {
                    left.symbol == right.symbol
                }
                (ContentPlaceSegment::Case(left), ContentPlaceSegment::Case(right)) => {
                    left.symbol == right.symbol
                }
                (ContentPlaceSegment::FixedIndex(left), ContentPlaceSegment::FixedIndex(right)) => {
                    left == right
                }
                _ => false,
            }
        }

        fn owned_input(
            parameters: &[StateParameter],
            inputs: &[ContentSourceExpansion],
            projection: &ContentProjectionPlan,
            place: &ContentStructuralPlace,
        ) -> bool {
            let ContentPlaceRoot::Parameter {
                position, symbol, ..
            } = place.root
            else {
                return false;
            };
            let Ok(position) = usize::try_from(position) else {
                return false;
            };
            let Some(parameter) = parameters.get(position) else {
                return false;
            };
            let Some(input) = inputs.get(position) else {
                return false;
            };
            input.complete
                && parameter.symbol == symbol
                && input.sources.iter().any(|source| {
                    !source.borrowed
                        && source.domain.symbol == projection.domain
                        && source.domain.semantic_domain == projection.semantic_domain
                        && path_matches(&source.path, &place.segments)
                })
        }
    }
    mod structural_sources {
        /*
            Exact qualification sources beneath structural value shells.

            Borrowed-source rejection and boundary partition coverage share this walk.
            It preserves lexical generic bindings and structural paths, visits an array's
            element type once, and reports incomplete expansion explicitly. These are
            declaration subjects, not live claims or authority evidence.
        */
        use super::DomainApplication;
        use facts::PlaceRoot;
        use facts::PlaceSegment;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::data::DataMember;
        use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
        use typed_trees::types::{
            FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };
        use validation::canonical_place_from_expression;
        use validation::owned_projection_type_reference;

        #[derive(Clone)]
        pub(super) struct ContentSource {
            pub(super) domain: DomainApplication,
            pub(super) borrowed: bool,
            pub(super) nested: bool,
            pub(super) path: Vec<SourceSegment>,
            pub(super) carrier: TypeReferenceHandle,
            pub(super) applications: Vec<TypeReferenceHandle>,
        }

        /*
            Carrier operands explain a subject; they do not duplicate a claim already
            named by the same domain and structural path in a requires clause.
        */
        impl PartialEq for ContentSource {
            fn eq(&self, other: &Self) -> bool {
                self.domain == other.domain
                    && self.borrowed == other.borrowed
                    && self.nested == other.nested
                    && self.path == other.path
            }
        }
        impl Eq for ContentSource {}

        /*
            Known declaration sources and the independent completeness of their frontier.
            Existing source checks can inspect the known paths of an open generic carrier;
            absence of those paths must never prove that its owned content is empty.
        */
        pub(super) struct ContentSourceExpansion {
            pub(super) sources: Vec<ContentSource>,
            pub(super) complete: bool,
            has_open_owned_carrier: bool,
        }

        impl ContentSourceExpansion {
            pub(super) fn frontier_is_closed(&self) -> bool {
                self.complete && !self.has_open_owned_carrier
            }
        }

        /*
            Array coverage stays symbolic: source discovery visits an element type once,
            even when its declared extent is much larger than the authored proof.
        */
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(super) enum SourceSegment {
            Field(SymbolHandle),
            Case(SymbolHandle),
            FixedIndex(u64),
            Elements(Option<usize>),
        }

        pub(super) fn parameter_sources(
            program: &dyn ::validation::CheckingInput,
            parameter: &StateParameter,
            contracts: &[&SignatureContract],
        ) -> ContentSourceExpansion {
            let ContentSourceExpansion {
                mut sources,
                complete,
                mut has_open_owned_carrier,
            } = domain_sources(program, parameter.type_reference);
            for contract in contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    if let Some(membership) = fact.as_membership()
                        && membership.domain_symbol.is_valid()
                        && membership.semantic_domain.is_valid()
                        && let Some(place) =
                            canonical_place_from_expression(program, membership.value)
                        && place.root == PlaceRoot::Symbol(parameter.symbol)
                    {
                        let path = place
                            .segments
                            .iter()
                            .map(|segment| match segment {
                                PlaceSegment::Field { symbol } => {
                                    Some(SourceSegment::Field(*symbol))
                                }
                                PlaceSegment::Case { variant } => {
                                    Some(SourceSegment::Case(*variant))
                                }
                                PlaceSegment::FixedIndex { index } => {
                                    u64::try_from(*index).ok().map(SourceSegment::FixedIndex)
                                }
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>();
                        let Some(path) = path else {
                            has_open_owned_carrier = true;
                            continue;
                        };
                        let selected = owned_projection_type_reference(
                            program,
                            parameter.type_reference,
                            &place.segments,
                        );
                        let carrier = validation::source_type_application_from_segments(
                            program,
                            parameter.type_reference,
                            &place.segments,
                        );
                        let source = ContentSource {
                            carrier: carrier
                                .as_ref()
                                .map_or_else(TypeReferenceHandle::invalid, |carrier| carrier.0),
                            applications: carrier.map_or_else(Vec::new, |carrier| carrier.1),
                            domain: DomainApplication {
                                symbol: membership.domain_symbol,
                                semantic_domain: membership.semantic_domain,
                            },
                            borrowed: super::direct_reference(program, parameter.type_reference)
                                .is_some()
                                || selected
                                    .is_none_or(|selected| reference_shell(program, selected, &[])),
                            nested: !path.is_empty(),
                            path,
                        };
                        crate::shared::collections::push_unique(&mut sources, source);
                    }
                }
            }
            ContentSourceExpansion {
                sources,
                complete,
                has_open_owned_carrier,
            }
        }

        pub(super) fn domain_sources(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> ContentSourceExpansion {
            let mut sources = Vec::new();
            let mut has_open_owned_carrier = false;
            let complete = append_sources(
                program,
                reference,
                &[],
                false,
                false,
                &[],
                &mut Vec::new(),
                &mut sources,
                &mut has_open_owned_carrier,
            );
            ContentSourceExpansion {
                sources,
                complete,
                has_open_owned_carrier,
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn append_sources(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
            applications: &[TypeReferenceHandle],
            borrowed: bool,
            nested: bool,
            path: &[SourceSegment],
            visiting: &mut Vec<(SymbolHandle, usize)>,
            sources: &mut Vec<ContentSource>,
            has_open_owned_carrier: &mut bool,
        ) -> bool {
            let Some((reference, frames)) =
                program.project_enclosing_data_type_reference(reference, applications)
            else {
                return false;
            };
            let applications = &applications[..frames];
            if !reference.is_valid() {
                return true;
            }
            let borrowed = borrowed || reference_shell(program, reference, applications);
            let definition_symbol = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        if let TypeConstraintNode::Domain(domain) = constraint {
                            let source = ContentSource {
                                carrier: reference,
                                applications: applications.to_vec(),
                                domain: DomainApplication {
                                    symbol: domain.symbol,
                                    semantic_domain: domain.semantic_id,
                                },
                                borrowed,
                                nested,
                                path: path.to_vec(),
                            };
                            if domain.semantic_id.is_valid() && !sources.contains(&source) {
                                sources.push(source);
                            }
                        }
                    }
                    return append_sources(
                        program,
                        *base_type,
                        applications,
                        borrowed,
                        nested,
                        path,
                        visiting,
                        sources,
                        has_open_owned_carrier,
                    );
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    return append_sources(
                        program,
                        *referee,
                        applications,
                        true,
                        nested,
                        path,
                        visiting,
                        sources,
                        has_open_owned_carrier,
                    );
                }
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Literal(0),
                    ..
                } => return true,
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(_),
                }
                | TypeReferenceNode::Slice { element_type } => {
                    let length = match program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::FixedArray {
                            length: FixedArrayLength::Literal(length),
                            ..
                        } => Some(*length),
                        _ => None,
                    };
                    let mut element_path = path.to_vec();
                    element_path.push(SourceSegment::Elements(length));
                    return append_sources(
                        program,
                        *element_type,
                        applications,
                        borrowed,
                        true,
                        &element_path,
                        visiting,
                        sources,
                        has_open_owned_carrier,
                    );
                }
                TypeReferenceNode::FixedArray { .. } => return false,
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => return true,
            };
            let Some(definition) = program
                .data_application_definition(reference)
                .or_else(|| program.closed_data_definition(reference))
                .or_else(|| program.data_definition_by_symbol(definition_symbol))
            else {
                if !borrowed
                    && definition_symbol.is_valid()
                    && program.symbols.get(definition_symbol).kind == SymbolKind::TypeParameter
                {
                    *has_open_owned_carrier = true;
                }
                return true;
            };
            let size = type_size(program, reference, applications);
            if visiting
                .iter()
                .any(|(owner, ancestor_size)| *owner == definition_symbol && size >= *ancestor_size)
            {
                return false;
            }
            let mut instantiated = applications.to_vec();
            if matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Generic { .. }
            ) {
                instantiated.push(reference);
            }
            visiting.push((definition_symbol, size));
            let mut complete = true;
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) => {
                        let mut field_path = path.to_vec();
                        field_path.push(SourceSegment::Field(field.symbol));
                        complete &= append_sources(
                            program,
                            field.type_reference,
                            &instantiated,
                            borrowed,
                            true,
                            &field_path,
                            visiting,
                            sources,
                            has_open_owned_carrier,
                        );
                    }
                    DataMember::Variant(variant) => {
                        for field in program.data_payload_fields(variant) {
                            let mut field_path = path.to_vec();
                            field_path.push(SourceSegment::Case(variant.symbol));
                            field_path.push(SourceSegment::Field(field.symbol));
                            complete &= append_sources(
                                program,
                                field.type_reference,
                                &instantiated,
                                borrowed,
                                true,
                                &field_path,
                                visiting,
                                sources,
                                has_open_owned_carrier,
                            );
                        }
                    }
                }
            }
            visiting.pop();
            complete
        }

        fn reference_shell(
            program: &dyn ::validation::CheckingInput,
            mut reference: TypeReferenceHandle,
            mut applications: &[TypeReferenceHandle],
        ) -> bool {
            loop {
                let Some((projected, frames)) =
                    program.project_enclosing_data_type_reference(reference, applications)
                else {
                    return false;
                };
                reference = projected;
                applications = &applications[..frames];
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference { .. } => return true,
                    _ => return false,
                }
            }
        }

        fn type_size(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
            applications: &[TypeReferenceHandle],
        ) -> usize {
            let Some((reference, frames)) =
                program.project_enclosing_data_type_reference(reference, applications)
            else {
                return usize::MAX;
            };
            let applications = &applications[..frames];
            let children = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => {
                    type_size(program, *base_type, applications)
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    type_size(program, *referee, applications)
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    type_size(program, *element_type, applications)
                }
                TypeReferenceNode::Generic { arguments, .. } => program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .fold(0usize, |size, argument| {
                        size.saturating_add(type_size(program, *argument, applications))
                    }),
                _ => 0,
            };
            children.saturating_add(1)
        }
    }

    pub(super) use lifecycle_backing::lifecycle_backing_transfers;
    pub(crate) use partitioned_results::check_boundary_partition_results;

    use crate::flow::expressions;
    use checked_trees::{CheckFacts, RetainedBorrowCustodyFact};
    use diagnostics::Diagnostic;
    use language_semantics::content::{
        ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionPlan,
        ContentStructuralPlace,
    };
    use language_semantics::{Multiplicity, ReferenceAccess, SemanticDomainId};
    use symbols::SymbolHandle;
    use typed_trees::name::Identifier;
    use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn check_callable(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        label: &str,
        callable: SymbolHandle,
        callable_lifetime_parameters: &[Identifier],
        parameters: &[StateParameter],
        return_type: TypeReferenceHandle,
        contracts: &[&SignatureContract],
        diagnostics: &mut Vec<Diagnostic>,
        retained_borrow_custodies: &mut Vec<RetainedBorrowCustodyFact>,
    ) {
        let checked_body = validation::call_target_has_checked_body_contract(program, callable);
        let mut result_domains = Vec::new();
        append_type_domains(program, return_type, &mut result_domains);
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if expressions(program).names_reserved_result(membership.value)
                    && let Some(domain) =
                        nominal_domain_application(program, membership.domain_symbol)
                {
                    push_unique_domain_application(&mut result_domains, domain);
                }
            }
        }

        let content_result_domain_count = result_domains
            .iter()
            .filter(|domain| {
                facts
                    .qualifications
                    .content
                    .for_semantic_domain(domain.semantic_domain)
                    .is_some()
            })
            .count();

        for result_domain in &result_domains {
            let Some(result_plan) = facts
                .qualifications
                .content
                .for_semantic_domain(result_domain.semantic_domain)
            else {
                continue;
            };
            let mut borrowed_sources = Vec::new();
            let mut owned_sources = Vec::new();

            for (parameter_position, parameter) in parameters.iter().enumerate() {
                let mut parameter_domains = Vec::new();
                append_type_domains(program, parameter.type_reference, &mut parameter_domains);
                for contract in contracts
                    .iter()
                    .filter(|contract| contract.kind == SignatureContractKind::Requires)
                {
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        let Some(membership) = fact.as_membership() else {
                            continue;
                        };
                        if expressions(program).names_parameter(membership.value, parameter)
                            && let Some(domain) =
                                nominal_domain_application(program, membership.domain_symbol)
                        {
                            push_unique_domain_application(&mut parameter_domains, domain);
                        }
                    }
                }

                let mut compatible_plans = Vec::new();
                for domain in parameter_domains {
                    let Some(input_plan) = facts
                        .qualifications
                        .content
                        .for_semantic_domain(domain.semantic_domain)
                    else {
                        continue;
                    };
                    if input_plan.algebra == result_plan.algebra
                        && !compatible_plans.contains(input_plan)
                    {
                        compatible_plans.push(input_plan.clone());
                    }
                }
                if compatible_plans.is_empty() {
                    continue;
                }

                let owned_source = program.type_multiplicity(parameter.type_reference)
                    == Multiplicity::Linear
                    && compatible_plans.iter().any(|plan| {
                        owned_source_can_supply(
                            program,
                            callable,
                            parameter.type_reference,
                            &[],
                            plan,
                            return_type,
                            &[],
                            result_plan,
                        )
                    });
                if owned_source {
                    owned_sources.push(parameter.name.as_str());
                } else if parameter.type_reference.is_valid()
                    && crate::borrow::view_link::returns_borrow(program, parameter.type_reference)
                {
                    borrowed_sources.push(BorrowedContentSource {
                        parameter_position,
                        parameter,
                        compatible_plans,
                    });
                }
            }

            if owned_sources.len() == 1 || (owned_sources.is_empty() && borrowed_sources.is_empty())
            {
                continue;
            }

            if let Some(selected) = authored_retention_source(
                facts,
                callable,
                result_domain.semantic_domain,
                result_plan,
                parameters,
            ) && owned_sources.contains(&selected)
            {
                continue;
            }

            if checked_body {
                continue;
            }

            if owned_sources.is_empty() && !borrowed_sources.is_empty() {
                match lifetime_bound_borrow_custody(
                    program,
                    label,
                    callable,
                    callable_lifetime_parameters,
                    return_type,
                    result_domain.semantic_domain,
                    result_plan,
                    content_result_domain_count,
                    &borrowed_sources,
                ) {
                    Ok(fact) => {
                        retained_borrow_custodies.push(fact);
                        continue;
                    }
                    Err(BorrowCustodyFailure::Directed(reason)) => {
                        diagnostics.push(Diagnostic::error(format!(
                            "callable `{label}` cannot retain borrowed content-bearing custody: {reason}",
                        )));
                        continue;
                    }
                    Err(BorrowCustodyFailure::RequiresOwnedCustody) => {}
                }
            }

            let result_name = domain_name(program, result_domain.symbol);
            if owned_sources.len() > 1 {
                let owned = owned_sources
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` returns content-bearing custody `{result_name}` with ambiguous compatible consumed inputs {owned}; retained-after-return authority requires one unambiguous owned source or an exact postcondition correspondence",
                )));
                continue;
            }
            let borrowed = borrowed_sources
                .iter()
                .map(|source| format!("`{}`", source.parameter.name))
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(Diagnostic::error(format!(
                "callable `{label}` returns content-bearing custody `{result_name}` sourced only from borrowed parameter{} {borrowed}; retained-after-return authority requires a consumed owned input",
                if borrowed_sources.len() == 1 { "" } else { "s" },
            )));
        }

        if checked_body {
            return;
        }
        borrowed_fields::check_structural_borrow_sources(
            program,
            facts,
            label,
            callable,
            parameters,
            return_type,
            contracts,
            &result_domains,
            retained_borrow_custodies,
            diagnostics,
        );
    }

    /*
        Root claims and structural subclaims use the same ownership judgment.
        Equal algebra does not turn an owned loan carrier into owned backing.
    */
    #[allow(clippy::too_many_arguments)]
    fn owned_source_can_supply(
        program: &dyn ::validation::CheckingInput,
        callable: SymbolHandle,
        source: TypeReferenceHandle,
        source_applications: &[TypeReferenceHandle],
        source_plan: &ContentProjectionPlan,
        result: TypeReferenceHandle,
        result_applications: &[TypeReferenceHandle],
        result_plan: &ContentProjectionPlan,
    ) -> bool {
        use validation::borrow_contracts::OwnedCarrierLoanRelation;
        match validation::borrow_contracts::owned_carrier_loan_relation(
            program,
            callable,
            source,
            source_applications,
            result,
            result_applications,
        ) {
            OwnedCarrierLoanRelation::ViewFree => true,
            OwnedCarrierLoanRelation::Preserved => source_plan == result_plan,
            OwnedCarrierLoanRelation::Unproven => false,
        }
    }

    struct BorrowedContentSource<'a> {
        parameter_position: usize,
        parameter: &'a StateParameter,
        compatible_plans: Vec<ContentProjectionPlan>,
    }

    enum BorrowCustodyFailure {
        RequiresOwnedCustody,
        Directed(String),
    }

    #[allow(clippy::too_many_arguments)]
    fn lifetime_bound_borrow_custody(
        program: &dyn ::validation::CheckingInput,
        label: &str,
        callable: SymbolHandle,
        callable_lifetime_parameters: &[Identifier],
        return_type: TypeReferenceHandle,
        retained_semantic_domain: SemanticDomainId,
        result_plan: &ContentProjectionPlan,
        content_result_domain_count: usize,
        borrowed_sources: &[BorrowedContentSource<'_>],
    ) -> Result<RetainedBorrowCustodyFact, BorrowCustodyFailure> {
        let Some((result_data, result_lifetime)) = lifetime_bound_result(program, return_type)
        else {
            return Err(BorrowCustodyFailure::RequiresOwnedCustody);
        };
        if content_result_domain_count != 1 {
            return Err(BorrowCustodyFailure::Directed(format!(
                "the bounded retained-loan rung requires exactly one content-bearing result domain, but `{label}` has {content_result_domain_count}",
            )));
        }
        let Some(callable_lifetime_parameter_ordinal) = callable_lifetime_parameters
            .iter()
            .position(|candidate| candidate == &result_lifetime)
        else {
            return Err(BorrowCustodyFailure::Directed(format!(
                "result lifetime `'{}'` does not name an explicit lifetime parameter of `{label}`",
                result_lifetime.as_str(),
            )));
        };
        if borrowed_sources.len() != 1 {
            let names = borrowed_sources
                .iter()
                .map(|source| format!("`{}`", source.parameter.name))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(BorrowCustodyFailure::Directed(format!(
                "the result lifetime has ambiguous compatible borrowed inputs {names}; exactly one whole direct source is required",
            )));
        }
        let source = &borrowed_sources[0];

        let Some((access, source_lifetime)) =
            direct_reference(program, source.parameter.type_reference)
        else {
            return Err(BorrowCustodyFailure::Directed(format!(
                "borrowed parameter `{}` carries its loan through a nested or indirect shape; only a whole direct reference is admitted",
                source.parameter.name,
            )));
        };
        if !matches!(access, ReferenceAccess::Shared | ReferenceAccess::Mutable) {
            return Err(BorrowCustodyFailure::Directed(format!(
                "borrowed parameter `{}` uses {access:?} access; retained lifetime-bound custody requires readable shared or mutable access",
                source.parameter.name,
            )));
        }
        let Some(source_lifetime) = source_lifetime else {
            return Err(BorrowCustodyFailure::Directed(format!(
                "borrowed parameter `{}` elides its lifetime; retained custody requires the same explicit callable lifetime on source and result",
                source.parameter.name,
            )));
        };
        if source_lifetime != result_lifetime {
            return Err(BorrowCustodyFailure::Directed(format!(
                "borrowed parameter `{}` uses lifetime `'{}'`, which does not match result lifetime `'{}'`",
                source.parameter.name,
                source_lifetime.as_str(),
                result_lifetime.as_str(),
            )));
        }
        let [source_projection] = source.compatible_plans.as_slice() else {
            return Err(BorrowCustodyFailure::Directed(format!(
                "borrowed parameter `{}` has {} compatible content projections; exactly one exact projection is required",
                source.parameter.name,
                source.compatible_plans.len(),
            )));
        };

        Ok(RetainedBorrowCustodyFact {
            callable,
            source: ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: ContentPlaceRoot::Parameter {
                    position: u32::try_from(source.parameter_position)
                        .expect("state parameter position fits in u32"),
                    symbol: source.parameter.symbol,
                    name: source.parameter.name.as_str().to_owned(),
                },
                segments: Vec::new(),
            },
            result: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root: ContentPlaceRoot::Result,
                segments: Vec::new(),
            },
            access,
            lifetime: result_lifetime,
            callable_lifetime_parameter_ordinal: u32::try_from(callable_lifetime_parameter_ordinal)
                .expect("callable lifetime parameter ordinal fits in u32"),
            result_data,
            result_lifetime_argument_ordinal: 0,
            retained_semantic_domain,
            source_projection: source_projection.clone(),
            result_projection: result_plan.clone(),
        })
    }

    fn lifetime_bound_result(
        program: &dyn ::validation::CheckingInput,
        return_type: TypeReferenceHandle,
    ) -> Option<(SymbolHandle, Identifier)> {
        let return_type = strip_constraints(program, return_type);
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = program.type_reference_table.type_reference(return_type)
        else {
            return None;
        };
        let definition = program.data_definition_by_symbol(*base_symbol)?;
        if program.type_multiplicity(return_type) != Multiplicity::Linear
            || definition.lifetime_parameters.len() != 1
            || lifetime_arguments.len() != 1
            || !program
                .type_reference_table
                .type_reference_handles(*arguments)
                .is_empty()
        {
            return None;
        }
        Some((*base_symbol, lifetime_arguments[0].clone()))
    }

    fn direct_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<(ReferenceAccess, Option<Identifier>)> {
        let type_reference = strip_constraints(program, type_reference);
        let TypeReferenceNode::Reference {
            access, lifetime, ..
        } = program.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        Some((*access, lifetime.clone()))
    }

    fn strip_constraints(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        ::validation::peel_declared_constraints(program, type_reference)
    }

    /*
        Return the one parameter selected by an exact authored custody equality.
        The equation must relate the whole current result projection directly to
        the whole entry projection of one parameter in the same algebra. Partition
        terms and structural subplaces describe transformations rather than the
        one-to-one correspondence needed to resolve retained ownership.
    */
    fn authored_retention_source<'a>(
        facts: &CheckFacts,
        callable: SymbolHandle,
        result_domain: SemanticDomainId,
        result_plan: &ContentProjectionPlan,
        parameters: &'a [StateParameter],
    ) -> Option<&'a str> {
        facts
            .qualifications
            .content
            .conservation_plans
            .iter()
            .filter(|plan| plan.callable == callable && plan.algebra == result_plan.algebra)
            .find_map(|plan| {
                let left = exact_projection_subject(plan.equation.left())?;
                let right = exact_projection_subject(plan.equation.right())?;
                let parameter = match (&left.root, left.version, &right.root, right.version) {
                    (
                        ContentPlaceRoot::Result,
                        ContentPlaceVersion::Current,
                        ContentPlaceRoot::Parameter { symbol, .. },
                        ContentPlaceVersion::Entry,
                    ) if projection_domain(plan.equation.left()) == Some(result_domain) => *symbol,
                    (
                        ContentPlaceRoot::Parameter { symbol, .. },
                        ContentPlaceVersion::Entry,
                        ContentPlaceRoot::Result,
                        ContentPlaceVersion::Current,
                    ) if projection_domain(plan.equation.right()) == Some(result_domain) => *symbol,
                    _ => return None,
                };
                parameters
                    .iter()
                    .find(|candidate| candidate.symbol == parameter)
                    .map(|candidate| candidate.name.as_str())
            })
    }

    fn exact_projection_subject(term: &ContentConservationTerm) -> Option<&ContentStructuralPlace> {
        let ContentConservationTerm::Projection { subject, .. } = term else {
            return None;
        };
        subject.segments.is_empty().then_some(subject)
    }

    fn projection_domain(term: &ContentConservationTerm) -> Option<SemanticDomainId> {
        let ContentConservationTerm::Projection {
            semantic_domain, ..
        } = term
        else {
            return None;
        };
        Some(*semantic_domain)
    }

    fn append_type_domains(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        domains: &mut Vec<DomainApplication>,
    ) {
        if !type_reference.is_valid() {
            return;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                append_type_domains(program, *referee, domains);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                append_type_domains(program, *base_type, domains);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        push_unique_domain_application(
                            domains,
                            DomainApplication {
                                symbol: domain.symbol,
                                semantic_domain: domain.semantic_id,
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct DomainApplication {
        symbol: SymbolHandle,
        semantic_domain: SemanticDomainId,
    }

    fn nominal_domain_application(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<DomainApplication> {
        let definition = program.domain_definition(symbol)?;
        definition
            .semantic_id
            .is_valid()
            .then_some(DomainApplication {
                symbol,
                semantic_domain: definition.semantic_id,
            })
    }

    fn push_unique_domain_application(
        domains: &mut Vec<DomainApplication>,
        domain: DomainApplication,
    ) {
        if domain.symbol.is_valid()
            && domain.semantic_domain.is_valid()
            && !domains.contains(&domain)
        {
            domains.push(domain);
        }
    }

    fn domain_name(program: &dyn ::validation::CheckingInput, symbol: SymbolHandle) -> &str {
        program
            .domain_definition(symbol)
            .map(|domain| domain.name.as_str())
            .unwrap_or("<unknown domain>")
    }
}

pub(crate) use body_conservation::{
    ConservationCandidate, ConservationSites, conservation_sites, observe_body_completion,
};
pub(crate) use call_transformations::observe_call_completion;
pub(crate) use content_paths::{content_path, projection_observations};
pub(crate) use derived_memberships::completed_membership;

use crate::checks::content::content_paths::{
    applicable_projection_plans, projection_term, unique_entry_claim_identity,
};
use crate::checks::content::partition_wrappers::{
    AvailablePartitionSource, equation_contains_partition, instantiate_partition_wrapper,
    returned_partition_invocations,
};
use crate::checks::content::retained_custody::{check_boundary_partition_results, check_callable};
use checked_trees::{
    CheckFacts, ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    FlowClaimOutcomeSource,
};
use diagnostics::Diagnostic;
use language_semantics::content::{
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentPlaceRoot, ContentPlaceVersion, ContentStructuralPlace, conservation_report_fingerprint,
    content_conservation_plan_bytes,
};

/*
    Derive the content equality attached to every exact input-relative claim
    outcome. These are deliberately individual rewrite rows: distinct linear
    claims do not imply that their projected content is disjoint, so this pass
    never manufactures a `separate(...)` term. The later frontier theorem may
    compose rows only when it also has the required partition evidence.
*/
pub(crate) fn infer_identity_preserving_reshuffles(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) {
    let outcomes = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .flat_map(|(_, map)| {
            facts
                .flow
                .ownership
                .claim_outcome_entries
                .span_or_empty(map.entries)
                .iter()
                .map(|entry| {
                    (
                        map.machine_symbol,
                        map.state_symbol,
                        entry.output_segments,
                        entry.source,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut reshuffles = Vec::new();

    for (machine_symbol, state_symbol, output_segments, source) in outcomes {
        let FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments: input_segments,
        } = source
        else {
            continue;
        };
        let Some(machine) = program.machine_by_symbol(machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_by_symbol(state_symbol) else {
            continue;
        };
        let Some((parameter_position, parameter)) = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == parameter_symbol)
        else {
            continue;
        };
        let input_path = facts.flow.ownership.segments.span_or_empty(input_segments);
        let output_path = facts.flow.ownership.segments.span_or_empty(output_segments);
        let Some(input_claim) =
            super::multiplicity::linear_claim_frontier(program, parameter.type_reference)
                .into_iter()
                .find(|claim| claim.path == input_path)
        else {
            continue;
        };
        let Some(output_claim) =
            super::multiplicity::linear_claim_frontier(program, state.return_type)
                .into_iter()
                .find(|claim| claim.path == output_path)
        else {
            continue;
        };
        let Some(input_content_path) = content_path(program, input_path) else {
            continue;
        };
        let Some(output_content_path) = content_path(program, output_path) else {
            continue;
        };
        let input_subject = ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: u32::try_from(parameter_position)
                    .expect("state parameter position fits in u32"),
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
            },
            segments: input_content_path,
        };
        let output_subject = ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: output_content_path,
        };
        let input_plans = applicable_projection_plans(
            program,
            facts,
            machine,
            state,
            input_claim.type_reference,
            &input_subject,
        );
        let output_plans = applicable_projection_plans(
            program,
            facts,
            machine,
            state,
            output_claim.type_reference,
            &output_subject,
        );
        let Some(claim_identity) =
            unique_entry_claim_identity(facts, state_symbol, parameter_symbol, input_path)
        else {
            continue;
        };

        for input_plan in input_plans {
            for output_plan in output_plans.iter().copied().filter(|output_plan| {
                output_plan.semantic_domain == input_plan.semantic_domain
                    && output_plan.report_fingerprint == input_plan.report_fingerprint
                    && output_plan.algebra == input_plan.algebra
            }) {
                let left = projection_term(input_plan, input_subject.clone());
                let right = projection_term(output_plan, output_subject.clone());
                let equation = ContentConservationEquation::new(left, right);
                let report_fingerprint =
                    conservation_report_fingerprint(&input_plan.algebra, &equation);
                reshuffles.push(ContentIdentityReshuffleFact {
                    machine_symbol,
                    state_symbol,
                    claim_identity,
                    input_parameter_symbol: parameter_symbol,
                    input_segments,
                    output_segments,
                    plan: ContentConservationPlan {
                        owner_kind: ContentConservationOwnerKind::Machine,
                        owner: machine_symbol,
                        callable: state_symbol,
                        algebra: input_plan.algebra.clone(),
                        equation,
                        report_fingerprint,
                    },
                });
            }
        }
    }

    reshuffles.sort_by_key(|fact| {
        (
            fact.machine_symbol.arena_index(),
            fact.state_symbol.arena_index(),
            content_conservation_plan_bytes(&fact.plan),
        )
    });
    reshuffles.dedup();
    facts.qualifications.content.identity_reshuffles = reshuffles;
}

/*
    Compose proved partition theorems at completed invocations. The shared
    entailment owner decides which authored clauses are proved; requirements
    and admitted boundary declarations supply the other roots. Merely writing
    a guarantee cannot make an unproved cycle authorize itself.

    The capture owner relates actual entry observations and returned contents,
    while the permission ledger separately relates exact claims. Derived rows
    bind one outgoing occurrence. Only a theorem established on every ordinary
    outgoing edge becomes a callable source for the next fixed-point round.
    Substitution preserves the source's partition algebra; it never invents
    disjointness from independent claim identities or equal scalar totals.
*/
pub(crate) fn compose_partition_wrappers(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    entailment: &mut super::contracts::ProvenExitExpressions<'_, '_>,
) {
    let source_plans = validation::content_conservation_plans(program);
    entailment.record_preserved_content_sources(facts, &source_plans);
    let outgoing_rows = partition_wrappers::OutgoingRows::new(&facts.flow.control.outgoing);
    let conditional_sources = source_plans
        .iter()
        .filter_map(|source| {
            if source.plan.owner_kind != ContentConservationOwnerKind::Machine {
                return None;
            }
            let returns = if let Some(proof) =
                entailment.conditional_content(&source.plan, source.source_expression)
            {
                proof.returns.clone()
            } else if entailment
                .for_machine(facts, source.plan.owner)
                .expressions
                .contains(&source.source_expression)
            {
                return None;
            } else {
                body_proofs::completed_source(program, facts, &outgoing_rows, source)?
            };
            Some((source.plan.clone(), returns))
        })
        .collect::<Vec<_>>();
    let mut available = source_plans
        .iter()
        .filter(|source| {
            source.plan.owner_kind == ContentConservationOwnerKind::TraitRequirement
                || program
                    .machine_by_symbol(source.plan.owner)
                    .is_some_and(|machine| {
                        machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                            || entailment
                                .for_machine(facts, machine.symbol)
                                .expressions
                                .contains(&source.source_expression)
                    })
        })
        .map(|source| AvailablePartitionSource {
            plan: source.plan.clone(),
            derivation_depth: 0,
        })
        .collect::<Vec<_>>();
    let mut compositions = Vec::<ContentPartitionCompositionFact>::new();
    let mut composition_conditions = Vec::new();
    let mut conditions = crate::flow::SelectionConditions::new();

    loop {
        let ready = conditional_sources
            .iter()
            .filter_map(|(plan, proof)| {
                if available.iter().any(|source| source.plan == *plan) {
                    return None;
                }
                proof
                    .iter()
                    .try_fold(0u32, |depth, returned| {
                        returned
                            .derivation_depth(|required| {
                                available
                                    .iter()
                                    .find(|source| source.plan == *required)
                                    .map(|source| source.derivation_depth)
                            })
                            .map(|next| depth.max(next))
                    })
                    .map(|depth| AvailablePartitionSource {
                        plan: plan.clone(),
                        derivation_depth: if proof.is_empty() {
                            0
                        } else {
                            depth.saturating_add(1)
                        },
                    })
            })
            .collect::<Vec<_>>();
        let mut added_source = !ready.is_empty();
        available.extend(ready);
        let sources = available.clone();
        let mut round = Vec::new();

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for invocation in returned_partition_invocations(
                    program,
                    facts,
                    &outgoing_rows,
                    state,
                    &mut conditions,
                ) {
                    for source in sources.iter().filter(|source| {
                        source.plan.callable == invocation.target_symbol
                            && equation_contains_partition(&source.plan.equation)
                    }) {
                        let Some((composition, condition)) = instantiate_partition_wrapper(
                            program,
                            facts,
                            machine.symbol,
                            state,
                            &invocation,
                            &source.plan,
                            source.derivation_depth,
                            &mut conditions,
                        ) else {
                            continue;
                        };
                        round.push((composition, condition));
                    }
                }
            }
        }

        let mut added_composition = false;
        for (composition, condition) in round {
            if let Some(index) = compositions
                .iter()
                .position(|existing| *existing == composition)
            {
                let combined = conditions.any(composition_conditions[index], condition);
                added_composition |= combined != composition_conditions[index];
                composition_conditions[index] = combined;
            } else {
                compositions.push(composition);
                composition_conditions.push(condition);
                added_composition = true;
            }
        }
        if !added_composition && !added_source {
            break;
        }
        for composition in &compositions {
            if available
                .iter()
                .any(|source| source.plan == composition.plan)
            {
                continue;
            }
            let Some(state) = program.state_by_symbol(composition.state_symbol) else {
                continue;
            };
            let outgoing = partition_wrappers::ordinary_returns(
                program,
                outgoing_rows.for_state(state.symbol).into_iter(),
                state,
            );
            let covered = outgoing.iter().all(|returned_at| {
                let mut coverage = conditions.never();
                for (candidate, condition) in compositions.iter().zip(&composition_conditions) {
                    if candidate.returned_at == *returned_at
                        && candidate.machine_symbol == composition.machine_symbol
                        && candidate.state_symbol == composition.state_symbol
                        && candidate.plan == composition.plan
                    {
                        coverage = conditions.any(coverage, *condition);
                    }
                }
                conditions.covers(&facts.flow.control, coverage)
            });
            if outgoing.is_empty() || !covered {
                continue;
            }
            available.push(AvailablePartitionSource {
                plan: composition.plan.clone(),
                derivation_depth: composition.source_derivation_depth.saturating_add(1),
            });
            added_source = true;
        }
        if !added_source {
            break;
        }
    }

    let mut complete_returns = Vec::new();
    for composition in &compositions {
        if complete_returns
            .iter()
            .any(|complete: &checked_trees::ContentPartitionReturnFact| {
                complete.returned_at == composition.returned_at && complete.plan == composition.plan
            })
        {
            continue;
        }
        let mut coverage = conditions.never();
        for (candidate, condition) in compositions.iter().zip(&composition_conditions) {
            if candidate.returned_at == composition.returned_at
                && candidate.plan == composition.plan
            {
                coverage = conditions.any(coverage, *condition);
            }
        }
        if conditions.covers(&facts.flow.control, coverage) {
            complete_returns.push(checked_trees::ContentPartitionReturnFact {
                returned_at: composition.returned_at,
                plan: composition.plan.clone(),
            });
        }
    }
    facts.qualifications.content.partition_returns = complete_returns;
    compositions.sort_by_key(|fact| {
        (
            fact.machine_symbol.arena_index(),
            fact.state_symbol.arena_index(),
            content_conservation_plan_bytes(&fact.plan),
        )
    });
    compositions.dedup();
    for source in source_plans
        .iter()
        .filter(|source| source.plan.owner_kind == ContentConservationOwnerKind::Machine)
    {
        let Some(machine) = program.machine_by_symbol(source.plan.owner) else {
            continue;
        };
        if program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == source.plan.callable)
            && available
                .iter()
                .any(|complete| complete.plan == source.plan)
        {
            entailment.record_proved_normal_return(facts, machine.symbol, source.source_expression);
        }
    }
    entailment
        .publish_content_sources(available.iter().map(|source| source.plan.clone()).collect());
    facts.qualifications.content.partition_compositions = compositions;
}

/*
    Consume a derived theorem only at the outgoing occurrence whose current
    result claims justified its substitution. Equal statement indices on two
    transition arms are not the same completed invocation.
*/
pub(super) fn proves_composed_exit(
    facts: &CheckFacts,
    exit: &checked_trees::FlowExitFact,
    fact: &facts::Fact,
    sources: &[validation::ContentConservationSourcePlan],
) -> bool {
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    sources
        .iter()
        .filter(|source| {
            source.source_expression == expression && source.plan.owner == exit.machine_symbol
        })
        .any(|source| {
            facts
                .qualifications
                .content
                .partition_returns
                .iter()
                .any(|composition| {
                    let outgoing = facts.flow.control.outgoing.get(composition.returned_at);
                    outgoing.machine_symbol == exit.machine_symbol
                        && outgoing.state_symbol == exit.state_symbol
                        && outgoing.statement_index == exit.statement_index
                        && outgoing.transition_target == exit.transition_target
                        && composition.plan.algebra == source.plan.algebra
                        && composition.plan.equation == source.plan.equation
                })
        })
}

/*
    Admit declaration-level foreign retention before building the caller loan
    graph. Completion, conservation and actual call facts remain later checks.
*/
pub(crate) fn prepare_retained_content_custody(
    program: &dyn ::validation::CheckingInput,
    content: checked_trees::ContentProjectionFacts,
) -> Result<checked_trees::ContentProjectionFacts, Vec<Diagnostic>> {
    let mut facts = CheckFacts {
        qualifications: checked_trees::QualificationFacts {
            content,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut rows = Vec::new();
    let mut diagnostics = Vec::new();
    for definition in program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        // Keyed as `check_retained_content_custody` keys them: by each closed
        // instance a call of the requirement targets.
        for signature in program.trait_machine_instances(definition) {
            let contracts = program
                .state_signature_contracts(signature)
                .iter()
                .collect::<Vec<_>>();
            check_callable(
                program,
                &facts,
                &format!("{}::{}", definition.name, signature.name),
                signature.symbol,
                &signature.lifetime_parameters,
                program.state_signature_parameters(signature),
                signature.return_type,
                &contracts,
                &mut diagnostics,
                &mut rows,
            );
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    facts.qualifications.content.retained_borrow_custodies = rows;
    Ok(facts.qualifications.content)
}

/*
    The retained borrow custodies the declarations admit, or every diagnostic.
    It only reads the facts; the caller publishes the rows, and clears them on
    failure.
*/
pub(crate) fn check_retained_content_custody(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<Vec<checked_trees::RetainedBorrowCustodyFact>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    call_ancestry::check_used_calls(program, facts, &mut diagnostics);
    call_results::check_call_result_qualifications(program, facts, &mut diagnostics);
    if facts.qualifications.content.plans.is_empty() {
        return if diagnostics.is_empty() {
            Ok(Vec::new())
        } else {
            Err(diagnostics)
        };
    }

    let mut retained_borrow_custodies = Vec::new();

    // A call of a requirement closed over static arguments targets its closed
    // instance, and lowering realizes that instance as the boundary machine, so
    // a custody names each instance the requirement executes as.
    for trait_definition in program.traits() {
        for signature in program.trait_machine_instances(trait_definition) {
            let contracts = program
                .state_signature_contracts(signature)
                .iter()
                .collect::<Vec<_>>();
            {
                check_callable(
                    program,
                    facts,
                    &format!("{}::{}", trait_definition.name, signature.name),
                    signature.symbol,
                    &signature.lifetime_parameters,
                    program.state_signature_parameters(signature),
                    signature.return_type,
                    &contracts,
                    &mut diagnostics,
                    &mut retained_borrow_custodies,
                );
            }
            if trait_definition.is_boundary {
                check_boundary_partition_results(
                    program,
                    facts,
                    &format!("{}::{}", trait_definition.name, signature.name),
                    signature.symbol,
                    program.state_signature_parameters(signature),
                    signature.return_type,
                    &contracts,
                    &mut diagnostics,
                );
            }
        }
    }

    program.for_each_machine_state_contract_frame(|machine, state, contracts, label| {
        check_callable(
            program,
            facts,
            &label,
            state.symbol,
            &machine.lifetime_parameters,
            program.state_parameters(state),
            state.return_type,
            &contracts,
            &mut diagnostics,
            &mut retained_borrow_custodies,
        );
        if !validation::call_target_has_checked_body_contract(program, machine.symbol) {
            check_boundary_partition_results(
                program,
                facts,
                &label,
                state.symbol,
                program.state_parameters(state),
                state.return_type,
                &contracts,
                &mut diagnostics,
            );
        }
    });

    if diagnostics.is_empty() {
        Ok(retained_borrow_custodies)
    } else {
        Err(diagnostics)
    }
}
