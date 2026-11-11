/*
    Borrow checks over the facts constructed once by the check stage.
    Calls and statements judge conflicts at their actual evaluation points;
    reads, writes, call operands and exclusive operations share one live-loan
    judgment over the storage they may denote (`place_access`); a call's
    operands and an aggregate's reference leaves share one simultaneous-hold
    judgment; returned views and stored carriers are checked for escape and
    persistence.
    Resource rows are prepared before this pass for lowering. Compatibility
    results are transient judgments, not certificates to serialize and replay.
    Compatibility also consumes completed scalar observations at each use;
    immutable owned values retain identity, while mutable storage and borrowed
    referents require their existing version and lineage evidence.
*/

/* Checks over signatures and bodies that retain no evidence. */
mod elision {
    /*
        Lifetimes: declaration-level check that a view-returning machine has an
        unambiguous source for the returned borrow, and that captured body origins
        belong to that declaration's lifetime relation. Formation owns current
        descriptor contents; source spelling cannot waive this relation.

        Explicit output lifetimes name matching input leaves; elided outputs need
        one unambiguous input. Type-dependent declarations are templates, not closed
        loan contracts: selected instances and closed bodyless calls must establish
        the complete relation. Declaration checks and caller loan attribution share
        `borrow::view_link` rather than deriving different structural frontiers.
    */
    use crate::borrow::returned_body_source_mismatch;
    use diagnostics::Diagnostic;

    use crate::borrow::view_link::{
        DeclarationLifetimeFrontier, ViewReturnAmbiguity, ViewReturnSource,
        declaration_lifetime_frontier, resolve_applied_signature_view_return_source,
        resolve_signature_view_return_source, resolve_view_return_source,
    };
    mod templates {
        /*
            An uninstantiated declaration frontier cannot silently produce no loan.
        */
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::machine::Machine;
        use typed_trees::signature::StateSignature;
        use typed_trees::state::State;
        use typed_trees::state::state_by_symbol;
        use typed_trees::types::TypeReferenceHandle;
        use typed_trees::{
            expression::{ExpressionHandle, StaticMachineArgument},
            statement::{StatementNode, TransitionTargetNode},
        };
        use validation::closed_static_call_type_bindings;
        use validation::expression_traversal::collect_statement_expression_trees;
        use validation::named_conformance_target_requirement;

        pub(super) fn check_calls(
            program: &dyn ::validation::CheckingInput,
            deferred: &[SymbolHandle],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if deferred.is_empty() {
                return;
            }
            let mut expressions = Vec::new();
            for machine in program.machines() {
                if program.callable_body_has_open_parameters(machine) {
                    continue;
                }
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        match statement {
                            StatementNode::Call(call) => {
                                check_target(
                                    program,
                                    machine,
                                    state,
                                    deferred,
                                    call.target_symbol,
                                    call.target.as_str(),
                                    &call.machine_arguments,
                                    program.statement_table.expression_handles(call.arguments),
                                    diagnostics,
                                );
                            }
                            StatementNode::Transition(transition) => {
                                for target in transition.targets() {
                                    if let TransitionTargetNode::Named {
                                        path, arguments, ..
                                    } = program.statement_table.transition_target(target)
                                    {
                                        let members =
                                            program.statement_table.name_path_members(path.members);
                                        let name =
                                            members.last().map(|name| name.as_str()).unwrap_or("");
                                        check_target(
                                            program,
                                            machine,
                                            state,
                                            deferred,
                                            path.symbol,
                                            name,
                                            &[],
                                            program.statement_table.expression_handles(*arguments),
                                            diagnostics,
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                        expressions.clear();
                        collect_statement_expression_trees(program, statement, &mut expressions);
                        for handle in &expressions {
                            let Some(call) = program.expression_table.call_node(*handle) else {
                                continue;
                            };
                            check_target(
                                program,
                                machine,
                                state,
                                deferred,
                                call.target_symbol,
                                call.target.as_str(),
                                &call.machine_arguments,
                                program.expression_table.expression_handles(call.arguments),
                                diagnostics,
                            );
                        }
                    }
                }
            }
        }

        fn check_target(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            deferred: &[SymbolHandle],
            target: SymbolHandle,
            name: &str,
            static_arguments: &[StaticMachineArgument],
            arguments: &[ExpressionHandle],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let unresolved_frontier = deferred.contains(&target)
                || program
                    .machine_parameter_signature_in(machine, target)
                    .is_some_and(|signature| deferred.contains(&signature.symbol))
                || {
                    let concrete = program.machines().iter().any(|candidate| {
                        candidate.symbol == target
                            || state_by_symbol(program.machine_states(candidate), target).is_some()
                    });
                    !concrete
                        && named_conformance_target_requirement(program, machine, target)
                            .ok()
                            .flatten()
                            .is_some_and(|requirement| {
                                deferred.contains(&requirement.signature.symbol)
                            })
                };
            if unresolved_frontier
                && !closed_return_frontier(
                    program,
                    machine,
                    state,
                    target,
                    static_arguments,
                    arguments,
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "call `{name}` in machine `{}` has a template-dependent returned-carrier lifetime frontier; a concrete checked callable is required before this call can produce or discard a value",
                    machine.name,
                )));
            }
        }

        /*
            A template-dependent returned-carrier frontier admits a call once the
            call's own static-callable substitution closes it: the instantiated
            frontier must be complete and the substituted result-to-input relation
            must resolve every returned view leaf to an exact caller source. A
            closed view-free result is the same check with an empty frontier;
            unresolved structure, unmatched leaves, and access escalation all keep
            the historical rejection.
        */
        fn closed_return_frontier(
            program: &dyn ::validation::CheckingInput,
            caller: &Machine,
            state: &State,
            target: SymbolHandle,
            static_arguments: &[StaticMachineArgument],
            arguments: &[ExpressionHandle],
        ) -> bool {
            if let Ok(Some(requirement)) =
                named_conformance_target_requirement(program, caller, target)
            {
                return complete_return_frontier(
                    program,
                    requirement.signature,
                    &requirement.application.substitutions,
                    &requirement.application.substitution_scope_starts,
                    caller.symbol,
                );
            }
            let signature = program
                .traits()
                .iter()
                .flat_map(|definition| program.trait_machine_instances(definition))
                .find(|signature| signature.symbol == target)
                .or_else(|| program.machine_parameter_signature_in(caller, target));
            let Some(signature) = signature else {
                return false;
            };
            let Some(substitutions) = closed_static_call_type_bindings(
                program,
                caller,
                state,
                signature,
                static_arguments,
                arguments,
            ) else {
                return false;
            };
            complete_return_frontier(program, signature, &substitutions, &[0], caller.symbol)
        }

        fn complete_return_frontier(
            program: &dyn ::validation::CheckingInput,
            signature: &StateSignature,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            scope_starts: &[usize],
            argument_owner: SymbolHandle,
        ) -> bool {
            if !crate::borrow::view_link::substituted_result_frontier_is_complete(
                program,
                signature.return_type,
                substitutions,
                scope_starts,
            ) {
                return false;
            }
            !matches!(
                crate::borrow::view_link::resolve_applied_signature_view_return_source(
                    program,
                    signature,
                    substitutions,
                    scope_starts,
                    argument_owner,
                ),
                crate::borrow::view_link::ViewReturnSource::Ambiguous(_)
            )
        }
    }

    pub(super) fn check_view_return_elision(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut deferred = Vec::new();
        for machine in program.machines() {
            let (scope, _) = program
                .machine_type_reference_scope(machine, |left, right| left.name == right.name);
            let parameters = scope
                .iter()
                .filter(|parameter| {
                    matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            for state in program.machine_states(machine) {
                if let Some(parameter_index) =
                    returned_body_source_mismatch(program, facts, state, call_frames)
                {
                    let parameter = &program.state_parameters(state)[parameter_index];
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` returns a view borrowing input `{}`, which is not permitted by the declared result lifetime relation",
                        super::escape::state_subject(machine, state),
                        parameter.name,
                    )));
                }
                if frontier_requires_application(
                    program,
                    state.return_type,
                    program.state_parameters(state),
                    &parameters,
                ) {
                    deferred.push(state.symbol);
                    continue;
                }
                let ViewReturnSource::Ambiguous(ambiguity) =
                    resolve_view_return_source(program, state)
                else {
                    continue;
                };

                let subject = if state.name.as_str() == "entry"
                    || machine.name.as_str().ends_with(state.name.as_str())
                {
                    machine.name.as_str()
                } else {
                    state.name.as_str()
                };

                diagnostics.push(Diagnostic::error(ambiguity_message(subject, &ambiguity)));
            }

            for parameter in program.machine_type_parameters(machine) {
                let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
                else {
                    continue;
                };
                let signature = program
                    .machine_parameter_contract_view(contract)
                    .expect(
                        "typed machine-parameter contract must retain a valid requirement identity",
                    )
                    .signature();
                check_signature_tree(
                    program,
                    signature,
                    &parameters,
                    &mut deferred,
                    diagnostics,
                    &mut Vec::new(),
                );
            }
        }

        for trait_definition in program.traits() {
            for signature in program.trait_machine_signatures(trait_definition) {
                let parameters = program
                    .trait_type_parameters(trait_definition)
                    .iter()
                    .filter(|parameter| {
                        matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                    })
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>();
                check_signature_tree(
                    program,
                    signature,
                    &parameters,
                    &mut deferred,
                    diagnostics,
                    &mut Vec::new(),
                );
            }
        }
        templates::check_calls(program, &deferred, diagnostics);
    }

    fn check_signature_tree(
        program: &dyn ::validation::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        captured_types: &[symbols::SymbolHandle],
        deferred: &mut Vec<symbols::SymbolHandle>,
        diagnostics: &mut Vec<Diagnostic>,
        active: &mut Vec<symbols::SymbolHandle>,
    ) {
        if active.contains(&signature.symbol) {
            diagnostics.push(Diagnostic::error(
                "unimplemented: recursive callable lifetime frontier requires cycle-aware applied telescopes",
            ));
            return;
        }
        active.push(signature.symbol);
        let parameters = program.state_signature_type_parameters(signature);
        let mut types = captured_types.to_vec();
        types.extend(parameters.iter().filter_map(|parameter| {
            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                .then_some(parameter.symbol)
        }));
        if !defer_signature_frontier(program, signature, &types, deferred, diagnostics) {
            check_bodyless_signature(program, signature, diagnostics);
        }
        for parameter in parameters {
            if let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
                && let Some(contract) = program.machine_parameter_contract_view(contract)
            {
                check_signature_tree(
                    program,
                    contract.signature(),
                    &types,
                    deferred,
                    diagnostics,
                    active,
                );
            }
        }
        active.pop();
    }

    fn defer_signature_frontier(
        program: &dyn ::validation::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        type_parameters: &[symbols::SymbolHandle],
        deferred: &mut Vec<symbols::SymbolHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        if !frontier_requires_application(
            program,
            signature.return_type,
            program.state_signature_parameters(signature),
            type_parameters,
        ) {
            return false;
        }
        if let ViewReturnSource::Ambiguous(ambiguity) = resolve_applied_signature_view_return_source(
            program,
            signature,
            &[],
            &[0],
            signature.symbol,
        ) && !matches!(ambiguity, ViewReturnAmbiguity::IncompleteStructure { .. })
        {
            diagnostics.push(Diagnostic::error(ambiguity_message(
                signature.name.as_str(),
                &ambiguity,
            )));
        }
        deferred.push(signature.symbol);
        true
    }

    fn frontier_requires_application(
        program: &dyn ::validation::CheckingInput,
        result: typed_trees::types::TypeReferenceHandle,
        parameters: &[typed_trees::signature::StateParameter],
        type_parameters: &[symbols::SymbolHandle],
    ) -> bool {
        let result_frontier = declaration_lifetime_frontier(program, result, type_parameters);
        let mut dependent = match result_frontier {
            DeclarationLifetimeFrontier::Incomplete
            | DeclarationLifetimeFrontier::Complete { has_borrows: false } => return false,
            DeclarationLifetimeFrontier::Complete { has_borrows: true } => false,
            DeclarationLifetimeFrontier::TemplateDependent { .. } => true,
        };
        for parameter in parameters {
            match declaration_lifetime_frontier(program, parameter.type_reference, type_parameters)
            {
                DeclarationLifetimeFrontier::Incomplete => return false,
                DeclarationLifetimeFrontier::TemplateDependent { .. } => dependent = true,
                DeclarationLifetimeFrontier::Complete { .. } => {}
            }
        }
        dependent
    }

    fn check_bodyless_signature(
        program: &dyn ::validation::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let ViewReturnSource::Ambiguous(ambiguity) = resolve_signature_view_return_source(
            program,
            program.state_signature_parameters(signature),
            signature.return_type,
        ) else {
            return;
        };

        diagnostics.push(Diagnostic::error(ambiguity_message(
            signature.name.as_str(),
            &ambiguity,
        )));
    }

    fn ambiguity_message(subject: &str, ambiguity: &ViewReturnAmbiguity) -> String {
        match ambiguity {
            ViewReturnAmbiguity::IncompleteStructure { subject: source } => format!(
                "machine `{subject}` returns a view whose {source} has an incomplete structural lifetime frontier"
            ),
            ViewReturnAmbiguity::IncompatibleSourceAccess { input } => format!(
                "machine `{subject}` returns a view whose access cannot be supplied by matching input `{input}`"
            ),
            ViewReturnAmbiguity::ElidedMultipleInputs { candidates } => format!(
                "machine `{subject}` returns a view but takes {} candidate ref inputs ({}); \
             cannot infer which input the returned view borrows — annotate the return type \
             with the lifetime of the input it borrows, e.g. `-> &'a T` matching an input `&'a T`",
                candidates.len(),
                quote_join(candidates),
            ),
            ViewReturnAmbiguity::LifetimeMatchesNoInput { lifetime } => format!(
                "machine `{subject}` returns a view with lifetime `'{lifetime}`, but no input \
             borrows `'{lifetime}`; annotate the input the view comes from, e.g. \
             `buffer: &'{lifetime} T`",
            ),
        }
    }

    fn quote_join(names: &[String]) -> String {
        names
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
mod escape {
    /*
        Normal returned views must denote storage that outlives this invocation.
        The borrow builder captures their leaves against the current descriptor
        roster, shared with local and store formation. An incoming referent is not
        its owned carrier's storage; a declared lifetime alone is not an origin.
        Locals and by-value inputs are both invocation-owned. Returning their stored
        external references is valid; returning a fresh borrow of their storage is not.
    */
    use checked_trees::CheckFacts;
    use diagnostics::Diagnostic;

    pub(super) fn check_view_return_escape(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (_, borrow_state) in facts.borrow.states.iter() {
            let Some((machine, state)) = validation::machine_state_by_symbols(
                program,
                borrow_state.machine_symbol,
                borrow_state.state_symbol,
            ) else {
                continue;
            };
            let mut reported: Vec<facts::PlaceRoot> = Vec::new();
            for loan in facts
                .borrow
                .returned_loans
                .span_or_empty(borrow_state.returned_loans)
            {
                if loan.incoming_reference.is_valid() || reported.contains(&loan.root) {
                    continue;
                }
                let storage = match loan.root {
                    facts::PlaceRoot::Symbol(root) => {
                        if let Some((_, local)) = program.state_local_declaration(state, root) {
                            format!("the local `{}`", local.name)
                        } else if let Some(parameter) =
                            program.state_parameters(state).iter().find(|parameter| {
                                parameter.symbol == root
                                    && !parameter.is_const
                                    && validation::reference_shell(
                                        program,
                                        parameter.type_reference,
                                    )
                                    .is_none()
                            })
                        {
                            format!("the owned input `{}`", parameter.name)
                        } else {
                            continue;
                        }
                    }
                    facts::PlaceRoot::Expression(expression) => {
                        let is_view = validation::expression_type_reference_in_state(
                            program,
                            state.symbol,
                            loan.statement_index,
                            expression,
                        )
                        .is_some_and(|reference| {
                            validation::reference_shell(program, reference).is_some()
                        });
                        let image_literal = loan.kind == checked_trees::BorrowAccessKind::Read
                            && matches!(
                                program.expression_table.expression(expression),
                                typed_trees::expression::ExpressionNode::String(_)
                            );
                        if (is_view || image_literal)
                            && super::persistent::is_state_independent_borrow_source(
                                program,
                                state,
                                expression,
                                &mut Vec::new(),
                            )
                        {
                            continue;
                        }
                        "temporary storage".to_owned()
                    }
                    facts::PlaceRoot::Unknown
                    | facts::PlaceRoot::TypeReference(_)
                    | facts::PlaceRoot::InvocationReferent { .. }
                    | facts::PlaceRoot::InvocationOutput(_) => {
                        diagnostics.push(Diagnostic::error(
                            "returned view has no captured storage origin",
                        ));
                        continue;
                    }
                };
                reported.push(loan.root);
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` returns a view borrowing {storage}, which does not outlive \
                 the call; return a view of storage that outlives the invocation instead",
                    state_subject(machine, state),
                )));
            }
        }
    }

    pub(super) fn state_subject<'a>(
        machine: &'a typed_trees::machine::Machine,
        state: &'a typed_trees::state::State,
    ) -> &'a str {
        if state.name.as_str() == "entry" || machine.name.as_str().ends_with(state.name.as_str()) {
            machine.name.as_str()
        } else {
            state.name.as_str()
        }
    }
}
mod persistent {
    /*
        Fail-closed fence for borrow-carrying values stored beyond one graph state.

        Local loan attribution is statement/state scoped today. A borrow backed by
        program-static storage needs no source loan, so literals and machine results
        whose every value exit is likewise static may be stored persistently. Stable
        fixed-index paths and runtime indexes named by immutable state parameters or
        locals preserve that provenance across named edges when the same symbol, or
        a direct immutable local-copy alias, is forwarded into an immutable target
        parameter. Non-static sources remain fenced until the flow plan propagates
        a persistent owner's loan through every outgoing transition.
    */
    use crate::borrow::view_link::borrow_carrying_owner_paths;
    use crate::borrow::view_link::{DeclarationLifetimeFrontier, declaration_lifetime_frontier};
    use crate::flow::expressions;
    use crate::flow::indexed_statements;
    use crate::published::places::place_at;
    use diagnostics::Diagnostic;
    use language_core::place_label;
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct StaticPersistentPath {
        field: SymbolHandle,
        segments: Vec<StaticPersistentSegment>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum StaticPersistentSegment {
        Field(SymbolHandle),
        Case(SymbolHandle),
        FixedIndex(usize),
        StableIndex(SymbolHandle),
    }

    #[derive(Debug, Clone, Copy)]
    struct StateTransitionEdge {
        target: SymbolHandle,
        arguments: arena::HandleSpan<typed_trees::expression::ExpressionHandle>,
    }

    pub(super) fn check_persistent_borrow_assignments(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut owned_call_frames = None;
        let call_frames =
            crate::flow::shared_call_frames_or(call_frames, program, &mut owned_call_frames);
        check_stored_output_relations(program, facts, call_frames, diagnostics);
        for machine in program.machines() {
            let persistent = persistent_storage(program, machine);
            if persistent.is_empty() {
                continue;
            }

            let states = program.machine_states(machine);
            let entry_paths = static_persistent_paths_at_state_entries(
                program,
                machine,
                &persistent,
                states,
                call_frames,
            );
            for (state, entry_paths) in states.iter().zip(entry_paths) {
                let (_, state_diagnostics) = analyze_persistent_state(
                    program,
                    machine,
                    state,
                    &persistent,
                    &entry_paths,
                    call_frames,
                    true,
                );
                diagnostics.extend(state_diagnostics);
            }
        }
    }

    /*
        Actual installed loans create the effect obligation. Merely accepting a
        mutable aggregate parameter does not: scalar writes and unrelated helpers
        must not acquire a new slot-summary requirement from their parameter type.
    */
    fn check_stored_output_relations(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        frames: Option<&validation::CallFrameResolver<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut obligated = Vec::new();
        for machine in program.machines() {
            let mut stores = false;
            'machine_states: for state in program.machine_states(machine) {
                let Some((_, borrowing)) = facts
                    .borrow
                    .first_state_borrow(machine.symbol, state.symbol)
                else {
                    continue;
                };
                if borrowing.loans.is_empty() {
                    continue;
                }
                for (_, loan) in facts.borrow.state_loans(borrowing) {
                    if facts.borrow.loan_owner_path(loan).is_empty()
                        || !program
                            .state_parameters(state)
                            .iter()
                            .any(|parameter| parameter.symbol == loan.owner_symbol)
                        || validation::loan_carrier_has_owned_storage(
                            program,
                            state.symbol,
                            loan.statement_index,
                            loan.owner_symbol,
                            facts.borrow.loan_owner_path(loan),
                        )
                    {
                        continue;
                    }
                    stores = true;
                    break 'machine_states;
                }
            }
            if stores {
                obligated.push(machine.symbol);
            }
        }
        for machine in program.machines() {
            if obligated.contains(&machine.symbol)
                && program.machine_states(machine).first().is_none_or(|entry| {
                    frames
                        .and_then(|frames| frames.state_stored_reference_effects(machine, entry))
                        .is_none()
                })
            {
                diagnostics.push(Diagnostic::error(format!("machine `{}` stores a borrowed value without a complete normal-exit backing relation", machine.name)));
            }
            for state in program.machine_states(machine) {
                let calls = facts
                    .flow
                    .control
                    .machine_states(machine.symbol)
                    .into_iter()
                    .filter(|flow| flow.state_symbol == state.symbol)
                    .flat_map(|flow| facts.flow.control.calls.span_or_empty(flow.calls));
                let calls = calls.collect::<Vec<_>>();
                for (statement_index, statement) in
                    crate::flow::state_statements(program, state.statement_nodes)
                        .iter()
                        .enumerate()
                {
                    let requires_relation = calls.iter().any(|call| {
                        if ::validation::trace_flags::struct_trace()
                            && call.statement_index == statement_index
                        {
                            eprintln!("STRUCT stored-output machine={} state={} statement={} resolver={} target={:?}", machine.name, state.name, statement_index, frames.is_some(), call.target_symbol);
                        }
                        call.statement_index == statement_index
                            && (frames.is_none_or(|frames| {
                                frames.provider_output_relation_required(
                                    machine.symbol,
                                    call.target_symbol,
                                    call.authored_expression,
                                )
                            }) || validation::call_target_state(program, call.target_symbol)
                                .and_then(|target| program.machine_holding_state(target.symbol))
                                .is_some_and(|callee| obligated.contains(&callee.symbol)))
                    });
                    if !requires_relation {
                        continue;
                    }
                    let failures = frames.and_then(|frames| {
                        frames.stored_reference_call_failures(machine, statement)
                    });
                    let incomplete = match failures {
                        Some(failures) => failures.iter().any(|failure| {
                            failure.requires_relation
                                || validation::call_target_state(program, failure.target)
                                    .and_then(|target| program.machine_holding_state(target.symbol))
                                    .is_some_and(|callee| obligated.contains(&callee.symbol))
                        }),
                        None => requires_relation,
                    };
                    if incomplete {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` state `{}` statement {statement_index}: call stores a borrowed value without a complete destination/backing relation",
                            machine.name, state.name,
                        )));
                    }
                }
            }
        }
    }

    fn static_persistent_paths_at_state_entries(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
        states: &[typed_trees::state::State],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Vec<Vec<StaticPersistentPath>> {
        let mut entries = vec![None::<Vec<StaticPersistentPath>>; states.len()];
        if !states.is_empty() {
            let mut entry = Vec::new();
            for (field, _, field_type) in attached_persistent_fields(program, machine) {
                if !crate::borrow::view_link::returns_borrow(program, field_type) {
                    continue;
                }
                let path = StaticPersistentPath {
                    field,
                    segments: Vec::new(),
                };
                entry.push(path.clone());
                add_static_borrow_frontier(program, path, field_type, &mut entry);
            }
            entries[0] = Some(entry);
        }

        loop {
            let mut changed = false;
            for (state_index, state) in states.iter().enumerate() {
                let Some(entry) = entries[state_index].clone() else {
                    continue;
                };
                let (exit, _) = analyze_persistent_state(
                    program,
                    machine,
                    state,
                    persistent,
                    &entry,
                    call_frames,
                    false,
                );
                for edge in state_transition_edges(program, state) {
                    let Some(target_index) = states
                        .iter()
                        .position(|candidate| candidate.symbol == edge.target)
                    else {
                        continue;
                    };
                    let exit = rebase_static_paths_for_transition(
                        program,
                        state,
                        &states[target_index],
                        edge.arguments,
                        &exit,
                    );
                    let merged = entries[target_index].as_ref().map_or_else(
                        || exit.clone(),
                        |prior| {
                            prior
                                .iter()
                                .filter(|&path| exit.contains(path))
                                .cloned()
                                .collect()
                        },
                    );
                    if entries[target_index].as_ref() != Some(&merged) {
                        entries[target_index] = Some(merged);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        entries
            .into_iter()
            .map(|entry| entry.unwrap_or_default())
            .collect()
    }

    fn state_transition_edges(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
    ) -> Vec<StateTransitionEdge> {
        crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .filter_map(|statement| statement.as_transition())
            .flat_map(|transition| transition.targets())
            .filter_map(|target| {
                let typed_trees::statement::TransitionTargetNode::Named {
                    path, arguments, ..
                } = program.statement_table.transition_target(target)
                else {
                    return None;
                };
                path.symbol.is_valid().then_some(StateTransitionEdge {
                    target: path.symbol,
                    arguments: *arguments,
                })
            })
            .collect()
    }

    fn rebase_static_paths_for_transition(
        program: &dyn ::validation::CheckingInput,
        source: &typed_trees::state::State,
        target: &typed_trees::state::State,
        arguments: arena::HandleSpan<typed_trees::expression::ExpressionHandle>,
        paths: &[StaticPersistentPath],
    ) -> Vec<StaticPersistentPath> {
        let arguments = program.statement_table.expression_handles(arguments);
        let target_parameters = program.state_parameters(target).iter().collect::<Vec<_>>();
        if arguments.len() != target_parameters.len() {
            return paths
                .iter()
                .filter(|path| {
                    !path
                        .segments
                        .iter()
                        .any(|segment| matches!(segment, StaticPersistentSegment::StableIndex(_)))
                })
                .cloned()
                .collect();
        }

        let mut rebased = Vec::new();
        for path in paths {
            let mut path = path.clone();
            let mut complete = true;
            for segment in &mut path.segments {
                let StaticPersistentSegment::StableIndex(source_symbol) = *segment else {
                    continue;
                };
                let replacement =
                    arguments
                        .iter()
                        .zip(&target_parameters)
                        .find_map(|(argument, parameter)| {
                            (!parameter.is_mutable
                                && expression_is_exact_stable_index(
                                    program,
                                    source,
                                    *argument,
                                    source_symbol,
                                ))
                            .then_some(parameter.symbol)
                        });
                let Some(replacement) = replacement else {
                    complete = false;
                    break;
                };
                *segment = StaticPersistentSegment::StableIndex(replacement);
            }
            if complete && !rebased.contains(&path) {
                rebased.push(path);
            }
        }
        rebased
    }

    fn expression_is_exact_stable_index(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
        symbol: SymbolHandle,
    ) -> bool {
        let Some(candidate) = exact_name_symbol(program, state, expression) else {
            return false;
        };
        stable_index_origin_symbol(program, state, symbol, &mut Vec::new())
            .zip(stable_index_origin_symbol(
                program,
                state,
                candidate,
                &mut Vec::new(),
            ))
            .is_some_and(|(source, candidate)| source == candidate)
    }

    fn stable_index_origin_symbol(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        symbol: SymbolHandle,
        visiting: &mut Vec<SymbolHandle>,
    ) -> Option<SymbolHandle> {
        if program
            .state_parameters(state)
            .iter()
            .any(|parameter| (!parameter.is_mutable) && parameter.symbol == symbol)
        {
            return Some(symbol);
        }
        let (_, local) = program
            .state_local_declaration(state, symbol)
            .filter(|(_, local)| !local.is_mutable)?;
        if visiting.contains(&symbol) {
            return None;
        }
        visiting.push(symbol);
        let origin = exact_name_symbol(program, state, local.initial_value)
            .and_then(|source| stable_index_origin_symbol(program, state, source, visiting))
            .unwrap_or(symbol);
        visiting.pop();
        Some(origin)
    }

    fn analyze_persistent_state(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
        entry_paths: &[StaticPersistentPath],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        report_diagnostics: bool,
    ) -> (Vec<StaticPersistentPath>, Vec<Diagnostic>) {
        let type_parameters = program
            .machine_type_parameters(machine)
            .iter()
            .filter(|parameter| {
                matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let mut static_persistent_places = Vec::new();
        let mut static_persistent_paths = entry_paths.to_vec();
        let mut diagnostics = Vec::new();
        for (statement_index, statement) in indexed_statements(program, state.statement_nodes) {
            if retain_static_paths_across_call_frame(
                program,
                state,
                persistent,
                &mut static_persistent_paths,
                &static_persistent_places,
                || {
                    call_frames.and_then(|frames| {
                        frames.statement_value_may_write_paths(machine, statement)
                    })
                },
            ) {
                static_persistent_places.clear();
            }

            if let Some(call) = statement.as_call() {
                if retain_static_paths_across_call_frame(
                    program,
                    state,
                    persistent,
                    &mut static_persistent_paths,
                    &static_persistent_places,
                    || call_frames.and_then(|frames| frames.may_write_paths(machine, call)),
                ) {
                    static_persistent_places.clear();
                }
                continue;
            }

            let Some(assignment) = statement.as_assignment() else {
                continue;
            };
            let Some(place) = place_at(program, state.symbol, statement_index, assignment.target)
            else {
                continue;
            };
            let target_field = persistent_field_and_tail(&place, persistent);
            let target_path = stable_persistent_path(program, state, &place, persistent);
            let Some((name, target_type)) =
                persistent_target_type(program, state.symbol, statement_index, &place, persistent)
            else {
                static_persistent_places.retain(|known| {
                    !static_provenance_invalidated_by_mutation(program, known, &place)
                });
                invalidate_static_persistent_paths(
                    &mut static_persistent_paths,
                    target_field,
                    target_path.as_ref(),
                );
                continue;
            };
            let has_borrows =
                match declaration_lifetime_frontier(program, target_type, &type_parameters) {
                    DeclarationLifetimeFrontier::Complete { has_borrows }
                    | DeclarationLifetimeFrontier::TemplateDependent { has_borrows } => has_borrows,
                    DeclarationLifetimeFrontier::Incomplete => true,
                };
            if !has_borrows {
                static_persistent_places.retain(|known| {
                    !static_provenance_invalidated_by_mutation(program, known, &place)
                });
                invalidate_static_persistent_paths(
                    &mut static_persistent_paths,
                    target_field,
                    target_path.as_ref(),
                );
                continue;
            }
            let initializers = crate::borrow::borrow_initializer_expressions(
                program,
                target_type,
                assignment.value,
            );
            let has_only_static_sources = if initializers.is_empty() {
                is_state_independent_borrow_source(
                    program,
                    state,
                    assignment.value,
                    &mut Vec::new(),
                )
            } else {
                initializers.into_iter().all(|initializer| {
                    is_state_independent_borrow_source(program, state, initializer, &mut Vec::new())
                })
            };
            let has_only_static_sources = has_only_static_sources
                || source_is_known_static_persistent_place(
                    program,
                    state.symbol,
                    statement_index,
                    assignment.value,
                    target_type,
                    &static_persistent_places,
                    &static_persistent_paths,
                    persistent,
                );

            static_persistent_places
                .retain(|known| !static_provenance_invalidated_by_mutation(program, known, &place));
            invalidate_static_persistent_paths(
                &mut static_persistent_paths,
                target_field,
                target_path.as_ref(),
            );
            if has_only_static_sources {
                static_persistent_places.push(place);
                if let Some(target_path) = target_path {
                    add_static_borrow_frontier(
                        program,
                        target_path,
                        target_type,
                        &mut static_persistent_paths,
                    );
                }
                continue;
            }

            if report_diagnostics {
                diagnostics.push(Diagnostic::error(format!(
                    "assignment stores a borrow-carrying value in persistent field `{name}` of \
                 machine `{}`; persistent loans must be propagated through graph-state \
                 transitions before this write can be admitted",
                    machine.name,
                )));
            }
        }
        (static_persistent_paths, diagnostics)
    }

    fn source_is_known_static_persistent_place(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        source_type: TypeReferenceHandle,
        known_static: &[validation::CanonicalPlace],
        known_static_paths: &[StaticPersistentPath],
        persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    ) -> bool {
        let Some(source) = place_at(program, state_symbol, statement_index, expression) else {
            return false;
        };

        let Some(state) = program.state_by_symbol(state_symbol) else {
            return false;
        };
        if known_static
            .iter()
            .any(|known| place_is_proven_prefix_of(program, state, known, &source))
        {
            return true;
        }

        let Some(borrow_paths) = borrow_carrying_owner_paths(program, source_type) else {
            return false;
        };
        !borrow_paths.is_empty()
            && borrow_paths.into_iter().all(|path| {
                let Some(segments) = owner_path_place_segments(&path) else {
                    return false;
                };
                let mut leaf = source.clone();
                leaf.extend_segments(&segments);
                known_static
                    .iter()
                    .any(|known| place_is_proven_prefix_of(program, state, known, &leaf))
                    || stable_persistent_path(program, state, &leaf, persistent).is_some_and(
                        |leaf| {
                            known_static_paths.iter().any(|known| {
                                known.field == leaf.field
                                    && known.segments.len() <= leaf.segments.len()
                                    && known
                                        .segments
                                        .iter()
                                        .zip(&leaf.segments)
                                        .all(|(left, right)| left == right)
                            })
                        },
                    )
            })
    }

    /*
        Return the persistent field selected by a canonical place and the segment
        index immediately below that field. Attached-data places carry the field as
        a segment below the state-local receiver root; machine-owned data can use
        the field symbol as the root directly.
    */
    fn persistent_field_and_tail(
        place: &validation::CanonicalPlace,
        persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    ) -> Option<(SymbolHandle, usize)> {
        if let Some(symbol) = place.root.as_symbol()
            && persistent
                .iter()
                .any(|(candidate, _, _)| *candidate == symbol)
        {
            return Some((symbol, 0));
        }

        place
            .segments
            .iter()
            .enumerate()
            .find_map(|(index, segment)| {
                let facts::PlaceSegment::Field { symbol } = segment else {
                    return None;
                };
                persistent
                    .iter()
                    .any(|(candidate, _, _)| candidate == symbol)
                    .then_some((*symbol, index + 1))
            })
    }

    fn stable_persistent_path(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        place: &validation::CanonicalPlace,
        persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    ) -> Option<StaticPersistentPath> {
        let (field, tail) = persistent_field_and_tail(place, persistent)?;
        let segments = place.segments[tail..]
            .iter()
            .map(|segment| static_persistent_segment(program, state, *segment))
            .collect::<Option<Vec<_>>>()?;
        Some(StaticPersistentPath { field, segments })
    }

    fn static_persistent_segment(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        segment: facts::PlaceSegment,
    ) -> Option<StaticPersistentSegment> {
        match segment {
            facts::PlaceSegment::Field { symbol } => Some(StaticPersistentSegment::Field(symbol)),
            facts::PlaceSegment::Case { variant } => Some(StaticPersistentSegment::Case(variant)),
            facts::PlaceSegment::FixedIndex { index } => {
                Some(StaticPersistentSegment::FixedIndex(index))
            }
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => None,
            facts::PlaceSegment::Index { expression } => {
                immutable_state_index_symbol(program, state, expression)
                    .map(StaticPersistentSegment::StableIndex)
            }
        }
    }

    fn immutable_state_index_symbol(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let symbol = exact_name_symbol(program, state, expression)?;
        let parameter_matches = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| (!parameter.is_mutable) && parameter.symbol == symbol);
        let local_matches = program
            .state_local_declaration(state, symbol)
            .filter(|(_, local)| !local.is_mutable)
            .map(|(_, local)| local.symbol);
        let mut matching = parameter_matches
            .map(|parameter| parameter.symbol)
            .chain(local_matches);
        let matched = matching.next()?;
        matching.next().is_none().then_some(matched)
    }

    fn exact_name_symbol(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(expression)
        else {
            return None;
        };
        let [name] = program.expression_table.name_path_members(path.members) else {
            return None;
        };
        if path.symbol.is_valid() {
            return Some(path.symbol);
        }
        crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .find_map(|statement| {
                let local = statement.as_local_data()?;
                (local.name == *name).then_some(local.symbol)
            })
    }

    fn invalidate_static_persistent_paths(
        paths: &mut Vec<StaticPersistentPath>,
        target_field: Option<(SymbolHandle, usize)>,
        target_path: Option<&StaticPersistentPath>,
    ) {
        let Some((field, _)) = target_field else {
            return;
        };
        paths.retain(|known| {
            if known.field != field {
                return true;
            }
            let Some(target) = target_path else {
                return false;
            };
            !static_persistent_segments_may_overlap(&known.segments, &target.segments)
        });
    }

    fn static_persistent_segments_may_overlap(
        left: &[StaticPersistentSegment],
        right: &[StaticPersistentSegment],
    ) -> bool {
        for (left, right) in left.iter().zip(right) {
            match (*left, *right) {
                (StaticPersistentSegment::Field(left), StaticPersistentSegment::Field(right))
                | (StaticPersistentSegment::Case(left), StaticPersistentSegment::Case(right))
                    if left != right =>
                {
                    return false;
                }
                (
                    StaticPersistentSegment::FixedIndex(left),
                    StaticPersistentSegment::FixedIndex(right),
                ) if left != right => return false,
                (
                    StaticPersistentSegment::StableIndex(left),
                    StaticPersistentSegment::StableIndex(right),
                ) if left == right => {}
                (StaticPersistentSegment::Field(_), StaticPersistentSegment::Field(_))
                | (StaticPersistentSegment::Case(_), StaticPersistentSegment::Case(_))
                | (
                    StaticPersistentSegment::FixedIndex(_),
                    StaticPersistentSegment::FixedIndex(_),
                )
                | (
                    StaticPersistentSegment::FixedIndex(_),
                    StaticPersistentSegment::StableIndex(_),
                )
                | (
                    StaticPersistentSegment::StableIndex(_),
                    StaticPersistentSegment::FixedIndex(_),
                )
                | (
                    StaticPersistentSegment::StableIndex(_),
                    StaticPersistentSegment::StableIndex(_),
                ) => {}
                _ => return true,
            }
        }
        true
    }

    fn add_static_borrow_frontier(
        program: &dyn ::validation::CheckingInput,
        target: StaticPersistentPath,
        target_type: TypeReferenceHandle,
        paths: &mut Vec<StaticPersistentPath>,
    ) {
        let Some(frontier) = borrow_carrying_owner_paths(program, target_type) else {
            return;
        };
        for owner_path in frontier {
            let Some(owner_segments) = owner_path_place_segments(&owner_path) else {
                continue;
            };
            let mut path = target.clone();
            path.segments
                .extend(owner_segments.into_iter().map(|segment| match segment {
                facts::PlaceSegment::Field { symbol } => StaticPersistentSegment::Field(symbol),
                facts::PlaceSegment::Case { variant } => StaticPersistentSegment::Case(variant),
                facts::PlaceSegment::FixedIndex { index } => StaticPersistentSegment::FixedIndex(index),
                facts::PlaceSegment::FixedRange { .. }
                | facts::PlaceSegment::OpaqueRange
                | facts::PlaceSegment::Index { .. } => {
                    unreachable!(
                        "range/dynamic borrow-owner paths are rejected before persistent propagation"
                    )
                }
            }));
            crate::shared::collections::push_unique(&mut *paths, path);
        }
    }

    /*
        Apply one complete R5 may-write frame to the stable provenance paths.
        Returns whether existing state-local canonical markers must retire as well.
        Resolve the frame only when either provenance frontier needs invalidation.
    */
    fn retain_static_paths_across_call_frame(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
        paths: &mut Vec<StaticPersistentPath>,
        local_places: &[validation::CanonicalPlace],
        written: impl FnOnce() -> Option<Vec<String>>,
    ) -> bool {
        if paths.is_empty() && local_places.is_empty() {
            return false;
        }
        let Some(written) = written() else {
            paths.clear();
            return true;
        };
        if written.is_empty() {
            return false;
        }
        paths.retain(|path| {
            let aliases = static_path_frame_aliases(program, state, persistent, path);
            !aliases.is_empty()
                && written.iter().all(|written| {
                    aliases
                        .iter()
                        .all(|path| !validation::frame_paths_overlap(path, written))
                })
        });
        true
    }

    fn static_path_frame_aliases(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
        path: &StaticPersistentPath,
    ) -> Vec<String> {
        let Some((_, root_name, _)) = persistent.iter().find(|(field, _, _)| *field == path.field)
        else {
            return Vec::new();
        };
        let mut suffix = String::new();
        for segment in &path.segments {
            match *segment {
                StaticPersistentSegment::Field(symbol) => {
                    let Some(name) = crate::checks::multiplicity::data_field_name(program, symbol)
                    else {
                        break;
                    };
                    suffix.push('.');
                    suffix.push_str(name);
                }
                StaticPersistentSegment::FixedIndex(index) => {
                    suffix.push('[');
                    suffix.push_str(&index.to_string());
                    suffix.push(']');
                }
                StaticPersistentSegment::StableIndex(symbol) => {
                    let parameter_name =
                        program
                            .state_parameters(state)
                            .iter()
                            .find_map(|parameter| {
                                (parameter.symbol == symbol).then_some(parameter.name.as_str())
                            });
                    let local_name = program
                        .state_local_declaration(state, symbol)
                        .map(|(_, local)| local.name.as_str());
                    let Some(name) = parameter_name.or(local_name) else {
                        return Vec::new();
                    };
                    suffix.push('[');
                    suffix.push_str(name);
                    suffix.push(']');
                }
                StaticPersistentSegment::Case(_) => break,
            }
        }
        let field_path = format!("{root_name}{suffix}");
        let rooted_path = program
            .machine_holding_state(state.symbol)
            .and_then(|machine| program.state_data_binding(machine, state))
            .map(|root| place_label(root.as_str(), &field_path));
        match rooted_path {
            Some(rooted_path) => vec![field_path, rooted_path],
            None => vec![field_path],
        }
    }

    fn owner_path_place_segments(
        path: &[crate::borrow::BorrowOwnerSegment],
    ) -> Option<Vec<facts::PlaceSegment>> {
        path.iter()
            .map(|segment| match segment {
                crate::borrow::BorrowOwnerSegment::Field(symbol) => {
                    Some(facts::PlaceSegment::Field { symbol: *symbol })
                }
                crate::borrow::BorrowOwnerSegment::Case(variant) => {
                    Some(facts::PlaceSegment::Case { variant: *variant })
                }
                crate::borrow::BorrowOwnerSegment::FixedIndex(index) => {
                    Some(facts::PlaceSegment::FixedIndex { index: *index })
                }
                crate::borrow::BorrowOwnerSegment::DynamicIndex => None,
            })
            .collect()
    }

    fn static_provenance_invalidated_by_mutation(
        program: &dyn ::validation::CheckingInput,
        known: &validation::CanonicalPlace,
        mutated: &validation::CanonicalPlace,
    ) -> bool {
        if known.root == mutated.root
            && crate::flow::canonical_place_segments_may_overlap(
                program,
                &known.segments,
                &mutated.segments,
            )
        {
            return true;
        }

        known.segments.iter().any(|segment| {
            let facts::PlaceSegment::Index { expression } = segment else {
                return false;
            };
            validation::canonical_place_from_expression(program, *expression).is_some_and(
                |dependency| {
                    dependency.root == mutated.root
                        && crate::flow::canonical_place_segments_may_overlap(
                            program,
                            &dependency.segments,
                            &mutated.segments,
                        )
                },
            )
        })
    }

    fn place_is_proven_prefix_of(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        prefix: &validation::CanonicalPlace,
        place: &validation::CanonicalPlace,
    ) -> bool {
        prefix.root == place.root
            && prefix.segments.len() <= place.segments.len()
            && prefix
                .segments
                .iter()
                .zip(&place.segments)
                .all(|(left, right)| place_segments_proven_equal(program, state, *left, *right))
    }

    fn place_segments_proven_equal(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        left: facts::PlaceSegment,
        right: facts::PlaceSegment,
    ) -> bool {
        match (left, right) {
            (
                facts::PlaceSegment::Index {
                    expression: left_expression,
                },
                facts::PlaceSegment::Index {
                    expression: right_expression,
                },
            ) => {
                if let (Some(left), Some(right)) = (
                    expressions(program).constant_integer_value(left_expression),
                    expressions(program).constant_integer_value(right_expression),
                ) {
                    return left == right;
                }
                immutable_local_index_symbol(program, state, left_expression).is_some_and(|left| {
                    immutable_local_index_symbol(program, state, right_expression)
                        .is_some_and(|right| left == right)
                })
            }
            _ => crate::flow::canonical_place_segments_equal(program, left, right),
        }
    }

    fn immutable_local_index_symbol(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(expression)
        else {
            return None;
        };
        let members = program.expression_table.name_path_members(path.members);
        let [name] = members else {
            return None;
        };
        let resolved = path
            .head_symbol
            .is_valid()
            .then_some(path.head_symbol)
            .or_else(|| path.symbol.is_valid().then_some(path.symbol));
        let mut matches = crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .filter_map(|statement| {
                let local = statement.as_local_data()?;
                (!local.is_mutable
                    && resolved
                        .map(|symbol| local.symbol == symbol)
                        .unwrap_or_else(|| local.name == *name))
                .then_some(local.symbol)
            });
        let symbol = matches.next()?;
        matches.next().is_none().then_some(symbol)
    }

    /*
        Whether a value's loans all outlive every state, so storing it in a
        persistent field borrows nothing the machine could invalidate. `state` is
        the state the expression is written in, which an immutable local's name
        needs to reach its initializer; `resolving` stops a chain of locals from
        revisiting one.
    */
    pub(super) fn is_state_independent_borrow_source(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
        resolving: &mut Vec<SymbolHandle>,
    ) -> bool {
        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::SavedSubject(saved) => {
                program.saved_value_source(saved).is_some_and(|source| {
                    is_state_independent_borrow_source(program, state, source, resolving)
                })
            }
            typed_trees::expression::ExpressionNode::String(_) => true,
            typed_trees::expression::ExpressionNode::Cast(cast) => {
                is_state_independent_borrow_source(program, state, cast.value, resolving)
            }
            typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                validation::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    state.statement_nodes.count() as usize,
                    borrow.target,
                )
                .is_some_and(|reference| validation::reference_shell(program, reference).is_some())
                    && is_state_independent_borrow_source(program, state, borrow.target, resolving)
            }
            typed_trees::expression::ExpressionNode::Binary(binary) => {
                is_state_independent_borrow_source(program, state, binary.left, resolving)
                    && is_state_independent_borrow_source(program, state, binary.right, resolving)
            }
            typed_trees::expression::ExpressionNode::Call(call) => {
                let Some(callee) = program.state_by_symbol(call.target_symbol) else {
                    return false;
                };
                state_returns_only_static_borrows(program, callee, resolving)
            }
            typed_trees::expression::ExpressionNode::Name(path) => {
                if !path.symbol.is_valid() || resolving.contains(&path.symbol) {
                    return false;
                }
                let Some(initial_value) = program
                    .state_local_declaration(state, path.symbol)
                    .filter(|(_, local_data)| !local_data.is_mutable)
                    .map(|(_, local_data)| local_data.initial_value)
                else {
                    return false;
                };
                resolving.push(path.symbol);
                let is_static =
                    is_state_independent_borrow_source(program, state, initial_value, resolving);
                resolving.pop();
                is_static
            }
            typed_trees::expression::ExpressionNode::ArrayLiteral(_)
            | typed_trees::expression::ExpressionNode::Match(_)
            | typed_trees::expression::ExpressionNode::Atomic(_)
            | typed_trees::expression::ExpressionNode::Boolean(_)
            | typed_trees::expression::ExpressionNode::Float(_)
            | typed_trees::expression::ExpressionNode::UndeterminedNaN(_)
            | typed_trees::expression::ExpressionNode::Indexed(_)
            | typed_trees::expression::ExpressionNode::Integer(_)
            | typed_trees::expression::ExpressionNode::Member(_)
            | typed_trees::expression::ExpressionNode::Range(_)
            | typed_trees::expression::ExpressionNode::StructLiteral(_)
            | typed_trees::expression::ExpressionNode::Unary(_)
            | typed_trees::expression::ExpressionNode::TypeExpression(_)
            | typed_trees::expression::ExpressionNode::ZeroValue(_) => false,
        }
    }

    fn state_returns_only_static_borrows(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        visiting: &mut Vec<SymbolHandle>,
    ) -> bool {
        if visiting.contains(&state.symbol) {
            return false;
        }
        visiting.push(state.symbol);

        let mut found_value_exit = false;
        let statements = program.statement_table.statements(state.statement_nodes);
        let expression_exit = statements.last().and_then(|statement| match statement {
            typed_trees::statement::StatementNode::Expression(expression) => Some(*expression),
            _ => None,
        });
        let expression_is_static = expression_exit.is_none_or(|expression| {
            found_value_exit = true;
            is_state_independent_borrow_source(program, state, expression, visiting)
        });
        let all_static = crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .filter_map(|statement| {
                let transition = statement.as_transition()?;
                Some([transition.target, transition.continuation])
            })
            .flatten()
            .filter(|target| target.is_valid())
            .all(
                |target| match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        found_value_exit = true;
                        is_state_independent_borrow_source(program, state, *expression, visiting)
                    }
                    typed_trees::statement::TransitionTargetNode::Named { path, .. } => {
                        let Some(target_state) = program.state_by_symbol(path.symbol) else {
                            return false;
                        };
                        let is_static =
                            state_returns_only_static_borrows(program, target_state, visiting);
                        found_value_exit |= is_static;
                        is_static
                    }
                    typed_trees::statement::TransitionTargetNode::Terminal => true,
                },
            );

        visiting.pop();
        expression_is_static && all_static && found_value_exit
    }

    fn persistent_target_type<'program>(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        place: &validation::CanonicalPlace,
        persistent: &[(SymbolHandle, &'program str, TypeReferenceHandle)],
    ) -> Option<(&'program str, TypeReferenceHandle)> {
        let name = persistent
            .iter()
            .find(|(candidate, _, _)| Some(*candidate) == place.root.as_symbol())
            .map(|(_, name, _)| *name)
            .or_else(|| {
                place.segments.iter().find_map(|segment| {
                    let facts::PlaceSegment::Field { symbol } = segment else {
                        return None;
                    };
                    persistent
                        .iter()
                        .find_map(|(candidate, name, _)| (candidate == symbol).then_some(*name))
                })
            })?;
        let target_type =
            validation::canonical_place_type_reference(program, state, statement_index, place)?;
        Some((name, target_type))
    }

    fn attached_persistent_fields<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
    ) -> Vec<(SymbolHandle, &'program str, TypeReferenceHandle)> {
        machine
            .attached_data
            .as_ref()
            .and_then(|name| program.data_definition_by_name(name))
            .into_iter()
            .flat_map(|definition| program.data_members(definition).iter())
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => {
                    Some((field.symbol, field.name.as_str(), field.type_reference))
                }
                typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect()
    }

    fn persistent_storage<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
    ) -> Vec<(SymbolHandle, &'program str, TypeReferenceHandle)> {
        attached_persistent_fields(program, machine)
            .into_iter()
            .chain(
                program
                    .machine_owned_data(machine)
                    .iter()
                    .map(|owned| (owned.symbol, owned.name.as_str(), owned.type_reference)),
            )
            .collect()
    }
}

/*
    Call, aggregate formation and statement judgments, plus resource rows
    consumed by lowering.
*/
mod aggregates {
    /*
        An aggregate value formed in one evaluation holds every reference leaf at
        once, just as a call holds every operand, so its leaves face the same
        simultaneous-hold judgment (`simultaneous`). A `let` or a store forms its
        owner's leaves at one statement and records them as that statement's
        loans of that owner. A returning state forms its result at the return;
        the borrow builder records those leaves separately from local activations,
        against the current descriptor roster at the actual return statement.

        Two loans are leaves of one formed value when they share a statement and
        an owner, sit at different positions of the owner, and come from different
        source expressions. Loans at one position are alternatives rather than
        simultaneous holds: a `match` lends each arm's target at the whole value,
        and a source local holding several loans rebases a leaf onto each
        candidate. Loans sharing a source expression come from one call result or
        moved aggregate, whose own formation was judged where it happened. A store
        through a reference with several candidate destinations repeats each leaf
        per destination with unchanged places, so each pair of source expressions
        is reported once.

        A saved result can also keep the helper's original inputs unavailable.
        Those restrictions are checked against other references in the aggregate;
        they are not additional pointers. Views from the same checked call may
        use their separate referents while outside access keeps those inputs held.
    */
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::{
        BorrowAccessKind, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts, FlowStateFact,
        StateBorrowFact,
    };
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;

    use crate::published::labels::symbol_name;

    use super::overlap::StatedOrderingPremise;
    use super::simultaneous::{Hold, simultaneous_hold_conflict};

    /* One reference leaf of a formed value, borrowed from its loan record. */
    struct FormedLeaf<'a> {
        loan: arena::Handle<checked_trees::BorrowLoanFact>,
        lineage: &'a checked_trees::BorrowLoanLineage,
        retained_operand: Option<checked_trees::BorrowRetainedOperand>,
        incoming_reference: arena::Handle<checked_trees::IncomingReferenceIdentity>,
        incoming_owner_path: &'a [facts::PlaceSegment],
        owner_symbol: SymbolHandle,
        formation: ExpressionHandle,
        position: &'a [BorrowLoanOwnerSegment],
        root: facts::PlaceRoot,
        segments: &'a [facts::PlaceSegment],
        kind: &'a BorrowAccessKind,
    }

    impl FormedLeaf<'_> {
        fn formed_with(&self, other: &Self) -> bool {
            self.owner_symbol == other.owner_symbol
                && self.formation != other.formation
                && self.position != other.position
        }

        fn place(&self) -> CapturedPlace {
            CapturedPlace {
                incoming_reference: self.incoming_reference,
                incoming_owner_path: self.incoming_owner_path.to_vec(),
                root: self.root,
                segments: self.segments.to_vec(),
            }
        }

        fn label(&self, program: &dyn ::validation::CheckingInput) -> String {
            facts::canonical_place_label_from_parts(program, self.root, self.segments)
        }
    }

    pub(super) fn check_formed_aggregates<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        borrow_state: &StateBorrowFact,
        stated_premises: &[StatedOrderingPremise],
        guarantees: &mut crate::checks::contracts::prover::call_guarantees::StateGuarantees<'_, 'p>,
        diagnostics: &mut Vec<Diagnostic>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) {
        let mut offset = 0u32;
        for formed in facts
            .borrow
            .loans
            .span_or_empty(borrow_state.loans)
            .chunk_by(|left, right| left.statement_index == right.statement_index)
        {
            let start = offset;
            offset += formed.len() as u32;
            if formed.len() < 2 {
                continue;
            }
            let leaves: Vec<FormedLeaf<'_>> = formed
                .iter()
                .enumerate()
                .map(|(index, loan)| FormedLeaf {
                    loan: borrow_state.loans.handle_at(start + index as u32),
                    lineage: &loan.lineage,
                    retained_operand: loan.retained_operand,
                    incoming_reference: loan.incoming_reference,
                    incoming_owner_path: facts
                        .borrow
                        .access_segments
                        .span_or_empty(loan.incoming_owner_path),
                    owner_symbol: loan.owner_symbol,
                    formation: loan.formation_expression,
                    position: facts.borrow.loan_owner_path(loan),
                    root: loan.root,
                    segments: facts.borrow.loan_segments(loan),
                    kind: &loan.kind,
                })
                .collect();
            check_formed_leaves(
                program,
                facts,
                formed[0].statement_index,
                &leaves,
                |owner| format!("`{}` holds", symbol_name(program, owner)),
                stated_premises,
                guarantees,
                diagnostics,
                bound_lookup,
            );
        }

        let Some((machine, state)) = validation::machine_state_by_symbols(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        ) else {
            return;
        };
        for loans in facts
            .borrow
            .returned_loans
            .span_or_empty(borrow_state.returned_loans)
            .chunk_by(|left, right| {
                left.statement_index == right.statement_index
                    && left.transition_target == right.transition_target
            })
        {
            if loans.len() < 2 {
                continue;
            }
            let leaves: Vec<FormedLeaf<'_>> = loans
                .iter()
                .map(|loan| FormedLeaf {
                    loan: arena::Handle::invalid(),
                    lineage: &loan.lineage,
                    retained_operand: loan.retained_operand,
                    incoming_reference: loan.incoming_reference,
                    incoming_owner_path: facts
                        .borrow
                        .access_segments
                        .span_or_empty(loan.incoming_owner_path),
                    owner_symbol: state.symbol,
                    formation: loan.formation_expression,
                    position: facts.borrow.owner_segments.span_or_empty(loan.owner_path),
                    root: loan.root,
                    segments: facts.borrow.access_segments.span_or_empty(loan.segments),
                    kind: &loan.kind,
                })
                .collect();
            let subject = super::escape::state_subject(machine, state);
            check_formed_leaves(
                program,
                facts,
                loans[0].statement_index,
                &leaves,
                |_| format!("machine `{subject}` returns"),
                stated_premises,
                guarantees,
                diagnostics,
                bound_lookup,
            );
        }
    }

    /*
        Judge every pair of leaves formed together at one statement. `subject`
        renders the diagnostic's subject and verb for the leaves' owner.
    */
    fn check_formed_leaves<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        statement_index: usize,
        leaves: &[FormedLeaf<'_>],
        subject: impl Fn(SymbolHandle) -> String,
        stated_premises: &[StatedOrderingPremise],
        guarantees: &mut crate::checks::contracts::prover::call_guarantees::StateGuarantees<'_, 'p>,
        diagnostics: &mut Vec<Diagnostic>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) {
        let mut premises: Option<Vec<StatedOrderingPremise>> = None;
        let mut reported: Vec<(ExpressionHandle, ExpressionHandle)> = Vec::new();
        for (index, first) in leaves.iter().enumerate() {
            for second in &leaves[index + 1..] {
                if !first.formed_with(second)
                    || reported.contains(&(first.formation, second.formation))
                    || reported.contains(&(second.formation, first.formation))
                {
                    continue;
                }
                let premises = premises.get_or_insert_with(|| {
                    let mut available = stated_premises.to_vec();
                    super::overlap::append_statement_premises(
                        program,
                        facts,
                        statement_index,
                        guarantees,
                        &mut available,
                        bound_lookup,
                    );
                    available
                });
                let first_debt = lifetime_windows(&facts.borrow, first, leaves);
                let second_debt = lifetime_windows(&facts.borrow, second, leaves);
                if let Some(Err(reason)) = first_debt
                    .as_ref()
                    .filter(|windows| windows.is_err())
                    .or_else(|| second_debt.as_ref().filter(|windows| windows.is_err()))
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{} a reference whose retained input windows are incomplete: {reason}",
                        subject(first.owner_symbol)
                    )));
                    continue;
                }
                if first_debt.is_some() && second_debt.is_some() {
                    continue;
                }
                if let Some((debt, held, windows)) = first_debt
                    .as_ref()
                    .map(|windows| (first, second, windows))
                    .or_else(|| second_debt.as_ref().map(|windows| (second, first, windows)))
                {
                    let windows = match windows {
                        Ok(windows) => windows,
                        Err(reason) => {
                            diagnostics.push(Diagnostic::error(format!(
                                "{} a reference whose retained input windows are incomplete: {reason}",
                                subject(first.owner_symbol)
                            )));
                            continue;
                        }
                    };
                    if shared_result_access(
                        program,
                        facts,
                        debt,
                        held,
                        leaves,
                        premises,
                        bound_lookup,
                    ) {
                        continue;
                    }
                    for window in windows {
                        if let Some(conflict) = simultaneous_hold_conflict(
                            program,
                            &facts.borrow,
                            &window.place,
                            &window.access,
                            &held.place(),
                            held.kind,
                            premises,
                            bound_lookup,
                        ) {
                            diagnostics.push(Diagnostic::error(format!(
                                "{} {}",
                                subject(first.owner_symbol),
                                conflict.phrase(
                                    |hold| match hold {
                                        Hold::First => facts::canonical_place_label_from_parts(
                                            program,
                                            window.place.root,
                                            &window.place.segments
                                        ),
                                        Hold::Second => held.label(program),
                                    },
                                    "another of its references"
                                )
                            )));
                            reported.push((first.formation, second.formation));
                            break;
                        }
                    }
                    continue;
                }
                let Some(conflict) = simultaneous_hold_conflict(
                    program,
                    &facts.borrow,
                    &first.place(),
                    first.kind,
                    &second.place(),
                    second.kind,
                    premises,
                    bound_lookup,
                ) else {
                    continue;
                };
                reported.push((first.formation, second.formation));
                let label = |hold| match hold {
                    Hold::First => first.label(program),
                    Hold::Second => second.label(program),
                };
                diagnostics.push(Diagnostic::error(format!(
                    "{} {}",
                    subject(first.owner_symbol),
                    conflict.phrase(label, "another of its references"),
                )));
            }
        }
    }

    fn lifetime_windows(
        borrow: &checked_trees::BorrowFacts,
        debt: &FormedLeaf<'_>,
        leaves: &[FormedLeaf<'_>],
    ) -> Option<Result<Vec<super::resources::supplied_permissions::HeldInput>, &'static str>> {
        if *debt.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency {
            return None;
        }
        if debt.loan.is_valid()
            && let Some(windows) =
                super::resources::supplied_permissions::debt_windows(borrow, debt.loan)
        {
            return Some(windows);
        }
        let mut windows = Vec::new();
        let mut completed = false;
        let mut other = false;
        for peer in leaves.iter().filter(|peer| {
            *peer.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                && peer.owner_symbol == debt.owner_symbol
                && peer.position == debt.position
                && peer.formation == debt.formation
        }) {
            if let Some(found) = super::resources::supplied_permissions::output_windows(
                borrow,
                peer.incoming_reference,
            ) {
                completed = true;
                match found {
                    Ok(found) => windows.extend(found),
                    Err(reason) => return Some(Err(reason)),
                }
            } else {
                other = true;
            }
        }
        if completed {
            return Some(if other {
                Err("the returned value has unjoined input generations")
            } else {
                Ok(windows)
            });
        }
        Some(Ok(vec![
            super::resources::supplied_permissions::HeldInput {
                place: debt.place(),
                access: debt.kind.clone(),
            },
        ]))
    }

    fn shared_result_access<'p>(
        program: &'p dyn validation::CheckingInput,
        facts: &CheckFacts,
        debt: &FormedLeaf<'_>,
        held: &FormedLeaf<'_>,
        leaves: &[FormedLeaf<'_>],
        premises: &[StatedOrderingPremise],
        lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> bool {
        let Some(retained) = debt
            .retained_operand
            .filter(|retained| retained.formation_expression.is_valid())
        else {
            return false;
        };
        if held.retained_operand != Some(retained) {
            return false;
        }
        let peers = leaves
            .iter()
            .filter(|peer| {
                *peer.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                    && peer.owner_symbol == debt.owner_symbol
                    && peer.position == debt.position
                    && peer.formation == debt.formation
            })
            .collect::<Vec<_>>();
        !peers.is_empty()
            && peers.iter().all(|peer| {
                peer.retained_operand == Some(retained)
                    && super::overlap::captured_place_compatibility(
                        program,
                        &facts.borrow,
                        &peer.place(),
                        peer.kind,
                        &held.place(),
                        held.kind,
                        premises,
                        lookup,
                    )
                    .non_interfering
            })
    }
}
mod captured_descriptors {
    /*
        Join the invocation's captured operands to their physical backing.
        The shared completed-storage query owns descriptor/enclosure geometry;
        this borrow-checking adapter alone supplies checked completion-time loans.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use arena::Handle;
    use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};
    use facts::PlaceSegment;

    mod loan_origins {
        /*
            Resolve compatibility geometry from the loans at operand completion.
            Recasts and placed results can establish exact local backing without a
            descriptor supplier row. The shared access resolver follows those checked
            origins and refuses unresolved reference endpoints. Lifetime dependencies
            alone identify no pointer, and geometry never grants ancestry or access.
            Incoming parameters retain their separately admitted joint namespace.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use crate::published::places::place_at;
        use arena::Handle;
        use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};

        pub(super) fn places(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            operand: Handle<FlowOperandFact>,
            path: &[facts::PlaceSegment],
        ) -> Option<Vec<CapturedPlace>> {
            let control = &facts.flow.control;
            if !control.operands.is_valid(operand) {
                return None;
            }
            let capture = control.operands.get(operand);
            match validation::completed_storage::frozen_reference_leaf(
                program, control, operand, path,
            ) {
                validation::completed_storage::FrozenReferenceSelection::Unavailable => None,
                validation::completed_storage::FrozenReferenceSelection::Endpoint {
                    owner,
                    leaf,
                    path,
                } => captured_places(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    &owner,
                    &path,
                    leaf.constraints,
                ),
                validation::completed_storage::FrozenReferenceSelection::NotFrozen
                | validation::completed_storage::FrozenReferenceSelection::ThroughReference => {
                    captured_places(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        capture,
                        path,
                        capture.constraints,
                    )
                }
            }
        }

        fn captured_places(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            capture: &FlowOperandFact,
            path: &[facts::PlaceSegment],
            constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
        ) -> Option<Vec<CapturedPlace>> {
            if capture.state_symbol != state.state_symbol {
                return None;
            }
            let mut source = place_at(
                program,
                capture.state_symbol,
                capture.statement_index,
                capture.expression,
            )?;
            source.segments.extend_from_slice(path);
            let resolved = super::super::resources::retained_backing::retained_places(
                program,
                facts,
                loan_occurrences,
                state,
                capture.statement_index,
                constraints,
                CapturedPlace {
                    root: source.root,
                    segments: source.segments,
                    ..Default::default()
                },
                &mut Vec::new(),
            );
            let resolved = resolved?;
            if resolved.is_empty() {
                return None;
            }
            resolved
                .into_iter()
                .map(|source| {
                    if source.lineage.iter().any(|loan| {
                        facts.borrow.loans.get(*loan).lineage
                            == checked_trees::BorrowLoanLineage::LifetimeDependency
                    }) {
                        return None;
                    }
                    if source.place.incoming_reference.is_valid() {
                        if !facts
                            .borrow
                            .incoming_references
                            .is_valid(source.place.incoming_reference)
                        {
                            return None;
                        }
                        let incoming = facts
                            .borrow
                            .incoming_references
                            .get(source.place.incoming_reference);
                        if incoming.machine_symbol != state.machine_symbol
                            || incoming.state_symbol != capture.state_symbol
                            || !matches!(
                                incoming.origin,
                                checked_trees::IncomingReferenceOrigin::Parameter { .. }
                            )
                        {
                            return None;
                        }
                    }
                    Some(source.place)
                })
                .collect()
        }
    }

    pub(super) fn places(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        operand: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
    ) -> Option<Vec<CapturedPlace>> {
        validation::completed_storage::reference_backing_places(
            program,
            &facts.flow.control,
            operand,
            path,
            &mut |operand, path| {
                loan_origins::places(program, facts, loan_occurrences, state, operand, path)
            },
        )
    }
}
mod incoming_backing {
    /*
        A helper may keep a saved pointer after overwriting the field it came from.
        Prove that the write cannot touch storage protected by the saved reference,
        using each caller's actual pointers and write destination. If the caller
        also needs its caller to prove this, carry the same exact requirement outward.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use arena::Handle;
    use checked_trees::{
        BorrowAccessKind, BorrowLoanFact, CapturedPlace, CheckFacts, FlowCallFact, FlowOperandFact,
        FlowStateFact, IncomingReferenceOrigin,
    };
    use facts::PlaceSegment;
    use symbols::SymbolHandle;
    use validation::CanonicalPlace;

    use super::aliases::ResolvedAlias;

    mod callers {
        /*
            Check every call: its write must not conflict with the saved reference.
            If the arguments still depend on the caller's inputs, check its callers in
            turn. A recursive call cycle cannot supply its own proof.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use checked_trees::{BorrowAccessKind, CheckFacts};
        use symbols::SymbolHandle;

        use super::sources::{
            actual_for, actual_storage, collect_backings, input_backing, input_storage, state_flow,
        };
        use super::{
            Backing, IncomingBackingCalls, IncomingCallerRoster, Invocation, Requirement, Storage,
        };

        impl<'facts> IncomingBackingCalls<'facts> {
            pub(in crate::checks::borrows) fn new(
                program: &dyn validation::CheckingInput,
                facts: &'facts CheckFacts,
                external_entries: &[SymbolHandle],
            ) -> Self {
                let mut callers: Vec<Vec<IncomingCallerRoster<'facts>>> = Vec::new();
                for (_, state) in facts.flow.control.states.iter() {
                    for call in facts.flow.control.calls.span_or_empty(state.calls) {
                        let target =
                            validation::call_target_entry_symbol(program, call.target_symbol);
                        let position = target.arena_index() as usize;
                        if callers.len() <= position {
                            callers.resize_with(position + 1, Vec::new);
                        }
                        let bucket = &mut callers[position];
                        let roster = match bucket.iter().position(|roster| roster.target == target)
                        {
                            Some(position) => position,
                            None => {
                                bucket.push(IncomingCallerRoster {
                                    target,
                                    invocations: Vec::new(),
                                });
                                bucket.len() - 1
                            }
                        };
                        bucket[roster].invocations.push(Invocation {
                            caller: state,
                            call,
                        });
                    }
                }
                let mut external_symbols = external_entries.to_vec();
                for association in validation::provider_application_associations(program).iter() {
                    external_symbols.push(association.identity.provider);
                }
                for conformance in program.conformances() {
                    for row in &conformance.rows {
                        external_symbols.push(row.realization_machine);
                        external_symbols.push(row.realization_state);
                    }
                }
                for usage in &facts.nominal_machine_uses.uses {
                    if usage.callback_entry.is_some() {
                        external_symbols.push(usage.selected_machine);
                        external_symbols.push(usage.selected_entry);
                    }
                }
                let mut external_machines: Vec<Vec<SymbolHandle>> = Vec::new();
                for machine in external_symbols.into_iter().filter_map(|symbol| {
                    program
                        .machine_by_symbol(symbol)
                        .map(|machine| machine.symbol)
                        .or_else(|| {
                            validation::find_state(program, symbol)
                                .map(|(machine, _)| machine.symbol)
                        })
                }) {
                    let position = machine.arena_index() as usize;
                    if external_machines.len() <= position {
                        external_machines.resize_with(position + 1, Vec::new);
                    }
                    let bucket = &mut external_machines[position];
                    if !bucket.contains(&machine) {
                        bucket.push(machine);
                    }
                }
                Self {
                    callers,
                    external_machines,
                }
            }

            pub(super) fn discharge(
                &self,
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                loan_occurrences: &LoanOccurrences<'_>,
                mut pending: Vec<Requirement>,
            ) -> Result<(), &'static str> {
                let mut pending = pending
                    .drain(..)
                    .map(|requirement| (requirement, Vec::new()))
                    .collect::<Vec<_>>();
                let mut observed = Vec::new();
                let mut dependencies = Vec::new();
                let mut established = Vec::new();
                let mut lookup = None;
                while let Some((requirement, ancestry)) = pending.pop() {
                    if observed.contains(&requirement) {
                        continue;
                    }
                    if ancestry.iter().any(|previous: &Requirement| {
                        previous.state == requirement.state && previous != &requirement
                    }) {
                        return Err(
                            "recursive calls change the unresolved backing/storage relation",
                        );
                    }
                    observed.push(requirement.clone());
                    let mut ancestry = ancestry;
                    ancestry.push(requirement.clone());
                    let state = state_flow(facts, requirement.state)?;
                    if self
                        .external_machines
                        .get(state.machine_symbol.arena_index() as usize)
                        .is_some_and(|bucket| bucket.contains(&state.machine_symbol))
                    {
                        return Err(
                            "external entry has no established incoming backing/storage compatibility",
                        );
                    }
                    let roster = self
                        .callers
                        .get(requirement.state.arena_index() as usize)
                        .and_then(|bucket| {
                            bucket
                                .iter()
                                .find(|roster| roster.target == requirement.state)
                        })
                        .filter(|roster| !roster.invocations.is_empty())
                        .ok_or(
                            "incoming backing/storage compatibility reaches an entry without actual inputs",
                        )?;
                    for invocation in &roster.invocations {
                        let backing_actual =
                            actual_for(program, facts, invocation, requirement.backing.slot.root)?;
                        let source = validation::completed_storage::reference_sources_at_path(
                            program,
                            &facts.flow.control,
                            backing_actual,
                            &requirement.backing.slot.segments,
                            invocation.call.invocation_storage,
                        );
                        let mut backings = Vec::new();
                        collect_backings(
                            program,
                            facts,
                            loan_occurrences,
                            invocation.caller,
                            source,
                            &mut backings,
                        )?;
                        if backings.is_empty() {
                            return Err(
                                "an invocation has no complete captured backing alternatives",
                            );
                        }
                        let storage_actual =
                            actual_for(program, facts, invocation, requirement.storage.root)?;
                        let storage = actual_storage(
                            program,
                            facts,
                            loan_occurrences,
                            invocation.caller,
                            storage_actual,
                            &requirement.storage.segments,
                            invocation.call.invocation_storage,
                        )?;
                        if storage.is_empty() {
                            return Err(
                                "an invocation has no complete exclusive storage alternatives",
                            );
                        }
                        for backing in backings {
                            for destination in &storage {
                                match (&backing, destination) {
                                    (Backing::Immutable, _) => {
                                        if !established.contains(&requirement) {
                                            established.push(requirement.clone());
                                        }
                                    }
                                    (
                                        Backing::Place {
                                            place: source,
                                            projectable,
                                        },
                                        Storage::Place(destination),
                                    ) => {
                                        let mut source = source.clone();
                                        if *projectable {
                                            source.segments = facts::project_captured_view_segments(
                                                &source.segments,
                                                &requirement.backing.suffix,
                                            )
                                            .ok_or(
                                                "the captured pointer cannot project its retained backing",
                                            )?;
                                        }
                                        let compatibility =
                                            super::super::overlap::captured_place_compatibility(
                                                program,
                                                &facts.borrow,
                                                &source,
                                                &BorrowAccessKind::Read,
                                                destination,
                                                &BorrowAccessKind::Mutable,
                                                &[],
                                                &mut lookup,
                                            );
                                        if !compatibility.non_interfering {
                                            return Err(
                                                "an invocation's captured backing conflicts with its exclusive storage",
                                            );
                                        }
                                        if !established.contains(&requirement) {
                                            established.push(requirement.clone());
                                        }
                                    }
                                    _ => {
                                        let mut source = match &backing {
                                            Backing::Incoming(source) => source.clone(),
                                            Backing::Place { place: source, .. } => input_backing(
                                                program,
                                                facts,
                                                invocation.caller,
                                                source,
                                            )?,
                                            Backing::Immutable => {
                                                return Err(
                                                    "immutable backing entered unresolved input propagation",
                                                );
                                            }
                                        };
                                        if !matches!(
                                            &backing,
                                            Backing::Place {
                                                projectable: false,
                                                ..
                                            }
                                        ) {
                                            source
                                                .suffix
                                                .extend_from_slice(&requirement.backing.suffix);
                                        }
                                        let storage = match destination {
                                            Storage::Incoming(source) => source.clone(),
                                            Storage::Place(source) => input_storage(
                                                program,
                                                facts,
                                                invocation.caller,
                                                source,
                                            )?,
                                        };
                                        let next = Requirement {
                                            state: invocation.caller.state_symbol,
                                            backing: source,
                                            storage,
                                        };
                                        dependencies.push((requirement.clone(), next.clone()));
                                        pending.push((next, ancestry.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
                loop {
                    let mut changed = false;
                    for (parent, child) in &dependencies {
                        if established.contains(child) && !established.contains(parent) {
                            established.push(parent.clone());
                            changed = true;
                        }
                    }
                    if !changed {
                        break;
                    }
                }
                if observed
                    .iter()
                    .any(|requirement| !established.contains(requirement))
                {
                    return Err(
                        "a recursive incoming compatibility relation has no established actual boundary",
                    );
                }
                Ok(())
            }
        }
    }
    mod publication {
        /*
            Keep the caller requirements with the checked source. When Build selects
            an external entry, check them again: that entry has no ordinary caller whose
            arguments can prove its outstanding requirements.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use checked_trees::CheckFacts;
        use facts::{PlaceRoot, PlaceSegment};
        use symbols::SymbolHandle;
        use validation::CanonicalPlace;

        use super::sources::state_flow;
        use super::{IncomingBacking, IncomingBackingCalls, PendingRequirement, Requirement};

        /*
            Build can select different entries while reusing the same checked source.
            Check those entries' saved requirements for each request. Their statements
            and other borrow conflicts have already been checked.
        */
        pub(crate) fn check_external_roots(
            checked: &checked_trees::CheckedTrees,
            roots: &[SymbolHandle],
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            if roots.is_empty() || checked.facts.borrow.incoming_compatibility.is_empty() {
                return Ok(());
            }
            let program: &dyn validation::CheckingInput = checked;
            let facts = &checked.facts;
            let prepared_loans = LoanOccurrences::new(program, facts);
            let loan_occurrences = &prepared_loans;
            let calls = IncomingBackingCalls::new(program, facts, roots);
            let mut diagnostics = Vec::new();
            for (_, row) in facts.borrow.incoming_compatibility.iter() {
                let result = retained_requirement(facts, row).and_then(|requirement| {
                    calls.discharge(program, facts, loan_occurrences, vec![requirement])
                });
                if let Err(reason) = result {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "external root admission cannot establish captured backing compatibility at statement {}: {reason}",
                        row.statement_index,
                    )));
                }
            }
            crate::checks::finish(diagnostics)
        }

        pub(in crate::checks::borrows) fn publish(
            facts: &mut checked_trees::BorrowFacts,
            pending: Vec<PendingRequirement>,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            for pending in pending {
                let requirement = pending.requirement;
                let input_error = || {
                    vec![diagnostics::Diagnostic::error(
                        "incoming backing compatibility publication lost an exact input parameter identity",
                    )]
                };
                let backing_input = requirement
                    .backing
                    .slot
                    .root
                    .as_symbol()
                    .filter(|symbol| symbol.is_valid())
                    .ok_or_else(input_error)?;
                let storage_input = requirement
                    .storage
                    .root
                    .as_symbol()
                    .filter(|symbol| symbol.is_valid())
                    .ok_or_else(input_error)?;
                let backing_path = facts
                    .access_segments
                    .insert_many(requirement.backing.slot.segments);
                let backing_suffix = facts
                    .access_segments
                    .insert_many(requirement.backing.suffix);
                let storage_path = facts
                    .access_segments
                    .insert_many(requirement.storage.segments);
                facts.incoming_compatibility.append(
                    checked_trees::BorrowIncomingCompatibilityRequirement {
                        machine_symbol: pending.machine,
                        state_symbol: requirement.state,
                        statement_index: pending.statement,
                        loan: pending.loan,
                        load: pending.load,
                        read: pending.read,
                        store_target: pending.store_target,
                        backing_input,
                        backing_path,
                        backing_suffix,
                        storage_input,
                        storage_path,
                    },
                );
            }
            Ok(())
        }

        fn retained_requirement(
            facts: &CheckFacts,
            row: &checked_trees::BorrowIncomingCompatibilityRequirement,
        ) -> Result<Requirement, &'static str> {
            let state = state_flow(facts, row.state_symbol)?;
            if state.machine_symbol != row.machine_symbol {
                return Err(
                    "the retained compatibility requirement has a different machine/state owner",
                );
            }
            let control = &facts.flow.control;
            if !facts.borrow.loans.is_valid(row.loan)
                || !control.operands.is_valid(row.load)
                || !control.value_observations.is_valid(row.read)
                || !control.operands.is_valid(row.store_target)
            {
                return Err(
                    "the retained incoming compatibility requirement lost its occurrence identities",
                );
            }
            let loan = facts.borrow.loans.get(row.loan);
            let load = control.operands.get(row.load);
            let store = control.operands.get(row.store_target);
            let descriptor_read = load.descriptor_permission.loaded_read();
            let descriptor_loan = load.descriptor_permission.loaded_loan();
            if descriptor_read != row.read
                || load.state_symbol != row.state_symbol
                || store.state_symbol != row.state_symbol
                || store.statement_index != row.statement_index
                || !facts.borrow.loans.is_valid(descriptor_loan)
                || facts.borrow.loans.get(descriptor_loan).incoming_reference
                    != loan.incoming_reference
            {
                return Err(
                    "the retained incoming compatibility requirement has mismatched load/store owners",
                );
            }
            let segments = |span| {
                facts.borrow.access_segments.span(span).map(<[PlaceSegment]>::to_vec)
                .ok_or("the retained incoming compatibility requirement lost its exact structural path")
            };
            Ok(Requirement {
                state: row.state_symbol,
                backing: IncomingBacking {
                    slot: CanonicalPlace {
                        root: PlaceRoot::Symbol(row.backing_input),
                        segments: segments(row.backing_path)?,
                    },
                    suffix: segments(row.backing_suffix)?,
                },
                storage: CanonicalPlace {
                    root: PlaceRoot::Symbol(row.storage_input),
                    segments: segments(row.storage_path)?,
                },
            })
        }
    }
    mod sources {
        /*
            Keep the data reached by a pointer separate from the field storing it.
            Resolve every possible source at its recorded read or call-entry snapshot.
            A recast may identify only an enclosing storage region; keep that whole
            region unless checked field/index coordinates identify a smaller projection.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use arena::Handle;
        use checked_trees::{
            CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact, IncomingReferenceOrigin,
        };
        use facts::{PlaceRoot, PlaceSegment};
        use symbols::SymbolHandle;
        use validation::CanonicalPlace;
        use validation::completed_storage::StoredOperand;

        use super::{Backing, IncomingBacking, IncomingLoad, Invocation, Storage};

        pub(super) fn state_flow(
            facts: &CheckFacts,
            symbol: SymbolHandle,
        ) -> Result<&FlowStateFact, &'static str> {
            let mut states = facts
                .flow
                .control
                .states
                .iter()
                .filter_map(|(_, state)| (state.state_symbol == symbol).then_some(state));
            let state = states
                .next()
                .ok_or("the compatibility relation has no checked state")?;
            if states.next().is_some() {
                return Err("the compatibility relation has ambiguous checked state ownership");
            }
            Ok(state)
        }

        pub(super) fn require_static_path(path: &[PlaceSegment]) -> Result<(), &'static str> {
            if path.iter().all(|segment| {
                matches!(
                    segment,
                    PlaceSegment::Field { .. }
                        | PlaceSegment::Case { .. }
                        | PlaceSegment::FixedIndex { .. }
                )
            }) {
                Ok(())
            } else {
                Err("incoming compatibility requires exact captured structural coordinates")
            }
        }

        /*
            The activation identifies the completed read which supplied this pointer.
            A later copy or a later load of the same slot cannot replace that event.
        */
        pub(super) fn incoming_backing(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            identity: Handle<checked_trees::IncomingReferenceIdentity>,
        ) -> Result<IncomingLoad, &'static str> {
            let load = validation::reference_result_custody::completed_descriptor_load(
                program, facts, state, identity,
            )?;
            let mut pending = vec![load.sources];
            let mut output = Vec::new();
            while let Some(source) = pending.pop() {
                match source {
                    StoredOperand::Incoming(slot) => {
                        require_input_slot(program, state, &slot)?;
                        output.push(IncomingBacking {
                            slot,
                            suffix: Vec::new(),
                        });
                    }
                    StoredOperand::Alternatives(alternatives) if !alternatives.is_empty() => {
                        pending.extend(alternatives)
                    }
                    _ => {
                        return Err(
                            "the descriptor load has an unavailable or non-entry source alternative",
                        );
                    }
                }
            }
            if output.is_empty() {
                return Err("the descriptor load has no incoming source alternatives");
            }
            Ok(IncomingLoad {
                capture: load.capture,
                sources: output,
            })
        }

        fn require_input_slot(
            program: &dyn validation::CheckingInput,
            state: &FlowStateFact,
            slot: &CanonicalPlace,
        ) -> Result<(), &'static str> {
            require_static_path(&slot.segments)?;
            let declaration = program
                .state_by_symbol(state.state_symbol)
                .ok_or("the incoming slot has no declaration state")?;
            if !slot.root.as_symbol().is_some_and(|symbol| {
                program
                    .state_parameter_by_symbol(declaration, symbol)
                    .is_some()
            }) {
                return Err("the unchanged descriptor slot has no exact invocation input");
            }
            Ok(())
        }

        pub(super) fn input_storage(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            place: &CapturedPlace,
        ) -> Result<CanonicalPlace, &'static str> {
            let mut slot = CanonicalPlace {
                root: place.root,
                segments: place.segments.clone(),
            };
            if place.incoming_reference.is_valid() {
                if !facts
                    .borrow
                    .incoming_references
                    .is_valid(place.incoming_reference)
                {
                    return Err("the exclusive storage has no checked incoming namespace");
                }
                let incoming = facts
                    .borrow
                    .incoming_references
                    .get(place.incoming_reference);
                if incoming.machine_symbol != state.machine_symbol
                    || incoming.state_symbol != state.state_symbol
                {
                    return Err("the exclusive storage belongs to another invocation");
                }
                match incoming.origin {
                    IncomingReferenceOrigin::Parameter { symbol } => {
                        slot.root = PlaceRoot::Symbol(symbol);
                        slot.segments = crate::borrow::owner_place_segments(
                            facts
                                .borrow
                                .owner_segments
                                .span(incoming.owner_path)
                                .ok_or("the exclusive storage has no complete input owner path")?,
                        );
                        slot.segments.extend_from_slice(&place.incoming_owner_path);
                        slot.segments.extend_from_slice(&place.segments);
                    }
                    IncomingReferenceOrigin::ParameterReferent { symbol } => {
                        slot.root = PlaceRoot::Symbol(symbol)
                    }
                    _ => return Err("the exclusive storage has no exact input-relative relation"),
                }
            }
            require_input_slot(program, state, &slot)?;
            let declaration = program
                .state_by_symbol(state.state_symbol)
                .ok_or("the exclusive storage has no declaration state")?;
            let symbol = slot
                .root
                .as_symbol()
                .ok_or("the exclusive storage has no parameter identity")?;
            let parameter = program
                .state_parameter_by_symbol(declaration, symbol)
                .ok_or("the exclusive storage has no exact input parameter")?;
            if !validation::reference_shell(program, parameter.type_reference)
                .is_some_and(|shell| shell.access.is_exclusive())
            {
                return Err("the compatibility demand is not supplied by an exclusive input");
            }
            Ok(slot)
        }

        pub(super) fn actual_for(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            invocation: &Invocation<'_>,
            parameter: PlaceRoot,
        ) -> Result<Handle<FlowOperandFact>, &'static str> {
            let target = program
                .state_by_symbol(validation::call_target_entry_symbol(
                    program,
                    invocation.call.target_symbol,
                ))
                .ok_or("the ordinary invocation has no exact target state")?;
            let parameter = parameter
                .as_symbol()
                .ok_or("the relation is not rooted in an input parameter")?;
            let index = program
                .state_parameters(target)
                .iter()
                .position(|formal| formal.symbol == parameter)
                .ok_or("the relation's parameter is absent from its invocation target")?;
            let actuals = invocation.call.argument_operands();
            let index = u32::try_from(index)
                .map_err(|_| "the input ordinal does not fit the completed call roster")?;
            if index >= actuals.count() {
                return Err("the ordinary invocation has no completed actual for the relation");
            }
            let actual = actuals.handle_at(index);
            if !facts.flow.control.operands.is_valid(actual)
                || facts.flow.control.operands.get(actual).state_symbol
                    != invocation.caller.state_symbol
            {
                return Err("the ordinary invocation's actual has no exact completed owner");
            }
            Ok(actual)
        }

        pub(super) fn collect_backings(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            source: StoredOperand,
            output: &mut Vec<Backing>,
        ) -> Result<(), &'static str> {
            match source {
                StoredOperand::Unavailable => {
                    return Err("an actual's descriptor source is unavailable");
                }
                StoredOperand::Alternatives(alternatives) => {
                    if alternatives.is_empty() {
                        return Err("an actual's descriptor source alternatives are empty");
                    }
                    for alternative in alternatives {
                        collect_backings(
                            program,
                            facts,
                            loan_occurrences,
                            state,
                            alternative,
                            output,
                        )?;
                    }
                }
                StoredOperand::Incoming(slot) => {
                    require_input_slot(program, state, &slot)?;
                    output.push(Backing::Incoming(IncomingBacking {
                        slot,
                        suffix: Vec::new(),
                    }));
                }
                StoredOperand::Value {
                    capture,
                    projection,
                    ..
                } => {
                    if !facts.flow.control.operands.is_valid(capture) {
                        return Err("an actual's descriptor source capture is missing");
                    }
                    let operand = facts.flow.control.operands.get(capture);
                    let descriptor_loan = operand.descriptor_permission.loaded_loan();
                    if projection.is_empty() && facts.borrow.loans.is_valid(descriptor_loan) {
                        let loan = facts.borrow.loans.get(descriptor_loan);
                        if facts
                            .borrow
                            .incoming_references
                            .is_valid(loan.incoming_reference)
                            && matches!(
                                facts
                                    .borrow
                                    .incoming_references
                                    .get(loan.incoming_reference)
                                    .origin,
                                IncomingReferenceOrigin::DescriptorLoad { .. }
                            )
                        {
                            for mut source in
                                incoming_backing(program, facts, state, loan.incoming_reference)?
                                    .sources
                            {
                                source
                                    .suffix
                                    .extend_from_slice(facts.borrow.loan_segments(loan));
                                output.push(Backing::Incoming(source));
                            }
                            return Ok(());
                        }
                    }
                    let places = super::super::captured_descriptors::places(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        capture,
                        &projection,
                    )
                    .ok_or(
                        "an actual's completed descriptor has no complete backing association",
                    )?;
                    if places.is_empty() {
                        output.push(Backing::Immutable);
                    }
                    let projectable = validation::completed_storage::projectable_reference_places(
                        program,
                        &facts.flow.control,
                        capture,
                        &projection,
                    );
                    for place in places {
                        if facts
                            .borrow
                            .incoming_references
                            .is_valid(place.incoming_reference)
                            && matches!(
                                facts
                                    .borrow
                                    .incoming_references
                                    .get(place.incoming_reference)
                                    .origin,
                                IncomingReferenceOrigin::DescriptorLoad { .. }
                            )
                        {
                            for mut source in
                                incoming_backing(program, facts, state, place.incoming_reference)?
                                    .sources
                            {
                                source.suffix.extend_from_slice(&place.segments);
                                output.push(Backing::Incoming(source));
                            }
                        } else {
                            let exact = projectable.as_ref().is_some_and(|places| {
                                places.iter().any(|candidate| {
                                    candidate.root == place.root
                                        && candidate.segments == place.segments
                                })
                            });
                            output.push(Backing::Place {
                                place,
                                projectable: exact,
                            });
                        }
                    }
                }
            }
            Ok(())
        }

        /*
            Locate the field being overwritten without following its stored pointer.
            Replacing the pointer changes that field's bytes, not the data it pointed to.
        */
        pub(super) fn actual_storage(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            actual: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            current: Handle<checked_trees::FlowStorageSnapshot>,
        ) -> Result<Vec<Storage>, &'static str> {
            require_static_path(path)?;
            let operand = facts.flow.control.operands.get(actual);
            let mut crossing = 0;
            for length in 1..path.len() {
                let reference = validation::project_type_reference_from_segments(
                    program,
                    operand.type_reference,
                    &path[..length],
                )
                .ok_or("the exclusive actual has no exact typed storage projection")?;
                if validation::reference_shell(program, reference).is_some() {
                    crossing = length;
                }
            }
            let mut backings = Vec::new();
            if crossing == 0 {
                collect_backings(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    StoredOperand::Value {
                        capture: actual,
                        reference: operand.type_reference,
                        projection: Vec::new(),
                        slot: None,
                    },
                    &mut backings,
                )?;
            } else {
                let selected = validation::completed_storage::reference_sources_at_path(
                    program,
                    &facts.flow.control,
                    actual,
                    &path[..crossing],
                    current,
                );
                collect_backings(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    selected,
                    &mut backings,
                )?;
            }
            let mut output = Vec::new();
            for backing in backings {
                match backing {
                    Backing::Place {
                        mut place,
                        projectable,
                    } => {
                        if projectable {
                            place.segments = facts::project_captured_view_segments(
                                &place.segments,
                                &path[crossing..],
                            )
                            .ok_or("the exclusive actual cannot project its captured storage")?;
                        }
                        output.push(Storage::Place(place));
                    }
                    Backing::Incoming(mut source) => {
                        source.slot.segments.extend_from_slice(&source.suffix);
                        source.slot.segments.extend_from_slice(&path[crossing..]);
                        output.push(Storage::Incoming(source.slot));
                    }
                    Backing::Immutable => {
                        return Err("immutable image backing cannot supply exclusive storage");
                    }
                }
            }
            Ok(output)
        }

        pub(super) fn input_backing(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            place: &CapturedPlace,
        ) -> Result<IncomingBacking, &'static str> {
            let mut slot = CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            };
            if place.incoming_reference.is_valid() {
                if !facts
                    .borrow
                    .incoming_references
                    .is_valid(place.incoming_reference)
                {
                    return Err("the backing has no checked incoming namespace");
                }
                let incoming = facts
                    .borrow
                    .incoming_references
                    .get(place.incoming_reference);
                if incoming.machine_symbol != state.machine_symbol
                    || incoming.state_symbol != state.state_symbol
                {
                    return Err("the backing belongs to another invocation");
                }
                match incoming.origin {
                    IncomingReferenceOrigin::Parameter { symbol } => {
                        slot.root = PlaceRoot::Symbol(symbol);
                        slot.segments = crate::borrow::owner_place_segments(
                            facts
                                .borrow
                                .owner_segments
                                .span(incoming.owner_path)
                                .ok_or("the backing has no complete input owner path")?,
                        );
                        slot.segments.extend_from_slice(&place.incoming_owner_path);
                    }
                    IncomingReferenceOrigin::ParameterReferent { symbol } => {
                        slot.root = PlaceRoot::Symbol(symbol)
                    }
                    _ => return Err("the backing has no exact input-relative relation"),
                }
            }
            require_input_slot(program, state, &slot)?;
            Ok(IncomingBacking {
                slot,
                suffix: place.segments.clone(),
            })
        }
    }

    pub(crate) use publication::check_external_roots;
    pub(super) use publication::publish;
    use sources::{incoming_backing, input_storage, require_static_path};

    /*
        Scratch belongs to the state's check worker. Publication occurs only after
        every source borrow check succeeds, into the existing checked borrow owner.
    */
    pub(super) struct PendingRequirement {
        requirement: Requirement,
        machine: SymbolHandle,
        statement: usize,
        loan: Handle<BorrowLoanFact>,
        load: Handle<FlowOperandFact>,
        read: Handle<checked_trees::FlowValueObservationFact>,
        store_target: Handle<FlowOperandFact>,
    }

    #[derive(Clone, Copy)]
    struct Invocation<'facts> {
        caller: &'facts FlowStateFact,
        call: &'facts FlowCallFact,
    }

    /*
        One check invocation owns this index. It borrows the already completed call
        roster; proving a relation never rebuilds calls or re-evaluates actuals.
    */
    pub(super) struct IncomingBackingCalls<'facts> {
        callers: Vec<Vec<IncomingCallerRoster<'facts>>>,
        external_machines: Vec<Vec<SymbolHandle>>,
    }

    struct IncomingCallerRoster<'facts> {
        target: SymbolHandle,
        invocations: Vec<Invocation<'facts>>,
    }

    #[derive(Clone, PartialEq, Eq)]
    struct IncomingBacking {
        slot: CanonicalPlace,
        suffix: Vec<PlaceSegment>,
    }

    struct IncomingLoad {
        capture: Handle<FlowOperandFact>,
        sources: Vec<IncomingBacking>,
    }

    #[derive(Clone, PartialEq, Eq)]
    struct Requirement {
        state: SymbolHandle,
        backing: IncomingBacking,
        storage: CanonicalPlace,
    }

    enum Backing {
        Place {
            place: CapturedPlace,
            projectable: bool,
        },
        Incoming(IncomingBacking),
        Immutable,
    }

    enum Storage {
        Place(CapturedPlace),
        Incoming(CanonicalPlace),
    }

    impl<'facts> IncomingBackingCalls<'facts> {
        /*
            This is an additional proof of the reported collision, not an access
            exemption. Other loans, unknown formations and missing alternatives
            retain the ordinary conflict diagnostic.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn require(
            &self,
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            statement: &checked_trees::FlowStatementFact,
            loan_handle: Handle<BorrowLoanFact>,
            destinations: &[ResolvedAlias],
            retained: &mut Vec<PendingRequirement>,
        ) -> Result<bool, &'static str> {
            let loan = facts.borrow.loans.get(loan_handle);
            if loan.kind != BorrowAccessKind::Read
                || !facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
            {
                return Ok(false);
            }
            let incoming = facts
                .borrow
                .incoming_references
                .get(loan.incoming_reference);
            if !matches!(
                incoming.origin,
                IncomingReferenceOrigin::DescriptorLoad { .. }
            ) {
                return Ok(false);
            }
            let load = incoming_backing(program, facts, state, loan.incoming_reference)?;
            let suffix = facts.borrow.loan_segments(loan);
            require_static_path(suffix)?;
            if destinations.is_empty() {
                return Err("the exclusive access has no complete storage alternatives");
            }
            let mut pending = Vec::new();
            for source in load.sources {
                let mut source = source;
                source.suffix.extend_from_slice(suffix);
                for destination in destinations {
                    pending.push(Requirement {
                        state: state.state_symbol,
                        backing: source.clone(),
                        storage: input_storage(program, facts, state, &destination.place)?,
                    });
                }
            }
            self.discharge(program, facts, loan_occurrences, pending.clone())?;
            for requirement in pending {
                retained.push(PendingRequirement {
                    requirement,
                    machine: state.machine_symbol,
                    statement: statement.statement_index,
                    loan: loan_handle,
                    load: load.capture,
                    read: facts
                        .flow
                        .control
                        .operands
                        .get(load.capture)
                        .descriptor_permission
                        .loaded_read(),
                    store_target: statement.store_target,
                });
            }
            Ok(true)
        }
    }
}
pub(crate) use incoming_backing::check_external_roots as check_external_root_borrows;
mod completed_custody {
    /*
        Availability of owned parents at an implicit transformation's completion.

        Numeric conservation cannot retire borrowed storage. Use the same ordered
        occurrences and spatial judgments as explicit moves, including loans made
        by preceding operands and accesses held by an enclosing invocation. These
        parents are already exact owned claim places: resolving the *result*
        expression as their source would substitute an unrelated storage identity.
        This query excludes candidates; it grants no output membership or custody.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::checks::ownership_occurrences::{
        StatementStep, routes_compatible, statement_steps_at_evaluation,
    };
    use checked_trees::{BorrowAccessKind, CapturedPlace, FlowOperandFact};
    use validation::CanonicalPlace;

    pub(crate) fn require_unborrowed_completion(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        operand: &FlowOperandFact,
        evaluation: checked_trees::FlowPermissionEvaluation,
        parents: &[CanonicalPlace],
    ) -> Result<(), &'static str> {
        let state_flow = facts
            .flow
            .control
            .state(machine.symbol, state.symbol)
            .ok_or("missing completion state flow")?;
        let borrow_state = super::matching_borrow_state(facts, state_flow)
            .ok_or("missing completion borrow state")?;
        let statement = facts
            .flow
            .control
            .statements
            .span_or_empty(state_flow.statements)
            .iter()
            .find(|statement| statement.statement_index == operand.statement_index)
            .ok_or("missing completion statement flow")?;
        let node = program
            .statement_table
            .statement_at(state, operand.statement_index)
            .ok_or("missing completion statement")?;
        let moves = facts
            .flow
            .ownership
            .storage_transfers
            .span_or_empty(state_flow.storage_transfers);
        let occurrences = statement_steps_at_evaluation(
            program,
            machine,
            state,
            operand.statement_index,
            node,
            moves,
            facts.operators,
            Some(evaluation),
        );
        let mut completions = occurrences.iter().enumerate().filter(|(_, occurrence)| {
            matches!(occurrence.step, StatementStep::Complete(expression)
                if expression == operand.expression)
        });
        let (position, completion) = completions
            .next()
            .ok_or("missing exact completion occurrence")?;
        if completions.next().is_some() {
            return Err("ambiguous completion occurrence");
        }
        let final_holds = super::reads::final_holds_in_statement(
            program,
            facts,
            state_flow,
            statement,
            node,
            &occurrences,
            false,
        );
        let mut active = facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
            .filter(|loan| {
                !final_holds
                    .iter()
                    .any(|(ended, held_until)| ended == loan && *held_until <= position)
            })
            .collect::<Vec<_>>();
        for (handle, loan) in facts.borrow.state_loans(borrow_state) {
            if loan.statement_index == operand.statement_index
                && occurrences[..=position].iter().any(|occurrence| {
                    matches!(occurrence.step, StatementStep::Complete(expression)
                        if expression == loan.formation_expression)
                        && routes_compatible(&occurrence.route, &completion.route)
                })
                && !active.contains(&handle)
            {
                active.push(handle);
            }
        }
        let mut bounds = None;
        for parent in parents {
            for handle in &active {
                if !super::overlap::canonical_place_non_interfering(
                    program,
                    parent,
                    *handle,
                    facts.borrow,
                    &[],
                    &mut bounds,
                ) {
                    return Err("input still has a live loan at completion");
                }
            }
        }

        for loan in facts
            .borrow
            .returned_loans
            .span_or_empty(borrow_state.returned_loans)
        {
            let loan_evaluation = if loan.transition_target.is_valid() {
                checked_trees::FlowPermissionEvaluation::TransitionTarget(loan.transition_target)
            } else {
                checked_trees::FlowPermissionEvaluation::Statement
            };
            if loan.statement_index != operand.statement_index
                || loan_evaluation != evaluation
                || !occurrences[..=position].iter().any(|occurrence| {
                    matches!(occurrence.step, StatementStep::Complete(expression)
                        if expression == loan.formation_expression)
                        && routes_compatible(&occurrence.route, &completion.route)
                })
            {
                continue;
            }
            let borrowed = CapturedPlace {
                incoming_reference: loan.incoming_reference,
                incoming_owner_path: facts
                    .borrow
                    .access_segments
                    .span_or_empty(loan.incoming_owner_path)
                    .to_vec(),
                root: loan.root,
                segments: facts
                    .borrow
                    .access_segments
                    .span_or_empty(loan.segments)
                    .to_vec(),
            };
            for parent in parents {
                let owned = CapturedPlace {
                    root: parent.root,
                    segments: parent.segments.clone(),
                    ..Default::default()
                };
                if !super::overlap::captured_place_compatibility(
                    program,
                    facts.borrow,
                    &borrowed,
                    &loan.kind,
                    &owned,
                    &BorrowAccessKind::Mutable,
                    &[],
                    &mut bounds,
                )
                .non_interfering
                {
                    return Err("completed result still borrows its input");
                }
            }
        }

        let prepared_loans = LoanOccurrences::for_state(program, facts, state_flow);
        let loan_occurrences = &prepared_loans;
        for call in facts
            .borrow
            .calls
            .span_or_empty(borrow_state.calls)
            .iter()
            .filter(|call| call.statement_index == operand.statement_index)
        {
            let flow_call = crate::flow::call_flows(facts.flow.control, state_flow.calls)
                .iter()
                .find(|candidate| {
                    candidate.statement_index == call.statement_index
                        && candidate.call_ordinal == call.call_ordinal
                })
                .ok_or("missing staged invocation occurrence")?;
            if !occurrences[position + 1..].iter().any(|occurrence| {
                matches!(occurrence.step, StatementStep::Complete(expression)
                    if expression == flow_call.authored_expression)
                    && routes_compatible(&occurrence.route, &completion.route)
            }) {
                continue;
            }
            let accesses = facts.borrow.argument_accesses.span_or_empty(call.accesses);
            for access in accesses {
                let observed = occurrences[..position].iter().any(|occurrence| {
                    matches!(occurrence.step, StatementStep::Observe { expression, .. }
                        if expression == access.expression)
                        && routes_compatible(&occurrence.route, &completion.route)
                });
                if !observed
                    || super::calls::access_released_in_prefix(
                        access,
                        &occurrences[..=position],
                        &completion.route,
                    )
                {
                    continue;
                }
                if occurrences[..position].iter().any(|occurrence| {
                    let StatementStep::Move {
                        event,
                        ordered: true,
                        ..
                    } = occurrence.step
                    else {
                        return false;
                    };
                    let previous = &moves[event];
                    previous.expression == access.expression
                        && routes_compatible(&occurrence.route, &completion.route)
                        && crate::checks::multiplicity::transfers_owned_storage(
                            program,
                            state.symbol,
                            operand.statement_index,
                            previous,
                            facts
                                .flow
                                .ownership
                                .segments
                                .span_or_empty(previous.segments),
                        )
                }) {
                    continue;
                }
                let backings = super::place_access::place_backings(
                    program,
                    facts,
                    loan_occurrences,
                    state_flow,
                    operand.statement_index,
                    active.iter().copied(),
                    CapturedPlace {
                        root: access.root,
                        segments: facts.borrow.access_segments(access).to_vec(),
                        ..Default::default()
                    },
                )
                .ok_or("staged input loan has no exact backing")?;
                for parent in parents {
                    let owned = CapturedPlace {
                        root: parent.root,
                        segments: parent.segments.clone(),
                        ..Default::default()
                    };
                    if backings.iter().any(|backing| {
                        !super::overlap::captured_place_compatibility(
                            program,
                            facts.borrow,
                            &backing.place,
                            &access.kind,
                            &owned,
                            &BorrowAccessKind::Mutable,
                            &[],
                            &mut bounds,
                        )
                        .non_interfering
                    }) {
                        return Err("input is still borrowed by an enclosing invocation");
                    }
                }
            }
        }
        Ok(())
    }
}
pub(crate) use completed_custody::require_unborrowed_completion;
#[path = "checks_borrows_calls.rs"]
pub(crate) mod calls;
mod moves {
    /*
        Owned transfers exclude outstanding loans over the storage they detach.
        Statement extraction and assignment use the same captured-place compatibility
        judgment; call transfers retain their occurrence-specific call certificates.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use validation::CanonicalPlace;

    use checked_trees::{CheckFacts, FlowStateFact, FlowStatementFact};
    use diagnostics::Diagnostic;

    use super::aliases::ResolvedAlias;
    use super::details::active_loan_detail;
    use super::overlap::{StatedOrderingPremise, canonical_place_non_interfering};
    use crate::flow::DiscoveredMoveEvent;
    use crate::published::labels::symbol_name;

    pub(super) fn check_storage_move<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement: &FlowStatementFact,
        event: &DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
        active_loans: impl Iterator<Item = arena::Handle<checked_trees::BorrowLoanFact>>,
        premises: &[StatedOrderingPremise],
        diagnostics: &mut Vec<Diagnostic>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'program>>,
    ) {
        let resolved = match owned_storage_source(
            program,
            facts,
            loan_occurrences,
            state,
            statement,
            event,
            path,
        ) {
            Ok(Some(resolved)) => resolved,
            Ok(None) => return,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                return;
            }
        };
        let place = CanonicalPlace {
            root: resolved.place.root,
            segments: resolved.place.segments,
        };
        for loan_handle in active_loans {
            if super::selected_retirement::before_evaluation(
                program,
                facts,
                state,
                statement.statement_index,
                event.evaluation,
                loan_handle,
            ) {
                continue;
            }
            let loan = facts.borrow.loans.get(loan_handle);
            if loan.kind.is_exclusive() && resolved.lineage.contains(&loan_handle) {
                continue;
            }
            if canonical_place_non_interfering(
                program,
                &place,
                loan_handle,
                &facts.borrow,
                premises,
                bound_lookup,
            ) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "statement {} moves `{}` while local borrow `{}` is still active ({})",
                statement.statement_index,
                place.label(program),
                symbol_name(program, loan.owner_symbol),
                active_loan_detail(state, facts, loan_handle, statement.statement_index)
                    .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
            )));
        }
    }

    /*
        Resolve the storage being detached without following loans merely stored
        inside a whole owned carrier. Both live-local and staged-call loan checks
        consume this source; reference forwarding never becomes a referent move.
    */
    pub(super) fn owned_storage_source(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement: &FlowStatementFact,
        event: &DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> Result<Option<ResolvedAlias>, Diagnostic> {
        if !crate::checks::multiplicity::transfers_owned_storage(
            program,
            state.state_symbol,
            statement.statement_index,
            event,
            path,
        ) {
            return Ok(None);
        }
        let Some(root) = event.root.as_symbol() else {
            return Ok(None);
        };
        let endpoint_is_reference = validation::canonical_place_type_reference(
            program,
            state.state_symbol,
            statement.statement_index,
            &CanonicalPlace {
                root: event.root,
                segments: path.to_vec(),
            },
        )
        .is_some_and(|reference| crate::checks::type_reference_is_reference(program, reference));
        let resolved = if path.is_empty() && !endpoint_is_reference {
            Some(ResolvedAlias {
                place: checked_trees::CapturedPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments: Vec::new(),
                    ..Default::default()
                },
                lineage: Vec::new(),
            })
        } else {
            super::access_sources::resolve(
                program,
                facts,
                loan_occurrences,
                state,
                statement,
                event.expression,
            )
        };
        resolved.map(Some).ok_or_else(|| {
            Diagnostic::error(
                "owned transfer requires an exact storage source and available loan authority",
            )
        })
    }
}
mod retained_outputs {
    /*
        Attribution of completed receiving storage, separate from whether its
        loans are still live. A temporary argument's borrow ends at invocation;
        only an actual retained output owes a continuing loan. Missing captures or
        lifetime frontiers cannot certify absence. A matching historical root is
        insufficient: the receiving storage path and establishment statement must
        identify the produced occurrence whose lifetime the flow ledger tracks.
    */
    use crate::borrow::owner_place_segments;
    use crate::flow::canonical_place_from_semantic_place;
    use crate::flow::place_case_has_value;
    use crate::published::places::place_at;
    use checked_trees::{BorrowLoanFact, FlowOperandFact, FlowStateFact};
    use facts::{PlaceRoot, PlaceSegment};
    use symbols::SymbolHandle;
    use typed_trees::machine::Machine;
    use typed_trees::statement::StatementNode;
    use validation::borrow_contracts::borrow_carrying_owner_paths;
    use validation::completed_storage::StoredObservation;
    use validation::completed_storage::at_snapshot;
    use validation::completed_storage::captured_retention_profiles_at_path;
    use validation::completed_storage::project_captured_view_place;
    use validation::{
        CallFrameResolver, CanonicalPlace, CheckingInput, project_type_reference_from_segments,
        reference_shell,
    };

    pub(crate) fn retained_outputs_are_attributed(
        program: &dyn CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        machine: &Machine,
        state_flow: &FlowStateFact,
        statement_index: usize,
        statement: &StatementNode,
        source: SymbolHandle,
        frames: Option<&CallFrameResolver<'_>>,
    ) -> bool {
        let Some(completed) = facts.flow.state_statement(state_flow, statement_index) else {
            return false;
        };
        let control = &facts.flow.control;
        if matches!(
            statement,
            StatementNode::LocalData(_) | StatementNode::Assignment(_)
        ) && !control.operands.is_valid(completed.stored_value)
        {
            return false;
        }
        if control.operands.is_valid(completed.stored_value) {
            let value = control.operands.get(completed.stored_value);
            let Some(paths) = borrow_carrying_owner_paths(program, value.type_reference) else {
                return false;
            };
            for path in paths {
                let path = owner_place_segments(&path);
                if inactive_path(program, facts, state_flow, value, &path) {
                    continue;
                }
                let Some(profiles) =
                    captured_retention_profiles_at_path(program, control, value, &path)
                else {
                    return false;
                };
                let Some(destination) = facts
                    .semantic
                    .places
                    .is_valid(completed.stored_destination)
                    .then(|| facts.semantic.places.get(completed.stored_destination))
                    .and_then(|place| {
                        canonical_place_from_semantic_place(program, facts.semantic, place)
                    })
                else {
                    return false;
                };
                let Some(destination) = project_captured_view_place(destination, &path) else {
                    return false;
                };
                for profile in profiles {
                    let Some(roots) = control.retained_roots(profile) else {
                        return false;
                    };
                    if roots.contains(&PlaceRoot::Symbol(source))
                        && !attributed(facts, state_flow, statement_index, &destination, source)
                    {
                        return false;
                    }
                }
            }
        }
        let Some(effects) =
            frames.and_then(|frames| frames.stored_reference_effects(machine, statement))
        else {
            return false;
        };
        for effect in effects {
            let destination = CanonicalPlace {
                root: PlaceRoot::Symbol(effect.destination.local_symbol),
                segments: effect.destination.local_segments,
            };
            let Some(captures) = at_snapshot(
                program,
                control,
                completed.storage_snapshot,
                &destination,
                StoredObservation::Binding,
            )
            .and_then(|stored| stored.captures()) else {
                return false;
            };
            for capture in captures {
                let value = control.operands.get(capture.capture);
                if inactive_path(program, facts, state_flow, value, &capture.projection) {
                    continue;
                }
                let Some(profiles) = captured_retention_profiles_at_path(
                    program,
                    control,
                    value,
                    &capture.projection,
                ) else {
                    return false;
                };
                for profile in profiles {
                    let Some(roots) = control.retained_roots(profile) else {
                        return false;
                    };
                    if roots.contains(&PlaceRoot::Symbol(source))
                        && !attributed(facts, state_flow, statement_index, &destination, source)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn attributed(
        facts: checked_trees::CheckFactsRef<'_>,
        state: &FlowStateFact,
        statement_index: usize,
        destination: &CanonicalPlace,
        source: SymbolHandle,
    ) -> bool {
        let Some((_, borrowing)) = facts
            .borrow
            .first_state_borrow(state.machine_symbol, state.state_symbol)
        else {
            return false;
        };
        facts.borrow.state_loans(borrowing).any(|(_, loan)| {
            loan.statement_index == statement_index
                && loan.storage_owner == destination.root
                && loan.root == PlaceRoot::Symbol(source)
                && owner_path_matches(facts, loan, &destination.segments)
        })
    }

    fn owner_path_matches(
        facts: checked_trees::CheckFactsRef<'_>,
        loan: &BorrowLoanFact,
        path: &[PlaceSegment],
    ) -> bool {
        owner_place_segments(facts.borrow.loan_owner_path(loan)) == path
    }

    /*
        A possible type leaf is not necessarily present in this completed value.
        Consume its captured case fact, without rereading later mutable storage or
        inferring presence/absence from a reference-shaped type. A reference
        crossing ends this snapshot: the pointee's tag may have changed since then.
    */
    fn inactive_path(
        program: &dyn CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state: &FlowStateFact,
        operand: &FlowOperandFact,
        path: &[PlaceSegment],
    ) -> bool {
        let Some(mut subject) = place_at(
            program,
            operand.state_symbol,
            operand.statement_index,
            operand.expression,
        ) else {
            return false;
        };
        let contexts = facts
            .flow
            .semantic_constraint_contexts(operand.constraints)
            .collect::<Vec<_>>();
        let mut reference = operand.type_reference;
        for segment in path {
            if reference_shell(program, reference).is_some() {
                return false;
            }
            if let PlaceSegment::Case { variant } = segment
                && place_case_has_value(
                    program,
                    facts.semantic,
                    &contexts,
                    state.machine_symbol,
                    operand.state_symbol,
                    operand.statement_index,
                    &subject,
                    *variant,
                    false,
                )
            {
                return true;
            }
            let Some(projected) = project_type_reference_from_segments(
                program,
                reference,
                std::slice::from_ref(segment),
            ) else {
                return false;
            };
            reference = projected;
            let Some(projected) =
                project_captured_view_place(subject, std::slice::from_ref(segment))
            else {
                return false;
            };
            subject = projected;
        }
        false
    }
}
pub(crate) use retained_outputs::retained_outputs_are_attributed;
mod reads {
    /*
        Statement reads under live loans.

        A read observes the storage it names, so an exclusive loan that suspends
        that storage excludes it until the loan's last use; a shared loan freezes
        only mutation. Every evaluated observation of a statement, in the order
        `ownership_occurrences` replays it, is judged by the shared place-access
        judgment against the loans live where it is evaluated: those live at the
        statement's entry and those the statement already formed on a compatible
        route. A borrow expression accesses its place with its own access kind.

        A loan whose last use is this statement ends inside it, once the last
        observation naming its owner is no longer held by an invocation; a read
        evaluated after that point is a read after the last use. When the use is
        not such an observation (an assignment destination, a projected carrier),
        the loan stays live through the whole statement.

        Every carrier read is checked at its evaluated occurrence, including a
        reference operand later staged by a call. The completed operand separately
        holds its captured referent through invocation; that hold cannot authorize
        the earlier carrier read or erase a loan suspending its descriptor slot.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use arena::Handle;
    use checked_trees::{
        BorrowAccessKind, BorrowLoanFact, CapturedPlace, CheckFacts, FlowStateFact,
        FlowStatementFact,
    };
    use diagnostics::Diagnostic;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::StatementNode;
    use validation::CanonicalPlace;

    use super::details::active_loan_detail;
    use super::overlap::StatedOrderingPremise;
    use super::place_access;
    use crate::checks::ownership_occurrences::{StatementOccurrence, StatementStep};
    use crate::published::labels::symbol_name;
    use crate::published::places::place_at;

    /*
        Refine conservative syntactic holds with the producer's exact access ends.
        A scalar copied into an operand no longer borrows its source during the
        invocation. References and borrowed results instead retain the endpoints
        recorded by the argument-access producer. Unknown or ambiguous occurrences
        keep the conservative hold; nothing is released merely because it is Copy.
    */
    fn completed_access_holds(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        flow: &FlowStateFact,
        statement: &FlowStatementFact,
        occurrences: &[StatementOccurrence],
    ) -> Vec<usize> {
        use crate::checks::ownership_occurrences::{InvocationIdentity, routes_compatible};

        let mut holds: Vec<_> = occurrences
            .iter()
            .map(|occurrence| occurrence.held_until)
            .collect();
        let Some(borrow_state) = super::matching_borrow_state(facts, flow) else {
            return holds;
        };
        let Some((machine, state)) =
            validation::machine_state_by_symbols(program, flow.machine_symbol, flow.state_symbol)
        else {
            return holds;
        };
        for call in facts
            .borrow
            .calls
            .span_or_empty(borrow_state.calls)
            .iter()
            .filter(|call| call.statement_index == statement.statement_index)
        {
            let Some(site) = validation::find_call_site_in_state(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let target = validation::transition_call_target(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            );
            let Some(identity) = InvocationIdentity::from_call_site(site, target) else {
                continue;
            };
            let mut completions = occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| occurrence.invocation == Some(identity));
            let Some((completion, _)) = completions.next() else {
                continue;
            };
            if completions.next().is_some() {
                continue;
            }
            let accesses = facts.borrow.argument_accesses.span_or_empty(call.accesses);
            for (position, occurrence) in occurrences.iter().enumerate() {
                let StatementStep::Observe { expression, .. } = occurrence.step else {
                    continue;
                };
                if occurrence.held_until != completion {
                    continue;
                }
                let mut released = None;
                let mut retained = false;
                for access in accesses
                    .iter()
                    .filter(|access| access.expression == expression)
                {
                    if access.end_before_access.is_none() || !access.released_at.is_valid() {
                        retained = true;
                        break;
                    }
                    let mut endpoints = occurrences.iter().enumerate().filter(|(endpoint, end)| {
                        *endpoint >= position && *endpoint < completion
                            && matches!(end.step, StatementStep::Complete(value) if value == access.released_at)
                            && routes_compatible(&occurrence.route, &end.route)
                    });
                    let Some((endpoint, _)) = endpoints.next() else {
                        retained = true;
                        break;
                    };
                    if endpoints.next().is_some() {
                        retained = true;
                        break;
                    }
                    released =
                        Some(released.map_or(endpoint, |previous: usize| previous.max(endpoint)));
                }
                if !retained && let Some(released) = released {
                    holds[position] = released;
                }
            }
        }
        holds
    }

    /*
        Each loan live at the statement's entry whose last use is this statement,
        with the last step that still holds it there.
    */
    pub(super) fn final_holds_in_statement(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state: &FlowStateFact,
        statement: &FlowStatementFact,
        statement_node: &StatementNode,
        occurrences: &[StatementOccurrence],
        ends_source_path: bool,
    ) -> Vec<(Handle<BorrowLoanFact>, usize)> {
        if facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
            .next()
            .is_none()
        {
            return Vec::new();
        }
        let holds = completed_access_holds(program, facts, state, statement, occurrences);
        let destination_root = match statement_node {
            StatementNode::Assignment(assignment) => place_at(
                program,
                state.state_symbol,
                statement.statement_index,
                assignment.target,
            )
            .and_then(|place| place.root.as_symbol()),
            StatementNode::LocalData(_)
            | StatementNode::Expression(_)
            | StatementNode::Call(_)
            | StatementNode::Transition(_) => None,
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => return Vec::new(),
        };
        facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
            .filter_map(|handle| {
                let loan = facts.borrow.loans.get(handle);
                if ends_source_path {
                    let (_, local) = program.state_local_declaration(
                        program.state_by_symbol(state.state_symbol)?,
                        loan.owner_symbol,
                    )?;
                    if !matches!(
                        program
                            .type_reference_table
                            .type_reference(local.type_reference),
                        typed_trees::types::TypeReferenceNode::Reference { .. }
                    ) {
                        return None;
                    }
                }
                if (!ends_source_path && loan.last_use_statement_index != statement.statement_index)
                    || !loan.owner_path.is_empty()
                    || destination_root == Some(loan.owner_symbol)
                {
                    return None;
                }
                occurrences
                    .iter()
                    .enumerate()
                    .filter(|(_, occurrence)| {
                        matches!(&occurrence.step, StatementStep::Observe { place, .. }
                            if place.root.as_symbol() == Some(loan.owner_symbol))
                    })
                    .map(|(position, _)| holds[position])
                    .max()
                    .map(|held_until| (handle, held_until))
            })
            .collect()
    }

    /*
        A completed guard or earlier operand can end a local observation before
        an ordinary returning call, not just at a state-scope exit. Reuse the
        evaluated occurrence order: an operand held through this invocation ends
        at its completion and therefore remains live during it. Missing/ambiguous
        correspondence and carried reference leaves never supply this release.
    */
    pub(super) fn loans_ended_before_call(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        call: &checked_trees::BorrowCallFact,
        boundary: Option<&super::calls::InvocationBoundary>,
    ) -> Vec<Handle<BorrowLoanFact>> {
        let Some(boundary) = boundary else {
            return Vec::new();
        };
        let Some((_, state)) =
            validation::machine_state_by_symbols(program, flow.machine_symbol, flow.state_symbol)
        else {
            return Vec::new();
        };
        let Some(statement) = facts
            .flow
            .control
            .statements
            .span_or_empty(flow.statements)
            .iter()
            .find(|statement| statement.statement_index == call.statement_index)
        else {
            return Vec::new();
        };
        let Some(node) = program
            .statement_table
            .statement_at(state, call.statement_index)
        else {
            return Vec::new();
        };
        final_holds_in_statement(
            program,
            facts.view(),
            flow,
            statement,
            node,
            &boundary.occurrences,
            boundary.ends_source_path,
        )
        .into_iter()
        .filter_map(|(loan, held_until)| (held_until < boundary.completion).then_some(loan))
        .collect()
    }

    /*
        One evaluated observation and the loans live where its statement
        evaluates it.
    */
    pub(super) struct StatementRead<'a> {
        pub(super) state: &'a FlowStateFact,
        pub(super) statement: &'a FlowStatementFact,
        pub(super) place: &'a CanonicalPlace,
        /* Invalid for a statement call's receiver, which has no expression. */
        pub(super) expression: ExpressionHandle,
        pub(super) evaluation: checked_trees::FlowPermissionEvaluation,
        pub(super) active_loans: &'a [Handle<BorrowLoanFact>],
    }

    /*
        An actual transferred view value names its descriptor binding, not a read
        through that descriptor. Projected slots still read their complete receiver
        path, and contextual scalar observations still read the referee. Ordinary
        ownership/liveness and the eventual invocation's permission judgment remain
        independent; this classification creates neither a loan nor early release.
    */
    fn transfers_descriptor_value(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        read: &StatementRead<'_>,
    ) -> bool {
        if !read.expression.is_valid()
            || !read.place.segments.is_empty()
            || matches!(
                program.expression_table.expression(read.expression),
                ExpressionNode::Borrow(_)
            )
        {
            return false;
        }
        let Some(source) = place_at(
            program,
            read.state.state_symbol,
            read.statement.statement_index,
            read.expression,
        ) else {
            return false;
        };
        if source.root != read.place.root || !source.segments.is_empty() {
            return false;
        }
        let Some(declared) = validation::canonical_place_type_reference(
            program,
            read.state.state_symbol,
            read.statement.statement_index,
            &source,
        ) else {
            return false;
        };
        if !crate::flow::operand_binds_live_view(program, declared) {
            return false;
        }
        let control = &facts.flow.control;
        let mut pending = Vec::new();
        pending.extend(
            read.statement
                .stored_value
                .is_valid()
                .then_some(read.statement.stored_value),
        );
        for call in control.calls.span_or_empty(read.state.calls) {
            if call.statement_index == read.statement.statement_index {
                pending.extend(call.operands.handles());
            }
        }
        for outgoing in control.outgoing.span_or_empty(read.state.outgoing) {
            if outgoing.statement_index == read.statement.statement_index
                && outgoing.returned_value.is_valid()
            {
                pending.push(outgoing.returned_value);
            }
        }
        for observation in control
            .value_observations
            .span_or_empty(read.statement.value_observations)
        {
            if observation.result.is_valid() {
                pending.push(observation.result);
            }
        }
        let mut visited = Vec::new();
        let mut found = false;
        while let Some(handle) = pending.pop() {
            if visited.contains(&handle) {
                continue;
            }
            visited.push(handle);
            if !control.operands.is_valid(handle) {
                return false;
            }
            let value = control.operands.get(handle);
            if value.state_symbol != read.state.state_symbol
                || value.statement_index != read.statement.statement_index
            {
                continue;
            }
            if value.expression == read.expression {
                if value.backing.is_valid()
                    || !crate::flow::operand_binds_live_view(program, value.type_reference)
                    || program.normalized_type_identity(value.type_reference)
                        != program.normalized_type_identity(declared)
                {
                    return false;
                }
                found = true;
                continue;
            }
            match value.completion {
                checked_trees::FlowOperandCompletion::Constructed(children) => {
                    let Some(children) = control.operand_children.span(children) else {
                        return false;
                    };
                    pending.extend(children.iter().map(|child| child.value));
                }
                checked_trees::FlowOperandCompletion::Selected(alternatives) => {
                    let Some(alternatives) = control.operand_alternatives.span(alternatives) else {
                        return false;
                    };
                    if alternatives.is_empty()
                        || alternatives
                            .iter()
                            .any(|alternative| !alternative.source_arm.is_valid())
                    {
                        return false;
                    }
                    pending.extend(alternatives.iter().map(|alternative| alternative.value));
                }
                checked_trees::FlowOperandCompletion::Observation
                | checked_trees::FlowOperandCompletion::IncomingContents { .. }
                | checked_trees::FlowOperandCompletion::InvocationResult { .. }
                | checked_trees::FlowOperandCompletion::Copied { .. } => {}
            }
        }
        found
    }

    pub(super) fn check_storage_read<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        read: StatementRead<'_>,
        premises: &[StatedOrderingPremise],
        diagnostics: &mut Vec<Diagnostic>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) {
        if read.active_loans.is_empty() || transfers_descriptor_value(program, facts, &read) {
            return;
        }
        let Some(root) = read.place.root.as_symbol() else {
            return;
        };
        let statement_index = read.statement.statement_index;
        let borrow = read
            .expression
            .is_valid()
            .then(|| program.expression_table.expression(read.expression))
            .and_then(|node| match node {
                ExpressionNode::Borrow(borrow) => Some(borrow.access),
                _ => None,
            });
        let kind = borrow.map_or(BorrowAccessKind::Read, BorrowAccessKind::from);
        let verb = if borrow.is_some() { "borrows" } else { "reads" };
        let metadata_receiver = if read.expression.is_valid() {
            validation::machine_state_by_symbols(
                program,
                read.state.machine_symbol,
                read.state.state_symbol,
            )
            .and_then(|(machine, state)| {
                validation::collection_length_receiver(
                    program,
                    machine,
                    Some(state),
                    read.expression,
                )
            })
        } else {
            None
        };
        let place = CapturedPlace {
            root: facts::PlaceRoot::Symbol(root),
            segments: read.place.segments.clone(),
            ..Default::default()
        };
        let backings = if metadata_receiver.is_some() {
            metadata_backings(program, facts, loan_occurrences, &read, place)
        } else {
            place_access::place_backings(
                program,
                facts.view(),
                loan_occurrences,
                read.state,
                statement_index,
                read.active_loans.iter().copied(),
                place,
            )
        };
        let conflicts: Vec<_> = backings.as_ref().map_or_else(Vec::new, |backings| {
            read.active_loans
                .iter()
                .copied()
                .filter(|loan| {
                    place_access::access_interferes(
                        program,
                        facts,
                        loan_occurrences,
                        backings,
                        &kind,
                        *loan,
                        premises,
                        bound_lookup,
                    )
                })
                .collect()
        });
        if backings.is_some() && conflicts.is_empty() {
            return;
        }
        let label = read.place.label(program);
        let refusal = |message: String| {
            super::staged_values::refusal(
                program,
                read.state,
                statement_index,
                read.expression,
                &message,
            )
        };
        if backings.is_none() {
            diagnostics.push(refusal(format!(
                "statement {statement_index} {verb} `{label}` without an exact retained backing for its loan",
            )));
            return;
        }
        for loan_handle in conflicts {
            let loan = facts.borrow.loans.get(loan_handle);
            diagnostics.push(refusal(format!(
                "statement {statement_index} {verb} `{label}` while local borrow `{}` is still active ({})",
                symbol_name(program, loan.owner_symbol),
                active_loan_detail(read.state, facts, loan_handle, statement_index)
                    .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
            )));
        }
    }

    /*
        The producer records the receiver after its selectors finish. Earlier
        statement-entry loans cannot replace that exact storage generation.
    */
    fn metadata_backings(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        read: &StatementRead<'_>,
        place: CapturedPlace,
    ) -> Option<Vec<super::aliases::ResolvedAlias>> {
        let control = &facts.flow.control;
        let mut observations = control
            .value_observations
            .span(read.statement.value_observations)?
            .iter()
            .filter(|observation| {
                observation.expression == read.expression
                    && observation.evaluation == read.evaluation
            });
        let observed = observations.next()?;
        if observations.next().is_some() {
            return None;
        }
        super::access_sources::metadata_storage(
            program,
            facts,
            loan_occurrences,
            read.state,
            read.statement.statement_index,
            observed,
            place,
        )
    }
}
#[path = "checks_borrows_resources.rs"]
mod resources;
mod selected_retirement {
    /*
        A completed storage observation can release its unused local descriptor on
        the selected edge. Its exact checked reference-release boundary owns that death;
        later guards on the missed path do not keep it live on this taken path.
        Lowering must place that same checked release after the observation and
        before the target graph, independently checking the completed use inventory.
        Carried loans, derived holders and unknown origins retain ordinary lifetime
        checks. This supplies no new loan or restoration authority.
    */
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::{BorrowCallFact, BorrowLoanFact, CheckFacts, FlowStateFact};
    use typed_trees::statement::TransitionTargetHandle;

    pub(super) fn before_call(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        call: &BorrowCallFact,
        loan: Handle<BorrowLoanFact>,
    ) -> bool {
        let Some(machine) = program.machine_by_symbol(flow.machine_symbol) else {
            return false;
        };
        let Some(state) = program.state_in_machine(machine, flow.state_symbol) else {
            return false;
        };
        let evaluation = crate::flow::call_permission_evaluation(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        );
        before_evaluation(program, facts, flow, call.statement_index, evaluation, loan)
    }

    /*
        All accesses in a selected target consume the same completed-observation
        boundary. The guard itself, ordinary statements and unknown occurrences
        retain their active loans. The target proof below also retains every child
        or later use, so this query cannot manufacture restoration from last use.
    */
    pub(super) fn before_evaluation(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        statement_index: usize,
        evaluation: checked_trees::FlowPermissionEvaluation,
        loan: Handle<BorrowLoanFact>,
    ) -> bool {
        let checked_trees::FlowPermissionEvaluation::TransitionTarget(target) = evaluation else {
            return false;
        };
        before_target(program, facts, flow, statement_index, target, loan)
    }

    fn before_target(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        statement_index: usize,
        target: TransitionTargetHandle,
        loan_handle: Handle<BorrowLoanFact>,
    ) -> bool {
        let Some(state) = program.state_by_symbol(flow.state_symbol) else {
            return false;
        };
        let Some(transition) = program
            .statement_table
            .statement_at(state, statement_index)
            .and_then(|statement| statement.as_transition())
        else {
            return false;
        };
        if !target.is_valid() || (target != transition.target && target != transition.continuation)
        {
            return false;
        }
        let loan = facts.borrow.loans.get(loan_handle);
        let Some((declaration_index, local)) =
            program.state_local_declaration(state, loan.owner_symbol)
        else {
            return false;
        };
        if declaration_index >= statement_index
            || local.is_mutable
            || loan.storage_owner != facts::PlaceRoot::Symbol(local.symbol)
            || !loan.owner_path.is_empty()
            || validation::reference_shell(program, local.type_reference).is_none()
            || validation::transition_target_uses_symbol_except(
                program,
                program.statement_table.transition_target(target),
                loan.owner_symbol,
                &|expression| {
                    let Some((_, copied)) = crate::flow::copied_payload_at(
                        program,
                        &facts.flow,
                        flow.machine_symbol,
                        flow.state_symbol,
                        statement_index,
                        checked_trees::FlowPermissionEvaluation::TransitionTarget(target),
                        expression,
                    ) else {
                        return false;
                    };
                    if validation::DispatchExtraction::payload_binding_receiver(
                        program,
                        expression,
                        copied.expression,
                    ) != Some(expression)
                    {
                        return false;
                    }
                    let checked_trees::FlowOperandCompletion::Copied { copy, .. } =
                        copied.completion
                    else {
                        return false;
                    };
                    facts
                        .flow
                        .control
                        .copied_values
                        .get(copy)
                        .references
                        .is_empty()
                        && validation::has_contents(
                            program,
                            copied.type_reference,
                            &[],
                            validation::ContentsRequirement::BorrowFree,
                        )
                },
            )
        {
            return false;
        }
        if !observed_local_read(program, facts, flow, statement_index, loan_handle) {
            return false;
        }
        let Some(borrow_state) = super::matching_borrow_state(facts.view(), flow) else {
            return false;
        };
        let Some(mut state_loans) = facts.borrow.loan_group(borrow_state.loans) else {
            return false;
        };
        if state_loans.any(|(handle, child)| {
            let Some(suppliers) = facts.borrow.loan_suppliers.span(child.supplying_loans) else {
                return true;
            };
            handle != loan_handle
                && (child.source_owner_symbol == loan.owner_symbol
                    || child.root == facts::PlaceRoot::Symbol(loan.owner_symbol)
                    || matches!(child.lineage, checked_trees::BorrowLoanLineage::Reborrow { parent_loan }
                        if parent_loan == loan_handle)
                    || suppliers.contains(&loan_handle))
        }) {
            return false;
        }
        let Some(release) = validation::reference_result_custody::release_statement(
            program,
            facts,
            flow.machine_symbol,
            flow.state_symbol,
            loan_handle,
        ) else {
            return false;
        };
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        order.precedes(statement_index, release as usize)
    }

    fn observed_local_read(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        statement_index: usize,
        loan: Handle<BorrowLoanFact>,
    ) -> bool {
        let owner = facts.borrow.loans.get(loan).owner_symbol;
        let Some(statement) = facts.flow.state_statement(flow, statement_index) else {
            return false;
        };
        facts
            .flow
            .control
            .value_observations
            .span_or_empty(statement.value_observations)
            .iter()
            .any(|observation| {
                if observation.evaluation
                    != checked_trees::FlowPermissionEvaluation::TransitionGuard
                    || !facts.flow.control.operands.is_valid(observation.receiver)
                    || !facts
                        .flow
                        .borrow_loan_constraints(observation.constraints)
                        .any(|active| active == loan)
                {
                    return false;
                }
                let receiver = facts.flow.control.operands.get(observation.receiver);
                receiver.state_symbol == flow.state_symbol
                    && receiver.statement_index == statement_index
                    && place_at(
                        program,
                        flow.state_symbol,
                        statement_index,
                        receiver.expression,
                    )
                    .is_some_and(|place| {
                        place.root == facts::PlaceRoot::Symbol(owner) && place.segments.is_empty()
                    })
            })
    }
}
mod staged_values {
    /*
        Loans retained by completed values while an invocation stages its operands.

        A copied record stops observing its former home, but its reference leaves
        still exclude overlapping exclusive accesses. Statement loan inventories do
        not describe every intermediate generation installed by a nested call. Use
        the actual captured descriptors, their immutable storage cuts and the shared
        occurrence schedule instead. These are exclusion demands, never new access
        authority. Lifetime roots cannot supply pointer geometry; an unavailable
        descriptor stays potentially interfering, including a known-static opaque one.
    */
    use super::overlap::StatedOrderingPremise;
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::checks::ownership_occurrences::{InvocationIdentity, StatementOccurrence};
    use checked_trees::{CheckFacts, FlowCallDispatchRole, FlowStateFact};
    use diagnostics::Diagnostic;
    use typed_trees::expression::ExpressionHandle;
    use validation::CallSite;
    use validation::CheckingInput;
    use validation::ImmutableBoundLookup;
    use validation::borrow_contracts::is_borrow_carrying_data;
    use validation::find_call_site_in_state;
    use validation::transition_call_target;

    mod captures {
        /*
            Frozen reference leaves held by completed actuals. Unknown address coverage
            remains unknown; lifetime dependencies never substitute for referents.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use crate::checks::ownership_occurrences::{StatementOccurrence, StatementStep};
        use crate::published::places::place_at;
        use arena::Handle;
        use checked_trees::{
            BorrowAccessKind, CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact,
        };

        pub(super) struct HeldLeaf {
            pub(super) expression: typed_trees::expression::ExpressionHandle,
            pub(super) completion: usize,
            pub(super) route: Option<usize>,
            pub(super) until: usize,
            pub(super) copied: bool,
            pub(super) access: BorrowAccessKind,
            pub(super) place: Option<CapturedPlace>,
        }

        impl HeldLeaf {
            /*
                A mutable leaf with no per-path descriptor.

                Unknown address coverage stays unknown: the route names the single
                completing occurrence, and the reference holds mutably until the
                caller's boundary.
            */
            fn mutable(
                expression: typed_trees::expression::ExpressionHandle,
                route: Option<usize>,
                until: usize,
                copied: bool,
            ) -> Self {
                Self {
                    expression,
                    completion: route.unwrap_or(0),
                    route,
                    until,
                    copied,
                    access: BorrowAccessKind::Mutable,
                    place: None,
                }
            }
        }

        pub(super) fn append(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            operand: Handle<FlowOperandFact>,
            until: usize,
            copied_parent: bool,
            occurrences: &[StatementOccurrence],
            leaves: &mut Vec<HeldLeaf>,
        ) {
            let capture = facts.flow.control.operands.get(operand);
            let copied = copied_parent
                || crate::borrow::accesses::is_owned_copy_type(program, capture.type_reference);
            if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion
            {
                let rows = facts.flow.control.operand_alternatives.span(alternatives);
                if rows.is_none_or(|rows| {
                    rows.is_empty()
                        || rows.iter().any(|row| {
                            !row.source_arm.is_valid()
                                || !facts.flow.control.operands.is_valid(row.value)
                        })
                }) {
                    leaves.push(HeldLeaf::mutable(capture.expression, None, until, copied));
                    return;
                }
                for alternative in rows.unwrap() {
                    append(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        alternative.value,
                        until,
                        copied,
                        occurrences,
                        leaves,
                    );
                }
                return;
            }
            if !capture.constructed_children().is_empty() {
                let Some(children) = facts
                    .flow
                    .control
                    .operand_children
                    .span(capture.constructed_children())
                else {
                    leaves.push(HeldLeaf::mutable(capture.expression, None, until, copied));
                    return;
                };
                for child in children {
                    if !facts.flow.control.operands.is_valid(child.value) {
                        leaves.push(HeldLeaf::mutable(capture.expression, None, until, copied));
                        continue;
                    }
                    append(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        child.value,
                        until,
                        copied,
                        occurrences,
                        leaves,
                    );
                }
                return;
            }
            let mut completions = occurrences.iter().enumerate().filter(|(position, occurrence)| {
                *position < until && matches!(occurrence.step, StatementStep::Complete(expression) if expression == capture.expression)
            });
            let first = completions.next().map(|(position, _)| position);
            let route = if completions.next().is_some() {
                None
            } else {
                first
            };
            let completion = route.unwrap_or(0);
            let Some(paths) = validation::borrow_contracts::carried_reference_paths(
                program,
                capture.type_reference,
            ) else {
                leaves.push(HeldLeaf::mutable(capture.expression, route, until, copied));
                return;
            };
            for (path, access) in paths {
                let path = crate::borrow::owner_place_segments(&path);
                if path
                    .iter()
                    .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
                    && let Some(mut source) = place_at(
                        program,
                        capture.state_symbol,
                        capture.statement_index,
                        capture.expression,
                    )
                    && let Some(machine) = program.machine_holding_state(capture.state_symbol)
                {
                    source.segments.extend_from_slice(&path);
                    let contexts = facts
                        .flow
                        .semantic_constraint_contexts(capture.constraints)
                        .collect::<Vec<_>>();
                    if crate::flow::place_case_path_is_inactive(
                        program,
                        &facts.semantic,
                        &contexts,
                        machine.symbol,
                        capture.state_symbol,
                        capture.statement_index,
                        &source,
                    ) {
                        continue;
                    }
                }
                let places = super::super::captured_descriptors::places(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    operand,
                    &path,
                );
                match places {
                    Some(places) => {
                        for place in places {
                            leaves.push(HeldLeaf {
                                expression: capture.expression,
                                completion,
                                route,
                                until,
                                copied,
                                access: access.clone(),
                                place: Some(place),
                            });
                        }
                    }
                    None => leaves.push(HeldLeaf {
                        expression: capture.expression,
                        completion,
                        route,
                        until,
                        copied,
                        access,
                        place: None,
                    }),
                }
            }
        }
    }
    mod conflicts {
        /*
            Exclude exclusive access and owned consumption while a copied operand
            retains the selected backing. Timing and branch compatibility are shared
            with ownership replay; this judgment grants no permission or ancestry.
        */
        use super::super::overlap::StatedOrderingPremise;
        use super::captures::HeldLeaf;
        use crate::checks::multiplicity::transfers_owned_storage;
        use crate::checks::ownership_occurrences::{
            StatementOccurrence, StatementStep, routes_compatible,
        };
        use checked_trees::{BorrowAccessKind, CapturedPlace, CheckFacts, FlowStateFact};
        use diagnostics::Diagnostic;

        pub(super) fn check<'program>(
            program: &'program dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            statement: usize,
            occurrences: &[StatementOccurrence],
            leaves: &[HeldLeaf],
            premises: &[StatedOrderingPremise],
            diagnostics: &mut Vec<Diagnostic>,
            bounds: &mut Option<validation::ImmutableBoundLookup<'program>>,
        ) {
            let moves = facts
                .flow
                .ownership
                .storage_transfers
                .span_or_empty(state.storage_transfers);
            for (position, occurrence) in occurrences.iter().enumerate() {
                let StatementStep::Move {
                    event,
                    ordered: true,
                    ..
                } = occurrence.step
                else {
                    continue;
                };
                let event = &moves[event];
                let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
                if !event.expression.is_valid()
                    || !transfers_owned_storage(
                        program,
                        state.state_symbol,
                        statement,
                        event,
                        segments,
                    )
                {
                    continue;
                }
                let moved = CapturedPlace {
                    root: event.root,
                    segments: segments.to_vec(),
                    ..Default::default()
                };
                for held in leaves.iter().filter(|held| {
                    held.copied
                        && held.completion <= position
                        && position < held.until
                        && held.route.is_none_or(|route| {
                            routes_compatible(&occurrences[route].route, &occurrence.route)
                        })
                }) {
                    if held.place.as_ref().is_some_and(|place| {
                        super::super::overlap::captured_place_compatibility(
                            program,
                            &facts.borrow,
                            place,
                            &held.access,
                            &moved,
                            &BorrowAccessKind::Mutable,
                            premises,
                            bounds,
                        )
                        .non_interfering
                    }) {
                        continue;
                    }
                    diagnostics.push(super::refusal(
                        program,
                        state,
                        statement,
                        held.expression,
                        "a completed copied argument retains backing storage consumed by a later operand",
                    ));
                    return;
                }
            }
            for (index, left) in leaves.iter().enumerate() {
                for right in &leaves[index + 1..] {
                    if !(left.copied || right.copied)
                        || !(left.access.is_exclusive() || right.access.is_exclusive())
                        || left.completion >= right.until
                        || right.completion >= left.until
                        || !compatible_routes(occurrences, left.route, right.route)
                    {
                        continue;
                    }
                    let compatible = match (&left.place, &right.place) {
                        (Some(left_place), Some(right_place)) => {
                            super::super::overlap::captured_place_compatibility(
                                program,
                                &facts.borrow,
                                left_place,
                                &left.access,
                                right_place,
                                &right.access,
                                premises,
                                bounds,
                            )
                            .non_interfering
                        }
                        _ => false,
                    };
                    if compatible {
                        continue;
                    }
                    diagnostics.push(super::refusal(
                        program, state, statement, left.expression,
                        "a completed copied argument retains backing storage overlapping a staged exclusive loan",
                    ));
                    return;
                }
            }
        }

        fn compatible_routes(
            occurrences: &[StatementOccurrence],
            left: Option<usize>,
            right: Option<usize>,
        ) -> bool {
            match (left, right) {
                (Some(left), Some(right)) => {
                    routes_compatible(&occurrences[left].route, &occurrences[right].route)
                }
                _ => true,
            }
        }
    }

    pub(super) fn check<'program>(
        program: &'program dyn CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement: usize,
        occurrences: &[StatementOccurrence],
        premises: &[StatedOrderingPremise],
        diagnostics: &mut Vec<Diagnostic>,
        bounds: &mut Option<ImmutableBoundLookup<'program>>,
    ) {
        let Some(machine) = program.machine_by_symbol(state.machine_symbol) else {
            return;
        };
        let Some(typed_state) = program.state_by_symbol(state.state_symbol) else {
            return;
        };
        let mut leaves = Vec::new();
        for call in crate::flow::call_flows(&facts.flow.control, state.calls)
            .iter()
            .filter(|call| {
                call.statement_index == statement
                    && call.authored_dispatch_role != FlowCallDispatchRole::SavedSubjectRead
            })
        {
            let identity = match find_call_site_in_state(
                program,
                machine,
                typed_state,
                statement,
                call.call_ordinal,
            ) {
                Some(CallSite::Expression { expression, .. }) => {
                    Some(InvocationIdentity::Expression(expression))
                }
                Some(CallSite::Statement(_)) => Some(InvocationIdentity::Statement),
                Some(CallSite::TransitionNamed { .. }) => transition_call_target(
                    program,
                    machine,
                    typed_state,
                    statement,
                    call.call_ordinal,
                )
                .map(InvocationIdentity::Transition),
                None => None,
            };
            let mut completions = occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| occurrence.invocation == identity && identity.is_some());
            let completion = completions.next().map(|(position, _)| position);
            let Some(until) = completion.filter(|_| completions.next().is_none()) else {
                if call.operands.handles().any(|operand| {
                    let capture = facts.flow.control.operands.get(operand);
                    crate::borrow::accesses::is_owned_copy_type(program, capture.type_reference)
                        && is_borrow_carrying_data(program, capture.type_reference)
                }) {
                    diagnostics.push(refusal(
                        program,
                        state,
                        statement,
                        call.authored_expression,
                        "a copied argument has no unique staged invocation boundary",
                    ));
                }
                continue;
            };
            for operand in call.operands.handles() {
                captures::append(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    operand,
                    until,
                    false,
                    occurrences,
                    &mut leaves,
                );
            }
        }
        conflicts::check(
            program,
            facts,
            state,
            statement,
            occurrences,
            &leaves,
            premises,
            diagnostics,
            bounds,
        );
    }

    /*
        Produced observations can have no authored expression or source span. Keep
        their refusal locatable without treating zero as a source occurrence.
    */
    pub(super) fn refusal(
        program: &dyn CheckingInput,
        state: &FlowStateFact,
        statement: usize,
        expression: ExpressionHandle,
        message: &str,
    ) -> Diagnostic {
        if expression.is_valid() {
            let span = program.expression_table.source_span(expression);
            if span.span.end > span.span.start {
                return Diagnostic::error(message).with_source_span(span);
            }
        }
        Diagnostic::error(format!(
            "{message} in machine `{}`, state `{}`, statement {statement}",
            crate::published::labels::symbol_name(program, state.machine_symbol),
            crate::published::labels::symbol_name(program, state.state_symbol)
        ))
    }
}
mod statements {
    use crate::borrow::argument_borrow_loan_place;
    use crate::borrow::calls::replaces_reference_loan;
    use crate::borrow::view_link::substituted_result_is_view_free;
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::checks::ownership_occurrences::SelectedEvaluation;
    use crate::checks::ownership_occurrences::statement_steps;
    use crate::flow::FlowOwnershipEventSource;
    use crate::flow::call_flows;
    use crate::flow::shared_call_frames_or;
    use crate::shared::collections::push_unique;
    use checked_trees::{CheckFacts, FlowStateFact};
    use diagnostics::Diagnostic;
    use validation::find_state_in_machine;

    use crate::flow::{
        StateMutationSummaryCache, StateReceiverReadCache, assignment_requires_exclusive_access,
        call_write_accesses, statement_mutated_place,
    };
    use crate::published::labels::symbol_name;

    use super::details::active_loan_detail;
    use super::overlap::{
        StatedOrderingPremise, borrow_loan_compatibility, canonical_place_non_interfering,
    };

    pub(super) fn check_statement_borrows<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        borrow_state: &checked_trees::StateBorrowFact,
        incoming_backing: &super::incoming_backing::IncomingBackingCalls<'_>,
        incoming_requirements: &mut Vec<super::incoming_backing::PendingRequirement>,
        stated_premises: &[StatedOrderingPremise],
        diagnostics: &mut Vec<Diagnostic>,
        state_mutation_summaries: &StateMutationSummaryCache,
        receiver_reads: &StateReceiverReadCache,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        guarantees: &mut crate::checks::contracts::prover::call_guarantees::StateGuarantees<'_, 'p>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) {
        let Some(state) =
            find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
        else {
            return;
        };
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            return;
        };
        let move_segments = &facts.flow.ownership.segments;
        let moves = facts
            .flow
            .ownership
            .storage_transfers
            .span_or_empty(state_flow.storage_transfers);
        for statement in facts
            .flow
            .control
            .statements
            .span_or_empty(state_flow.statements)
        {
            let mut available_premises = stated_premises.to_vec();
            super::overlap::append_statement_premises(
                program,
                facts,
                statement.statement_index,
                guarantees,
                &mut available_premises,
                bound_lookup,
            );
            let stated_premises = available_premises.as_slice();
            let Some(statement_node) = program
                .statement_table
                .statement_at(state, statement.statement_index)
            else {
                continue;
            };

            for (loan_handle, loan) in facts
                .borrow
                .state_loans(borrow_state)
                .filter(|(_, loan)| loan.statement_index == statement.statement_index)
            {
                if super::resources::supplied_permissions::has_debt_resource(
                    &facts.borrow,
                    loan_handle,
                ) {
                    continue;
                }
                if facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                    && matches!(
                        facts
                            .borrow
                            .incoming_references
                            .get(loan.incoming_reference)
                            .origin,
                        checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
                    )
                    && !facts
                        .flow
                        .borrow_lifetimes
                        .activations
                        .span_or_empty(state_flow.borrow_activations)
                        .iter()
                        .any(|activation| activation.loan == loan_handle)
                {
                    continue;
                }
                let supplying_accesses = super::access_sources::resolve_all(
                    program,
                    facts.view(),
                    loan_occurrences,
                    state_flow,
                    statement,
                    loan.formation_expression,
                );
                let retained_lineage = super::aliases::formation_lineage(
                    facts,
                    loan_occurrences,
                    borrow_state,
                    loan_handle,
                );
                let formation_constraints = if loan.established_by_call.is_valid() {
                    if !facts.borrow.calls.is_valid(loan.established_by_call) {
                        diagnostics.push(Diagnostic::error(
                            "reference-slot custody names an unavailable establishing invocation",
                        ));
                        continue;
                    }
                    let establishing = facts.borrow.calls.get(loan.established_by_call);
                    let mut actual = call_flows(&facts.flow.control, state_flow.calls)
                        .iter()
                        .filter(|call| {
                            call.statement_index == statement.statement_index
                                && call.call_ordinal == establishing.call_ordinal
                                && call.target_symbol == establishing.target_symbol
                                && call.authored_expression == establishing.authored_expression
                        });
                    let Some(call) = actual.next() else {
                        continue;
                    };
                    if actual.next().is_some() {
                        diagnostics.push(Diagnostic::error(
                            "reference-slot custody has ambiguous normal-exit invocation evidence",
                        ));
                        continue;
                    }
                    if !facts
                        .flow
                        .borrow_loan_constraints(call.exit_constraints)
                        .any(|active| active == loan_handle)
                    {
                        diagnostics.push(Diagnostic::error(
                            "reference-slot custody is absent from its invocation normal exit",
                        ));
                        continue;
                    }
                    call.exit_constraints
                } else if !loan.formation_expression.is_valid() {
                    if !facts
                        .flow
                        .borrow_loan_constraints(statement.pre_store_constraints)
                        .any(|active| active == loan_handle)
                    {
                        diagnostics.push(Diagnostic::error(
                            "statement-call output custody is absent from its completion boundary",
                        ));
                        continue;
                    }
                    statement.pre_store_constraints
                } else if facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                    && matches!(
                        facts
                            .borrow
                            .incoming_references
                            .get(loan.incoming_reference)
                            .origin,
                        checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
                    )
                {
                    let Some(activation) = facts
                        .flow
                        .borrow_lifetimes
                        .activations
                        .span_or_empty(state_flow.borrow_activations)
                        .iter()
                        .find(|activation| activation.loan == loan_handle)
                    else {
                        continue;
                    };
                    if !facts
                        .flow
                        .control
                        .operands
                        .is_valid(activation.formation_value)
                    {
                        continue;
                    }
                    let value = facts.flow.control.operands.get(activation.formation_value);
                    if !facts
                        .flow
                        .control
                        .value_observations
                        .is_valid(value.descriptor_permission.loaded_read())
                    {
                        continue;
                    }
                    facts
                        .flow
                        .control
                        .value_observations
                        .get(value.descriptor_permission.loaded_read())
                        .constraints
                } else {
                    statement.entry_constraints
                };
                for active_loan_handle in facts.flow.borrow_loan_constraints(formation_constraints)
                {
                    if super::aliases::same_descriptor(
                        facts.view(),
                        loan_occurrences,
                        loan_handle,
                        active_loan_handle,
                    ) {
                        continue;
                    }
                    let active_loan = facts.borrow.loans.get(active_loan_handle);
                    let retained_debt = super::overlap::retained_debt_non_interfering(
                        program,
                        &facts.borrow,
                        active_loan_handle,
                        &checked_trees::CapturedPlace {
                            incoming_reference: loan.incoming_reference,
                            incoming_owner_path: facts
                                .borrow
                                .access_segments
                                .span_or_empty(loan.incoming_owner_path)
                                .to_vec(),
                            root: loan.root,
                            segments: facts.borrow.loan_segments(loan).to_vec(),
                        },
                        &loan.kind,
                        stated_premises,
                        bound_lookup,
                    );
                    let compatibility = borrow_loan_compatibility(
                        program,
                        facts,
                        loan,
                        active_loan,
                        stated_premises,
                        bound_lookup,
                    );
                    if retained_debt == Some(true)
                        || (retained_debt.is_none()
                            && (compatibility.non_interfering
                                || supplier_preserves_loan(
                                    &facts.borrow,
                                    loan,
                                    active_loan_handle,
                                    active_loan,
                                    &compatibility,
                                )
                                || (active_loan.kind.is_exclusive()
                                    && active_loan
                                        .kind
                                        .direct_reborrow_effect(&loan.kind)
                                        .is_some()
                                    && retained_lineage.as_ref().is_some_and(|lineage| {
                                        super::aliases::formation_preserves_loan(
                                            program,
                                            facts,
                                            loan_occurrences,
                                            borrow_state,
                                            lineage,
                                            active_loan_handle,
                                        )
                                    }))
                                || supplying_accesses.as_ref().is_some_and(|sources| {
                                    active_loan.kind.is_exclusive()
                                        && active_loan
                                            .kind
                                            .direct_reborrow_effect(&loan.kind)
                                            .is_some()
                                        && sources.iter().any(|source| {
                                            source.place == compatibility.left
                                                && source.lineage.contains(&active_loan_handle)
                                        })
                                })))
                    {
                        continue;
                    }

                    if retained_debt.is_none()
                        && completion_roster_supplies_operand_loan(
                            program,
                            facts,
                            loan_handle,
                            loan,
                            active_loan,
                            stated_premises,
                            bound_lookup,
                        )
                    {
                        continue;
                    }
                    if ::validation::trace_flags::struct_trace() {
                        eprintln!(
                            "STRUCT local-loan-conflict new={loan_handle:?} row={loan:?} active={active_loan_handle:?} row={active_loan:?} new_path={:?} active_path={:?} lineage={retained_lineage:?}",
                            facts.borrow.loan_segments(loan),
                            facts.borrow.loan_segments(active_loan)
                        );
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "statement {} creates local borrow `{}` while local borrow `{}` is still active ({})",
                        statement.statement_index,
                        symbol_name(program, loan.owner_symbol),
                        symbol_name(program, active_loan.owner_symbol),
                        active_loan_detail(
                            state_flow,
                            facts,
                            active_loan_handle,
                            statement.statement_index,
                        )
                        .unwrap_or_else(|| format!("borrowed at statement {}", active_loan.statement_index)),
                    )));
                }
            }

            let mut current_loans = facts
                .flow
                .borrow_loan_constraints(statement.entry_constraints)
                .collect::<Vec<_>>();
            let mut selected = SelectedEvaluation::new();
            let occurrences = statement_steps(
                program,
                machine,
                state,
                statement.statement_index,
                statement_node,
                moves,
                &facts.operators,
            );
            super::staged_values::check(
                program,
                facts,
                loan_occurrences,
                state_flow,
                statement.statement_index,
                &occurrences,
                stated_premises,
                diagnostics,
                bound_lookup,
            );
            let final_holds = super::reads::final_holds_in_statement(
                program,
                facts.view(),
                state_flow,
                statement,
                statement_node,
                &occurrences,
                false,
            );
            for (position, occurrence) in occurrences.iter().enumerate() {
                use crate::checks::ownership_occurrences::StatementStep;
                if let Err(error) = selected.enter(
                    &occurrence.route,
                    &mut current_loans,
                    |_, _| Ok(()),
                    |_, current, sibling| {
                        for loan in sibling {
                            if !current.contains(loan) {
                                current.push(*loan);
                            }
                        }
                        Ok(())
                    },
                ) {
                    diagnostics.push(error);
                    break;
                }
                match &occurrence.step {
                    StatementStep::Observe { place, expression } => {
                        let active = current_loans
                            .iter()
                            .copied()
                            .filter(|loan| {
                                !final_holds.iter().any(|(ended, held_until)| {
                                    ended == loan && *held_until < position
                                }) && !super::selected_retirement::before_evaluation(
                                    program,
                                    facts,
                                    state_flow,
                                    statement.statement_index,
                                    occurrence.evaluation,
                                    *loan,
                                )
                            })
                            .collect::<Vec<_>>();
                        super::reads::check_storage_read(
                            program,
                            facts,
                            loan_occurrences,
                            super::reads::StatementRead {
                                state: state_flow,
                                statement,
                                place,
                                expression: *expression,
                                evaluation: occurrence.evaluation,
                                active_loans: &active,
                            },
                            stated_premises,
                            diagnostics,
                            bound_lookup,
                        );
                    }
                    StatementStep::Complete(expression) => {
                        for call in facts
                            .borrow
                            .calls
                            .span_or_empty(borrow_state.calls)
                            .iter()
                            .filter(|call| {
                                call.statement_index == statement.statement_index
                                    && call.authored_expression == *expression
                                    && (!call.reference_slot_effects.is_empty()
                                        || !call.completed_reference_loans.is_empty())
                            })
                        {
                            current_loans.retain(|loan| {
                                !replaces_reference_loan(program, &facts.borrow, call, *loan)
                            });
                            for handle in call.completed_reference_loans.handles() {
                                push_unique(&mut current_loans, handle);
                            }
                        }
                        for (handle, _) in facts.borrow.state_loans(borrow_state).filter(
                            |(handle, loan)| {
                                let loaded = facts
                                    .borrow
                                    .incoming_references
                                    .is_valid(loan.incoming_reference)
                                    && matches!(
                                        facts
                                            .borrow
                                            .incoming_references
                                            .get(loan.incoming_reference)
                                            .origin,
                                        checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
                                    );
                                loan.statement_index == statement.statement_index
                                    && !loan.established_by_call.is_valid()
                                    && loan.formation_expression == *expression
                                    && (!loaded
                                        || facts
                                            .flow
                                            .borrow_lifetimes
                                            .activations
                                            .span_or_empty(state_flow.borrow_activations)
                                            .iter()
                                            .any(|activation| {
                                                activation.loan == *handle
                                                    && facts
                                                        .flow
                                                        .control
                                                        .operands
                                                        .is_valid(activation.formation_value)
                                                    && {
                                                        let capture = facts
                                                            .flow
                                                            .control
                                                            .operands
                                                            .get(activation.formation_value);
                                                        facts.flow.control.value_observations.is_valid(
                                                            capture.descriptor_permission.loaded_read(),
                                                        ) && facts
                                                            .flow
                                                            .control
                                                            .value_observations
                                                            .get(
                                                                capture
                                                                    .descriptor_permission
                                                                    .loaded_read(),
                                                            )
                                                            .evaluation
                                                            == occurrence.evaluation
                                                    }
                                            }))
                            },
                        ) {
                            push_unique(&mut current_loans, handle);
                        }
                    }
                    StatementStep::Move {
                        event,
                        ordered: true,
                        ..
                    } => {
                        let event = &moves[*event];
                        super::calls::check_staged_operand_move(
                            program,
                            facts,
                            loan_occurrences,
                            state_flow,
                            borrow_state,
                            statement,
                            event,
                            &occurrences[..position],
                            &occurrence.route,
                            stated_premises,
                            diagnostics,
                            bound_lookup,
                        );
                        if !event.expression.is_valid()
                            || !matches!(event.source, FlowOwnershipEventSource::Statement { .. })
                        {
                            continue;
                        }
                        let active = current_loans.iter().copied();
                        super::moves::check_storage_move(
                            program,
                            facts,
                            loan_occurrences,
                            state_flow,
                            statement,
                            event,
                            move_segments.span_or_empty(event.segments),
                            active,
                            stated_premises,
                            diagnostics,
                            bound_lookup,
                        );
                    }
                    _ => {}
                }
            }

            let Some(mutated_place) = statement_mutated_place(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                statement.statement_index,
                statement_node,
            ) else {
                continue;
            };

            let Some(assignment) = statement_node.as_assignment() else {
                continue;
            };
            let access = if assignment_requires_exclusive_access(
                program, machine, state, assignment,
            ) {
                checked_trees::BorrowAccessKind::Mutable
            } else {
                checked_trees::BorrowAccessKind::Read
            };
            let mut owned_frames = None;
            let frames = shared_call_frames_or(call_frames, program, &mut owned_frames);
            let binding_replacement = frames.is_some_and(|frames| {
                frames.assignment_replaces_reference_binding(machine, statement_node) == Some(true)
            });
            let destinations = super::access_sources::stored_address(
                program,
                facts,
                loan_occurrences,
                state_flow,
                statement.store_target,
                binding_replacement,
            );
            let destinations = match destinations {
                Ok(destinations) => destinations,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "statement {} cannot resolve the complete borrowed destination of `{}`: {reason}",
                        statement.statement_index,
                        mutated_place.label(program),
                    )));
                    continue;
                }
            };
            let ingress_slot = binding_replacement
                && mutated_place.segments.is_empty()
                && program
                    .state_by_symbol(state_flow.state_symbol)
                    .is_some_and(|state| {
                        mutated_place
                            .root
                            .as_symbol()
                            .and_then(|root| program.state_parameter_by_symbol(state, root))
                            .is_some_and(|parameter| {
                                parameter.is_mutable
                                    && validation::reference_shell(
                                        program,
                                        parameter.type_reference,
                                    )
                                    .is_some_and(|shell| {
                                        shell.access == language_semantics::ReferenceAccess::Shared
                                    })
                            })
                    });
            let local_descriptor_home = binding_replacement
                && mutated_place.segments.is_empty()
                && program
                    .statement_table
                    .admitted_execution_statements(state.statement_nodes)
                    .take_while(|(index, _)| *index != statement.statement_index)
                    .any(|(_, statement)| {
                        statement.as_local_data().is_some_and(|local| {
                            mutated_place.root == facts::PlaceRoot::Symbol(local.symbol)
                        })
                    });
            for loan_handle in facts
                .flow
                .borrow_loan_constraints(statement.pre_store_constraints)
            {
                if ingress_slot && facts.borrow.loans.get(loan_handle).root == mutated_place.root {
                    continue;
                }
                let loan = facts.borrow.loans.get(loan_handle);
                if local_descriptor_home
                    && facts
                        .borrow
                        .incoming_references
                        .is_valid(loan.incoming_reference)
                    && {
                        let origin = facts
                            .borrow
                            .incoming_references
                            .get(loan.incoming_reference);
                        origin.machine_symbol == state_flow.machine_symbol
                            && origin.state_symbol == state_flow.state_symbol
                            && matches!(
                                origin.origin,
                                checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
                            )
                    }
                {
                    continue;
                }
                if !super::place_access::access_interferes(
                    program,
                    facts,
                    loan_occurrences,
                    &destinations,
                    &access,
                    loan_handle,
                    stated_premises,
                    bound_lookup,
                ) {
                    continue;
                }
                let compatibility = incoming_backing.require(
                    program,
                    facts,
                    loan_occurrences,
                    state_flow,
                    statement,
                    loan_handle,
                    &destinations,
                    incoming_requirements,
                );
                if matches!(compatibility, Ok(true)) {
                    continue;
                }
                let reason = compatibility
                    .err()
                    .map(|reason| format!("; {reason}"))
                    .unwrap_or_default();
                let loan = facts.borrow.loans.get(loan_handle);
                if ::validation::trace_flags::struct_trace() {
                    eprintln!(
                        "STRUCT mutation-loan-conflict statement={} loan={loan_handle:?} row={loan:?} path={:?} suppliers={:?} incoming={:?}",
                        statement.statement_index,
                        facts.borrow.loan_segments(loan),
                        facts
                            .borrow
                            .loan_suppliers
                            .span_or_empty(loan.supplying_loans),
                        facts
                            .borrow
                            .incoming_references
                            .is_valid(loan.incoming_reference)
                            .then(|| facts
                                .borrow
                                .incoming_references
                                .get(loan.incoming_reference))
                    );
                }
                diagnostics.push(Diagnostic::error(format!(
                    "statement {} mutates `{}` while local borrow `{}` is still active ({}{reason})",
                    statement.statement_index,
                    mutated_place.label(program),
                    symbol_name(program, loan.owner_symbol),
                    active_loan_detail(state_flow, facts, loan_handle, statement.statement_index)
                        .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
                )));
            }
        }

        check_call_mutation_borrows(
            program,
            facts,
            loan_occurrences,
            state_flow,
            borrow_state,
            stated_premises,
            diagnostics,
            state_mutation_summaries,
            receiver_reads,
            call_frames,
            guarantees,
            bound_lookup,
        );
    }

    /*
        Vec-views borrow rule (and, generally, owner-mutation-through-a-call vs a
        live borrowed view).

        A mutating call through an owner -- e.g. `Vec::push`/`Vec::index_mut` or any
        explicit mutable-argument call that reallocates or writes the owner -- may
        invalidate a borrowed slice/string view (`&[T]`/`&mut [T]`/`&string`) taken
        from that owner. The owner-write rule already rejects this for *assignment*
        statements; this extends the same conflict to *call* statements, whose
        write accesses are computed by [`crate::flow::call_write_accesses`] (the receiver place
        plus any mutable-argument places). A mutated place overlapping a loan that is
        still live at the call point is rejected.

        This reuses the existing loan-overlap engine, so it inherits the disjoint
        subslice/element precision and stays conservative when a window is unknown.
        A named same-machine jump is different from a returning call: its successor
        writes happen after source-state scope exit. Replayed local-loan closure can
        discharge that conflict when no argument carries a loan into the successor.
        With authenticated source-binding closure, a forwarded child's held summary
        instead follows its remaining successor lifetime. Guard/argument calls and
        unclosed or stored carriers retain the full run; a syntactic jump alone is
        not release evidence.
    */
    fn check_call_mutation_borrows<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        borrow_state: &checked_trees::StateBorrowFact,
        stated_premises: &[StatedOrderingPremise],
        diagnostics: &mut Vec<Diagnostic>,
        summary_cache: &StateMutationSummaryCache,
        receiver_reads: &StateReceiverReadCache,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        guarantees: &mut crate::checks::contracts::prover::call_guarantees::StateGuarantees<'_, 'p>,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) {
        for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
            let mutated_places = call_write_accesses(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                &facts.borrow,
                borrow_call,
                summary_cache,
                None,
            );
            let retains_exclusive_argument = facts
                .borrow
                .argument_accesses
                .span_or_empty(borrow_call.accesses)
                .iter()
                .any(|argument| {
                    argument.kind.is_exclusive() && argument.end_before_access.is_none()
                });
            let live_loans = if mutated_places.is_empty() {
                Vec::new()
            } else {
                loans_live_at_call(program, facts, state_flow, borrow_call, call_frames)
            };
            if mutated_places.is_empty() && !retains_exclusive_argument {
                continue;
            }
            let mut available_premises = stated_premises.to_vec();
            super::overlap::append_statement_premises(
                program,
                facts,
                borrow_call.statement_index,
                guarantees,
                &mut available_premises,
                bound_lookup,
            );
            let stated_premises = available_premises.as_slice();

            super::calls::check_receiver_footprint_against_argument_loans(
                program,
                facts,
                loan_occurrences,
                state_flow,
                borrow_call,
                stated_premises,
                super::calls::ReceiverSummaries {
                    writes: summary_cache,
                    reads: receiver_reads,
                },
                diagnostics,
                bound_lookup,
            );

            let exiting_source = local_jump(program, state_flow, borrow_call).and_then(|_| {
                find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
            });
            for &loan_handle in &live_loans {
                let loan = facts.borrow.loans.get(loan_handle);
                let held_writes = exiting_source
                    .filter(|source| {
                        local_loan_ends_before_successor(
                            program,
                            facts,
                            state_flow,
                            source,
                            loan_handle,
                            borrow_call,
                            call_frames,
                        )
                    })
                    .map(|_| {
                        let captured = validation::CanonicalPlace {
                            root: loan.root,
                            segments: facts.borrow.loan_segments(loan).to_vec(),
                        };

                        call_write_accesses(
                            program,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                            &facts.borrow,
                            borrow_call,
                            summary_cache,
                            Some(&captured),
                        )
                    });
                for mutated_place in held_writes.as_ref().unwrap_or(&mutated_places) {
                    if canonical_place_non_interfering(
                        program,
                        mutated_place,
                        loan_handle,
                        &facts.borrow,
                        stated_premises,
                        bound_lookup,
                    ) {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "statement {} mutates `{}` while local borrow `{}` is still active ({})",
                        borrow_call.statement_index,
                        mutated_place.label(program),
                        symbol_name(program, loan.owner_symbol),
                        active_loan_detail(
                            state_flow,
                            facts,
                            loan_handle,
                            borrow_call.statement_index,
                        )
                        .unwrap_or_else(|| format!(
                            "borrowed at statement {}",
                            loan.statement_index
                        )),
                    )));
                    break;
                }
            }
        }
    }

    /*
        The loans live *at* a call: that invocation's actual entry constraints,
        less the local observations a named jump's source-state
        exit closes before its successor runs.
    */
    fn loans_live_at_call(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        borrow_call: &checked_trees::BorrowCallFact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Vec<arena::Handle<checked_trees::BorrowLoanFact>> {
        let mut loans = facts
            .flow
            .borrow_loan_constraints(facts.flow.state_call_entry_constraints(
                state_flow,
                borrow_call.statement_index,
                borrow_call.call_ordinal,
                borrow_call.target_symbol,
                borrow_call.receiver_symbol,
            ))
            .peekable();
        if loans.peek().is_none() {
            return Vec::new();
        }
        let exiting_source = source_exiting_without_local_loan_carriers(
            program,
            state_flow,
            borrow_call,
            call_frames,
        );
        let boundary =
            super::calls::InvocationBoundary::at(program, facts, state_flow, borrow_call);
        let ended_before_call = super::reads::loans_ended_before_call(
            program,
            facts,
            state_flow,
            borrow_call,
            boundary.as_ref(),
        );
        loans
            .filter(|loan_handle| {
                !super::selected_retirement::before_call(
                    program,
                    facts,
                    state_flow,
                    borrow_call,
                    *loan_handle,
                ) && !ended_before_call.contains(loan_handle)
                    && !exiting_source.is_some_and(|state| {
                        local_loan_ends_before_successor(
                            program,
                            facts,
                            state_flow,
                            state,
                            *loan_handle,
                            borrow_call,
                            call_frames,
                        )
                    })
            })
            .collect()
    }

    /*
        A transfer to a non-entry block retained by the caller's own machine.
        Machine-entry calls keep their caller's live loans through completion;
        nesting and recursive graph fusion do not change that lifetime boundary.
    */
    pub(super) use crate::flow::LocalJump;

    pub(super) fn local_jump<'program>(
        program: &'program dyn ::validation::CheckingInput,
        flow: &FlowStateFact,
        call: &checked_trees::BorrowCallFact,
    ) -> Option<LocalJump<'program>> {
        crate::flow::local_jump(program, flow.machine_symbol, flow.state_symbol, call)
    }

    /*
        A named jump enters another state only after the caller's local scope ends.
        Calls in its guard or arguments are ordinary pre-exit calls. Keep carried
        loans out of this judgment: their successor use needs its own correspondence.
    */
    pub(super) fn source_exiting_without_local_loan_carriers<'program>(
        program: &'program dyn ::validation::CheckingInput,
        flow: &FlowStateFact,
        call: &checked_trees::BorrowCallFact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<&'program typed_trees::state::State> {
        let source = find_state_in_machine(program, flow.machine_symbol, flow.state_symbol)?;
        let LocalJump { target, site } = local_jump(program, flow, call)?;
        let machine = program.machine_by_symbol(flow.machine_symbol)?;
        let arguments = validation::call_site_argument_expressions(program, &site);
        let parameters = program.state_parameters(target);
        let mut positional = parameters.iter();
        for argument in arguments {
            let parameter = positional.next()?;
            let actual =
                validation::expression_result_type_reference(program, machine, source, *argument)?;
            if [parameter.type_reference, actual]
                .into_iter()
                .all(|reference| substituted_result_is_view_free(program, reference, &[]))
            {
                continue;
            }
            for reference in [parameter.type_reference, actual] {
                let typed_trees::types::TypeReferenceNode::Reference { .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
            }
            let place = argument_borrow_loan_place(
                program,
                source.symbol,
                call.statement_index,
                machine.symbol,
                *argument,
            )?;
            let root = place.root.as_symbol()?;
            if let Some(incoming) = program
                .state_parameters(source)
                .iter()
                .find(|candidate| candidate.symbol == root)
            {
                let typed_trees::types::TypeReferenceNode::Reference { .. } = program
                    .type_reference_table
                    .type_reference(incoming.type_reference)
                else {
                    return None;
                };
            } else {
                let (_, local) = program.state_local_declaration(source, root)?;
                if !substituted_result_is_view_free(program, local.type_reference, &[])
                    || !call_frames?.local_backing_survives_state_jump(
                        machine,
                        source,
                        call.statement_index,
                        target,
                        arguments,
                        root,
                    )
                {
                    return None;
                }
            }
        }
        positional.next().is_none().then_some(source)
    }

    /*
        Whether `forming_loan` is recorded as deriving its authority through this
        exact active loan occurrence.

        A retained reborrow names its unique parent loan handle; the edge is
        replayed independently from the typed statement by lineage replay. Its
        inherited parent path retains the parent's captured selectors, not their
        current mutable storage values. An exact inherited prefix therefore proves
        containment without equating independently evaluated selectors. An
        unretained transfer (aggregate-carried or helper-derived) keeps only its
        rebasing source owner local, so it may rejoin an active loan of that owner
        only when the replayed containment verdict proves the forming place still
        sits inside the active loan's captured place. Either way the active loan
        must be exclusive and permit the child's access: sharing a read parent does
        not suspend anything, and a write-only parent cannot grant observation.

        This judgment only reads already-recorded loan rows and the independently
        replayed captured places. It creates no authority: the forming loan
        was itself produced by rebasing through the named source, and the pair's
        compatibility certificate still retains the honest spatial conclusion.
    */
    pub(super) fn supplier_preserves_loan(
        borrow: &checked_trees::BorrowFacts,
        forming_loan: &checked_trees::BorrowLoanFact,
        active_loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
        active_loan: &checked_trees::BorrowLoanFact,
        compatibility: &checked_trees::CapturedPlaceCompatibility,
    ) -> bool {
        if forming_loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
            return active_loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
                && forming_loan.kind == active_loan.kind
                && forming_loan.source_owner_symbol == active_loan.owner_symbol
                && matches!(
                    compatibility.containment,
                    checked_trees::CapturedPlaceContainment::Same
                )
                && borrow
                    .loan_suppliers
                    .span(forming_loan.supplying_loans)
                    .is_some_and(|suppliers| suppliers.contains(&active_loan_handle));
        }
        if !active_loan.kind.is_exclusive()
            || active_loan
                .kind
                .direct_reborrow_effect(&forming_loan.kind)
                .is_none()
        {
            return false;
        }
        let proven_containment = matches!(
            compatibility.containment,
            checked_trees::CapturedPlaceContainment::Same
                | checked_trees::CapturedPlaceContainment::RightContainsLeft
        );
        match forming_loan.lineage {
            checked_trees::BorrowLoanLineage::Reborrow { parent_loan } => {
                parent_loan == active_loan_handle
                    && forming_loan.source_owner_symbol == active_loan.owner_symbol
                    && (proven_containment
                        || (compatibility.right.root.is_valid()
                            && compatibility.left.root == compatibility.right.root
                            && compatibility
                                .left
                                .segments
                                .starts_with(&compatibility.right.segments)))
            }
            checked_trees::BorrowLoanLineage::UnretainedDerived => {
                proven_containment && forming_loan.source_owner_symbol == active_loan.owner_symbol
            }
            checked_trees::BorrowLoanLineage::DirectRoot
            | checked_trees::BorrowLoanLineage::SuppliedOutput
            | checked_trees::BorrowLoanLineage::LifetimeDependency => false,
        }
    }

    /*
        A statement-completion roster row (zero formation expression) records a
        descriptor that a call in this statement installed, but it is published at
        the completion cut, after the operands that ran later in the same statement.
        An operand loan formed by one of those later operands that transports this
        very descriptor out of the roster's owner (a match arm moving the slot an
        effectful subject refilled) is therefore derived from the roster row, not
        simultaneous with it. Judge that pair in evaluation order, as the same loans
        split across two statements are: the operand loan is the forming loan and
        the roster row its supplier.
    */
    fn completion_roster_supplies_operand_loan<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        roster_handle: arena::Handle<checked_trees::BorrowLoanFact>,
        roster: &checked_trees::BorrowLoanFact,
        operand: &checked_trees::BorrowLoanFact,
        stated_premises: &[StatedOrderingPremise],
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> bool {
        if roster.formation_expression.is_valid()
            || roster.established_by_call.is_valid()
            || !operand.formation_expression.is_valid()
            || operand.statement_index != roster.statement_index
        {
            return false;
        }
        let compatibility = borrow_loan_compatibility(
            program,
            facts,
            operand,
            roster,
            stated_premises,
            bound_lookup,
        );
        supplier_preserves_loan(
            &facts.borrow,
            operand,
            roster_handle,
            roster,
            &compatibility,
        )
    }

    pub(super) fn local_loan_ends_before_successor(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        source: &typed_trees::state::State,
        loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
        call: &checked_trees::BorrowCallFact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let loan = facts.borrow.loans.get(loan_handle);
        let Some((_, local)) = program.state_local_declaration(source, loan.owner_symbol) else {
            return false;
        };
        if !loan.owner_path.is_empty()
            || !matches!(
                program
                    .type_reference_table
                    .type_reference(local.type_reference),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            )
        {
            return source_exiting_without_local_loan_carriers(program, flow, call, call_frames)
                .is_some()
                && super::successor_cleanup::disposes_loan(program, facts, flow, call, loan);
        }
        let execution_order = program
            .statement_table
            .admitted_execution_order(source.statement_nodes);
        facts
            .flow
            .borrow_lifetimes
            .weakenings
            .span_or_empty(flow.borrow_weakenings)
            .iter()
            .any(|weakening| {
                weakening.loan == loan_handle
                    && match (weakening.reason, weakening.source) {
                        (
                            checked_trees::FlowBorrowWeakeningReason::StateExit,
                            checked_trees::FlowInvalidationSource::Statement { statement_index },
                        ) => statement_index == source.statement_nodes.count() as usize,
                        (
                            checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                            checked_trees::FlowInvalidationSource::Statement { statement_index },
                        ) => {
                            Some(statement_index)
                                == execution_order.successor(loan.last_use_statement_index)
                                && statement_index <= source.statement_nodes.count() as usize
                        }
                        _ => false,
                    }
            })
    }
}
mod successor_cleanup {
    /*
        Cleanup before an internal block transfer can end a stored loan.
        A named cleanup hook runs before its fields are released, so a last-use
        marker alone cannot prove that release. Use the selected edge's checked
        cleanup and exclude fields that move or belong to inactive cases.
        Machine-entry calls keep the caller's loans until completion.
    */
    use checked_trees::{BorrowCallFact, BorrowLoanFact, CheckFacts, FlowStateFact};
    use language_semantics::PermissionEventSource;

    pub(super) fn disposes_loan(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        call: &BorrowCallFact,
        loan: &BorrowLoanFact,
    ) -> bool {
        let Some(jump) = super::statements::local_jump(program, flow, call) else {
            return false;
        };
        let Some(source) = program.state_by_symbol(flow.state_symbol) else {
            return false;
        };
        let Some(transition) = program
            .statement_table
            .statement_at(source, call.statement_index)
            .and_then(|statement| statement.as_transition())
        else {
            return false;
        };
        [transition.target, transition.continuation]
            .into_iter()
            .filter(|target| target.is_valid())
            .any(|target| {
                jump.site.completes_transition(program, target)
                    && disposes_loan_on_target(
                        program,
                        facts,
                        flow,
                        call.statement_index,
                        target,
                        loan,
                    )
            })
    }

    pub(super) fn disposes_loan_on_target(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        statement_index: usize,
        target: typed_trees::statement::TransitionTargetHandle,
        loan: &BorrowLoanFact,
    ) -> bool {
        let Some(source) = program.state_by_symbol(flow.state_symbol) else {
            return false;
        };
        let Some(transition) = program
            .statement_table
            .statement_at(source, statement_index)
            .and_then(|statement| statement.as_transition())
        else {
            return false;
        };
        let cleanup = &facts.contextual_cleanup;
        let owner_path = facts.borrow.loan_owner_path(loan);
        cleanup.dispositions.iter().any(|(_, disposition)| {
            if disposition.machine != flow.machine_symbol
                || disposition.state != flow.state_symbol
                || disposition.source != PermissionEventSource::StateExit
                || disposition.exit.state_symbol != flow.state_symbol
                || disposition.exit.statement_index != statement_index
                || disposition.root.as_symbol() != Some(loan.owner_symbol)
                || disposition.zero_image
                || disposition.completion.is_valid()
            {
                return false;
            }
            let disposed_target = match disposition.exit.kind {
                checked_trees::FlowClaimJoinExitKind::TransitionTarget => transition.target,
                checked_trees::FlowClaimJoinExitKind::TransitionContinuation => {
                    transition.continuation
                }
                _ => return false,
            };
            if !target.is_valid() || disposed_target != target {
                return false;
            }
            if !crate::borrow::place_path_covers_owner(
                program,
                facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(disposition.path),
                owner_path,
            ) {
                return false;
            }
            [disposition.exclusions, disposition.inactive_cases]
                .into_iter()
                .flat_map(|regions| cleanup.exclusions.span_or_empty(regions))
                .all(|excluded| {
                    excluded.root != disposition.root
                        || !crate::borrow::owner_path_overlaps_place_segments(
                            program,
                            owner_path,
                            cleanup.segments.span_or_empty(excluded.segments),
                        )
                })
        })
    }
}

/*
    Shared by the checks: compatibility judgments, and binding and loan
    details.
*/
mod access_sources {
    /*
        Resolve storage accesses through retained aliases and checked source returns.

        A lifetime annotation names the complete input obligation, while checked
        exits refine its descriptor sources. Use the loan producer's classified
        source relation, then rejoin local results to live loans at formation. Selection
        keeps all captured backings; it never grants access to a private descriptor
        slot or relabels derived loans as direct reborrows.
    */
    use super::aliases::{self, ResolvedAlias};
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::published::places::place_at;
    use arena::HandleSpan;
    use checked_trees::{
        BorrowLoanLineage, CapturedPlace, CheckFacts, FlowConstraintRef, FlowStateFact,
        FlowStatementFact,
    };
    use language_semantics::ReferenceAccess;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    mod descriptor_loads {
        /*
            A completed descriptor load crosses its typed receivers, then its own slot.

            The requested leaf is not the address of its enclosing pointer's referent.
            Keep the unconsumed typed suffix separate from each captured backing address:
            `&Record` followed by a reference field owes two loads, while `&&Value` with
            no suffix owes just the outer load. Every step uses the original occurrence's
            complete constraint cut. No resulting address is repeatedly loaded merely
            because storage at that address can itself contain a reference.

            These vectors are one query's construction scratch. Existing loan and ingress
            handles retain all authority; no new durable proof, pointer or lifetime is
            manufactured by this traversal.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use arena::{Handle, HandleSpan};
        use checked_trees::{
            BorrowLoanFact, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts, FlowConstraintRef,
            FlowStateFact, IncomingReferenceIdentity,
        };
        use facts::PlaceSegment;
        use typed_trees::types::TypeReferenceHandle;
        use validation::CanonicalPlace;

        use super::super::aliases::{self, ResolvedAlias};

        /*
            A resource occurrence plus its remaining typed request. Repeating a graph
            edge after consuming a path segment is finite and legitimate; repeating the
            same resource and request cannot finish and supplies no authority.
        */
        #[derive(Clone, PartialEq, Eq)]
        struct LoadOccurrence {
            loan: Handle<BorrowLoanFact>,
            incoming: Handle<IncomingReferenceIdentity>,
            descriptor_type: TypeReferenceHandle,
            remaining: Vec<PlaceSegment>,
        }

        struct PendingLoad {
            /* Address in this namespace before traversing `remaining`. */
            address: CapturedPlace,
            /* Declared presentation at that address, after the preceding load. */
            carrier: TypeReferenceHandle,
            remaining: Vec<PlaceSegment>,
            ancestors: Vec<LoadOccurrence>,
            /*
                The receiver's authority locates this address, independently of the
                final descriptor's grant over its own referent.
            */
            lineage: Vec<Handle<BorrowLoanFact>>,
            /*
                A borrowed formal supplies its outer grant separately from the stored
                ingress leaves of its referee. In `&&T`, both have the empty owner path.
            */
            formal_root: bool,
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum LoadPurpose {
            Referent,
            DescriptorStorage,
        }

        pub(super) fn resolve(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            statement_index: usize,
            constraints: HandleSpan<FlowConstraintRef>,
            place: CapturedPlace,
        ) -> Option<Vec<ResolvedAlias>> {
            resolve_for(
                program,
                facts,
                loan_occurrences,
                state,
                statement_index,
                constraints,
                place,
                LoadPurpose::Referent,
            )
        }

        pub(super) fn storage(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            statement_index: usize,
            constraints: HandleSpan<FlowConstraintRef>,
            place: CapturedPlace,
        ) -> Option<Vec<ResolvedAlias>> {
            resolve_for(
                program,
                facts,
                loan_occurrences,
                state,
                statement_index,
                constraints,
                place,
                LoadPurpose::DescriptorStorage,
            )
        }

        fn resolve_for(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            statement_index: usize,
            constraints: HandleSpan<FlowConstraintRef>,
            place: CapturedPlace,
            purpose: LoadPurpose,
        ) -> Option<Vec<ResolvedAlias>> {
            match resolve_loads(
                program,
                facts,
                loan_occurrences,
                state,
                statement_index,
                constraints,
                place.clone(),
                purpose,
            ) {
                Ok(resolved) => Some(resolved),
                Err(reason) => {
                    if ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE descriptor-load state={:?} statement={} constraints={:?} place={:?} refusal={}",
                            state.state_symbol, statement_index, constraints, place, reason,
                        );
                    }
                    None
                }
            }
        }

        fn resolve_loads(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            statement_index: usize,
            constraints: HandleSpan<FlowConstraintRef>,
            place: CapturedPlace,
            purpose: LoadPurpose,
        ) -> Result<Vec<ResolvedAlias>, &'static str> {
            if purpose == LoadPurpose::DescriptorStorage {
                let declared = validation::canonical_place_type_reference(
                    program,
                    state.state_symbol,
                    statement_index,
                    &CanonicalPlace {
                        root: place.root,
                        segments: place.segments.clone(),
                    },
                )
                .and_then(|reference| validation::unwrapped_type_reference(program, reference))
                .ok_or("the metadata receiver has no exact collection carrier")?;
                match program.type_reference_table.type_reference(declared) {
                    typed_trees::types::TypeReferenceNode::FixedArray { .. } => {
                        return Ok(Vec::new());
                    }
                    typed_trees::types::TypeReferenceNode::Slice { .. } => {}
                    _ => return Err("the metadata receiver is not a declared collection"),
                }
            }
            let mut address = place.clone();
            address.segments.clear();
            let carrier = if place.incoming_reference.is_valid() {
                let incoming = current_incoming(facts, state, constraints, &place)
                    .ok_or("the source has no exact current incoming namespace")?;
                let owner = facts
                    .borrow
                    .owner_segments
                    .span(incoming.owner_path)
                    .ok_or("the incoming source has no complete owner path")?;
                let descriptor = validation::owned_projection_type_reference(
                    program,
                    incoming.carrier_type,
                    &crate::borrow::owner_place_segments(owner),
                )
                .ok_or("the incoming source has no declared descriptor type")?;
                validation::reference_shell(program, descriptor)
                    .ok_or("the incoming source carries a region rather than a pointer")?
                    .referee
            } else {
                validation::canonical_place_type_reference(
                    program,
                    state.state_symbol,
                    statement_index,
                    &CanonicalPlace {
                        root: place.root,
                        segments: Vec::new(),
                    },
                )
                .ok_or("the descriptor root has no declared carrier type")?
            };
            let formal_root = !place.incoming_reference.is_valid()
                && program
                    .state_by_symbol(state.state_symbol)
                    .is_some_and(|declaration| {
                        program
                            .state_parameters(declaration)
                            .iter()
                            .any(|parameter| {
                                place.root == facts::PlaceRoot::Symbol(parameter.symbol)
                                    && !parameter.is_const
                                    && !parameter.relevance.is_erased()
                                    && validation::reference_shell(
                                        program,
                                        parameter.type_reference,
                                    )
                                    .is_some()
                            })
                    });
            let mut pending = vec![PendingLoad {
                address,
                carrier,
                remaining: place.segments,
                ancestors: Vec::new(),
                lineage: Vec::new(),
                formal_root,
            }];
            let mut resolved = Vec::new();
            while let Some(mut cursor) = pending.pop() {
                cursor.remaining = validation::corresponding_place_projection(
                    program,
                    cursor.carrier,
                    &cursor.remaining,
                )
                .ok_or("the descriptor path does not correspond to its current carrier")?;
                let (prefix, descriptor_type) =
                    first_descriptor(program, cursor.carrier, &cursor.remaining)
                        .ok_or("the remaining typed suffix has no finite reference descriptor")?;
                let reference = validation::reference_shell(program, descriptor_type)
                    .ok_or("the selected typed boundary is not a reference")?;
                let terminal = prefix == cursor.remaining.len()
                    && (purpose == LoadPurpose::Referent
                        || matches!(
                            program.type_reference_table.type_reference(
                                validation::peel_declared_constraints(program, reference.referee)
                            ),
                            typed_trees::types::TypeReferenceNode::Slice { .. }
                        ));
                if !terminal && !reference.access.is_readable() {
                    return Err("the descriptor receiver is write-only");
                }
                let mut descriptor = cursor.address.clone();
                descriptor
                    .segments
                    .extend_from_slice(&cursor.remaining[..prefix]);
                let remaining = &cursor.remaining[prefix..];

                if terminal && purpose == LoadPurpose::DescriptorStorage {
                    append_distinct(
                        &mut resolved,
                        ResolvedAlias {
                            place: descriptor,
                            lineage: cursor.lineage,
                        },
                    );
                    continue;
                }

                if cursor.formal_root && prefix == 0 {
                    if terminal {
                        append_distinct(
                            &mut resolved,
                            ResolvedAlias {
                                place: descriptor,
                                lineage: Vec::new(),
                            },
                        );
                    } else {
                        pending.push(PendingLoad {
                            address: descriptor,
                            carrier: reference.referee,
                            remaining: remaining.to_vec(),
                            ancestors: cursor.ancestors,
                            lineage: cursor.lineage,
                            formal_root: false,
                        });
                    }
                    continue;
                }

                let lookup = descriptor_lookup(program, facts, state, constraints, &descriptor)
                    .ok_or("the selected slot has no exact current descriptor namespace")?;
                let candidates = aliases::resolve_descriptor_places(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    constraints,
                    lookup.clone(),
                )
                .ok_or("the selected slot has no complete current occurrence roster")?;
                if candidates.is_empty() {
                    return Err("the selected descriptor has no authority alternatives");
                }
                let mut occurrences = Vec::with_capacity(candidates.len());
                for candidate in &candidates {
                    let incoming = if candidate.local_loan.is_valid() {
                        let loan = facts.borrow.loans.get(candidate.local_loan);
                        if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
                            return Err("a lifetime dependency cannot supply a descriptor address");
                        }
                        candidate.place.incoming_reference
                    } else {
                        current_incoming(facts, state, constraints, &candidate.place).ok_or(
                            "an authority alternative has no exact current incoming identity",
                        )?;
                        candidate.place.incoming_reference
                    };
                    if candidate.consumed_segments != lookup.segments.len() {
                        return Err(
                            "an enclosing descriptor cannot supply the selected slot's pointer",
                        );
                    }
                    occurrences.push(LoadOccurrence {
                        loan: candidate.local_loan,
                        incoming,
                        descriptor_type,
                        remaining: remaining.to_vec(),
                    });
                }
                let alternatives = super::retain_captured_lineage(
                    program,
                    facts.view(),
                    loan_occurrences,
                    state,
                    statement_index,
                    lookup,
                    candidates,
                )
                .ok_or("a selected descriptor alternative has incomplete captured lineage")?;
                if alternatives.len() != occurrences.len() {
                    return Err("the selected occurrence roster lost an authority alternative");
                }
                for (alternative, occurrence) in alternatives.into_iter().zip(occurrences) {
                    if cursor.ancestors.contains(&occurrence) {
                        return Err("the typed descriptor request repeats a captured occurrence");
                    }
                    if terminal {
                        append_distinct(&mut resolved, alternative);
                    } else {
                        let mut ancestors = cursor.ancestors.clone();
                        ancestors.push(occurrence);
                        pending.push(PendingLoad {
                            address: alternative.place,
                            carrier: reference.referee,
                            remaining: remaining.to_vec(),
                            ancestors,
                            lineage: alternative.lineage,
                            formal_root: false,
                        });
                    }
                }
            }
            if resolved.is_empty() && purpose != LoadPurpose::DescriptorStorage {
                Err("the completed descriptor has no resolved authority")
            } else {
                Ok(resolved)
            }
        }

        /*
            Find the next physical descriptor without implicitly crossing it. The final
            reference is selected even when its referee is another reference.
        */
        fn first_descriptor(
            program: &dyn validation::CheckingInput,
            carrier: TypeReferenceHandle,
            remaining: &[PlaceSegment],
        ) -> Option<(usize, TypeReferenceHandle)> {
            for prefix in 0..=remaining.len() {
                let reference = validation::owned_projection_type_reference(
                    program,
                    carrier,
                    &remaining[..prefix],
                )?;
                if validation::reference_shell(program, reference).is_some() {
                    return Some((prefix, reference));
                }
            }
            None
        }

        /*
            An opaque referent stays in the exact namespace that supplied it. Reattach
            only that identity's declared carrier path for the existing occurrence
            lookup; the returned authority retains the original opaque identity.
        */
        fn descriptor_lookup(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            constraints: HandleSpan<FlowConstraintRef>,
            place: &CapturedPlace,
        ) -> Option<CapturedPlace> {
            if !place.incoming_reference.is_valid() {
                return Some(place.clone());
            }
            let incoming = current_incoming(facts, state, constraints, place)?;
            let owner = facts.borrow.owner_segments.span(incoming.owner_path)?;
            let mut segments = if place.incoming_owner_path.is_empty() {
                if owner.contains(&BorrowLoanOwnerSegment::DynamicIndex) {
                    return None;
                }
                crate::borrow::owner_place_segments(owner)
            } else {
                if !crate::borrow::owner_path_selects_leaf(
                    program,
                    owner,
                    &place.incoming_owner_path,
                ) {
                    return None;
                }
                place.incoming_owner_path.clone()
            };
            segments.extend_from_slice(&place.segments);
            Some(CapturedPlace {
                root: incoming.place_root(),
                segments,
                ..Default::default()
            })
        }

        fn current_incoming<'facts>(
            facts: &'facts CheckFacts,
            state: &FlowStateFact,
            constraints: HandleSpan<FlowConstraintRef>,
            place: &CapturedPlace,
        ) -> Option<&'facts IncomingReferenceIdentity> {
            let identity = place.incoming_reference;
            if !facts.borrow.incoming_references.is_valid(identity)
                || !facts
                    .flow
                    .incoming_reference_constraints(constraints)
                    .any(|candidate| candidate == identity)
            {
                return None;
            }
            let (_, borrow_state) = facts
                .borrow
                .first_state_borrow(state.machine_symbol, state.state_symbol)?;
            if !facts
                .borrow
                .state_owns_incoming_reference(borrow_state, identity)
            {
                return None;
            }
            let incoming = facts.borrow.incoming_references.get(identity);
            (incoming.machine_symbol == state.machine_symbol
                && incoming.state_symbol == state.state_symbol
                && incoming.place_root() == place.root)
                .then_some(incoming)
        }

        fn append_distinct(resolved: &mut Vec<ResolvedAlias>, alternative: ResolvedAlias) {
            if !resolved.iter().any(|previous| {
                previous.place == alternative.place && previous.lineage == alternative.lineage
            }) {
                resolved.push(alternative);
            }
        }
    }
    mod storage_addresses {
        /*
            Writes retain their evaluated receiver and the field or index selected below
            it, even when a returned view has no absolute source address. Relative paths
            are produced with the operand, rather than reconstructed from newer storage.
            Loading an intermediate pointer uses the ordinary descriptor authority
            resolver at that receiver's own cut. The final field's address does not load
            its old pointer when the operation replaces that field. Captured projections
            also keep an index's evaluated coordinate when a later operand changes it.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use arena::Handle;
        use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};

        use super::super::aliases::ResolvedAlias;

        pub(super) fn resolve(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            capture: Handle<FlowOperandFact>,
            replaces_descriptor: bool,
        ) -> Result<Vec<ResolvedAlias>, &'static str> {
            address(
                program,
                facts,
                loan_occurrences,
                state,
                capture,
                replaces_descriptor,
                &mut Vec::new(),
            )
        }

        fn address(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            capture: Handle<FlowOperandFact>,
            replaces_descriptor: bool,
            ancestors: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Result<Vec<ResolvedAlias>, &'static str> {
            let control = &facts.flow.control;
            if !control.operands.is_valid(capture) || ancestors.contains(&capture) {
                return Err("the completed storage address is missing or cyclic");
            }
            let operand = control.operands.get(capture);
            if operand.state_symbol != state.state_symbol {
                return Err("the completed storage address belongs to another state");
            }
            if !replaces_descriptor
                && validation::reference_shell(program, operand.type_reference).is_some()
                && !matches!(
                    operand.address_projection,
                    Some(checked_trees::FlowOperandAddressProjection::View(_))
                )
            {
                return super::super::completed_backings::resolve(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    capture,
                )
                .map(|backings| backings.places);
            }
            ancestors.push(capture);
            let result = project_address(
                program,
                facts,
                loan_occurrences,
                state,
                capture,
                replaces_descriptor,
                ancestors,
            );
            ancestors.pop();
            result
        }

        fn project_address(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            capture: Handle<FlowOperandFact>,
            replaces_descriptor: bool,
            ancestors: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Result<Vec<ResolvedAlias>, &'static str> {
            let control = &facts.flow.control;
            let operand = control.operands.get(capture);
            let mut resolved = Vec::new();
            if let Some(projection) = operand.address_projection {
                if !control.operands.is_valid(operand.backing) {
                    return Err("the captured address projection has no completed receiver");
                }
                let receiver = control.operands.get(operand.backing);
                if receiver.state_symbol != operand.state_symbol
                    || receiver.statement_index != operand.statement_index
                {
                    return Err("the completed receiver belongs to another evaluation");
                }
                let projection = control
                    .operand_child_paths
                    .span(projection.segments())
                    .ok_or("the completed address has no captured receiver projection")?;
                let backings = address(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    operand.backing,
                    false,
                    ancestors,
                )?;
                let projectable = validation::completed_storage::projectable_reference_places(
                    program,
                    control,
                    operand.backing,
                    &[],
                );
                for backing in backings {
                    let mut projected = backing.place;
                    if projectable.as_ref().is_some_and(|places| {
                        places.iter().any(|place| {
                            place.root == projected.root && place.segments == projected.segments
                        })
                    }) {
                        projected.segments =
                            facts::project_captured_view_segments(&projected.segments, projection)
                                .ok_or(
                                    "the captured address projection exceeds its receiver window",
                                )?;
                    }
                    append_distinct(
                        &mut resolved,
                        ResolvedAlias {
                            place: projected,
                            lineage: backing.lineage,
                        },
                    );
                }
            } else {
                let places = control
                    .operand_referents
                    .span(operand.referents)
                    .ok_or("the completed storage address has no complete place roster")?;
                if places.is_empty() {
                    return Err("the completed storage address has no exact places");
                }
                for place in places {
                    let place = CapturedPlace {
                        root: place.root,
                        segments: control
                            .operand_referent_segments
                            .span(place.segments)
                            .ok_or("the completed address has an incomplete projection")?
                            .to_vec(),
                        ..Default::default()
                    };
                    if replaces_descriptor {
                        append_distinct(
                            &mut resolved,
                            ResolvedAlias {
                                place,
                                lineage: Vec::new(),
                            },
                        );
                    } else {
                        for backing in super::resolve_captured_all(
                            program,
                            facts.view(),
                            loan_occurrences,
                            state,
                            operand.statement_index,
                            operand.constraints,
                            place,
                        )
                        .ok_or("the completed address has no retained access lineage")?
                        {
                            append_distinct(&mut resolved, backing);
                        }
                    }
                }
            }
            if resolved.is_empty() {
                Err("the completed address has no resolved storage alternatives")
            } else {
                Ok(resolved)
            }
        }

        fn append_distinct(resolved: &mut Vec<ResolvedAlias>, candidate: ResolvedAlias) {
            if !resolved.iter().any(|previous| {
                previous.place == candidate.place && previous.lineage == candidate.lineage
            }) {
                resolved.push(candidate);
            }
        }
    }

    /*
        The destination was evaluated before its RHS. Follow that completed address
        and each receiver's own descriptor observation, never newer source storage.
    */
    pub(in crate::checks::borrows) fn stored_address(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        capture: arena::Handle<checked_trees::FlowOperandFact>,
        replaces_descriptor: bool,
    ) -> Result<Vec<ResolvedAlias>, &'static str> {
        storage_addresses::resolve(
            program,
            facts,
            loan_occurrences,
            state,
            capture,
            replaces_descriptor,
        )
    }

    pub(in crate::checks::borrows) fn resolve(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement: &FlowStatementFact,
        expression: ExpressionHandle,
    ) -> Option<ResolvedAlias> {
        let mut sources = resolve_all(
            program,
            facts.view(),
            loan_occurrences,
            state,
            statement,
            expression,
        )?;
        (sources.len() == 1).then(|| sources.remove(0))
    }

    /*
        Resolve every possible backing with its independently established access
        lineage. The complete roster is required for writes through a selection.
    */
    pub(in crate::checks::borrows) fn resolve_all(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement: &FlowStatementFact,
        expression: ExpressionHandle,
    ) -> Option<Vec<ResolvedAlias>> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Mutable => {
                return resolve_all(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    statement,
                    borrow.target,
                );
            }
            ExpressionNode::Call(call) => {
                let mut resolved = Vec::new();
                let arguments = program.expression_table.expression_handles(call.arguments);
                for (source, projection) in
                    crate::borrow::call_reference_sources(program, state.state_symbol, call)?
                {
                    if !source.owner_path.is_empty() || !source.source_path.is_empty() {
                        return None;
                    }
                    let argument = *arguments.get(source.parameter_index)?;
                    for mut actual in
                        resolve_all(program, facts, loan_occurrences, state, statement, argument)?
                    {
                        actual.place.segments.extend_from_slice(&projection);
                        if !resolved.iter().any(|previous: &ResolvedAlias| {
                            previous.place == actual.place && previous.lineage == actual.lineage
                        }) {
                            resolved.push(actual);
                        }
                    }
                }
                return (!resolved.is_empty()).then_some(resolved);
            }
            _ => {}
        }
        let source = place_at(
            program,
            state.state_symbol,
            statement.statement_index,
            expression,
        )?;
        let place = CapturedPlace {
            root: source.root,
            segments: source.segments,
            ..Default::default()
        };
        resolve_captured_all(
            program,
            facts,
            loan_occurrences,
            state,
            statement.statement_index,
            statement.entry_constraints,
            place,
        )
    }

    /*
        Resolve a place already captured at `statement_index`, such as a call
        operand's recorded access, against the loans live at `entry_constraints`.
        A root that is not a local reference resolves to itself without lineage.
    */
    pub(in crate::checks::borrows) fn resolve_captured_all(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement_index: usize,
        entry_constraints: HandleSpan<FlowConstraintRef>,
        place: CapturedPlace,
    ) -> Option<Vec<ResolvedAlias>> {
        resolve_captured_with_loans(
            program,
            facts,
            loan_occurrences,
            state,
            statement_index,
            facts.flow.borrow_loan_constraints(entry_constraints),
            place,
        )
    }

    pub(in crate::checks::borrows) fn resolve_captured_with_loans(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement_index: usize,
        loans: impl Iterator<Item = arena::Handle<checked_trees::BorrowLoanFact>>,
        place: CapturedPlace,
    ) -> Option<Vec<ResolvedAlias>> {
        let candidates = aliases::resolve_places_with_loans(
            program,
            facts,
            loan_occurrences,
            state,
            loans,
            place.clone(),
        )?;
        retain_captured_lineage(
            program,
            facts,
            loan_occurrences,
            state,
            statement_index,
            place,
            candidates,
        )
    }

    /*
        The argument already evaluated a reference value, including a descriptor
        stored inside an owned carrier. Its completion constraints select that
        descriptor's supplying loans independently of later carrier replacement.
    */
    pub(in crate::checks::borrows) fn resolve_descriptor_all(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement_index: usize,
        constraints: HandleSpan<FlowConstraintRef>,
        place: CapturedPlace,
    ) -> Option<Vec<ResolvedAlias>> {
        descriptor_loads::resolve(
            program,
            facts,
            loan_occurrences,
            state,
            statement_index,
            constraints,
            place,
        )
    }

    /*
        Storage read by collection metadata, before loading its final descriptor.
        Receiver crossings retain their own address and access lineage.
    */
    pub(in crate::checks::borrows) fn metadata_storage(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement_index: usize,
        observed: &checked_trees::FlowValueObservationFact,
        place: CapturedPlace,
    ) -> Option<Vec<ResolvedAlias>> {
        let (machine, declaration) = validation::machine_state_by_symbols(
            program,
            state.machine_symbol,
            state.state_symbol,
        )?;
        let receiver_expression = validation::collection_length_receiver(
            program,
            machine,
            Some(declaration),
            observed.expression,
        )?;
        let control = &facts.flow.control;
        if !control.operands.is_valid(observed.receiver) {
            return None;
        }
        let receiver = control.operands.get(observed.receiver);
        if receiver.expression != receiver_expression
            || receiver.state_symbol != state.state_symbol
            || receiver.statement_index != statement_index
        {
            return None;
        }
        descriptor_loads::storage(
            program,
            facts,
            loan_occurrences,
            state,
            statement_index,
            observed.constraints,
            place,
        )
    }

    pub(super) fn retain_captured_lineage(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement_index: usize,
        place: CapturedPlace,
        candidates: Vec<aliases::ResolvedPlace>,
    ) -> Option<Vec<ResolvedAlias>> {
        let root = place.root;
        let (_, borrow_state) = facts
            .borrow
            .first_state_borrow(state.machine_symbol, state.state_symbol)?;
        for candidate in &candidates {
            if candidate.local_loan.is_valid() {
                let loan = facts.borrow.loans.get(candidate.local_loan);
                if loan.established_by_call.is_valid()
                    && !completed_call_established_loan(facts, state, candidate.local_loan)
                {
                    return None;
                }
            }
        }
        let descriptor_alternatives = candidates
            .iter()
            .map(|candidate| {
                if !candidate.local_loan.is_valid() {
                    return Vec::new();
                }
                let loan = facts.borrow.loans.get(candidate.local_loan);
                let path = facts.borrow.loan_owner_path(loan);
                if path.contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex) {
                    return vec![candidate.local_loan];
                }
                candidates
                    .iter()
                    .filter_map(|other| {
                        if !other.local_loan.is_valid() {
                            return None;
                        }
                        let other_loan = facts.borrow.loans.get(other.local_loan);
                        (loan.storage_owner == other_loan.storage_owner
                            && facts.borrow.loan_owner_path(other_loan) == path)
                            .then_some(other.local_loan)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut resolved = Vec::with_capacity(candidates.len());
        for (candidate, alternatives) in candidates.into_iter().zip(descriptor_alternatives) {
            if !candidate.local_loan.is_valid() {
                resolved.push(ResolvedAlias {
                    place: candidate.place,
                    lineage: Vec::new(),
                });
                continue;
            }
            let loan = facts.borrow.loans.get(candidate.local_loan);
            let available_at_call = loan.established_by_call.is_valid();
            if let Some(mut lineage) = aliases::retained_lineage(
                facts,
                loan_occurrences,
                borrow_state,
                candidate.local_loan,
            ) {
                for alternative in alternatives {
                    crate::shared::collections::push_unique(&mut lineage, alternative);
                }
                resolved.push(ResolvedAlias {
                    place: candidate.place,
                    lineage,
                });
                continue;
            }
            if loan.lineage != BorrowLoanLineage::UnretainedDerived
                || (loan.statement_index >= statement_index && !available_at_call)
            {
                return None;
            }
            let supplying = (|| {
                let source_state = program.state_by_symbol(state.state_symbol)?;
                let local = program
                    .statement_table
                    .statement_at(source_state, loan.statement_index)?
                    .as_local_data()?;
                if facts::PlaceRoot::Symbol(local.symbol) != root {
                    return None;
                }
                let formation = facts
                    .flow
                    .control
                    .statements
                    .span_or_empty(state.statements)
                    .iter()
                    .find(|previous| previous.statement_index == loan.statement_index)?;
                resolve_all(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    formation,
                    local.initial_value,
                )
            })();
            let mut lineage = alternatives;
            if let Some(supplying) = supplying {
                for mut source in supplying {
                    source.place.segments.extend_from_slice(&place.segments);
                    if source.place == candidate.place {
                        for ancestor in source.lineage {
                            crate::shared::collections::push_unique(&mut lineage, ancestor);
                        }
                    }
                }
            }
            resolved.push(ResolvedAlias {
                place: candidate.place,
                lineage,
            });
        }
        Some(resolved)
    }

    /*
        A statement's future loans cannot supply authority. An explicitly linked
        invocation generation instead has its own actual post-call constraint cut;
        the caller above already required that loan in the descriptor's capture.
    */
    fn completed_call_established_loan(
        facts: checked_trees::CheckFactsRef<'_>,
        state: &FlowStateFact,
        handle: arena::Handle<checked_trees::BorrowLoanFact>,
    ) -> bool {
        let loan = facts.borrow.loans.get(handle);
        if !facts.borrow.calls.is_valid(loan.established_by_call) {
            return false;
        }
        let call = facts.borrow.calls.get(loan.established_by_call);
        facts.borrow.call_at(
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        ) == Some(loan.established_by_call)
            && call
                .completed_reference_loans
                .handles()
                .any(|candidate| candidate == handle)
            && crate::flow::call_flows(facts.flow.control, state.calls)
                .iter()
                .any(|actual| {
                    actual.statement_index == loan.statement_index
                        && actual.call_ordinal == call.call_ordinal
                        && actual.target_symbol == call.target_symbol
                        && actual.authored_expression == call.authored_expression
                        && facts
                            .flow
                            .borrow_loan_constraints(actual.exit_constraints)
                            .any(|candidate| candidate == handle)
                })
    }
}
mod aliases {
    /*
        Local reference operands rejoin retained storage before overlap comparison.
        A captured place does not grant ancestry: only receivers with independently
        retained lineage may exempt their own parent loans from interference.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        BorrowFacts, BorrowLoanFact, BorrowLoanLineage, BorrowLoanOwnerSegment, StateBorrowFact,
    };
    use checked_trees::{CapturedPlace, CheckFacts, FlowStateFact};
    use typed_trees::statement::StatementNode;
    use typed_trees::types::TypeReferenceNode;

    mod lineage {
        /*
            Follow the recorded supplier history of a saved reference. Formation may
            retain lifetime obligations; ordinary access still requires a descriptor
            route. Every supplier must belong to the same state and grant the carried
            access. Joined routes retain only ancestors present on every route.
        */
        use super::same_descriptor;
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use arena::Handle;
        use checked_trees::{
            BorrowFacts, BorrowLoanFact, BorrowLoanLineage, BorrowLoanOwnerSegment, StateBorrowFact,
        };

        /*
            Forming a lifetime debt retains an already supplied input; it does not
            borrow that input again. Authenticate every supplier before admitting its
            formation alongside a common ancestor. This query is only for formation:
            ordinary descriptor access still rejects LifetimeDependency as a source.
        */
        pub(in crate::checks::borrows) fn formation_lineage(
            facts: &checked_trees::CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &StateBorrowFact,
            handle: Handle<BorrowLoanFact>,
        ) -> Option<Vec<Handle<BorrowLoanFact>>> {
            descriptor_lineage(
                facts.view(),
                loan_occurrences,
                state,
                handle,
                LineagePurpose::Formation,
            )
        }

        /*
            The exact occurrences a descriptor transfer transports. Each supplier is
            owned by this state, is read through the transfer's source owner and
            grants at least the transported access, so the transfer adds no borrow,
            owner or access of its own. A transfer naming no supplier has no lineage.
        */
        pub(in crate::checks) fn transport_suppliers<'borrow>(
            borrow: &'borrow BorrowFacts,
            state: &StateBorrowFact,
            handle: Handle<BorrowLoanFact>,
        ) -> Option<&'borrow [Handle<BorrowLoanFact>]> {
            let loan = borrow.loans.get(handle);
            if loan.lineage != BorrowLoanLineage::UnretainedDerived {
                return None;
            }
            let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
            let exact = !suppliers.is_empty()
                && suppliers.iter().all(|supplier| {
                    let source = borrow.loans.get(*supplier);
                    borrow.state_owns_loan(state, *supplier)
                        && source.owner_symbol == loan.source_owner_symbol
                        && source.kind.direct_reborrow_effect(&loan.kind).is_some()
                });
            exact.then_some(suppliers)
        }

        /*
            A moved input descriptor retains its entry identity without a local parent
            loan. Recognize only that admitted fixed leaf, not another descriptor with
            the same backing. Its lineage contains this occurrence alone; it supplies
            neither a new entry permission nor a local borrowing resource.
        */
        fn retained_input_terminal(
            borrow: &BorrowFacts,
            state: &StateBorrowFact,
            loan: &BorrowLoanFact,
        ) -> bool {
            if loan.lineage != BorrowLoanLineage::UnretainedDerived
                || !borrow
                    .loan_suppliers
                    .span(loan.supplying_loans)
                    .is_some_and(<[_]>::is_empty)
                || !borrow.state_owns_incoming_reference(state, loan.incoming_reference)
                || !borrow
                    .access_segments
                    .span(loan.segments)
                    .is_some_and(<[_]>::is_empty)
                || !borrow
                    .access_segments
                    .span(loan.incoming_owner_path)
                    .is_some_and(<[_]>::is_empty)
            {
                return false;
            }
            let incoming = borrow.incoming_references.get(loan.incoming_reference);
            if !borrow
                .owner_segments
                .span(incoming.owner_path)
                .is_some_and(|path| !path.contains(&BorrowLoanOwnerSegment::DynamicIndex))
            {
                return false;
            }
            matches!(incoming.origin, checked_trees::IncomingReferenceOrigin::Parameter { symbol }
                if symbol.is_valid()
                    && loan.source_owner_symbol == symbol
                    && loan.root == incoming.place_root())
        }

        pub(in crate::checks::borrows) fn retained_lineage(
            facts: checked_trees::CheckFactsRef<'_>,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &StateBorrowFact,
            handle: Handle<BorrowLoanFact>,
        ) -> Option<Vec<Handle<BorrowLoanFact>>> {
            descriptor_lineage(
                facts,
                loan_occurrences,
                state,
                handle,
                LineagePurpose::Access,
            )
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum LineagePurpose {
            Access,
            Formation,
        }

        fn descriptor_lineage(
            facts: checked_trees::CheckFactsRef<'_>,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &StateBorrowFact,
            handle: Handle<BorrowLoanFact>,
            purpose: LineagePurpose,
        ) -> Option<Vec<Handle<BorrowLoanFact>>> {
            let borrow = &facts.borrow;
            let mut completed: Vec<(Handle<BorrowLoanFact>, Vec<Handle<BorrowLoanFact>>)> =
                Vec::new();
            let mut active = Vec::new();
            let mut pending = vec![(handle, false)];
            while let Some((handle, closing)) = pending.pop() {
                if closing {
                    if active.pop() != Some(handle) {
                        return None;
                    }
                    let loan = borrow.loans.get(handle);
                    let parents = match &loan.lineage {
                        BorrowLoanLineage::DirectRoot => &[][..],
                        BorrowLoanLineage::SuppliedOutput => {
                            borrow.supplied_output_call(handle)?;
                            &[][..]
                        }
                        BorrowLoanLineage::Reborrow { parent_loan } => {
                            std::slice::from_ref(parent_loan)
                        }
                        BorrowLoanLineage::UnretainedDerived => {
                            borrow.loan_suppliers.span(loan.supplying_loans)?
                        }
                        BorrowLoanLineage::LifetimeDependency
                            if purpose == LineagePurpose::Formation =>
                        {
                            borrow.loan_suppliers.span(loan.supplying_loans)?
                        }
                        BorrowLoanLineage::LifetimeDependency => return None,
                    };
                    let mut common = Vec::new();
                    for (position, parent) in parents.iter().enumerate() {
                        let (_, ancestors) = completed
                            .iter()
                            .find(|(candidate, _)| candidate == parent)?;
                        if position == 0 {
                            common.clone_from(ancestors);
                        } else {
                            common.retain(|ancestor| {
                                ancestors.iter().any(|candidate| {
                                    same_descriptor(facts, loan_occurrences, *ancestor, *candidate)
                                })
                            });
                        }
                    }
                    if matches!(loan.lineage, BorrowLoanLineage::Reborrow { .. }) {
                        let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
                        let mut forwarded = Vec::new();
                        for (position, supplier) in suppliers.iter().enumerate() {
                            let (_, ancestors) = completed
                                .iter()
                                .find(|(candidate, _)| candidate == supplier)?;
                            if position == 0 {
                                forwarded.clone_from(ancestors);
                            } else {
                                forwarded.retain(|ancestor| {
                                    ancestors.iter().any(|candidate| {
                                        same_descriptor(
                                            facts,
                                            loan_occurrences,
                                            *ancestor,
                                            *candidate,
                                        )
                                    })
                                });
                            }
                        }
                        for ancestor in forwarded {
                            crate::shared::collections::push_unique(&mut common, ancestor);
                        }
                    }
                    common.push(handle);
                    completed.push((handle, common));
                    continue;
                }
                if completed.iter().any(|(candidate, _)| *candidate == handle) {
                    continue;
                }
                if !borrow.state_owns_loan(state, handle) || active.contains(&handle) {
                    return None;
                }
                active.push(handle);
                pending.push((handle, true));
                let loan = borrow.loans.get(handle);
                match loan.lineage {
                    BorrowLoanLineage::DirectRoot => {}
                    BorrowLoanLineage::SuppliedOutput => {
                        borrow.supplied_output_call(handle)?;
                    }
                    BorrowLoanLineage::Reborrow { parent_loan } => {
                        if !borrow.state_owns_loan(state, parent_loan) {
                            return None;
                        }
                        let parent = borrow.loans.get(parent_loan);
                        if loan.source_owner_symbol != parent.owner_symbol
                            || parent.kind.direct_reborrow_effect(&loan.kind).is_none()
                        {
                            return None;
                        }
                        pending.push((parent_loan, false));
                        for supplier in borrow.loan_suppliers.span(loan.supplying_loans)? {
                            if !borrow.state_owns_loan(state, *supplier)
                                || borrow
                                    .loans
                                    .get(*supplier)
                                    .kind
                                    .direct_reborrow_effect(&loan.kind)
                                    .is_none()
                            {
                                return None;
                            }
                            pending.push((*supplier, false));
                        }
                    }
                    BorrowLoanLineage::UnretainedDerived => {
                        if !retained_input_terminal(borrow, state, loan) {
                            for supplier in transport_suppliers(borrow, state, handle)? {
                                pending.push((*supplier, false));
                            }
                        }
                    }
                    BorrowLoanLineage::LifetimeDependency
                        if purpose == LineagePurpose::Formation =>
                    {
                        let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
                        if suppliers.is_empty() || !loan.source_owner_symbol.is_valid() {
                            return None;
                        }
                        for supplier in suppliers {
                            if !borrow.state_owns_loan(state, *supplier) {
                                return None;
                            }
                            let source = borrow.loans.get(*supplier);
                            if source.owner_symbol != loan.source_owner_symbol
                                || source.kind.direct_reborrow_effect(&loan.kind).is_none()
                            {
                                return None;
                            }
                            pending.push((*supplier, false));
                        }
                    }
                    BorrowLoanLineage::LifetimeDependency => return None,
                }
            }
            completed
                .into_iter()
                .find_map(|(candidate, lineage)| (candidate == handle).then_some(lineage))
        }
    }
    pub(in crate::checks) use lineage::transport_suppliers;
    pub(in crate::checks::borrows) use lineage::{formation_lineage, retained_lineage};

    pub(in crate::checks::borrows) struct ResolvedAlias {
        pub(in crate::checks::borrows) place: CapturedPlace,
        pub(in crate::checks::borrows) lineage: Vec<Handle<BorrowLoanFact>>,
    }

    impl ResolvedAlias {
        /*
            Lineage names descriptor occurrences, each of which may retain several
            alternative origins. Comparing one origin with another origin of that
            same descriptor must not treat them as simultaneous exclusive loans.
            The producer groups a completed descriptor by storage, statement and
            leaf path; match arms may have distinct formation expressions. A later
            replacement, another leaf or a child descriptor is a different occurrence.
        */
        pub(in crate::checks::borrows) fn retains_loan(
            &self,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            handle: Handle<BorrowLoanFact>,
        ) -> bool {
            self.lineage.iter().any(|ancestor| {
                same_descriptor(facts.view(), loan_occurrences, *ancestor, handle)
                    || retains_lifetime_dependency(facts, loan_occurrences, *ancestor, handle)
            })
        }
    }

    /*
        Alternatives belong to an exact descriptor occurrence, never merely to
        equal backing geometry. Wildcard paths may describe coexisting slots.
    */
    pub(in crate::checks::borrows) fn same_descriptor(
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        left: Handle<BorrowLoanFact>,
        right: Handle<BorrowLoanFact>,
    ) -> bool {
        let borrow = &facts.borrow;
        if left == right {
            return left.is_valid();
        }
        if !borrow.loans.is_valid(left) || !borrow.loans.is_valid(right) {
            return false;
        }
        let left = borrow.loans.get(left);
        let right = borrow.loans.get(right);
        if left.lineage == BorrowLoanLineage::LifetimeDependency
            || right.lineage == BorrowLoanLineage::LifetimeDependency
        {
            return false;
        }
        same_carrier_occurrence(facts, loan_occurrences, left, right)
    }

    /*
        One spatial descriptor retains its own lifetime cohort independently of
        where it points. `lineage` contains only suppliers common to every route;
        requiring this exact historical occurrence cannot revive a replaced slot or
        turn an unselected input's address into a descriptor alternative.
    */
    fn retains_lifetime_dependency(
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        descriptor: Handle<BorrowLoanFact>,
        debt: Handle<BorrowLoanFact>,
    ) -> bool {
        let borrow = &facts.borrow;
        if !borrow.loans.is_valid(descriptor) || !borrow.loans.is_valid(debt) {
            return false;
        }
        if borrow.loans.get(descriptor).lineage == BorrowLoanLineage::SuppliedOutput
            && borrow.supplied_output_call(descriptor).is_none()
        {
            return false;
        }
        let descriptor = borrow.loans.get(descriptor);
        let debt = borrow.loans.get(debt);
        descriptor.lineage != BorrowLoanLineage::LifetimeDependency
            && descriptor.root.is_valid()
            && (!matches!(descriptor.root, facts::PlaceRoot::InvocationOutput(_))
                || descriptor.incoming_reference.is_valid())
            && debt.lineage == BorrowLoanLineage::LifetimeDependency
            && same_carrier_occurrence(facts.view(), loan_occurrences, descriptor, debt)
    }

    fn same_carrier_occurrence(
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        left: &BorrowLoanFact,
        right: &BorrowLoanFact,
    ) -> bool {
        let borrow = &facts.borrow;
        let path = borrow.loan_owner_path(right);
        (same_carrier_generation(left, right)
            || same_resolved_descriptor_load(facts, loan_occurrences, left, right))
            && borrow.loan_owner_path(left) == path
            && !path.contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex)
    }

    /*
        Repeated reads of one unchanged input slot retain that descriptor's
        occurrence. The completed source identifies the slot; expression-owned
        temporary homes remain independent of the input's field path.
    */
    fn same_resolved_descriptor_load(
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        left: &BorrowLoanFact,
        right: &BorrowLoanFact,
    ) -> bool {
        if !is_descriptor_load(facts.borrow, left)
            || !is_descriptor_load(facts.borrow, right)
            || left.owner_symbol != right.owner_symbol
            || left.statement_index != right.statement_index
            || left.established_by_call != right.established_by_call
        {
            return false;
        }
        let slot = |loan| loan_occurrences.descriptor_slot(loan);
        matches!((slot(left), slot(right)), (Ok(left), Ok(right)) if left == right)
    }

    fn is_descriptor_load(borrow: &BorrowFacts, loan: &BorrowLoanFact) -> bool {
        borrow.incoming_references.is_valid(loan.incoming_reference)
            && matches!(
                borrow
                    .incoming_references
                    .get(loan.incoming_reference)
                    .origin,
                checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
            )
    }

    fn same_carrier_generation(left: &BorrowLoanFact, right: &BorrowLoanFact) -> bool {
        right.owner_symbol.is_valid()
            && right.storage_owner.is_valid()
            && left.owner_symbol == right.owner_symbol
            && left.storage_owner == right.storage_owner
            && left.statement_index == right.statement_index
            && left.established_by_call == right.established_by_call
    }

    /*
        Formation may retain a descriptor together with its existing lifetime
        obligations. Distinct cases of that same saved carrier cannot be present
        together. Neither judgment supplies an address or permits a later read.
    */
    pub(in crate::checks::borrows) fn formation_preserves_loan(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &StateBorrowFact,
        lineage: &[Handle<BorrowLoanFact>],
        active: Handle<BorrowLoanFact>,
    ) -> bool {
        lineage.iter().any(|ancestor| {
            same_descriptor(facts.view(), loan_occurrences, *ancestor, active)
                || retains_lifetime_dependency(facts, loan_occurrences, *ancestor, active)
                || retains_lifetime_dependency(facts, loan_occurrences, active, *ancestor)
                || exclusive_carrier_cases(program, &facts.borrow, state, *ancestor, active)
        })
    }

    fn exclusive_carrier_cases(
        program: &dyn validation::CheckingInput,
        borrow: &BorrowFacts,
        state: &StateBorrowFact,
        left: Handle<BorrowLoanFact>,
        right: Handle<BorrowLoanFact>,
    ) -> bool {
        let exclusive = (|| {
            if !borrow.state_owns_loan(state, left) || !borrow.state_owns_loan(state, right) {
                return None;
            }
            let left = borrow.loans.get(left);
            let right = borrow.loans.get(right);
            if !same_carrier_generation(left, right) {
                return None;
            }
            let owner = left.storage_owner.as_symbol()?;
            let state = program.state_by_symbol(state.state_symbol)?;
            let reference = validation::state_root_type(program, state, owner)?;
            let left = crate::borrow::named_carrier_path(
                program,
                state,
                owner,
                reference,
                borrow.owner_segments.span(left.owner_path)?,
            )?;
            let right = crate::borrow::named_carrier_path(
                program,
                state,
                owner,
                reference,
                borrow.owner_segments.span(right.owner_path)?,
            )?;
            for (left, right) in left.iter().zip(&right) {
                if left != right {
                    return Some(matches!(
                        (left, right),
                        (
                            checked_trees::CheckedUnitStructuralPathSegment::Case(_),
                            checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                        )
                    ));
                }
            }
            None
        })();
        exclusive == Some(true)
    }

    pub(in crate::checks::borrows) struct ResolvedPlace {
        pub(in crate::checks::borrows) place: CapturedPlace,
        pub(in crate::checks::borrows) local_loan: Handle<BorrowLoanFact>,
        /*
            Prefix consumed at the descriptor's source slot. A loaded pointer's
            own home may be empty even when that slot lies inside an input record.
        */
        pub(in crate::checks::borrows) consumed_segments: usize,
    }

    pub(in crate::checks::borrows) fn resolve_place(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
        place: CapturedPlace,
    ) -> Option<ResolvedPlace> {
        let mut places = resolve_places(
            program,
            facts,
            loan_occurrences,
            state_flow,
            entry_constraints,
            place,
        )?;
        (places.len() == 1).then(|| places.remove(0))
    }

    pub(in crate::checks::borrows) fn resolve_places(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
        place: CapturedPlace,
    ) -> Option<Vec<ResolvedPlace>> {
        resolve_places_with_loans(
            program,
            facts.view(),
            loan_occurrences,
            state_flow,
            facts.flow.borrow_loan_constraints(entry_constraints),
            place,
        )
    }

    /*
        The caller supplies its exact current occurrence roster, either a retained
        flow cut or the shared statement replay cursor. No geometry selects a loan.
    */
    pub(in crate::checks::borrows) fn resolve_places_with_loans(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
        place: CapturedPlace,
    ) -> Option<Vec<ResolvedPlace>> {
        let state = program.state_by_symbol(state_flow.state_symbol)?;
        let reference_local = crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, StatementNode::LocalData(local)
                    if facts::PlaceRoot::Symbol(local.symbol) == place.root
                        && matches!(program.type_reference_table.type_reference(local.type_reference),
                            TypeReferenceNode::Reference { .. }))
            });
        if !reference_local {
            return Some(vec![ResolvedPlace {
                place,
                local_loan: Handle::invalid(),
                consumed_segments: 0,
            }]);
        }
        resolve_descriptor_places_with_loans(
            program,
            facts,
            loan_occurrences,
            state_flow,
            loans,
            std::iter::empty(),
            place,
        )
    }

    /*
        Resolve the completed value of a reference descriptor, as distinct from
        access to the owned storage holding it. Projected stored references use the
        same captured loan occurrence and lineage as a whole reference binding.
    */
    pub(in crate::checks::borrows) fn resolve_descriptor_places(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
        place: CapturedPlace,
    ) -> Option<Vec<ResolvedPlace>> {
        resolve_descriptor_places_with_loans(
            program,
            facts.view(),
            loan_occurrences,
            state_flow,
            facts.flow.borrow_loan_constraints(entry_constraints),
            facts.flow.incoming_reference_constraints(entry_constraints),
            place,
        )
    }

    fn resolve_descriptor_places_with_loans(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        state_flow: &FlowStateFact,
        loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
        incoming: impl Iterator<Item = Handle<checked_trees::IncomingReferenceIdentity>>,
        mut place: CapturedPlace,
    ) -> Option<Vec<ResolvedPlace>> {
        let borrow = &facts.borrow;
        let (_, state_borrow) =
            borrow.first_state_borrow(state_flow.machine_symbol, state_flow.state_symbol)?;
        let carrier = place.root.as_symbol().and_then(|root| {
            validation::state_root_type(
                program,
                program.state_by_symbol(state_flow.state_symbol)?,
                root,
            )
        });
        if let Some(carrier) = carrier {
            place.segments =
                validation::corresponding_place_projection(program, carrier, &place.segments)?;
        }
        let matches_owner = |owner: &[BorrowLoanOwnerSegment]| {
            let selected;
            let owner = if let Some(carrier) = carrier {
                let Some(path) = validation::corresponding_loan_owner_path(program, carrier, owner)
                else {
                    return false;
                };
                selected = path;
                selected.as_slice()
            } else {
                owner
            };
            owner.len() <= place.segments.len()
                && owner.iter().zip(&place.segments).all(|(owner, selected)| {
                    crate::borrow::owner_segment_matches_place_segment(program, owner, selected)
                        == Some(true)
                })
        };
        let mut candidates = loans
            .filter_map(|handle| {
                let loan = borrow.loans.get(handle);
                if !borrow.state_owns_loan(state_borrow, handle)
                    || loan.lineage == BorrowLoanLineage::LifetimeDependency
                {
                    return None;
                }
                if loan.storage_owner == place.root {
                    let owner = borrow.loan_owner_path(loan);
                    return matches_owner(owner).then_some((handle, owner.len()));
                }
                let slot = loan_occurrences.descriptor_slot(loan).ok()?;
                (slot.root == place.root
                    && slot.segments.len() <= place.segments.len()
                    && slot
                        .segments
                        .iter()
                        .zip(&place.segments)
                        .all(|(left, right)| {
                            validation::storage_path_relations::canonical_place_segments_equal(
                                program, *left, *right,
                            )
                        }))
                .then_some((handle, slot.segments.len()))
            })
            .collect::<Vec<_>>();
        let incoming = incoming.collect::<Vec<_>>();
        if incoming
            .iter()
            .any(|identity| !borrow.state_owns_incoming_reference(state_borrow, *identity))
        {
            return None;
        }
        let matches_incoming = |identity| {
            let incoming = borrow.incoming_references.get(identity);
            let owner = borrow.owner_segments.span_or_empty(incoming.owner_path);
            incoming.place_root() == place.root && matches_owner(owner)
        };
        let incoming = incoming
            .into_iter()
            .filter(|identity| matches_incoming(*identity))
            .collect::<Vec<_>>();
        let depth = candidates
            .iter()
            .map(|(_, depth)| *depth)
            .chain(incoming.iter().map(|identity| {
                borrow.incoming_references.get(*identity).owner_path.count() as usize
            }))
            .max();
        let Some(depth) = depth else {
            let state = program.state_by_symbol(state_flow.state_symbol)?;
            let root_parameter = program.state_parameters(state).iter().any(|parameter| {
                if place.root != facts::PlaceRoot::Symbol(parameter.symbol)
                    || parameter.is_const
                    || parameter.relevance.is_erased()
                {
                    return false;
                }
                let Some(reference) =
                    validation::reference_shell(program, parameter.type_reference)
                else {
                    return false;
                };
                validation::owned_projection_type_reference(
                    program,
                    reference.referee,
                    &place.segments,
                )
                .is_some_and(|selected| validation::reference_shell(program, selected).is_none())
            });
            return root_parameter.then_some(vec![ResolvedPlace {
                place,
                local_loan: Handle::invalid(),
                consumed_segments: 0,
            }]);
        };
        candidates.retain(|(_, candidate_depth)| *candidate_depth == depth);
        let mut places: Vec<_> = candidates
            .into_iter()
            .map(|(handle, _)| {
                let loan = borrow.loans.get(handle);
                if !loan.root.is_valid() {
                    return None;
                }
                let mut segments = borrow.loan_segments(loan).to_vec();
                segments.extend_from_slice(&place.segments[depth..]);
                Some(ResolvedPlace {
                    place: CapturedPlace {
                        incoming_reference: loan.incoming_reference,
                        incoming_owner_path: facts
                            .borrow
                            .access_segments
                            .span_or_empty(loan.incoming_owner_path)
                            .to_vec(),
                        root: loan.root,
                        segments,
                    },
                    local_loan: handle,
                    consumed_segments: depth,
                })
            })
            .collect::<Option<_>>()?;
        for identity in incoming {
            let incoming = borrow.incoming_references.get(identity);
            if incoming.owner_path.count() as usize == depth {
                places.push(ResolvedPlace {
                    place: CapturedPlace {
                        incoming_reference: identity,
                        incoming_owner_path: Vec::new(),
                        root: incoming.place_root(),
                        segments: place.segments[depth..].to_vec(),
                    },
                    local_loan: Handle::invalid(),
                    consumed_segments: depth,
                });
            }
        }
        (!places.is_empty()).then_some(places)
    }

    /*
        Join an unchanged selected slot from the completed-storage owner to its
        actual stored entry descriptor. This includes the inner `&Cell` of an
        `&mut &Cell` input: the outer borrowed grant has a different origin even
        though both owner paths are empty. Only a completed-storage Incoming
        selection may use this join; a declared path alone is no current value.
    */
    pub(in crate::checks::borrows) fn unchanged_incoming_leaf(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
        source: &validation::CanonicalPlace,
    ) -> Result<Option<Vec<ResolvedAlias>>, &'static str> {
        let borrow = &facts.borrow;
        let (_, state_borrow) = borrow
            .first_state_borrow(state.machine_symbol, state.state_symbol)
            .ok_or("the incoming descriptor has no owning borrow state")?;
        let mut resolved = Vec::new();
        for identity in facts.flow.incoming_reference_constraints(constraints) {
            if !borrow.state_owns_incoming_reference(state_borrow, identity) {
                return Err("the current descriptor roster contains a foreign incoming identity");
            }
            let incoming = borrow.incoming_references.get(identity);
            let owner = borrow
                .owner_segments
                .span(incoming.owner_path)
                .ok_or("the incoming descriptor has an incomplete owner path")?;
            if incoming.place_root() != source.root
                || !matches!(
                    incoming.origin,
                    checked_trees::IncomingReferenceOrigin::Parameter { .. }
                )
            {
                continue;
            }
            let Some(segments) = validation::corresponding_place_projection(
                program,
                incoming.carrier_type,
                &source.segments,
            ) else {
                continue;
            };
            if !crate::borrow::owner_path_selects_leaf(program, owner, &segments) {
                continue;
            }
            let selected = validation::owned_projection_type_reference(
                program,
                incoming.carrier_type,
                &segments,
            )
            .ok_or("the selected incoming descriptor has no typed owner projection")?;
            validation::reference_shell(program, selected)
                .ok_or("the incoming selection is not a descriptor")?;
            resolved.push(ResolvedAlias {
                place: CapturedPlace {
                    incoming_reference: identity,
                    incoming_owner_path: if owner.contains(&BorrowLoanOwnerSegment::DynamicIndex) {
                        segments
                    } else {
                        Vec::new()
                    },
                    root: incoming.place_root(),
                    segments: Vec::new(),
                },
                lineage: Vec::new(),
            });
        }
        Ok((!resolved.is_empty()).then_some(resolved))
    }
}
pub(in crate::checks) use aliases::transport_suppliers;
mod completed_backings {
    /*
        Authority of a completed descriptor follows its actual value producer.

        Stored descriptors retain their exact loan occurrences at the evaluated
        cut. Normal calls continue the full declared lifetime frontier of their
        completed actuals; selections retain every alternative. These are retained
        access footprints, which may conservatively cover more than the pointer's
        exact address. Missing or coarse address geometry supplies no authority and
        cannot narrow that frontier, but does not erase the actual descriptor value.
        Physical descriptor loads still use the typed, current ingress judgment.
        Copied descriptors follow their earlier producer rather than rereading a
        source slot that later operands may have replaced.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::{
        CapturedPlace, CheckFacts, FlowOperandCompletion, FlowOperandFact, FlowStateFact,
    };
    use facts::PlaceSegment;
    use typed_trees::expression::ExpressionNode;

    use super::aliases::ResolvedAlias;

    pub(super) struct CompletedDescriptorBackings {
        pub(super) places: Vec<ResolvedAlias>,
        /*
            Actual reference operands whose own referent is forwarded through the
            authenticated invocation/result relation. Stored child descriptors and
            unrelated input lifetimes are not this temporary invocation access.
        */
        pub(super) forwarded_inputs: Vec<Handle<FlowOperandFact>>,
    }

    /*
        A selected incoming alternative keeps its original entry descriptor even
        when a conditional store retires that identity from the current roster.
        Callers have selected this exact Incoming member on the immutable storage
        timeline. Resolve its entry namespace without reactivating current access;
        the reference used to reach the slot cannot supply that permission.
    */
    pub(in crate::checks::borrows) fn incoming_sources(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement_index: usize,
        constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
        source: validation::CanonicalPlace,
    ) -> Result<Vec<ResolvedAlias>, &'static str> {
        if let Some(places) = super::aliases::unchanged_incoming_leaf(
            program,
            facts,
            state,
            state.entry_constraints,
            &source,
        )? {
            return Ok(places);
        }
        super::access_sources::resolve_descriptor_all(
            program,
            facts,
            loan_occurrences,
            state,
            statement_index,
            constraints,
            CapturedPlace {
                root: source.root,
                segments: source.segments,
                ..Default::default()
            },
        )
        .ok_or("the incoming descriptor has no retained loan occurrence")
    }

    pub(super) fn resolve(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        capture: Handle<FlowOperandFact>,
    ) -> Result<CompletedDescriptorBackings, &'static str> {
        resolve_at_path(program, facts, loan_occurrences, state, capture, &[])
    }

    pub(super) fn resolve_at_path(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        capture: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
    ) -> Result<CompletedDescriptorBackings, &'static str> {
        let descriptor = if path.is_empty() {
            descriptor_source(program, facts, capture)?
        } else if facts.flow.control.operands.is_valid(capture) {
            capture
        } else {
            return Err("the completed projection has no value occurrence");
        };
        let mut forwarded_inputs = Vec::new();
        let resolved = Resolver {
            program,
            facts,
            loan_occurrences,
            state,
        }
        .retained_descriptor(descriptor, path, &mut Vec::new(), &mut forwarded_inputs)?;
        Ok(CompletedDescriptorBackings {
            places: resolved.places,
            forwarded_inputs,
        })
    }

    /*
        Identify the exact descriptor of a completed value observation. Dispatch
        and resolution use this same occurrence check; declared value shape alone
        does not distinguish a carried reference from an ordinary storage read.
    */
    pub(super) fn descriptor_source(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        capture: Handle<FlowOperandFact>,
    ) -> Result<Handle<FlowOperandFact>, &'static str> {
        let control = &facts.flow.control;
        if !control.operands.is_valid(capture) {
            return Err("the completed access has no value occurrence");
        }
        let operand = control.operands.get(capture);
        if validation::reference_shell(program, operand.type_reference).is_some() {
            Ok(capture)
        } else {
            value_observation_descriptor_source(program, facts, capture)
        }
    }

    /*
        A contextual value observation detached its referee bytes, while retaining
        the exact descriptor that was read. A newly formed live view instead owns
        its own geometry: its bound reference type or receiver backing cannot turn
        it into that value-observation operation.
    */
    pub(super) fn value_observation_descriptor_source(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        capture: Handle<FlowOperandFact>,
    ) -> Result<Handle<FlowOperandFact>, &'static str> {
        let control = &facts.flow.control;
        if !control.operands.is_valid(capture) {
            return Err("the completed access has no value occurrence");
        }
        let operand = control.operands.get(capture);
        if crate::flow::operand_binds_live_view(program, operand.type_reference) {
            return Err(
                "the completed access retains a live view, not a detached value observation",
            );
        }
        if !control.operands.is_valid(operand.backing) {
            return Err("the completed observation has no retained descriptor source");
        }
        let source = control.operands.get(operand.backing);
        if source.expression != operand.expression
            || source.state_symbol != operand.state_symbol
            || source.statement_index != operand.statement_index
            || !validation::reference_shell(program, source.type_reference)
                .is_some_and(|reference| reference.access.is_readable())
        {
            return Err("the completed observation has no exact readable descriptor source");
        }
        Ok(operand.backing)
    }

    /*
        Complete authority alternatives. Image storage has intrinsic shared
        formation, with no fabricated caller address or borrowed-loan lineage.
    */
    #[derive(Default)]
    struct BackingFrontier {
        places: Vec<ResolvedAlias>,
        immutable_image: bool,
    }

    impl BackingFrontier {
        fn borrowed(places: Vec<ResolvedAlias>) -> Self {
            Self {
                places,
                immutable_image: false,
            }
        }

        fn append(&mut self, other: Self) {
            append_distinct(&mut self.places, other.places);
            self.immutable_image |= other.immutable_image;
        }
    }

    struct Resolver<'input> {
        program: &'input dyn validation::CheckingInput,
        facts: &'input CheckFacts,
        loan_occurrences: &'input LoanOccurrences<'input>,
        state: &'input FlowStateFact,
    }

    impl Resolver<'_> {
        /*
            Resolve the complete retained footprint through the actual value graph.
            Type, state, occurrence, source path and loan lineage remain mandatory;
            geometric precision never replaces them or invents a narrower grant.
        */
        fn retained_descriptor(
            &self,
            capture: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            ancestors: &mut Vec<Handle<FlowOperandFact>>,
            forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Result<BackingFrontier, &'static str> {
            let control = &self.facts.flow.control;
            if !control.operands.is_valid(capture) || ancestors.contains(&capture) {
                return Err("the completed descriptor graph is missing or cyclic");
            }
            let operand = control.operands.get(capture);
            if operand.state_symbol != self.state.state_symbol {
                return Err("the completed descriptor belongs to another state");
            }
            let reference = validation::project_type_reference_from_segments(
                self.program,
                operand.type_reference,
                path,
            )
            .ok_or("the completed descriptor path has no declared type")?;
            if validation::reference_shell(self.program, reference).is_none() {
                return Err("the completed value path is not a reference descriptor");
            }
            ancestors.push(capture);
            let resolved = self.producer(capture, path, ancestors, forwarded_inputs);
            ancestors.pop();
            let resolved = resolved?;
            if resolved.places.is_empty() && !resolved.immutable_image {
                return Err("the completed descriptor has no retained authority frontier");
            }
            if resolved.immutable_image
                && validation::reference_shell(self.program, reference).is_none_or(|reference| {
                    reference.access != language_semantics::ReferenceAccess::Shared
                })
            {
                return Err("immutable image storage cannot grant writable access");
            }
            Ok(resolved)
        }

        fn producer(
            &self,
            capture: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            ancestors: &mut Vec<Handle<FlowOperandFact>>,
            forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Result<BackingFrontier, &'static str> {
            let control = &self.facts.flow.control;
            let operand = control.operands.get(capture);
            match operand.completion {
                FlowOperandCompletion::IncomingContents { .. } => {
                    let frontier = validation::completed_storage::reference_sources_at_path(
                        self.program,
                        control,
                        capture,
                        path,
                        operand.storage_snapshot,
                    );
                    return self.stored_frontier(frontier, operand, ancestors, forwarded_inputs);
                }
                FlowOperandCompletion::Selected(alternatives) => {
                    let alternatives = control
                        .operand_alternatives
                        .span(alternatives)
                        .ok_or("the completed selection has no alternative roster")?;
                    if alternatives.is_empty() {
                        return Err("the completed selection has no incoming descriptor");
                    }
                    let mut resolved = BackingFrontier::default();
                    for alternative in alternatives {
                        if !alternative.source_arm.is_valid() {
                            return Err("the completed selection has an unauthenticated arm");
                        }
                        resolved.append(self.retained_descriptor(
                            alternative.value,
                            path,
                            ancestors,
                            forwarded_inputs,
                        )?);
                    }
                    return Ok(resolved);
                }
                FlowOperandCompletion::Copied { copy, path: prefix } => {
                    if !control.copied_values.is_valid(copy) {
                        return Err("the completed copy has no source value");
                    }
                    let mut projection = control
                        .operand_child_paths
                        .span(prefix)
                        .ok_or("the completed copy has no source projection")?
                        .to_vec();
                    projection.extend_from_slice(path);
                    return self.retained_descriptor(
                        control.copied_values.get(copy).source,
                        &projection,
                        ancestors,
                        forwarded_inputs,
                    );
                }
                _ => {}
            }
            if path.is_empty()
                && matches!(
                    operand.descriptor_permission,
                    checked_trees::FlowDescriptorPermission::SuppliedOutput { .. }
                )
            {
                let handle = crate::borrow::supplied_outputs::captured_permission(
                    self.program,
                    &self.facts.borrow,
                    &self.facts.flow,
                    operand,
                )?;
                let loan = self.facts.borrow.loans.get(handle);
                return Ok(BackingFrontier::borrowed(vec![ResolvedAlias {
                    place: CapturedPlace {
                        incoming_reference: loan.incoming_reference,
                        incoming_owner_path: self
                            .facts
                            .borrow
                            .access_segments
                            .span(loan.incoming_owner_path)
                            .ok_or("the produced output lost its incoming path")?
                            .to_vec(),
                        root: loan.root,
                        segments: self.facts.borrow.loan_segments(loan).to_vec(),
                    },
                    lineage: vec![handle],
                }]));
            }
            if path.is_empty() && operand.descriptor_permission.loaded_loan().is_valid() {
                let handle = operand.descriptor_permission.loaded_loan();
                let (_, owner) = self
                    .facts
                    .borrow
                    .first_state_borrow(self.state.machine_symbol, self.state.state_symbol)
                    .ok_or("the loaded descriptor has no borrow owner")?;
                if !self.facts.borrow.state_owns_loan(owner, handle) {
                    return Err("the loaded descriptor carries a foreign permission");
                }
                let loan = self.facts.borrow.loans.get(handle);
                if !self
                    .facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                {
                    return Err("the loaded descriptor has no supplied pointer identity");
                }
                let incoming = self
                    .facts
                    .borrow
                    .incoming_references
                    .get(loan.incoming_reference);
                if !matches!(incoming.origin,
                    checked_trees::IncomingReferenceOrigin::DescriptorLoad { expression }
                        if expression == operand.expression)
                    || incoming.state_symbol != operand.state_symbol
                    || loan.statement_index != operand.statement_index
                    || !self
                        .facts
                        .flow
                        .borrow_loan_constraints(operand.constraints)
                        .any(|active| active == handle)
                    || !control.case_accesses.is_valid(operand.case_access)
                {
                    return Err("the loaded descriptor has no exact completed permission");
                }
                let access = control.case_accesses.get(operand.case_access);
                if access.expression != operand.expression
                    || access.state_symbol != operand.state_symbol
                    || access.statement_index != operand.statement_index
                    || self
                        .facts
                        .flow
                        .borrow_loan_constraints(access.constraints)
                        .any(|active| active == handle)
                {
                    return Err("the loaded descriptor cannot authorize its own formation");
                }
                let mut activations = self
                    .facts
                    .flow
                    .borrow_lifetimes
                    .activations
                    .span_or_empty(self.state.borrow_activations)
                    .iter()
                    .filter(|activation| activation.loan == handle);
                let activation = activations
                    .next()
                    .ok_or("the loaded descriptor has no completed activation")?;
                if activations.next().is_some()
                    || !control.operands.is_valid(activation.formation_value)
                    || !control
                        .value_observations
                        .is_valid(operand.descriptor_permission.loaded_read())
                {
                    return Err("the loaded descriptor has an ambiguous activation or source read");
                }
                let formed = control.operands.get(activation.formation_value);
                let read = control
                    .value_observations
                    .get(operand.descriptor_permission.loaded_read());
                if formed.descriptor_permission.loaded_loan() != handle
                    || formed.descriptor_permission.loaded_read()
                        != operand.descriptor_permission.loaded_read()
                    || formed.expression != operand.expression
                    || formed.state_symbol != operand.state_symbol
                    || formed.statement_index != operand.statement_index
                    || formed.backing != operand.backing
                    || formed.case_access != operand.case_access
                    || formed.retention != operand.retention
                    || formed.referents != operand.referents
                    || read.expression != operand.expression
                    || read.evaluation != access.evaluation
                    || read.receiver != operand.backing
                    || read.constraints != access.constraints
                    || read.places.is_empty()
                    || self
                        .facts
                        .flow
                        .borrow_loan_constraints(read.constraints)
                        .any(|active| active == handle)
                    || !validation::declared_carriers_match(
                        self.program,
                        incoming.carrier_type,
                        formed.type_reference,
                    )
                    || crate::borrow::reference_borrow_access_kind(
                        self.program,
                        formed.type_reference,
                    )
                    .as_ref()
                        != Some(&loan.kind)
                    || control.retained_roots(formed.retention).is_none()
                {
                    return Err(
                        "the loaded descriptor disagrees with its completed read, carrier or lifetime",
                    );
                }
                return Ok(BackingFrontier::borrowed(vec![ResolvedAlias {
                    place: CapturedPlace {
                        incoming_reference: loan.incoming_reference,
                        root: loan.root,
                        segments: self.facts.borrow.loan_segments(loan).to_vec(),
                        ..Default::default()
                    },
                    lineage: vec![handle],
                }]));
            }
            let coordinate = place_at(
                self.program,
                self.state.state_symbol,
                operand.statement_index,
                operand.expression,
            );
            if let Some(profiles) =
                validation::completed_storage::captured_retention_profiles_at_path(
                    self.program,
                    control,
                    operand,
                    path,
                )
                && !profiles.is_empty()
                && profiles
                    .iter()
                    .all(|profile| self.immutable_image_profile(*profile, &mut Vec::new()))
            {
                return Ok(BackingFrontier {
                    places: Vec::new(),
                    immutable_image: true,
                });
            }
            if !matches!(
                self.program.expression_table.expression(operand.expression),
                ExpressionNode::Borrow(_)
            ) && let Some(source) = &coordinate
                && source.root.as_symbol().is_some()
            {
                return self.stored(capture, operand, source, path, ancestors, forwarded_inputs);
            }
            if matches!(
                self.program.expression_table.expression(operand.expression),
                ExpressionNode::SavedSubject(_)
            ) {
                let source = control
                    .operands
                    .is_valid(operand.backing)
                    .then(|| control.operands.get(operand.backing))
                    .ok_or("the saved descriptor has no completed subject")?;
                if source.state_symbol != operand.state_symbol
                    || source.statement_index != operand.statement_index
                    || self.program.normalized_type_identity(source.type_reference)
                        != self
                            .program
                            .normalized_type_identity(operand.type_reference)
                {
                    return Err("the saved descriptor disagrees with its completed subject");
                }
                return self.retained_descriptor(
                    operand.backing,
                    path,
                    ancestors,
                    forwarded_inputs,
                );
            }
            if let Some(selected) = &coordinate
                && control.operands.is_valid(operand.backing)
            {
                let receiver = control.operands.get(operand.backing);
                if receiver.state_symbol != operand.state_symbol
                    || receiver.statement_index != operand.statement_index
                {
                    return Err("the descriptor projection has another receiver occurrence");
                }
                if !crate::flow::operand_binds_live_view(self.program, receiver.type_reference)
                    && let Some(source) = place_at(
                        self.program,
                        self.state.state_symbol,
                        receiver.statement_index,
                        receiver.expression,
                    )
                    && source.root == selected.root
                    && source.segments.len() < selected.segments.len()
                    && same_path(
                        self.program,
                        &source.segments,
                        &selected.segments[..source.segments.len()],
                    )
                {
                    let mut projection = selected.segments[source.segments.len()..].to_vec();
                    let reference = validation::project_type_reference_from_segments(
                        self.program,
                        receiver.type_reference,
                        &projection,
                    )
                    .ok_or("the descriptor projection has no typed receiver suffix")?;
                    if self.program.normalized_type_identity(reference)
                        != self
                            .program
                            .normalized_type_identity(operand.type_reference)
                    {
                        return Err(
                            "the descriptor projection disagrees with its completed receiver",
                        );
                    }
                    projection.extend_from_slice(path);
                    return self.retained_descriptor(
                        operand.backing,
                        &projection,
                        ancestors,
                        forwarded_inputs,
                    );
                }
            }
            if let FlowOperandCompletion::Constructed(_) = operand.completion
                && !path.is_empty()
            {
                let children = validation::completed_storage::captured_operand_children_at_path(
                    self.program,
                    control,
                    capture,
                    path,
                )
                .ok_or("the constructed descriptor has no complete evaluated child frontier")?;
                let mut resolved = BackingFrontier::default();
                for child in children {
                    resolved.append(self.retained_descriptor(
                        child.capture,
                        &child.projection,
                        ancestors,
                        forwarded_inputs,
                    )?);
                }
                return Ok(resolved);
            }
            if let FlowOperandCompletion::InvocationResult {
                invocation,
                result_type,
                path: output,
            } = operand.completion
            {
                if !path.is_empty() {
                    return Err(
                        "a returned descriptor observation does not capture pointee contents",
                    );
                }
                let output = control
                    .operand_retained_paths
                    .span(output)
                    .ok_or("the returned descriptor has no result coordinate")?;
                let selected = validation::project_type_reference_from_segments(
                    self.program,
                    result_type,
                    output,
                )
                .ok_or("the returned descriptor coordinate has no declared type")?;
                if self.program.normalized_type_identity(selected)
                    != self
                        .program
                        .normalized_type_identity(operand.type_reference)
                {
                    return Err("the returned descriptor disagrees with its completed result leaf");
                }
                return self.invocation_result(
                    operand,
                    invocation,
                    result_type,
                    output,
                    ancestors,
                    forwarded_inputs,
                );
            }
            if matches!(
                self.program.expression_table.expression(operand.expression),
                ExpressionNode::Call(_)
            ) {
                let mut calls = self.state.calls.handles().filter(|handle| {
                    let row = control.calls.get(*handle);
                    row.statement_index == operand.statement_index
                        && row.authored_expression == operand.expression
                });
                let invocation = calls
                    .next()
                    .ok_or("the completed result has no actual invocation")?;
                if calls.next().is_some() {
                    return Err("the completed result has no unique checked invocation");
                }
                return self.invocation_result(
                    operand,
                    invocation,
                    operand.type_reference,
                    path,
                    ancestors,
                    forwarded_inputs,
                );
            }
            if matches!(
                self.program.expression_table.expression(operand.expression),
                ExpressionNode::Borrow(_)
            ) {
                let formation = super::formation_captures::reference_formation(
                    self.program,
                    self.facts,
                    capture,
                )?
                .ok_or("the completed borrow has no exact formation receiver")?;
                if formation.storage && path.is_empty() {
                    return super::access_sources::stored_address(
                        self.program,
                        self.facts,
                        self.loan_occurrences,
                        self.state,
                        formation.target,
                        true,
                    )
                    .map(BackingFrontier::borrowed);
                }
                let source = coordinate
                    .ok_or("the completed descriptor has no source storage coordinate")?;
                let mut place = CapturedPlace {
                    root: source.root,
                    segments: source.segments,
                    ..Default::default()
                };
                place.segments.extend_from_slice(path);
                super::access_sources::resolve_captured_all(
                    self.program,
                    self.facts.view(),
                    self.loan_occurrences,
                    self.state,
                    operand.statement_index,
                    operand.constraints,
                    place,
                )
                .map(BackingFrontier::borrowed)
                .ok_or("the completed borrow has no retained source authority")
            } else {
                Err("the computed descriptor has no retained authority producer")
            }
        }

        /*
            Both an evaluated call and its retained result-leaf observations name
            the same performed invocation. The normal return contract selects the
            actual captured inputs; lifetime debt and exclusion regions alone never
            become a descriptor supplier.
        */
        fn invocation_result(
            &self,
            operand: &FlowOperandFact,
            invocation: Handle<checked_trees::FlowCallFact>,
            result_type: typed_trees::types::TypeReferenceHandle,
            path: &[PlaceSegment],
            ancestors: &mut Vec<Handle<FlowOperandFact>>,
            forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Result<BackingFrontier, &'static str> {
            let control = &self.facts.flow.control;
            if !control.calls.is_valid(invocation)
                || !self
                    .state
                    .calls
                    .handles()
                    .any(|candidate| candidate == invocation)
            {
                return Err("the completed result invocation belongs to another state");
            }
            let invocation = control.calls.get(invocation);
            let ExpressionNode::Call(call) = self
                .program
                .expression_table
                .expression(invocation.authored_expression)
            else {
                return Err("the completed result has no authored call occurrence");
            };
            if invocation.statement_index != operand.statement_index
                || invocation.authored_target != call.target_symbol
            {
                return Err("the completed result disagrees with its checked invocation");
            }
            if let Some(permission) = super::resources::invocation_permissions::resolve(
                self.program,
                self.facts,
                self.state,
                invocation,
                operand,
                result_type,
                path,
            ) {
                let handle = permission?;
                let loan = self.facts.borrow.loans.get(handle);
                return Ok(BackingFrontier::borrowed(vec![ResolvedAlias {
                    place: CapturedPlace {
                        incoming_reference: loan.incoming_reference,
                        incoming_owner_path: self
                            .facts
                            .borrow
                            .access_segments
                            .span_or_empty(loan.incoming_owner_path)
                            .to_vec(),
                        root: loan.root,
                        segments: self.facts.borrow.loan_segments(loan).to_vec(),
                    },
                    lineage: vec![handle],
                }]));
            }
            let sources = crate::borrow::completed_call_reference_sources(
                self.program,
                self.state.state_symbol,
                call,
            )
            .ok_or("the completed call has no declared return-source relation")?;
            let mut resolved = BackingFrontier::default();
            for source in sources {
                let (source, projection) = match source {
                    crate::borrow::CompletedReturnSource::ImmutableImage { owner_path, kind } => {
                        if kind != checked_trees::BorrowAccessKind::Read {
                            return Err("immutable image storage cannot grant writable access");
                        }
                        let structural =
                            crate::borrow::structural_owner_path(self.program, &owner_path)
                                .ok_or("the image result has no declared output path")?;
                        let output = validation::reference_result_custody::loan_place_segments(
                            self.program,
                            result_type,
                            &structural,
                        )
                        .ok_or("the image result path disagrees with its completed carrier")?;
                        if same_path(self.program, &output, path) {
                            resolved.immutable_image = true;
                        }
                        continue;
                    }
                    crate::borrow::CompletedReturnSource::DescriptorCandidate {
                        field,
                        projection,
                    } => (field, projection),
                    crate::borrow::CompletedReturnSource::LifetimeDependency { .. } => continue,
                    crate::borrow::CompletedReturnSource::UnresolvedDescriptor { .. } => {
                        return Err("the completed call has an unresolved descriptor alternative");
                    }
                };
                let structural =
                    crate::borrow::structural_owner_path(self.program, &source.owner_path)
                        .ok_or("the returned descriptor has no declared output path")?;
                let output = validation::reference_result_custody::loan_place_segments(
                    self.program,
                    result_type,
                    &structural,
                )
                .ok_or("the returned descriptor path disagrees with its completed carrier")?;
                if !same_path(self.program, &output, path) {
                    continue;
                }
                let actuals = invocation.argument_operands();
                let index = u32::try_from(source.parameter_index)
                    .map_err(|_| "the returned descriptor's input coordinate is out of range")?;
                if index >= actuals.count() {
                    return Err("the returned descriptor has no completed input");
                }
                let input = crate::borrow::owner_place_segments(&source.source_path);
                let mut backings = self.input_descriptor(
                    actuals.handle_at(index),
                    source.source,
                    &input,
                    ancestors,
                    forwarded_inputs,
                )?;
                for backing in &mut backings.places {
                    backing.place.segments =
                        facts::project_captured_view_segments(&backing.place.segments, &projection)
                            .ok_or("the returned descriptor cannot project its retained source")?;
                }
                resolved.append(backings);
            }
            (!resolved.places.is_empty() || resolved.immutable_image)
                .then_some(resolved)
                .ok_or("the completed call does not cover the returned descriptor path")
        }

        fn immutable_image_profile(
            &self,
            profile: Handle<checked_trees::FlowOperandRetention>,
            ancestors: &mut Vec<Handle<checked_trees::FlowOperandRetention>>,
        ) -> bool {
            let control = &self.facts.flow.control;
            if !control.operand_retentions.is_valid(profile) || ancestors.contains(&profile) {
                return false;
            }
            let retained = control.operand_retentions.get(profile);
            ancestors.push(profile);
            let image = if !retained.alternatives.is_empty() {
                control
                    .operand_retentions
                    .span(retained.alternatives)
                    .is_some_and(|alternatives| {
                        !alternatives.is_empty()
                            && retained.alternatives.handles().all(|alternative| {
                                self.immutable_image_profile(alternative, ancestors)
                            })
                    })
            } else if control.operands.is_valid(retained.descriptor) {
                self.immutable_image_profile(
                    control.operands.get(retained.descriptor).retention,
                    ancestors,
                )
            } else {
                retained.immutable_image
            };
            ancestors.pop();
            image
        }

        fn stored(
            &self,
            capture: Handle<FlowOperandFact>,
            operand: &FlowOperandFact,
            source: &validation::CanonicalPlace,
            path: &[PlaceSegment],
            ancestors: &mut Vec<Handle<FlowOperandFact>>,
            forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Result<BackingFrontier, &'static str> {
            let mut place = CapturedPlace {
                root: source.root,
                segments: source.segments.clone(),
                ..Default::default()
            };
            place.segments.extend_from_slice(path);
            if let Some(places) = super::access_sources::resolve_descriptor_all(
                self.program,
                self.facts,
                self.loan_occurrences,
                self.state,
                operand.statement_index,
                operand.constraints,
                place,
            ) {
                return Ok(BackingFrontier::borrowed(places));
            }
            let control = &self.facts.flow.control;
            if control.operands.is_valid(operand.backing) && operand.backing != capture {
                let receiver = control.operands.get(operand.backing);
                if receiver.state_symbol == operand.state_symbol
                    && let Some(receiver_place) = place_at(
                        self.program,
                        self.state.state_symbol,
                        receiver.statement_index,
                        receiver.expression,
                    )
                    && receiver_place.root == source.root
                    && receiver_place.segments.len() < source.segments.len()
                    && same_path(
                        self.program,
                        &receiver_place.segments,
                        &source.segments[..receiver_place.segments.len()],
                    )
                {
                    let mut projection = source.segments[receiver_place.segments.len()..].to_vec();
                    projection.extend_from_slice(path);
                    let frontier = validation::completed_storage::reference_sources_at_path(
                        self.program,
                        control,
                        operand.backing,
                        &projection,
                        operand.storage_snapshot,
                    );
                    return self.stored_frontier(frontier, operand, ancestors, forwarded_inputs);
                }
            }
            Err("the stored descriptor has no retained loan occurrence")
        }

        fn stored_frontier(
            &self,
            source: validation::completed_storage::StoredOperand,
            operand: &FlowOperandFact,
            ancestors: &mut Vec<Handle<FlowOperandFact>>,
            forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Result<BackingFrontier, &'static str> {
            use validation::completed_storage::StoredOperand;
            match source {
                StoredOperand::Unavailable => {
                    Err("the stored descriptor has no current value source")
                }
                StoredOperand::Incoming(source) => incoming_sources(
                    self.program,
                    self.facts,
                    self.loan_occurrences,
                    self.state,
                    operand.statement_index,
                    operand.constraints,
                    source,
                )
                .map(BackingFrontier::borrowed),
                StoredOperand::Value {
                    capture,
                    projection,
                    slot,
                    ..
                } => {
                    if let Some(formed) =
                        self.current_formation(capture, &projection, slot.as_ref(), operand)?
                    {
                        Ok(formed)
                    } else {
                        self.retained_descriptor(capture, &projection, ancestors, forwarded_inputs)
                    }
                }
                StoredOperand::Alternatives(alternatives) => {
                    if alternatives.is_empty() {
                        return Err("the stored descriptor has no incoming alternatives");
                    }
                    let mut result = BackingFrontier::default();
                    for alternative in alternatives {
                        result.append(self.stored_frontier(
                            alternative,
                            operand,
                            ancestors,
                            forwarded_inputs,
                        )?);
                    }
                    Ok(result)
                }
            }
        }

        /*
            The storage owner selected this exact produced descriptor. Rejoin its
            active installed occurrence before following older construction inputs:
            those inputs retain backing, but cannot exempt a current descriptor's
            own loan from interference. Every matching formation alternative stays.
        */
        fn current_formation(
            &self,
            capture: Handle<FlowOperandFact>,
            projection: &[PlaceSegment],
            slot: Option<&validation::CanonicalPlace>,
            observation: &FlowOperandFact,
        ) -> Result<Option<BackingFrontier>, &'static str> {
            let Some(slot) = slot else {
                return Ok(None);
            };
            let control = &self.facts.flow.control;
            if !control.operands.is_valid(capture) {
                return Err("the installed descriptor has no completed formation");
            }
            let source = control.operands.get(capture);
            if source.state_symbol != self.state.state_symbol {
                return Err("the installed descriptor formation belongs to another state");
            }
            let (_, owner) = self
                .facts
                .borrow
                .first_state_borrow(self.state.machine_symbol, self.state.state_symbol)
                .ok_or("the installed descriptor has no owning borrow state")?;
            let sources = validation::completed_storage::descriptor_formation_sources(
                self.program,
                control,
                capture,
                projection,
                self.state.statements,
            )
            .ok_or("the installed descriptor has no complete formation graph")?;
            let typed_state = self
                .program
                .state_by_symbol(self.state.state_symbol)
                .ok_or("the installed descriptor has no state declaration")?;
            let order = self
                .program
                .statement_table
                .admitted_execution_order(typed_state.statement_nodes);
            let homes = validation::completed_storage::InstalledLoanHomes {
                control,
                semantic: &self.facts.semantic,
                borrow: &self.facts.borrow,
                statements: self.state.statements,
                order: &order,
                state: self.state.state_symbol,
            };
            let mut candidates = Vec::new();
            for handle in self
                .facts
                .flow
                .borrow_loan_constraints(observation.constraints)
            {
                if !self.facts.borrow.state_owns_loan(owner, handle) {
                    return Err("the installed descriptor roster contains a foreign loan");
                }
                let loan = self.facts.borrow.loans.get(handle);
                if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
                    || loan.storage_owner != slot.root
                {
                    continue;
                }
                let Some(formed) = loan.formation_owner_path else {
                    continue;
                };
                let path = self
                    .facts
                    .borrow
                    .owner_segments
                    .span(formed)
                    .ok_or("the installed descriptor formation has an incomplete leaf path")?;
                if path.contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex)
                    || !sources.iter().any(|formed| {
                        let value = control.operands.get(formed.capture);
                        value.expression == loan.formation_expression
                            && value.statement_index == loan.statement_index
                            && crate::borrow::owner_path_selects_leaf(
                                self.program,
                                path,
                                &formed.projection,
                            )
                    })
                    || crate::checks::storage_path_compatibility(
                        self.program,
                        crate::checks::StorageSelectorFacts {
                            flow: self.facts.flow.view(),
                            semantic: &self.facts.semantic,
                        },
                        &crate::borrow::owner_place_segments(&homes.owner_path(self.program, loan)),
                        &slot.segments,
                    )
                    .1 != checked_trees::CapturedPlaceContainment::Same
                {
                    continue;
                }
                candidates.push(super::aliases::ResolvedPlace {
                    place: CapturedPlace {
                        incoming_reference: loan.incoming_reference,
                        incoming_owner_path: self
                            .facts
                            .borrow
                            .access_segments
                            .span_or_empty(loan.incoming_owner_path)
                            .to_vec(),
                        root: loan.root,
                        segments: self.facts.borrow.loan_segments(loan).to_vec(),
                    },
                    local_loan: handle,
                    consumed_segments: slot.segments.len(),
                });
            }
            if candidates.is_empty() {
                return Ok(None);
            }
            let coordinate = slot.clone();
            let alternatives = candidates
                .iter()
                .map(|candidate| candidate.local_loan)
                .collect::<Vec<_>>();
            let mut places = super::access_sources::retain_captured_lineage(
                self.program,
                self.facts.view(),
                self.loan_occurrences,
                self.state,
                observation.statement_index,
                CapturedPlace {
                    root: coordinate.root,
                    segments: coordinate.segments,
                    ..Default::default()
                },
                candidates,
            )
            .ok_or("the installed descriptor has incomplete formation lineage")?;
            for place in &mut places {
                for alternative in &alternatives {
                    crate::shared::collections::push_unique(&mut place.lineage, *alternative);
                }
            }
            Ok(Some(BackingFrontier::borrowed(places)))
        }

        fn input_descriptor(
            &self,
            capture: Handle<FlowOperandFact>,
            source: validation::borrow_contracts::ViewReturnInputSource,
            path: &[PlaceSegment],
            ancestors: &mut Vec<Handle<FlowOperandFact>>,
            forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
        ) -> Result<BackingFrontier, &'static str> {
            let control = &self.facts.flow.control;
            if !control.operands.is_valid(capture) {
                return Err("the return-source relation has no completed actual");
            }
            let actual = control.operands.get(capture);
            if source == validation::borrow_contracts::ViewReturnInputSource::Stored
                && validation::reference_shell(self.program, actual.type_reference).is_some()
            {
                if !control.operand_retentions.is_valid(actual.retention) {
                    return Err("the borrowed input has no captured stored-descriptor inventory");
                }
                let retained = control.operand_retentions.get(actual.retention);
                if !control.operands.is_valid(retained.stored_value) {
                    return Err("the borrowed input has no captured contents source");
                }
                let mut projection = control
                    .operand_retained_paths
                    .span(retained.stored_projection)
                    .ok_or("the borrowed input has no captured stored-descriptor projection")?
                    .to_vec();
                projection.extend_from_slice(path);
                let frontier = validation::completed_storage::reference_sources_at_path(
                    self.program,
                    control,
                    retained.stored_value,
                    &projection,
                    actual.storage_snapshot,
                );
                return self.stored_frontier(frontier, actual, ancestors, forwarded_inputs);
            }
            let resolved = self.retained_descriptor(capture, path, ancestors, forwarded_inputs)?;
            if source == validation::borrow_contracts::ViewReturnInputSource::Referent
                && path.is_empty()
                && validation::reference_shell(self.program, actual.type_reference).is_some()
                && !forwarded_inputs.contains(&capture)
            {
                forwarded_inputs.push(capture);
            }
            Ok(resolved)
        }
    }

    fn same_path(
        program: &dyn validation::CheckingInput,
        left: &[PlaceSegment],
        right: &[PlaceSegment],
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(left, right)| {
                crate::flow::canonical_place_segments_equal(program, *left, *right)
            })
    }

    fn append_distinct(
        target: &mut Vec<ResolvedAlias>,
        values: impl IntoIterator<Item = ResolvedAlias>,
    ) {
        for value in values {
            if !target
                .iter()
                .any(|existing| existing.place == value.place && existing.lineage == value.lineage)
            {
                target.push(value);
            }
        }
    }
}
mod formation_captures {
    /*
        A new view must use the reference actually read, even if its source variable
        later changes. Follow saved inputs and selected fields to that captured value;
        its access permission is checked separately.
    */
    use arena::Handle;
    use checked_trees::{CheckFacts, FlowOperandFact};

    pub(super) fn descriptor_capture(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        source: Handle<FlowOperandFact>,
    ) -> Option<Handle<FlowOperandFact>> {
        let mut capture = source;
        let mut visited = Vec::new();
        while facts.flow.control.operands.is_valid(capture) && !visited.contains(&capture) {
            visited.push(capture);
            let row = facts.flow.control.operands.get(capture);
            if validation::reference_shell(program, row.type_reference).is_some() {
                return Some(capture);
            }
            row.address_projection?;
            capture = row.backing;
        }
        None
    }

    pub(super) fn formation_capture(
        facts: &CheckFacts,
        formation: Handle<FlowOperandFact>,
        source: typed_trees::expression::ExpressionHandle,
    ) -> Option<Handle<FlowOperandFact>> {
        let mut pending = vec![formation];
        let mut visited = Vec::new();
        let mut selected = Handle::invalid();
        while let Some(capture) = pending.pop() {
            if visited.contains(&capture) || !facts.flow.control.operands.is_valid(capture) {
                continue;
            }
            visited.push(capture);
            let row = facts.flow.control.operands.get(capture);
            if row.expression == source {
                if selected.is_valid() {
                    return None;
                }
                selected = capture;
            }
            pending.extend(row.evaluated_inputs.handles());
            pending.push(row.backing);
        }
        selected.is_valid().then_some(selected)
    }

    pub(super) enum AddressReceiver {
        Reference {
            capture: Handle<FlowOperandFact>,
            projection: Vec<facts::PlaceSegment>,
        },
        InlineValue,
        Unavailable,
    }

    pub(super) fn address_receiver(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        source: Handle<FlowOperandFact>,
    ) -> AddressReceiver {
        let mut capture = source;
        let mut projection = Vec::new();
        let mut visited = Vec::new();
        while facts.flow.control.operands.is_valid(capture) && !visited.contains(&capture) {
            visited.push(capture);
            let row = facts.flow.control.operands.get(capture);
            let forms_view = matches!(
                row.address_projection,
                Some(checked_trees::FlowOperandAddressProjection::View(_))
            );
            if forms_view {
                if !facts.flow.control.operands.is_valid(row.backing) {
                    return AddressReceiver::Unavailable;
                }
                let receiver = facts.flow.control.operands.get(row.backing);
                if receiver.state_symbol != row.state_symbol
                    || receiver.statement_index != row.statement_index
                {
                    return AddressReceiver::Unavailable;
                }
            }
            if validation::reference_shell(program, row.type_reference).is_some() && !forms_view {
                return AddressReceiver::Reference {
                    capture,
                    projection,
                };
            }
            let Some(path) = row.address_projection else {
                return if row.backing.is_valid() {
                    AddressReceiver::Unavailable
                } else {
                    AddressReceiver::InlineValue
                };
            };
            let Some(path) = facts.flow.control.operand_child_paths.span(path.segments()) else {
                return AddressReceiver::Unavailable;
            };
            let mut complete = path.to_vec();
            complete.extend(projection);
            projection = complete;
            capture = row.backing;
        }
        AddressReceiver::Unavailable
    }

    struct CapturedArgument {
        call: Handle<checked_trees::FlowCallFact>,
        operand: Handle<FlowOperandFact>,
    }

    pub(super) fn actual_invocation(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &checked_trees::FlowStateFact,
        source: Handle<FlowOperandFact>,
    ) -> Result<Option<Handle<checked_trees::FlowCallFact>>, &'static str> {
        let mut selected = None;
        for argument in argument_occurrences(program, facts, state, source)? {
            if selected.is_some_and(|call| call != argument.call) {
                return Err("the saved actual belongs to multiple invocation tuples");
            }
            selected = Some(argument.call);
        }
        Ok(selected)
    }

    /*
        A reference saved by an earlier call keeps that call's actual occurrence.
        Follow evaluated children, stopping at a completed nested call: its inputs
        belong to its own invocation, not to the later call receiving its result.
    */
    fn argument_occurrences(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &checked_trees::FlowStateFact,
        source: Handle<FlowOperandFact>,
    ) -> Result<Vec<CapturedArgument>, &'static str> {
        let control = &facts.flow.control;
        if !control.operands.is_valid(source) {
            return Err("the argument source has no completed occurrence");
        }
        let captured = control.operands.get(source);
        let mut arguments = Vec::new();
        for call in state.calls.handles() {
            let row = control.calls.get(call);
            if row.statement_index != captured.statement_index {
                continue;
            }
            for operand in row.argument_operands().handles() {
                let values = evaluated_captures(program, facts, operand)?;
                if values.contains(&source) {
                    arguments.push(CapturedArgument { call, operand });
                }
            }
        }
        Ok(arguments)
    }

    pub(super) struct ArgumentFormation {
        pub(super) value: Handle<FlowOperandFact>,
        pub(super) target: Handle<FlowOperandFact>,
        pub(super) storage: bool,
    }

    pub(super) fn reference_formation(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        value: Handle<FlowOperandFact>,
    ) -> Result<Option<ArgumentFormation>, &'static str> {
        if !facts.flow.control.operands.is_valid(value) {
            return Err("the reference formation has no completed value");
        }
        let actual = facts.flow.control.operands.get(value);
        let Some(formation) = validation::reference_formation_source(program, actual.expression)
        else {
            return Ok(None);
        };
        let target = formation_capture(facts, value, formation.source)
            .ok_or("the reference formation lost its exact evaluated receiver")?;
        Ok(Some(ArgumentFormation {
            value,
            target,
            storage: formation.interpretation
                == validation::ReferenceFormationInterpretation::Declared
                && validation::borrow_selects_reference_storage(
                    program,
                    formation.source,
                    actual.type_reference,
                    &[],
                ),
        }))
    }

    /*
        Keep the endpoint that was admitted for this actual. Borrowing the pointer
        variable itself must not become an access through its current pointer.
    */
    pub(super) fn argument_formation(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &checked_trees::FlowStateFact,
        source: Handle<FlowOperandFact>,
    ) -> Result<Option<ArgumentFormation>, &'static str> {
        let mut selected = None;
        let expression = facts.flow.control.operands.get(source).expression;
        for argument in argument_occurrences(program, facts, state, source)? {
            for value in evaluated_captures(program, facts, argument.operand)? {
                let actual = facts.flow.control.operands.get(value);
                if value != source
                    && validation::reference_formation_source(program, actual.expression)
                        .is_none_or(|formation| formation.source != expression)
                {
                    continue;
                }
                let Some(formation) = reference_formation(program, facts, value)? else {
                    continue;
                };
                if value != source && formation.target != source {
                    continue;
                }
                if selected
                    .as_ref()
                    .is_some_and(|old: &ArgumentFormation| old.value != value)
                {
                    return Err("the argument source has multiple formed reference occurrences");
                }
                selected = Some(formation);
            }
        }
        Ok(selected)
    }

    fn evaluated_captures(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        root: Handle<FlowOperandFact>,
    ) -> Result<Vec<Handle<FlowOperandFact>>, &'static str> {
        let control = &facts.flow.control;
        if !control.operands.is_valid(root) {
            return Err("the actual argument has no evaluated value");
        }
        let owner = control.operands.get(root);
        let mut pending = vec![(root, false)];
        let mut seen = Vec::new();
        let mut active = Vec::new();
        let mut completed = Vec::new();
        while let Some((value, finished)) = pending.pop() {
            if finished {
                active.retain(|current| *current != value);
                completed.push(value);
                continue;
            }
            if completed.contains(&value) {
                continue;
            }
            if active.contains(&value) {
                return Err("the actual argument's evaluation graph is cyclic");
            }
            if !control.operands.is_valid(value) {
                return Err("the actual argument has an unavailable evaluated child");
            }
            let row = control.operands.get(value);
            if row.state_symbol != owner.state_symbol
                || row.statement_index != owner.statement_index
            {
                continue;
            }
            seen.push(value);
            active.push(value);
            pending.push((value, true));
            if matches!(
                row.completion,
                checked_trees::FlowOperandCompletion::InvocationResult { .. }
            ) || matches!(
                program.expression_table.expression(row.expression),
                typed_trees::expression::ExpressionNode::Call(_)
            ) {
                continue;
            }
            pending.extend(row.evaluated_inputs.handles().map(|value| (value, false)));
            if row.backing.is_valid() {
                pending.push((row.backing, false));
            }
            match row.completion {
                checked_trees::FlowOperandCompletion::Constructed(children) => pending.extend(
                    control
                        .operand_children
                        .span(children)
                        .ok_or("the actual argument has an unavailable child roster")?
                        .iter()
                        .map(|child| (child.value, false)),
                ),
                checked_trees::FlowOperandCompletion::Selected(alternatives) => pending.extend(
                    control
                        .operand_alternatives
                        .span(alternatives)
                        .ok_or("the actual argument has an unavailable alternative roster")?
                        .iter()
                        .map(|alternative| (alternative.value, false)),
                ),
                _ => {}
            }
        }
        Ok(seen)
    }
}
mod loan_occurrences {
    /*
        Source-slot associations are prepared once for an immutable pass or question.
        Every use still selects its live loans and checks access and lifetime duties.
    */
    use checked_trees::{CheckFacts, FlowStateFact};
    use validation::reference_result_custody::PreparedDescriptorLoadSlots;

    pub(super) struct LoanOccurrences<'facts> {
        slots: PreparedDescriptorLoadSlots<'facts>,
    }

    impl<'facts> LoanOccurrences<'facts> {
        pub(super) fn new(
            program: &dyn validation::CheckingInput,
            facts: &'facts CheckFacts,
        ) -> Self {
            Self {
                slots: PreparedDescriptorLoadSlots::new(program, facts),
            }
        }

        pub(super) fn for_state(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'facts>,
            state: &FlowStateFact,
        ) -> Self {
            Self {
                slots: PreparedDescriptorLoadSlots::for_state(program, facts, state),
            }
        }

        pub(super) fn descriptor_slot(
            &self,
            loan: &checked_trees::BorrowLoanFact,
        ) -> Result<&validation::CanonicalPlace, &'static str> {
            self.slots.slot(loan)
        }
    }
}
mod shared_observations {
    /*
        Rejoin a saved descriptor's content observation through its held permission.

        The source predicate is queried separately at its original completion. This
        judgment never restores an old context to the current storage namespace.
        A recorded invocation operand, exact descriptor geometry and a shared hold
        must agree. Every intervening write must fit an authenticated permission
        envelope compatible with that hold. Unknown writes and interior descriptor
        crossings refuse; observation footprints never become physical addresses.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use checked_trees::{BorrowAccessKind, CapturedPlace, CheckFacts, FlowCallFact, FlowStateFact};
    use facts::PlaceRoot;
    use validation::CanonicalPlace;

    /*
        Return only the actual earlier value capture whose requested predicate may
        be checked. Classification supplies no permission: the consuming Read and
        complete retained source authority are independently authenticated below.
    */
    pub(crate) fn shared_reference_observation<'facts>(
        program: &dyn validation::CheckingInput,
        facts: &'facts CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        captured: &crate::flow::CapturedValue<'facts>,
    ) -> Result<crate::flow::CapturedValue<'facts>, &'static str> {
        let control = &facts.flow.control;
        let observation = captured
            .observation
            .recorded_in(control)
            .ok_or("the observation has no recorded operand association")?;
        let capture = captured
            .operand
            .recorded_in(control)
            .ok_or("the value producer has no recorded operand association")?;
        let (actuals, _) =
            crate::flow::captured_call_operands(program, facts.flow.view(), state, call)
                .ok_or("the invocation has no complete actual roster")?;
        if !call.operands.handles().any(|actual| actual == observation)
            || captured.observation.state_symbol != state.state_symbol
            || captured.observation.statement_index != call.statement_index
            || actuals.iter().any(|actual| {
                actual.state_symbol != state.state_symbol
                    || actual.statement_index != call.statement_index
            })
            || actuals
                .iter()
                .filter(|actual| actual.expression == captured.observation.expression)
                .count()
                != 1
        {
            return Err("the observation is not this invocation's unique actual");
        }
        let borrow_call = facts
            .borrow
            .call_at(
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )
            .filter(|handle| facts.borrow.calls.is_valid(*handle))
            .map(|handle| facts.borrow.calls.get(handle))
            .ok_or("the invocation has no checked access roster")?;
        if borrow_call.target_symbol != call.target_symbol
            || borrow_call.authored_expression != call.authored_expression
            || borrow_call.accesses != call.accesses
        {
            return Err("the checked access roster belongs to another invocation");
        }
        let mut accesses = facts
            .borrow
            .argument_accesses
            .span(call.accesses)
            .ok_or("the invocation access span is unavailable")?
            .iter()
            .filter(|access| access.expression == captured.observation.expression);
        let access = accesses.next().ok_or("the actual has no held access")?;
        if accesses.next().is_some()
            || access.kind != BorrowAccessKind::Read
            || access.end_before_access.is_some()
            || access.released_at.is_valid()
        {
            return Err("the actual has no unique continuing shared hold");
        }
        let selected_type = validation::project_type_reference_from_segments(
            program,
            captured.operand.type_reference,
            &captured.projection.remaining,
        )
        .ok_or("the saved projection has no exact type")?;
        let reference = validation::reference_shell(program, selected_type)
            .filter(|reference| reference.access == language_semantics::ReferenceAccess::Shared)
            .ok_or("the saved projection is not a shared reference")?;
        if !validation::has_contents(
            program,
            reference.referee,
            &[],
            validation::ContentsRequirement::StableObservation,
        ) {
            return Err("the saved referee has mutable or unclassified observations");
        }
        let mut source = capture;
        let mut path = captured.projection.remaining.clone();
        let mut visited = Vec::new();
        while let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } =
            control.operands.get(source).completion
        {
            if !control.copied_values.is_valid(copy) || visited.contains(&source) {
                return Err("the saved copy graph is missing or cyclic");
            }
            visited.push(source);
            let prefix = control
                .operand_child_paths
                .span(prefix)
                .ok_or("the saved copy has no declared projection")?;
            let mut projected = prefix.to_vec();
            projected.extend(path);
            path = projected;
            source = control.copied_values.get(copy).source;
            if !control.operands.is_valid(source) {
                return Err("the saved copy has no source occurrence");
            }
        }
        if visited.is_empty() {
            return Err("the reference has no saved copy occurrence");
        }
        let earlier = control.operands.get(source);
        let source_reference = validation::project_type_reference_from_segments(
            program,
            earlier.type_reference,
            &path,
        )
        .and_then(|reference| validation::reference_shell(program, reference))
        .filter(|source| {
            source.access == language_semantics::ReferenceAccess::Shared
                && program.normalized_type_identity(source.referee)
                    == program.normalized_type_identity(reference.referee)
        })
        .ok_or("the saved source has no matching shared referee type")?;
        if !validation::has_contents(
            program,
            source_reference.referee,
            &[],
            validation::ContentsRequirement::StableObservation,
        ) {
            return Err("the source referee has no stable observation shape");
        }
        let epoch = actuals
            .last()
            .ok_or("the invocation has no completed actual cut")?;
        let prepared_loans = LoanOccurrences::for_state(program, facts.view(), state);
        let loan_occurrences = &prepared_loans;
        let backings = super::completed_backings::resolve_at_path(
            program,
            facts,
            loan_occurrences,
            state,
            capture,
            &captured.projection.remaining,
        )?;
        if backings.places.is_empty() {
            return Err("the saved reference has no complete backing roster");
        }
        let actual = super::place_access::argument_backings(
            program,
            facts,
            loan_occurrences,
            state,
            borrow_call,
            call.entry_constraints,
            access,
        )?;
        let same = |left: &super::aliases::ResolvedAlias, right: &super::aliases::ResolvedAlias| {
            left.place == right.place
                && left.lineage.iter().all(|loan| right.lineage.contains(loan))
                && right.lineage.iter().all(|loan| left.lineage.contains(loan))
        };
        if !actual.represents(observation)
            || actual.places.is_empty()
            || !actual
                .places
                .iter()
                .all(|actual| backings.places.iter().any(|saved| same(actual, saved)))
            || !backings
                .places
                .iter()
                .all(|saved| actual.places.iter().any(|actual| same(actual, saved)))
        {
            return Err(
                "the continuing access has a different descriptor occurrence or supplier frontier",
            );
        }
        let writes =
            crate::flow::operand_contents_writes_between(program, &facts.flow, earlier, epoch)
                .ok_or("the observation interval has missing, reordered or unknown writes")?;
        let mut lookup = None;
        for (statement, write) in writes {
            let grants = write_permission_envelope(
                program,
                facts,
                loan_occurrences,
                state,
                earlier,
                epoch,
                statement,
                write,
            )?;
            if grants.is_empty()
                || !grants.iter().all(|grant| {
                    backings.places.iter().all(|saved| {
                        super::overlap::captured_place_compatibility(
                            program,
                            &facts.borrow,
                            &saved.place,
                            &BorrowAccessKind::Read,
                            &grant.place,
                            &BorrowAccessKind::Mutable,
                            &[],
                            &mut lookup,
                        )
                        .non_interfering
                    })
                })
            {
                return Err("an intervening write may change the saved referee");
            }
        }
        let source = crate::flow::CapturedOperand::recorded(control, source)
            .ok_or("the earlier observation association is unavailable")?;
        let projection = crate::flow::LiteralExpressionProjection {
            expression: source.expression,
            remaining: path,
            destination: Vec::new(),
        };
        Ok(crate::flow::CapturedValue {
            observation: source,
            observation_type: source.type_reference,
            observation_projection: projection.clone(),
            operand: source,
            type_reference: source.type_reference,
            projection,
            choices: captured.choices.clone(),
            producer_epoch: None,
        })
    }

    /*
        A contents footprint may overapproximate a whole invocation referent. Use
        the complete grant, not guessed bytes: a mutable Buffer grant can replace
        its pointer fields without acquiring the nested shared Cell permission.
        Crossing a stored descriptor would require that descriptor's write-time
        occurrence, which this footprint does not carry; it therefore refuses.
    */
    fn write_permission_envelope(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        earlier: &checked_trees::FlowOperandFact,
        epoch: &checked_trees::FlowOperandFact,
        statement: usize,
        write: &checked_trees::FlowOperandReferent,
    ) -> Result<Vec<super::aliases::ResolvedAlias>, &'static str> {
        let symbol = match write.root {
            PlaceRoot::Symbol(symbol) => symbol,
            PlaceRoot::InvocationReferent {
                parameter,
                type_reference,
            } => {
                let machine = program
                    .machine_by_symbol(state.machine_symbol)
                    .ok_or("the write has no owning machine")?;
                let entry = program
                    .machine_states(machine)
                    .first()
                    .filter(|entry| entry.symbol == state.state_symbol)
                    .ok_or("the invocation envelope has no current entry binding")?;
                let parameter_row = program
                    .state_parameter_by_symbol(entry, parameter)
                    .filter(|row| {
                        row.type_reference == type_reference
                            && !row.is_const
                            && !row.relevance.is_erased()
                    })
                    .ok_or("the invocation envelope has no authenticated formal")?;
                if validation::reference_shell(program, parameter_row.type_reference).is_none() {
                    return Err("the invocation envelope has no reference grant");
                }
                parameter
            }
            _ => return Err("the write has no authenticated storage permission"),
        };
        let root = CanonicalPlace {
            root: PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        };
        let root_type = validation::canonical_place_type_reference(
            program,
            state.state_symbol,
            earlier.statement_index,
            &root,
        )
        .ok_or("the writer did not exist at the saved observation")?;
        let segments = facts
            .flow
            .control
            .operand_referent_segments
            .span(write.segments)
            .ok_or("the write footprint has no complete path")?;
        let reference = validation::reference_shell(program, root_type);
        let owned_type = if let Some(reference) = reference.as_ref() {
            if !matches!(
                reference.access,
                language_semantics::ReferenceAccess::Mutable
                    | language_semantics::ReferenceAccess::WriteOnly
            ) || !crate::flow::operand_bindings_agree(
                program,
                facts.flow.view(),
                earlier,
                epoch,
                &root,
            ) {
                return Err("the writer has no unchanged mutable descriptor grant");
            }
            reference.referee
        } else {
            root_type
        };
        if validation::owned_projection_type_reference(program, owned_type, segments).is_none()
            || validation::canonical_place_type_reference(
                program,
                state.state_symbol,
                statement,
                &root,
            ) != Some(root_type)
        {
            return Err("the write crosses an unavailable or mutable descriptor boundary");
        }
        let place = CapturedPlace {
            root: root.root,
            segments: Vec::new(),
            ..Default::default()
        };
        if reference.is_none() {
            return Ok(vec![super::aliases::ResolvedAlias {
                place,
                lineage: Vec::new(),
            }]);
        }
        super::access_sources::resolve_descriptor_all(
            program,
            facts,
            loan_occurrences,
            state,
            earlier.statement_index,
            earlier.constraints,
            place,
        )
        .ok_or("the writer has no retained permission correspondence")
    }
}
pub(crate) use shared_observations::shared_reference_observation;
mod details {
    use checked_trees::{CheckFacts, FlowStateFact};
    use symbols::SymbolHandle;

    /*
        The checked access a reference-typed binding's declaration lends over its
        referent — the parent authority for any borrow expression formed on that
        binding. `&T`/`&mut T`/`&write T` bindings answer `Read`/`Mutable`/
        `WriteOnly`; value bindings, attached storage, and machine-owned roots
        answer `None` and stay on the writable-place gate instead. Locals are only
        visible before `statement_index`, matching how the borrow places resolve
        them.
    */
    pub(super) fn binding_reference_access(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        root: facts::PlaceRoot,
    ) -> Option<checked_trees::BorrowAccessKind> {
        let root = root.as_symbol()?;
        let state = validation::find_state_in_machine(program, machine_symbol, state_symbol)?;
        let type_reference = program
            .state_parameter_by_symbol(state, root)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .state_local_declaration(state, root)
                    .filter(|&(ordinal, _)| ordinal < statement_index)
                    .map(|(_, local)| local.type_reference)
            })?;
        crate::borrow::reference_borrow_access_kind(program, type_reference)
    }

    pub(super) fn active_loan_detail(
        state_flow: &FlowStateFact,
        facts: &CheckFacts,
        loan: arena::Handle<checked_trees::BorrowLoanFact>,
        statement_index: usize,
    ) -> Option<String> {
        facts
            .flow.borrow_lifetimes.weakenings
            .span_or_empty(state_flow.borrow_weakenings)
            .iter()
            .find(|weakening| weakening.loan == loan)
            .and_then(|weakening| {
                let loan = facts.borrow.loans.get(loan);
                match (weakening.reason, weakening.source) {
                    (
                        checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                        checked_trees::FlowInvalidationSource::Statement {
                            statement_index: weakening_statement,
                        },
                    ) if weakening_statement > statement_index => Some(format!(
                        "borrowed at statement {}; its last use is at statement {}",
                        loan.statement_index, loan.last_use_statement_index
                    )),
                    (
                        checked_trees::FlowBorrowWeakeningReason::StateExit,
                        checked_trees::FlowInvalidationSource::Statement { .. },
                    ) if loan.last_use_statement_index > statement_index => Some(format!(
                        "borrowed at statement {}; its last use is at statement {} and it is released at state exit",
                        loan.statement_index, loan.last_use_statement_index
                    )),
                    (
                        checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                        checked_trees::FlowInvalidationSource::Statement {
                            statement_index: weakening_statement,
                        },
                    ) if weakening_statement > statement_index => Some(format!(
                        "borrowed at statement {}; it is reassigned at statement {}",
                        loan.statement_index, weakening_statement
                    )),
                    (
                        checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                        checked_trees::FlowInvalidationSource::Statement { .. },
                    )
                    | (
                        checked_trees::FlowBorrowWeakeningReason::StateExit,
                        checked_trees::FlowInvalidationSource::Statement { .. },
                    )
                    | (
                        checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                        checked_trees::FlowInvalidationSource::Statement { .. },
                    )
                    | (
                        checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                        checked_trees::FlowInvalidationSource::Call { .. },
                    )
                    | (
                        checked_trees::FlowBorrowWeakeningReason::StateExit,
                        checked_trees::FlowInvalidationSource::Call { .. },
                    )
                    | (
                        checked_trees::FlowBorrowWeakeningReason::LocalReassigned,
                        checked_trees::FlowInvalidationSource::Call { .. },
                    ) => None,
                }
            })
    }
}
#[path = "checks_borrows_overlap.rs"]
mod overlap;
mod place_access {
    /*
        One live-loan judgment for every access to storage.

        A pointer variable and the data it points to require different checks.
        The captured receiving endpoint selects whether an operation lends the
        variable's storage or accesses data through its reference. Accesses through
        a reference rejoin every backing its live loan may denote. Storage identity
        and authority are separate judgments: each backing is compared with every
        live loan, and only the access's own retained lineage is exempt. A child
        formed through the accessing alias is not part of that lineage, so an
        exclusive child suspends the branch it captured for any other access until
        its last use, and a shared child freezes that branch's mutation.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::flow::expressions;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        BorrowAccessKind, BorrowLoanFact, CapturedPlace, CheckFacts, FlowConstraintRef,
        FlowStateFact,
    };

    use super::access_sources;
    use super::aliases::ResolvedAlias;
    use super::overlap::{StatedOrderingPremise, captured_place_compatibility};

    /*
        Every backing an access through `place` may denote, with the retained lineage
        that grants its authority. A root that owns none of the live loans (a
        parameter, an owner, a reference initialized from static text) names its
        own storage. A local reference that owns one is rejoined to the storage
        its loans captured; `None` when that backing cannot be established.
    */
    pub(super) fn place_backings(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement_index: usize,
        live_loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
        place: CapturedPlace,
    ) -> Option<Vec<ResolvedAlias>> {
        let live_loans = live_loans.collect::<Vec<_>>();
        if !live_loans
            .iter()
            .any(|loan| facts.borrow.loans.get(*loan).storage_owner == place.root)
        {
            return Some(vec![ResolvedAlias {
                place,
                lineage: Vec::new(),
            }]);
        }
        access_sources::resolve_captured_with_loans(
            program,
            facts,
            loan_occurrences,
            state,
            statement_index,
            live_loans.into_iter(),
            place,
        )
    }

    /*
        Backing authority and exact invocation ancestry remain separate from
        storage coincidence. Only the completed result's own input occurrence can
        continue a temporary invocation hold without creating a second loan.
    */
    #[derive(Default)]
    pub(super) struct ArgumentBackings {
        pub(super) places: Vec<ResolvedAlias>,
        capture: Handle<checked_trees::FlowOperandFact>,
        forwarded_inputs: Vec<Handle<checked_trees::FlowOperandFact>>,
    }

    impl ArgumentBackings {
        /*
            The resolver's exact actual occurrence, independently of its source
            expression or the places its descriptor happens to denote.
        */
        pub(super) fn represents(&self, capture: Handle<checked_trees::FlowOperandFact>) -> bool {
            capture.is_valid() && self.capture == capture
        }

        pub(super) fn forwards(&self, input: &Self) -> bool {
            input.capture.is_valid()
                && self.capture != input.capture
                && self.forwarded_inputs.contains(&input.capture)
        }
    }

    /*
        A forwarded descriptor lends the referent captured when that actual
        completed. Its private carrier read has finished; later actuals may replace
        that carrier without retargeting the earlier loan. Existing alias resolution
        still supplies lineage, using that observation's own live-loan constraints.
    */
    pub(super) fn argument_backings(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: &checked_trees::BorrowCallFact,
        entry_constraints: HandleSpan<FlowConstraintRef>,
        access: &checked_trees::BorrowArgumentAccessFact,
    ) -> Result<ArgumentBackings, &'static str> {
        let place = CapturedPlace {
            root: access.root,
            segments: facts.borrow.access_segments(access).to_vec(),
            ..Default::default()
        };

        let selected = argument_capture(program, facts, state, call, access);
        let metadata_context =
            validation::machine_state_by_symbols(program, state.machine_symbol, state.state_symbol)
                .filter(|(machine, declaration)| {
                    validation::collection_length_receiver(
                        program,
                        machine,
                        Some(declaration),
                        access.expression,
                    )
                    .is_some()
                });
        if let Some((machine, declaration)) = metadata_context {
            let selected = selected.ok_or("metadata has no unique completed operand")?;
            let control = &facts.flow.control;
            let captured = control.operands.get(selected);
            let evaluation = crate::flow::call_permission_evaluation(
                program,
                machine,
                declaration,
                call.statement_index,
                call.call_ordinal,
            );
            let statement = control
                .statements
                .span(state.statements)
                .and_then(|statements| {
                    statements
                        .iter()
                        .find(|statement| statement.statement_index == call.statement_index)
                })
                .ok_or("metadata lost its exact statement observation roster")?;
            let mut observations = control
                .value_observations
                .span(statement.value_observations)
                .ok_or("metadata has no complete observation roster")?
                .iter()
                .filter(|observed| {
                    observed.expression == access.expression
                        && observed.evaluation == evaluation
                        && observed.receiver == captured.backing
                });
            let observed = observations
                .next()
                .ok_or("metadata lost its captured receiver observation")?;
            if observations.next().is_some() {
                return Err("metadata has competing captured receiver observations");
            }
            let places = access_sources::metadata_storage(
                program,
                facts,
                loan_occurrences,
                state,
                call.statement_index,
                observed,
                place,
            )
            .ok_or("metadata descriptor has no retained storage authority")?;
            return Ok(ArgumentBackings {
                places,
                capture: selected,
                forwarded_inputs: Vec::new(),
            });
        }
        let forwarded = !matches!(
            program.expression_table.expression(access.expression),
            typed_trees::expression::ExpressionNode::Borrow(_)
        ) && (validation::expression_type_reference_in_state(
            program,
            state.state_symbol,
            call.statement_index,
            access.expression,
        )
        .is_some_and(|reference| validation::reference_shell(program, reference).is_some())
            || selected.is_some_and(|capture| {
                super::completed_backings::value_observation_descriptor_source(
                    program, facts, capture,
                )
                .is_ok()
            }));
        if !forwarded {
            if let Some(selected) = selected
                && let Some(formation) =
                    super::formation_captures::argument_formation(program, facts, state, selected)?
                && formation.storage
            {
                let resolved = super::completed_backings::resolve(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    formation.value,
                )?;
                return Ok(ArgumentBackings {
                    places: resolved.places,
                    capture: formation.value,
                    forwarded_inputs: resolved.forwarded_inputs,
                });
            }
            return place_backings(
                program,
                facts.view(),
                loan_occurrences,
                state,
                call.statement_index,
                facts.flow.borrow_loan_constraints(entry_constraints),
                place,
            )
            .map(|places| ArgumentBackings {
                places,
                capture: selected.unwrap_or_default(),
                forwarded_inputs: Vec::new(),
            })
            .ok_or("the source place has no retained authority");
        }
        let selected = selected.ok_or_else(|| {
            if ::validation::trace_flags::check_sites() {
                trace_argument_capture_refusal(facts, state, call, access);
            }
            "the access has no unique completed operand"
        })?;
        let resolved =
            super::completed_backings::resolve(program, facts, loan_occurrences, state, selected)?;
        Ok(ArgumentBackings {
            places: resolved.places,
            capture: selected,
            forwarded_inputs: resolved.forwarded_inputs,
        })
    }

    fn argument_capture(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &checked_trees::BorrowCallFact,
        access: &checked_trees::BorrowArgumentAccessFact,
    ) -> Option<Handle<checked_trees::FlowOperandFact>> {
        let mut own_calls = facts
            .flow
            .control
            .calls
            .span(state.calls)?
            .iter()
            .filter(|source| {
                source.statement_index == call.statement_index
                    && source.call_ordinal == call.call_ordinal
            });
        let Some(source) = own_calls.next() else {
            let observation = storage_address_observation(program, facts, state, call)?;
            let receiver = observation.receiver;
            return (facts.flow.control.operands.get(receiver).expression == access.expression)
                .then_some(receiver);
        };
        if own_calls.next().is_some()
            || source.target_symbol != call.target_symbol
            || source.accesses != call.accesses
        {
            return None;
        }
        // The call retains its completed operand trees, including constructed
        // children and evaluated backing. Other observations in this statement
        // belong to their own evaluation cuts and cannot supply this call's loan.
        let mut pending = source.operands.handles().collect::<Vec<_>>();
        let mut visited = Vec::new();
        let mut selected = Handle::invalid();
        while let Some(handle) = pending.pop() {
            if visited.contains(&handle) {
                continue;
            }
            visited.push(handle);
            if !facts.flow.control.operands.is_valid(handle) {
                return None;
            }
            let value = facts.flow.control.operands.get(handle);
            if value.state_symbol != state.state_symbol
                || value.statement_index != call.statement_index
            {
                continue;
            }
            if value.expression == access.expression {
                if selected.is_valid() {
                    return None;
                }
                selected = handle;
                continue;
            }
            if value.backing.is_valid() {
                pending.push(value.backing);
            }
            pending.extend(value.evaluated_inputs.handles());
            match value.completion {
                checked_trees::FlowOperandCompletion::Constructed(children) => {
                    pending.extend(
                        facts
                            .flow
                            .control
                            .operand_children
                            .span(children)?
                            .iter()
                            .map(|child| child.value),
                    );
                }
                checked_trees::FlowOperandCompletion::Selected(alternatives) => {
                    pending.extend(
                        facts
                            .flow
                            .control
                            .operand_alternatives
                            .span(alternatives)?
                            .iter()
                            .map(|alternative| alternative.value),
                    );
                }
                checked_trees::FlowOperandCompletion::Observation
                | checked_trees::FlowOperandCompletion::IncomingContents { .. }
                | checked_trees::FlowOperandCompletion::InvocationResult { .. }
                | checked_trees::FlowOperandCompletion::Copied { .. } => {}
            }
        }
        selected.is_valid().then_some(selected)
    }

    /*
        Address exposure is a compiler operation, not a machine invocation. Its
        shared observation owner retains both the exact input and the post-operand
        cut. Capture selection and conflict checking consume this same occurrence;
        a missing ordinary call never authorizes an arena/expression fallback.
    */
    pub(super) fn storage_address_observation<'facts>(
        program: &dyn validation::CheckingInput,
        facts: &'facts CheckFacts,
        state: &FlowStateFact,
        call: &checked_trees::BorrowCallFact,
    ) -> Option<&'facts checked_trees::FlowValueObservationFact> {
        let control = &facts.flow.control;
        if !call.authored_expression.is_valid()
            || control.calls.span(state.calls)?.iter().any(|source| {
                source.statement_index == call.statement_index
                    && source.call_ordinal == call.call_ordinal
            })
        {
            return None;
        }
        let typed_trees::expression::ExpressionNode::Call(operation) =
            expressions(program).expression(call.authored_expression)
        else {
            return None;
        };
        let input = validation::compiler_operations::storage_address_operand(program, operation)?;
        let mut selected = None;
        for statement in control.statements.span(state.statements)? {
            if statement.statement_index != call.statement_index {
                continue;
            }
            for observation in control
                .value_observations
                .span(statement.value_observations)?
            {
                if observation.expression != call.authored_expression {
                    continue;
                }
                if selected.is_some() || !control.operands.is_valid(observation.receiver) {
                    return None;
                }
                let value = control.operands.get(observation.receiver);
                if value.expression != input
                    || value.state_symbol != state.state_symbol
                    || value.statement_index != call.statement_index
                    || value.constraints != observation.constraints
                {
                    return None;
                }
                selected = Some(observation);
            }
        }
        selected
    }

    /*
        Opt-in refusal context reads immutable occurrence inventories only.
        Arena-wide matches are reported for diagnosis, never chosen as a fallback.
    */
    fn trace_argument_capture_refusal(
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &checked_trees::BorrowCallFact,
        access: &checked_trees::BorrowArgumentAccessFact,
    ) {
        let control = &facts.flow.control;
        eprintln!(
            "CHECKSITE argument-capture state={:?} statement={} ordinal={} access={:?}",
            state.state_symbol, call.statement_index, call.call_ordinal, access
        );
        for invocation in control.calls.span_or_empty(state.calls) {
            if invocation.statement_index == call.statement_index {
                eprintln!("CHECKSITE argument-capture-invocation {:?}", invocation);
            }
        }
        for (handle, value) in control.operands.iter() {
            if value.state_symbol == state.state_symbol
                && value.statement_index == call.statement_index
                && value.expression == access.expression
            {
                eprintln!(
                    "CHECKSITE argument-capture-candidate {:?} {:?}",
                    handle, value
                );
            }
        }
    }

    /*
        Whether an access of `kind` through `backings` interferes with the live
        loan `loan_handle`.
    */
    pub(super) fn access_interferes<'p>(
        program: &'p dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        backings: &[ResolvedAlias],
        kind: &BorrowAccessKind,
        loan_handle: Handle<BorrowLoanFact>,
        premises: &[StatedOrderingPremise],
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> bool {
        let borrow = &facts.borrow;
        let loan = borrow.loans.get(loan_handle);
        !backings.iter().all(|backing| {
            if let Some(non_interfering) = super::overlap::retained_debt_non_interfering(
                program,
                borrow,
                loan_handle,
                &backing.place,
                kind,
                premises,
                bound_lookup,
            ) {
                return non_interfering;
            }
            if backing.retains_loan(facts, loan_occurrences, loan_handle) {
                return true;
            }
            let shares_call = loan.retained_operand.is_some_and(|retained| {
                retained.formation_expression.is_valid()
                    && backing.lineage.iter().any(|ancestor| {
                        let ancestor = borrow.loans.get(*ancestor);
                        ancestor.root == loan.root
                            && ancestor.incoming_reference == loan.incoming_reference
                            && ancestor.retained_operand == Some(retained)
                            && captured_place_compatibility(
                                program,
                                borrow,
                                &CapturedPlace {
                                    incoming_reference: ancestor.incoming_reference,
                                    incoming_owner_path: borrow
                                        .access_segments
                                        .span_or_empty(ancestor.incoming_owner_path)
                                        .to_vec(),
                                    root: ancestor.root,
                                    segments: borrow.loan_segments(ancestor).to_vec(),
                                },
                                &ancestor.kind,
                                &CapturedPlace {
                                    incoming_reference: loan.incoming_reference,
                                    incoming_owner_path: borrow
                                        .access_segments
                                        .span_or_empty(loan.incoming_owner_path)
                                        .to_vec(),
                                    root: loan.root,
                                    segments: borrow.loan_segments(loan).to_vec(),
                                },
                                &loan.kind,
                                premises,
                                bound_lookup,
                            )
                            .disjoint
                    })
            });
            let segments = borrow.loan_segments(loan);
            let depth = loan
                .retained_operand
                .filter(|_| !shares_call)
                .map_or(segments.len(), |retained| retained.depth);
            let loan_place = CapturedPlace {
                incoming_reference: loan.incoming_reference,
                incoming_owner_path: borrow
                    .access_segments
                    .span_or_empty(loan.incoming_owner_path)
                    .to_vec(),
                root: loan.root,
                segments: segments[..depth].to_vec(),
            };
            captured_place_compatibility(
                program,
                borrow,
                &backing.place,
                kind,
                &loan_place,
                &loan.kind,
                premises,
                bound_lookup,
            )
            .non_interfering
        })
    }
}
pub(crate) use overlap::{
    StorageSelectorFacts, captured_storage_index, captured_storage_paths_are_disjoint,
    storage_path_compatibility,
};
mod simultaneous {
    /*
        Places one evaluation holds together: the operands of a call, or the
        reference leaves of one aggregate value formed at once. Every pair must be
        non-interfering under `captured_place_compatibility`: an exclusive or
        write-only hold conflicts with any overlapping hold, and two shared holds
        conflict only when the checker cannot establish non-interference. Calls and
        aggregate formation share this judgment and its wording; each diagnostic
        supplies its own subject and verb.
    */
    use checked_trees::{BorrowAccessKind, CapturedPlace};

    use super::overlap::{StatedOrderingPremise, captured_place_compatibility};

    /* One of the two holds a conflict compares. */
    #[derive(Clone, Copy)]
    pub(super) enum Hold {
        First,
        Second,
    }

    /*
        Why two holds formed together interfere. Each variant names the hold whose
        place the diagnostic reports.
    */
    pub(super) enum SimultaneousHoldConflict {
        RepeatedMutable,
        MutableAndRead { mutable: Hold },
        WriteOnly { write_only: Hold },
        DependentReads,
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn simultaneous_hold_conflict<'p>(
        program: &'p dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        first: &CapturedPlace,
        first_kind: &BorrowAccessKind,
        second: &CapturedPlace,
        second_kind: &BorrowAccessKind,
        premises: &[StatedOrderingPremise],
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> Option<SimultaneousHoldConflict> {
        if captured_place_compatibility(
            program,
            borrow,
            first,
            first_kind,
            second,
            second_kind,
            premises,
            bound_lookup,
        )
        .non_interfering
        {
            return None;
        }
        Some(match (first_kind, second_kind) {
            (BorrowAccessKind::Mutable, BorrowAccessKind::Mutable) => {
                SimultaneousHoldConflict::RepeatedMutable
            }
            (BorrowAccessKind::Mutable, BorrowAccessKind::Read) => {
                SimultaneousHoldConflict::MutableAndRead {
                    mutable: Hold::First,
                }
            }
            (BorrowAccessKind::Read, BorrowAccessKind::Mutable) => {
                SimultaneousHoldConflict::MutableAndRead {
                    mutable: Hold::Second,
                }
            }
            (BorrowAccessKind::WriteOnly, _) => SimultaneousHoldConflict::WriteOnly {
                write_only: Hold::First,
            },
            (_, BorrowAccessKind::WriteOnly) => SimultaneousHoldConflict::WriteOnly {
                write_only: Hold::Second,
            },
            (BorrowAccessKind::Read, BorrowAccessKind::Read) => {
                SimultaneousHoldConflict::DependentReads
            }
        })
    }

    impl SimultaneousHoldConflict {
        /*
            The held-place phrase that follows a subject and verb such as
            "state `touch` receives" or "`messages` holds". `label` renders a
            hold's place; `sibling` names the other hold of a write-only overlap.
        */
        pub(super) fn phrase(&self, label: impl Fn(Hold) -> String, sibling: &str) -> String {
            match self {
                Self::RepeatedMutable => {
                    format!("`{}` as mutable more than once", label(Hold::First))
                }
                Self::MutableAndRead { mutable } => {
                    format!("`{}` as both mutable and read-only", label(*mutable))
                }
                Self::WriteOnly { write_only } => {
                    format!("write-only `{}` overlapping {sibling}", label(*write_only))
                }
                Self::DependentReads => {
                    "dependent read-only places that do not establish non-interference".to_owned()
                }
            }
        }
    }
}

use super::ranges::incoming_guards::IncomingGuardIndex;
use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use self::elision::check_view_return_elision;
use self::escape::check_view_return_escape;
use self::loan_occurrences::LoanOccurrences;
use self::persistent::check_persistent_borrow_assignments;
use self::statements::check_statement_borrows;

pub(crate) fn check_flow_call_borrows(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    incoming_guards: &IncomingGuardIndex,
) -> Result<(), Vec<Diagnostic>> {
    let prepared_loans = LoanOccurrences::new(program, facts);
    let loan_occurrences = &prepared_loans;
    let mut diagnostics = Vec::new();
    let mut borrow_by_state = symbols::SymbolKeyMap::<
        (symbols::SymbolHandle, symbols::SymbolHandle),
        &checked_trees::StateBorrowFact,
    >::default();
    for (_, state_borrow) in facts.borrow.states.iter() {
        borrow_by_state
            .entry((state_borrow.machine_symbol, state_borrow.state_symbol))
            .or_insert(state_borrow);
    }
    let state_flows: Vec<(&FlowStateFact, Option<&checked_trees::StateBorrowFact>)> = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state_flow)| {
            let borrow_state = borrow_by_state
                .get(&(state_flow.machine_symbol, state_flow.state_symbol))
                .copied();
            (state_flow, borrow_state)
        })
        .collect();

    calls::check_compatibility(
        program,
        facts,
        loan_occurrences,
        &state_flows,
        incoming_guards,
        call_frames,
        &mut diagnostics,
    );

    check_view_return_elision(program, facts, call_frames, &mut diagnostics);
    check_view_return_escape(program, facts, &mut diagnostics);
    check_persistent_borrow_assignments(program, facts, call_frames, &mut diagnostics);

    let receiver_reads = crate::flow::StateReceiverReadCache::default();
    let incoming_backing = incoming_backing::IncomingBackingCalls::new(program, facts, &[]);
    let per_state_results: Vec<(Vec<Diagnostic>, Vec<incoming_backing::PendingRequirement>)> =
        validation::checking_thread_pool().install(|| {
            use rayon::prelude::*;
            state_flows
                .par_iter()
                .filter_map(|&(state_flow, borrow_state)| {
                    let borrow_state = borrow_state?;
                    let mut local_diagnostics = Vec::new();
                    let mut incoming_requirements = Vec::new();
                    let mut local_bound_lookup = None;

                    let stated_premises = validation::find_state_in_machine(
                        program,
                        state_flow.machine_symbol,
                        state_flow.state_symbol,
                    )
                    .and_then(|state| {
                        program
                            .machine_by_symbol(state_flow.machine_symbol)
                            .map(|machine| (machine, state))
                    })
                    .map(|(machine, state)| {
                        overlap::stated_ordering_premises(
                            program,
                            facts,
                            machine,
                            state,
                            incoming_guards,
                            &mut local_bound_lookup,
                        )
                    })
                    .unwrap_or_default();

                    let mut guarantees =
                        crate::checks::contracts::prover::call_guarantees::StateGuarantees::new(
                            program,
                            facts.view(),
                            state_flow,
                            call_frames,
                        );
                    check_statement_borrows(
                        program,
                        facts,
                        loan_occurrences,
                        state_flow,
                        borrow_state,
                        &incoming_backing,
                        &mut incoming_requirements,
                        &stated_premises,
                        &mut local_diagnostics,
                        mutation_summaries,
                        &receiver_reads,
                        call_frames,
                        &mut guarantees,
                        &mut local_bound_lookup,
                    );
                    aggregates::check_formed_aggregates(
                        program,
                        facts,
                        state_flow,
                        borrow_state,
                        &stated_premises,
                        &mut guarantees,
                        &mut local_diagnostics,
                        &mut local_bound_lookup,
                    );
                    Some((local_diagnostics, incoming_requirements))
                })
                .collect()
        });
    let mut incoming_requirements = Vec::new();
    for (local_diagnostics, requirements) in per_state_results {
        diagnostics.extend(local_diagnostics);
        incoming_requirements.extend(requirements);
    }
    crate::checks::finish(diagnostics)?;
    drop(prepared_loans);
    incoming_backing::publish(&mut facts.borrow, incoming_requirements)?;
    Ok(())
}

pub(super) fn initialize_checked_direct_borrow_resources(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    resources::initialize_checked_direct_borrow_resources(program, facts, mutation_summaries)
}

fn matching_borrow_state<'a>(
    facts: checked_trees::CheckFactsRef<'a>,
    state_flow: &FlowStateFact,
) -> Option<&'a checked_trees::StateBorrowFact> {
    facts
        .borrow
        .first_state_borrow(state_flow.machine_symbol, state_flow.state_symbol)
        .map(|(_, state)| state)
}
