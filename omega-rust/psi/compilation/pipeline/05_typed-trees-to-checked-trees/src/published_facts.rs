/*

    This file builds the check facts. `access_views_and_uses.rs` builds placed
    view inputs and nominal machine uses, `machine_facts.rs` termination,
    blocking, suspension and invocation facts, `dynamic_conformance.rs`
    dynamic conformance facts, `contract_plan_facts.rs` contract plans and
    mutation facts, `crash_plan_facts.rs` crash capsules, buckets and sites,
    `requirement_call_specializations.rs` the specializations generic calls
    derived at their static machine arguments, and `qualification_facts.rs`
    qualification and service reach facts.
    `resolved_types.rs` publishes final executable place/result and carrier
    decisions after the typed arenas stop changing in `checking.rs`.
    Validation's `contract_structure` queries supply public declaration structure.
*/

#[path = "published_facts_boolean_predicates.rs"]
pub(crate) mod boolean_predicates;
pub(crate) mod capabilities {
    /*
        Checked capability-flow fact population.

        Capability-flow facts describe how authority-bearing values move through a
        package (chapter 18, "Capabilities And Authority Flow"). Each boundary call
        is classified into one or more verbs, inferred from boundary provenance,
        receiver provenance, signature service reach, and capability-typed returns — no new
        source keywords are required:

        - `uses`     — a boundary capability is reached by a call.
        - `stores`   — the capability is retained in a machine-owned field, either as
          the receiver of the boundary call or as the target of a `self.field = <call>`
          assignment.
        - `acquires` — the package obtains authority it holds itself (a boundary call
          through a machine-owned capability, or a boundary that returns a fresh
          capability value).
        - `returns`  — a capability value leaves through the enclosing state's
          capability-typed return.
        - `derives`  — a narrower capability is produced from an existing capability
          passed into the boundary call.
    */
    use crate::flow::expressions;
    use arena::{Arena, Handle};
    use checked_trees::FlowFacts;
    use flow_effects::{
        CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan, ServiceReachInferencePlan,
    };
    use language_core::is_self_receiver;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::machine::Machine;
    use typed_trees::signature::StateSignature;
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;

    /* Provenance of a boundary call's receiver: where the capability came from. */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ReceiverProvenance {
        /* The capability entered through a caller-provided parameter. */
        CallerProvided,
        /* The capability is retained in a machine-owned field. */
        MachineOwned,
        /* Provenance could not be determined. */
        Unknown,
    }

    pub(crate) fn build_capability_facts(
        program: &dyn ::validation::CheckingInput,
        service_reaches: &ServiceReachInferencePlan,
        flow: &FlowFacts,
    ) -> CapabilityFlowPlan {
        let mut flows = CapabilityFlowBuilder::new(flow.boundaries.edges.len());

        for (_, state) in flow.control.states.iter() {
            for call in flow.control.calls.span_or_empty(state.calls) {
                for edge in flow.boundaries.edges.span_or_empty(call.boundary_edges) {
                    push_unique(
                        &mut flows,
                        CapabilityFlowKind::Uses,
                        edge.boundary_trait_symbol,
                        state.machine_symbol,
                        state.state_symbol,
                        edge.statement_index,
                        edge.call_ordinal,
                        SymbolHandle::invalid(),
                    );
                }
            }
        }

        for machine_reach in service_reaches.machines() {
            let machine = machine_for_symbol(program, machine_reach.machine);
            for state_reach in service_reaches.states_for(machine_reach) {
                let enclosing = machine
                    .and_then(|machine| enclosing_state(program, machine, state_reach.state));
                for call in service_reaches.calls_for(state_reach) {
                    let Some((capability_symbol, signature)) =
                        boundary_capability(program, call.target_state)
                    else {
                        continue;
                    };

                    push_unique(
                        &mut flows,
                        CapabilityFlowKind::Uses,
                        capability_symbol,
                        machine_reach.machine,
                        state_reach.state,
                        call.statement_index,
                        call.call_ordinal,
                        SymbolHandle::invalid(),
                    );

                    let returns_capability = signature
                        .map(|signature| is_capability_type(program, signature.return_type))
                        .unwrap_or(false);
                    let derives_argument = signature
                        .map(|signature| passes_capability_argument(program, signature))
                        .unwrap_or(false);

                    let provenance = enclosing
                        .map(|state| call_receiver_provenance(program, state, call.statement_index))
                        .unwrap_or(ReceiverProvenance::Unknown);

                    let stored_into_field = matches!(provenance, ReceiverProvenance::MachineOwned)
                        || enclosing
                            .map(|state| {
                                call_stored_into_field(program, state, call.statement_index)
                            })
                            .unwrap_or(false);
                    if stored_into_field {
                        push_unique(
                            &mut flows,
                            CapabilityFlowKind::Stores,
                            capability_symbol,
                            machine_reach.machine,
                            state_reach.state,
                            call.statement_index,
                            call.call_ordinal,
                            SymbolHandle::invalid(),
                        );
                    }

                    if returns_capability || matches!(provenance, ReceiverProvenance::MachineOwned)
                    {
                        push_unique(
                            &mut flows,
                            CapabilityFlowKind::Acquires,
                            capability_symbol,
                            machine_reach.machine,
                            state_reach.state,
                            call.statement_index,
                            call.call_ordinal,
                            SymbolHandle::invalid(),
                        );
                    }

                    if derives_argument
                        && (returns_capability
                            || matches!(provenance, ReceiverProvenance::CallerProvided))
                    {
                        push_unique(
                            &mut flows,
                            CapabilityFlowKind::Derives,
                            capability_symbol,
                            machine_reach.machine,
                            state_reach.state,
                            call.statement_index,
                            call.call_ordinal,
                            SymbolHandle::invalid(),
                        );
                    }

                    if returns_capability
                        && enclosing
                            .map(|state| is_capability_type(program, state.return_type))
                            .unwrap_or(false)
                    {
                        push_unique(
                            &mut flows,
                            CapabilityFlowKind::Returns,
                            capability_symbol,
                            machine_reach.machine,
                            state_reach.state,
                            call.statement_index,
                            call.call_ordinal,
                            SymbolHandle::invalid(),
                        );
                    }
                }
            }
        }

        propagate_nested_capability_flows(program, service_reaches, &mut flows);

        CapabilityFlowPlan::with_roots(flows.rows)
    }

    /*
        Keep each state's facts in append order while constructing the published
        arena. Nested calls need only their callee's facts; unrelated states cannot
        contribute a matching fact or a duplicate. A dense slot bucket can contain
        several generations; reads still compare the complete state handle.
    */
    struct CapabilityFlowBuilder {
        rows: Arena<CapabilityFlowFact>,
        states: Vec<Vec<Handle<CapabilityFlowFact>>>,
    }

    impl CapabilityFlowBuilder {
        fn new(capacity: usize) -> Self {
            Self {
                rows: Arena::with_capacity(capacity),
                states: Vec::new(),
            }
        }

        fn state_facts(&self, state: SymbolHandle) -> impl Iterator<Item = &CapabilityFlowFact> {
            self.states
                .get(state.arena_index() as usize)
                .into_iter()
                .flatten()
                .map(|handle| self.rows.get(*handle))
                .filter(move |fact| fact.state_symbol == state)
        }

        fn push_unique(&mut self, fact: CapabilityFlowFact) -> bool {
            if self
                .state_facts(fact.state_symbol)
                .any(|existing| *existing == fact)
            {
                return false;
            }
            let position = fact.state_symbol.arena_index() as usize;
            if self.states.len() <= position {
                self.states.resize_with(position + 1, Vec::new);
            }
            self.states[position].push(self.rows.append(fact));
            true
        }
    }

    /*
        A nested-call edge in the service-reach plan's call graph: a caller state
        calling a target state, with the source location of the call.
    */
    struct CallEdge {
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        target_state_symbol: SymbolHandle,
    }

    /*
        Flows `returns`/`derives`/`acquires` capability verbs from nested in-package
        callees up to their callers. If a call's target state itself returns (resp.
        derives, acquires) a boundary capability and the authority value reaches the
        caller, the caller records the same verb for that capability with the helper
        state as provenance. Iterated to a fixpoint so the verb can travel several
        call levels.
    */
    fn propagate_nested_capability_flows(
        program: &dyn ::validation::CheckingInput,
        service_reaches: &ServiceReachInferencePlan,
        flows: &mut CapabilityFlowBuilder,
    ) {
        let edges = collect_call_edges(service_reaches);

        loop {
            let mut added = false;

            for edge in &edges {
                for kind in [
                    CapabilityFlowKind::Returns,
                    CapabilityFlowKind::Derives,
                    CapabilityFlowKind::Acquires,
                ] {
                    let propagated: Vec<SymbolHandle> = flows
                        .state_facts(edge.target_state_symbol)
                        .filter(|fact| fact.kind == kind)
                        .map(|fact| fact.capability_symbol)
                        .collect();

                    if propagated.is_empty() {
                        continue;
                    }

                    if !nested_flow_reaches_caller(program, edge, kind) {
                        continue;
                    }

                    for capability_symbol in propagated {
                        if push_unique(
                            flows,
                            kind,
                            capability_symbol,
                            edge.caller_machine_symbol,
                            edge.caller_state_symbol,
                            edge.statement_index,
                            edge.call_ordinal,
                            edge.target_state_symbol,
                        ) {
                            added = true;
                        }
                    }
                }
            }

            if !added {
                break;
            }
        }
    }

    fn collect_call_edges(service_reaches: &ServiceReachInferencePlan) -> Vec<CallEdge> {
        let mut edges = Vec::new();
        for machine_reach in service_reaches.machines() {
            for state_reach in service_reaches.states_for(machine_reach) {
                for call in service_reaches.calls_for(state_reach) {
                    if !call.target_state.is_valid() {
                        continue;
                    }
                    edges.push(CallEdge {
                        caller_machine_symbol: machine_reach.machine,
                        caller_state_symbol: state_reach.state,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_state_symbol: call.target_state,
                    });
                }
            }
        }
        edges
    }

    /*
        Whether a propagated `kind` verb actually reaches the caller across `edge`.
        Mirrors the direct-boundary gating: `returns` requires the caller's enclosing
        state to return a capability type; `derives` requires the caller to return a
        capability or to hold a capability parameter that feeds the nested
        derivation; `acquires` requires the callee to hand the minted authority back
        through its capability-typed return, which places it in the caller's hands.
    */
    fn nested_flow_reaches_caller(
        program: &dyn ::validation::CheckingInput,
        edge: &CallEdge,
        kind: CapabilityFlowKind,
    ) -> bool {
        if kind == CapabilityFlowKind::Acquires {
            return state_for_symbol(program, edge.target_state_symbol)
                .map(|callee| is_capability_type(program, callee.return_type))
                .unwrap_or(false);
        }

        let Some(machine) = machine_for_symbol(program, edge.caller_machine_symbol) else {
            return false;
        };
        let Some(state) = enclosing_state(program, machine, edge.caller_state_symbol) else {
            return false;
        };

        let returns_capability = is_capability_type(program, state.return_type);

        match kind {
            CapabilityFlowKind::Returns => returns_capability,
            CapabilityFlowKind::Derives => {
                returns_capability || state_has_capability_parameter(program, state)
            }
            _ => false,
        }
    }

    /* Whether `state` declares a non-self capability-typed parameter. */
    fn state_has_capability_parameter(
        program: &dyn ::validation::CheckingInput,
        state: &State,
    ) -> bool {
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| is_capability_type(program, parameter.type_reference))
    }

    #[allow(clippy::too_many_arguments)]
    fn push_unique(
        flows: &mut CapabilityFlowBuilder,
        kind: CapabilityFlowKind,
        capability_symbol: SymbolHandle,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        via_state_symbol: SymbolHandle,
    ) -> bool {
        let fact = CapabilityFlowFact {
            kind,
            capability_symbol,
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
            via_state_symbol,
        };

        flows.push_unique(fact)
    }

    /*
        Classifies the receiver of the boundary call at `statement_index` as
        caller-provided (rooted at a state parameter) or machine-owned (rooted at
        `self`, i.e. a retained field).
    */
    fn call_receiver_provenance(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
    ) -> ReceiverProvenance {
        let Some(statement) = program.statement_table.statement_at(state, statement_index) else {
            return ReceiverProvenance::Unknown;
        };

        let head = match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => None,
            StatementNode::Call(call) => program
                .statement_table
                .name_path_members(call.receiver)
                .first()
                .map(|identifier| identifier.as_str().to_owned()),
            StatementNode::Assignment(assignment) => {
                expression_receiver_head(program, assignment.value)
            }
            StatementNode::LocalData(local_data) => {
                expression_receiver_head(program, local_data.initial_value)
            }
            StatementNode::Expression(expression) => expression_receiver_head(program, *expression),
            StatementNode::Transition(_) => None,
        };

        let Some(head) = head else {
            return ReceiverProvenance::Unknown;
        };

        if is_self_receiver(&head) {
            return ReceiverProvenance::MachineOwned;
        }

        if program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.name.as_str() == head)
        {
            return ReceiverProvenance::CallerProvided;
        }

        ReceiverProvenance::Unknown
    }

    /*
        Extracts the head identifier of the receiver of a call nested in an
        expression (`self.x = receiver.call()` / `let y = receiver.call()`).
    */
    fn expression_receiver_head(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<String> {
        if !expression.is_valid() {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => expression_path_head(program, call.receiver),
            ExpressionNode::Borrow(inner) => expression_receiver_head(program, inner.target),
            ExpressionNode::Cast(cast) => expression_receiver_head(program, cast.value),
            _ => None,
        }
    }

    fn expression_path_head(
        program: &dyn ::validation::CheckingInput,
        receiver: typed_trees::expression::ExpressionHandle,
    ) -> Option<String> {
        if !receiver.is_valid() {
            return None;
        }
        match program.expression_table.expression(receiver) {
            ExpressionNode::Member(member) => expression_path_head(program, member.receiver),
            ExpressionNode::Borrow(inner) => expression_path_head(program, inner.target),
            ExpressionNode::Name(name) => expressions(program)
                .name_path_members(name.members)
                .first()
                .map(|identifier| identifier.as_str().to_owned()),
            _ => None,
        }
    }

    fn machine_for_symbol(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> Option<&Machine> {
        program.machine(machine_symbol)
    }

    fn enclosing_state<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &Machine,
        state_symbol: SymbolHandle,
    ) -> Option<&'program State> {
        program.state_in_machine(machine, state_symbol)
    }

    /* Finds a state by symbol across every machine in the program. */
    fn state_for_symbol(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
    ) -> Option<&State> {
        program
            .machines()
            .iter()
            .find_map(|machine| program.state_in_machine(machine, state_symbol))
    }

    /*
        Whether the assignment at `statement_index` stores its value into a field
        (`self.field = ...`).
    */
    fn call_stored_into_field(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
    ) -> bool {
        let Some(statement) = program.statement_table.statement_at(state, statement_index) else {
            return false;
        };

        let Some(assignment) = statement.as_assignment() else {
            return false;
        };

        assignment_target_is_field(program, assignment.target)
    }

    fn assignment_target_is_field(
        program: &dyn ::validation::CheckingInput,
        target: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        if !target.is_valid() {
            return false;
        }

        match program.expression_table.expression(target) {
            ExpressionNode::Member(_) => true,
            ExpressionNode::Borrow(inner) => assignment_target_is_field(program, inner.target),
            ExpressionNode::Name(name) => {
                expressions(program).name_path_members(name.members).len() > 1
            }
            _ => false,
        }
    }

    /*
        Whether a boundary signature takes another capability as an argument, making
        its result a derived sub-capability.
    */
    fn passes_capability_argument(
        program: &dyn ::validation::CheckingInput,
        signature: &StateSignature,
    ) -> bool {
        program
            .state_signature_parameters(signature)
            .iter()
            .any(|parameter| is_capability_type(program, parameter.type_reference))
    }

    /*
        Whether a type reference resolves to a capability type: a non-primitive
        named/data type carried through boundary surfaces (e.g. `Folder`). Primitive
        types are never capabilities.
    */
    fn is_capability_type(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        if program.primitive_type_reference(type_reference).is_some() {
            return false;
        }
        program.type_reference_symbol(type_reference).is_valid()
    }

    /*
        Resolves a call target to the boundary trait it reaches and the boundary
        signature, if any. The target may be a boundary trait signature directly, or
        an in-package implementation state whose machine conforms to a boundary trait.
    */
    fn boundary_capability(
        program: &dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
    ) -> Option<(SymbolHandle, Option<&StateSignature>)> {
        if !target_symbol.is_valid() {
            return None;
        }

        if let Some(found) = program.traits().iter().find_map(|trait_definition| {
            if !trait_definition.is_boundary {
                return None;
            }
            program
                .trait_machine_signature_by_symbol(trait_definition, target_symbol)
                .map(|signature| (trait_definition.symbol, Some(signature)))
        }) {
            return Some(found);
        }

        let (machine, state) = program.machines().iter().find_map(|machine| {
            program
                .state_in_machine(machine, target_symbol)
                .map(|state| (machine, state))
        })?;

        for conformance in program.machine_trait_conformances(machine) {
            let Some(trait_definition) = program.trait_definition_by_symbol(conformance.symbol)
            else {
                continue;
            };
            if !trait_definition.is_boundary {
                continue;
            }
            if let Some(signature) = program
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|signature| signature.name == state.name)
            {
                return Some((trait_definition.symbol, Some(signature)));
            }
        }

        None
    }
}
mod carry {
    use checked_trees::{CarryFacts, DataCarryFact};

    pub(super) fn build_carry_facts(program: &dyn ::validation::CheckingInput) -> CarryFacts {
        let data = program
            .data_definitions()
            .iter()
            .map(|definition| DataCarryFact {
                data: definition.symbol,
                declared: definition.properties.carry,
                effective: validation::effective_data_carry_policy(program, definition),
            })
            .collect();

        CarryFacts {
            data,
            suspension_crossings: Vec::new(),
            activation_wide_carry: Vec::new(),
            claim_policies: Vec::new(),
            suspension_loans: arena::Arena::new(),
            suspension_value_loans: arena::Arena::new(),
        }
    }
}
pub(crate) mod contextual_cleanup {
    /*
        Bind selected cleanup occurrences to their exact source and edge context.

        Specialization owns the lexical symbol correspondence. Consume its exact
        generated-from links instead of reconstructing parameter/local identity from
        spelling, declaration order, or generated machine names. The occurrence and
        its excluded subtrees still come from the one custody producer.
    */
    pub(crate) mod entry_flow {
        /*
            Seed bound cleanup predicates into ordinary entry flow as relational facts.
            Rebuilding starts from declarations; observations from an earlier flow sweep
            never become assumptions, and implications never seed their consequences.
        */
        use super::super::boolean_predicates::bind_entry;
        use super::super::boolean_predicates::seed;
        use checked_trees::ContextualCleanupEntryRequirements;
        use diagnostics::Diagnostic;
        use facts::{FactOrigin, FactPlan, ProgramPoint};

        pub(crate) fn seed_entry_requirements(
            program: &dyn ::validation::CheckingInput,
            semantic: &mut FactPlan,
            requirements: &ContextualCleanupEntryRequirements,
        ) -> Result<(), Vec<Diagnostic>> {
            for (_, requirement) in requirements.rows.iter() {
                let Some(predicate) =
                    bind_entry(program, requirement.machine, requirement.predicate())
                else {
                    return Err(vec![Diagnostic::error(
                        "bound cleanup prerequisite has no exact invocation-entry predicate bindings",
                    )]);
                };
                seed(
                    semantic,
                    &predicate,
                    ProgramPoint::State {
                        machine_symbol: predicate.machine,
                        state_symbol: predicate.entry_state,
                    },
                    FactOrigin::ContextualCleanup {
                        machine_symbol: predicate.machine,
                        state_symbol: predicate.entry_state,
                    },
                );
            }
            Ok(())
        }
    }
    mod receiver_loans {
        /*
            Every disposed residual remains an intact nominal receiver until its
            admitted death, whether its owner came from an ordinary establishment or a
            conditional selection. Lexical observations cannot end its carried loans
            before the hook's implicit use. The ownership producer chooses the residual;
            this fixed-point extension preserves its receiver through that boundary.
        */
        use checked_trees::{BorrowFacts, ContextualCleanupFacts, FlowOwnershipFacts};
        use language_semantics::PermissionEventSource;

        pub(crate) fn retain_selected_receiver_loans(
            program: &dyn validation::CheckingInput,
            borrow: &mut BorrowFacts,
            ownership: &FlowOwnershipFacts,
            cleanup: &ContextualCleanupFacts,
        ) -> bool {
            let mut changes: Vec<(arena::Handle<checked_trees::BorrowLoanFact>, usize)> =
                Vec::new();
            for (_, disposition) in cleanup.dispositions.iter() {
                if disposition.zero_image {
                    continue;
                }
                let Some(state) = program.state_by_symbol(disposition.state) else {
                    continue;
                };
                let death = match disposition.source {
                    PermissionEventSource::StateExit => disposition.exit.statement_index,
                    PermissionEventSource::Statement { statement_index }
                    | PermissionEventSource::Call {
                        statement_index, ..
                    } => statement_index,
                    _ => continue,
                };
                let order = program
                    .statement_table
                    .admitted_execution_order(state.statement_nodes);
                let receiver_coordinate = if disposition.selection.is_valid() {
                    ownership
                        .owned_selections
                        .get(disposition.selection)
                        .statement_ordinal as usize
                } else {
                    death
                };
                let death = if order
                    .position(death)
                    .is_some_and(|position| position < order.indices().len())
                {
                    death
                } else {
                    let Some(last) = order.indices().last().copied() else {
                        continue;
                    };
                    last
                };
                for (handle, loan) in borrow.loans.iter().filter(|(_, loan)| {
                    Some(loan.owner_symbol) == disposition.root.as_symbol()
                        && (loan.statement_index == receiver_coordinate
                            || order.precedes(loan.statement_index, receiver_coordinate))
                }) {
                    let Some(reference) =
                        validation::reference_result_custody::storage_owner_type_reference(
                            program,
                            state,
                            loan.owner_symbol,
                            receiver_coordinate as u32,
                        )
                    else {
                        continue;
                    };
                    let path = crate::borrow::owner_place_segments(borrow.loan_owner_path(loan));
                    if crate::borrow::place_path_covers_owner(
                        program,
                        ownership.segments.span_or_empty(disposition.path),
                        borrow.loan_owner_path(loan),
                    ) && ![disposition.exclusions, disposition.inactive_cases]
                        .into_iter()
                        .flat_map(|regions| cleanup.exclusions.span_or_empty(regions))
                        .any(|excluded| {
                            excluded.root == disposition.root
                                && crate::borrow::owner_path_overlaps_place_segments(
                                    program,
                                    borrow.loan_owner_path(loan),
                                    cleanup.segments.span_or_empty(excluded.segments),
                                )
                        })
                        && validation::reference_result_custody::path_has_nominal_receiver(
                            program, reference, &path,
                        )
                        && order.precedes(loan.last_use_statement_index, death)
                    {
                        if let Some((_, previous)) = changes
                            .iter_mut()
                            .find(|(candidate, _)| *candidate == handle)
                        {
                            if order.precedes(*previous, death) {
                                *previous = death;
                            }
                        } else {
                            changes.push((handle, death));
                        }
                    }
                }
            }
            let changed = !changes.is_empty();
            for (handle, death) in changes {
                borrow.loans.get_mut(handle).last_use_statement_index = death;
            }
            changed
        }
    }
    mod requirements {
        /*
            Check each selected death against the hook's complete callable predicates.
            Formal observations rebase onto the exact live receiver at the selected
            edge, then use the ordinary current-value predicate algebra. Unsupported
            clauses fail closed; a missing Boolean recognizer cannot erase a requirement.
            Generic entry transport uses that same complete predicate vocabulary.

            A fixed concrete owner in a generic signature is not a symbolic death. Only
            storage under the source type parameter can introduce a bound obligation.
            Its missing proposition must still denote the invocation's original value;
            writes and non-entry locals cannot be repaired by assuming a fact at exit.
        */
        use validation::CanonicalPlace;

        use checked_trees::{
            CheckFacts, CheckedBooleanExpression, CheckedStructuralPredicatePathSegment,
            ContextualCleanupEntryRequirement, ContextualCleanupEntryRequirements, FlowStateFact,
        };
        use diagnostics::Diagnostic;
        use facts::{FactContextHandle, PlaceRoot, PlaceSegment};
        use language_semantics::PermissionEventSource;
        use symbols::SymbolHandle;
        use typed_trees::{expression::ExpressionHandle, types::TypeReferenceNode};

        use crate::checks::contracts::{boolean_value_at_place, contextual_cleanup as calls};
        use crate::execution::terminal_unit::cleanup::{
            checked_requires_expressions, direct_boolean_field_requirement,
        };
        use crate::flow::{CleanupLeaf, selected_cleanup_leaves};

        use super::super::boolean_predicates::{
            self, BoundPredicate, conjunction, entry_path_condition,
        };
        use super::sequence::{CurrentCleanupFacts, bound_clause, ordered_hooks};

        struct Pending {
            machine: SymbolHandle,
            consequence: CheckedBooleanExpression,
            antecedent: CheckedBooleanExpression,
            hook_machine: SymbolHandle,
            hook_clause: ExpressionHandle,
            template_machine: SymbolHandle,
        }

        pub(crate) fn infer_entry_requirements(
            program: &dyn ::validation::CheckingInput,
            facts: &mut CheckFacts,
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Result<bool, Vec<Diagnostic>> {
            let mut pending = Vec::new();
            for obligation in selected_requirements(program, facts, frames)? {
                let place = &obligation.place;
                let instance = facts
                    .contextual_cleanup
                    .dispositions
                    .get(obligation.disposition);
                let introduced =
                    facts
                        .contextual_cleanup
                        .applications
                        .iter()
                        .find_map(|(_, application)| {
                            if application.instance != obligation.disposition {
                                return None;
                            }
                            let template = facts
                                .contextual_cleanup
                                .dispositions
                                .get(application.template);
                            let leaves = selected_cleanup_leaves(program, facts, template)?;
                            let path =
                                super::source_path(program, &place.segments, instance.machine);
                            leaves
                                .iter()
                                .any(|leaf| {
                                    matches!(leaf,
                        CleanupLeaf::Symbolic { path: symbolic, .. } if covers(symbolic, &path))
                                })
                                .then_some(template.machine)
                        });
                let Some(template) = introduced else { continue };
                if obligation.proven(program, facts) {
                    continue;
                }
                if !obligation.entry_stable {
                    continue;
                }
                let Some(antecedent) = entry_path_condition(
                    &facts.semantic,
                    &obligation.current.contexts,
                    instance.machine,
                ) else {
                    continue;
                };
                if let Some(row) = transport(
                    program,
                    instance.machine,
                    instance.state,
                    obligation.statement,
                    &obligation.predicate,
                    antecedent,
                    obligation.hook,
                    obligation.clause,
                    template,
                ) {
                    pending.push(row);
                }
            }
            if !facts.contextual_cleanup.entry_requirements.rows.is_empty() {
                for (_, state) in facts.flow.control.states.iter() {
                    let Some(specialization) = program.machine_specialization(state.machine_symbol)
                    else {
                        continue;
                    };
                    for call in facts.flow.control.calls.span_or_empty(state.calls) {
                        let Some(target) = calls::target_machine(program, call) else {
                            continue;
                        };
                        if !facts
                            .contextual_cleanup
                            .entry_requirements
                            .rows
                            .iter()
                            .any(|(_, row)| row.machine == target)
                        {
                            continue;
                        }
                        let contexts: Vec<_> = facts
                            .flow
                            .state_call_entry_semantic_contexts(
                                state,
                                call.statement_index,
                                call.call_ordinal,
                                call.target_symbol,
                                call.receiver_symbol,
                            )
                            .collect();
                        for (_, requirement) in facts
                            .contextual_cleanup
                            .entry_requirements
                            .rows
                            .iter()
                            .filter(|(_, row)| row.machine == target)
                        {
                            if calls::proves(
                                program,
                                facts,
                                state,
                                call,
                                &contexts,
                                requirement,
                                frames,
                            ) {
                                continue;
                            }
                            let Some(place) =
                                calls::actual_place(program, state, call, requirement)
                            else {
                                continue;
                            };
                            if !has_symbolic_origin(program, state, call.statement_index, &place) {
                                continue;
                            }
                            let Some(path_condition) = entry_path_condition(
                                &facts.semantic,
                                &contexts,
                                state.machine_symbol,
                            ) else {
                                continue;
                            };
                            let Some(callee_condition) =
                                calls::entry_antecedent(program, facts, state, call, requirement)
                            else {
                                continue;
                            };
                            let Some(consequence) =
                                calls::entry_consequence(program, facts, state, call, requirement)
                            else {
                                continue;
                            };
                            pending.push(Pending {
                                machine: state.machine_symbol,
                                consequence,
                                antecedent: conjunction(path_condition, callee_condition),
                                hook_machine: requirement.hook_machine,
                                hook_clause: requirement.hook_clause,
                                template_machine: specialization.template,
                            });
                        }
                    }
                }
            }
            let mut changed = false;
            for row in pending {
                changed |= append(&mut facts.contextual_cleanup.entry_requirements, row)?;
            }
            Ok(changed)
        }

        struct SelectedRequirement {
            disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
            statement: usize,
            current: CurrentCleanupFacts,
            entry_stable: bool,
            place: CanonicalPlace,
            hook: SymbolHandle,
            clause: ExpressionHandle,
            predicate: BoundPredicate,
        }

        impl SelectedRequirement {
            fn proven(
                &self,
                program: &dyn ::validation::CheckingInput,
                facts: &CheckFacts,
            ) -> bool {
                let disposition = facts.contextual_cleanup.dispositions.get(self.disposition);
                let point = facts::ProgramPoint::Statement {
                    machine_symbol: disposition.machine,
                    state_symbol: disposition.state,
                    statement_index: self.statement,
                };
                let mut retained = boolean_predicates::retained(
                    program,
                    &facts.operators,
                    &facts.semantic,
                    &self.current.contexts,
                    point,
                );
                retained.extend(self.current.guarantees.iter().cloned());
                let mut required = self.predicate.clone();
                if let Some(state) = facts
                    .flow
                    .control
                    .state(disposition.machine, disposition.state)
                {
                    let transported = crate::checks::contracts::cleanup_predicates(
                        program,
                        facts,
                        state,
                        &self.current.contexts,
                        &mut required,
                        &retained,
                        |binding| {
                            self.current.entry_stable(
                                program,
                                &BoundPredicate {
                                    bindings: vec![binding.clone()],
                                    ..self.predicate.clone()
                                },
                            )
                        },
                    );
                    if transported.proves_required {
                        return true;
                    }
                    retained.extend(transported.predicates);
                }
                crate::checks::contracts::proves_captured_predicates(
                    program,
                    &facts.semantic,
                    &self.current.contexts,
                    &retained,
                    &required,
                    |place| {
                        self.current
                            .value_at_place(program, facts, point, place)
                            .or_else(|| {
                                boolean_value_at_place(
                                    program,
                                    &facts.operators,
                                    &facts.semantic,
                                    &self.current.contexts,
                                    point,
                                    place,
                                )
                                .map(facts::ScalarValue::Boolean)
                                .or_else(|| {
                                    crate::values::scalar_value_at_place(
                                        program,
                                        &facts.semantic,
                                        self.current
                                            .contexts
                                            .iter()
                                            .map(|context| facts.semantic.contexts.get(*context)),
                                        place,
                                    )
                                })
                            })
                    },
                )
            }

            fn transportable_subject(
                &self,
                program: &dyn ::validation::CheckingInput,
            ) -> Option<(CanonicalPlace, bool)> {
                let hook = program.machine_by_symbol(self.hook)?;
                let state = program.machine_states(hook).first()?;
                let receiver = program.state_parameters(state).first()?;
                let requirement =
                    direct_boolean_field_requirement(program, state.symbol, receiver, self.clause)?;
                let (_, path, _, _) = crate::values::resolve_structural_parameter_path(
                    program,
                    program.state_parameters(state),
                    0,
                    &[CheckedStructuralPredicatePathSegment::Field(
                        requirement.field_identity,
                    )],
                )?;
                let mut place = self.place.clone();
                place.segments.extend(path);
                Some((place, requirement.expected))
            }
        }

        /*
            Selection and actual-edge substitution are shared by inference and final
            source checking. A Terminal planner declining a shape must not silently
            authorize cleanup whose source premise was never established.
        */
        fn selected_requirements(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            shared_call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Result<Vec<SelectedRequirement>, Vec<Diagnostic>> {
            struct Hook {
                disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
                selected: bool,
                receiver: super::sequence::SelectedHook,
            }
            struct Group<'a> {
                state: &'a FlowStateFact,
                source: PermissionEventSource,
                exit: checked_trees::FlowClaimJoinExit,
                root: PlaceRoot,
                statement: usize,
                contexts: Vec<FactContextHandle>,
                hooks: Vec<Hook>,
            }
            struct Occurrence {
                machine: SymbolHandle,
                state: SymbolHandle,
                source: PermissionEventSource,
                exit: checked_trees::FlowClaimJoinExit,
                statement: usize,
                entry: Vec<FactContextHandle>,
                current: CurrentCleanupFacts,
            }
            if facts.contextual_cleanup.dispositions.is_empty() {
                return Ok(Vec::new());
            }
            let mut result = Vec::new();
            let proof_only = validation::proof_only_classification(program);
            let summaries = crate::flow::StateMutationSummaryCache::default();
            let mut owned_frames = None;
            let call_frames =
                crate::flow::shared_call_frames_or(shared_call_frames, program, &mut owned_frames);
            let mut groups = Vec::<Group<'_>>::new();
            for (handle, disposition) in facts.contextual_cleanup.dispositions.iter() {
                let Some(machine) = program.machine_by_symbol(disposition.machine) else {
                    continue;
                };
                if proof_only.is_proof_machine(program, machine) {
                    continue;
                }
                let Some(state) = facts
                    .flow
                    .control
                    .state(disposition.machine, disposition.state)
                else {
                    continue;
                };
                let Some(leaves) = selected_cleanup_leaves(program, facts, disposition) else {
                    continue;
                };
                if leaves.is_empty() {
                    continue;
                }
                let path = facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(disposition.path);
                let hooks = ordered_hooks(program, path, &leaves).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "cannot prove automatic cleanup requires: selected receiver order is unresolved",
                    )]
                })?;
                if hooks.is_empty() {
                    continue;
                }
                let selected = facts.contextual_cleanup.is_selected_disposition(handle);
                let points = occurrence_contexts(program, facts.view(), state, disposition)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "cleanup prerequisite has no exact outgoing edge context in {}",
                            crate::published::labels::machine_name(program, disposition.machine)
                        ))]
                    })?;
                for (statement, contexts) in points {
                    let position = groups
                        .iter()
                        .position(|group| {
                            group.state.machine_symbol == disposition.machine
                                && group.state.state_symbol == disposition.state
                                && group.source == disposition.source
                                && group.exit == disposition.exit
                                && group.root == disposition.root
                                && group.statement == statement
                                && group.contexts == contexts
                        })
                        .unwrap_or_else(|| {
                            let position = groups.len();
                            groups.push(Group {
                                state,
                                source: disposition.source,
                                exit: disposition.exit,
                                root: disposition.root,
                                statement,
                                contexts,
                                hooks: Vec::new(),
                            });
                            position
                        });
                    groups[position]
                        .hooks
                        .extend(hooks.iter().cloned().map(|receiver| Hook {
                            disposition: handle,
                            selected,
                            receiver,
                        }));
                }
            }
            let mut occurrences = Vec::<Occurrence>::new();
            for mut group in groups {
                if group.hooks.iter().any(|hook| hook.selected) {
                    let mut ordered = group.hooks.into_iter().map(|hook| {
                        crate::flow::cleanup_path_order(program, &hook.receiver.path)
                            .map(|order| (order, hook))
                    }).collect::<Option<Vec<_>>>().ok_or_else(|| vec![Diagnostic::error(
                        "cannot prove automatic cleanup requires: selected partition order is unresolved",
                    )])?;
                    ordered.sort_by(|left, right| left.0.cmp(&right.0));
                    group.hooks = ordered.into_iter().map(|(_, hook)| hook).collect();
                }
                let state = group.state;
                let statement = group.statement;
                let position = occurrences
                    .iter()
                    .position(|row| {
                        row.machine == state.machine_symbol
                            && row.state == state.state_symbol
                            && row.source == group.source
                            && row.exit == group.exit
                            && row.statement == statement
                            && row.entry == group.contexts
                    })
                    .unwrap_or_else(|| {
                        let position = occurrences.len();
                        occurrences.push(Occurrence {
                            machine: state.machine_symbol,
                            state: state.state_symbol,
                            source: group.source,
                            exit: group.exit,
                            statement,
                            entry: group.contexts.clone(),
                            current: CurrentCleanupFacts::new(
                                group.contexts.clone(),
                                call_frames
                                    .as_ref()
                                    .map(|frames| {
                                        crate::checks::contracts::current_call_guarantee_predicates(
                                            program,
                                            facts,
                                            state,
                                            match group.source {
                                                PermissionEventSource::StateEntry => crate::checks::contracts::GuaranteeUse::StatementEntry(statement),
                                                _ => crate::checks::contracts::GuaranteeUse::EvaluatedContext(statement),
                                            },
                                            &group.contexts,
                                            frames,
                                        )
                                    })
                                    .unwrap_or_default(),
                            ),
                        });
                        position
                    });
                let current = &mut occurrences[position].current;
                let mut offset = 0;
                while offset < group.hooks.len() {
                    let first = &group.hooks[offset];
                    let mut end = offset + 1;
                    if first.selected {
                        while end < group.hooks.len()
                            && group.hooks[end].disposition == first.disposition
                        {
                            end += 1;
                        }
                    }
                    let skipped = first.selected.then(|| current.clone());
                    for item in &group.hooks[offset..end] {
                        let handle = item.disposition;
                        let disposition = facts.contextual_cleanup.dispositions.get(handle);
                        let selected = &item.receiver;
                        let hook = selected.machine;
                        let mut place = CanonicalPlace {
                            root: disposition.root,
                            segments: selected.path.clone(),
                        };
                        crate::flow::narrow_pinned_index_segments(
                            program,
                            &facts.semantic,
                            state.state_symbol,
                            statement,
                            &current.contexts,
                            &mut place.segments,
                        );
                        crate::flow::compose_captured_view_segments(&mut place.segments).ok_or_else(
                            || {
                                vec![Diagnostic::error(
                                    "selected cleanup receiver has invalid captured view coordinates",
                                )]
                            },
                        )?;
                        let hook_machine = program.machine_by_symbol(hook).ok_or_else(|| {
                            vec![Diagnostic::error("selected cleanup hook is absent")]
                        })?;
                        let hook_state =
                            program
                                .machine_states(hook_machine)
                                .first()
                                .ok_or_else(|| {
                                    vec![Diagnostic::error("selected cleanup hook entry is absent")]
                                })?;
                        let clauses = checked_requires_expressions(program, facts, hook, hook_state.symbol).ok_or_else(|| vec![Diagnostic::error(format!(
                            "cannot prove automatic cleanup requires: {} has no complete callable clause roster",
                            crate::published::labels::machine_name(program, hook),
                        ))])?;
                        for clause in clauses {
                            let predicate = bound_clause(program, facts, hook, clause, checked_trees::ContractProofFactKind::Requires, &place).ok_or_else(|| vec![Diagnostic::error(format!(
                                "cannot prove automatic cleanup requires: {} clause `{}` has no complete callable predicate",
                                crate::published::labels::machine_name(program, hook), program.expression_table.display_name(clause),
                            ))])?;
                            result.push(SelectedRequirement {
                                disposition: handle,
                                statement,
                                entry_stable: current.entry_stable(program, &predicate),
                                current: current.clone(),
                                place: place.clone(),
                                hook,
                                clause,
                                predicate,
                            });
                        }
                        current.advance(program, facts, &summaries, call_frames, hook, &place).ok_or_else(|| vec![Diagnostic::error(
                            "cannot prove automatic cleanup requires: selected hook completion is unresolved",
                        )])?;
                    }
                    if let Some(skipped) = skipped {
                        current.join_skipped(&skipped);
                    }
                    offset = end;
                }
            }
            Ok(result)
        }

        pub(crate) fn check_cleanup_requirements(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
        ) -> Vec<Diagnostic> {
            let selected = match selected_requirements(program, facts, None) {
                Ok(selected) => selected,
                Err(diagnostics) => return diagnostics,
            };
            selected.into_iter().filter_map(|obligation| {
                if obligation.proven(program, facts) { return None; }
                let disposition = facts.contextual_cleanup.dispositions.get(obligation.disposition);
                if let Some((place, expected)) = obligation.transportable_subject(program) {
                    return Some(Diagnostic::error(format!(
                        "cannot prove automatic cleanup requires at selected edge from {} state {} after statement {}: missing {} == {} required by {} clause `{}`",
                        crate::published::labels::machine_name(program, disposition.machine),
                        program.symbols.name(disposition.state), obligation.statement,
                        place.label(program), expected,
                        crate::published::labels::machine_name(program, obligation.hook),
                        program.expression_table.display_name(obligation.clause),
                    )));
                }
                Some(Diagnostic::error(format!(
                    "cannot prove automatic cleanup requires at selected edge from {} state {} after statement {}: {} does not establish {} clause `{}`",
                    crate::published::labels::machine_name(program, disposition.machine),
                    program.symbols.name(disposition.state), obligation.statement,
                    obligation.place.label(program),
                    crate::published::labels::machine_name(program, obligation.hook),
                    program.expression_table.display_name(obligation.clause),
                )))
            }).collect()
        }

        fn transport(
            program: &dyn ::validation::CheckingInput,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            predicate: &BoundPredicate,
            antecedent: CheckedBooleanExpression,
            hook_machine: SymbolHandle,
            hook_clause: ExpressionHandle,
            template_machine: SymbolHandle,
        ) -> Option<Pending> {
            /*
                `bound_clause` binds the clause in the hook's own parameter namespace
                and then rebases every observation onto the dying place, which is
                storage of `machine`. The entry projection below reads only those
                rebased places, so the proposition is re-owned by the machine whose
                invocation values it is about to name. Left under the hook's identity
                it was refused as a foreign predicate, and no generic death edge
                could publish its prerequisite to callers.
            */
            let owner = program.machine_by_symbol(machine)?;
            let entry = validation::activation_target_state_index(program, owner, machine)?;
            let predicate = BoundPredicate {
                machine,
                entry_state: program.machine_states(owner).get(entry)?.symbol,
                expression: predicate.expression.clone(),
                bindings: predicate.bindings.clone(),
            };
            let consequence = boolean_predicates::entry_expression(
                program, machine, state, statement, &predicate,
            )?;
            Some(Pending {
                machine,
                consequence,
                antecedent,
                hook_machine,
                hook_clause,
                template_machine,
            })
        }

        fn append(
            requirements: &mut ContextualCleanupEntryRequirements,
            row: Pending,
        ) -> Result<bool, Vec<Diagnostic>> {
            for (_, prior) in requirements.rows.iter() {
                if prior.machine == row.machine && prior.antecedent == row.antecedent {
                    let opposed = matches!(&prior.consequence,
                        CheckedBooleanExpression::Not(subject) if subject.as_ref() == &row.consequence)
                        || matches!(&row.consequence,
                            CheckedBooleanExpression::Not(subject) if subject.as_ref() == &prior.consequence);
                    if opposed && row.antecedent == CheckedBooleanExpression::Constant(true) {
                        return Err(vec![Diagnostic::error(
                            "contextual cleanup has contradictory prerequisites for the same invocation-entry place",
                        )]);
                    }
                    if prior.consequence == row.consequence {
                        return Ok(false);
                    }
                }
            }
            requirements.rows.append(ContextualCleanupEntryRequirement {
                machine: row.machine,
                consequence: row.consequence,
                antecedent: row.antecedent,
                hook_machine: row.hook_machine,
                hook_clause: row.hook_clause,
                template_machine: row.template_machine,
            });
            Ok(true)
        }

        fn covers(symbolic: &[PlaceSegment], concrete: &[PlaceSegment]) -> bool {
            symbolic.len() <= concrete.len()
                && symbolic
                    .iter()
                    .zip(concrete)
                    .all(|(left, right)| match (*left, *right) {
                        (
                            PlaceSegment::FixedRange { start, end },
                            PlaceSegment::FixedIndex { index },
                        ) => (start..end).contains(&index),
                        (
                            PlaceSegment::FixedRange { start, end },
                            PlaceSegment::FixedRange {
                                start: right_start,
                                end: right_end,
                            },
                        ) => start <= right_start && right_end <= end,
                        _ => left == right,
                    })
        }

        fn has_symbolic_origin(
            program: &dyn ::validation::CheckingInput,
            state: &FlowStateFact,
            statement: usize,
            place: &CanonicalPlace,
        ) -> bool {
            let Some(root) = place.root.as_symbol() else {
                return false;
            };
            let root = super::source_symbol(program, root, state.machine_symbol);
            let source_state =
                super::source_symbol(program, state.state_symbol, state.machine_symbol);
            let path = super::source_path(program, &place.segments, state.machine_symbol);
            for length in 0..=path.len() {
                let source = CanonicalPlace {
                    root: PlaceRoot::Symbol(root),
                    segments: path[..length].to_vec(),
                };
                if let Some(reference) = validation::canonical_place_type_reference(
                    program,
                    source_state,
                    statement,
                    &source,
                ) {
                    let reference = program
                        .type_reference_table
                        .unconstrained_type_reference(reference);
                    if let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(reference)
                        && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
                    {
                        return true;
                    }
                }
            }
            false
        }

        pub(crate) fn occurrence_contexts(
            program: &dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            state: &FlowStateFact,
            disposition: &checked_trees::ContextualCleanupDisposition,
        ) -> Option<Vec<(usize, Vec<FactContextHandle>)>> {
            if disposition.completion.is_valid() {
                let PermissionEventSource::Statement { statement_index } = disposition.source
                else {
                    return None;
                };
                facts.flow.state_statement(state, statement_index)?;
                let constraints = disposition.completion_constraints?;
                facts.flow.contexts.constraint_refs.span(constraints)?;
                return Some(vec![(
                    statement_index,
                    facts
                        .flow
                        .semantic_constraint_contexts(constraints)
                        .collect(),
                )]);
            }
            if disposition.selection.is_valid() {
                let receipt = facts
                    .flow
                    .ownership
                    .owned_selections
                    .get(disposition.selection);
                if receipt.machine != disposition.machine
                    || receipt.state != disposition.state
                    || !facts
                        .flow
                        .ownership
                        .selection_sources
                        .span_or_empty(receipt.sources)
                        .iter()
                        .any(|source| Some(source.symbol) == disposition.root.as_symbol())
                {
                    return None;
                }
                let statement = receipt.statement_ordinal as usize;
                if !facts.flow.control.operands.is_valid(receipt.value) {
                    return None;
                }
                let value = facts.flow.control.operands.get(receipt.value);
                if value.state_symbol != receipt.state
                    || value.statement_index != statement
                    || value.expression != receipt.expression
                    || value.type_reference != receipt.type_reference
                {
                    return None;
                }
                return Some(vec![(
                    statement,
                    facts
                        .flow
                        .semantic_constraint_contexts(value.constraints)
                        .collect(),
                )]);
            }
            let points = match disposition.source {
                PermissionEventSource::InstructionDisposition { .. }
                | PermissionEventSource::InstructionExitDisposition { .. } => return None,
                PermissionEventSource::StateEntry => vec![(
                    0,
                    facts
                        .flow
                        .semantic_constraint_contexts(state.entry_constraints)
                        .collect(),
                )],
                PermissionEventSource::Statement { statement_index } => facts
                    .flow
                    .state_statement(state, statement_index)
                    .map(|statement| {
                        vec![(
                            statement_index,
                            facts
                                .flow
                                .semantic_constraint_contexts(statement.pre_store_constraints)
                                .collect(),
                        )]
                    })
                    .unwrap_or_default(),
                PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol,
                } => crate::flow::call_flows(facts.flow.control, state.calls)
                    .iter()
                    .filter(|call| {
                        call.statement_index == statement_index
                            && call.call_ordinal == call_ordinal
                            && call.target_symbol == target_symbol
                    })
                    .map(|call| {
                        (
                            statement_index,
                            facts
                                .flow
                                .semantic_constraint_contexts(call.exit_constraints)
                                .collect(),
                        )
                    })
                    .collect(),
                PermissionEventSource::StateExit => {
                    let coordinate = disposition.exit;
                    if coordinate.state_symbol != state.state_symbol
                        || !coordinate.state_symbol.is_valid()
                    {
                        return None;
                    }
                    let target = match coordinate.kind {
                        checked_trees::FlowClaimJoinExitKind::FinalExpression => Default::default(),
                        checked_trees::FlowClaimJoinExitKind::TransitionTarget
                        | checked_trees::FlowClaimJoinExitKind::TransitionContinuation => {
                            let typed_state = program.state_by_symbol(state.state_symbol)?;
                            let Some(typed_trees::statement::StatementNode::Transition(transition)) =
                                program
                                    .statement_table
                                    .statement_at(typed_state, coordinate.statement_index)
                            else {
                                return None;
                            };
                            let target = if coordinate.kind
                                == checked_trees::FlowClaimJoinExitKind::TransitionTarget
                            {
                                transition.target
                            } else {
                                transition.continuation
                            };
                            if !target.is_valid() {
                                return None;
                            }
                            target
                        }
                        checked_trees::FlowClaimJoinExitKind::CallResult { .. }
                        | checked_trees::FlowClaimJoinExitKind::SelectionArm { .. }
                        | checked_trees::FlowClaimJoinExitKind::SelectionDecision { .. } => {
                            return None;
                        }
                    };
                    if target.is_valid()
                        && let typed_trees::statement::TransitionTargetNode::Named {
                            path,
                            arguments,
                            ..
                        } = program.statement_table.transition_target(target)
                    {
                        let arguments = program.statement_table.expression_handles(*arguments);
                        let mut calls = state.calls.handles().filter(|handle| {
                            let call = facts.flow.control.calls.get(*handle);
                            call.statement_index == coordinate.statement_index
                                && call.authored_site
                                    == checked_trees::FlowCallSite::NamedTransition
                                && call.authored_target == path.symbol
                                && facts
                                    .flow
                                    .control
                                    .call_arguments
                                    .span_or_empty(call.authored_arguments)
                                    == arguments
                                && (facts
                                    .borrow
                                    .argument_accesses
                                    .span_or_empty(call.accesses)
                                    .iter()
                                    .any(|access| {
                                        access.root == disposition.root
                                            && access.end_before_access.is_none()
                                    })
                                    || facts.borrow.call_loan_resources.iter().any(
                                        |(_, resource)| {
                                            resource.call == *handle
                                                && resource.machine_symbol == state.machine_symbol
                                                && resource.state_symbol == state.state_symbol
                                                && resource.parent_lifetime.root == disposition.root
                                                && call.argument_operands().handles().any(
                                                    |operand| operand == resource.owner_operand,
                                                )
                                        },
                                    ))
                        });
                        if let Some(call) = calls.next() {
                            if calls.next().is_some() {
                                return None;
                            }
                            let call = facts.flow.control.calls.get(call);
                            return Some(vec![(
                                coordinate.statement_index,
                                facts
                                    .flow
                                    .semantic_constraint_contexts(call.exit_constraints)
                                    .collect(),
                            )]);
                        }
                    }
                    facts
                        .flow
                        .control
                        .outgoing
                        .span_or_empty(state.outgoing)
                        .iter()
                        .filter(|exit| {
                            exit.statement_index == coordinate.statement_index
                                && exit.transition_target == target
                        })
                        .map(|exit| {
                            (
                                exit.statement_index,
                                facts
                                    .flow
                                    .semantic_constraint_contexts(exit.constraints)
                                    .collect(),
                            )
                        })
                        .collect()
                }
            };
            (!points.is_empty()).then_some(points)
        }
    }
    mod sequence {
        /*
            Current-value facts between successive selected cleanup hooks.

            A death edge starts with its ordinary flow contexts. Each hook then has the
            same contract lifecycle as a call: prove prerequisites, invalidate its
            transitive storage writes, and admit its normal guarantees. No source call
            coordinate or independently replayed ownership trace is manufactured here.
            Effect discovery compresses array paths; declaration geometry supplies the
            exact parent-first, reverse-child execution order when premises are checked.
            The selected receiver's captured path identifies its storage; ordering uses
            geometry inside that value, not the runtime selector that located it.
        */
        use crate::flow;
        use validation::CanonicalPlace;

        use crate::flow::{CleanupLeaf, StateMutationSummaryCache};
        use crate::published::facts::boolean_predicates::{self, BoundPredicate};
        use checked_trees::{CheckFacts, CheckedBooleanExpression, ContractProofFactKind};
        use facts::{FactContextHandle, PlaceRoot, PlaceSegment, ScalarValue};
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;

        #[derive(Clone, Default)]
        pub(super) struct CurrentCleanupFacts {
            pub contexts: Vec<FactContextHandle>,
            pub guarantees: Vec<BoundPredicate>,
            entry_contexts: Vec<FactContextHandle>,
            writes: Vec<CanonicalPlace>,
            opaque_write: bool,
        }

        impl CurrentCleanupFacts {
            pub(super) fn new(
                contexts: Vec<FactContextHandle>,
                guarantees: Vec<BoundPredicate>,
            ) -> Self {
                let mut current_guarantees = Vec::new();
                for predicate in guarantees {
                    append_guarantee_conjuncts(&mut current_guarantees, predicate);
                }
                Self {
                    entry_contexts: contexts.clone(),
                    contexts,
                    guarantees: current_guarantees,
                    ..Self::default()
                }
            }

            pub(super) fn value_at_place(
                &self,
                program: &dyn ::validation::CheckingInput,
                facts: &CheckFacts,
                point: facts::ProgramPoint,
                place: &CanonicalPlace,
            ) -> Option<ScalarValue> {
                if self.opaque_write
                    || self.writes.iter().any(|write| {
                        ::validation::storage_path_relations::canonical_places_may_overlap(
                            program, place, write,
                        )
                    })
                {
                    return None;
                }
                ordinary_value(program, facts, &self.entry_contexts, point, place)
            }

            /* Entry transport cannot repair a premise killed by an earlier hook. */
            pub(super) fn entry_stable(
                &self,
                program: &dyn ::validation::CheckingInput,
                predicate: &BoundPredicate,
            ) -> bool {
                !self.opaque_write
                    && predicate.bindings.iter().all(|binding| {
                        !self.writes.iter().any(|write| {
                            ::validation::storage_path_relations::canonical_places_may_overlap(
                                program,
                                &binding.place,
                                write,
                            )
                        })
                    })
            }

            /*
                A candidate owner may already have transferred. Preserve only facts
                shared by executing its complete cleanup and skipping that cleanup;
                possible writes still invalidate all later prerequisites.
            */
            pub(super) fn join_skipped(&mut self, skipped: &Self) {
                self.contexts
                    .retain(|context| skipped.contexts.contains(context));
                self.guarantees
                    .retain(|predicate| skipped.guarantees.contains(predicate));
                self.opaque_write |= skipped.opaque_write;
                for write in &skipped.writes {
                    if !self.writes.contains(write) {
                        self.writes.push(write.clone());
                    }
                }
            }

            pub(super) fn advance(
                &mut self,
                program: &dyn ::validation::CheckingInput,
                facts: &CheckFacts,
                summaries: &StateMutationSummaryCache,
                call_frames: Option<&validation::CallFrameResolver<'_>>,
                hook: SymbolHandle,
                place: &CanonicalPlace,
            ) -> Option<()> {
                let machine = program.machine_by_symbol(hook)?;
                let state = program.machine_states(machine).first()?;
                let receiver = program.state_parameters(state).first()?;
                let writes = flow::state_storage_writes(
                    program,
                    &facts.borrow,
                    summaries,
                    state,
                    call_frames,
                )
                .and_then(|writes| {
                    writes
                        .into_iter()
                        .map(|write| rebase_place(receiver.symbol, place, write))
                        .collect::<Option<Vec<_>>>()
                });
                if let Some(writes) = writes {
                    self.contexts.retain(|context| {
                        flow::context_survives_place_mutations(
                            program,
                            &facts.semantic,
                            &facts.domains,
                            *context,
                            &writes,
                        )
                    });
                    self.guarantees.retain(|predicate| {
                        predicate.bindings.iter().all(|binding| {
                            !writes.iter().any(|write| {
                                ::validation::storage_path_relations::canonical_places_may_overlap(
                                    program,
                                    &binding.place,
                                    write,
                                )
                            })
                        })
                    });
                    self.writes.extend(writes);
                } else {
                    self.contexts.clear();
                    self.guarantees.clear();
                    self.opaque_write = true;
                }
                let clauses =
                    crate::execution::terminal_unit::cleanup::checked_contract_expressions(
                        program,
                        facts,
                        hook,
                        state.symbol,
                        ContractProofFactKind::Ensures,
                    )?;
                for clause in clauses {
                    if let Some(predicate) = bound_clause(
                        program,
                        facts,
                        hook,
                        clause,
                        ContractProofFactKind::Ensures,
                        place,
                    ) {
                        append_guarantee_conjuncts(&mut self.guarantees, predicate);
                    }
                }
                Some(())
            }
        }

        /*
            A checked conjunction establishes each conjunct independently. Retain each
            conjunct's exact observation bindings so a child hook's writes cannot erase
            a guarantee about an untouched sibling. Cross-field relations still carry
            both places, and disjunctions remain one proposition. These are admitted
            Boolean formulas, not source operator spellings or assumed body effects.
        */
        fn append_guarantee_conjuncts(
            guarantees: &mut Vec<BoundPredicate>,
            mut predicate: BoundPredicate,
        ) {
            if let CheckedBooleanExpression::And { left, right } = &predicate.expression {
                let mut first = predicate.clone();
                first.expression = *left.clone();
                let second = *right.clone();
                append_guarantee_conjuncts(guarantees, first);
                predicate.expression = second;
                append_guarantee_conjuncts(guarantees, predicate);
            } else if let Some(predicate) = boolean_predicates::fold(&predicate, |_| None) {
                guarantees.push(predicate);
            }
        }

        fn ordinary_value(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            contexts: &[FactContextHandle],
            point: facts::ProgramPoint,
            place: &CanonicalPlace,
        ) -> Option<ScalarValue> {
            crate::checks::contracts::boolean_value_at_place(
                program,
                &facts.operators,
                &facts.semantic,
                contexts,
                point,
                place,
            )
            .map(ScalarValue::Boolean)
            .or_else(|| {
                crate::values::scalar_value_at_place(
                    program,
                    &facts.semantic,
                    contexts
                        .iter()
                        .map(|context| facts.semantic.contexts.get(*context)),
                    place,
                )
            })
        }

        pub(super) fn bound_clause(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            hook: SymbolHandle,
            clause: ExpressionHandle,
            kind: ContractProofFactKind,
            place: &CanonicalPlace,
        ) -> Option<BoundPredicate> {
            let machine = program.machine_by_symbol(hook)?;
            let state = program.machine_states(machine).first()?;
            let parameters = program.state_parameters(state);
            let receiver = parameters.first()?;
            let expression = match kind {
                ContractProofFactKind::Requires => {
                    crate::values::lower_state_scalar_contract_predicate(
                        program,
                        &facts.operators,
                        machine,
                        state,
                        clause,
                        &mut 4096,
                    )
                }
                ContractProofFactKind::Ensures => crate::values::lower_scalar_contract_predicate(
                    program,
                    &facts.operators,
                    machine,
                    clause,
                    true,
                    &mut 4096,
                ),
            }?;
            let mut predicate = boolean_predicates::bind_parameters(
                program,
                hook,
                state.symbol,
                parameters,
                None,
                expression,
            )?;
            for binding in &mut predicate.bindings {
                binding.place = rebase_place(receiver.symbol, place, binding.place.clone())?;
            }
            Some(predicate)
        }

        fn rebase_place(
            receiver: SymbolHandle,
            place: &CanonicalPlace,
            mut formal: CanonicalPlace,
        ) -> Option<CanonicalPlace> {
            if formal.root != PlaceRoot::Symbol(receiver) {
                return None;
            }
            let mut segments = place.segments.clone();
            segments.append(&mut formal.segments);
            Some(CanonicalPlace {
                root: place.root,
                segments,
            })
        }

        #[derive(Clone)]
        pub(super) struct SelectedHook {
            pub machine: SymbolHandle,
            pub path: Vec<PlaceSegment>,
            order: Vec<usize>,
        }

        /*
            Expand only the already selected live receivers, counting before allocating.
            Ancestors sort before their descendants; field/index ordinals come from
            declaration geometry, never symbol allocation or rendered names. Every leaf
            retains the selected receiver prefix for predicate substitution. Its shared
            fields, view ranges and computed selectors are not relative ordering
            coordinates. Only ranges below the disposed value select several child
            receivers. Execution uses that same selected-prefix/relative-child split;
            expanding a captured view prefix would invent several cleanup invocations.
        */
        pub(super) fn ordered_hooks(
            program: &dyn ::validation::CheckingInput,
            selected_path: &[PlaceSegment],
            leaves: &[CleanupLeaf],
        ) -> Option<Vec<SelectedHook>> {
            let mut hooks = Vec::new();
            for leaf in leaves {
                let CleanupLeaf::Nominal { machine, path } = leaf else {
                    continue;
                };
                let relative_path = path.strip_prefix(selected_path)?;
                let paths = flow::concrete_cleanup_paths(relative_path)?;
                hooks.try_reserve_exact(paths.len()).ok()?;
                for relative_path in paths {
                    let order = flow::cleanup_path_order(program, &relative_path)?;
                    let mut path = selected_path.to_vec();
                    path.extend(relative_path);
                    hooks.push(SelectedHook {
                        machine: *machine,
                        path,
                        order,
                    });
                }
            }
            hooks.sort_by(|left, right| left.order.cmp(&right.order));
            Some(hooks)
        }
    }
    pub(crate) use receiver_loans::retain_selected_receiver_loans;
    pub(crate) use requirements::{
        check_cleanup_requirements, infer_entry_requirements, occurrence_contexts,
    };

    use arena::Arena;
    use checked_trees::{ContextualCleanupApplication, ContextualCleanupFacts, FlowFacts};
    use facts::{PlaceRoot, PlaceSegment};
    use language_semantics::PermissionEventSource;
    use symbols::SymbolHandle;

    pub(crate) fn bind_applications(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        cleanup: &mut ContextualCleanupFacts,
    ) {
        let mut applications = Arena::default();
        for (instance_handle, instance) in cleanup.dispositions.iter() {
            let Some(specialization) = program.machine_specialization(instance.machine) else {
                continue;
            };
            let state = source_symbol(program, instance.state, instance.machine);
            let Some(root) = instance.root.as_symbol() else {
                continue;
            };
            let root = PlaceRoot::Symbol(source_symbol(program, root, instance.machine));
            let path = source_path(
                program,
                flow.ownership.segments.span_or_empty(instance.path),
                instance.machine,
            );
            let source = source_occurrence(program, instance.source);
            for (template_handle, template) in
                cleanup.dispositions.iter().filter(|(_, template)| {
                    template.machine == specialization.template
                        && template.state == state
                        && template.root == root
                        && template.source == source
                        && template.exit.state_symbol
                            == source_symbol(program, instance.exit.state_symbol, instance.machine)
                        && template.exit.statement_index == instance.exit.statement_index
                        && template.exit.kind == instance.exit.kind
                        && flow.ownership.segments.span_or_empty(template.path) == path
                })
            {
                let corresponding_regions = [
                    (template.exclusions, instance.exclusions),
                    (template.inactive_cases, instance.inactive_cases),
                ]
                .into_iter()
                .all(|(template_regions, instance_regions)| {
                    let template_exclusions = cleanup.exclusions.span_or_empty(template_regions);
                    let instance_exclusions = cleanup.exclusions.span_or_empty(instance_regions);
                    template_exclusions.len() == instance_exclusions.len()
                        && template_exclusions.iter().zip(instance_exclusions).all(
                            |(template, instance)| {
                                let Some(instance_root) = instance.root.as_symbol() else {
                                    return false;
                                };
                                template.cause == instance.cause
                                    && template.root
                                        == PlaceRoot::Symbol(source_symbol(
                                            program,
                                            instance_root,
                                            specialization.instance,
                                        ))
                                    && cleanup.segments.span_or_empty(template.segments)
                                        == source_path(
                                            program,
                                            cleanup.segments.span_or_empty(instance.segments),
                                            specialization.instance,
                                        )
                            },
                        )
                });
                if !corresponding_regions {
                    continue;
                }
                applications.append(ContextualCleanupApplication {
                    template: template_handle,
                    instance: instance_handle,
                });
            }
        }
        cleanup.applications = applications;
    }

    fn source_symbol(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
        instance: SymbolHandle,
    ) -> SymbolHandle {
        let mut scope = symbol;
        while scope.is_valid() && scope != instance {
            scope = program.symbols.get(scope).parent;
        }
        if scope != instance {
            return symbol;
        }
        let source = program.symbols.get(symbol).generated_from;
        if source.is_valid() && program.symbols.get(source).kind == program.symbols.get(symbol).kind
        {
            source
        } else {
            symbol
        }
    }

    fn source_path(
        program: &dyn ::validation::CheckingInput,
        path: &[PlaceSegment],
        instance: SymbolHandle,
    ) -> Vec<PlaceSegment> {
        path.iter()
            .map(|segment| match *segment {
                PlaceSegment::Field { symbol } => PlaceSegment::Field {
                    symbol: source_symbol(program, symbol, instance),
                },
                PlaceSegment::Case { variant } => PlaceSegment::Case {
                    variant: source_symbol(program, variant, instance),
                },
                other => other,
            })
            .collect()
    }

    fn source_occurrence(
        program: &dyn ::validation::CheckingInput,
        source: PermissionEventSource,
    ) -> PermissionEventSource {
        match source {
            PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol,
            } => {
                let source = program.symbols.get(target_symbol).generated_from;
                PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol: if source.is_valid()
                        && program.symbols.get(source).kind
                            == program.symbols.get(target_symbol).kind
                    {
                        source
                    } else {
                        target_symbol
                    },
                }
            }
            other => other,
        }
    }
}
pub(crate) mod contract_occurrences {
    use crate::flow::expressions;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    /*
        Return each structural value occurrence referenced by one contract fact.

        A member or indexed expression is one complete place. Runtime index
        expressions are additional occurrences because their evaluated values
        select which place the fact describes and must therefore participate in
        revision invalidation too.
    */
    pub(crate) fn fact_referenced_occurrences(
        program: &dyn ::validation::CheckingInput,
        fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> Vec<ExpressionHandle> {
        fact_referenced_occurrences_in_state(program, fact, None)
    }

    /*
        Declaration-owned requirements may omit a private descriptor's payload
        dependency. Call substitution keeps the unfiltered occurrences: a stable
        callee formal does not freeze the caller's replaceable descriptor home.
    */
    pub(crate) fn fact_referenced_occurrences_in_state(
        program: &dyn ::validation::CheckingInput,
        fact: arena::Handle<typed_trees::domain::ProofFact>,
        state: Option<symbols::SymbolHandle>,
    ) -> Vec<ExpressionHandle> {
        let mut occurrences = Vec::new();
        match program.proof_facts.get(fact) {
            typed_trees::domain::ProofFact::Expression(expression) => {
                append_occurrences(program, *expression, state, &mut occurrences);
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                append_occurrences(program, membership.value, state, &mut occurrences);
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                for argument in expressions(program).expression_handles(application.arguments) {
                    append_occurrences(program, *argument, state, &mut occurrences);
                }
            }
        }
        occurrences.sort_by_key(|expression| expression.arena_index());
        occurrences.dedup();
        occurrences
    }

    pub(crate) fn append_expression_occurrences(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        occurrences: &mut Vec<ExpressionHandle>,
    ) {
        append_occurrences(program, expression, None, occurrences);
    }

    fn append_occurrences(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        state: Option<symbols::SymbolHandle>,
        occurrences: &mut Vec<ExpressionHandle>,
    ) {
        if !expression.is_valid() {
            return;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Match(dispatch) => {
                append_occurrences(program, dispatch.subject, state, occurrences);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                        append_occurrences(program, pattern, state, occurrences);
                    }
                    append_occurrences(program, arm.value, state, occurrences);
                }
            }
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant => {}
            ExpressionNode::Name(_) => occurrences.push(expression),
            ExpressionNode::Member(member) => {
                let measure =
                    validation::expression_place_type_reference(program, member.receiver, &[])
                        .and_then(|receiver| {
                            validation::collection_operations::collection_measure_member(
                                program,
                                member,
                                validation::collection_operations::MeasureReceiver::Declared(
                                    receiver,
                                ),
                            )
                        });
                if measure
                    == Some(language_semantics::declaration_selection::CollectionMeasure::Length)
                    && state.is_some_and(|state| {
                        stable_descriptor_occurrence(program, member.receiver, state)
                    })
                {
                    return;
                }
                if measure.is_some()
                    || matches!(
                        program.expression_table.expression(member.receiver),
                        ExpressionNode::Call(_)
                    )
                {
                    append_occurrences(program, member.receiver, state, occurrences);
                } else {
                    occurrences.push(expression);
                    append_selector_occurrences(program, member.receiver, occurrences);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                occurrences.push(expression);
                append_selector_occurrences(program, indexed.collection, occurrences);
                append_occurrences(program, indexed.index, state, occurrences);
            }
            ExpressionNode::Borrow(inner) => {
                append_occurrences(program, inner.target, state, occurrences);
            }
            ExpressionNode::Atomic(atomic) => {
                append_occurrences(program, atomic.value, state, occurrences);
                append_occurrences(program, atomic.result, state, occurrences);
            }
            ExpressionNode::Binary(binary) => {
                append_occurrences(program, binary.left, state, occurrences);
                append_occurrences(program, binary.right, state, occurrences);
            }
            ExpressionNode::Cast(cast) => {
                append_occurrences(program, cast.value, state, occurrences);
            }
            ExpressionNode::Call(call) => {
                append_occurrences(program, call.receiver, state, occurrences);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    append_occurrences(program, *argument, state, occurrences);
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in program.expression_table.expression_handles(*values) {
                    append_occurrences(program, *value, state, occurrences);
                }
            }
            ExpressionNode::Range(range) => {
                append_occurrences(program, range.start, state, occurrences);
                append_occurrences(program, range.end, state, occurrences);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    append_occurrences(program, field.value, state, occurrences);
                }
            }
            ExpressionNode::Unary(unary) => {
                append_occurrences(program, unary.operand, state, occurrences);
            }
            ExpressionNode::TypeExpression(_)
            | typed_trees::spelled_literal_expression_pattern!() => {}
        }
    }

    fn stable_descriptor_occurrence(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        state_symbol: symbols::SymbolHandle,
    ) -> bool {
        let Some(place) = validation::canonical_place_from_expression(program, expression) else {
            return false;
        };
        let Some(root) = place.root.as_symbol() else {
            return false;
        };
        let parent = program.symbols.get(root).parent;
        parent == state_symbol
            && validation::find_state(program, parent).is_some_and(|(_, state)| {
                validation::stable_slice_descriptor_place(program, state, root, &place.segments)
                    .is_some()
            })
    }

    /*
        A selected field remains one complete storage dependency, but selectors
        beneath its receiver are independent values that can redirect the read.
        Non-place producers compute a completed value from their operands, so their
        ordinary value dependencies must also survive any later source replay.
    */
    pub(crate) fn append_selector_occurrences(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        occurrences: &mut Vec<ExpressionHandle>,
    ) {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                append_selector_occurrences(program, member.receiver, occurrences)
            }
            ExpressionNode::Indexed(indexed) => {
                append_selector_occurrences(program, indexed.collection, occurrences);
                append_expression_occurrences(program, indexed.index, occurrences);
            }
            ExpressionNode::Borrow(borrow) => {
                append_selector_occurrences(program, borrow.target, occurrences)
            }
            ExpressionNode::Name(_)
            | ExpressionNode::TypeExpression(_)
            | typed_trees::literal_expression_pattern!() => {}
            _ => append_expression_occurrences(program, expression, occurrences),
        }
    }
}
mod contract_plan_facts {
    /*
        Contract plans, resource envelope coverage, mutation facts and closed
        scalar value contract plans.
    */
    use crate::published::facts::crash_plan_facts::{
        build_crash_contract_capsules, build_published_crash_plan, machine_entry_parameter_names,
    };
    use crate::published::facts::{crash_calls, operator_crashes};
    use validation::contract_structure::{
        callable_binders, declared_callable, declared_contracts, declared_termination,
        generic_binders,
    };

    /*
        Normalization scratch retained until every generated contract's envelope
        settles. Publishing a head before its member graph closes would commit an
        empty crash permission set and then silently change that public contract.
    */
    struct ContractPublication<'program> {
        type_identities: &'program typed_trees::type_identity::TypeIdentityTable,
        supply: language_semantics::MachineSupplyMode,
        external_binding: Option<language_semantics::ExternalBindingIdentity>,
        services: Vec<String>,
        invocations: language_semantics::SynchronousInvocationInterface,
        published_invocations: Vec<String>,
        suspension: language_semantics::SuspensionInterface,
        blocking: language_semantics::BlockingInterface,
        termination: checked_trees::DeclaredTermination,
        declaration: checked_trees::DeclaredContractStructure,
    }

    impl ContractPublication<'_> {
        fn identity(
            &self,
            crashes: checked_trees::CrashInterface,
        ) -> checked_trees::MachineContractIdentity {
            checked_trees::contract_identity(
                self.supply,
                self.external_binding.as_ref(),
                &self.services,
                self.invocations,
                &self.published_invocations,
                self.suspension,
                self.blocking,
                crashes,
                &self.termination,
                &self.declaration,
                self.type_identities,
            )
        }
    }

    /*
        Add only causes inferred from private members, leaving authored head routes
        guarded. The prepared graph excludes proof pruning: a stronger prover must
        not narrow the published interface. Invocation-specific refinement happens
        later against this finalized ceiling, never against a lexical body.
    */
    fn publish_family_crash_envelopes(
        contracts: &mut [checked_trees::MachineContractPlan],
        publications: &mut [ContractPublication<'_>],
        additions: &[(symbols::SymbolHandle, Vec<checked_trees::CrashCause>)],
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        for (head, causes) in additions {
            let position = contracts
                .iter()
                .position(|contract| contract.machine == *head)
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "target-family crash envelope has no owning contract",
                    )]
                })?;
            let publication = publications.get_mut(position).ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "target-family crash envelope has no declaration publication",
                )]
            })?;
            let contract = &mut contracts[position];
            let mut published = contract.crash.published().to_vec();
            for cause in causes {
                published.retain(|bucket| bucket.cause() != *cause);
                published.push(checked_trees::CrashRouteBucket::unconditional(*cause));
                publication.declaration.clauses.retain(|clause| {
                    clause.kind != checked_trees::DeclaredContractKind::Crashes(*cause)
                });
                publication
                    .declaration
                    .clauses
                    .push(checked_trees::DeclaredContractClause {
                        kind: checked_trees::DeclaredContractKind::Crashes(*cause),
                        fact: None,
                    });
            }
            publication.declaration.clauses.sort();
            publication.declaration.clauses.dedup();
            contract.crash = contract.crash.clone().with_published_ceiling(published);
        }
        Ok(())
    }

    /*
        STR4 checked plans (wiki/spec/language/machines.md): assemble each machine's
        normalized contract plan from the published halves already carried on
        the records (supply mode, service/operational ceilings, published termination),
        with a deterministic fingerprint over them. Checked service rows include
        conservative call propagation, without proof-based pruning; a stronger
        prover therefore cannot change an exported contract ID (acceptance 8).
    */
    pub(crate) fn build_contract_plans(
        program: &dyn ::validation::CheckingInput,
        proof: &checked_trees::ProofFacts,
        service_reaches: &checked_trees::ServiceReachFacts,
        synchronous_invocations: &checked_trees::SynchronousInvocationFacts,
        suspensions: &checked_trees::SuspensionFacts,
        blocking: &checked_trees::BlockingFacts,
        termination: &checked_trees::TerminationFacts,
        mutation: &checked_trees::MutationFacts,
        capabilities: &flow_effects::CapabilityFlowPlan,
        flow: &checked_trees::FlowFacts,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &facts::FactPlan,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
        scalar_expressions: &checked_trees::CheckedScalarExpressionPlans,
        scalar_computations: &checked_trees::CheckedScalarComputationPlans,
        structural_values: &checked_trees::CheckedStructuralValuePlans,
    ) -> Result<
        (
            checked_trees::MachineContractPlans,
            crash_calls::CompletedBodyCrashSummaries,
        ),
        Vec<diagnostics::Diagnostic>,
    > {
        let content_conservation = validation::content_conservation_plans(program);
        let trapping_index = PrecomputedTrappingSites::build(
            scalar_expressions,
            scalar_computations,
            structural_values,
            operators,
        );
        let (mut machines, mut publications) = {
            use rayon::prelude::*;
            let results: Result<
                Vec<(checked_trees::MachineContractPlan, ContractPublication<'_>)>,
                Vec<diagnostics::Diagnostic>,
            > = validation::checking_thread_pool().install(|| {
                program
                    .machines()
                    .par_iter()
                    .enumerate()
                    .map(|(machine_index, machine)| {
                        let service_fact = service_reaches
                            .machines()
                            .get(machine_index)
                            .filter(|fact| fact.machine == machine.symbol)
                            .or_else(|| service_reaches.for_machine(machine.symbol));
                        let published_service_row = service_fact
                            .map(|fact| fact.published_ceiling)
                            .unwrap_or(language_semantics::ServiceReachRowTable::EMPTY_ROW);
                        let published_service_names = service_reaches
                            .rows
                            .services(published_service_row)
                            .iter()
                            .filter_map(|service| service_reaches.services.definition(*service))
                            .map(|definition| definition.name.clone())
                            .collect::<Vec<_>>();
                        let synchronous_invocation = synchronous_invocations
                            .machines
                            .get(machine_index)
                            .filter(|fact| fact.machine == machine.symbol)
                            .map(|fact| &fact.plan)
                            .or_else(|| synchronous_invocations.for_machine(machine.symbol))
                            .expect("every checked machine must publish synchronous invocation facts");
                        let suspension = suspensions
                            .machines
                            .get(machine_index)
                            .filter(|fact| fact.machine == machine.symbol)
                            .map(|fact| fact.plan)
                            .or_else(|| suspensions.for_machine(machine.symbol))
                            .expect("every checked machine must publish suspension facts");
                        let blocking = blocking
                            .machines
                            .get(machine_index)
                            .filter(|fact| fact.machine == machine.symbol)
                            .map(|fact| fact.plan)
                            .or_else(|| blocking.for_machine(machine.symbol))
                            .expect("every checked machine must publish blocking facts");
                        let termination = termination
                            .machines
                            .get(machine_index)
                            .filter(|fact| fact.machine == machine.symbol)
                            .map(|fact| &fact.plan)
                            .or_else(|| termination.for_machine(machine.symbol))
                            .expect("every checked machine must publish termination facts");
                        let generics = generic_binders(program.machine_type_parameters(machine));
                        let mut callables = program
                            .machine_states(machine)
                            .iter()
                            .map(|state| {
                                declared_callable(
                                    program,
                                    program.state_parameters(state),
                                    state.return_type,
                                    program.state_contracts(state),
                                    &generics,
                                    &content_conservation,
                                    true,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|diagnostic| vec![diagnostic])?;
                        callables.sort();
                        let parameter_names = machine_entry_parameter_names(program, machine);
                        let declaration_predicates =
                            validation::machine_declaration_predicates(program, operators, machine);
                        let crash = build_published_crash_plan(
                            program,
                            machine,
                            &parameter_names,
                            &content_conservation,
                            operators,
                            &declaration_predicates,
                            exact_integer_casts,
                        )
                        .map_err(|diagnostic| vec![diagnostic])?
                        .with_trapping_sites(trapping_index.sites_for_machine(program, machine)?);
                        let binders = program
                            .machine_states(machine)
                            .first()
                            .map(|entry| callable_binders(&generics, program.state_parameters(entry)))
                            .unwrap_or_else(|| generics.clone());
                        let declaration = checked_trees::DeclaredContractStructure {
                            callables,
                            clauses: declared_contracts(
                                program,
                                program.machine_contracts(machine),
                                &binders,
                                &content_conservation,
                                true,
                            )
                            .map_err(|diagnostic| vec![diagnostic])?,
                        };
                        let published_termination =
                            declared_termination(program, &termination.interface, &binders);
                        let closed_scalar_values =
                            build_closed_scalar_value_contract_plan(program, machine, operators);
                        let external_binding = match machine.supply_mode {
                            language_semantics::MachineSupplyMode::ExternalRealization {
                                binding: Some(binding),
                                ..
                            } => Some(
                                program
                                    .external_bindings
                                    .identity(binding)
                                    .cloned()
                                    .ok_or_else(|| {
                                        vec![diagnostics::Diagnostic::error(
                                            "checked external supply lost its owning binding identity",
                                        )]
                                    })?,
                            ),
                            _ => None,
                        };
                        let publication = ContractPublication {
                            type_identities: &program.type_identities,
                            supply: machine.supply_mode,
                            external_binding,
                            services: published_service_names,
                            invocations: synchronous_invocation.interface,
                            published_invocations: synchronous_invocation.published.clone(),
                            suspension: suspension.interface,
                            blocking: blocking.interface,
                            termination: published_termination,
                            declaration,
                        };
                        let plan = checked_trees::MachineContractPlan {
                            machine: machine.symbol,
                            closed_scalar_values,
                            crash,
                            restored_declaration_predicates: declaration_predicates
                                .into_iter()
                                .filter(|row| row.restored_on_return)
                                .map(|row| row.predicate)
                                .collect(),
                            report_fingerprint: 0,
                            commitment: checked_trees::MachineContractCommitment::from_digest([0; 32]),
                        };
                        Ok((plan, publication))
                    })
                    .collect()
            });
            results.map(|pairs| pairs.into_iter().unzip::<_, _, Vec<_>, Vec<_>>())
        }?;
        let operator_crashes = operator_crashes::build(program, operators, flow, semantic)?;
        let mut operator_crashes_by_machine = symbols::SymbolKeyMap::<
            symbols::SymbolHandle,
            Vec<checked_trees::CheckedCrashOperatorSite>,
        >::default();
        for (machine, site) in operator_crashes {
            operator_crashes_by_machine
                .entry(machine)
                .or_default()
                .push(site);
        }
        for plan in &mut machines {
            let sites = operator_crashes_by_machine
                .remove(&plan.machine)
                .unwrap_or_default();
            plan.crash = std::mem::take(&mut plan.crash)
                .with_checked_operators(sites)
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "invalid selected operator crash invocation roster",
                    )]
                })?;
        }
        if !operator_crashes_by_machine.is_empty() {
            return Err(vec![diagnostics::Diagnostic::error(
                "selected operator crash invocation has no owning machine contract",
            )]);
        }
        let crash_capsules =
            build_crash_contract_capsules(program, &content_conservation, operators)
                .map_err(|diagnostic| vec![diagnostic])?;
        let summary_graph = crash_calls::prepare_crash_summary_graph(
            program,
            proof,
            operators,
            flow,
            &crash_capsules,
            &machines,
            &content_conservation,
        )?;
        publish_family_crash_envelopes(
            &mut machines,
            &mut publications,
            summary_graph.family_potential_crash_causes(),
        )?;
        {
            use rayon::prelude::*;
            validation::checking_thread_pool().install(|| {
                machines
                    .par_iter_mut()
                    .zip(publications.par_iter())
                    .for_each(|(contract, publication)| {
                        let identity = publication.identity(contract.crash.interface());
                        contract.report_fingerprint = identity.report_fingerprint;
                        contract.commitment = identity.commitment;
                    });
            });
        };
        let completed_crashes = crash_calls::attach_checked_crash_calls(
            program,
            operators,
            exact_integer_casts,
            semantic,
            flow,
            &content_conservation,
            &crash_capsules,
            &mut machines,
            &summary_graph,
        );
        let mut capabilities_by_machine = symbols::SymbolKeyMap::<
            symbols::SymbolHandle,
            Vec<flow_effects::CapabilityFlowFact>,
        >::default();
        for flow in capabilities.flows() {
            capabilities_by_machine
                .entry(flow.machine_symbol)
                .or_default()
                .push(*flow);
        }
        let machine_capabilities = machines
            .iter()
            .map(|contract| {
                capabilities_by_machine
                    .remove(&contract.machine)
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let realized_envelopes = validation::checking_thread_pool().install(|| {
            use rayon::prelude::*;
            machines
                .par_iter()
                .zip(machine_capabilities)
                .enumerate()
                .map(|(machine_index, (contract, capability_rows))| {
                    let machine = program
                        .machines()
                        .get(machine_index)
                        .filter(|machine| machine.symbol == contract.machine)
                        .or_else(|| program.machine(contract.machine))
                        .expect("every contract must retain its exact typed machine");
                    let service_fact = service_reaches
                        .machines()
                        .get(machine_index)
                        .filter(|fact| fact.machine == contract.machine)
                        .or_else(|| service_reaches.for_machine(contract.machine))
                        .expect("every checked machine must retain service-reach facts");
                    let effective_service_reach = service_reaches
                        .rows
                        .services(service_fact.effective)
                        .iter()
                        .filter_map(|service| service_reaches.services.definition(*service))
                        .map(|definition| definition.name.clone())
                        .collect::<Vec<_>>();
                    let concrete_service_reach = service_reaches
                        .rows
                        .services(service_fact.concrete_effective)
                        .iter()
                        .filter_map(|service| service_reaches.services.definition(*service))
                        .map(|definition| definition.name.clone())
                        .collect::<Vec<_>>();
                    let invocation = synchronous_invocations
                        .machines
                        .get(machine_index)
                        .filter(|fact| fact.machine == contract.machine)
                        .map(|fact| &fact.plan)
                        .or_else(|| synchronous_invocations.for_machine(contract.machine))
                        .expect("every checked machine must retain invocation facts");
                    let suspension = suspensions
                        .machines
                        .get(machine_index)
                        .filter(|fact| fact.machine == contract.machine)
                        .map(|fact| fact.plan)
                        .or_else(|| suspensions.for_machine(contract.machine))
                        .expect("every checked machine must retain suspension facts");
                    let blocking = blocking
                        .machines
                        .get(machine_index)
                        .filter(|fact| fact.machine == contract.machine)
                        .map(|fact| fact.plan)
                        .or_else(|| blocking.for_machine(contract.machine))
                        .expect("every checked machine must retain blocking facts");
                    let termination = termination
                        .machines
                        .get(machine_index)
                        .filter(|fact| fact.machine == contract.machine)
                        .map(|fact| &fact.plan)
                        .or_else(|| termination.for_machine(contract.machine))
                        .expect("every checked machine must retain termination facts");
                    let mutation = mutation
                        .machines
                        .get(machine_index)
                        .filter(|fact| fact.machine == contract.machine)
                        .or_else(|| mutation.for_machine(contract.machine))
                        .map(|fact| fact.state_write_frames.clone())
                        .unwrap_or_default();
                    checked_trees::RealizedMachineContractEnvelope {
                    machine: contract.machine,
                    contract_report_fingerprint: contract.report_fingerprint,
                    contract_commitment: contract.commitment,
                    effective_service_reach,
                    concrete_service_reach,
                    unresolved_installation_reaches: service_fact
                        .unresolved_installation_reaches
                        .clone(),
                    effective_synchronous_invocations: invocation.checked_inferred.clone(),
                    checked_may_suspend: suspension.checked_may_suspend,
                    checked_may_block: blocking.checked_may_block,
                    checked_termination: termination.checked_summary.clone(),
                    checked_crash: contract.crash.clone(),
                    mutation,
                    capabilities: capability_rows,
                    resources:
                        checked_trees::CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                            contract.machine,
                            contract.report_fingerprint,
                            contract.commitment,
                            program.machine_states(machine).iter().map(|entry| entry.symbol),
                        ),
                }
                })
                .collect::<Vec<_>>()
        });
        let plans =
            checked_trees::MachineContractPlans::new(machines, crash_capsules, realized_envelopes);
        plans.validate_resource_envelopes().map_err(|error| {
            vec![diagnostics::Diagnostic::error(format!(
                "checked resource-envelope replay failed: {error}"
            ))]
        })?;
        validate_checked_resource_envelope_coverage(program, &plans)?;
        Ok((plans, completed_crashes))
    }

    /*
        Reconstruct the complete per-entry resource roster from typed ownership
        and declaration order. Structural checked validation above does not reopen
        typed trees, so this second gate is what rejects a missing, foreign, or
        reordered entry before the source-independent carrier leaves this stage.
    */
    fn validate_checked_resource_envelope_coverage(
        program: &dyn ::validation::CheckingInput,
        plans: &checked_trees::MachineContractPlans,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        for (machine_index, machine) in program.machines().iter().enumerate() {
            let contract = plans
                .machines()
                .get(machine_index)
                .filter(|contract| contract.machine == machine.symbol)
                .or_else(|| plans.for_machine(machine.symbol))
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "checked resource-envelope replay is missing an exact machine contract",
                    )]
                })?;
            let realized = plans
                .realized_envelopes
                .get(machine_index)
                .filter(|realized| realized.machine == machine.symbol)
                .or_else(|| plans.realized_envelope(machine.symbol))
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "checked resource-envelope replay is missing an exact realized machine row",
                    )]
                })?;
            let expected_entries = program.machine_states(machine);
            if realized.resources.len() != expected_entries.len() {
                return Err(vec![diagnostics::Diagnostic::error(
                    "checked resource-envelope replay does not cover every owned machine entry",
                )]);
            }
            for (resource, entry) in realized.resources.iter().zip(expected_entries) {
                let replayed = checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
                    machine.symbol,
                    entry.symbol,
                    contract.report_fingerprint,
                    contract.commitment,
                );
                if resource != &replayed {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "checked resource-envelope replay changed entry ownership or declaration order",
                    )]);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn build_mutation_facts(
        program: &dyn ::validation::CheckingInput,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> checked_trees::MutationFacts {
        let mut owned_resolver = None;
        let frame_resolver =
            crate::flow::shared_call_frames_or(call_frames, program, &mut owned_resolver);
        let machines = program
            .machines()
            .iter()
            .map(|machine| {
                let states = program.machine_states(machine);
                let frames = frame_resolver.map_or_else(
                    || {
                        (0..states.len())
                            .map(|_| facts::NormalizedWriteFrame::opaque())
                            .collect()
                    },
                    |resolver| resolver.inferred_machine_state_write_frames(machine),
                );
                checked_trees::MachineMutationFact {
                    machine: machine.symbol,
                    state_write_frames: states
                        .iter()
                        .zip(frames)
                        .map(|(state, frame)| checked_trees::StateWriteFramePlan {
                            state: state.symbol,
                            frame,
                        })
                        .collect(),
                }
            })
            .collect();
        checked_trees::MutationFacts {
            machines,
            ..Default::default()
        }
    }

    pub(crate) fn build_closed_scalar_value_contract_plan(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        operators: &checked_trees::CheckedOperatorFacts,
    ) -> checked_trees::ClosedScalarValueContractPlan {
        let parameters =
            crate::values::scalar::contract_entry::authored_entry_parameters(program, machine);
        let result_type = program
            .machine_states(machine)
            .first()
            .map(|state| state.return_type)
            .unwrap_or_default();
        build_callable_scalar_value_contract_plan(
            program,
            machine.symbol,
            parameters,
            result_type,
            program.machine_contracts(machine),
            operators,
        )
    }

    /*
        Executable bodies and opaque requirement signatures publish scalar facts
        through the same declared formal telescope. Provider selection must not
        replace an authored requirement with an empty contract, nor may publication
        infer its requirements from whichever provider happens to be selected.
    */
    pub(crate) fn build_callable_scalar_value_contract_plan(
        program: &dyn ::validation::CheckingInput,
        owner: symbols::SymbolHandle,
        parameters: Option<&[typed_trees::signature::StateParameter]>,
        result_type: typed_trees::types::TypeReferenceHandle,
        contracts: &[typed_trees::signature::SignatureContract],
        operators: &checked_trees::CheckedOperatorFacts,
    ) -> checked_trees::ClosedScalarValueContractPlan {
        use typed_trees::{
            domain::ProofFact,
            expression::{BinaryOperator, ExpressionNode},
            signature::SignatureContractKind,
        };

        let boolean_type = std::cell::Cell::new(None);
        let resolve_boolean_type = || {
            if let Some(cached) = boolean_type.get() {
                return cached;
            }
            let resolved = program.type_reference_table.named_references().find_map(
                |(type_reference, symbol, _)| {
                    (program.symbols.builtin_type_atom(symbol)
                        == Some(symbols::BuiltinTypeAtom::Bool))
                    .then_some(type_reference)
                },
            );
            boolean_type.set(Some(resolved));
            resolved
        };

        let lower_closed_clause = |contract: &typed_trees::signature::SignatureContract| {
            let [ProofFact::Expression(expression)] =
                program.proof_facts.span_or_empty(contract.facts)
            else {
                return None;
            };
            let binary = program.expression_table.binary_node(*expression)?;
            if binary.operator != BinaryOperator::Equal {
                return None;
            }
            let operand_types = [binary.left, binary.right].map(|operand| {
                matches!(
                    program.expression_table.expression(operand),
                    ExpressionNode::Boolean(_)
                )
                .then(&resolve_boolean_type)
                .flatten()
            });
            let mut has_non_builtin_operator = false;
            operators.for_each_expression_use(*expression, |_, operator| {
                has_non_builtin_operator |= operator.status
                    != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback;
            });
            if has_non_builtin_operator
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner,
                    *expression,
                    language_core::OperatorSpelling::Equal,
                    &operand_types,
                )
            {
                return None;
            }
            match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right))
                    if left == right =>
                {
                    Some(checked_trees::ClosedScalarContractValue::Boolean(*left))
                }
                (ExpressionNode::Integer(left), ExpressionNode::Integer(right))
                    if left == right =>
                {
                    Some(checked_trees::ClosedScalarContractValue::Integer(
                        left.clone(),
                    ))
                }
                _ => None,
            }
        };

        let lower_clause = |contract: &typed_trees::signature::SignatureContract| {
            lower_closed_clause(contract)
                .or_else(|| {
                    let facts = program.proof_facts.span_or_empty(contract.facts);
                    let mut remaining = 4096;
                    let mut conjunction: Option<checked_trees::CheckedBooleanExpression> = None;
                    for fact in facts {
                        let expression = fact.as_expression()?;
                        let predicate = crate::values::lower_callable_scalar_contract_predicate(
                            program,
                            operators,
                            owner,
                            parameters?,
                            result_type,
                            *expression,
                            contract.kind == SignatureContractKind::Ensures,
                            &mut remaining,
                        )?;
                        conjunction = Some(match conjunction {
                            None => predicate,
                            Some(left) => checked_trees::CheckedBooleanExpression::And {
                                left: Box::new(left),
                                right: Box::new(predicate),
                            },
                        });
                    }
                    conjunction.map(checked_trees::ClosedScalarContractValue::Predicate)
                })
                .or_else(|| {
                    (contract.kind == SignatureContractKind::Ensures).then(|| {
                        let mut result_qualifications = Vec::new();
                        for fact in program.proof_facts.span_or_empty(contract.facts) {
                            let typed_trees::domain::ProofFact::Membership(membership) = fact
                            else {
                                continue;
                            };
                            let Some((result_owner, carrier)) =
                                ::validation::reserved_result_owner(program, membership.value)
                            else {
                                continue;
                            };
                            if result_owner != owner
                                && !program
                                    .machine(owner)
                                    .and_then(|machine| program.machine_states(machine).first())
                                    .is_some_and(|entry| result_owner == entry.symbol)
                            {
                                continue;
                            }
                            if let Some((path, _)) = ::validation::result_membership_carrier(
                                program,
                                carrier,
                                membership.domain_symbol,
                            ) {
                                let whole_carrier =
                                    ::validation::unwrapped_type_reference(program, carrier)
                                        .unwrap_or(carrier);
                                if path.is_empty()
                                    && program.primitive_type_reference(whole_carrier).is_some()
                                {
                                    continue;
                                }
                                result_qualifications.push(
                                    checked_trees::CheckedStructuralPathQualification {
                                        path,
                                        domain: membership.semantic_domain,
                                    },
                                );
                            }
                        }
                        result_qualifications.sort_by(|left, right| {
                            left.path
                                .cmp(&right.path)
                                .then(left.domain.0.cmp(&right.domain.0))
                        });
                        result_qualifications.dedup();
                        checked_trees::ClosedScalarContractValue::SourceCheckedGuarantee {
                            facts: contract.facts,
                            result_qualifications: result_qualifications.into_boxed_slice(),
                        }
                    })
                })
        };

        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        let mut has_crash_clauses = false;
        let mut has_outcome_specific_clauses = false;
        for contract in contracts {
            if matches!(
                &contract.kind,
                SignatureContractKind::EnsuresForResultCase { .. }
            ) {
                has_outcome_specific_clauses = true;
                continue;
            }
            if contract.binding.is_some() {
                continue;
            }
            match contract.kind {
                SignatureContractKind::Requires => requires.push(lower_clause(contract)),
                SignatureContractKind::Ensures => ensures.push(lower_clause(contract)),
                SignatureContractKind::EnsuresForResultCase { .. } => unreachable!(
                    "outcome-specific clauses were separated from unconditional scalar contracts"
                ),
                SignatureContractKind::Crashes { .. } => has_crash_clauses = true,
            }
        }
        let authored_requires_len = requires.len();
        let ranges = crate::values::lower_callable_parameter_range_requirements(
            program,
            parameters.unwrap_or_default(),
        );
        requires.extend(ranges.scalar_clauses);
        checked_trees::ClosedScalarValueContractPlan::new(
            requires,
            ensures,
            has_crash_clauses,
            has_outcome_specific_clauses,
        )
        .with_authored_requires_len(authored_requires_len)
        .with_float_entry_ranges(ranges.float_entry_ranges)
        .with_integer_entry_ranges(ranges.integer_entry_ranges)
    }

    /*
        Body statements whose planned scalar value executes a Trapping primitive.

        Each Trapping operation owns a `Trap` crash site at run time, so it is
        crash evidence exactly like an explicit crash transition: an inferred body
        widens its `Trap` cause to the unconditional route, and private callers
        inherit it through the summary fixed point. The rows come from the same
        checked scalar expression and computation plans lowering consumes, so a
        Trapping operation reaching Terminal has its evidence here. A published
        ceiling instead has to cover each such operation: the arithmetic-domain
        walk decides that coverage over the operation's own operands while it
        decides Exact formation (`validation`'s `trap_coverage`).
        Body statements whose planned scalar value executes a Trapping primitive,
        indexed once across the program's scalar expression, computation, and
        operator plans so per-machine contract assembly performs O(1) lookups.
    */
    struct PrecomputedTrappingSites {
        expressions_by_state:
            symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<checked_trees::CheckedTrappingSite>>,
        computations_by_machine:
            symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<checked_trees::CheckedTrappingSite>>,
        float_adapters_by_machine:
            symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<(symbols::SymbolHandle, usize)>>,
    }

    impl PrecomputedTrappingSites {
        fn build(
            scalar_expressions: &checked_trees::CheckedScalarExpressionPlans,
            scalar_computations: &checked_trees::CheckedScalarComputationPlans,
            structural_values: &checked_trees::CheckedStructuralValuePlans,
            operators: &checked_trees::CheckedOperatorFacts,
        ) -> Self {
            let mut expressions_by_state = symbols::SymbolKeyMap::<
                symbols::SymbolHandle,
                Vec<checked_trees::CheckedTrappingSite>,
            >::default();
            for located in &scalar_expressions.expressions {
                if located.expression.contains_trapping_operation() {
                    expressions_by_state.entry(located.state).or_default().push(
                        checked_trees::CheckedTrappingSite::new(
                            checked_trees::CrashSiteLocation::new(
                                located.state,
                                located.statement_ordinal,
                            ),
                            located.expression.contains_possible_trapping_operation(),
                        ),
                    );
                }
            }
            let mut computations_by_machine = symbols::SymbolKeyMap::<
                symbols::SymbolHandle,
                Vec<checked_trees::CheckedTrappingSite>,
            >::default();
            for (_, root) in scalar_computations.roots.iter() {
                if computation_contains_trapping_operation(
                    scalar_computations,
                    structural_values,
                    root.root,
                    TrapObservation::Physical,
                ) {
                    computations_by_machine
                        .entry(root.machine)
                        .or_default()
                        .push(checked_trees::CheckedTrappingSite::new(
                            checked_trees::CrashSiteLocation::new(
                                root.state,
                                root.statement_ordinal,
                            ),
                            computation_contains_trapping_operation(
                                scalar_computations,
                                structural_values,
                                root.root,
                                TrapObservation::Possible,
                            ),
                        ));
                }
            }
            let mut float_adapters_by_machine = symbols::SymbolKeyMap::<
                symbols::SymbolHandle,
                Vec<(symbols::SymbolHandle, usize)>,
            >::default();
            for (_, origin, _, adapter) in operators.resolved_expression_selections() {
                if !matches!(
                    adapter,
                    checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. }
                ) {
                    continue;
                }
                let checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    ..
                } = origin
                else {
                    continue;
                };
                float_adapters_by_machine
                    .entry(machine_symbol)
                    .or_default()
                    .push((state_symbol, statement_index));
            }
            Self {
                expressions_by_state,
                computations_by_machine,
                float_adapters_by_machine,
            }
        }

        fn sites_for_machine(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
        ) -> Result<Vec<checked_trees::CheckedTrappingSite>, Vec<diagnostics::Diagnostic>> {
            let states = program.machine_states(machine);
            let mut sites = Vec::new();
            for state in states {
                if let Some(state_sites) = self.expressions_by_state.get(&state.symbol) {
                    sites.extend_from_slice(state_sites);
                }
            }
            if let Some(comp_sites) = self.computations_by_machine.get(&machine.symbol) {
                sites.extend_from_slice(comp_sites);
            }
            if let Some(adapters) = self.float_adapters_by_machine.get(&machine.symbol) {
                let invalid_site = || {
                    vec![diagnostics::Diagnostic::error(
                        "selected float result policy has no representable owning statement",
                    )]
                };
                for &(state_symbol, statement_index) in adapters {
                    if !states.iter().any(|state| state.symbol == state_symbol) {
                        return Err(invalid_site());
                    }
                    let statement = u32::try_from(statement_index).map_err(|_| invalid_site())?;
                    sites.push(checked_trees::CheckedTrappingSite::new(
                        checked_trees::CrashSiteLocation::new(state_symbol, statement),
                        true,
                    ));
                }
            }
            Ok(sites)
        }
    }

    #[derive(Clone, Copy)]
    enum TrapObservation {
        Physical,
        Possible,
    }

    impl TrapObservation {
        fn scalar(self, expression: &checked_trees::CheckedScalarExpression) -> bool {
            match self {
                Self::Physical => expression.contains_trapping_operation(),
                Self::Possible => expression.contains_possible_trapping_operation(),
            }
        }
    }

    /*
        Whether evaluating one computation tree executes a Trapping primitive in
        any of its pure expressions, operands, arms, or constructed arguments.
    */
    fn computation_contains_trapping_operation(
        plans: &checked_trees::CheckedScalarComputationPlans,
        values: &checked_trees::CheckedStructuralValuePlans,
        root: checked_trees::CheckedScalarComputationHandle,
        query: TrapObservation,
    ) -> bool {
        use checked_trees::{
            CheckedScalarComputationKind as Kind,
            CheckedScalarComputationStructuralArgument as Argument,
            CheckedScalarDispatchPattern as Pattern,
        };
        fn push_argument(
            plans: &checked_trees::CheckedScalarComputationPlans,
            query: TrapObservation,
            argument: &Argument,
            pending: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
        ) -> bool {
            match argument {
                Argument::Place(place) => return push_place(plans, query, place, pending),
                Argument::Case(case) => pending.extend(
                    plans
                        .case_fields
                        .span_or_empty(case.fields)
                        .iter()
                        .map(|field| field.value),
                ),
            }
            false
        }
        fn push_place(
            plans: &checked_trees::CheckedScalarComputationPlans,
            query: TrapObservation,
            place: &checked_trees::CheckedUnitStructuralArgumentPlan,
            pending: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
        ) -> bool {
            pending.extend(place.computation_operands(&plans.operands));
            place.scalar_operands().any(|operand| match operand {
                checked_trees::CheckedCallScalarArgument::Pure(value) => query.scalar(value),
                checked_trees::CheckedCallScalarArgument::Computation(_) => false,
            })
        }
        fn push_structural(
            plans: &checked_trees::CheckedScalarComputationPlans,
            values: &checked_trees::CheckedStructuralValuePlans,
            query: TrapObservation,
            root: checked_trees::CheckedStructuralValueHandle,
            pending: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
        ) -> bool {
            use checked_trees::{
                CheckedStructuralRecordFieldValue as Field, CheckedStructuralValueKind as Value,
            };
            let Some(mut dependencies) = values.value_dependencies(root) else {
                return false;
            };
            let mut position = 0;
            while position < dependencies.len() {
                let value = dependencies[position];
                position += 1;
                for call in values
                    .call_schedule(value)
                    .map_or(&[][..], |(_, calls)| calls)
                {
                    for operation in
                        call.operands().iter().chain(call.operation()).flat_map(
                            checked_trees::CheckedUnitEffectOperationPlan::with_value_calls,
                        )
                    {
                        if let checked_trees::CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                                value, ..
                            },
                        )
                        | checked_trees::CheckedUnitEffectOperationPlan::EvaluateStructuralValue {
                            value,
                            ..
                        } = operation
                            && let Some(children) = values.value_dependencies(*value)
                        {
                            for child in children {
                                crate::shared::collections::push_unique(&mut dependencies, child);
                            }
                        }
                        if let Some(call) = operation.call() {
                            for argument in call.scalar_operands() {
                                match argument {
                                    checked_trees::CheckedCallScalarArgument::Pure(expression) => {
                                        if query.scalar(expression) {
                                            return true;
                                        }
                                    }
                                    checked_trees::CheckedCallScalarArgument::Computation(
                                        value,
                                    ) => {
                                        pending.push(*value);
                                    }
                                }
                            }
                        }
                    }
                }
                match &values.nodes.get(value).kind {
                    Value::Record { fields, .. } | Value::StructuralCase { fields, .. } => {
                        for field in values.record_fields.span_or_empty(*fields) {
                            if let Field::Scalar(value) = field.value {
                                pending.push(value);
                            }
                        }
                    }
                    Value::Case(case) => pending.extend(
                        plans.case_fields.span_or_empty(case.fields).iter().map(|field| field.value),
                    ),
                    Value::ScalarArray { elements } => pending.extend(elements),
                    Value::Scalar { value } | Value::ZeroedScalarArray { element: value, .. } => {
                        pending.push(*value);
                    }
                    Value::Reference { source, selectors, .. } => {
                        pending.extend(plans.operands.span_or_empty(*selectors));
                        if push_place(plans, query, source, pending) {
                            return true;
                        }
                    }
                    Value::Projection { path, projection_evaluations, .. } => pending.extend(
                        checked_trees::CheckedUnitStructuralArgumentPlan::projection_computation_operands(
                            path, *projection_evaluations, &plans.operands,
                        ),
                    ),
                    Value::ViewSubslice { start, end, .. } => {
                        for endpoint in start.iter().chain(end.iter()) {
                            match endpoint {
                                checked_trees::CheckedCallScalarArgument::Pure(expression) => {
                                    if query.scalar(expression) {
                                        return true;
                                    }
                                }
                                checked_trees::CheckedCallScalarArgument::Computation(value) => {
                                    pending.push(*value);
                                }
                            }
                        }
                    }
                    Value::Dispatch { subject, arms } => {
                        pending.extend(subject.scalar_operands(&plans.operands));
                        for arm in values.dispatch_arms.span_or_empty(*arms) {
                            if let Pattern::Value(pattern) = arm.pattern {
                                pending.push(pattern);
                            }
                        }
                    }
                    Value::Place(source) | Value::CopiedStructuralPlace { source, .. } => {
                        if push_place(plans, query, source, pending) {
                            return true;
                        }
                    }
                    Value::BorrowedSliceView { source, region }
                    | Value::MutableSliceView { source, region } => {
                        if push_place(plans, query, source, pending) {
                            return true;
                        }
                        if let checked_trees::CheckedElementViewRegion::RepresentationBytes { start, .. } = region {
                            pending.push(*start);
                        }
                    }
                    Value::NativeCodeLayout { .. }
                    | Value::QuotientCall { .. }
                    | Value::QuotientAdaptation { .. }
                    | Value::Call { .. }
                    | Value::FixedArray { .. }
                    | Value::ByteLiteralView { .. }
                    | Value::ZeroedStructural { .. } => {}
                }
            }
            false
        }
        let mut pending = vec![root];
        while let Some(handle) = pending.pop() {
            if !plans.nodes.is_valid(handle) {
                continue;
            }
            match &plans.nodes.get(handle).kind {
                Kind::RetainedConstant { .. } => {}
                Kind::Value(expression) => {
                    if query.scalar(expression) {
                        return true;
                    }
                }
                Kind::Apply {
                    expression,
                    operands,
                    ..
                }
                | Kind::StorageObservation {
                    expression,
                    operands,
                    ..
                } => {
                    if query.scalar(expression) {
                        return true;
                    }
                    pending.extend(plans.operands.span_or_empty(*operands).iter().copied());
                }
                Kind::StorageAddress { subject, .. }
                | Kind::ByteSequenceLiteralComparison { subject, .. }
                | Kind::ByteSequencePredicate { subject, .. } => {
                    if push_place(plans, query, subject, &mut pending) {
                        return true;
                    }
                }
                Kind::StructuralField { .. } => {}
                Kind::CaseMembership { subject, .. } => {
                    if push_argument(plans, query, subject, &mut pending) {
                        return true;
                    }
                }
                Kind::WithOperands {
                    operands, operand, ..
                } => {
                    pending.push(*operand);
                    for dependency in plans.evaluation_operands.span_or_empty(*operands) {
                        match dependency {
                            checked_trees::CheckedScalarEvaluationOperand::Scalar(value) => {
                                pending.push(*value);
                            }
                            checked_trees::CheckedScalarEvaluationOperand::Structural(value) => {
                                if push_structural(plans, values, query, *value, &mut pending) {
                                    return true;
                                }
                            }
                        }
                    }
                }
                Kind::SelectedComparison { left, right, .. } => pending.extend([*left, *right]),
                Kind::Qualification { operand, .. }
                | Kind::BooleanToInteger { operand, .. }
                | Kind::AuthoredCast { operand, .. } => pending.push(*operand),
                Kind::Dispatch { subject, arms, .. } => {
                    pending.extend(subject.scalar_operands(&plans.operands));
                    for arm in plans.dispatch_arms.span_or_empty(*arms) {
                        pending.push(arm.value);
                        if let Some(value) = arm.pattern.computation() {
                            pending.push(value);
                        }
                    }
                }
                Kind::Call {
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                } => {
                    pending.extend(callee.selector());
                    pending.extend(plans.operands.span_or_empty(*arguments).iter().copied());
                    if matches!(callee, checked_trees::CheckedScalarCallee::Boundary { .. }) {
                        continue;
                    }
                    for structural in plans
                        .structural_arguments
                        .span_or_empty(*structural_arguments)
                    {
                        if push_argument(plans, query, structural, &mut pending) {
                            return true;
                        }
                    }
                }
                Kind::Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => pending.extend([*condition, *when_true, *when_false]),
            }
        }
        false
    }
}
pub(crate) use contract_plan_facts::build_callable_scalar_value_contract_plan;
#[path = "published_facts_crash_calls.rs"]
mod crash_calls;
#[path = "published_facts_crash_entry_values.rs"]
mod crash_entry_values;
pub(crate) use crash_entry_values::entry_parameter_place_before_statement;
mod crash_plan_facts {
    /*
        Crash contract capsules, published crash plans, buckets and sites.
    */
    use crate::flow::indexed_statements;
    use crate::published::facts::crash_calls;
    use crate::published::proof::machine_parameter_evidence_signatures;
    use crate::values;
    use validation::contract_structure::{
        callable_binders, declared_callable, declared_fact, declared_termination, generic_binders,
        is_true_crash_route, signature_generic_binders,
    };

    pub(crate) fn build_crash_contract_capsules(
        program: &dyn ::validation::CheckingInput,
        content_conservation: &[validation::ContentConservationSourcePlan],
        operators: &checked_trees::CheckedOperatorFacts,
    ) -> Result<Vec<checked_trees::CrashContractCapsule>, diagnostics::Diagnostic> {
        let mut signatures = Vec::new();
        for machine in program.machines() {
            for (owner_symbol, target_state, signature) in machine_parameter_evidence_signatures(
                program,
                program.machine_type_parameters(machine),
            ) {
                signatures.push((owner_symbol, target_state, signature));
            }
        }
        for definition in program.data_definitions() {
            signatures.extend(machine_parameter_evidence_signatures(
                program,
                program.data_type_parameters(definition),
            ));
        }
        for definition in program.domain_definitions() {
            signatures.extend(machine_parameter_evidence_signatures(
                program,
                program.domain_type_parameters(definition),
            ));
        }
        for definition in program.traits() {
            signatures.extend(machine_parameter_evidence_signatures(
                program,
                program.trait_type_parameters(definition),
            ));
            for declaration in program.trait_machine_signatures(definition) {
                for signature in std::iter::once(declaration).chain(
                    program
                        .requirement_machine_instances(declaration)
                        .filter(|instance| instance.symbol != declaration.symbol),
                ) {
                    signatures.extend(machine_parameter_evidence_signatures(
                        program,
                        program.state_signature_type_parameters(signature),
                    ));
                    signatures.push((definition.symbol, signature.symbol, signature));
                }
            }
        }

        let mut capsules = signatures
            .into_iter()
            .map(|(target_machine, target_state, signature)| {
                build_signature_crash_capsule(
                    program,
                    target_machine,
                    target_state,
                    signature,
                    content_conservation,
                    operators,
                )
            })
            .collect::<Result<Vec<_>, diagnostics::Diagnostic>>()?;
        for (contract, row) in program.runtime_callable_contracts.iter() {
            capsules.push(
                build_signature_crash_capsule(
                    program,
                    symbols::SymbolHandle::invalid(),
                    symbols::SymbolHandle::invalid(),
                    &row.signature,
                    content_conservation,
                    operators,
                )?
                .with_runtime_callable_source(contract, row.lexical_owner),
            );
        }
        capsules.extend(
            program
                .all_operators()
                .filter(|operator| operator.is_boundary)
                .map(|operator| {
                    build_operator_crash_contract_capsule(
                        program,
                        operator,
                        content_conservation,
                        operators,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        capsules.sort_by_key(|capsule| {
            (
                capsule.target_machine().arena_index(),
                capsule.target_machine().generation(),
                capsule.target_state().arena_index(),
                capsule.target_state().generation(),
                capsule
                    .runtime_callable_source()
                    .map(|(contract, _)| (contract.arena_index(), contract.generation())),
            )
        });
        capsules.dedup();
        Ok(capsules)
    }

    /*
        The signature supplies both the crash conditions and their contract identity.
        An applied call and an exported declaration use this same producer; callers
        cannot combine one signature's conditions with another's identity.
    */
    pub(super) fn build_signature_crash_capsule(
        program: &dyn validation::CheckingInput,
        target_machine: symbols::SymbolHandle,
        target_state: symbols::SymbolHandle,
        signature: &typed_trees::signature::StateSignature,
        content_conservation: &[validation::ContentConservationSourcePlan],
        operators: &checked_trees::CheckedOperatorFacts,
    ) -> Result<checked_trees::CrashContractCapsule, diagnostics::Diagnostic> {
        let parameters = program.state_signature_parameters(signature);
        let parameter_names = declared_parameter_names(parameters);
        let contracts = program.state_signature_contracts(signature);
        let published = build_published_crash_buckets(
            program,
            contracts,
            &parameter_names,
            content_conservation,
            CrashContractOwner::Signature(signature),
            Some(operators),
            &[],
        )?;
        let crash = checked_trees::CrashPlan::published_ceiling(published.clone());

        let published_service_names = program
            .service_reach_rows
            .services(signature.service_reach_row)
            .iter()
            .filter_map(|service| program.service_reaches.definition(*service))
            .map(|definition| definition.name.clone())
            .collect::<Vec<_>>();
        let published_invocations = validation::declared_signature_invocations(program, signature)
            .into_iter()
            .map(|invocation| match invocation {
                flow_effects::InvocationTarget::Parameter(index) => {
                    format!("parameter:{index}")
                }
                flow_effects::InvocationTarget::Service(symbol) => program
                    .trait_definition_by_symbol(symbol)
                    .map(|definition| format!("service:{}", definition.name))
                    .unwrap_or_else(|| format!("service:#{}", symbol.arena_index())),
            })
            .collect::<Vec<_>>();

        let generics = signature_generic_binders(program, signature);
        let declaration = checked_trees::DeclaredContractStructure {
            callables: vec![declared_callable(
                program,
                parameters,
                signature.return_type,
                contracts,
                &generics,
                content_conservation,
                true,
            )?],
            clauses: Vec::new(),
        };
        let termination = language_semantics::TerminationInterface::Published(
            signature.termination_guarantee.clone(),
        );
        let termination = declared_termination(
            program,
            &termination,
            &callable_binders(&generics, parameters),
        );
        let identity = checked_trees::contract_identity(
            language_semantics::MachineSupplyMode::Requirement,
            None,
            &published_service_names,
            language_semantics::SynchronousInvocationInterface::PublishedCeiling,
            &published_invocations,
            language_semantics::SuspensionInterface::PublishedMaySuspend(signature.suspends),
            language_semantics::BlockingInterface::PublishedMayBlock(signature.blocks),
            crash.interface(),
            &termination,
            &declaration,
            &program.type_identities,
        );
        Ok(checked_trees::CrashContractCapsule::new_with_commitment(
            target_machine,
            target_state,
            identity.report_fingerprint,
            identity.commitment,
            published,
        )
        .with_operational_envelope(
            published_service_names,
            published_invocations,
            signature.suspends,
            signature.blocks,
            signature.termination_guarantee.clone(),
        ))
    }

    fn build_operator_crash_contract_capsule(
        program: &dyn ::validation::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
        content_conservation: &[validation::ContentConservationSourcePlan],
        operators: &checked_trees::CheckedOperatorFacts,
    ) -> Result<checked_trees::CrashContractCapsule, diagnostics::Diagnostic> {
        let parameters = program.operator_parameters(operator);
        let generics = generic_binders(program.operator_type_parameters(operator));
        let declaration = checked_trees::DeclaredContractStructure {
            callables: vec![declared_callable(
                program,
                parameters,
                operator.return_type,
                program.operator_contracts(operator),
                &generics,
                content_conservation,
                true,
            )?],
            clauses: Vec::new(),
        };
        let published = derive_authored_operator_crash_buckets(
            program,
            operator,
            operators,
            content_conservation,
        )?;
        let crash = checked_trees::CrashPlan::published_ceiling(published.clone());

        let termination = language_semantics::TerminationGuarantee::NoGuarantee;
        let identity = checked_trees::contract_identity(
            language_semantics::MachineSupplyMode::Requirement,
            None,
            &[],
            language_semantics::SynchronousInvocationInterface::PublishedCeiling,
            &[],
            language_semantics::SuspensionInterface::PublishedMaySuspend(false),
            language_semantics::BlockingInterface::PublishedMayBlock(false),
            crash.interface(),
            &checked_trees::DeclaredTermination::NoGuarantee,
            &declaration,
            &program.type_identities,
        );
        Ok(checked_trees::CrashContractCapsule::new_with_commitment(
            operator.symbol,
            operator.symbol,
            identity.report_fingerprint,
            identity.commitment,
            published,
        )
        .with_operational_envelope(Vec::new(), Vec::new(), false, false, termination))
    }

    pub(crate) fn build_published_crash_plan(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        parameter_names: &[String],
        content_conservation: &[validation::ContentConservationSourcePlan],
        operators: &checked_trees::CheckedOperatorFacts,
        declaration_predicates: &[validation::DeclarationPredicateRow],
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Result<checked_trees::CrashPlan, diagnostics::Diagnostic> {
        let mut runtime_membership_subjects = Vec::new();
        let structural_runtime_requirements = build_structural_runtime_requirements(
            program,
            machine,
            operators,
            declaration_predicates,
            exact_integer_casts,
            &mut runtime_membership_subjects,
        );
        let structural_runtime_guarantees =
            build_structural_runtime_guarantees(program, machine, operators, exact_integer_casts);
        let published = build_published_crash_buckets(
            program,
            program.machine_contracts(machine),
            parameter_names,
            content_conservation,
            CrashContractOwner::Machine(machine),
            Some(operators),
            exact_integer_casts,
        )?;
        let plan = (if program.machine_publishes_crash_ceiling(machine) {
            checked_trees::CrashPlan::published_ceiling(published)
        } else {
            checked_trees::CrashPlan::default()
        })
        .with_structural_runtime_requirements(structural_runtime_requirements)
        .with_runtime_membership_subjects(runtime_membership_subjects)
        .with_structural_runtime_guarantees(structural_runtime_guarantees);
        let checked_sites = build_checked_crash_sites(program, machine, &plan);
        plan.with_checked_sites(checked_sites).ok_or_else(|| {
            diagnostics::Diagnostic::error(
                "checked crash publication has conflicting causes at one transition site",
            )
        })
    }

    fn build_structural_runtime_requirements(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        operators: &checked_trees::CheckedOperatorFacts,
        declaration_predicates: &[validation::DeclarationPredicateRow],
        exact_integer_casts: &[validation::ExactIntegerCastFact],
        subjects: &mut Vec<checked_trees::CheckedRuntimeMembershipSubject>,
    ) -> Option<Vec<checked_trees::CheckedBooleanExpression>> {
        let entry = program.machine_states(machine).first()?;
        let facts = program
            .machine_contracts(machine)
            .iter()
            .map(|contract| (true, contract))
            .chain(
                program
                    .state_contracts(entry)
                    .iter()
                    .map(|contract| (false, contract)),
            )
            .filter(|(_, contract)| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .flat_map(|(machine_owned, contract)| {
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .map(move |fact| (machine_owned, contract, fact))
            })
            .enumerate();
        let mut requirements = Vec::new();
        for (fact_ordinal, (machine_owned, contract, fact)) in facts {
            if contract.binding.is_some() {
                if !machine_owned || fact.as_proposition().is_none() {
                    return None;
                }
                continue;
            }
            if validation::restates_structural_parameter_qualification(program, entry, fact) {
                continue;
            }
            match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    requirements.push(values::lower_machine_entry_boolean_expression(
                        program,
                        operators,
                        machine,
                        *expression,
                        exact_integer_casts,
                    )?)
                }
                typed_trees::domain::ProofFact::Membership(membership) => {
                    let primitive = validation::scalar_membership_carrier(program, membership)?;
                    let subject = values::lower_machine_entry_scalar_argument(
                        program,
                        operators,
                        machine,
                        membership.value,
                        primitive,
                    )?;
                    let predicates =
                        validation::scalar_membership_predicates(program, membership, &subject)?;
                    let predicate_count = u32::try_from(predicates.len()).ok()?;
                    requirements.extend(predicates);
                    subjects.push(checked_trees::CheckedRuntimeMembershipSubject {
                        fact_ordinal: u32::try_from(fact_ordinal).ok()?,
                        membership: *membership,
                        subject,
                        predicate_count,
                    });
                }
                typed_trees::domain::ProofFact::Proposition(_) => return None,
            }
        }
        requirements.extend(
            values::lower_scalar_parameter_refinement_predicates(program, machine)
                .into_iter()
                .collect::<Option<Vec<_>>>()?,
        );
        requirements.extend(
            declaration_predicates
                .iter()
                .map(|row| row.predicate.clone()),
        );
        Some(requirements)
    }

    /*
        Lower every unbound `ensures` clause of the machine signature into the
        same structural scalar vocabulary the entry requirements use. The clauses
        observe post-state parameter storage through the identical authored
        positions; bound witness lanes and outcome-specific clauses stay outside
        this capsule because they carry separate evidence plans.
    */
    fn build_structural_runtime_guarantees(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        operators: &checked_trees::CheckedOperatorFacts,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<Vec<checked_trees::CheckedBooleanExpression>> {
        program
            .machine_contracts(machine)
            .iter()
            .chain(
                program
                    .machine_states(machine)
                    .first()
                    .into_iter()
                    .flat_map(|entry| program.state_contracts(entry)),
            )
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Ensures
                    && contract.binding.is_none()
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .map(|fact| {
                let expression = fact.as_expression()?;
                values::lower_machine_entry_boolean_expression(
                    program,
                    operators,
                    machine,
                    *expression,
                    exact_integer_casts,
                )
            })
            .collect()
    }

    /*
        The entry state's declared parameter names, in declaration order — the
        positional row every machine-level contract and crash surface builds.
    */
    pub(crate) fn machine_entry_parameter_names(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<String> {
        program
            .machine_states(machine)
            .first()
            .map(|entry| declared_parameter_names(program.state_parameters(entry)))
            .unwrap_or_default()
    }

    /*
        The declared names of one parameter telescope, in declaration order. A
        source contract names its positions by parameter name, so every contract
        and crash surface reads the same positional row from here.
    */
    pub(crate) fn declared_parameter_names(
        parameters: &[typed_trees::signature::StateParameter],
    ) -> Vec<String> {
        parameters
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect()
    }

    pub(crate) fn derive_authored_machine_crash_buckets(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        operators: Option<&checked_trees::CheckedOperatorFacts>,
    ) -> Result<Vec<checked_trees::CrashRouteBucket>, diagnostics::Diagnostic> {
        let parameter_names = machine_entry_parameter_names(program, machine);
        let conservation = validation::content_conservation_plans(program);
        build_published_crash_buckets(
            program,
            program.machine_contracts(machine),
            &parameter_names,
            &conservation,
            CrashContractOwner::Machine(machine),
            operators,
            &[],
        )
    }

    pub(crate) fn derive_authored_signature_crash_buckets(
        program: &dyn ::validation::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        operators: Option<&checked_trees::CheckedOperatorFacts>,
    ) -> Result<Vec<checked_trees::CrashRouteBucket>, diagnostics::Diagnostic> {
        let parameter_names =
            declared_parameter_names(program.state_signature_parameters(signature));
        let conservation = validation::content_conservation_plans(program);
        build_published_crash_buckets(
            program,
            program.state_signature_contracts(signature),
            &parameter_names,
            &conservation,
            CrashContractOwner::Signature(signature),
            operators,
            &[],
        )
    }

    /*
        An operator declaration's published buckets. Each guarded route carries
        its structured scalar form over the operator's own formal parameters
        (dense scalar position, the telescope Terminal operation crash contracts
        read as formal `k + 1`); a route the operator reader cannot structure
        keeps its identity only, so downstream lowering still fails closed on it.
    */
    pub(crate) fn derive_authored_operator_crash_buckets(
        program: &dyn ::validation::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
        operators: &checked_trees::CheckedOperatorFacts,
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> Result<Vec<checked_trees::CrashRouteBucket>, diagnostics::Diagnostic> {
        let parameter_names = declared_parameter_names(program.operator_parameters(operator));
        build_published_crash_buckets(
            program,
            program
                .signature_contracts
                .span_or_empty(operator.contracts),
            &parameter_names,
            content_conservation,
            CrashContractOwner::Operator(operator),
            Some(operators),
            &[],
        )
    }

    enum CrashContractOwner<'program> {
        Machine(&'program typed_trees::machine::Machine),
        Signature(&'program typed_trees::signature::StateSignature),
        Operator(&'program typed_trees::operator::OperatorDefinition),
    }

    fn build_published_crash_buckets(
        program: &dyn ::validation::CheckingInput,
        contracts: &[typed_trees::signature::SignatureContract],
        parameter_names: &[String],
        content_conservation: &[validation::ContentConservationSourcePlan],
        owner: CrashContractOwner<'_>,
        operators: Option<&checked_trees::CheckedOperatorFacts>,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Result<Vec<checked_trees::CrashRouteBucket>, diagnostics::Diagnostic> {
        pub(crate) use std::collections::BTreeMap;

        #[derive(Default)]
        struct Bucket {
            unconditional: bool,
            routes: Vec<checked_trees::CrashPredicateIdentity>,
        }

        let mut buckets = BTreeMap::<checked_trees::CrashCause, Bucket>::new();
        let binders = match &owner {
            CrashContractOwner::Machine(machine) => {
                let generics = generic_binders(program.machine_type_parameters(machine));
                program
                    .machine_states(machine)
                    .first()
                    .map(|entry| callable_binders(&generics, program.state_parameters(entry)))
                    .unwrap_or(generics)
            }
            CrashContractOwner::Signature(signature) => callable_binders(
                &signature_generic_binders(program, signature),
                program.state_signature_parameters(signature),
            ),
            CrashContractOwner::Operator(operator) => callable_binders(
                &generic_binders(program.operator_type_parameters(operator)),
                program.operator_parameters(operator),
            ),
        };
        for contract in contracts {
            let typed_trees::signature::SignatureContractKind::Crashes { cause } = &contract.kind
            else {
                continue;
            };
            let cause = checked_trees::CrashCause::from(*cause);
            let bucket = buckets.entry(cause).or_default();
            let facts = program.proof_facts.span_or_empty(contract.facts);
            if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
                bucket.unconditional = true;
                continue;
            }
            for fact in facts {
                let identity = match fact {
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        let structured = crash_calls::crash_predicate_from_expression(
                            program,
                            *expression,
                            parameter_names,
                            Some(content_conservation),
                        );
                        let scalar = operators.and_then(|operators| match &owner {
                            CrashContractOwner::Machine(machine) => {
                                values::lower_machine_entry_boolean_expression(
                                    program,
                                    operators,
                                    machine,
                                    *expression,
                                    exact_integer_casts,
                                )
                            }
                            CrashContractOwner::Signature(signature) => {
                                values::lower_signature_crash_contract_expression(
                                    program,
                                    operators,
                                    signature,
                                    *expression,
                                )
                            }
                            CrashContractOwner::Operator(operator) => {
                                values::lower_operator_crash_contract_expression(
                                    program,
                                    operators,
                                    operator,
                                    *expression,
                                )
                            }
                        });
                        if let Some(scalar) = scalar {
                            checked_trees::CrashPredicateIdentity::from_expression_and_scalar(
                                structured, scalar,
                            )
                        } else {
                            checked_trees::CrashPredicateIdentity::from_expression(structured)
                        }
                    }
                    _ => {
                        let declaration =
                            declared_fact(program, fact, &binders, content_conservation)?;
                        checked_trees::CrashPredicateIdentity::from_declared_fact(
                            &declaration,
                            &program.type_identities,
                        )
                    }
                };
                bucket.routes.push(identity);
            }
        }

        buckets
            .into_iter()
            .map(|(cause, mut bucket)| {
                let alternative_guards = if bucket.unconditional {
                    vec![checked_trees::CrashRouteGuard::Truth]
                } else {
                    bucket.routes.sort();
                    bucket.routes.dedup();
                    bucket
                        .routes
                        .into_iter()
                        .map(checked_trees::CrashRouteGuard::Predicate)
                        .collect()
                };
                checked_trees::CrashRouteBucket::new(cause, alternative_guards).ok_or_else(|| {
                    diagnostics::Diagnostic::error(
                        "authored crash publication has no canonical nonempty route set",
                    )
                })
            })
            .collect()
    }

    fn build_checked_crash_sites(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        crash_plan: &checked_trees::CrashPlan,
    ) -> Vec<checked_trees::CheckedCrashSite> {
        let mut sites = Vec::new();
        for state in program.machine_states(machine) {
            for (statement_ordinal, statement) in indexed_statements(program, state.statement_nodes)
            {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                let typed_trees::statement::TransitionExit::Crash(cause) = transition.exit else {
                    continue;
                };
                let cause = checked_trees::CrashCause::from(cause);
                let guard_covering_buckets = crash_plan
                    .published_with_ids()
                    .filter_map(|(id, bucket)| {
                        (bucket.cause() == cause && bucket.is_unconditional()).then_some(id)
                    })
                    .collect();
                sites.push(checked_trees::CheckedCrashSite::new(
                    checked_trees::CrashSiteLocation::new(
                        state.symbol,
                        u32::try_from(statement_ordinal)
                            .expect("state-local statement ordinal exceeds checked identity range"),
                    ),
                    cause,
                    guard_covering_buckets,
                    Vec::new(),
                ));
            }
        }
        sites
    }

    /*
        Canonical identity of one body-derived path predicate in the namespace of
        a machine's published crash routes. This deliberately shares the exact
        encoder used by [`build_published_crash_plan`]: checked guard coverage may
        join a source expression to a published bucket only after both normalize to
        the same source-handle-free bytes.
    */
    pub(crate) fn canonical_crash_path_predicate(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
        negated: bool,
        parameter_names: &[String],
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> checked_trees::CrashPredicateIdentity {
        let expression = crash_calls::crash_predicate_from_expression(
            program,
            expression,
            parameter_names,
            Some(content_conservation),
        );
        let expression = if negated {
            checked_trees::CrashPredicateExpression::Unary {
                operator: typed_trees::expression::UnaryOperator::LogicalNot as u8,
                operand: Box::new(expression),
            }
        } else {
            expression
        };
        checked_trees::CrashPredicateIdentity::from_expression(expression)
    }

    /*
        Canonical identity of a checker-derived binary predicate assembled from
        existing typed operands. This uses the published crash-route encoder but
        never rewrites the published contract itself.
    */
    pub(crate) fn canonical_crash_binary_path_predicate(
        program: &dyn ::validation::CheckingInput,
        operator: typed_trees::expression::BinaryOperator,
        left: typed_trees::expression::ExpressionHandle,
        right: typed_trees::expression::ExpressionHandle,
        parameter_names: &[String],
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> checked_trees::CrashPredicateIdentity {
        let left = crash_calls::crash_predicate_from_expression(
            program,
            left,
            parameter_names,
            Some(content_conservation),
        );
        let right = crash_calls::crash_predicate_from_expression(
            program,
            right,
            parameter_names,
            Some(content_conservation),
        );
        checked_trees::CrashPredicateIdentity::from_expression(
            checked_trees::CrashPredicateExpression::Binary {
                operator: operator as u8,
                left: Box::new(left),
                right: Box::new(right),
            },
        )
    }

    /*
        Proof assumptions and published ceilings use the same typed identity
        producer; executable guard keys remain in their independent namespace.
    */
    pub(crate) fn canonical_crash_contract_predicate(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
        negated: bool,
        parameter_names: &[String],
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> checked_trees::CrashPredicateIdentity {
        let expression = crash_calls::crash_predicate_from_expression(
            program,
            expression,
            parameter_names,
            Some(content_conservation),
        );
        let expression = if negated {
            checked_trees::CrashPredicateExpression::Unary {
                operator: typed_trees::expression::UnaryOperator::LogicalNot as u8,
                operand: Box::new(expression),
            }
        } else {
            expression
        };
        checked_trees::CrashPredicateIdentity::from_expression(expression)
    }

    /*
        Canonical source-handle-free identity for one operand participating in a
        checker-derived crash-predicate relation. This is an internal join key;
        only complete predicate identities enter checked crash plans.
    */
    pub(crate) fn canonical_crash_operand_identity(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
        parameter_names: &[String],
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> checked_trees::CrashPredicateIdentity {
        checked_trees::CrashPredicateIdentity::from_operand(
            crash_calls::crash_predicate_from_expression(
                program,
                expression,
                parameter_names,
                Some(content_conservation),
            ),
        )
    }
}
mod declaration_calls {
    /*
        Settle declaration-body calls against this invocation's completed value-call
        admission and public contract plans. The application retains its own input
        formation; this join neither creates a statement nor replays its proof.
    */
    pub(super) fn bind(
        applications: &mut checked_trees::CheckedRankArrivalPlan,
        admitted: &[validation::SpecificationCallRequest],
        contracts: &checked_trees::MachineContractPlans,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let calls = applications
            .application_calls
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in calls {
            let call = applications.application_calls.get_mut(handle);
            if !admitted.iter().any(|request| {
                request.call_expression == call.expression
                    && request.target_machine == call.target_machine
                    && request.target_state == call.target_state
            }) {
                return Err(vec![diagnostics::Diagnostic::error(
                    "declaration call lost its admitted denotational invocation",
                )]);
            }
            let Some(contract) = contracts.for_machine(call.target_machine) else {
                return Err(vec![diagnostics::Diagnostic::error(
                    "declaration call target has no completed public contract plan",
                )]);
            };
            call.target_contract = Some(checked_trees::MachineContractIdentity {
                report_fingerprint: contract.report_fingerprint,
                commitment: contract.commitment,
            });
        }
        Ok(())
    }
}
pub(crate) mod declaration_descriptions {
    /*
        Freeze callable descriptions before product pruning can remove their bodies.

        This is the identity projection formerly reconstructed by stage05. Only
        declaration coordinates and checked commitments survive, not source bodies.
        Checking owns this publication; later consumers read the frozen inventory
        rather than requesting a source-signature refresh.
    */
    use checked_trees::{
        CheckedCallableHeader, CheckedDeclarationCapture, CheckedDeclarationDescription,
        CheckedDeclarationDescriptions, CheckedDeclarationParameter,
        CheckedDeclarationSpecialization, CheckedMachineDescription,
        CheckedSynchronousBoundaryTarget, CheckedTrees,
    };
    use symbols::SymbolHandle;

    pub(crate) fn retain_declaration_descriptions(checked: &mut CheckedTrees) {
        let mut descriptions = CheckedDeclarationDescriptions::with_requirements(
            validation::checked_selection_requirements(checked),
            validation::compiler_evaluation::checked_dependencies(checked),
        );
        let sidecars = typed_trees::references::MachineSidecarIndex::build(&checked.typed);
        let mut summaries = checked
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .collect::<Vec<_>>();
        summaries
            .sort_by_key(|summary| (summary.machine.arena_index(), summary.machine.generation()));
        for machine in checked.typed.machines() {
            let synchronous_invocations = synchronous_invocations(checked, machine, &summaries)
                .map(|targets| descriptions.invocation_targets.insert_many(targets));
            descriptions.headers.insert(CheckedCallableHeader {
                machine: machine.symbol,
                callable: checked.typed.machine_callable_identity(machine),
                package: checked.symbols.symbol_package_identity(machine.symbol),
                source_target: checked.source_target_context(machine).map(str::to_owned),
                product_eligible: !machine.target_family_interface
                    && checked.symbols.symbol_is_in_dependency_scope(
                        machine.symbol,
                        source::DependencyScope::Product,
                    ),
                supplies_checked_body: machine.supplies_checked_body(),
                type_parameter_count: checked.typed.machine_type_parameters(machine).len(),
                compiler_evaluation: validation::compiler_evaluation::checked_declaration(
                    checked,
                    machine,
                    &descriptions.selections,
                    &sidecars,
                ),
                synchronous_invocations,
            });
            let row = match describe(checked, &mut descriptions, machine.symbol) {
                Ok(description) => CheckedDeclarationDescription::Machine(description),
                Err(reason) => CheckedDeclarationDescription::Unavailable {
                    machine: machine.symbol,
                    reason,
                },
            };
            descriptions.insert_machine(row);
        }
        let previous = &checked.facts.declaration_descriptions;
        for (_, header) in previous.headers.iter() {
            if checked.machine(header.machine).is_none() {
                let mut header = header.clone();
                header.synchronous_invocations = header.synchronous_invocations.and_then(|span| {
                    previous
                        .invocation_targets
                        .span(span)
                        .map(|targets| {
                            descriptions
                                .invocation_targets
                                .insert_many(targets.iter().cloned())
                        })
                        .ok_or_else(|| {
                            "checked declaration lost its retained invocation targets".to_owned()
                        })
                });
                descriptions.headers.insert(header);
            }
        }
        for (_, row) in previous.machines().iter() {
            if checked.machine(row.machine()).is_some() {
                continue;
            }
            let mut row = row.clone();
            if let CheckedDeclarationDescription::Machine(description) = &mut row {
                let Some(parameters) = previous.parameters.span(description.parameters) else {
                    descriptions.insert_machine(CheckedDeclarationDescription::Unavailable {
                        machine: description.machine,
                        reason: "checked declaration lost its retained parameter identities".into(),
                    });
                    continue;
                };
                description.parameters = descriptions
                    .parameters
                    .insert_many(parameters.iter().cloned());
            }
            descriptions.insert_machine(row);
        }
        descriptions.index_machines();
        checked.facts.declaration_descriptions = descriptions;
    }

    fn synchronous_invocations(
        checked: &CheckedTrees,
        machine: &typed_trees::machine::Machine,
        summaries: &[&checked_trees::MachineSynchronousInvocationFact],
    ) -> Result<Vec<CheckedSynchronousBoundaryTarget>, String> {
        let key = (machine.symbol.arena_index(), machine.symbol.generation());
        let start = summaries.partition_point(|summary| {
            (summary.machine.arena_index(), summary.machine.generation()) < key
        });
        let end = summaries.partition_point(|summary| {
            (summary.machine.arena_index(), summary.machine.generation()) <= key
        });
        let [summary] = &summaries[start..end] else {
            return Err(format!(
                "checked adapter `{}` resolves to {} exact checked synchronous-invocation facts",
                machine.name,
                end - start
            ));
        };
        summary
            .realized_targets
            .iter()
            .map(|target| synchronous_target(checked, machine, *target))
            .collect()
    }

    fn synchronous_target(
        checked: &CheckedTrees,
        machine: &typed_trees::machine::Machine,
        target: flow_effects::RealizedInvocationTarget,
    ) -> Result<CheckedSynchronousBoundaryTarget, String> {
        let typed = &checked.typed;
        let (symbol, application) = match target {
            flow_effects::RealizedInvocationTarget::Requirement(requirement) => {
                let scope = if let Some(application) = typed.closed_requirement_application(requirement) {
                    application.trait_scope.scope(typed)
                } else {
                    typed.trait_machine_signature(requirement).and_then(|(declaration, _)| {
                        (typed.trait_type_parameters(declaration).is_empty() && declaration.lifetime_parameters.is_empty())
                            .then(|| typed_trees::trait_definition::AppliedTraitScope::abstract_scope(declaration))
                    })
                }.ok_or_else(|| format!("checked adapter `{}` synchronous invocation lost its exact requirement application", machine.name))?;
                let application = scope.portable_application_identity(typed)
                    .and_then(|identity| typed.type_identity_text(identity))
                    .ok_or_else(|| format!("checked adapter `{}` synchronous invocation has no portable application identity", machine.name))?;
                (scope.declaration.symbol, Some(application))
            }
            flow_effects::RealizedInvocationTarget::Abstract(
                flow_effects::InvocationTarget::Service(symbol),
            ) => (symbol, None),
            flow_effects::RealizedInvocationTarget::Abstract(
                flow_effects::InvocationTarget::Parameter(parameter),
            ) => {
                return Err(format!(
                    "checked adapter `{}` retains retired runtime boundary-binding parameter {parameter} in its synchronous invocation graph",
                    machine.name
                ));
            }
        };
        if !symbol.is_valid() {
            return Err(format!(
                "checked adapter `{}` has an invalid exact synchronous-invocation service symbol",
                machine.name
            ));
        }
        if !typed
            .symbols
            .symbol_is_in_dependency_scope(symbol, source::DependencyScope::Product)
        {
            return Err(format!(
                "checked adapter `{}` synchronous-invocation service {:?} is outside the Product planning context",
                machine.name, symbol
            ));
        }
        let definition = typed.trait_definition_by_symbol(symbol).filter(|definition| definition.is_boundary)
            .ok_or_else(|| format!("checked adapter `{}` synchronous-invocation symbol {:?} has no exact boundary trait", machine.name, symbol))?;
        Ok(CheckedSynchronousBoundaryTarget {
            declaration: typed.trait_declaration_path(definition),
            package: typed.symbols.symbol_package_identity(symbol),
            application,
        })
    }

    fn describe(
        checked: &CheckedTrees,
        descriptions: &mut CheckedDeclarationDescriptions,
        machine: SymbolHandle,
    ) -> Result<CheckedMachineDescription, String> {
        let declaration = checked
            .machine(machine)
            .ok_or("checked machine declaration is absent")?;
        let entry = checked
            .machine_states(declaration)
            .first()
            .ok_or("checked machine declaration has no entry state")?;
        let callable = checked.typed.machine_callable_identity(declaration);
        if !callable.is_valid() {
            return Err("checked machine has no callable signature identity".into());
        }
        let contract = checked
            .facts
            .contract_plans
            .for_machine(machine)
            .ok_or("checked machine declaration has no contract")?
            .commitment;
        let specialization = checked
            .typed
            .executable_application_template(machine)?
            .map(|template| {
                let identity = checked
                    .facts
                    .specialization_identities
                    .for_instance(machine)
                    .ok_or("checked executable application has no retained identity")?;
                let callable = checked
                    .machine(template)
                    .map(|template| checked.typed.machine_callable_identity(template))
                    .filter(|identity| identity.is_valid())
                    .ok_or("checked specialization lost its exact template callable")?;
                Ok::<_, String>(CheckedDeclarationSpecialization {
                    template,
                    callable,
                    commitment: identity.commitment.as_bytes(),
                    capture: capture(checked, machine)?,
                })
            })
            .transpose()?;
        let parameters =
            descriptions
                .parameters
                .insert_many(checked.state_parameters(entry).iter().map(|parameter| {
                    CheckedDeclarationParameter {
                        normalized: checked.normalized_type_identity(parameter.type_reference),
                        qualified: checked
                            .package_qualified_type_identity(parameter.type_reference),
                    }
                }));
        Ok(CheckedMachineDescription {
            machine,
            entry: entry.symbol,
            parameters,
            result: checked.package_qualified_type_identity(entry.return_type),
            callable,
            contract,
            specialization,
        })
    }

    fn capture(
        checked: &CheckedTrees,
        machine: SymbolHandle,
    ) -> Result<Option<CheckedDeclarationCapture>, String> {
        let template = checked.executable_application_template(machine)?;
        let mut direct = checked
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| capture.instance == machine)
            .map(|(_, capture)| capture);
        let direct_capture = direct.next();
        if direct.next().is_some() {
            return Err("checked declaration has ambiguous capture ownership".into());
        }
        let capture = if direct_capture.is_some() {
            direct_capture
        } else {
            let mut outer = checked
                .captured_machine_applications
                .iter()
                .filter(|(_, capture)| Some(capture.instance) == template)
                .map(|(_, capture)| capture);
            let capture = outer.next();
            if outer.next().is_some() {
                return Err("checked invocation has ambiguous capture ownership".into());
            }
            capture
        };
        capture
            .map(|capture| {
                let receipt = checked
                    .facts
                    .specialization_identities
                    .for_instance(capture.instance)
                    .ok_or("checked capture declaration lost its independent receipt")?;
                let template_identity = checked
                    .type_identities
                    .intern(&capture.normalized_template_identity);
                Ok(CheckedDeclarationCapture {
                    template_identity,
                    template_contract: receipt.template_contract_commitment.as_bytes(),
                    specialization: receipt.commitment.as_bytes(),
                })
            })
            .transpose()
    }
}
mod dynamic_conformance {
    /*
        Checked fact retention shares the exact source-owned dynamic row projector.
        No second finite-family or callable-identity derivation lives in this stage.
    */
    pub(crate) use validation::{
        describe_dynamic_conformances as build_dynamic_conformance_facts,
        normalized_dynamic_row_identities,
    };
}
pub(crate) mod identity_formations {
    /*
    Publish the exact checked Id formation plan before source erasure.

    Id remains a statement, not a witness. This owner resolves source metadata
    and closes each subject telescope; receiving independently checks the
    mathematical judgment after lowering encodes this prepared plan.
    */

    use checked_trees::{
        CheckedIdentityFormation, CheckedIdentityFormationPlan, CheckedIdentityOwner,
        CheckedIdentityPublication, CheckedMathematicalSignature,
    };
    use diagnostics::Diagnostic;
    use proof_admission::{Declaration, Term};
    use typed_trees::TypedTrees;

    pub(crate) fn record(
        program: &TypedTrees,
        formations: &[CheckedIdentityFormation],
        signature: Option<&CheckedMathematicalSignature>,
    ) -> Result<CheckedIdentityFormationPlan, Vec<Diagnostic>> {
        let refuse = |reason: &str| vec![Diagnostic::error(reason)];
        if formations.is_empty() {
            return Ok(CheckedIdentityFormationPlan::default());
        }
        let signature = signature
            .ok_or_else(|| refuse("Id formation lost its checked mathematical signature"))?;
        let mut arena = signature.arena.clone();
        let mut declarations = signature.declarations.clone();
        let mut rows = Vec::with_capacity(formations.len());
        for formation in formations {
            if formation.domains.len() != formation.captures.len() {
                return Err(refuse(
                    "Id formation lost its exact source subject telescope",
                ));
            }
            let owner = if program
                .symbols
                .symbol_package_identity(formation.owner)
                .is_none()
                && program.symbols.symbol_source_origin(formation.owner)
                    == Some(source::SourceOrigin::User)
            {
                CheckedIdentityOwner::InvocationLocal(format!(
                    "invocation-declaration({}:{})",
                    formation.owner.arena_index(),
                    formation.owner.generation()
                ))
            } else {
                CheckedIdentityOwner::Hermetic(
                    program
                        .normalized_hermetic_symbol_identity(formation.owner)
                        .map_err(|reason| refuse(&reason))?,
                )
            };
            let source_carrier = program
                .type_identity_text(formation.type_identity)
                .ok_or_else(|| refuse("Id formation lost its exact source carrier"))?;
            let subject_carriers = formation
                .captures
                .iter()
                .map(|(_, ty)| {
                    program
                        .type_identity_text(*ty)
                        .ok_or_else(|| refuse("Id subject lost its exact source carrier"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let subject_origins = formation
                .captures
                .iter()
                .map(|(expression, _)| {
                    let span = program.expression_table.source_span(*expression).span;
                    Ok((
                        u32::try_from(span.start)
                            .map_err(|_| refuse("Id source subject position exceeds u32"))?,
                        u32::try_from(span.end)
                            .map_err(|_| refuse("Id source subject position exceeds u32"))?,
                    ))
                })
                .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            let mut body = formation.term;
            let mut ty = formation.ty;
            for domain in formation.domains.iter().rev() {
                body = arena.insert(Term::Lambda {
                    domain: *domain,
                    body,
                });
                ty = arena.insert(Term::Pi {
                    domain: *domain,
                    codomain: ty,
                });
            }
            let declaration = u32::try_from(declarations.len())
                .map_err(|_| refuse("Id formation declaration roster exceeds u32"))?;
            declarations.push(Declaration {
                level_arity: formation.level_arity,
                ty,
                body: Some(body),
            });
            rows.push(CheckedIdentityPublication {
                owner,
                source_carrier,
                subject_carriers,
                subject_origins,
                declaration,
            });
        }
        let term = arena.insert(Term::TwoZero);
        let ty = arena.insert(Term::Two);
        Ok(CheckedIdentityFormationPlan {
            rows,
            arena,
            declarations,
            term,
            ty,
        })
    }
}
pub(crate) mod index_compatibility {
    use crate::flow::expressions;
    use arena::HandleSpan;
    use checked_trees::{
        CheckedOperatorFacts, FlowFacts, FlowSemanticContextRef, IndexCompatibilityDischarge,
        IndexCompatibilityFact, IndexCompatibilityFacts, ProofFacts,
    };
    use diagnostics::Diagnostic;
    use facts::{FactPlan, ProgramPoint};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    use typed_trees::types::TypeReferenceHandle;

    mod index_terms {
        /*
            Index compatibility borrows the canonical identity owner. Call bindings are
            simultaneous caller-side values; neither token shape nor diagnostic spelling
            can identify an operation or license reordering. Checking creates no AST.
        */
        use super::{BoundIndexArgument, BoundIndexSubstitutions};
        use symbols::SymbolHandle;
        use typed_trees::type_identity::{IndexIdentityArgument, normalized_index_identity};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn index_identity_bindings<'bindings>(
            substitutions: &'bindings BoundIndexSubstitutions<'_>,
        ) -> Vec<(SymbolHandle, IndexIdentityArgument<'bindings>)> {
            substitutions
                .iter()
                .map(|(symbol, value)| {
                    (
                        *symbol,
                        match value {
                            BoundIndexArgument::Subject(subject) => {
                                IndexIdentityArgument::Subject(*subject)
                            }
                            BoundIndexArgument::Reference(reference) => {
                                IndexIdentityArgument::TypeReference(*reference)
                            }
                            BoundIndexArgument::Algebra(argument) => {
                                IndexIdentityArgument::StaticArgument(argument)
                            }
                            BoundIndexArgument::Expression(expression) => {
                                IndexIdentityArgument::Expression(*expression)
                            }
                        },
                    )
                })
                .collect()
        }

        pub(super) fn index_arguments_equal(
            program: &dyn ::validation::CheckingInput,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
            actual_substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
            expected_substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
        ) -> bool {
            normalized_index_identity(
                program,
                IndexIdentityArgument::TypeReference(actual),
                actual_substitutions,
            )
            .zip(normalized_index_identity(
                program,
                IndexIdentityArgument::TypeReference(expected),
                expected_substitutions,
            ))
            .is_some_and(|(actual, expected)| actual == expected)
        }

        pub(super) fn bound_index_arguments_equal(
            program: &dyn ::validation::CheckingInput,
            actual: &[TypeReferenceHandle],
            expected: &[TypeReferenceHandle],
            actual_substitutions: &BoundIndexSubstitutions,
            expected_substitutions: &BoundIndexSubstitutions,
        ) -> bool {
            let actual_bindings = index_identity_bindings(actual_substitutions);
            let expected_bindings = index_identity_bindings(expected_substitutions);
            !actual.is_empty()
                && actual.len() == expected.len()
                && actual.iter().zip(expected).all(|(actual, expected)| {
                    index_arguments_equal(
                        program,
                        *actual,
                        *expected,
                        &actual_bindings,
                        &expected_bindings,
                    )
                })
        }

        pub(super) fn is_closed_index_argument(
            program: &dyn ::validation::CheckingInput,
            argument: TypeReferenceHandle,
            substitutions: &BoundIndexSubstitutions,
        ) -> bool {
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(argument)
            else {
                return false;
            };
            if let Some((_, bound)) = substitutions.iter().find(|(binder, _)| *binder == *symbol) {
                return match bound {
                    BoundIndexArgument::Reference(reference) => {
                        is_closed_index_argument(program, *reference, &Vec::new())
                    }
                    _ => false,
                };
            }
            language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str()).is_some()
                || name.as_str().parse::<i128>().is_ok()
        }
    }
    mod instance_collection {
        /*
            Indexed-instance collection: the compatibility key and state-call
            index, typed-indexed instance discovery, and ensured-instance plumbing.
        */
        use super::labels::{domain_label, qualification_label};
        use crate::flow::call_flows;
        use crate::flow::expressions;
        use crate::published::proof::contracts::call_signatures::owns_contract;
        use arena::HandleSpan;
        use checked_trees::{
            CheckedOperatorFacts, ContractProofFactKind, FlowFacts, FlowSemanticContextRef,
            ProofFacts,
        };
        use facts::ProgramPoint;
        use language_semantics::SemanticDomainId;
        use symbols::JoinedPath;
        use symbols::SymbolHandle;
        use typed_trees::data::DataMember;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{
            DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };

        #[derive(Debug, Clone)]
        pub(super) struct IndexedInstance {
            pub(super) family: SymbolHandle,
            pub(super) semantic_id: SemanticDomainId,
            pub(super) arguments: Vec<TypeReferenceHandle>,
            pub(super) label: String,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) struct CompatibilityKey {
            pub(super) point: ProgramPoint,
            pub(super) value: ExpressionHandle,
            pub(super) target_type: TypeReferenceHandle,
            pub(super) family: SymbolHandle,
            pub(super) actual: SemanticDomainId,
            pub(super) expected: SemanticDomainId,
        }

        pub(super) struct ResolvedStateCall<'program, 'flow> {
            pub(super) fact: &'flow checked_trees::FlowCallFact,
            pub(super) site: validation::CallSite<'program>,
        }

        pub(super) struct StateCallIndex<'program, 'flow> {
            pub(super) calls: Vec<ResolvedStateCall<'program, 'flow>>,
        }

        impl<'program, 'flow> StateCallIndex<'program, 'flow> {
            pub(super) fn new(
                program: &'program dyn ::validation::CheckingInput,
                flow: &'flow FlowFacts,
                state_flow: &'flow checked_trees::FlowStateFact,
            ) -> Self {
                let calls = call_flows(&flow.control, state_flow.calls)
                    .iter()
                    .filter_map(|fact| {
                        validation::find_call_site(
                            program,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                            fact.statement_index,
                            fact.call_ordinal,
                        )
                        .map(|site| ResolvedStateCall { fact, site })
                    })
                    .collect();
                Self { calls }
            }

            pub(super) fn contexts_after_value(
                &self,
                statement_index: usize,
                value: ExpressionHandle,
                fallback: HandleSpan<FlowSemanticContextRef>,
            ) -> HandleSpan<FlowSemanticContextRef> {
                self.calls
                    .iter()
                    .filter(|call| call.fact.statement_index == statement_index)
                    .find_map(|call| match &call.site {
                        validation::CallSite::Expression { expression, .. }
                            if *expression == value =>
                        {
                            Some(call.fact.exit_semantic_contexts)
                        }
                        _ => None,
                    })
                    .unwrap_or(fallback)
            }
        }

        pub(super) fn construction_field_type(
            program: &dyn ::validation::CheckingInput,
            definition: &typed_trees::data::DataDefinition,
            case_name: Option<&str>,
            field_name: &str,
        ) -> Option<TypeReferenceHandle> {
            if let Some(case_name) = case_name
                && let Some(variant) =
                    program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant) if variant.name.as_str() == case_name => {
                                Some(variant)
                            }
                            _ => None,
                        })
            {
                for field in program.data_payload_fields(variant) {
                    if field.name.as_str() == field_name && field.type_reference.is_valid() {
                        return Some(field.type_reference);
                    }
                }
            }
            program.data_members(definition).iter().find_map(|member| {
                let field = member.as_field()?;
                (field.name.as_str() == field_name && field.type_reference.is_valid())
                    .then_some(field.type_reference)
            })
        }

        pub(super) fn literal_element_type(
            program: &dyn ::validation::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            if !type_reference.is_valid() {
                return None;
            }
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => {
                    literal_element_type(program, *referee)
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    literal_element_type(program, *base_type)
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                _ => None,
            }
        }

        pub(super) fn expression_indexed_instances(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            machine: &Machine,
            state: &State,
            statement_index: usize,
            expression: ExpressionHandle,
        ) -> Vec<IndexedInstance> {
            let mut instances = Vec::new();
            match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(inner) => {
                    return expression_indexed_instances(
                        program,
                        operators,
                        machine,
                        state,
                        statement_index,
                        inner.target,
                    );
                }
                ExpressionNode::Atomic(atomic) => {
                    return expression_indexed_instances(
                        program,
                        operators,
                        machine,
                        state,
                        statement_index,
                        atomic.result,
                    );
                }
                ExpressionNode::Cast(cast) => {
                    collect_type_indexed_instances(
                        program,
                        cast.target_type,
                        &mut instances,
                        &mut Vec::new(),
                    );
                    if cast.semantic_domain_symbol.is_valid()
                        && !cast.semantic_domain_arguments.is_empty()
                    {
                        instances.push(IndexedInstance {
                            family: cast.semantic_domain_symbol,
                            semantic_id: cast.semantic_domain_id,
                            arguments: program
                                .type_reference_table
                                .type_reference_handles(cast.semantic_domain_arguments)
                                .to_vec(),
                            label: qualification_label(program, cast),
                        });
                    }
                    return instances;
                }
                ExpressionNode::Call(call) => {
                    let return_type = program
                        .requirement_call_signature(machine.symbol, call)
                        .map(|signature| signature.return_type)
                        .or_else(|| {
                            (!call.requirement_application.is_valid()
                                && call.static_requirement_dispatch.is_none())
                            .then(|| call_return_type(program, call.target_symbol))
                            .flatten()
                        });
                    if let Some(return_type) = return_type {
                        collect_type_indexed_instances(
                            program,
                            return_type,
                            &mut instances,
                            &mut Vec::new(),
                        );
                    } else if let Some(operator_use) = operators
                        .named_uses()
                        .find(|operator_use| operator_use.expression == expression)
                        && let Some(operator) = program
                            .operator_definition_by_symbol(operator_use.selected_operator_symbol)
                    {
                        collect_type_indexed_instances(
                            program,
                            operator.return_type,
                            &mut instances,
                            &mut Vec::new(),
                        );
                    }
                    return instances;
                }
                ExpressionNode::Binary(_)
                | ExpressionNode::Unary(_)
                | ExpressionNode::Indexed(_) => {
                    if let Some(operator_use) = operators.expression_use(expression)
                        && let Some(operator) = program
                            .operator_definition_by_symbol(operator_use.selected_operator_symbol)
                    {
                        collect_type_indexed_instances(
                            program,
                            operator.return_type,
                            &mut instances,
                            &mut Vec::new(),
                        );
                        return instances;
                    }
                }
                _ => {}
            }
            if let Some(type_reference) = validation::expression_type_reference_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            ) {
                collect_type_indexed_instances(
                    program,
                    type_reference,
                    &mut instances,
                    &mut Vec::new(),
                );
            } else if let Some(type_reference) =
                validation::declared_place_type_raw(program, machine, Some(state), expression)
            {
                collect_type_indexed_instances(
                    program,
                    type_reference,
                    &mut instances,
                    &mut Vec::new(),
                );
            }
            instances
        }

        /*
            The indexed instances a value-position call's own `ensures` declare on
            its reserved `result`. A declared return type `-> Extent` names no
            instance, so `expression_indexed_instances` finds none there; the
            requirement's `ensures result in Granted & Resident<SlotPlacement, Slot>`
            does, and the typed membership fact attached to this call carries the
            identity the typer interned on it. A restating `let` (or store, or
            return) then compares against that instance exactly as it compares
            against a declared field's, instead of joining the family by symbol and
            keeping whichever index the restatement chose. The facts are read from
            the call's contract row, before `call_contract_evidence` decides whether
            the promise is admitted as establishment: like the declared return type,
            the declared instance is what the value is, whatever its evidence.
        */
        pub(super) fn append_call_result_ensured_instances(
            program: &dyn ::validation::CheckingInput,
            proof: &ProofFacts,
            state_calls: &StateCallIndex<'_, '_>,
            machine: &Machine,
            state: &State,
            statement_index: usize,
            value: ExpressionHandle,
            instances: &mut Vec<IndexedInstance>,
        ) {
            if !matches!(
                program.expression_table.expression(value),
                ExpressionNode::Call(_)
            ) {
                return;
            }
            let Some(call) = state_calls.calls.iter().find(|call| {
                call.fact.statement_index == statement_index
                    && matches!(
                        &call.site,
                        validation::CallSite::Expression { expression, .. }
                            if *expression == value
                    )
            }) else {
                return;
            };
            let Some((_, contract_call)) =
                proof.contract_calls.iter().find(|(_, contract_call)| {
                    contract_call.caller_machine_symbol == machine.symbol
                        && contract_call.caller_state_symbol == state.symbol
                        && contract_call.statement_index == statement_index
                        && contract_call.call_ordinal == call.fact.call_ordinal
                })
            else {
                return;
            };
            for reference in proof
                .contract_fact_refs
                .span_or_empty(contract_call.ensures)
            {
                let contract = proof.contract_facts.get(reference.fact);
                if contract.kind != ContractProofFactKind::Ensures
                    || !owns_contract(program, proof, contract_call, contract)
                {
                    continue;
                }
                let mut selected_fact = program.proof_facts.get(contract.fact);
                if let validation::CallSite::Expression {
                    call: expression, ..
                } = &call.site
                    && let checked_trees::ContractProofFactOwner::StateSignature {
                        state_symbol,
                        ..
                    } = contract.owner
                    && let Some((_, declared)) = program.trait_machine_signature(state_symbol)
                    && let Some(selected) =
                        program.requirement_call_signature(machine.symbol, expression)
                {
                    let Some(applied_fact) =
                        program.applied_signature_contract_fact(declared, selected, contract.fact)
                    else {
                        continue;
                    };
                    selected_fact = applied_fact;
                }
                let Some(membership) = selected_fact.as_membership() else {
                    continue;
                };
                if !membership.semantic_domain.is_valid()
                    || !expressions(program).names_reserved_result(membership.value)
                {
                    continue;
                }
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                    .to_vec();
                if arguments.is_empty()
                    || instances.iter().any(|instance| {
                        instance.family == membership.domain_symbol
                            && instance.semantic_id == membership.semantic_domain
                    })
                {
                    continue;
                }
                let name = program.domain_path_members(membership.domain).joined_path();
                let label = domain_label(program, &name, &arguments);
                instances.push(IndexedInstance {
                    family: membership.domain_symbol,
                    semantic_id: membership.semantic_domain,
                    arguments,
                    label,
                });
            }
        }
        pub(super) fn call_return_type(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
        ) -> Option<TypeReferenceHandle> {
            if let Some(state) = program.state_by_symbol(state_symbol) {
                return Some(state.return_type);
            }
            if let Some((_, signature)) = program.machine_parameter_signature(state_symbol) {
                return Some(signature.return_type);
            }
            program
                .trait_machine_signature(state_symbol)
                .map(|(_, signature)| signature.return_type)
        }

        pub(super) fn collect_type_indexed_instances(
            program: &dyn ::validation::CheckingInput,
            type_reference: TypeReferenceHandle,
            instances: &mut Vec<IndexedInstance>,
            visited: &mut Vec<TypeReferenceHandle>,
        ) {
            if !type_reference.is_valid() || visited.contains(&type_reference) {
                return;
            }
            visited.push(type_reference);
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => {
                    collect_type_indexed_instances(program, *referee, instances, visited)
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    collect_type_indexed_instances(program, *base_type, instances, visited);
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        let TypeConstraintNode::Domain(domain) = constraint else {
                            continue;
                        };
                        if domain.symbol.is_valid() && !domain.arguments.is_empty() {
                            instances.push(instance_from_constraint(program, domain));
                        }
                    }
                }
                TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::FixedArray { .. }
                | TypeReferenceNode::Generic { .. }
                | TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::Unit => {}
            }
        }

        pub(super) fn instance_from_constraint(
            program: &dyn ::validation::CheckingInput,
            domain: &DomainConstraint,
        ) -> IndexedInstance {
            IndexedInstance {
                family: domain.symbol,
                semantic_id: domain.semantic_id,
                arguments: domain.arguments.clone(),
                label: domain_label(program, domain.name.as_str(), &domain.arguments),
            }
        }

        /*
            The laws each distinct selected operation in `arguments` records, one clause
            per requirement. A rejected compatibility names what licensed normalization
            could have consumed, so a missing law reads as one instead of leaving the
            reader to guess which of commutativity, associativity or distributivity the
            two expressions needed.
        */
        pub(super) fn selected_operation_licenses(
            program: &dyn ::validation::CheckingInput,
            arguments: impl Iterator<Item = TypeReferenceHandle>,
        ) -> Vec<String> {
            let mut licenses = Vec::new();
            for argument in arguments {
                let TypeReferenceNode::ConstExpression(expression) =
                    program.type_reference_table.type_reference(argument)
                else {
                    continue;
                };
                for operation in expressions(program).open_index_operations(*expression) {
                    let mut laws = Vec::new();
                    if operation.commutativity_licensed {
                        laws.push("commutativity".to_owned());
                    }
                    if operation.associativity_licensed {
                        laws.push("associativity".to_owned());
                    }
                    for addition in &operation.distributes_over {
                        laws.push(format!("distributivity over `{addition}`"));
                    }
                    let clause = format!(
                        "`{}` of `{}` licenses {}",
                        operation.algebra_requirement,
                        program.symbols.display_path(operation.algebra_trait, "::"),
                        if laws.is_empty() {
                            "no law".to_owned()
                        } else {
                            laws.join(", ")
                        }
                    );
                    if !licenses.contains(&clause) {
                        licenses.push(clause);
                    }
                }
            }
            licenses
        }

        pub(super) fn selected_operation_count(
            program: &dyn ::validation::CheckingInput,
            arguments: impl Iterator<Item = TypeReferenceHandle>,
        ) -> usize {
            let mut selected_expressions = Vec::new();
            for argument in arguments {
                let TypeReferenceNode::ConstExpression(expression) =
                    program.type_reference_table.type_reference(argument)
                else {
                    continue;
                };
                if !selected_expressions.contains(expression) {
                    selected_expressions.push(*expression);
                }
            }
            selected_expressions
                .into_iter()
                .map(|expression| {
                    expressions(program)
                        .open_index_operations(expression)
                        .count()
                })
                .sum()
        }
    }
    mod labels {
        /*
            Diagnostic labels: the domain, point, carrier, and family names
            index-compatibility diagnostics print.
        */
        use super::instance_collection::IndexedInstance;
        use crate::flow::expressions;
        use facts::ProgramPoint;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        /*
            The family spelling inside an instance label: `Resident<SlotPlacement, Slot>`
            names family `Resident`, so a diagnostic can name the family whose
            establishment is missing beside the exact instance that was declared.
        */
        pub(super) fn family_label(instance_label: &str) -> &str {
            instance_label
                .split_once('<')
                .map_or(instance_label, |(family, _)| family)
        }

        /* Whether the declared family carries no predicate body. An unresolved symbol */
        pub(super) fn compatibility_name(
            program: &dyn ::validation::CheckingInput,
            actual: &IndexedInstance,
            expected: &IndexedInstance,
            point: ProgramPoint,
        ) -> String {
            format!(
                "index-equality:{}:{}:{}=={}",
                program.symbols.display_path(actual.family, "::"),
                point_label(program, point),
                actual.label,
                expected.label,
            )
        }

        pub(super) fn domain_label(
            program: &dyn ::validation::CheckingInput,
            name: &str,
            arguments: &[TypeReferenceHandle],
        ) -> String {
            if arguments.is_empty() {
                return name.to_owned();
            }
            let arguments = arguments
                .iter()
                .map(|argument| index_argument_label(program, *argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}<{arguments}>")
        }

        pub(super) fn qualification_label(
            program: &dyn ::validation::CheckingInput,
            cast: &typed_trees::expression::TableCastExpression,
        ) -> String {
            let name = expressions(program).name_path_string(cast.semantic_domain);
            domain_label(
                program,
                &name,
                program
                    .type_reference_table
                    .type_reference_handles(cast.semantic_domain_arguments),
            )
        }

        pub(super) fn index_argument_label(
            program: &dyn ::validation::CheckingInput,
            argument: TypeReferenceHandle,
        ) -> String {
            match program.type_reference_table.type_reference(argument) {
                TypeReferenceNode::Named { name, .. } => {
                    language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                        .map_or_else(|| name.to_string(), |value| value.display)
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    index_expression_label(program, *expression)
                }
                _ => program.display_type_reference(argument),
            }
        }

        /*
            An index expression with the parentheses its structure needs. The shared
            expression display writes `A * (B + C)` as `A * B + C`, a different
            expression, so a rejected compatibility would name the wrong operands.
            Only the index fragment's four operators bind here; any other operator
            keeps the shared display.
        */
        fn index_expression_label(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> String {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                return program.expression_table.display_name(expression);
            };
            let strength = |operator| match operator {
                BinaryOperator::Multiply | BinaryOperator::Divide => Some(2),
                BinaryOperator::Add | BinaryOperator::Subtract => Some(1),
                _ => None,
            };
            let operand = |child: ExpressionHandle, right: bool| {
                let text = index_expression_label(program, child);
                match program.expression_table.binary_node(child) {
                    Some(inner) => match (strength(inner.operator), strength(binary.operator)) {
                        (Some(inner_strength), Some(outer_strength))
                            if inner_strength < outer_strength
                                || (inner_strength == outer_strength && right) =>
                        {
                            format!("({text})")
                        }
                        _ => text,
                    },
                    None => text,
                }
            };
            format!(
                "{} {} {}",
                operand(binary.left, false),
                binary.operator.display_name(),
                operand(binary.right, true)
            )
        }

        pub(super) fn point_label(
            program: &dyn ::validation::CheckingInput,
            point: ProgramPoint,
        ) -> String {
            let symbol = |symbol| {
                let path = program.symbols.display_path(symbol, "::");
                if path.is_empty() {
                    format!("symbol-{}", symbol.arena_index())
                } else {
                    path
                }
            };
            match point {
                ProgramPoint::Global => "global".to_owned(),
                ProgramPoint::Definition { symbol: definition } => symbol(definition),
                ProgramPoint::Machine { machine_symbol } => symbol(machine_symbol),
                ProgramPoint::State { state_symbol, .. } => symbol(state_symbol),
                ProgramPoint::Call {
                    state_symbol,
                    statement_index,
                    call_ordinal,
                    ..
                } => format!(
                    "{}:call-{statement_index}-{call_ordinal}",
                    symbol(state_symbol)
                ),
                ProgramPoint::CallRequires {
                    state_symbol,
                    statement_index,
                    call_ordinal,
                    ..
                } => format!(
                    "{}:call-requires-{statement_index}-{call_ordinal}",
                    symbol(state_symbol)
                ),
                ProgramPoint::CallEnsures {
                    state_symbol,
                    statement_index,
                    call_ordinal,
                    ..
                } => format!(
                    "{}:call-ensures-{statement_index}-{call_ordinal}",
                    symbol(state_symbol)
                ),
                ProgramPoint::Exit {
                    state_symbol,
                    statement_index,
                    ..
                } => format!("{}:exit-{statement_index}", symbol(state_symbol)),
                ProgramPoint::TransitionArm {
                    state_symbol,
                    statement_index,
                    transition_target,
                    ..
                } => format!(
                    "{}:arm-{statement_index}-{}-{}",
                    symbol(state_symbol),
                    transition_target.arena_index(),
                    transition_target.generation(),
                ),
                ProgramPoint::Statement {
                    state_symbol,
                    statement_index,
                    ..
                } => format!("{}:statement-{statement_index}", symbol(state_symbol)),
            }
        }
    }
    mod substitutions {
        /*
            Call and fact substitutions: enclosing-call and value-call argument
            substitution, established index equalities, and substituted-expression
            matching.
        */
        use super::index_terms::{index_arguments_equal, index_identity_bindings};
        use super::instance_collection::StateCallIndex;
        use crate::flow::expressions;
        use arena::HandleSpan;
        use checked_trees::{
            CheckedOperatorFacts, CheckedOperatorOccurrence, FlowFacts, FlowSemanticContextRef,
        };
        use facts::{FactHandle, FactPlan, ProgramPoint};
        use symbols::SymbolHandle;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
        };
        use typed_trees::type_identity::{IndexIdentityArgument, normalized_index_identity};
        use typed_trees::types::TypeReferenceHandle;

        pub(super) fn established_index_equalities(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            semantic: &FactPlan,
            flow: &FlowFacts,
            contexts: HandleSpan<FlowSemanticContextRef>,
            actual: &[TypeReferenceHandle],
            expected: &[TypeReferenceHandle],
            actual_substitutions: &BoundIndexSubstitutions,
            expected_substitutions: &BoundIndexSubstitutions,
        ) -> Option<Vec<FactHandle>> {
            if actual.len() != expected.len() {
                return None;
            }
            let actual_bindings = index_identity_bindings(actual_substitutions);
            let expected_bindings = index_identity_bindings(expected_substitutions);
            let differing = actual
                .iter()
                .zip(expected)
                .filter(|(left, right)| {
                    !index_arguments_equal(
                        program,
                        **left,
                        **right,
                        &actual_bindings,
                        &expected_bindings,
                    )
                })
                .collect::<Vec<_>>();
            if differing.is_empty() {
                return None;
            }

            let mut evidence = Vec::new();
            for (actual, expected) in differing {
                let fact = established_index_equality_for_argument(
                    program,
                    operators,
                    semantic,
                    flow,
                    contexts,
                    *actual,
                    *expected,
                    actual_substitutions,
                    expected_substitutions,
                )?;
                crate::shared::collections::push_unique(&mut evidence, fact);
            }
            Some(evidence)
        }

        pub(super) fn established_index_equality_for_argument(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            semantic: &FactPlan,
            flow: &FlowFacts,
            contexts: HandleSpan<FlowSemanticContextRef>,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
            actual_substitutions: &BoundIndexSubstitutions,
            expected_substitutions: &BoundIndexSubstitutions,
        ) -> Option<FactHandle> {
            for context_ref in flow.contexts.semantic_context_refs.span_or_empty(contexts) {
                let context = semantic.contexts.get(context_ref.context);
                for fact_ref in semantic.refs.span_or_empty(context.facts) {
                    let fact = semantic.facts.get(fact_ref.fact);
                    let Some(expression) = fact.payload.boolean_expression() else {
                        continue;
                    };
                    let substitutions = fact_substitutions(program, flow, fact.point);
                    if expression_proves_index_equality(
                        program,
                        operators,
                        expression,
                        &substitutions,
                        actual,
                        expected,
                        actual_substitutions,
                        expected_substitutions,
                    ) {
                        return Some(fact_ref.fact);
                    }
                }
            }
            None
        }

        pub(super) fn expression_proves_index_equality(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            substitutions: &BoundIndexSubstitutions,
            actual: TypeReferenceHandle,
            expected: TypeReferenceHandle,
            actual_substitutions: &BoundIndexSubstitutions,
            expected_substitutions: &BoundIndexSubstitutions,
        ) -> bool {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                return false;
            };
            if binary.operator == BinaryOperator::And {
                return expression_proves_index_equality(
                    program,
                    operators,
                    binary.left,
                    substitutions,
                    actual,
                    expected,
                    actual_substitutions,
                    expected_substitutions,
                ) || expression_proves_index_equality(
                    program,
                    operators,
                    binary.right,
                    substitutions,
                    actual,
                    expected,
                    actual_substitutions,
                    expected_substitutions,
                );
            }
            if binary.operator != BinaryOperator::Equal
                || !has_builtin_equality_meaning(program, operators, expression)
            {
                return false;
            }
            let direct = substituted_expression_matches_index_argument(
                program,
                binary.left,
                substitutions,
                actual,
                actual_substitutions,
            ) && substituted_expression_matches_index_argument(
                program,
                binary.right,
                substitutions,
                expected,
                expected_substitutions,
            );
            let symmetric = substituted_expression_matches_index_argument(
                program,
                binary.left,
                substitutions,
                expected,
                expected_substitutions,
            ) && substituted_expression_matches_index_argument(
                program,
                binary.right,
                substitutions,
                actual,
                actual_substitutions,
            );
            direct || symmetric
        }

        /*
            A Boolean implementation relation is not substitution equality. Consume
            the checked domain selection when present; otherwise preparation must have
            retained builtin meaning on every authored occurrence. Absence of an
            operator row or a still-open selection grants no mathematical interpretation.
        */
        fn has_builtin_equality_meaning(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
        ) -> bool {
            let mut checked = false;
            let mut builtin = true;
            operators.for_each_expression_use(expression, |_, use_fact| {
                if use_fact.occurrence != CheckedOperatorOccurrence::Expression {
                    return;
                }
                checked = true;
                builtin &= use_fact
                    .resolved_through_builtin_fallback(language_core::OperatorSpelling::Equal);
            });
            if checked {
                return builtin;
            }
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionTarget,
            };
            let mut authored = false;
            for occurrence in expressions(program).authored_selection_occurrences(expression) {
                authored = true;
                if !program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        selection.target()
                            == AuthoredDeclarationSelectionTarget::Intrinsic(
                                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                            )
                    })
                {
                    return false;
                }
            }
            authored
        }

        pub(super) fn fact_substitutions<'program>(
            program: &'program dyn ::validation::CheckingInput,
            flow: &FlowFacts,
            point: ProgramPoint,
        ) -> BoundIndexSubstitutions<'program> {
            let ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            } = point
            else {
                return Vec::new();
            };
            let Some(call_site) = validation::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            ) else {
                return Vec::new();
            };
            let target_symbol = match &call_site {
                validation::CallSite::Statement(call) => call.target_symbol,
                validation::CallSite::Expression { call, .. } => {
                    validation::call_target_symbol(program, call)
                }
                validation::CallSite::TransitionNamed { .. } => call_flow_at_point(flow, point)
                    .map_or_else(SymbolHandle::invalid, |call| call.target_symbol),
            };
            let Some(parameters) = validation::call_target_parameters(program, target_symbol)
            else {
                return Vec::new();
            };
            let arguments = validation::call_site_argument_expressions(program, &call_site);
            let application = match &call_site {
                validation::CallSite::Statement(call) => call.authored_machine_application,
                validation::CallSite::Expression { call, .. } => call.authored_machine_application,
                validation::CallSite::TransitionNamed { .. } => {
                    typed_trees::static_applications::StaticMachineApplicationHandle::invalid()
                }
            };
            let mut substitutions = bound_index_substitutions(program, application);
            substitutions.extend(parameters.iter().zip(arguments).map(|(parameter, value)| {
                (parameter.symbol, BoundIndexArgument::Expression(*value))
            }));
            substitutions
        }

        pub(super) fn call_flow_at_point(
            flow: &FlowFacts,
            point: ProgramPoint,
        ) -> Option<&checked_trees::FlowCallFact> {
            let (machine_symbol, state_symbol, statement_index, call_ordinal) = match point {
                facts::call_point_triple_pattern!(
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    call_ordinal,
                    ..
                ) => (machine_symbol, state_symbol, statement_index, call_ordinal),
                _ => return None,
            };
            let state = flow.control.state(machine_symbol, state_symbol)?;
            flow.control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .find(|call| {
                    call.statement_index == statement_index && call.call_ordinal == call_ordinal
                })
        }

        /*
            Compatibility uses the producer's exact binder associations, not another
            walk over argument spellings or a reconstruction of the erased telescope.
            Type and const binders both index domain families; omitting either kind
            leaves a forwarded requirement in the callee's declaration namespace.
        */
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum BoundIndexArgument<'program> {
            Reference(TypeReferenceHandle),
            Subject(SymbolHandle),
            Algebra(&'program StaticMachineArgument),
            Expression(ExpressionHandle),
        }

        pub(crate) type BoundIndexSubstitutions<'program> =
            Vec<(SymbolHandle, BoundIndexArgument<'program>)>;

        pub(crate) fn bound_index_substitutions(
            program: &dyn ::validation::CheckingInput,
            application: typed_trees::static_applications::StaticMachineApplicationHandle,
        ) -> BoundIndexSubstitutions<'_> {
            use typed_trees::static_applications::StaticMachineBindingValue;
            if !application.is_valid() {
                return Vec::new();
            }
            expressions(program)
                .static_machine_application_bindings(application)
                .iter()
                .filter_map(|binding| {
                    Some((
                        binding.parameter,
                        match &binding.value {
                            StaticMachineBindingValue::Type(reference)
                            | StaticMachineBindingValue::Const(reference) => {
                                BoundIndexArgument::Reference(*reference)
                            }
                            StaticMachineBindingValue::RuntimeValue {
                                subject,
                                expression,
                                ..
                            } if expression.is_valid() => {
                                BoundIndexArgument::Expression(*expression)
                            }
                            StaticMachineBindingValue::RuntimeValue { subject, .. }
                                if subject.is_valid() =>
                            {
                                BoundIndexArgument::Subject(*subject)
                            }
                            StaticMachineBindingValue::Conformance(argument) => {
                                BoundIndexArgument::Algebra(argument)
                            }
                            _ => return None,
                        },
                    ))
                })
                .collect()
        }

        /*
            Keep both declaration and prepared-view binders attached to the exact
            invocation operands. Contract authority still names the declaration, while
            its selected payload uses freshly scoped member parameters.
        */
        pub(crate) fn call_index_substitutions<'program>(
            program: &'program dyn ::validation::CheckingInput,
            lexical_owner: SymbolHandle,
            site: &validation::CallSite<'_>,
        ) -> BoundIndexSubstitutions<'program> {
            let (application, target) = match site {
                validation::CallSite::Statement(call) => {
                    (call.authored_machine_application, call.target_symbol)
                }
                validation::CallSite::Expression { call, .. } => (
                    call.authored_machine_application,
                    call.static_requirement_dispatch
                        .as_ref()
                        .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol()),
                ),
                validation::CallSite::TransitionNamed { .. } => return Vec::new(),
            };
            let mut bindings = bound_index_substitutions(program, application);
            if let Some(parameters) = validation::call_target_parameters(program, target) {
                bindings.extend(
                    parameters
                        .iter()
                        .zip(validation::call_site_argument_expressions(program, site))
                        .map(|(parameter, actual)| {
                            (parameter.symbol, BoundIndexArgument::Expression(*actual))
                        }),
                );
            }
            if let validation::CallSite::Expression { call, .. } = site
                && let Some((_, declared)) = program.trait_machine_signature(target)
                && let Some(selected) = program.requirement_call_signature(lexical_owner, call)
            {
                let aliases = bindings
                    .iter()
                    .filter_map(|(parameter, actual)| {
                        let applied =
                            program.applied_signature_parameter(declared, selected, *parameter)?;
                        (applied != *parameter).then_some((applied, *actual))
                    })
                    .collect::<Vec<_>>();
                bindings.extend(aliases);
            }
            bindings
        }

        /*
            The substitutions implied by the call at `point`, if `point` is a call
            boundary — this is what lets an argument in `Coordinate<N>` discharge a
            parameter declared `in Coordinate<I>` when the call bound `I` to `N`.
        */
        pub(super) fn enclosing_call_substitutions<'program>(
            program: &'program dyn ::validation::CheckingInput,
            lexical_owner: SymbolHandle,
            state_calls: &StateCallIndex<'_, '_>,
            point: ProgramPoint,
        ) -> BoundIndexSubstitutions<'program> {
            let ProgramPoint::Call {
                statement_index,
                call_ordinal,
                ..
            } = point
            else {
                return Vec::new();
            };
            let Some(resolved) = state_calls.calls.iter().find(|resolved| {
                resolved.fact.statement_index == statement_index
                    && resolved.fact.call_ordinal == call_ordinal
            }) else {
                return Vec::new();
            };
            call_index_substitutions(program, lexical_owner, &resolved.site)
        }

        /*
            The substitutions implied by `value` itself when the checked value is a
            call whose declared result carries the callee's binders — a call result
            `Coordinate<I>` compares as `Coordinate<N>` under the call's own binding.
        */
        pub(super) fn value_call_substitutions(
            program: &dyn ::validation::CheckingInput,
            lexical_owner: SymbolHandle,
            value: ExpressionHandle,
        ) -> BoundIndexSubstitutions<'_> {
            let mut node = value;
            let call = loop {
                match program.expression_table.expression(node) {
                    ExpressionNode::Borrow(inner) => node = inner.target,
                    ExpressionNode::Atomic(atomic) => node = atomic.result,
                    ExpressionNode::Call(call) => break call,
                    _ => return Vec::new(),
                }
            };
            call_index_substitutions(
                program,
                lexical_owner,
                &validation::CallSite::Expression {
                    expression: node,
                    call,
                },
            )
        }

        /*
            Substitute exact fact parameters and call indices in their respective scopes,
            then compare through the canonical selected-operation normalizer. Established
            facts discharge compatibility; they never rewrite the declared index identity.
        */
        pub(super) fn substituted_expression_matches_index_argument(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            substitutions: &BoundIndexSubstitutions,
            argument: TypeReferenceHandle,
            bound_substitutions: &BoundIndexSubstitutions,
        ) -> bool {
            let fact_bindings = index_identity_bindings(substitutions);
            let index_bindings = index_identity_bindings(bound_substitutions);
            let fact_identity = normalized_index_identity(
                program,
                IndexIdentityArgument::Expression(expression),
                &fact_bindings,
            );
            let index_identity = normalized_index_identity(
                program,
                IndexIdentityArgument::TypeReference(argument),
                &index_bindings,
            );
            fact_identity
                .zip(index_identity)
                .is_some_and(|(fact, index)| fact == index)
        }
    }
    mod unevidenced_diagnostics {
        /*
            Missing indexed result-establishment diagnostics. Cast obligations belong to
            the selected domain's normalized theory, never to sibling domains on its carrier.
        */
        use super::instance_collection::{CompatibilityKey, IndexedInstance};
        use super::labels::{family_label, point_label};
        use diagnostics::Diagnostic;
        use facts::ProgramPoint;
        use language_semantics::SemanticDomainId;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::TypeReferenceHandle;

        /*
            A restating write whose declared type names an indexed instance of a family
            with no predicate body must be handed that instance by the value itself.
            Membership in such a family cannot follow from proving predicates, so the
            write establishes nothing on its own: `wiki/spec/language/domains.md` keeps
            establishment on the value's own route (transported qualification, an
            authorized `established by` route, or an explicit qualification of a
            predicate-free, route-free family), and
            `wiki/spec/resources/placed_access.md` fixes the entire route set of
            `Extent::Resident<P, T>`: initialization from `Vacant` and an owned `T`, a
            `ResidentContentTransfer<P, T>` issuance occurrence with its receipt, or
            forwarding an existing custody.

            A value-position call's result is exactly what its signature and contract
            row say it is. When neither the declared return type nor an `ensures` on the
            reserved `result` names any instance of the family, the restatement has no
            establishment at all, and refusing it here is what stops the local's
            declared-type seeding (`flow/transfers.rs`) from manufacturing the fact that
            later call `requires` consume. An instance either route does name keeps its
            ordinary index-compatibility treatment above, including the distinct
            normalized instance refusal, and a predicate-bearing domain keeps its
            `checks/contracts/writes.rs` discharge untouched.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn append_unevidenced_establishment_diagnostics(
            program: &dyn ::validation::CheckingInput,
            value: ExpressionHandle,
            target_type: TypeReferenceHandle,
            point: ProgramPoint,
            actual: &[IndexedInstance],
            expected: &[IndexedInstance],
            diagnostics: &mut Vec<Diagnostic>,
            unresolved: &mut Vec<CompatibilityKey>,
        ) {
            if !matches!(
                program.expression_table.expression(value),
                ExpressionNode::Call(_)
            ) {
                return;
            }
            for expected in expected {
                if !expected.semantic_id.is_valid()
                    || expected.arguments.is_empty()
                    || actual.iter().any(|actual| actual.family == expected.family)
                    || !is_bodyless_domain_family(program, expected.family)
                {
                    continue;
                }
                let key = CompatibilityKey {
                    point,
                    value,
                    target_type,
                    family: expected.family,
                    actual: SemanticDomainId::NULL,
                    expected: expected.semantic_id,
                };
                if unresolved.contains(&key) {
                    continue;
                }
                unresolved.push(key);
                diagnostics.push(Diagnostic::error(format!(
                    "declared instance `{}` has no establishment: the call establishes no instance of \
             domain family `{}` -- neither its declared return type nor its `ensures` names one \
             -- and a domain family with no predicate body has no predicate the write could \
             prove; the value must already carry the instance, through the callee's `ensures \
             result in {}`, a declared place of that type, or the family's authorized \
             establishment route (at {})",
                    expected.label,
                    family_label(&expected.label),
                    expected.label,
                    point_label(program, point),
                )));
            }
        }

        /*
            An unresolved domain is not bodyless: only its declaration establishes
            that there are no predicates a receiving write could prove.
        */
        pub(super) fn is_bodyless_domain_family(
            program: &dyn ::validation::CheckingInput,
            family: SymbolHandle,
        ) -> bool {
            program
                .domain_definition(family)
                .is_some_and(|domain| !domain.predicate_presence.is_present())
        }
    }

    pub(crate) use index_terms::index_identity_bindings;
    use index_terms::{bound_index_arguments_equal, is_closed_index_argument};
    use instance_collection::{
        CompatibilityKey, StateCallIndex, append_call_result_ensured_instances,
        collect_type_indexed_instances, construction_field_type, expression_indexed_instances,
        literal_element_type, selected_operation_count, selected_operation_licenses,
    };
    use labels::compatibility_name;
    pub(crate) use substitutions::{
        BoundIndexArgument, BoundIndexSubstitutions, call_index_substitutions,
    };
    use substitutions::{
        enclosing_call_substitutions, established_index_equalities, value_call_substitutions,
    };
    use unevidenced_diagnostics::append_unevidenced_establishment_diagnostics;

    pub(super) fn build_index_compatibility_facts(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        semantic: &FactPlan,
        flow: &FlowFacts,
        proof: &ProofFacts,
    ) -> Result<IndexCompatibilityFacts, Vec<Diagnostic>> {
        let mut scan = CompatibilityScan {
            program,
            operators,
            semantic,
            flow,
            proof,
            conditions: Vec::new(),
            diagnostics: Vec::new(),
            unresolved: Vec::new(),
        };

        for (_, state_flow) in flow.control.states.iter() {
            let Some((machine, state)) = validation::machine_state_by_symbols(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
            ) else {
                continue;
            };

            let state_calls = StateCallIndex::new(program, flow, state_flow);
            for resolved in &state_calls.calls {
                let call = resolved.fact;
                let Some(parameters) = resolved.site.parameters(program, machine.symbol) else {
                    continue;
                };
                let arguments = validation::call_site_argument_expressions(program, &resolved.site);
                let point = ProgramPoint::Call {
                    machine_symbol: state_flow.machine_symbol,
                    state_symbol: state_flow.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                };
                for (argument, parameter) in arguments.iter().zip(parameters.iter()) {
                    scan.append(
                        machine,
                        state,
                        &state_calls,
                        call.statement_index,
                        *argument,
                        parameter.type_reference,
                        point,
                        call.entry_semantic_contexts,
                    );
                }
            }

            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                let statement_contexts = flow
                    .state_statement(state_flow, statement_index)
                    .map(|statement| statement.entry_semantic_contexts)
                    .unwrap_or(state_flow.entry_semantic_contexts);
                let statement_point = ProgramPoint::Statement {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index,
                };
                let mut append = |value: ExpressionHandle,
                                  target_type: TypeReferenceHandle,
                                  point: ProgramPoint| {
                    scan.append(
                        machine,
                        state,
                        &state_calls,
                        statement_index,
                        value,
                        target_type,
                        point,
                        statement_contexts,
                    );
                };
                let exit_point = |transition_target| ProgramPoint::Exit {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index,
                    transition_target,
                };
                match statement {
                    StatementNode::Assembly(_) => {}
                    StatementNode::LocalData(local) => {
                        append(local.initial_value, local.type_reference, statement_point);
                    }
                    StatementNode::Assignment(assignment) => {
                        if let Some(target_type) = validation::expression_type_reference_in_state(
                            program,
                            state.symbol,
                            statement_index,
                            assignment.target,
                        ) {
                            append(assignment.value, target_type, statement_point);
                        }
                    }
                    StatementNode::Expression(expression)
                        if statement_index + 1 == statements.len() =>
                    {
                        append(
                            *expression,
                            state.return_type,
                            exit_point(Default::default()),
                        );
                    }
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            let TransitionTargetNode::Value(value) =
                                program.statement_table.transition_target(target)
                            else {
                                continue;
                            };
                            append(*value, state.return_type, exit_point(target));
                        }
                    }
                    StatementNode::AssemblyFact(_)
                    | StatementNode::Call(_)
                    | StatementNode::Expression(_) => {}
                }
            }
        }

        if scan.diagnostics.is_empty() {
            Ok(IndexCompatibilityFacts {
                conditions: scan.conditions,
            })
        } else {
            Err(scan.diagnostics)
        }
    }

    /*
        One index-compatibility scan: the fixed check scope every value position
        resolves against, plus the three accumulators it appends to. Stating the
        scope once keeps each occurrence to the value position it contributes.
    */
    struct CompatibilityScan<'program> {
        program: &'program dyn ::validation::CheckingInput,
        operators: &'program CheckedOperatorFacts,
        semantic: &'program FactPlan,
        flow: &'program FlowFacts,
        proof: &'program ProofFacts,
        conditions: Vec<IndexCompatibilityFact>,
        diagnostics: Vec<Diagnostic>,
        unresolved: Vec<CompatibilityKey>,
    }

    impl CompatibilityScan<'_> {
        /*
            Check one value position: the value's indexed instances against the
            receiving type's, then the children the value carries. `contexts` is the
            statement's fallback local context; the value's own call result `ensures`
            are applied here, so a caller never pre-resolves that span.
        */
        fn append(
            &mut self,
            machine: &Machine,
            state: &State,
            state_calls: &StateCallIndex<'_, '_>,
            statement_index: usize,
            value: ExpressionHandle,
            target_type: TypeReferenceHandle,
            point: ProgramPoint,
            contexts: HandleSpan<FlowSemanticContextRef>,
        ) {
            if !value.is_valid() || !target_type.is_valid() {
                return;
            }
            let contexts = state_calls.contexts_after_value(statement_index, value, contexts);
            let expected_substitutions =
                enclosing_call_substitutions(self.program, machine.symbol, state_calls, point);
            let actual_substitutions =
                value_call_substitutions(self.program, machine.symbol, value);
            let mut actual = expression_indexed_instances(
                self.program,
                self.operators,
                machine,
                state,
                statement_index,
                value,
            );
            append_call_result_ensured_instances(
                self.program,
                self.proof,
                state_calls,
                machine,
                state,
                statement_index,
                value,
                &mut actual,
            );
            let mut expected = Vec::new();
            collect_type_indexed_instances(
                self.program,
                target_type,
                &mut expected,
                &mut Vec::new(),
            );
            for actual in &actual {
                for expected in expected
                    .iter()
                    .filter(|expected| expected.family == actual.family)
                {
                    if !actual.semantic_id.is_valid()
                        || !expected.semantic_id.is_valid()
                        || actual.arguments.is_empty()
                        || expected.arguments.is_empty()
                    {
                        continue;
                    }
                    let discharge = if bound_index_arguments_equal(
                        self.program,
                        &actual.arguments,
                        &expected.arguments,
                        &actual_substitutions,
                        &expected_substitutions,
                    ) {
                        if actual.arguments.iter().all(|argument| {
                            is_closed_index_argument(self.program, *argument, &actual_substitutions)
                        }) && expected.arguments.iter().all(|argument| {
                            is_closed_index_argument(
                                self.program,
                                *argument,
                                &expected_substitutions,
                            )
                        }) {
                            IndexCompatibilityDischarge::ClosedEvaluation
                        } else {
                            IndexCompatibilityDischarge::LicensedNormalization {
                                operation_count: selected_operation_count(
                                    self.program,
                                    actual.arguments.iter().chain(&expected.arguments).copied(),
                                ),
                            }
                        }
                    } else if let Some(facts) = established_index_equalities(
                        self.program,
                        self.operators,
                        self.semantic,
                        self.flow,
                        contexts,
                        &actual.arguments,
                        &expected.arguments,
                        &actual_substitutions,
                        &expected_substitutions,
                    ) {
                        IndexCompatibilityDischarge::EstablishedLocalFacts { facts }
                    } else {
                        let key = CompatibilityKey {
                            point,
                            value,
                            target_type,
                            family: actual.family,
                            actual: actual.semantic_id,
                            expected: expected.semantic_id,
                        };
                        if !self.unresolved.contains(&key) {
                            self.unresolved.push(key);
                            let name = compatibility_name(self.program, actual, expected, point);
                            let licenses = selected_operation_licenses(
                                self.program,
                                actual.arguments.iter().chain(&expected.arguments).copied(),
                            );
                            let licensed = if licenses.is_empty() {
                                String::new()
                            } else {
                                format!("; selected operations: {}", licenses.join("; "))
                            };
                            self.diagnostics.push(Diagnostic::error(format!(
                            "index compatibility condition `{name}` is not established: actual `{}` \
                         and expected `{}` are distinct normalized instances; closed evaluation, \
                         licensed normalization, or an exact local equality fact is required\
                         {licensed}",
                            actual.label, expected.label,
                        )));
                        }
                        continue;
                    };
                    let name = compatibility_name(self.program, actual, expected, point);
                    let candidate = IndexCompatibilityFact {
                        name,
                        point,
                        value,
                        target_type,
                        family: actual.family,
                        actual_instance: actual.semantic_id,
                        expected_instance: expected.semantic_id,
                        actual_label: actual.label.clone(),
                        expected_label: expected.label.clone(),
                        discharge,
                    };
                    if !self.conditions.iter().any(|existing| {
                        existing.point == candidate.point
                            && existing.value == candidate.value
                            && existing.target_type == candidate.target_type
                            && existing.family == candidate.family
                            && existing.actual_instance == candidate.actual_instance
                            && existing.expected_instance == candidate.expected_instance
                            && existing.actual_label == candidate.actual_label
                            && existing.expected_label == candidate.expected_label
                    }) {
                        self.conditions.push(candidate);
                    }
                }
            }

            append_unevidenced_establishment_diagnostics(
                self.program,
                value,
                target_type,
                point,
                &actual,
                &expected,
                &mut self.diagnostics,
                &mut self.unresolved,
            );
            self.append_children(
                machine,
                state,
                state_calls,
                statement_index,
                value,
                target_type,
                point,
                contexts,
            );
        }

        /*
            The nested value positions one checked value carries: a constructor's
            fields at their declared types, an array literal's elements at the
            element type, and a match dispatch's arms at the receiving type.
        */
        fn append_children(
            &mut self,
            machine: &Machine,
            state: &State,
            state_calls: &StateCallIndex<'_, '_>,
            statement_index: usize,
            value: ExpressionHandle,
            target_type: TypeReferenceHandle,
            point: ProgramPoint,
            contexts: HandleSpan<FlowSemanticContextRef>,
        ) {
            let mut children = Vec::new();
            match self.program.expression_table.expression(value) {
                ExpressionNode::StructLiteral(literal) => {
                    if let Some(definition) = self
                        .program
                        .data_definition_by_name_str(literal.type_name.as_str())
                        .filter(|definition| definition.type_parameters.is_empty())
                    {
                        for field in self.program.expression_table.struct_fields(literal.fields) {
                            if let Some(field_type) = construction_field_type(
                                self.program,
                                definition,
                                literal.case_name.as_ref().map(|name| name.as_str()),
                                field.name.as_str(),
                            ) {
                                children.push((field.value, field_type));
                            }
                        }
                    }
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    if let Some(element_type) = literal_element_type(self.program, target_type) {
                        children.extend(
                            expressions(self.program)
                                .expression_handles(*elements)
                                .iter()
                                .map(|element| (*element, element_type)),
                        );
                    }
                }
                ExpressionNode::Match(dispatch) => {
                    children.extend(
                        expressions(self.program)
                            .match_arms(dispatch.arms)
                            .iter()
                            .map(|arm| (arm.value, target_type)),
                    );
                }
                _ => {}
            }
            for (value, target_type) in children {
                self.append(
                    machine,
                    state,
                    state_calls,
                    statement_index,
                    value,
                    target_type,
                    point,
                    contexts,
                );
            }
        }
    }
}
mod integer_writes {
    /*
        Preserve intermediate writes independently from return and mutation summaries.

        The existing assignment-frame inventory owns destinations, scalar lowering
        owns RHS algebra, and flow owns pre-store facts. Joining those occurrences
        here lets call admission ask about every value written, including a value
        overwritten before return. This is body evidence, not a published precondition
        or a claim that a transitive call's writes have already been admitted.
    */
    use super::boolean_predicates::entry_path_condition;
    use crate::flow::indexed_statements;
    use checked_trees::{
        CheckedOperatorFacts, CheckedValueFacts, CheckedValueOrigin, CheckedValueStatementRole,
        FlowFacts, MutationFacts, PerformedIntegerWrite,
    };

    pub(super) mod obligations {
        /*
            Concrete physical-width demands over one performed write. These private
            predicates describe that occurrence; they are not authored call contracts.
        */
        use checked_trees::{
            CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
            CheckedScalarExpression as Scalar, PerformedIntegerWrite,
        };
        use numerics::bignum::BigInt;
        use semantic_vocabulary::IntegerType;
        use typed_trees::types::PrimitiveType;

        /*
            Return the condition sufficient for this occurrence to fit its destination.
            A missing condition conservatively requires the write to fit unconditionally;
            a missing value is not evidence that the occurrence performs no write.
        */
        pub(crate) fn integer_write_obligation(
            write: &PerformedIntegerWrite,
            stored: IntegerType,
        ) -> Option<Boolean> {
            if stored.is_address()
                || stored.bits() == 0
                || stored.bits() > 64
                || !stored.bits().is_multiple_of(8)
            {
                return None;
            }
            let primitive = write.primitive_type?;
            let semantic = crate::values::integer_type(primitive)?;
            if write
                .entry_value
                .as_ref()
                .is_some_and(|value| value.primitive_type() != Some(primitive))
            {
                return None;
            }
            let semantic_low = crate::values::integer_magnitude(semantic.minimum_value());
            let semantic_high = crate::values::integer_magnitude(semantic.maximum_value());
            let stored_low = crate::values::integer_magnitude(stored.minimum_value());
            let stored_high = crate::values::integer_magnitude(stored.maximum_value());
            let (low, high) = if let Some(range) = &write.integer_range {
                if range.minimum > range.maximum {
                    return None;
                }
                (
                    range.minimum.clone().max(semantic_low.clone()),
                    range.maximum.clone().min(semantic_high.clone()),
                )
            } else {
                (semantic_low.clone(), semantic_high.clone())
            };
            if low > high {
                return None;
            }
            if low >= stored_low && high <= stored_high {
                return Some(Boolean::Constant(true));
            }
            let value = write.entry_value.as_ref()?;
            let fits = if stored_high < semantic_low || stored_low > semantic_high {
                Boolean::Constant(false)
            } else {
                let lower = if stored_low > semantic_low {
                    Some(Boolean::IntegerComparison {
                        kind: Comparison::LessOrEqual,
                        left: Box::new(literal(&stored_low, primitive)?),
                        right: Box::new(value.clone()),
                    })
                } else {
                    None
                };
                let upper = if stored_high < semantic_high {
                    Some(Boolean::IntegerComparison {
                        kind: Comparison::LessOrEqual,
                        left: Box::new(value.clone()),
                        right: Box::new(literal(&stored_high, primitive)?),
                    })
                } else {
                    None
                };
                match (lower, upper) {
                    (Some(left), Some(right)) => Boolean::And {
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    (Some(bound), None) | (None, Some(bound)) => bound,
                    (None, None) => Boolean::Constant(true),
                }
            };
            Some(match &write.entry_condition {
                None | Some(Boolean::Constant(true)) => fits,
                Some(condition) => Boolean::Or {
                    left: Box::new(Boolean::Not(Box::new(condition.clone()))),
                    right: Box::new(fits),
                },
            })
        }

        fn literal(value: &BigInt, primitive: PrimitiveType) -> Option<Scalar> {
            let text = value.to_string();
            let digits = text.strip_prefix('-').unwrap_or(&text);
            let literal = numerics::literals::IntegerLiteral::from_parts(
                text.starts_with('-'),
                numerics::literals::IntegerRadix::Decimal,
                digits,
            )
            .ok()?
            .with_landing(numerics::literals::IntegerLanding {
                landed_type: crate::values::landed_for_primitive(primitive)?,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            });
            Some(Scalar::IntegerLiteral { literal })
        }
    }

    /*
        Join independently checked receiving constraints at one authored write.
        A declared integer bound and an interval-only data domain can both constrain
        the same field. Neither supersedes the other, and duplicate rows must not
        turn a completed occurrence into an ambiguous decision handle.
    */
    pub(super) fn retain_assignment_decision(
        program: &dyn ::validation::CheckingInput,
        proof: &mut checked_trees::ProofFacts,
        decision: checked_trees::CheckedIntegerAssignmentDecision,
    ) {
        let existing = proof
            .integer_assignments
            .iter()
            .find_map(|(handle, retained)| {
                (retained.atomic_resident == decision.atomic_resident
                    && retained.machine == decision.machine
                    && retained.state == decision.state
                    && retained.statement_index == decision.statement_index
                    && retained.target == decision.target
                    && retained.value == decision.value
                    && program.primitive_type_reference(retained.target_type)
                        == program.primitive_type_reference(decision.target_type))
                .then_some(handle)
            });
        if let Some(handle) = existing {
            let retained = proof.integer_assignments.get_mut(handle);
            retained.minimum = retained.minimum.clone().max(decision.minimum);
            retained.maximum = retained.maximum.clone().min(decision.maximum);
        } else {
            proof.integer_assignments.append(decision);
        }
    }

    /*
        Record every integer assignment's performed write, machine by machine.

        One machine's rows read only `&` inputs and no other machine's rows, so
        each machine is derived on the checking pool and the rows are inserted in
        roster order afterwards: the same rows in the same order as one serial
        pass, whatever the thread count.
    */
    pub(super) fn retain(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        values: &CheckedValueFacts,
        flow: &FlowFacts,
        semantic: &facts::FactPlan,
        frames: Option<&validation::CallFrameResolver<'_>>,
        mutation: &mut MutationFacts,
    ) {
        use rayon::prelude::*;
        let rows = validation::checking_thread_pool().install(|| {
            program
                .machines()
                .par_iter()
                .map(|machine| {
                    machine_integer_writes(
                        program, operators, values, flow, semantic, frames, machine,
                    )
                })
                .collect::<Vec<_>>()
        });
        for row in rows.into_iter().flatten() {
            mutation.integer_writes.insert(row);
        }
    }

    /* One machine's performed integer writes, in state and statement order. */
    fn machine_integer_writes(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        values: &CheckedValueFacts,
        flow: &FlowFacts,
        semantic: &facts::FactPlan,
        frames: Option<&validation::CallFrameResolver<'_>>,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<PerformedIntegerWrite> {
        let mut rows = Vec::new();
        for state in program.machine_states(machine) {
            let flow_state = flow.control.state(machine.symbol, state.symbol);
            for (statement_index, statement) in indexed_statements(program, state.statement_nodes) {
                let Some(assignment) = statement.as_assignment() else {
                    continue;
                };
                let primitive = validation::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    assignment.target,
                )
                .map(|reference| validation::assignment_value_type(program, reference))
                .and_then(|reference| program.primitive_type_reference(reference));
                let Some(primitive) =
                    primitive.filter(|primitive| primitive.accepts_integer_literal())
                else {
                    continue;
                };
                let origin = CheckedValueOrigin::StateStatement {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index,
                    role: CheckedValueStatementRole::AssignmentValue,
                };
                let mut ranges = values
                    .expression_values(assignment.value)
                    .map(|(_, value)| value)
                    .filter(|value| value.origin == origin);
                let integer_range = ranges.next().and_then(|value| {
                    ranges
                        .next()
                        .is_none()
                        .then(|| value.integer_range.clone())
                        .flatten()
                });
                let contexts = flow_state
                    .and_then(|state| {
                        flow.control
                            .statements
                            .span_or_empty(state.statements)
                            .iter()
                            .find(|fact| fact.statement_index == statement_index)
                    })
                    .map(|statement| {
                        flow.semantic_constraint_contexts(statement.pre_store_constraints)
                            .collect::<Vec<_>>()
                    });
                let entry_condition = contexts
                    .as_ref()
                    .and_then(|contexts| entry_path_condition(semantic, contexts, machine.symbol));
                let operand_writes =
                    frames.map(|frames| frames.statement_value_write_frame(machine, statement));
                let entry_value = crate::values::lower_entry_scalar(
                    program,
                    operators,
                    machine,
                    state,
                    statement_index,
                    assignment.value,
                    primitive,
                    &|place| {
                        let paths = operand_writes.as_ref()?.complete_paths()?;
                        if paths.iter().any(|written| {
                            validation::frame_paths_overlap(written, &place.label(program))
                        }) {
                            return None;
                        }
                        super::entry_parameter_place_before_statement(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            place,
                        )
                    },
                );
                rows.push(PerformedIntegerWrite {
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index,
                    destination: assignment.target,
                    value: assignment.value,
                    primitive_type: Some(primitive),
                    frame: frames.map_or_else(facts::NormalizedWriteFrame::opaque, |frames| {
                        frames.assignment_write_frame(machine, statement)
                    }),
                    integer_range,
                    entry_value,
                    entry_condition,
                });
            }
        }
        rows
    }
}
pub(crate) mod proof_applications {
    /*
        Capture proof-application relationships while checked declarations are live.

        The shared typed reference visitor supplies formation edges; operational
        facts retain call/cleanup/static-binding edges. No body or call selection is
        inferred here, and preserving metadata does not retain executable roots.
    */
    use checked_trees::{CheckedTrees, ProofApplicationPlan};
    use symbols::SymbolHandle;

    pub(crate) fn retain(checked: &CheckedTrees) -> ProofApplicationPlan {
        let typed = &checked.typed;
        let facts = &checked.facts;
        let completion_owners = typed
            .constant_recipes
            .iter()
            .filter(|(recipe, declaration)| {
                typed.closed_constant_recipe_value(*recipe).is_none()
                    && matches!(
                        declaration.position,
                        typed_trees::constant_recipes::RetainedConstantPosition::ContractExpression(
                            _
                        )
                    )
            })
            .flat_map(|(_, declaration)| {
                typed
                    .constant_expression_contexts
                    .span_or_empty(declaration.contexts)
            })
            .filter(|context| typed.machine_by_symbol(context.evaluation_entry).is_some())
            .map(|context| context.owner)
            .collect::<Vec<_>>();
        let has_obligations = !facts.constant_predicate_obligations.is_empty()
            || !facts.integer_range_obligations.is_empty()
            || !facts.constant_recipe_obligations.is_empty()
            || !completion_owners.is_empty()
            || typed
                .constant_recipes
                .iter()
                .any(|(recipe, _)| typed.closed_constant_recipe_value(recipe).is_none())
            || typed.const_declarations().iter().any(|declaration| {
                declaration.evaluated_value.is_none() && declaration.evaluation_entry.is_valid()
            })
            || !facts
                .flow
                .terminal_unit_effects
                .layout_observations
                .is_empty();
        let mut plan =
            typed_trees::typed_trees::calls::application_graph::source_application_plan(typed);
        plan.has_obligations = has_obligations;
        let symbols = &mut plan.symbols;
        plan.machines.for_each_mut(|_, machine| {
            machine.dynamic_realizations = symbols.insert_many(
                facts
                    .dynamic_conformances
                    .selections
                    .iter()
                    .filter(|selection| selection.machine == machine.machine)
                    .flat_map(|selection| selection.rows.iter().map(|row| row.realization_machine)),
            );
        });
        let owners: Vec<SymbolHandle> = facts
            .constant_predicate_obligations
            .iter()
            .map(|row| row.caller())
            .chain(
                facts
                    .integer_range_obligations
                    .iter()
                    .map(|row| row.owner()),
            )
            .chain(
                facts
                    .constant_recipe_obligations
                    .iter()
                    .map(|row| row.owner())
                    .filter(|owner| owner.is_valid()),
            )
            .chain(completion_owners)
            .collect();
        plan.obligation_owners = plan.symbols.insert_many(owners);
        plan.unscoped_recipe = facts
            .constant_recipe_obligations
            .iter()
            .any(|row| !row.owner().is_valid());
        plan
    }
}
pub(crate) use integer_writes::obligations::integer_write_obligation;
mod access_views_and_uses {
    /*
        Checked access view inputs and nominal machine use facts.
    */
    pub(crate) fn build_checked_access_view_inputs(
        program: &dyn ::validation::CheckingInput,
    ) -> Vec<checked_trees::CheckedAccessViewInput> {
        let mut inputs = Vec::new();
        for machine in program.machines() {
            if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !machine.body_is_present
                || !program.machine_type_parameters(machine).is_empty()
            {
                continue;
            }
            for state in program.machine_states(machine) {
                for (position, parameter) in program.state_parameters(state).iter().enumerate() {
                    let typed_trees::types::TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime: _,
                    } = program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    else {
                        continue;
                    };
                    let Some(view) = program.access_view_plan_for_type_reference(*referee) else {
                        continue;
                    };
                    inputs.push(checked_trees::CheckedAccessViewInput {
                        machine: machine.symbol,
                        state: state.symbol,
                        position: u32::try_from(position)
                            .expect("state parameter count must fit checked input position"),
                        parameter: parameter.symbol,
                        reference_access: *access,
                        binding_is_const: parameter.is_const,
                        binding_is_mutable: parameter.is_mutable,
                        view: view.data_symbol,
                        policy: view.policy_symbol,
                        policy_plan_machine: view.policy_plan_machine_symbol,
                        schema: view.schema_symbol,
                        placement: view.placement.clone(),
                    });
                }
            }
        }
        inputs
    }

    pub(crate) fn build_nominal_machine_use_facts(
        program: &dyn ::validation::CheckingInput,
        nominal_machine_uses: Vec<validation::ValidatedNominalMachineUse>,
        contract_plans: &checked_trees::MachineContractPlans,
    ) -> Result<checked_trees::NominalMachineUseFacts, Vec<diagnostics::Diagnostic>> {
        let mut checked = Vec::with_capacity(nominal_machine_uses.len());
        for nominal_use in nominal_machine_uses {
            let Some(published) = contract_plans.crash_capsule(
                nominal_use.satisfaction_trait,
                nominal_use.satisfaction_requirement,
            ) else {
                return Err(vec![diagnostics::Diagnostic::error(
                    "admitted nominal machine use is missing its published requirement contract identity",
                )]);
            };
            let Some(actual) = contract_plans.for_machine(nominal_use.selected_machine) else {
                return Err(vec![diagnostics::Diagnostic::error(
                    "admitted nominal machine use is missing its selected machine contract identity",
                )]);
            };
            let Some(actual_envelope) =
                contract_plans.realized_envelope(nominal_use.selected_machine)
            else {
                return Err(vec![diagnostics::Diagnostic::error(
                    "admitted nominal machine use is missing its realized contract envelope",
                )]);
            };
            let published_fingerprint = published.target_contract_report_fingerprint();
            let published_commitment = published.target_contract_commitment();
            let actual_fingerprint = actual.report_fingerprint;
            if published_commitment.is_zero()
                || actual.commitment.is_zero()
                || actual_envelope.contract_report_fingerprint != actual_fingerprint
                || actual_envelope.contract_commitment != actual.commitment
            {
                return Err(vec![diagnostics::Diagnostic::error(
                    "admitted nominal machine use retained an empty contract-envelope identity",
                )]);
            }
            let callback_entry = match program.traits().iter().find(|definition| {
                definition.symbol == nominal_use.receiving_trait
                    && validation::boundary_has_explicit_calling_policy(program, definition)
            }) {
                Some(_) => {
                    let Some(resource_envelope) = contract_plans.resource_envelope(
                        nominal_use.selected_machine,
                        nominal_use.selected_entry,
                    ) else {
                        return Err(vec![diagnostics::Diagnostic::error(
                            "admitted nominal callback use is missing its exact checked entry resource envelope",
                        )]);
                    };
                    let resource_receipt =
                        checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(
                            resource_envelope,
                        )
                        .map_err(|error| {
                            vec![diagnostics::Diagnostic::error(format!(
                                "admitted nominal callback resource receipt failed checked replay: {error}"
                            ))]
                    })?;
                    Some(resource_receipt)
                }
                None => None,
            };
            let callback_requirements = if callback_entry.is_some() {
                let (_, signature) = program
                    .trait_machine_signature(nominal_use.satisfaction_requirement)
                    .ok_or_else(|| {
                        vec![diagnostics::Diagnostic::error(
                            "checked callback lost its exact inbound requirement",
                        )]
                    })?;
                program
                    .executable_requirement_machine_instances(signature)
                    .map(|instance| instance.symbol)
                    .collect()
            } else {
                Vec::new()
            };
            checked.push(checked_trees::CheckedNominalMachineUse {
                application_index: 0,
                site: match nominal_use.site {
                    validation::ValidatedNominalMachineUseSite::Statement(handle) => {
                        checked_trees::NominalMachineUseSite::Statement(handle)
                    }
                    validation::ValidatedNominalMachineUseSite::Expression(handle) => {
                        checked_trees::NominalMachineUseSite::Expression(handle)
                    }
                },
                registration_operation: nominal_use.registration_operation,
                registration_requirement_trait: nominal_use.registration_requirement_trait,
                static_machine_ordinal: nominal_use.static_machine_ordinal,
                selected_machine: nominal_use.selected_machine,
                selected_entry: nominal_use.selected_entry,
                receiving_trait: nominal_use.receiving_trait,
                receiving_application: nominal_use.receiving_application,
                satisfaction_trait: nominal_use.satisfaction_trait,
                satisfaction_requirement: nominal_use.satisfaction_requirement,
                canonical_requirement_overload: nominal_use.canonical_requirement_overload,
                published_requirement_envelope:
                    checked_trees::CheckedMachineContractEnvelopeIdentity {
                        contract_report_fingerprint: published_fingerprint,
                        contract_commitment: published_commitment,
                    },
                selected_actual_envelope: checked_trees::CheckedMachineContractEnvelopeIdentity {
                    contract_report_fingerprint: actual_fingerprint,
                    contract_commitment: actual.commitment,
                },
                callback_entry,
                callback_requirements,
                refinement: checked_trees::CheckedMachineContractRefinement {
                    published_requirement_report_fingerprint: published_fingerprint,
                    published_requirement_commitment: published_commitment,
                    selected_actual_report_fingerprint: actual_fingerprint,
                    selected_actual_commitment: actual.commitment,
                },
            });
        }
        checked_trees::NominalMachineUseFacts::try_with_uses(checked)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
    }
}
pub(crate) mod executable_bindings {
    /*
        Freeze the source-to-execution bindings after all typed arenas stop changing.
    */
    use checked_trees::{
        CheckedCaptureArgument, CheckedCaptureArgumentValue, CheckedCaptureInputs,
        CheckedExecutableBindings, CheckedExpressionSource, CheckedRequirementMachines,
        CheckedSelectedMachine, CheckedSourceCallTarget, CheckedSourceTransition,
    };
    use symbols::JoinedPath;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::static_applications::StaticMachineBindingValue;

    pub(crate) fn record(program: &dyn validation::CheckingInput) -> CheckedExecutableBindings {
        let mut facts = CheckedExecutableBindings::default();
        for (expression, node) in program.expression_table.iter_expressions() {
            let source = match node {
                ExpressionNode::SavedSubject(saved) => CheckedExpressionSource::Saved {
                    capture: saved.capture,
                    source: program
                        .expression_table
                        .saved_subject_source(saved)
                        .unwrap_or_else(typed_trees::expression::ExpressionHandle::invalid),
                },
                ExpressionNode::Call(call) => {
                    facts.calls.push(CheckedSourceCallTarget {
                        target: call.target_symbol,
                        diagnostic_name: call.target.as_str().to_owned(),
                    });
                    match program
                        .expression_table
                        .build_selection_request(call.build_selection)
                    {
                        Some(
                            typed_trees::expression::BuildSelectionRequest::ProviderReadiness(
                                request,
                            ),
                        ) => CheckedExpressionSource::ProviderReadiness {
                            selection: call.build_selection,
                            requirement: request.requirement,
                        },
                        _ => CheckedExpressionSource::Value,
                    }
                }
                _ => CheckedExpressionSource::Value,
            };
            facts.insert_expression(expression, source);
        }
        for (_, capture) in program.captured_machine_applications.iter() {
            let arguments = program
                .expression_table
                .static_machine_application_bindings(capture.binding_application)
                .iter()
                .map(|binding| CheckedCaptureArgument {
                    parameter: binding.parameter,
                    value: match &binding.value {
                        StaticMachineBindingValue::Type(reference) => {
                            CheckedCaptureArgumentValue::Type(
                                program.normalized_type_identity(*reference),
                            )
                        }
                        StaticMachineBindingValue::Const(reference) => {
                            CheckedCaptureArgumentValue::Const(
                                program
                                    .static_const_type_reference_identity(*reference)
                                    .map(|identity| program.type_identities.intern(&identity))
                                    .unwrap_or_default(),
                            )
                        }
                        StaticMachineBindingValue::Machine(_) => {
                            CheckedCaptureArgumentValue::Machine
                        }
                        StaticMachineBindingValue::RuntimeValue { .. } => {
                            CheckedCaptureArgumentValue::RuntimeValue
                        }
                        StaticMachineBindingValue::Conformance(_) => {
                            CheckedCaptureArgumentValue::Conformance
                        }
                    },
                })
                .collect();
            facts.captures.push(CheckedCaptureInputs {
                instance: capture.instance,
                arguments,
                machines: program
                    .expression_table
                    .static_machine_application_selected_machines(capture.binding_application)
                    .iter()
                    .map(|argument| argument.symbol)
                    .collect(),
            });
        }
        for application in program.closed_requirement_applications() {
            facts.requirements.push(CheckedRequirementMachines {
                requirement: application.signature.symbol,
                machines: program
                    .expression_table
                    .static_machine_application_bindings(application.application)
                    .iter()
                    .filter_map(|binding| match &binding.value {
                        StaticMachineBindingValue::Machine(target) => {
                            Some(CheckedSelectedMachine {
                                symbol: target.symbol,
                                diagnostic_path: target.path.joined_path(),
                            })
                        }
                        _ => None,
                    })
                    .collect(),
            });
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (statement_index, statement) in
                    crate::flow::indexed_statements(program, state.statement_nodes)
                {
                    if let typed_trees::statement::StatementNode::Call(call) = statement {
                        facts.calls.push(CheckedSourceCallTarget {
                            target: call.target_symbol,
                            diagnostic_name: call.target.as_str().to_owned(),
                        });
                    }
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    let named_targets = [transition.target, transition.continuation]
                        .into_iter()
                        .filter(|target| target.is_valid())
                        .filter_map(|target| {
                            match program.statement_table.transition_target(target) {
                                typed_trees::statement::TransitionTargetNode::Named {
                                    path,
                                    ..
                                } => Some(path.symbol),
                                _ => None,
                            }
                        })
                        .filter(|symbol| symbol.is_valid())
                        .collect();
                    facts.transitions.push(CheckedSourceTransition {
                        state: state.symbol,
                        statement_index,
                        primary: transition.target,
                        named_targets,
                    });
                }
            }
        }
        facts
    }

    pub(crate) fn freeze_contract_subjects(
        program: &dyn validation::CheckingInput,
        proof: &mut checked_trees::ProofFacts,
    ) {
        let handles = proof
            .contract_facts
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            let row = proof.contract_facts.get(handle);
            if row.evidence_term.is_none() {
                continue;
            }
            let checked_trees::ContractProofFactOwner::Machine { machine_symbol } = row.owner
            else {
                continue;
            };
            let Some(machine) = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)
            else {
                continue;
            };
            let Some(entry) = program.machine_states(machine).first() else {
                continue;
            };
            let Some(application) = program.proof_facts.get(row.fact).as_proposition() else {
                continue;
            };
            let roots = proof.nominal_argument_roots.insert_many(
                program
                    .expression_table
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|expression| {
                        validation::contract_argument_root(program, entry, *expression)
                    }),
            );
            proof.contract_facts.get_mut(handle).nominal_argument_roots = Some(roots);
        }
    }

    pub(crate) fn freeze_declaration_arguments(
        program: &dyn validation::CheckingInput,
        plan: &mut checked_trees::CheckedRankArrivalPlan,
    ) {
        let handles = plan
            .application_calls
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            let source = plan.application_calls.get(handle).arguments;
            let arguments = plan.application_call_arguments.insert_many(
                program
                    .expression_table
                    .expression_handles(source)
                    .iter()
                    .copied(),
            );
            plan.application_calls.get_mut(handle).checked_arguments = arguments;
        }
    }
}
mod machine_facts {
    /*
        Termination, blocking, suspension and synchronous invocation facts.
    */
    use crate::published::facts::{MachineBlockingRow, MachineSuspensionRow};

    pub(crate) fn build_termination_facts(
        program: &dyn ::validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        semantic: &facts::FactPlan,
        cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
        validation: &validation::ProgramValidationFacts,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Result<checked_trees::TerminationFacts, Vec<diagnostics::Diagnostic>> {
        let rank_arrivals =
            crate::checks::termination::admit_machine_rankings(program, call_frames);
        let summaries = crate::checks::termination::analyze_checked_progress(
            program,
            flow,
            semantic,
            cleanup_dependencies,
            &validation.proof_recursive_components,
            &validation.self_call_descents,
            call_frames,
            &rank_arrivals,
        )?;
        let mut summary_by_machine = symbols::SymbolKeyMap::<
            symbols::SymbolHandle,
            language_semantics::TerminationGuarantee,
        >::default();
        for summary in summaries {
            summary_by_machine
                .entry(summary.machine)
                .or_insert(summary.guarantee);
        }
        Ok(checked_trees::TerminationFacts {
            rank_arrivals,
            machines: program
                .machines()
                .iter()
                .map(|machine| checked_trees::MachineTerminationFact {
                    machine: machine.symbol,
                    plan: crate::checks::termination::build_checked_termination_plan(
                        program,
                        machine,
                        summary_by_machine
                            .get(&machine.symbol)
                            .cloned()
                            .unwrap_or(language_semantics::TerminationGuarantee::NoGuarantee),
                    ),
                })
                .collect(),
            proof_recursive_components: validation
                .proof_recursive_components
                .iter()
                .filter_map(|component| component.structural.as_ref())
                .map(
                    |component| checked_trees::CheckedProofRecursiveComponent {
                        members: component
                            .members
                            .iter()
                            .map(|member| checked_trees::CheckedProofRecursiveMember {
                                machine: member.machine,
                                rank_parameter: member.rank_parameter,
                            })
                            .collect(),
                        ranking_relation: match component.ranking_relation {
                            validation::ValidatedProofRankingRelation::StructuralSubterm => {
                                checked_trees::CheckedProofRankingRelation::StructuralSubterm
                            }
                            validation::ValidatedProofRankingRelation::StructuralSubtermCycles => {
                                checked_trees::CheckedProofRankingRelation::StructuralSubtermCycles
                            }
                        },
                        rank_type_identity: component.rank_type_identity,
                        edges: component
                            .edges
                            .iter()
                            .map(|edge| checked_trees::CheckedProofRecursiveEdge {
                                caller: edge.caller,
                                callee: edge.callee,
                                site: match edge.site {
                                    validation::ValidatedProofRecursiveCallSite::Statement {
                                        state,
                                        statement_index,
                                    } => checked_trees::CheckedProofRecursiveCallSite::Statement {
                                        state,
                                        statement_index,
                                    },
                                    validation::ValidatedProofRecursiveCallSite::Expression {
                                        state,
                                        statement_index,
                                        expression_ordinal,
                                    } => checked_trees::CheckedProofRecursiveCallSite::Expression {
                                        state,
                                        statement_index,
                                        expression_ordinal,
                                    },
                                    validation::ValidatedProofRecursiveCallSite::Transition {
                                        state,
                                        statement_index,
                                        lane,
                                    } => checked_trees::CheckedProofRecursiveCallSite::Transition {
                                        state,
                                        statement_index,
                                        lane: match lane {
                                            validation::ValidatedProofRecursiveTransitionLane::Target => checked_trees::CheckedProofRecursiveTransitionLane::Target,
                                            validation::ValidatedProofRecursiveTransitionLane::Continuation => checked_trees::CheckedProofRecursiveTransitionLane::Continuation,
                                        },
                                    },
                                },
                                caller_rank_parameter: edge.caller_rank_parameter,
                                callee_rank_parameter: edge.callee_rank_parameter,
                                strict_member_path: edge.strict_member_path.clone(),
                            })
                            .collect(),
                    },
                )
                .collect(),
        })
    }

    pub(crate) fn build_blocking_facts(
        program: &dyn ::validation::CheckingInput,
        blocking: &[MachineBlockingRow],
    ) -> checked_trees::BlockingFacts {
        let machines = program
            .machines()
            .iter()
            .map(|machine| {
                let blocking_row = blocking.iter().find(|row| row.symbol == machine.symbol);
                let checked_may_block = blocking_row.is_some_and(|row| row.transitive_may_block);
                let published_may_block =
                    blocking_row.map_or(machine.blocks, |row| row.published_may_block);
                let publishes_operational_contract = machine.is_public
                    || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody;
                checked_trees::MachineBlockingFact {
                    machine: machine.symbol,
                    plan: language_semantics::BlockingPlan {
                        interface: if publishes_operational_contract || machine.blocks {
                            language_semantics::BlockingInterface::PublishedMayBlock(
                                published_may_block,
                            )
                        } else {
                            language_semantics::BlockingInterface::InternalInferred
                        },
                        checked_may_block,
                    },
                }
            })
            .collect();
        checked_trees::BlockingFacts { machines }
    }

    pub(crate) fn build_suspension_facts(
        program: &dyn ::validation::CheckingInput,
        suspensions: &[MachineSuspensionRow],
    ) -> checked_trees::SuspensionFacts {
        let machines = program
            .machines()
            .iter()
            .map(|machine| {
                let suspension_row = suspensions.iter().find(|row| row.symbol == machine.symbol);
                let checked_may_suspend =
                    suspension_row.is_some_and(|row| row.transitive_may_suspend);
                let published_may_suspend =
                    suspension_row.map_or(machine.suspends, |row| row.published_may_suspend);
                let publishes_operational_contract = machine.is_public
                    || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody;
                checked_trees::MachineSuspensionFact {
                    machine: machine.symbol,
                    plan: language_semantics::SuspensionPlan {
                        interface: if publishes_operational_contract || machine.suspends {
                            language_semantics::SuspensionInterface::PublishedMaySuspend(
                                published_may_suspend,
                            )
                        } else {
                            language_semantics::SuspensionInterface::InternalInferred
                        },
                        checked_may_suspend,
                    },
                }
            })
            .collect();
        checked_trees::SuspensionFacts { machines }
    }

    pub(crate) fn build_synchronous_invocation_facts(
        program: &dyn ::validation::CheckingInput,
    ) -> checked_trees::SynchronousInvocationFacts {
        let inference = validation::infer_synchronous_invocations(program);
        let machines = program
            .machines()
            .iter()
            .map(|machine| {
                let invocation_summary = inference.for_machine(machine.symbol);
                let canonical_invocation = |target: flow_effects::InvocationTarget| match target {
                    flow_effects::InvocationTarget::Parameter(index) => {
                        format!("parameter:{index}")
                    }
                    flow_effects::InvocationTarget::Service(symbol) => program
                        .trait_definition_by_symbol(symbol)
                        .map(|definition| format!("service:{}", definition.name))
                        .unwrap_or_else(|| format!("service:#{}", symbol.arena_index())),
                };
                let published_targets = invocation_summary
                    .map(|summary| summary.published.clone())
                    .unwrap_or_default();
                let checked_inferred_targets = invocation_summary
                    .map(|summary| summary.inferred_transitive.clone())
                    .unwrap_or_default();
                let mut published = published_targets
                    .iter()
                    .copied()
                    .map(canonical_invocation)
                    .collect::<Vec<_>>();
                published.sort_unstable();
                published.dedup();
                let mut checked_inferred = checked_inferred_targets
                    .iter()
                    .copied()
                    .map(canonical_invocation)
                    .collect::<Vec<_>>();
                checked_inferred.sort_unstable();
                checked_inferred.dedup();
                let publishes_invocations = machine.supply_mode
                    != language_semantics::MachineSupplyMode::CheckedBody
                    || machine.is_public
                    || !program.machine_invokes(machine).is_empty();

                checked_trees::MachineSynchronousInvocationFact {
                    machine: machine.symbol,
                    published_targets,
                    checked_inferred_targets,
                    realized_targets: invocation_summary
                        .map(|summary| summary.realized.clone())
                        .unwrap_or_default(),
                    plan: language_semantics::SynchronousInvocationPlan {
                        interface: if publishes_invocations {
                            language_semantics::SynchronousInvocationInterface::PublishedCeiling
                        } else {
                            language_semantics::SynchronousInvocationInterface::InternalInferred
                        },
                        published,
                        checked_inferred,
                    },
                }
            })
            .collect();
        checked_trees::SynchronousInvocationFacts { machines }
    }
}
pub(crate) mod operator_crashes {
    /*
        Selected operator invocations have their own occurrence and operand custody.
        Their source crash routes never acquire fabricated ordinary-call coordinates.
    */
    use super::crash_entry_values::{
        entry_operand_projected, formal_member_projection, substitute_entry_projected,
    };
    use super::declared_parameter_names;
    use crate::flow::expressions;

    use checked_trees::{
        CheckedCrashOperatorSite, CheckedOperatorFacts, CheckedOperatorOccurrence,
        CheckedOperatorResolutionStatus, CheckedValueOrigin, CrashPredicateIdentity,
        CrashRouteBucket, CrashRouteGuard, FlowFacts,
    };
    use diagnostics::Diagnostic;
    use facts::FactPlan;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::types::TypeReferenceNode;

    mod named_routes {
        /*
            Route discharge for named `Namespace::requirement(...)` operator calls.

            A named use selects the same crash contract as a spelled use. The flow
            pass captures its operand-time evidence under a `FlowOperatorInvocationFact`
            keyed by the `named_use` handle rather than a `uses` row, so the
            operand-time `InvocationContexts` discharge in checks/operators applies
            unchanged: short-circuit premises, per-operand snapshots for copied scalars
            and stable by-value carriers, live-intersection for view carriers such as
            references and slices, and multi-operand per-context intersection all
            behave exactly as they do for `1 == value`.

            A named use whose position emitted no capture row — one inside a statement
            the expression scheduler never evaluates, or a row the operator-resolution
            stage never minted — has no operand-time custody to consult. The
            containing statement still records entry semantic contexts: facts the
            flow pass proved on arrival at that statement. Those facts describe
            storage as it stands at statement entry, so they falsify a route guard
            only while every place a guard leaf reads keeps its statement-entry value
            through the invocation. `operand_entry_provenance` supplies exactly that
            boundary per occurrence: its pristine-storage window includes the
            containing statement, so an earlier sibling effect — a receiver-mutating
            call, an exclusive borrow, an atomic — already voids provenance at the
            place the leaf reads, while writes confined to disjoint sibling fields
            leave it intact, and immutable bindings cannot be overwritten at all. A
            leaf occurrence that does not resolve to an entry-proven operand
            projection — a `self` field with unknown receiver provenance, a constant,
            an unproven binding — keeps the route, as does a leaf that would evaluate
            a nested call or atomic afresh rather than re-reading captured storage.
            A shared-borrow operand is the referent: the referent's provenance
            transports through the borrow and the instantiated label names the
            referent, mirroring the call-`requires` rule that a reference formal used
            as a predicate value reads its referent.
        */
        use super::super::contract_occurrences::append_expression_occurrences;
        use super::super::crash_entry_values::operand_entry_provenance;
        use checked_trees::FlowFacts;
        use facts::{FactContextHandle, FactPayload, FactPlan};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::signature::StateParameter;

        use crate::published::labels::{
            context_proves_boolean_label, instantiate_operator_contract_expression_label,
        };

        /*
            Whether the selected operator's published route guard is provably false at
            one named call. When the call's evaluation emitted an operand-time
            capture row, discharge goes through `checks::named_operator_route_is_false`
            with exactly the spelled use's selection rules — operand expressions must
            match, detached-copy carriers (copied scalars and stable by-value formals)
            keep operand-time snapshots, view carriers intersect capture with
            invocation-live facts, and several referenced
            operands intersect by exact context identity. Present-but-ambiguous or
            substituted custody fails closed: duplicated or mismatched rows select no
            contexts. A call with no capture row at all falls back to the containing
            statement's entry contexts, mirroring
            `checks::operator_route_is_false`'s Boolean polarity structure over rows
            every incoming path must satisfy.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn named_route_is_false(
            program: &dyn ::validation::CheckingInput,
            flow: &FlowFacts,
            semantic: &FactPlan,
            named_use: arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            parameters: &[StateParameter],
            operands: &[ExpressionHandle],
            expression: ExpressionHandle,
        ) -> bool {
            if flow
                .control
                .operator_invocations
                .iter()
                .any(|(_, invocation)| invocation.named_use == named_use)
            {
                let operand_labels =
                    super::named_call_operand_labels(program, parameters, operands);
                return crate::checks::named_operator_route_is_false(
                    program,
                    facts::ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                    },
                    flow,
                    semantic,
                    named_use,
                    parameters,
                    operands,
                    &operand_labels,
                    expression,
                );
            }
            statement_entry_route_is_false(
                program,
                flow,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                parameters,
                operands,
                expression,
            )
        }

        /*
            Shared entry-fact discharge for calls without operator operand captures.
            Ordinary machine calls use this same pristine-storage check after their
            selected contract's builtin meaning has been established. The containing
            statement remains inside the provenance window: sibling argument effects
            cannot make an old entry premise describe a new value.
        */
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn statement_entry_route_is_false(
            program: &dyn ::validation::CheckingInput,
            flow: &FlowFacts,
            semantic: &FactPlan,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            parameters: &[StateParameter],
            operands: &[ExpressionHandle],
            expression: ExpressionHandle,
        ) -> bool {
            let operand_labels = super::named_call_operand_labels(program, parameters, operands);
            let context_rows =
                statement_entry_context_rows(flow, machine_symbol, state_symbol, statement_index);
            if context_rows.is_empty() {
                return false;
            }
            expression_has_polarity(
                program,
                semantic,
                &context_rows,
                machine_symbol,
                state_symbol,
                statement_index,
                parameters,
                operands,
                &operand_labels,
                expression,
                false,
            )
        }

        /*
            The entry semantic contexts of every flow statement row at this source
            coordinate. A statement reached by several paths may carry several rows;
            every row must prove a leaf before it is discharged, so a fact live on
            only one incoming path can never narrow the surviving routes.
        */
        fn statement_entry_context_rows(
            flow: &FlowFacts,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
        ) -> Vec<Vec<FactContextHandle>> {
            flow.control
                .machine_states(machine_symbol)
                .into_iter()
                .filter(|state| state.state_symbol == state_symbol)
                .map(|state| ((), state))
                .flat_map(|(_, state)| {
                    flow.control
                        .statements
                        .span_or_empty(state.statements)
                        .iter()
                })
                .filter(|statement| statement.statement_index == statement_index)
                .map(|statement| {
                    flow.contexts
                        .semantic_context_refs
                        .span_or_empty(statement.entry_semantic_contexts)
                        .iter()
                        .map(|reference| reference.context)
                        .chain(flow.semantic_constraint_contexts(statement.entry_constraints))
                        .collect()
                })
                .collect()
        }

        #[allow(clippy::too_many_arguments)]
        fn expression_has_polarity(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            context_rows: &[Vec<FactContextHandle>],
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            before_statement: usize,
            parameters: &[StateParameter],
            operands: &[ExpressionHandle],
            operand_labels: &[String],
            expression: ExpressionHandle,
            polarity: bool,
        ) -> bool {
            crate::published::labels::boolean_expression_has_polarity(
                program,
                expression,
                polarity,
                &mut |expression, polarity| {
                    leaf_has_polarity(
                        program,
                        semantic,
                        context_rows,
                        machine_symbol,
                        state_symbol,
                        before_statement,
                        parameters,
                        operands,
                        operand_labels,
                        expression,
                        polarity,
                    )
                },
            )
        }

        fn leaf_has_polarity(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            context_rows: &[Vec<FactContextHandle>],
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            before_statement: usize,
            parameters: &[StateParameter],
            operands: &[ExpressionHandle],
            operand_labels: &[String],
            expression: ExpressionHandle,
            polarity: bool,
        ) -> bool {
            if !leaf_reads_only_entry_operands(
                program,
                machine_symbol,
                state_symbol,
                before_statement,
                parameters,
                operands,
                expression,
            ) || expression_evaluates_effects(program, expression)
            {
                return false;
            }
            let required = instantiate_operator_contract_expression_label(
                program,
                parameters,
                operand_labels,
                expression,
            );
            if polarity {
                return context_rows.iter().all(|row| {
                    row.iter().any(|context| {
                        context_proves_boolean_label(
                            program,
                            semantic,
                            *context,
                            row,
                            facts::ProgramPoint::Statement {
                                machine_symbol,
                                state_symbol,
                                statement_index: before_statement,
                            },
                            &required,
                        )
                    })
                });
            }
            context_rows.iter().all(|row| {
                row.iter().any(|context| {
                    semantic
                        .context_view(semantic.contexts.get(*context))
                        .facts()
                        .any(|fact| {
                            matches!(fact.payload, FactPayload::BooleanValue { expression, value: false }
                                if program.expression_table.display_name(expression) == required)
                        })
                })
            })
        }

        /*
            Every place occurrence the leaf reads must resolve to an operator
            parameter whose operand carries entry provenance *at the projected place*:
            only then does a statement-entry fact about that place still describe the
            value the invocation evaluates. The provenance is per occurrence rather
            than per operand — a mutable record whose `other` field was written still
            carries `count` provenance, so a guard leaf reading only `cell.count`
            through `&rec` discharges where whole-operand provenance would retain. A
            constant, a `self` field with unknown receiver provenance, or a binding
            whose projection was disturbed fails this gate and retains the guard.
        */
        fn leaf_reads_only_entry_operands(
            program: &dyn ::validation::CheckingInput,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            before_statement: usize,
            parameters: &[StateParameter],
            operands: &[ExpressionHandle],
            expression: ExpressionHandle,
        ) -> bool {
            let mut occurrences = Vec::new();
            append_expression_occurrences(program, expression, &mut occurrences);
            occurrences.iter().all(|occurrence| {
                let root = occurrence_root_symbol(program, *occurrence);
                parameters.iter().enumerate().any(|(index, parameter)| {
                    parameter.symbol == root
                        && operands.get(index).is_some_and(|operand| {
                            operand_entry_provenance(
                                program,
                                machine_symbol,
                                state_symbol,
                                before_statement,
                                *operand,
                                *occurrence,
                            )
                        })
                })
            })
        }

        /*
            A leaf proven by a statement-entry fact is re-evaluated at the invocation,
            so it must be a pure read of the proven storages. A nested call or atomic
            could observe storage beyond the operand occurrences — state a matching
            fact snapshot does not transport — and is kept conservative instead.
        */
        fn expression_evaluates_effects(
            program: &dyn ::validation::CheckingInput,
            root: ExpressionHandle,
        ) -> bool {
            let mut pending = vec![root];
            while let Some(expression) = pending.pop() {
                if !program.expression_table.expression_is_valid(expression) {
                    return true;
                }
                let node = program.expression_table.expression(expression);
                if matches!(node, ExpressionNode::Call(_) | ExpressionNode::Atomic(_)) {
                    return true;
                }
                program.subtree_children(node, typed_trees::SubtreeEdges::VALUES, |child| {
                    pending.push(child);
                    false
                });
            }
            false
        }

        /*
            Port of the requires prover's occurrence rooting: a place occurrence roots
            at the symbol its member/index/borrow selectors descend to.
        */
        fn occurrence_root_symbol(
            program: &dyn ::validation::CheckingInput,
            mut expression: ExpressionHandle,
        ) -> SymbolHandle {
            loop {
                match program.expression_table.expression(expression) {
                    ExpressionNode::Name(path) => return path.symbol,
                    ExpressionNode::Member(member) => expression = member.receiver,
                    ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                    ExpressionNode::Borrow(borrow) => expression = borrow.target,
                    _ => return SymbolHandle::invalid(),
                }
            }
        }
    }
    pub(super) use named_routes::statement_entry_route_is_false;

    pub(crate) fn build(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        flow: &FlowFacts,
        semantic: &FactPlan,
    ) -> Result<Vec<(SymbolHandle, CheckedCrashOperatorSite)>, Vec<Diagnostic>> {
        let mut sites = Vec::new();
        let mut diagnostics = Vec::new();
        let mut content_conservation = None;
        for (operator_use_handle, operator_use) in operators.uses.iter() {
            let Some(selected) = operators.selected_candidate(operator_use) else {
                continue;
            };
            if !program
                .signature_contracts
                .span_or_empty(selected.contracts)
                .iter()
                .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
            {
                continue;
            }
            if unreachable_match_arm(program, operator_use) {
                continue;
            }
            let invalid = || {
                Diagnostic::error(
                    "selected operator crash invocation requires exact selected meaning and ordered operand capture",
                )
            };
            let CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            } = operator_use.origin
            else {
                diagnostics.push(invalid());
                continue;
            };
            let Some(operator) =
                typed_trees::operator::declaration_by_symbol(program, selected.operator_symbol)
            else {
                diagnostics.push(invalid());
                continue;
            };
            let Some(operands) = operator_use.operands(program) else {
                diagnostics.push(invalid());
                continue;
            };
            let parameters = program.operator_parameters(operator);
            let mut captures = flow
                .control
                .operator_invocations
                .iter()
                .filter(|(_, invocation)| invocation.operator_use == operator_use_handle);
            let (Some((invocation_handle, invocation)), None) = (captures.next(), captures.next())
            else {
                diagnostics.push(invalid());
                continue;
            };
            if operator_use.status != CheckedOperatorResolutionStatus::Resolved
                || selected.operator_symbol != operator_use.selected_operator_symbol
                || parameters.len() != operands.len()
                || !flow
                    .control
                    .operands
                    .span_or_empty(invocation.operands)
                    .iter()
                    .map(|operand| operand.expression)
                    .eq(operands.iter().copied())
            {
                diagnostics.push(invalid());
                continue;
            }
            let content_conservation = content_conservation
                .get_or_insert_with(|| validation::content_conservation_plans(program));
            let routes = retained_operator_crash_routes(
                program,
                operators,
                flow,
                semantic,
                operator_use_handle,
                arena::Handle::invalid(),
                program
                    .signature_contracts
                    .span_or_empty(selected.contracts),
                operator,
                parameters,
                &operands,
                machine_symbol,
                state_symbol,
                statement_index,
                content_conservation,
            );
            let (published, surviving) = match routes {
                Ok(routes) => routes,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            sites.push((
                machine_symbol,
                CheckedCrashOperatorSite {
                    operator_use: operator_use_handle,
                    invocation: invocation_handle,
                    named_use: arena::Handle::invalid(),
                    selected_operator: selected.operator_symbol,
                    published,
                    surviving,
                },
            ));
        }
        for (named_use_handle, named_use) in operators.named_uses.iter() {
            let Some(operator) = typed_trees::operator::declaration_by_symbol(
                program,
                named_use.selected_operator_symbol,
            ) else {
                continue;
            };
            let contracts = program
                .signature_contracts
                .span_or_empty(operator.contracts);
            if !contracts
                .iter()
                .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
            {
                continue;
            }
            let invalid = || {
                Diagnostic::error(
                    "selected operator crash invocation requires exact selected meaning and ordered operand capture",
                )
            };
            let CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            } = named_use.origin
            else {
                diagnostics.push(invalid());
                continue;
            };
            let Some(call) = program.expression_table.call_node(named_use.expression) else {
                diagnostics.push(invalid());
                continue;
            };
            let parameters = program.operator_parameters(operator);
            let Some(operands) = named_call_operands(program, call, parameters) else {
                diagnostics.push(invalid());
                continue;
            };
            let content_conservation = content_conservation
                .get_or_insert_with(|| validation::content_conservation_plans(program));
            let routes = retained_operator_crash_routes(
                program,
                operators,
                flow,
                semantic,
                arena::Handle::invalid(),
                named_use_handle,
                contracts,
                operator,
                parameters,
                &operands,
                machine_symbol,
                state_symbol,
                statement_index,
                content_conservation,
            );
            let (published, surviving) = match routes {
                Ok(routes) => routes,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            sites.push((
                machine_symbol,
                CheckedCrashOperatorSite {
                    operator_use: arena::Handle::invalid(),
                    invocation: arena::Handle::invalid(),
                    named_use: named_use_handle,
                    selected_operator: named_use.selected_operator_symbol,
                    published,
                    surviving,
                },
            ));
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in
                    crate::flow::state_statements(program, state.statement_nodes).iter()
                {
                    let Some(call) = statement.as_call() else {
                        continue;
                    };
                    let Some(operator) =
                        crate::flow::resolved_operator_statement_symbol(program, call).and_then(
                            |symbol| typed_trees::operator::declaration_by_symbol(program, symbol),
                        )
                    else {
                        continue;
                    };
                    if !program
                        .signature_contracts
                        .span_or_empty(operator.contracts)
                        .iter()
                        .any(|contract| {
                            matches!(contract.kind, SignatureContractKind::Crashes { .. })
                        })
                    {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "statement call to named operator `{}` carries crash routes no checked site can cover",
                        program
                            .operator_path_members(operator.name)
                            .joined_path(),
                    )));
                }
            }
        }
        if diagnostics.is_empty() {
            Ok(sites)
        } else {
            Err(diagnostics)
        }
    }

    /*
        Published and surviving crash-route buckets for one selected use. A spelled
        use discharges each guard the captured invocation contexts prove false. A
        named call passes an invalid `operator_use` and a valid `named_use`: its
        own operand-time capture row serves the same `InvocationContexts`
        discharge, falling back to the containing statement's entry contexts —
        gated on every leaf occurrence resolving to an entry-proven operand — only
        for uses whose evaluation position emitted no capture (see
        `named_routes`). Anything unproven stays conservative.
    */
    #[allow(clippy::too_many_arguments)]
    fn retained_operator_crash_routes(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        flow: &FlowFacts,
        semantic: &FactPlan,
        operator_use: arena::Handle<checked_trees::CheckedOperatorUseFact>,
        named_use: arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
        contracts: &[typed_trees::signature::SignatureContract],
        operator: &typed_trees::operator::OperatorDefinition,
        parameters: &[typed_trees::signature::StateParameter],
        operands: &[ExpressionHandle],
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> Result<(Vec<CrashRouteBucket>, Vec<CrashRouteBucket>), Diagnostic> {
        let published = super::derive_authored_operator_crash_buckets(
            program,
            operator,
            operators,
            content_conservation,
        )?;
        let parameter_names = declared_parameter_names(parameters);
        let mut resolve_entry = |ordinal: u32, members: &[String]| {
            let operand = operands.get(ordinal as usize).copied()?;
            let parameter = parameters.get(ordinal as usize)?;
            let projection = formal_member_projection(program, parameter.type_reference, members);
            entry_operand_projected(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                operand,
                &projection,
            )
        };
        let mut surviving = Vec::new();
        for bucket in &published {
            let mut guards = Vec::new();
            for guard in bucket.alternative_guards() {
                let CrashRouteGuard::Predicate(predicate) = guard else {
                    guards.push(CrashRouteGuard::Truth);
                    continue;
                };
                let expression = contracts
                    .iter()
                    .filter(|contract| {
                        matches!(contract.kind, SignatureContractKind::Crashes { .. })
                    })
                    .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                    .find_map(|fact| {
                        let expression = fact.as_expression()?;
                        let candidate = super::crash_calls::crash_predicate_from_expression(
                            program,
                            *expression,
                            &parameter_names,
                            Some(content_conservation),
                        );
                        (CrashPredicateIdentity::from_expression(candidate) == *predicate)
                            .then_some(*expression)
                    });
                if expression.is_some_and(|expression| {
                    if operator_use.is_valid() {
                        crate::checks::operator_route_is_false(
                            program,
                            facts::ProgramPoint::Statement {
                                machine_symbol,
                                state_symbol,
                                statement_index,
                            },
                            flow,
                            semantic,
                            operator_use,
                            parameters,
                            operands,
                            expression,
                        )
                    } else {
                        named_routes::named_route_is_false(
                            program,
                            flow,
                            semantic,
                            named_use,
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            parameters,
                            operands,
                            expression,
                        )
                    }
                }) {
                    continue;
                }
                let Some(identity) = predicate
                    .expression()
                    .and_then(|identity| substitute_entry_projected(identity, &mut resolve_entry))
                else {
                    guards.push(CrashRouteGuard::Truth);
                    continue;
                };
                match identity.boolean_value() {
                    Some(false) => {}
                    Some(true) => guards.push(CrashRouteGuard::Truth),
                    None => guards.push(CrashRouteGuard::Predicate(
                        CrashPredicateIdentity::from_expression(identity),
                    )),
                }
            }
            if let Some(bucket) = CrashRouteBucket::new(bucket.cause(), guards) {
                surviving.push(bucket);
            }
        }
        Ok((published, surviving))
    }

    /*
        A named call's exact operands in operator-parameter order. Named resolution
        already fixed the arity: one parameter more than argument binds the value
        receiver to that single leading parameter, and equal counts bind the
        arguments directly — a static namespace receiver carries no operand at all.
        The flow pass reuses this ordering when it captures operand-time evidence
        for a named use, so captured operand rows align with this list.
    */
    pub(crate) fn named_call_operands(
        program: &dyn ::validation::CheckingInput,
        call: &typed_trees::expression::TableCallExpression,
        parameters: &[typed_trees::signature::StateParameter],
    ) -> Option<Vec<ExpressionHandle>> {
        let arguments = program.expression_table.expression_handles(call.arguments);
        if parameters.len() == arguments.len() + 1 {
            return call.receiver.is_valid().then(|| {
                std::iter::once(call.receiver)
                    .chain(arguments.iter().copied())
                    .collect()
            });
        }
        (parameters.len() == arguments.len()).then(|| arguments.to_vec())
    }

    /*
        Operand labels for leaf instantiation at a named call. A reference formal
        used as a predicate value reads its referent — the borrow at the call site
        supplies access, not an extra operator in the predicate — so a `&x`
        operand names `x`, exactly as call-`requires` instantiation renders it
        (checks/contracts/labels/calls.rs). Other operands keep their proof
        spelling.
    */
    pub(crate) fn named_call_operand_labels(
        program: &dyn ::validation::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        operands: &[ExpressionHandle],
    ) -> Vec<String> {
        operands
            .iter()
            .enumerate()
            .map(|(ordinal, operand)| {
                let operand = match (
                    parameters.get(ordinal),
                    program.expression_table.expression(*operand),
                ) {
                    (Some(parameter), ExpressionNode::Borrow(borrow))
                        if matches!(
                            program
                                .type_reference_table
                                .type_reference(parameter.type_reference),
                            TypeReferenceNode::Reference { .. }
                        ) =>
                    {
                        borrow.target
                    }
                    _ => *operand,
                };
                program.render_proof_expression(
                    operand,
                    typed_trees::proposition::ProofSubstitutions::None,
                )
            })
            .collect()
    }

    fn unreachable_match_arm(
        program: &dyn ::validation::CheckingInput,
        operator_use: &checked_trees::CheckedOperatorUseFact,
    ) -> bool {
        let CheckedOperatorOccurrence::MatchEquality { source_arm } = operator_use.occurrence
        else {
            return false;
        };
        let Some(dispatch) = program.expression_table.match_node(operator_use.expression) else {
            return false;
        };
        for ordinal in 0..program.expression_table.match_arms(dispatch.arms).len() {
            let Ok(arm_ordinal) = u32::try_from(ordinal) else {
                return false;
            };
            if dispatch.arms.handle_at(arm_ordinal) == source_arm {
                return expressions(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .take(ordinal)
                    .any(|arm| matches!(arm.pattern, MatchPattern::Wildcard));
            }
        }
        false
    }
}
pub(crate) mod provider_closed_requirements {
    /*
        Publish admitted provider applications after source checking completes.

        Typed association formation is shared with immutable body-origin readers;
        it supplies no checked authority until the pipeline reaches this stage.
    */
    use checked_trees::{CheckedProviderClosedRequirement, CheckedProviderClosedRequirements};

    pub(crate) fn record_provider_closed_requirements(
        program: &dyn validation::CheckingInput,
    ) -> CheckedProviderClosedRequirements {
        let mut recorded = CheckedProviderClosedRequirements::default();
        for association in validation::provider_application_associations(program).iter() {
            let row = &association.identity;
            recorded.insert(
                CheckedProviderClosedRequirement {
                    provider: row.provider,
                    selecting_declaration: row.selecting_declaration,
                    template: row.template,
                    closed_requirement: row.closed_requirement,
                    declaration_application: row.declaration_application,
                    receiving_application: row.receiving_application,
                    receiving_regions: arena::HandleSpan::empty(),
                },
                association.receiving_regions.iter().copied(),
            );
        }
        recorded
    }
}
pub(crate) mod provider_instance_publication {
    /*
        Freeze provider obligations and portable receiving coordinates at checking.

        A later executable closure may retain only some bodies. Its candidate list
        cannot establish which closed instances the selected declaration still owes.
        Unpublishable descriptions retain their reason until that interface is used.
    */
    use checked_trees::{
        CheckedProviderClosedRequirement, CheckedProviderInstanceDeclaration,
        CheckedProviderInstancePublication, CheckedProviderInstanceRequirement,
        CheckedProviderReceivingApplication, CheckedProviderReceivingDescription, CheckedTrees,
    };
    use symbols::SymbolHandle;
    use typed_trees::type_identity::CallableIdentityId;

    mod candidates {
        /*
            Record exact provider edges before executable bodies are projected away.

            A member's independent satisfaction belongs to that machine, while Build
            selects its recorded named implementation. Keep both identities and the
            exact receiving application here. Lowering consumes this association; it
            cannot reconstruct ownership from an attached data name or a body signature.
        */
        use checked_trees::{
            CheckedProviderCandidate, CheckedProviderCandidateDeclaration,
            CheckedProviderInstancePublication, CheckedProviderReceivingApplication, CheckedTrees,
        };
        use symbols::SymbolHandle;
        use typed_trees::{
            TypedTrees,
            machine::{Machine, SatisfiedDeclaration, resolve_satisfied_declaration},
            trait_definition::{Conformance, TraitDefinition},
        };

        #[derive(Clone, Copy)]
        enum Requirement<'a> {
            Trait {
                definition: &'a TraitDefinition,
                signature: &'a typed_trees::signature::StateSignature,
            },
            TopLevel(&'a Machine),
            Operator(&'a typed_trees::operator::OperatorDefinition),
        }
        impl Requirement<'_> {
            fn symbol(self) -> SymbolHandle {
                match self {
                    Self::Trait { signature, .. } => signature.symbol,
                    Self::TopLevel(machine) => machine.symbol,
                    Self::Operator(operator) => operator.symbol,
                }
            }
        }

        pub(super) fn retain(
            checked: &CheckedTrees,
            publication: &mut CheckedProviderInstancePublication,
        ) {
            let typed = &checked.typed;
            let mut requirements = Vec::new();
            for definition in typed
                .traits()
                .iter()
                .filter(|definition| definition.is_boundary)
            {
                requirements.extend(typed.executable_trait_machine_instances(definition).map(
                    |signature| Requirement::Trait {
                        definition,
                        signature,
                    },
                ));
            }
            requirements.extend(
                typed
                    .machines()
                    .iter()
                    .filter(|machine| {
                        machine.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                    })
                    .map(Requirement::TopLevel),
            );
            requirements.extend(
                typed
                    .all_operators()
                    .filter(|operator| operator.is_boundary)
                    .map(Requirement::Operator),
            );
            for machine in typed.machines().iter().filter(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            }) {
                for requirement in &requirements {
                    match candidates(checked, publication, machine, *requirement) {
                        Ok(declarations) => {
                            for declaration in declarations {
                                publication.candidates.insert(CheckedProviderCandidate {
                                    boundary: requirement.symbol(),
                                    candidate: machine.symbol,
                                    declaration: Ok(declaration),
                                });
                            }
                        }
                        Err(reason) => {
                            publication.candidates.insert(CheckedProviderCandidate {
                                boundary: requirement.symbol(),
                                candidate: machine.symbol,
                                declaration: Err(reason),
                            });
                        }
                    }
                }
            }
        }

        fn candidates(
            checked: &CheckedTrees,
            publication: &CheckedProviderInstancePublication,
            machine: &Machine,
            requirement: Requirement<'_>,
        ) -> Result<Vec<CheckedProviderCandidateDeclaration>, String> {
            let typed = &checked.typed;
            if let Requirement::Trait {
                definition,
                signature,
            } = requirement
            {
                let mut declarations = Vec::new();
                if let Some(application) = typed.closed_requirement_application(signature.symbol) {
                    for association in
                        checked
                            .facts
                            .provider_closed_requirements
                            .iter()
                            .filter(|row| {
                                row.provider == machine.symbol
                                    && row.template == application.template
                                    && row.closed_requirement == signature.symbol
                                    && row.receiving_application
                                        == application.trait_application_identity
                            })
                    {
                        let provider = provider_declaration(typed, association)?;
                        if !provider.is_valid() {
                            continue;
                        }
                        declarations.push(CheckedProviderCandidateDeclaration {
                            provider,
                            receiving: receiving_handle(publication, association)?,
                        });
                    }
                } else {
                    for edge in
                        exact_requirement_edges(typed, machine, definition, signature.symbol)?
                    {
                        if edge.lifetime_parameters > 0 || !edge.reach.reaches(typed, machine)? {
                            continue;
                        }
                        declarations.push(CheckedProviderCandidateDeclaration {
                            provider: edge.provider,
                            receiving: arena::Handle::default(),
                        });
                    }
                }
                return Ok(declarations);
            }
            let supplies = typed_trees::provider_declarations::independent_satisfactions(typed, machine)
                .any(|conformance| {
                    if conformance.external_binding.is_some() {
                        return false;
                    }
                    match requirement {
                        Requirement::TopLevel(declaration) => {
                            (conformance.symbol == declaration.symbol
                                && conformance.requirement_symbol == declaration.symbol)
                                || checked
                                    .facts
                                    .provider_closed_requirements
                                    .iter()
                                    .any(|row| {
                                        row.provider == machine.symbol
                                            && row.selecting_declaration == machine.symbol
                                            && row.template == conformance.symbol
                                            && row.closed_requirement == declaration.symbol
                                    })
                        }
                        Requirement::Operator(operator) => conformance.requirement_symbol
                            == operator.symbol
                            && typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                                typed,
                                machine,
                                conformance,
                            )
                            .is_some_and(|resolved| resolved.symbol == operator.symbol),
                        Requirement::Trait { .. } => unreachable!(),
                    }
                });
            if !supplies {
                return Ok(Vec::new());
            }
            let provider = machine_provider_declaration(typed, machine)?;
            if !provider.is_valid() {
                return Ok(Vec::new());
            }
            Ok(vec![CheckedProviderCandidateDeclaration {
                provider,
                receiving: arena::Handle::default(),
            }])
        }

        fn receiving_handle(
            publication: &CheckedProviderInstancePublication,
            association: &checked_trees::CheckedProviderClosedRequirement,
        ) -> Result<arena::Handle<CheckedProviderReceivingApplication>, String> {
            let mut rows = publication
                .receivers
                .iter()
                .filter(|(_, row)| row.association == *association);
            let (handle, _) = rows
                .next()
                .ok_or("provider lost its checked receiving publication")?;
            if rows.next().is_some() {
                return Err("provider has multiple checked receiving publications".into());
            }
            Ok(handle)
        }

        pub(super) fn provider_declaration(
            typed: &TypedTrees,
            association: &checked_trees::CheckedProviderClosedRequirement,
        ) -> Result<SymbolHandle, String> {
            if typed
                .conformance_by_symbol(association.selecting_declaration)
                .is_some()
            {
                return Ok(association.selecting_declaration);
            }
            let machine = typed
                .machine(association.selecting_declaration)
                .ok_or("provider association lost its selecting declaration")?;
            machine_provider_declaration(typed, machine)
        }

        /*
            An exact proof edge on a free helper creates no selectable implementation.
            An authored map member or legacy attached supplier does promise a provider
            owner; losing that recorded relation must refuse rather than become a skip.
        */
        fn machine_provider_declaration(
            typed: &TypedTrees,
            machine: &Machine,
        ) -> Result<SymbolHandle, String> {
            let provider = typed_trees::provider_declarations::satisfaction_provider_declaration(
                typed, machine,
            )
            .unwrap_or_default();
            let owner = typed.symbols.get(machine.symbol).declaration_owner;
            let owns_selection = (owner.is_valid()
                && typed.symbols.get(owner).kind == symbols::SymbolKind::Conformance)
                || machine.attached_data.is_some();
            if !provider.is_valid() && owns_selection {
                return Err("exact satisfaction lost its recorded provider declaration".into());
            }
            Ok(provider)
        }

        /* One exact requirement edge a candidate body may realize. */
        struct RequirementEdge {
            /* The declaration `Build::select_provider` names. */
            provider: symbols::SymbolHandle,
            /* How the edge reaches a body when the candidate is not `realization`. */
            reach: EdgeReach,
            /* The carrier declaration's own lifetime telescope. */
            lifetime_parameters: usize,
        }

        /* How an exact requirement edge reaches the body that realizes it. */
        enum EdgeReach {
            /*
                The edge names this body: a machine's own clause, or a conformance row
                whose realization is the candidate.
            */
            Named,
            /*
                A conformance row names a portable family head; one of its members
                realizes the row on the selected target.
            */
            FamilyHead(symbols::SymbolHandle),
            /*
                A conformance row names a template a checked specialization
                instantiates.
            */
            Specialization(symbols::SymbolHandle),
        }

        impl EdgeReach {
            /* Whether `machine` is the body this edge reaches. */
            fn reaches(&self, typed: &TypedTrees, machine: &Machine) -> Result<bool, String> {
                match self {
                    Self::Named => Ok(true),
                    Self::FamilyHead(head) => {
                        Ok(typed.target_machine_families.iter().any(|family| {
                            family.canonical == *head
                                && family
                                    .members
                                    .iter()
                                    .any(|member| member.machine == machine.symbol)
                        }))
                    }
                    Self::Specialization(template) => {
                        let source = typed
                            .machine(*template)
                            .and_then(|machine| typed.machine_states(machine).first());
                        let target = typed.machine_states(machine).first();
                        let (Some(source), Some(target)) = (source, target) else {
                            return Err("named provider lost its instantiated entry".into());
                        };
                        Ok(typed.instantiated_machine_state_matches(
                            machine.symbol,
                            *template,
                            source.symbol,
                            target.symbol,
                        ))
                    }
                }
            }
        }

        /*
            The declarations that can own an exact requirement edge for this candidate,
            in one walk: every whole conformance map, then the candidate's own
            declaration.
        */
        enum EdgeCarrier<'a> {
            /* A conformance map owns one row per exact requirement edge. */
            ConformanceMap(&'a Conformance),
            /* A machine owns its own exact clause, realized by its own body. */
            Machine {
                declaration: &'a Machine,
                lifetime_parameters: usize,
            },
        }

        fn edge_carriers<'a>(typed: &'a TypedTrees, machine: &'a Machine) -> Vec<EdgeCarrier<'a>> {
            let mut carriers = typed
                .conformances()
                .iter()
                .map(EdgeCarrier::ConformanceMap)
                .collect::<Vec<_>>();
            let (type_parameters, lifetime_parameters) =
                typed.machine_declaration_scope(machine, |left, right| left.symbol == right.symbol);
            if type_parameters.is_empty()
                && !typed
                    .machine_specializations
                    .iter()
                    .any(|receipt| receipt.instance == machine.symbol)
            {
                carriers.push(EdgeCarrier::Machine {
                    declaration: machine,
                    lifetime_parameters: lifetime_parameters.len(),
                });
            }
            carriers
        }

        /* Every exact requirement edge for `template` that `machine` realizes. */
        fn exact_requirement_edges(
            typed: &TypedTrees,
            machine: &Machine,
            definition: &TraitDefinition,
            template: symbols::SymbolHandle,
        ) -> Result<Vec<RequirementEdge>, String> {
            let mut edges = Vec::new();
            for carrier in edge_carriers(typed, machine) {
                match carrier {
                    EdgeCarrier::ConformanceMap(implementation) => {
                        let mut matching = implementation.rows.iter().filter(|row| {
                            row.declaring_trait == definition.symbol && row.requirement == template
                        });
                        let Some(row) = matching.next() else {
                            continue;
                        };
                        if matching.next().is_some() {
                            return Err(
                                "named provider repeats an exact boundary requirement row".into()
                            );
                        }
                        let Some(realization) = typed.machine(row.realization_machine) else {
                            return Err(
                                "named provider row has no exact realization machine".into()
                            );
                        };
                        if !typed
                            .machine_states(realization)
                            .first()
                            .is_some_and(|entry| entry.symbol == row.realization_state)
                        {
                            return Err("named provider row has no exact realization entry".into());
                        }
                        edges.push(RequirementEdge {
                            provider: implementation.symbol,
                            reach: if realization.target_family_interface {
                                EdgeReach::FamilyHead(realization.symbol)
                            } else if realization.symbol == machine.symbol {
                                EdgeReach::Named
                            } else {
                                EdgeReach::Specialization(realization.symbol)
                            },
                            lifetime_parameters: implementation.lifetime_parameters.len(),
                        });
                    }
                    EdgeCarrier::Machine {
                        declaration: owner,
                        lifetime_parameters,
                    } => {
                        if !declares_exact_requirement(typed, owner, definition, template) {
                            continue;
                        }
                        let provider = machine_provider_declaration(typed, owner)?;
                        if !provider.is_valid() {
                            continue;
                        }
                        edges.push(RequirementEdge {
                            provider,
                            reach: EdgeReach::Named,
                            lifetime_parameters,
                        });
                    }
                }
            }
            Ok(edges)
        }

        /*
            Whether this machine itself declares `satisfies Trait<::requirement` for the
            requirement template. A `via` binding is an external leaf rather than a
            checked adapter body, and resolves through its own supply path.
        */
        fn declares_exact_requirement(
            typed: &TypedTrees,
            machine: &Machine,
            definition: &TraitDefinition,
            template: symbols::SymbolHandle,
        ) -> bool {
            typed_trees::provider_declarations::independent_satisfactions(typed, machine).any(
                |conformance| {
                    conformance.external_binding.is_none()
                        && matches!(
                            resolve_satisfied_declaration(typed, machine, conformance),
                            Some(SatisfiedDeclaration::Trait {
                                definition: satisfied,
                                requirement,
                            }) if satisfied.symbol == definition.symbol && requirement.symbol == template
                        )
                },
            )
        }
    }

    pub(crate) fn retain(checked: &mut CheckedTrees) {
        let typed = &checked.typed;
        let mut publication = CheckedProviderInstancePublication::default();
        for association in checked.facts.provider_closed_requirements.iter() {
            let description = receiving(checked, &mut publication, association);
            let receiving = publication
                .receivers
                .insert(CheckedProviderReceivingApplication {
                    association: *association,
                    description,
                });
            if typed
                .trait_machine_signature(association.template)
                .is_none()
                || typed
                    .closed_requirement_application(association.closed_requirement)
                    .is_none()
            {
                continue;
            }
            let selecting_provider = match candidates::provider_declaration(typed, association) {
                Ok(provider) if provider.is_valid() => Ok(provider),
                Ok(_) => continue,
                Err(reason) => Err(reason),
            };
            publication
                .requirements
                .insert(CheckedProviderInstanceRequirement {
                    template: association.template,
                    boundary: association.closed_requirement,
                    provider: association.provider,
                    declaration: selecting_provider
                        .and_then(|provider| named(checked, association, receiving, provider)),
                });
        }
        for template in typed.machines().iter().filter(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && !typed.machine_type_parameters(machine).is_empty()
        }) {
            for instance in typed
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.template == template.symbol)
                .filter_map(|receipt| typed.machine(receipt.instance))
                .filter(|instance| {
                    instance.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
                })
            {
                let requirement = typed.machine_callable_identity(instance);
                if !requirement.is_valid() {
                    continue;
                }
                let template_identity = typed.machine_callable_identity(template);
                let declaration = if template_identity.is_valid() {
                    Ok(CheckedProviderInstanceDeclaration {
                        template_identity,
                        package: typed.symbols.symbol_package_identity(template.symbol),
                        selecting_declaration: SymbolHandle::default(),
                        source_candidate_identity: CallableIdentityId::default(),
                        requirement_identity: requirement,
                        candidate_identity: CallableIdentityId::default(),
                        receiving: arena::Handle::default(),
                    })
                } else {
                    Err("generic provider requirement has no template callable identity".to_owned())
                };
                publication
                    .requirements
                    .insert(CheckedProviderInstanceRequirement {
                        template: template.symbol,
                        boundary: instance.symbol,
                        provider: SymbolHandle::default(),
                        declaration,
                    });
            }
        }
        candidates::retain(checked, &mut publication);
        checked.facts.provider_instance_publication = publication;
    }

    fn named(
        checked: &CheckedTrees,
        association: &CheckedProviderClosedRequirement,
        receiving: arena::Handle<CheckedProviderReceivingApplication>,
        selecting_provider: SymbolHandle,
    ) -> Result<CheckedProviderInstanceDeclaration, String> {
        let typed = &checked.typed;
        let (owner, signature) = typed
            .trait_machine_signature(association.template)
            .ok_or("provider obligation lost its original signature")?;
        let application = typed
            .closed_requirement_application(association.closed_requirement)
            .ok_or("provider obligation lost its closed requirement application")?;
        let declaring = typed
            .trait_definition_by_symbol(application.declaring_trait)
            .ok_or("provider obligation lost its declaring trait")?;
        let requirement =
            typed.trait_requirement_callable_identity(declaring, application.signature);
        if !requirement.is_valid() {
            return Err("provider obligation has no closed requirement identity".into());
        }
        let provider = typed
            .machine(association.provider)
            .ok_or("provider obligation lost its checked realization")?;
        let mut receipts = typed
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.instance == association.provider);
        let template = receipts
            .next()
            .map_or(association.provider, |receipt| receipt.template);
        if receipts.next().is_some() {
            return Err("provider callable has more than one specialization receipt".into());
        }
        let template = typed
            .machine(template)
            .ok_or("provider obligation lost its immediate template")?;
        let source_candidate_identity = typed.machine_callable_identity(template);
        if !source_candidate_identity.is_valid() {
            return Err("provider obligation has no realization template identity".into());
        }
        let candidate_identity = typed.machine_callable_identity(provider);
        if !candidate_identity.is_valid() {
            return Err("provider obligation has no closed realization identity".into());
        }
        Ok(CheckedProviderInstanceDeclaration {
            template_identity: typed.trait_requirement_callable_identity(owner, signature),
            package: typed.symbols.symbol_package_identity(association.template),
            selecting_declaration: selecting_provider,
            source_candidate_identity,
            requirement_identity: requirement,
            candidate_identity,
            receiving,
        })
    }

    fn receiving(
        checked: &CheckedTrees,
        publication: &mut CheckedProviderInstancePublication,
        association: &CheckedProviderClosedRequirement,
    ) -> Result<CheckedProviderReceivingDescription, String> {
        let typed = &checked.typed;
        let Some(application) =
            typed.closed_requirement_application(association.closed_requirement)
        else {
            return Ok(CheckedProviderReceivingDescription {
                declaration_application: String::new(),
                application: String::new(),
                regions: arena::HandleSpan::empty(),
            });
        };
        if association.template != application.template
            || association.receiving_application != application.trait_application_identity
        {
            return Err("checked provider obligation has a different receiving application".into());
        }
        let regions = checked
            .facts
            .provider_closed_requirements
            .receiving_regions(association)
            .ok_or("provider candidate lost its checked receiving region span")?;
        let expected_regions = typed
            .conformance_by_symbol(association.selecting_declaration)
            .map_or(0, |implementation| implementation.lifetime_parameters.len());
        if regions.len() != expected_regions {
            return Err(
                "provider candidate has an incomplete checked receiving region tuple".into(),
            );
        }
        let application = typed
            .type_identity_text(application.portable_trait_application_identity)
            .filter(|text| !text.is_empty())
            .ok_or("provider receiving application has no portable identity")?;
        let regions = regions
            .iter()
            .map(|region| {
                typed_trees::type_identity::portable_lifetime_parameter_identity(typed, *region)
                    .ok_or_else(|| {
                        "provider receiving region has no portable declaration coordinate"
                            .to_owned()
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let declaration_application = typed
            .type_identity_text(association.declaration_application)
            .filter(|text| !text.is_empty())
            .ok_or("provider declaration application has no portable identity")?;
        Ok(CheckedProviderReceivingDescription {
            declaration_application,
            application,
            regions: publication.regions.insert_many(regions),
        })
    }
}
pub(crate) mod qualification_evidence {
    /*
        Qualification originates at an authorized result boundary, not from a type
        annotation. Exact-machine routes name authored declarations; executable
        specialization and target membership change the body symbol but not the
        invoked declaration. Follow retained application ownership and the resolved
        family roster for that join. Domain indices and
        result subjects remain in the ordinary contract facts and are not erased.
    */
    use checked_trees::ContractProofFact;
    use facts::{BoundaryQualificationIssuerRecord, FactPayload, FactPlan, QualificationEvidence};
    use language_semantics::{
        DomainEstablishmentRoute as EstablishmentRoute, MachineSupplyMode,
        QualificationEvidenceOrigin as EvidenceOrigin,
    };
    use symbols::SymbolHandle;
    use typed_trees::machine::Machine;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn domain_definition(
        program: &dyn ::validation::CheckingInput,
        domain_symbol: SymbolHandle,
    ) -> Option<&typed_trees::domain::DomainDefinition> {
        program.domain_definition(domain_symbol)
    }

    /*
        Evidence for a row the state's own declaration already establishes: the
        caller inherited the claim, it did not validate it here.
    */
    pub(crate) fn propagated_evidence(state_symbol: SymbolHandle) -> QualificationEvidence {
        QualificationEvidence::from_origin(EvidenceOrigin::Propagated, state_symbol)
    }

    /*
        Evidence for a row checked validation itself established at this point, not
        one the caller inherited.
    */
    pub(crate) fn checked_validation_evidence(state_symbol: SymbolHandle) -> QualificationEvidence {
        QualificationEvidence::from_origin(EvidenceOrigin::CheckedValidation, state_symbol)
    }

    /*
        Project authorized contract issuers once, before flow forks the fact plan.
        The shared baseline gives repeated calls and independent workers the same
        evidence identity. Unused declarations are inventory, not grant obligations:
        consumers follow issuer handles from actual admitted facts only.
    */
    pub(crate) fn prepare_issuers(
        program: &dyn ::validation::CheckingInput,
        proof: &checked_trees::ProofFacts,
        facts: &mut FactPlan,
    ) -> Result<(), diagnostics::Diagnostic> {
        for (_, contract) in proof.contract_facts.iter() {
            let Some(authorization) = contract.qualification_authorization else {
                continue;
            };
            if issuer_handle(facts, authorization).is_valid() {
                continue;
            }
            let owner = program
                .trait_definition_by_symbol(authorization.requirement_symbol)
                .filter(|owner| owner.is_boundary)
                .ok_or_else(|| {
                    diagnostics::Diagnostic::error(format!(
                        "qualification issuer {:?} has no exact boundary declaration",
                        authorization.requirement_symbol,
                    ))
                })?;
            let requirement = program
                .trait_machine_signature_by_symbol(owner, authorization.signature_symbol)
                .ok_or_else(|| {
                    diagnostics::Diagnostic::error(format!(
                        "qualification issuer {:?} does not own exact requirement {:?}",
                        authorization.requirement_symbol, authorization.signature_symbol,
                    ))
                })?;
            let is_product_owner = program
                .symbols
                .symbol_provenance_source_span(owner.symbol)
                .and_then(|span| program.symbols.source_file(span))
                .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product);
            facts
                .qualification_issuers
                .append(BoundaryQualificationIssuerRecord {
                    source_symbol: authorization.requirement_symbol,
                    requirement_symbol: authorization.signature_symbol,
                    issuer: language_semantics::BoundaryQualificationIssuer {
                        requirement_identity: program
                            .trait_requirement_callable_identity_text(owner, requirement),
                        boundary_path: program.trait_declaration_path(owner),
                        boundary_package: program.symbols.symbol_package_identity(owner.symbol),
                        is_product_owner,
                    },
                });
        }
        Ok(())
    }

    fn issuer_handle(
        facts: &FactPlan,
        authorization: checked_trees::BoundaryQualificationAuthorization,
    ) -> arena::Handle<BoundaryQualificationIssuerRecord> {
        facts
            .qualification_issuers
            .iter()
            .find_map(|(handle, record)| {
                (record.source_symbol == authorization.requirement_symbol
                    && record.requirement_symbol == authorization.signature_symbol)
                    .then_some(handle)
            })
            .unwrap_or_default()
    }

    fn admitted_evidence(
        facts: &FactPlan,
        authorization: checked_trees::BoundaryQualificationAuthorization,
    ) -> Option<QualificationEvidence> {
        let issuer = issuer_handle(facts, authorization);
        issuer.is_valid().then(|| {
            QualificationEvidence::from_admitted_requirement(
                authorization.requirement_symbol,
                authorization.signature_symbol,
                issuer,
            )
        })
    }

    pub(crate) fn call_contract_evidence(
        program: &dyn ::validation::CheckingInput,
        facts: &FactPlan,
        target_symbol: SymbolHandle,
        target_state_symbol: SymbolHandle,
        contract: &ContractProofFact,
        payload: FactPayload,
        is_ensures: bool,
    ) -> Option<QualificationEvidence> {
        if !is_ensures {
            return Some(QualificationEvidence::default());
        }
        if matches!(payload, FactPayload::ContractCarryPermission { .. }) {
            return call_carry_permission_evidence(
                program,
                facts,
                target_symbol,
                target_state_symbol,
                contract,
            );
        }
        let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
            return Some(QualificationEvidence::default());
        };
        let Some(domain) = domain_definition(program, domain_symbol) else {
            return Some(QualificationEvidence::default());
        };

        match ::validation::call_qualification_source(
            program,
            target_symbol,
            contract.qualification_authorization,
        )? {
            ::validation::CallQualificationSource::CheckedBody(machine) => {
                let origin = if machine_domain_establishment_route(program, machine, domain_symbol)
                    .is_some()
                {
                    EvidenceOrigin::AuthorizedRouteEstablishment
                } else if domain.predicate_presence.is_present() {
                    EvidenceOrigin::Prover
                } else {
                    EvidenceOrigin::CheckedTransformation
                };
                Some(QualificationEvidence::from_origin(origin, target_symbol))
            }
            ::validation::CallQualificationSource::AdmittedRequirement(authorization) => {
                admitted_evidence(facts, authorization)
            }
            ::validation::CallQualificationSource::Propagated => {
                Some(propagated_evidence(target_symbol))
            }
        }
    }

    pub(crate) fn machine_has_checked_domain_establishment(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        domain_symbol: SymbolHandle,
    ) -> bool {
        machine_domain_establishment_route(program, machine, domain_symbol).is_some()
    }

    pub(crate) fn machine_domain_establishment_route(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        domain_symbol: SymbolHandle,
    ) -> Option<EstablishmentRoute> {
        if !matches!(
            machine.supply_mode,
            MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
        ) {
            return None;
        }
        let domain = domain_definition(program, domain_symbol)?;
        domain
            .establishment_routes
            .iter()
            .find_map(|route| match route {
                EstablishmentRoute::CheckedRequirement {
                    trait_definition,
                    requirement,
                } => {
                    let attached =
                        program
                            .machine_trait_conformances(machine)
                            .iter()
                            .any(|conformance| {
                                conformance.symbol == *trait_definition
                                    && conformance.requirement_symbol == *requirement
                            });
                    let named = program.conformances().iter().any(|conformance| {
                        conformance.rows.iter().any(|row| {
                            row.source.owns_realization()
                                && row.declaring_trait == *trait_definition
                                && row.requirement == *requirement
                                && program
                                    .machine_invokes_declaration(
                                        machine.symbol,
                                        row.realization_machine,
                                    )
                                    .unwrap_or(false)
                        })
                    });
                    (attached || named).then_some(*route)
                }
                EstablishmentRoute::ExactMachine {
                    machine: authorized,
                } => program
                    .machine_invokes_declaration(machine.symbol, *authorized)
                    .unwrap_or(false)
                    .then_some(*route),
                EstablishmentRoute::BoundaryRequirement { .. } => None,
            })
    }

    fn call_carry_permission_evidence(
        program: &dyn ::validation::CheckingInput,
        facts: &FactPlan,
        target_symbol: SymbolHandle,
        _target_state_symbol: SymbolHandle,
        contract: &ContractProofFact,
    ) -> Option<QualificationEvidence> {
        match ::validation::call_qualification_source(
            program,
            target_symbol,
            contract.qualification_authorization,
        )? {
            ::validation::CallQualificationSource::CheckedBody(_) => {
                Some(QualificationEvidence::from_origin(
                    EvidenceOrigin::CheckedTransformation,
                    target_symbol,
                ))
            }
            ::validation::CallQualificationSource::AdmittedRequirement(authorization) => {
                admitted_evidence(facts, authorization)
            }
            ::validation::CallQualificationSource::Propagated => {
                Some(propagated_evidence(target_symbol))
            }
        }
    }

    pub(crate) fn operator_contract_evidence(
        program: &dyn ::validation::CheckingInput,
        operator_symbol: SymbolHandle,
        payload: FactPayload,
    ) -> QualificationEvidence {
        let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
            return QualificationEvidence::default();
        };
        let Some(domain) = domain_definition(program, domain_symbol) else {
            return QualificationEvidence::default();
        };
        if domain.predicate_presence.is_present() {
            return QualificationEvidence::from_origin(EvidenceOrigin::Prover, operator_symbol);
        }

        QualificationEvidence::from_origin(EvidenceOrigin::CheckedTransformation, operator_symbol)
    }
    pub(crate) fn unwrapped_type_references_match(
        program: &dyn ::validation::CheckingInput,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
    ) -> bool {
        let Some(left) = program.unwrapped_type_reference(left) else {
            return false;
        };
        let Some(right) = program.unwrapped_type_reference(right) else {
            return false;
        };
        program.normalized_type_identity(left) == program.normalized_type_identity(right)
    }
}
mod qualification_facts {
    /*
        Qualification facts and service reach facts.
    */
    use crate::flow::indexed_statements;
    use arena::Handle;
    use arena::HandleSpan;
    use checked_trees::CallServiceReachRows;
    use checked_trees::ContentProjectionFacts;
    use checked_trees::MachineQualifications;
    use checked_trees::MachineServiceReachRows;
    use checked_trees::QualificationFacts;
    use checked_trees::ServiceReachFacts;
    use checked_trees::StateServiceReachRows;
    use language_semantics::SemanticDomainId;
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::BuildSymbolHasher;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::TransitionGuardNode;
    use typed_trees::statement::TransitionTargetNode;
    use validation::build_content_projection_plans;
    use validation::content_conservation_plans;

    /*
        Declaration-only content plans are available before the ownership walk.
        A normalized equation is an obligation, not a successful body theorem or
        permission to consume its inputs. Keep the same table through later
        qualification construction so completion-owned evidence retains its indices.
    */
    pub(crate) fn build_content_facts(
        program: &dyn ::validation::CheckingInput,
    ) -> ContentProjectionFacts {
        ContentProjectionFacts {
            plans: build_content_projection_plans(program),
            conservation_plans: content_conservation_plans(program)
                .iter()
                .map(|source| source.plan.clone())
                .collect(),
            ..Default::default()
        }
    }

    /*
        The cast-collection walk context: the program, the (machine, state) pair,
        the statement occurrence, and the committed-domain / vacuous-use / visited
        accumulators every step threads. Each recursive cast step restated this
        eight-argument tuple at its call site; the walk now carries it once, and a
        step names only the expression it descends into.
    */
    struct CastWalk<'a, 'b> {
        program: &'a dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: u32,
        committed: &'b mut Vec<SemanticDomainId>,
        vacuous_uses: &'b mut Vec<checked_trees::VacuousQualificationUse>,
        visited: &'b mut std::collections::HashSet<u32, BuildSymbolHasher>,
    }

    impl CastWalk<'_, '_> {
        fn scan(&mut self, expression: ExpressionHandle) {
            use checked_trees::VacuousQualificationUse;
            use language_semantics::SemanticDomainTable;
            use typed_trees::expression::ExpressionNode;

            let program = self.program;
            let machine = self.machine;
            let state = self.state;
            let statement_index = self.statement_index;
            if !expression.is_valid() || !self.visited.insert(expression.arena_index()) {
                return;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    self.scan(dispatch.subject);
                    for arm in program.expression_table.match_arms(dispatch.arms) {
                        for pattern in program.expression_table.match_pattern_operands(arm.pattern)
                        {
                            self.scan(pattern);
                        }
                        self.scan(arm.value);
                    }
                }
                ExpressionNode::Cast(cast) => {
                    let policy = match cast.domain {
                        ArithmeticDomain::Exact => None,
                        ArithmeticDomain::Wrapping => Some(SemanticDomainTable::WRAPPING),
                        ArithmeticDomain::Saturating => Some(SemanticDomainTable::SATURATING),
                        ArithmeticDomain::Trapping => Some(SemanticDomainTable::TRAPPING),
                    };
                    if let Some(policy) = policy {
                        self.committed.push(policy);
                    }
                    for domain in ::validation::field_domains::declared_domain_constraint_records(
                        program,
                        cast.result_type,
                    ) {
                        if domain.semantic_id.is_valid() {
                            self.committed.push(domain.semantic_id);
                        }
                        if ::validation::field_domains::domain_is_vacuous(
                            program,
                            domain.symbol,
                            &mut Vec::new(),
                        ) {
                            self.vacuous_uses.push(VacuousQualificationUse {
                                machine,
                                state,
                                statement_index,
                                expression,
                                domain: domain.symbol,
                                semantic_domain: domain.semantic_id,
                            });
                        }
                    }
                    self.scan(cast.value);
                }
                ExpressionNode::Binary(binary) => {
                    self.scan(binary.left);
                    self.scan(binary.right);
                }
                ExpressionNode::Unary(unary) => self.scan(unary.operand),
                ExpressionNode::Member(member) => self.scan(member.receiver),
                ExpressionNode::Borrow(inner) => self.scan(inner.target),
                ExpressionNode::Indexed(indexed) => {
                    self.scan(indexed.collection);
                    self.scan(indexed.index);
                }
                ExpressionNode::Range(range) => {
                    self.scan(range.start);
                    self.scan(range.end);
                }
                ExpressionNode::Call(call) => {
                    self.scan(call.receiver);
                    for argument in program.expression_table.expression_handles(call.arguments) {
                        self.scan(*argument);
                    }
                }
                ExpressionNode::StructLiteral(literal) => {
                    for field in program.expression_table.struct_fields(literal.fields) {
                        self.scan(field.value);
                    }
                }
                ExpressionNode::ArrayLiteral(items) => {
                    for item in program.expression_table.expression_handles(*items) {
                        self.scan(*item);
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn build_qualification_facts(
        program: &dyn ::validation::CheckingInput,
        content: ContentProjectionFacts,
    ) -> QualificationFacts {
        type HashSet<T> = std::collections::HashSet<T, BuildSymbolHasher>;

        let mut machines = Vec::new();
        let mut vacuous_uses = Vec::new();
        for machine in program.machines() {
            let mut committed = Vec::new();
            for state in program.machine_states(machine) {
                for (statement_index, statement) in
                    indexed_statements(program, state.statement_nodes)
                {
                    use typed_trees::statement::StatementNode;
                    let statement_index = u32::try_from(statement_index)
                        .expect("qualification statement index overflow");
                    let mut visited = HashSet::default();
                    let mut walk = CastWalk {
                        program,
                        machine: machine.symbol,
                        state: state.symbol,
                        statement_index,
                        committed: &mut committed,
                        vacuous_uses: &mut vacuous_uses,
                        visited: &mut visited,
                    };
                    match statement {
                        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
                        StatementNode::Assignment(assignment) => {
                            walk.scan(assignment.target);
                            walk.scan(assignment.value);
                        }
                        StatementNode::Expression(expression) => {
                            walk.scan(*expression);
                        }
                        StatementNode::LocalData(local) => {
                            walk.scan(local.initial_value);
                        }
                        StatementNode::Call(call) => {
                            for argument in
                                program.statement_table.expression_handles(call.arguments)
                            {
                                walk.scan(*argument);
                            }
                        }
                        StatementNode::Transition(transition) => {
                            if let TransitionGuardNode::When(guard) = &transition.guard {
                                walk.scan(*guard);
                            }
                            for target in transition.targets() {
                                match program.statement_table.transition_target(target) {
                                    TransitionTargetNode::Value(value) => walk.scan(*value),
                                    TransitionTargetNode::Named { arguments, .. } => {
                                        for argument in
                                            program.statement_table.expression_handles(*arguments)
                                        {
                                            walk.scan(*argument);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            committed.sort_by_key(|id| id.0);
            committed.dedup();
            if !committed.is_empty() {
                machines.push(MachineQualifications {
                    machine: machine.symbol,
                    body_committed: committed,
                });
            }
        }
        QualificationFacts {
            scalar_returns: Vec::new(),
            value_formations: Default::default(),
            machines,
            vacuous_uses,
            content,
        }
    }

    /*
        Build the boundary-symbol service fixed point without consulting the
        legacy global effect catalog. A direct boundary-signature call contributes
        its containing service plus the signature's explicitly reached services.
        Checked callees contribute authored services plus transitive call reach;
        requirements, boundaries, and external realizations retain fixed published
        ceilings independently of a selected implementation.
    */
    pub(crate) fn build_service_reach_facts(
        program: &dyn ::validation::CheckingInput,
        inferred: flow_effects::ServiceReachInferencePlan,
    ) -> ServiceReachFacts {
        ServiceReachFacts {
            services: program.service_reaches.clone(),
            rows: inferred.rows,
            dependency_parameters: inferred.dependency_parameters,
            root_machines: remap_service_reach_span(inferred.root_machines),
            machines: inferred.machines.map(|machine| MachineServiceReachRows {
                machine: machine.machine,
                dependency: machine.dependency,
                cleanup_contribution: machine.cleanup_contribution,
                interface: machine.interface,
                published_ceiling: machine.published,
                inferred_direct: machine.inferred_direct,
                inferred_transitive: machine.inferred_transitive,
                concrete_transitive: machine.concrete_transitive,
                effective: machine.effective,
                concrete_effective: machine.concrete_effective,
                unresolved_installation_reaches: machine.unresolved_installation_reaches,
                states: remap_service_reach_span(machine.states),
            }),
            states: inferred.states.map(|state| StateServiceReachRows {
                state: state.state,
                inferred_direct: state.inferred_direct,
                inferred_transitive: state.inferred_transitive,
                concrete_direct: state.concrete_direct,
                concrete_transitive: state.concrete_transitive,
                unresolved_installation_reaches: state.unresolved_installation_reaches,
                calls: remap_service_reach_span(state.calls),
            }),
            calls: inferred.calls.map(|call| CallServiceReachRows {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_state: call.target_state,
                target_machine: call.target_machine,
                inferred_direct: call.inferred_direct,
                inferred_transitive: call.inferred_transitive,
                concrete_direct: call.concrete_direct,
                concrete_transitive: call.concrete_transitive,
                unresolved_installation_reaches: call.unresolved_installation_reaches,
            }),
        }
    }

    fn remap_service_reach_span<From, To>(span: HandleSpan<From>) -> HandleSpan<To> {
        let start = span.start();
        HandleSpan::from_parts(
            Handle::from_parts(start.arena_index(), start.generation()),
            span.count(),
        )
    }
}
mod requirement_call_specializations {
    /*
        Checked requirement-call specialization facts.

        MP2b admits each static machine argument at a generic call edge and, in
        doing so, derives the callee's `Type` substitutions from the selected
        callable's shape. A nominal contract replays that judgment into a
        `CheckedNominalMachineUse` row; a structural contract emits no row at
        all, so the specialization was dropped at validation. These rows carry
        it across the checked boundary so Unit call construction can substitute
        the requirement's formals and bind the selected provider.
    */
    use validation::{ValidatedNominalMachineUseSite, ValidatedRequirementCallSpecialization};

    pub(crate) fn build_requirement_call_specialization_facts(
        specializations: Vec<ValidatedRequirementCallSpecialization>,
    ) -> Result<checked_trees::RequirementCallSpecializationFacts, Vec<diagnostics::Diagnostic>>
    {
        let checked = specializations
            .into_iter()
            .map(
                |specialization| checked_trees::CheckedRequirementCallSpecialization {
                    site: match specialization.site {
                        ValidatedNominalMachineUseSite::Statement(handle) => {
                            checked_trees::NominalMachineUseSite::Statement(handle)
                        }
                        ValidatedNominalMachineUseSite::Expression(handle) => {
                            checked_trees::NominalMachineUseSite::Expression(handle)
                        }
                    },
                    registration_operation: specialization.registration_operation,
                    type_bindings: specialization
                        .type_bindings
                        .iter()
                        .map(|binding| checked_trees::CheckedRequirementCallTypeBinding {
                            parameter: binding.parameter,
                            actual: binding.actual,
                        })
                        .collect(),
                    machine_selections: specialization
                        .machine_selections
                        .iter()
                        .map(
                            |selection| checked_trees::CheckedRequirementCallMachineSelection {
                                static_machine_ordinal: selection.static_machine_ordinal,
                                parameter: selection.parameter,
                                selected_machine: selection.selected_machine,
                                selected: selection.selected,
                            },
                        )
                        .collect(),
                },
            )
            .collect::<Vec<_>>();
        checked_trees::RequirementCallSpecializationFacts::try_with_specializations(checked)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
    }
}
pub(crate) mod resolved_types {
    /*
        Publish resolved executable types at the final immutable typed boundary.

        State roots retain the declaration telescope needed by place/result
        resolution. Specialization deep-copies its occurrences before this point,
        so handles identify occurrences, not spellings shared by generic bodies.
        Lowering receives the decisions, including genuine absence; it cannot
        rebuild a type from a local, parameter or source-shaped fallback.
    */
    use crate::flow::expressions;
    use checked_trees::{ResolvedCarrierType, ResolvedExpressionType, ResolvedTypeFacts};
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;

    pub(crate) fn resolve_program_types(
        program: &dyn validation::CheckingInput,
    ) -> ResolvedTypeFacts {
        let highest_expression = expressions(program)
            .iter_expressions()
            .map(|(handle, _)| handle.arena_index())
            .max()
            .unwrap_or(0);
        let mut expressions =
            vec![ResolvedExpressionType::default(); highest_expression as usize + 1];
        let mut pending = Vec::new();
        for machine in program.machines() {
            let states = program.machine_states(machine);
            if let Some(entry) = states.first() {
                pending.extend(
                    program
                        .machine_owned_data(machine)
                        .iter()
                        .map(|field| field.initial_value),
                );
                resolve_occurrences(program, machine, entry, &mut pending, &mut expressions);
            }
            for state in states {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    pending.extend(
                        program
                            .tables
                            .expression_roots(statement)
                            .into_iter()
                            .map(|(_, expression)| expression),
                    );
                }
                resolve_occurrences(program, machine, state, &mut pending, &mut expressions);
            }
        }
        let highest_reference = program
            .type_reference_table
            .type_reference_rows()
            .map(|(handle, _)| handle.arena_index())
            .max()
            .unwrap_or(0);
        let mut carriers = vec![ResolvedCarrierType::default(); highest_reference as usize + 1];
        for (reference, _) in program.type_reference_table.type_reference_rows() {
            carriers[reference.arena_index() as usize] = ResolvedCarrierType {
                reference,
                carrier: program
                    .unwrapped_type_reference(reference)
                    .unwrap_or_default(),
            };
        }
        ResolvedTypeFacts::with_rows(expressions, carriers)
    }

    fn resolve_occurrences(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        state: &State,
        pending: &mut Vec<ExpressionHandle>,
        expressions: &mut [ResolvedExpressionType],
    ) {
        while let Some(expression) = pending.pop() {
            if !program.expression_table.expression_is_valid(expression) {
                continue;
            }
            let row = &mut expressions[expression.arena_index() as usize];
            if row.expression == expression {
                continue;
            }
            *row = ResolvedExpressionType {
                expression,
                place: validation::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
                .unwrap_or_default(),
                result: validation::expression_result_type_reference(
                    program, machine, state, expression,
                )
                .unwrap_or_default(),
            };
            program.subtree_children(
                program.expression_table.expression(expression),
                typed_trees::SubtreeEdges::STRUCTURAL,
                |child| {
                    pending.push(child);
                    false
                },
            );
        }
    }
}
pub(crate) mod review_sources {
    /*
        Attach authored occurrence custody to the checked call topology. Targets and
        operational acknowledgements come from the operational plan; generated
        calls acquire their acknowledgements during effect inference, so their
        checked rows must not be compared with pre-inference syntax here.
    */
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionOccurrenceId as OccurrenceId,
    };
    use source::SourceSpan;
    use validation::{CallSite, collect_call_sites_in_statement};

    use crate::flow::expressions;
    use crate::published::semantic::calls::dispatch_subjects;
    use checked_trees::{FlowCallResult, FlowCallSite};
    use typed_trees::statement::StatementNode;

    pub(crate) fn bind_checked_body_call_source_spans(
        program: &dyn ::validation::CheckingInput,
        flow: &mut checked_trees::FlowFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let checked_states = flow
            .control
            .states
            .iter()
            .map(|(_, state)| (state.machine_symbol, state.state_symbol, state.calls))
            .collect::<Vec<_>>();
        for (machine_symbol, state_symbol, state_calls) in checked_states {
            let Some(machine) = program.machine_by_symbol(machine_symbol) else {
                return Err(vec![Diagnostic::error(format!(
                    "checked machine {:?} is absent while binding call source custody",
                    machine_symbol,
                ))]);
            };
            let Some((_, state)) = program.machine_state(machine_symbol, state_symbol) else {
                return Err(vec![Diagnostic::error(format!(
                    "checked state {:?} is absent from machine {:?} while binding call source custody",
                    state_symbol, machine_symbol,
                ))]);
            };
            if state_calls.is_empty() {
                continue;
            }
            let mut call_sites_by_statement = std::collections::HashMap::<
                usize,
                (
                    Vec<validation::CallSite<'_>>,
                    dispatch_subjects::StatementDispatchCalls,
                ),
            >::new();
            let statements = program.statement_table.statements(state.statement_nodes);
            let mut selected_by_statement =
                std::collections::HashMap::<usize, SelectedEvaluation>::new();
            let control = &mut flow.control;
            for checked_call in control.calls.span_mut_or_empty(state_calls) {
                let (call_sites, dispatch_calls) = call_sites_by_statement
                    .entry(checked_call.statement_index)
                    .or_insert_with(|| {
                        (
                            collect_call_sites_in_statement(
                                program,
                                machine,
                                state,
                                checked_call.statement_index,
                            ),
                            dispatch_subjects::statement_dispatch_calls(
                                program,
                                statements,
                                checked_call.statement_index,
                            ),
                        )
                    });
                let Some(call_site) = call_sites.get(checked_call.call_ordinal).copied() else {
                    return Err(vec![Diagnostic::error(format!(
                        "checked body call {} in statement {} of machine {:?} state {:?} has no exact typed call site",
                        checked_call.call_ordinal,
                        checked_call.statement_index,
                        machine_symbol,
                        state_symbol,
                    ))]);
                };
                if checked_call.runtime_call != call_site.runtime_call() {
                    return Err(vec![Diagnostic::error(
                        "checked body call lost its exact runtime callable binding while binding source custody",
                    )]);
                }
                checked_call.authored_expression = match &call_site {
                    CallSite::Expression { expression, .. } => *expression,
                    CallSite::Statement(_) | CallSite::TransitionNamed { .. } => Default::default(),
                };
                let (site, target, arguments) = match &call_site {
                    CallSite::Statement(call) => (
                        FlowCallSite::Statement,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    ),
                    CallSite::Expression { call, .. } => (
                        FlowCallSite::Expression,
                        validation::call_target_symbol(program, call),
                        program.expression_table.expression_handles(call.arguments),
                    ),
                    CallSite::TransitionNamed {
                        path, arguments, ..
                    } => (
                        FlowCallSite::NamedTransition,
                        path.symbol,
                        program.statement_table.expression_handles(*arguments),
                    ),
                };
                checked_call.authored_receiver = match &call_site {
                    CallSite::Expression { call, .. } => call.receiver,
                    CallSite::Statement(_) | CallSite::TransitionNamed { .. } => Default::default(),
                };
                checked_call.authored_result = match (
                    program
                        .statement_table
                        .statement_at(state, checked_call.statement_index),
                    &call_site,
                ) {
                    (Some(StatementNode::Call(call)), _)
                        if checked_call.call_ordinal == 0 && call.discards_result =>
                    {
                        FlowCallResult::Discarded
                    }
                    (
                        Some(StatementNode::LocalData(local)),
                        CallSite::Expression { expression, .. },
                    ) if !local.is_mutable && *expression == local.initial_value => {
                        FlowCallResult::ImmutableLocal
                    }
                    _ => FlowCallResult::Operand,
                };
                checked_call.authored_closing_invocation =
                    matches!(call_site, CallSite::TransitionNamed { .. })
                        && closing_invocation(program, state, checked_call.statement_index);
                checked_call.authored_dispatch_role =
                    dispatch_calls.role(checked_call.authored_expression);
                let selected = selected_by_statement
                    .entry(checked_call.statement_index)
                    .or_insert_with(|| {
                        statements
                            .get(checked_call.statement_index)
                            .map(|statement| selected_evaluation(program, machine, statement))
                            .unwrap_or_default()
                    });
                checked_call.authored_selected = match &call_site {
                    CallSite::Statement(_) => false,
                    CallSite::Expression { expression, .. } => {
                        selected.expressions.contains(expression)
                    }
                    CallSite::TransitionNamed { .. } => selected.transition_targets,
                };
                checked_call.authored_site = site;
                checked_call.authored_target = target;
                checked_call.static_requirement_dispatch =
                    call_site.static_requirement_dispatch().map(Into::into);
                checked_call.authored_arguments = control
                    .call_arguments
                    .insert_many(arguments.iter().copied());
                let order = call_site
                    .argument_evaluation_order(program)
                    .map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "call operand scheduling: {reason}"
                        ))]
                    })?;
                checked_call.argument_evaluation_order = if order.is_natural() {
                    Default::default()
                } else {
                    let positions = order
                        .map(|position| {
                            u32::try_from(position).map_err(|_| {
                                vec![Diagnostic::error(
                                    "call operand position exceeds the supported range",
                                )]
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    control.call_argument_positions.insert_many(positions)
                };
                match authored_call_span(program, &call_site) {
                    Ok(source_span) => {
                        checked_call.authored_source_span = source_span;
                        checked_call.authored_source_custody_valid = true;
                    }
                    Err(_) => {
                        checked_call.authored_source_span = None;
                        checked_call.authored_source_custody_valid = false;
                    }
                }
            }
        }
        Ok(())
    }

    fn authored_call_span(
        program: &dyn ::validation::CheckingInput,
        site: &CallSite<'_>,
    ) -> Result<Option<SourceSpan>, Vec<Diagnostic>> {
        match site {
            CallSite::Statement(call) => authored_attached_call_span(
                program,
                call.authored_call_selection,
                call.source_span,
                call.operational_acknowledgement.origin,
                "statement",
            ),
            CallSite::Expression { expression, call } => {
                let mut call_occurrences = Vec::new();
                for occurrence in expressions(program).authored_selection_occurrences(*expression) {
                    let Some(selection) = program.authored_declaration_selections().get(occurrence)
                    else {
                        return Err(vec![Diagnostic::error(format!(
                            "expression call retains unknown authored selection occurrence {}",
                            occurrence.ordinal(),
                        ))]);
                    };
                    if selection.kind() == Kind::Call {
                        call_occurrences.push(occurrence);
                    }
                }
                match call.operational_acknowledgement.origin {
                    language_semantics::CallOperationalAcknowledgementOrigin::Source => {
                        let [occurrence] = call_occurrences.as_slice() else {
                            return Err(vec![Diagnostic::error(format!(
                                "source-authored checked expression call has {} attached call selections; expected one",
                                call_occurrences.len(),
                            ))]);
                        };
                        validate_occurrence(program, *occurrence, None, "expression").map(Some)
                    }
                    language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => {
                        if !call_occurrences.is_empty() {
                            return Err(vec![Diagnostic::error(
                                "compiler-synthesized checked expression call retains authored call provenance",
                            )]);
                        }
                        Ok(None)
                    }
                }
            }
            CallSite::TransitionNamed {
                source_span,
                authored_call_selection,
                ..
            } => match (*authored_call_selection, nonempty_span(*source_span)) {
                (Some(occurrence), Some(source_span)) => {
                    validate_occurrence(program, occurrence, Some(source_span), "transition")
                        .map(Some)
                }
                (None, None) => Ok(None),
                _ => Err(vec![Diagnostic::error(
                    "named transition has contradictory authored call provenance",
                )]),
            },
        }
    }

    fn authored_attached_call_span(
        program: &dyn ::validation::CheckingInput,
        occurrence: Option<OccurrenceId>,
        source_span: SourceSpan,
        origin: language_semantics::CallOperationalAcknowledgementOrigin,
        form: &str,
    ) -> Result<Option<SourceSpan>, Vec<Diagnostic>> {
        match origin {
            language_semantics::CallOperationalAcknowledgementOrigin::Source => {
                let Some(occurrence) = occurrence else {
                    return Err(vec![Diagnostic::error(format!(
                        "source-authored checked {form} call has no attached call selection",
                    ))]);
                };
                let Some(source_span) = nonempty_span(source_span) else {
                    return Err(vec![Diagnostic::error(format!(
                        "source-authored checked {form} call has no exact source span",
                    ))]);
                };
                validate_occurrence(program, occurrence, Some(source_span), form).map(Some)
            }
            language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => {
                if occurrence.is_some() || nonempty_span(source_span).is_some() {
                    return Err(vec![Diagnostic::error(format!(
                        "compiler-synthesized checked {form} call retains authored provenance",
                    ))]);
                }
                Ok(None)
            }
        }
    }

    fn validate_occurrence(
        program: &dyn ::validation::CheckingInput,
        occurrence: OccurrenceId,
        expected_span: Option<SourceSpan>,
        form: &str,
    ) -> Result<SourceSpan, Vec<Diagnostic>> {
        let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
            return Err(vec![Diagnostic::error(format!(
                "checked {form} call retains unknown authored selection occurrence {}",
                occurrence.ordinal(),
            ))]);
        };
        if selection.kind() != Kind::Call
            || selection.exposure() != Exposure::PrivateImplementation
            || nonempty_span(selection.source_span()).is_none()
            || expected_span.is_some_and(|expected| expected != selection.source_span())
        {
            return Err(vec![
                Diagnostic::error(format!(
                    "checked {form} call retains invalid authored call-selection evidence",
                ))
                .with_source_span(selection.source_span()),
            ]);
        }
        Ok(selection.source_span())
    }

    fn nonempty_span(source_span: SourceSpan) -> Option<SourceSpan> {
        (source_span.span.start < source_span.span.end).then_some(source_span)
    }

    /*
        Whether `statement_index` is the state's only transition: its final
        statement, unconditional, without continuation, exiting ordinarily to a
        selected named target. A paired unconditional arm after guarded ones is a
        guarded tail's fallthrough instead.
    */
    fn closing_invocation(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> bool {
        use typed_trees::statement::{TransitionExit, TransitionGuardNode, TransitionTargetNode};
        let statements = program.statement_table.statements(state.statement_nodes);
        let Some(StatementNode::Transition(transition)) = statements.get(statement_index) else {
            return false;
        };
        statement_index + 1 == statements.len()
            && !statements[..statement_index]
                .iter()
                .any(|statement| matches!(statement, StatementNode::Transition(_)))
            && transition.guard == TransitionGuardNode::Always
            && !transition.continuation.is_valid()
            && transition.exit == TransitionExit::Ordinary
            && program
                .statement_table
                .transition_target_is_valid(transition.target)
            && matches!(
                program.statement_table.transition_target(transition.target),
                TransitionTargetNode::Named { static_machine_parameter, .. }
                    if !static_machine_parameter.is_valid()
            )
    }

    /*
        One statement's selected evaluation: the expressions that run only on a
        selected path, and whether its transition targets are selected.
    */
    #[derive(Default)]
    struct SelectedEvaluation {
        expressions: Vec<typed_trees::expression::ExpressionHandle>,
        transition_targets: bool,
    }

    /*
        Mark what runs only on a selected path of `statement`: the right operand
        of `&&`/`||`, a match arm's pattern and value, and a transition's target
        and continuation, except an unguarded transition into another machine,
        whose call every path performs. Selection is inherited by every nested
        expression.
    */
    fn selected_evaluation(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        statement: &StatementNode,
    ) -> SelectedEvaluation {
        use typed_trees::expression::{BinaryOperator, ExpressionNode};
        use typed_trees::statement::{TransitionGuardNode, TransitionTargetNode};

        let table = &program.statement_table;
        let mut pending = Vec::new();
        let mut transition_targets = false;
        match statement {
            StatementNode::Call(call) => pending.extend(
                table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| (*argument, false)),
            ),
            StatementNode::LocalData(local) => pending.push((local.initial_value, false)),
            StatementNode::Assignment(assignment) => {
                pending.push((assignment.target, false));
                pending.push((assignment.value, false));
            }
            StatementNode::Expression(expression) => pending.push((*expression, false)),
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    pending.push((guard, false));
                }
                let internal = |target: symbols::SymbolHandle| {
                    target == machine.symbol
                        || typed_trees::state::state_by_symbol(
                            program.machine_states(machine),
                            target,
                        )
                        .is_some()
                };
                transition_targets = !(transition.guard == TransitionGuardNode::Always
                    && matches!(
                        table.transition_target(transition.target),
                        TransitionTargetNode::Named { path, .. } if !internal(path.symbol)
                    ));
                for target in transition.targets() {
                    match table.transition_target(target) {
                        TransitionTargetNode::Value(expression) => {
                            pending.push((*expression, transition_targets));
                        }
                        TransitionTargetNode::Named { arguments, .. } => pending.extend(
                            table
                                .expression_handles(*arguments)
                                .iter()
                                .map(|argument| (*argument, transition_targets)),
                        ),
                        TransitionTargetNode::Terminal => {}
                    }
                }
            }
            _ => {}
        }
        let mut expressions = Vec::new();
        let mut visited = Vec::new();
        while let Some((expression, selected)) = pending.pop() {
            if !program.expression_table.expression_is_valid(expression)
                || visited.contains(&expression)
            {
                continue;
            }
            visited.push(expression);
            if selected {
                expressions.push(expression);
            }
            let node = program.expression_table.expression(expression);
            let mut position = 0;
            ::validation::expression_traversal::children(program, node, |child| {
                let child_selected = selected
                    || match node {
                        ExpressionNode::Binary(binary) => {
                            matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                                && position == 1
                        }
                        ExpressionNode::Match(_) => position > 0,
                        _ => false,
                    };
                pending.push((child, child_selected));
                position += 1;
            });
        }
        SelectedEvaluation {
            expressions,
            transition_targets,
        }
    }
}
pub(crate) mod state_locals {
    /*
        Record every declared state's `let` locals for lowering
        (`checked_trees::StateLocalFacts`).
    */
    use crate::flow::indexed_statements;
    use checked_trees::{CheckedStateLocal, StateLocalFacts};

    pub(crate) fn record_state_locals(program: &dyn validation::CheckingInput) -> StateLocalFacts {
        let mut states = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut locals = Vec::new();
                for (statement_index, statement) in
                    indexed_statements(program, state.statement_nodes)
                {
                    let Some(local) = statement.as_local_data() else {
                        continue;
                    };
                    let Ok(statement_index) = u32::try_from(statement_index) else {
                        break;
                    };
                    locals.push(CheckedStateLocal {
                        statement_index,
                        symbol: local.symbol,
                        declared_type: local.type_reference,
                        type_is_inferred: local.type_is_inferred,
                        is_mutable: local.is_mutable,
                        is_erased: local.relevance.is_erased(),
                        is_scalar_value: !local.is_mutable
                            && program
                                .primitive_type_reference(local.type_reference)
                                .is_some(),
                    });
                }
                states.push((state.symbol, locals));
            }
        }
        StateLocalFacts::with_states(states)
    }
}
mod static_callable_crashes {
    /*
        Universal crash refinement consumes the same completed private summary as
        ordinary calls. An omitted authored private clause requests inference; it
        cannot stand for an empty actual ceiling before that inference completes.
        The retained obligation owns its exact applied requirement and selected
        entry, including demands from target-family members without public rows.
        No selecting caller's values or guards participate in this comparison.
    */
    use super::boolean_predicates::bind_parameters;
    use super::boolean_predicates::proves;
    use super::crash_calls::CompletedBodyCrashSummaries;
    use super::crash_plan_facts::derive_authored_machine_crash_buckets;
    use super::crash_plan_facts::derive_authored_signature_crash_buckets;
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedOperatorFacts, CrashInterface, CrashRouteGuard,
        MachineContractPlans,
    };
    use diagnostics::Diagnostic;
    use validation::StaticMachineContractObligation;

    pub(super) fn validate(
        program: &dyn validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        obligations: &[StaticMachineContractObligation],
        plans: &MachineContractPlans,
        completed: &CompletedBodyCrashSummaries,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        for obligation in obligations {
            if let Err(reason) = validate_one(program, operators, obligation, plans, completed) {
                diagnostics.push(Diagnostic::error(format!(
                    "{} does not refine its callable crash contract: {reason}",
                    obligation.label()
                )));
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn validate_one(
        program: &dyn validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        obligation: &StaticMachineContractObligation,
        plans: &MachineContractPlans,
        completed: &CompletedBodyCrashSummaries,
    ) -> Result<(), String> {
        let requirement = obligation
            .required_signature(program)
            .map_err(str::to_owned)?;
        let required =
            derive_authored_signature_crash_buckets(program, requirement, Some(operators))
                .map_err(|error| {
                    format!("the exact required crash ceiling cannot be normalized: {error:?}")
                })?;
        let selected = program
            .machine_by_own_or_state_symbol(obligation.selected_entry())
            .ok_or_else(|| "the selected entry has no concrete callable owner".to_owned())?;
        let plan = plans.for_machine(selected.symbol).ok_or_else(|| {
            "the selected callable has no completed checked contract plan".to_owned()
        })?;
        let actual = match plan.crash.interface() {
            CrashInterface::PublishedCeiling => {
                derive_authored_machine_crash_buckets(program, selected, Some(operators)).map_err(
                    |error| {
                        format!("the selected published ceiling cannot be normalized: {error:?}")
                    },
                )?
            }
            CrashInterface::InternalInferred => {
                completed.checked_buckets(selected.symbol).ok_or_else(|| {
                    "the selected private callable has an unresolved crash summary".to_owned()
                })?
            }
        };
        let entry = program
            .machine_states(selected)
            .first()
            .ok_or_else(|| "the selected callable has no exact entry telescope".to_owned())?;
        let parameters = program.state_parameters(entry);
        let premises = program
            .state_signature_contracts(requirement)
            .iter()
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .filter_map(|fact| {
                let expression = fact.as_expression()?;
                let checked = crate::values::lower_signature_crash_contract_expression(
                    program,
                    operators,
                    requirement,
                    *expression,
                )?;
                bind_parameters(
                    program,
                    selected.symbol,
                    entry.symbol,
                    parameters,
                    None,
                    checked,
                )
            })
            .collect::<Vec<_>>();
        for bucket in &actual {
            let allowed = required
                .iter()
                .filter(|permitted| permitted.cause() == bucket.cause())
                .flat_map(|permitted| permitted.alternative_guards())
                .filter_map(checked_guard)
                .reduce(|left, right| Boolean::Or {
                    left: Box::new(left),
                    right: Box::new(right),
                })
                .unwrap_or(Boolean::Constant(false));
            for route in bucket.alternative_guards() {
                let under_required_inputs = checked_guard(route)
                    .and_then(|actual| {
                        bind_parameters(
                            program,
                            selected.symbol,
                            entry.symbol,
                            parameters,
                            None,
                            Boolean::Or {
                                left: Box::new(Boolean::Not(Box::new(actual))),
                                right: Box::new(allowed.clone()),
                            },
                        )
                    })
                    .is_some_and(|goal| proves(&premises, &goal, |_| None));
                if !under_required_inputs
                    && !required
                        .iter()
                        .filter(|permitted| permitted.cause() == bucket.cause())
                        .any(|permitted| {
                            permitted.alternative_guards().iter().any(|guard| {
                                crate::checks::call_route_guard_covers(guard, route, &[])
                            })
                        })
                {
                    return Err(format!(
                        "its completed {:?} crash routes exceed the required ceiling",
                        bucket.cause()
                    ));
                }
            }
        }
        Ok(())
    }

    /*
        A missing annotation remains unavailable; structural identity comparison
        above can still authenticate an identical opaque route without reinterpreting it.
    */
    fn checked_guard(guard: &CrashRouteGuard) -> Option<Boolean> {
        match guard {
            CrashRouteGuard::Truth => Some(Boolean::Constant(true)),
            CrashRouteGuard::Predicate(predicate) => predicate.scalar_expression().cloned(),
        }
    }
}

pub(crate) use ::validation::field_domains::domain_is_vacuous;
pub(crate) use crash_calls::{
    infer_checked_crash_causes, infer_checked_machine_crash_causes, selected_invocation_target,
};
pub(crate) use crash_plan_facts::{
    canonical_crash_binary_path_predicate, canonical_crash_contract_predicate,
    canonical_crash_operand_identity, canonical_crash_path_predicate, declared_parameter_names,
    derive_authored_machine_crash_buckets, derive_authored_operator_crash_buckets,
    derive_authored_signature_crash_buckets, machine_entry_parameter_names,
};
pub(crate) use dynamic_conformance::normalized_dynamic_row_identities;
pub(crate) use qualification_evidence::{checked_validation_evidence, propagated_evidence};

/*
    Checker-side read of the shared crash entry-provenance law. Guard
    admission and call-actual substitution must agree on when a source read
    still evaluates to the invocation-entry operand its spelling claims;
    `crash_entry_values` stays the only owner of that answer, so this
    forwards without re-deriving provenance in the check layer.
*/
pub(crate) fn crash_entry_operand(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    before_statement: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<checked_trees::CrashPredicateExpression> {
    crash_entry_values::entry_operand(program, machine, state, before_statement, expression)
}

use crate::borrow::build_borrow_facts;
use crate::flow::{build_domain_facts, build_flow_facts};
use crate::published::facts::access_views_and_uses::{
    build_checked_access_view_inputs, build_nominal_machine_use_facts,
};
use crate::published::facts::capabilities::build_capability_facts;
use crate::published::facts::contract_plan_facts::{build_contract_plans, build_mutation_facts};
pub(crate) use crate::published::facts::dynamic_conformance::build_dynamic_conformance_facts;
use crate::published::facts::machine_facts::{
    build_blocking_facts, build_suspension_facts, build_synchronous_invocation_facts,
    build_termination_facts,
};
use crate::published::facts::qualification_facts::{
    build_qualification_facts, build_service_reach_facts,
};
use crate::published::facts::requirement_call_specializations::build_requirement_call_specialization_facts;
use crate::published::proof::build_proof_facts;
use crate::published::semantic::facts::build_semantic_facts;
use crate::shared::operators::bind_boundary_operator_application_demands;
use crate::values::build_value_facts;
use checked_trees::CheckFacts;
use flow_effects::OperationalPlan;
use proof::obligations::ProofPlan;
use symbols::SymbolHandle;
use validation::build_operator_facts;
use validation::select_pending_domain_operator_meanings;

#[derive(Clone, Copy)]
struct MachineSuspensionRow {
    symbol: SymbolHandle,
    published_may_suspend: bool,
    transitive_may_suspend: bool,
}

#[derive(Clone, Copy)]
struct MachineBlockingRow {
    symbol: SymbolHandle,
    published_may_block: bool,
    transitive_may_block: bool,
}

fn project_operational_rows(
    operational: &OperationalPlan,
) -> (Vec<MachineSuspensionRow>, Vec<MachineBlockingRow>) {
    let suspensions = operational
        .machines()
        .iter()
        .map(|summary| MachineSuspensionRow {
            symbol: summary.symbol,
            published_may_suspend: summary.published_may_suspend,
            transitive_may_suspend: summary.transitive_may_suspend,
        })
        .collect();
    let blocking = operational
        .machines()
        .iter()
        .map(|summary| MachineBlockingRow {
            symbol: summary.symbol,
            published_may_block: summary.published_may_block,
            transitive_may_block: summary.transitive_may_block,
        })
        .collect();
    (suspensions, blocking)
}

/*
    Keep completed storage available when reporting a later failure too.
    Earlier failures still use the original checking input.
*/
pub(crate) struct CheckFactFailure<'program> {
    pub(crate) program: Option<Box<validation::CheckingContext<'program>>>,
    pub(crate) diagnostics: Vec<diagnostics::Diagnostic>,
}

impl<'program> From<Vec<diagnostics::Diagnostic>> for CheckFactFailure<'program> {
    fn from(diagnostics: Vec<diagnostics::Diagnostic>) -> Self {
        Self {
            program: None,
            diagnostics,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_check_facts<'program>(
    program_context: &::validation::CheckingContext<'program>,
    proof_plan: &ProofPlan<'_>,
    content: checked_trees::ContentProjectionFacts,
    integer_assignments: &[proof::checker::CheckedIntegerAssignment],
    operational: OperationalPlan,
    service_reach_inference: flow_effects::ServiceReachInferencePlan,
    mut validation_facts: validation::ProgramValidationFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<
    (
        CheckFacts,
        validation::PreparedEffectPlans,
        validation::CheckingContext<'program>,
        Vec<diagnostics::Diagnostic>,
    ),
    CheckFactFailure<'program>,
> {
    let program: &dyn validation::CheckingInput = program_context;
    let call_frames = validation::CallFrameResolver::new(program);
    let borrow = build_borrow_facts(program, call_frames.as_ref())?;
    let mut values = build_value_facts(program, proof_plan);
    let mut operators = build_operator_facts(program, &values);
    operators.exact_integer_casts = validation_facts.exact_integer_casts.clone();
    operators.decided_formations = validation_facts.decided_formations.clone();
    operators.trap_free_expressions = validation_facts.trap_free_expressions.clone();
    let mut proof = build_proof_facts(program, proof_plan, &borrow, &operators)?;
    for decision in integer_assignments {
        let proof::obligations::ProofObligation::BoundedAssignment(assignment) =
            proof_plan.obligations.get(decision.obligation)
        else {
            unreachable!("integer assignment judgment retains its exact obligation");
        };
        proof
            .integer_assignments
            .append(checked_trees::CheckedIntegerAssignmentDecision {
                atomic_resident: false,
                machine: assignment.machine_symbol,
                state: assignment.state_symbol,
                statement_index: assignment.statement_index,
                target: assignment.target,
                value: assignment.value,
                target_type: assignment.base_type,
                minimum: decision.range.minimum.clone(),
                maximum: decision.range.maximum.clone(),
            });
    }
    for decision in validation_facts.integer_assignments.drain(..) {
        integer_writes::retain_assignment_decision(program, &mut proof, decision);
    }

    crate::published::proof::bind_float_meaning_projection_facts(
        program,
        &mut proof,
        &validation_facts.float_meaning_projection_invocations,
        &validation_facts.float_meaning_equality_propositions,
    )?;
    crate::published::proof::bind_proof_output_call_facts(program, &mut proof)?;
    crate::published::proof::bind_evidence_forwarding_facts(program, &mut proof)?;
    crate::published::proof::bind_outcome_specific_arm_facts(program, &mut proof)?;
    crate::published::proof::bind_evidence_projection_facts(program, &mut proof)?;
    let semantic =
        build_semantic_facts(program, &proof, &operators).map_err(|diagnostic| vec![diagnostic])?;
    let domains = build_domain_facts(program, &semantic);
    select_pending_domain_operator_meanings(program, &mut operators);
    bind_boundary_operator_application_demands(
        program,
        &validation_facts.boundary_operator_applications,
        &mut operators,
    )?;
    values.scalar_expressions = crate::values::build_checked_scalar_expression_plans(
        program,
        &operators,
        &validation_facts.exact_integer_casts,
        &mut values.proof_terms.terms,
    );
    values.scalar_expressions.seal_expressions();
    let mut prepared = CheckFacts {
        borrow,
        proof,
        values,
        domains,
        operators,
        qualifications: checked_trees::QualificationFacts {
            content,
            ..Default::default()
        },
        ..CheckFacts::default()
    };
    let mut claim_outcomes = crate::checks::ClaimOutcomePreparation::default();
    {
        mutation_summaries.prewarm(program, &prepared.borrow, call_frames.as_ref());
    };
    let mut previous_construction: Option<FlowConstruction> = None;
    let mut walk_memory = crate::checks::PermissionWalkMemory::default();
    let mut retrying_permissions = false;
    loop {
        prepared.semantic.clone_from(&semantic);
        contextual_cleanup::entry_flow::seed_entry_requirements(
            program,
            &mut prepared.semantic,
            &prepared.contextual_cleanup.entry_requirements,
        )?;
        let reusable = previous_construction
            .as_ref()
            .filter(|construction| construction.read_the_same_inputs(&prepared));
        let walk_inputs = crate::checks::WalkInputs {
            restored: std::mem::take(&mut retrying_permissions) && reusable.is_some(),
        };
        if let Some(construction) = reusable {
            prepared.semantic.clone_from(&construction.semantic_after);
            prepared.flow.clone_from(&construction.flow);
        } else {
            let inputs = FlowConstructionInputs::capture(&prepared);
            prepared.flow = build_flow_facts(
                program,
                &prepared.borrow,
                &prepared.proof,
                &mut prepared.semantic,
                &prepared.domains,
                &operational,
                &service_reach_inference,
                &prepared.values.scalar_expressions,
                &prepared.operators,
                mutation_summaries,
                call_frames.as_ref(),
            );
            prepared.flow.control.seal_indexes();
            previous_construction = Some(FlowConstruction {
                inputs,
                semantic_after: prepared.semantic.clone(),
                flow: prepared.flow.clone(),
            });
        }
        crate::published::facts::review_sources::bind_checked_body_call_source_spans(
            program,
            &mut prepared.flow,
        )?;
        let permission_outcome = crate::checks::prepare_permission_events(
            program,
            &mut prepared,
            &mut claim_outcomes,
            mutation_summaries,
            call_frames.as_ref(),
            &mut walk_memory,
            walk_inputs,
        );
        if permission_outcome? {
            retrying_permissions = true;
            continue;
        }
        if contextual_cleanup::retain_selected_receiver_loans(
            program,
            &mut prepared.borrow,
            &prepared.flow.ownership,
            &prepared.contextual_cleanup,
        ) {
            continue;
        }
        if !contextual_cleanup::infer_entry_requirements(
            program,
            &mut prepared,
            call_frames.as_ref(),
        )? {
            break;
        }
    }
    /*
        The fixed point is over. Its retry snapshots (the inputs one flow
        construction read, the facts it left, the walk's memory) and the
        declaration fact plan each iteration restarted from are whole copies
        of the largest tables in checking. Nothing after this reads them, so
        they are released before the publication phases below allocate theirs
        rather than at the end of this function.
    */
    drop(previous_construction);
    drop(walk_memory);
    drop(semantic);
    let completed =
        program_context.clone_with_storage_graph(std::sync::Arc::new(prepared.flow.clone()));
    match finish_check_facts(
        &completed,
        prepared,
        operational,
        service_reach_inference,
        validation_facts,
    ) {
        Ok((facts, effects, overflow)) => Ok((facts, effects, completed, overflow)),
        Err(diagnostics) => Err(CheckFactFailure {
            program: Some(Box::new(completed)),
            diagnostics,
        }),
    }
}

/* Consume the published storage history for proofs and execution plans. */
fn finish_check_facts(
    program_context: &validation::CheckingContext<'_>,
    mut prepared: CheckFacts,
    operational: OperationalPlan,
    service_reach_inference: flow_effects::ServiceReachInferencePlan,
    mut validation_facts: validation::ProgramValidationFacts,
) -> Result<
    (
        CheckFacts,
        validation::PreparedEffectPlans,
        Vec<diagnostics::Diagnostic>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let program: &dyn validation::CheckingInput = program_context;
    let overflow = validation_facts.discharge_storage_formations(program)?;
    prepared.operators.decided_formations = validation_facts.decided_formations.clone();
    let published = (|| -> Result<
        (CheckFacts, validation::PreparedEffectPlans),
        Vec<diagnostics::Diagnostic>,
    > {
    prepared.flow.ownership.seal_indexes();
    prepared.flow.control.seal_indexes();
    crate::checks::publish_owned_transfer_paths(program, &mut prepared);
    crate::borrow::accesses::retain_completed_dispatch_extractions(program, &mut prepared);
    {
            crate::values::retain_nested_call_arguments(
                program,
                &prepared.operators,
                &prepared.flow,
                &mut prepared.values.scalar_expressions,
                &mut prepared.values.proof_terms.terms,
                &validation_facts.exact_integer_casts,
            );
            (
                prepared.values.scalar_computations,
                prepared.values.structural_values,
                prepared.values.completions,
            ) = crate::execution::scalar_computations::build_checked_value_computation_plans(
                program,
                &prepared.operators,
                &prepared.semantic,
                &prepared.flow,
                &prepared.borrow,
                &prepared.proof,
                &mut prepared.values.scalar_expressions,
                &validation_facts.exact_integer_casts,
            );
        };
    crate::execution::content_consumption::attach(program, &mut prepared)?;
    let index_compatibility = index_compatibility::build_index_compatibility_facts(
                program,
                &prepared.operators,
                &prepared.semantic,
                &prepared.flow,
                &prepared.proof,
            )?;
    let dependencies =
        crate::flow::derive_cleanup_effect_dependencies(program, &operational, &prepared)?;
    let dynamic_dependencies =
        crate::flow::dynamic_effect_dependencies::derive(program, &prepared)?;
    let (operational, service_reach_inference) =
        if dependencies.is_empty() && dynamic_dependencies.calls.is_empty() {
            (operational, service_reach_inference)
        } else {
            let operational = validation::infer_operational_may_for_dependencies(
                program,
                &dependencies,
                &dynamic_dependencies,
            );
            let service_reach_inference = validation::infer_service_reaches(program, &operational);
            (operational, service_reach_inference)
        };
    let prepared_effects = validation::PreparedEffectPlans {
        operational: operational.clone(),
        service_reaches: service_reach_inference.clone(),
    };
    let completed = program_context.clone_with_prepared_effect_plans(&prepared_effects);
    let program: &dyn validation::CheckingInput = &completed;
    let call_frames = validation::CallFrameResolver::new(program);
    validation::admit_prepared_program_effects(program, &operational, &mut validation_facts)?;
    let static_machine_selections = std::mem::take(&mut validation_facts.static_machine_selections);
    crate::checking::call_acknowledgements::validate_call_acknowledgements(program, &operational)?;
    crate::flow::attach_reach_summaries(
        program,
        &mut prepared.flow,
        &service_reach_inference,
        &operational,
    );
    let CheckFacts {
        semantic,
        borrow,
        proof,
        values,
        domains,
        operators,
        mut flow,
        contextual_cleanup,
        qualifications,
        ..
    } = prepared;
    flow.guarded_exits = crate::execution::terminal_scalar::build_checked_guarded_exit_plans(
        program,
        &values.scalar_expressions,
        &values.scalar_computations,
    );
    flow.terminal_debug =
        crate::execution::terminal_debug::build_checked_terminal_debug_plans(program);
    let dynamic_conformances = build_dynamic_conformance_facts(program)?;
    let capabilities = build_capability_facts(program, &service_reach_inference, &flow);
    let (machine_suspensions, machine_blocking) = project_operational_rows(&operational);
    let qualifications = build_qualification_facts(program, qualifications.content);
    let synchronous_invocations = build_synchronous_invocation_facts(program);
    let suspensions = build_suspension_facts(program, &machine_suspensions);
    let blocking = build_blocking_facts(program, &machine_blocking);
    let mut termination = build_termination_facts(
                program,
                &flow,
                &semantic,
                &operational.cleanup_dependencies,
                &validation_facts,
                call_frames.as_ref(),
            )?;
    let mut fact_call_projection_diagnostics = Vec::new();
    validation::validate_ordered_requirement_call_totality(
        program,
        &operational,
        &service_reach_inference,
        &termination,
        &mut fact_call_projection_diagnostics,
    );
    let service_reaches = build_service_reach_facts(program, service_reach_inference);
    let mutation = {
            let mut mutation = build_mutation_facts(program, call_frames.as_ref());
            integer_writes::retain(
                program,
                &operators,
                &values,
                &flow,
                &semantic,
                call_frames.as_ref(),
                &mut mutation,
            );
            mutation
        };
    let (contract_plans, completed_crashes) = build_contract_plans(
                program,
                &proof,
                &service_reaches,
                &synchronous_invocations,
                &suspensions,
                &blocking,
                &termination,
                &mutation,
                &capabilities,
                &flow,
                &operators,
                &semantic,
                &validation_facts.exact_integer_casts,
                &values.scalar_expressions,
                &values.scalar_computations,
                &values.structural_values,
            )?;
    static_callable_crashes::validate(
        program,
        &operators,
        &static_machine_selections.crash_refinements,
        &contract_plans,
        &completed_crashes,
    )?;
    let nominal_machine_uses = build_nominal_machine_use_facts(
        program,
        static_machine_selections.nominal_uses,
        &contract_plans,
    )?;
    let mut installed_interrupt_registrations = Vec::new();
    let mut next_callback_index = nominal_machine_uses.uses.len();
    for registration in static_machine_selections.installed_interrupt_registrations {
        let mut callbacks =
            build_nominal_machine_use_facts(program, registration.nominal_uses, &contract_plans)?;
        for callback in &mut callbacks.uses {
            callback.application_index = u32::try_from(next_callback_index).map_err(|_| {
                vec![diagnostics::Diagnostic::error(
                    "installation callback count exceeds u32",
                )]
            })?;
            next_callback_index += 1;
        }
        installed_interrupt_registrations.push(
            checked_trees::CheckedInstalledInterruptRegistration {
                site: match registration.site {
                    validation::ValidatedNominalMachineUseSite::Statement(site) => {
                        checked_trees::NominalMachineUseSite::Statement(site)
                    }
                    validation::ValidatedNominalMachineUseSite::Expression(site) => {
                        checked_trees::NominalMachineUseSite::Expression(site)
                    }
                },
                control_service: registration.control_service,
                application: registration.application,
                target: registration.target,
                registrar_owner: registration.registrar_owner,
                registrar_application_identity: registration.registrar_application_identity,
                registrar_declaring_application: registration.registrar_declaring_application,
                registrar_arguments: registration.registrar_arguments,
                registrar_lifetimes: registration.registrar_lifetimes,
                timer_slot: registration.timer_slot,
                slot_application: registration.slot_application,
                callbacks,
                specializations: build_requirement_call_specialization_facts(
                    registration.specializations,
                )?,
            },
        );
    }
    let requirement_call_specializations = build_requirement_call_specialization_facts(
        static_machine_selections.requirement_call_specializations,
    )?;
    let carry = carry::build_carry_facts(program);
    validation::admit_specification_calls(
        program,
        &validation_facts.specification_calls,
        &|machine| {
            termination
                .for_machine(machine)
                .map(|plan| plan.checked_summary.clone())
        },
    )?;
    declaration_calls::bind(
        &mut termination.rank_arrivals,
        &validation_facts.specification_calls,
        &contract_plans,
    )?;
    let mut fact_call_projections = Vec::new();
    for call in &validation_facts.integer_embedding_calls {
        let exact_source = matches!(program.expression_table.expression(call.call_expression),
            typed_trees::expression::ExpressionNode::Call(source) if validation::call_target_entry_symbol(program, source.target_symbol) == call.target_state)
            && program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == call.target_machine)
                .filter(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .filter(|state| state.symbol == call.target_state)
                        .count()
                        == 1
                })
                .count()
                == 1;
        let total = termination.for_machine(call.target_machine).is_some_and(|plan| {
            matches!(&plan.checked_summary,
                language_semantics::TerminationGuarantee::Terminates { premises } if premises.is_empty())
        });
        if !exact_source || !total {
            let target = program.symbols.name(call.target_state);
            fact_call_projection_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "`embed` source call `{target}` is not denotational: the exact selected machine is not unconditionally terminating"
            )));
        }
    }
    for projection in &validation_facts.fact_call_projections {
        let total = termination
            .for_machine(projection.target_machine)
            .is_some_and(|plan| {
                matches!(
                    &plan.checked_summary,
                    language_semantics::TerminationGuarantee::Terminates { premises }
                        if premises.is_empty()
                )
            });
        if !total {
            let target = program.symbols.name(projection.target_state);
            fact_call_projection_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "fact-position projection from call `{target}` is not denotational: the selected machine is not unconditionally terminating"
            )));
            continue;
        }
        fact_call_projections.push(checked_trees::CheckedFactCallProjection {
            projection_expression: projection.projection_expression,
            call_expression: projection.call_expression,
            target_machine: projection.target_machine,
            target_state: projection.target_state,
            machine_arguments: projection.machine_arguments.clone(),
            result_type: projection.result_type,
            field: projection.field,
        });
    }
    if !fact_call_projection_diagnostics.is_empty() {
        Err(fact_call_projection_diagnostics)?;
    }

    let mut stored_integer_writes = arena::Arena::new();
    stored_integer_writes.insert_many(validation_facts.stored_integer_writes);
    Ok((
        CheckFacts {
            quotient_formations: validation_facts.quotient_formations,
            stored_integer_writes,
            operational,
            contextual_cleanup,
            semantic,
            borrow,
            proof,
            values,
            domains,
            dynamic_conformances,
            nominal_machine_uses,
            installed_interrupt_registrations,
            requirement_call_specializations,
            operators,
            capabilities,
            flow,
            index_compatibility,
            mutation,
            service_reaches,
            synchronous_invocations,
            suspensions,
            blocking,
            termination,
            qualifications,
            contract_plans,
            carry,
            fact_call_projections,
            access_view_inputs: build_checked_access_view_inputs(program),
            ..CheckFacts::default()
        },
        prepared_effects,
    ))
    })();
    match published {
        Ok((facts, effects)) => Ok((facts, effects, overflow)),
        Err(errors) => {
            let mut diagnostics = overflow;
            diagnostics.extend(errors);
            Err(diagnostics)
        }
    }
}

/*
    Crash refinement gains path-conditioned and permission-frontier evidence
    during checked-fact validation. Refresh only that independently mutable
    axis so the realized envelope cannot retain the earlier pre-check snapshot.
*/
pub(crate) fn refresh_realized_contract_envelopes(facts: &mut CheckFacts) {
    let (machines, envelopes) = facts.contract_plans.machines_and_envelopes_mut();
    for (index, envelope) in envelopes.iter_mut().enumerate() {
        let contract = machines
            .get(index)
            .filter(|contract| contract.machine == envelope.machine)
            .or_else(|| {
                machines
                    .iter()
                    .find(|contract| contract.machine == envelope.machine)
            })
            .expect("every realized envelope must retain its exact machine contract");
        assert_eq!(
            envelope.contract_report_fingerprint, contract.report_fingerprint,
            "realized envelope contract identity drifted during checked validation"
        );
        envelope.checked_crash = contract.crash.clone();
    }
    facts
        .contract_plans
        .validate_resource_envelopes()
        .expect("checked resource envelopes must survive independent post-validation replay");
}

pub(crate) use qualification_facts::build_content_facts;

/*
    The facts one flow construction read, captured before it ran. Flow
    construction is a function of these plus declaration data that does not
    change inside the fixed point.

    The semantic plan it read is not copied. Each iteration rebuilds that plan
    from the declaration plan, which the fixed point never changes, by seeding
    the entry requirements inferred so far; equal requirements therefore give
    an equal plan, and the requirements are a few rows where the plan is the
    largest table in checking.
*/
struct FlowConstructionInputs {
    entry_requirements: checked_trees::ContextualCleanupEntryRequirements,
    borrow: checked_trees::BorrowFacts,
    proof: checked_trees::ProofFacts,
    domains: checked_trees::DomainFacts,
    scalar_expressions: checked_trees::CheckedScalarExpressionPlans,
    operators: checked_trees::CheckedOperatorFacts,
}

impl FlowConstructionInputs {
    fn capture(prepared: &CheckFacts) -> Self {
        Self {
            entry_requirements: prepared.contextual_cleanup.entry_requirements.clone(),
            borrow: prepared.borrow.clone(),
            proof: prepared.proof.clone(),
            domains: prepared.domains.clone(),
            scalar_expressions: prepared.values.scalar_expressions.clone(),
            operators: prepared.operators.clone(),
        }
    }
}

/* One completed flow construction and the inputs it read. */
struct FlowConstruction {
    inputs: FlowConstructionInputs,
    semantic_after: facts::FactPlan,
    flow: checked_trees::FlowFacts,
}

impl FlowConstruction {
    fn read_the_same_inputs(&self, prepared: &CheckFacts) -> bool {
        let inputs = &self.inputs;
        inputs.entry_requirements == prepared.contextual_cleanup.entry_requirements
            && inputs.borrow == prepared.borrow
            && inputs.proof == prepared.proof
            && inputs.domains == prepared.domains
            && inputs.scalar_expressions == prepared.values.scalar_expressions
            && inputs.operators == prepared.operators
    }
}
