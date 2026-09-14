//! Discover complete call tuples and check their authored generic bounds.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_call_proposals(
    program: &TypedTrees,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    target_symbol: SymbolHandle,
    target_name: &str,
    machine_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    expected_return: Option<TypeReferenceHandle>,
    scope_limit: usize,
    machine_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    evidence_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    runtime_value_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
) {
    let Some(callee) = resolve_callee(callee_states, target_symbol, target_name) else {
        return;
    };
    collect_machine_proposals_for_callee(
        program,
        candidates,
        callee,
        machine_arguments,
        Some((caller_state, scope_limit)),
        machine_proposals,
        evidence_proposals,
        type_proposals,
        const_proposals,
        runtime_value_proposals,
    );

    // An explicit bound is selected before compatibility. It must not conflict
    // with an ordinary argument's narrower declared endpoint.
    let fixed_range_parameters: Vec<_> = machine_arguments
        .iter()
        .filter(|argument| {
            const_arguments::spelling(program, argument).is_some()
                || const_arguments::forwarded_type(program, argument).is_valid()
                || const_arguments::is_runtime_value_subject(program, argument)
        })
        .enumerate()
        .map(|(parameter_index, _)| parameter_index)
        .collect();
    let candidate = &candidates[callee.candidate_index];
    let skip = callee.parameter_types.len().saturating_sub(arguments.len());
    for (argument, required) in arguments
        .iter()
        .zip(callee.parameter_types.iter().skip(skip))
    {
        let Some(actual) = validation::declared_place_type_raw(
            program,
            caller_machine,
            Some(caller_state),
            *argument,
        ) else {
            continue;
        };
        infer_static_bindings(
            program,
            *required,
            actual,
            &candidate.template.type_parameters,
            &candidate.template.const_parameters,
            Some(&fixed_range_parameters),
            callee.candidate_index,
            type_proposals,
            const_proposals,
        );
    }
    if let Some(actual) = expected_return {
        // Result context can fill an omitted endpoint, but cannot reselect a
        // bound already supplied by an argument (even a still-open one).
        // The destination's range is a later compatibility obligation.
        let mut fixed_result_parameters = fixed_range_parameters;
        fixed_result_parameters.extend(const_proposals.iter().filter_map(
            |(candidate_index, parameter_index, _)| {
                (*candidate_index == callee.candidate_index).then_some(*parameter_index)
            },
        ));
        infer_static_bindings(
            program,
            callee.return_type,
            actual,
            &candidate.template.type_parameters,
            &candidate.template.const_parameters,
            Some(&fixed_result_parameters),
            callee.candidate_index,
            type_proposals,
            const_proposals,
        );
    }
}

pub(super) fn collect_machine_proposals_for_callee(
    program: &TypedTrees,
    candidates: &[Candidate],
    callee: &CalleeState,
    machine_arguments: &[StaticMachineArgument],
    runtime_scope: Option<(&typed_trees::state::State, usize)>,
    machine_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    evidence_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    runtime_value_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
) {
    let candidate = &candidates[callee.candidate_index];
    let mut type_index = 0usize;
    let mut const_index = 0usize;
    let mut machine_index = 0usize;
    let mut evidence_index = 0usize;
    for selected in machine_arguments {
        // A forwarded binder occupies its explicit slot even before it has a
        // closed value. Keep this call incomplete until the caller specializes;
        // neither range inference nor a later explicit argument may fill it.
        if const_arguments::forwarded_type(program, selected).is_valid() {
            if let Some(binding) = program
                .type_reference_table
                .find_named_type_reference(selected.symbol)
            {
                const_proposals.push((callee.candidate_index, const_index, binding));
            }
            const_index += 1;
            continue;
        }
        if let Some(literal) = const_arguments::spelling(program, selected) {
            if const_index < candidate.template.const_parameters.len()
                && let Some((handle, _, _)) = program
                    .type_reference_table
                    .named_references()
                    .find(|(_, symbol, name)| !symbol.is_valid() && *name == literal)
            {
                const_proposals.push((callee.candidate_index, const_index, handle));
                const_index += 1;
            }
            continue;
        }
        // A `Value` binder admits an ordinary runtime subject. Its slot binds
        // the declared carrier so every runtime argument of that carrier shares
        // one specialization; the resolved argument later becomes an appended
        // ordinary call argument.
        if const_index < candidate.template.const_parameters.len()
            && candidate
                .template
                .value_const_parameters
                .contains(&const_index)
            && let Some((scope_state, scope_limit)) = runtime_scope
            && let Some(subject) = const_arguments::resolve_runtime_subject(
                program,
                scope_state,
                scope_limit,
                selected,
            )
        {
            const_proposals.push((
                callee.candidate_index,
                const_index,
                candidate.template.const_parameters[const_index].2,
            ));
            let mut subject_argument = selected.clone();
            subject_argument.symbol = subject;
            runtime_value_proposals.push((callee.candidate_index, const_index, subject_argument));
            const_index += 1;
            continue;
        }
        if !selected.symbol.is_valid() {
            continue;
        }
        let kind = program.symbols.get(selected.symbol).kind;
        if matches!(
            kind,
            SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
        ) {
            if type_index < candidate.template.type_parameters.len()
                && let Some((handle, _, _)) = program
                    .type_reference_table
                    .named_references()
                    .find(|(_, symbol, _)| *symbol == selected.symbol)
            {
                type_proposals.push((callee.candidate_index, type_index, handle));
                type_index += 1;
            }
            continue;
        }
        if matches!(
            kind,
            SymbolKind::Conformance | SymbolKind::ConformanceParameter
        ) {
            if evidence_index < candidate.template.evidence_parameters.len() {
                evidence_proposals.push((callee.candidate_index, evidence_index, selected.clone()));
                evidence_index += 1;
            }
            continue;
        }
        if !matches!(kind, SymbolKind::State | SymbolKind::MachineParameter) {
            continue;
        }
        if machine_index >= candidate.template.machine_parameters.len() {
            continue;
        }
        machine_proposals.push((callee.candidate_index, machine_index, selected.clone()));
        let requirement = &candidate.template.machine_parameters[machine_index].2;
        machine_index += 1;
        let Some(actual_state) = state_by_symbol(program, selected.symbol) else {
            continue;
        };
        for (required, actual) in program
            .state_signature_parameters(requirement)
            .iter()
            .zip(program.state_parameters(actual_state))
        {
            // The selected entry's refinement remains part of its machine
            // contract. Generic specialization binds the underlying runtime
            // carrier so a qualified entry still matches the ordinary value
            // supplied at the selecting call site.
            let actual_type = validation::unwrapped_type_reference(program, actual.type_reference)
                .unwrap_or(actual.type_reference);
            infer_static_bindings(
                program,
                required.type_reference,
                actual_type,
                &candidate.template.type_parameters,
                &candidate.template.const_parameters,
                None,
                callee.candidate_index,
                type_proposals,
                const_proposals,
            );
        }
        infer_static_bindings(
            program,
            requirement.return_type,
            actual_state.return_type,
            &candidate.template.type_parameters,
            &candidate.template.const_parameters,
            None,
            callee.candidate_index,
            type_proposals,
            const_proposals,
        );
    }
}

pub(super) fn resolve_callee<'a>(
    callee_states: &'a [CalleeState],
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<&'a CalleeState> {
    if target_symbol.is_valid() {
        return callee_states
            .iter()
            .find(|callee| callee.symbol == target_symbol);
    }
    let mut matching = callee_states
        .iter()
        .filter(|callee| callee.name == target_name);
    match (matching.next(), matching.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

pub(super) fn state_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.symbol == symbol)
}

pub(super) fn collect_call_selections(
    program: &TypedTrees,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    contract_expressions: &[ExpressionHandle],
) -> Vec<CallSelection> {
    let mut selections = Vec::new();
    let mut covered_expressions = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for offset in 0..state.statement_nodes.count() {
                let handle = Handle::from_parts(
                    state.statement_nodes.start().arena_index() + offset,
                    state.statement_nodes.start().generation(),
                );
                match program.statement_table.statement(handle) {
                    StatementNode::Call(call)
                        if typed_trees::operator::declaration_by_symbol(
                            program,
                            call.target_symbol,
                        )
                        .is_none() =>
                    {
                        if let Some(selection) = selection_for_call(
                            program,
                            machine,
                            state,
                            candidates,
                            callee_states,
                            CallSite::Statement(handle),
                            call.target_symbol,
                            call.target.as_str(),
                            &call.machine_arguments,
                            program.statement_table.expression_handles(call.arguments),
                            None,
                            !program.machine_type_parameters(machine).is_empty(),
                        ) {
                            upsert_selection(&mut selections, selection);
                        }
                    }
                    StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(local.initial_value)
                            && typed_trees::operator::resolve_named_expression_call(program, call)
                                .is_none()
                        {
                            covered_expressions.push(local.initial_value);
                            if let Some(selection) = selection_for_call(
                                program,
                                machine,
                                state,
                                candidates,
                                callee_states,
                                CallSite::Expression(local.initial_value),
                                call.target_symbol,
                                call.target.as_str(),
                                &call.machine_arguments,
                                program.expression_table.expression_handles(call.arguments),
                                // An inferred result type is not independent
                                // destination evidence for its own call.
                                (local.type_reference.is_valid() && !local.type_is_inferred)
                                    .then_some(local.type_reference),
                                !program.machine_type_parameters(machine).is_empty(),
                            ) {
                                upsert_selection(&mut selections, selection);
                            }
                        }
                    }
                    StatementNode::Expression(expression) => {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(*expression)
                            && typed_trees::operator::resolve_named_expression_call(program, call)
                                .is_none()
                        {
                            covered_expressions.push(*expression);
                            if let Some(selection) = selection_for_call(
                                program,
                                machine,
                                state,
                                candidates,
                                callee_states,
                                CallSite::Expression(*expression),
                                call.target_symbol,
                                call.target.as_str(),
                                &call.machine_arguments,
                                program.expression_table.expression_handles(call.arguments),
                                None,
                                !program.machine_type_parameters(machine).is_empty(),
                            ) {
                                upsert_selection(&mut selections, selection);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Nested calls retain their lexical caller even without a direct result
    // annotation. Their argument types can determine the specialization tuple.
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut expressions = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                if !matches!(statement, StatementNode::AssemblyFact(_)) {
                    collect_statement_expression_trees(program, statement, &mut expressions);
                }
            }
            // Erased proof-output calls retain the same lexical owner as
            // ordinary calls. The arena fallback cannot recover its binders.
            for call in &program.proof_output_calls {
                if call.machine_symbol == machine.symbol && call.state_symbol == state.symbol {
                    collect_expression_tree(program, call.call, &mut expressions);
                }
            }
            for expression in expressions {
                if covered_expressions.contains(&expression)
                    || contract_expressions.contains(&expression)
                {
                    continue;
                }
                let ExpressionNode::Call(call) = program.expression_table.expression(expression)
                else {
                    continue;
                };
                if typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
                    continue;
                }
                covered_expressions.push(expression);
                if let Some(selection) = selection_for_call(
                    program,
                    machine,
                    state,
                    candidates,
                    callee_states,
                    CallSite::Expression(expression),
                    call.target_symbol,
                    call.target.as_str(),
                    &call.machine_arguments,
                    program.expression_table.expression_handles(call.arguments),
                    None,
                    !program.machine_type_parameters(machine).is_empty(),
                ) {
                    upsert_selection(&mut selections, selection);
                }
            }
        }
    }

    // Calls outside executable states have no caller argument context, but explicit
    // static-machine arguments still determine a complete tuple through the
    // authored machine requirement. Preserve the old all-expression scan for
    // precisely that case.
    for (handle, expression) in program.expression_table.iter_expressions() {
        if covered_expressions.contains(&handle) || contract_expressions.contains(&handle) {
            continue;
        }
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        if typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
            continue;
        }
        let Some(callee) = resolve_callee(callee_states, call.target_symbol, call.target.as_str())
        else {
            continue;
        };
        let candidate = &candidates[callee.candidate_index];
        let mut machine_proposals = Vec::new();
        let mut evidence_proposals = Vec::new();
        let mut type_proposals = Vec::new();
        let mut const_proposals = Vec::new();
        let mut runtime_value_proposals = Vec::new();
        // Calls outside executable states have no argument evaluation edge
        // that could carry a runtime subject.
        collect_machine_proposals_for_callee(
            program,
            candidates,
            callee,
            &call.machine_arguments,
            None,
            &mut machine_proposals,
            &mut evidence_proposals,
            &mut type_proposals,
            &mut const_proposals,
            &mut runtime_value_proposals,
        );
        let selection = selection_from_proposals(
            program,
            CallSite::Expression(handle),
            callee,
            candidate,
            false,
            machine_proposals,
            evidence_proposals,
            type_proposals,
            const_proposals,
            runtime_value_proposals,
        );
        upsert_selection(&mut selections, selection);
    }

    selections
}

#[test]
pub(super) fn discarded_call_inference_retains_fixed_array_const_proposal() {
    let source = "machine endpoint<const N: u64[0..=3]>(witness: &[u8; N]) -> u64 { N }
        machine forward<Value>(unused: Value, witness: &[u8; 5]) { _ = endpoint(witness); }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolution");
    let mut program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typing");
    materialize_static_argument_types(&mut program);
    let candidates = candidate::collect(&program);
    let callees = candidate::callees(&program, &candidates);
    let contracts = contract_expression_handles(&program);
    let selections = collect_call_selections(&program, &candidates, &callees, &contracts);
    let endpoint = candidates
        .iter()
        .position(|candidate| candidate.template.template_name == "endpoint")
        .expect("endpoint candidate");
    let selected = selections
        .iter()
        .filter(|selection| selection.candidate_index == endpoint)
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1, "one discarded application");
    assert!(selected[0].caller_is_generic);
    let binding = selected[0].const_bindings[0]
        .expect("array witness must select const argument before discarded-result validation");
    assert_eq!(program.display_type_reference(binding), "5");
}

/// Every expression node reachable from an ordinary machine/state contract.
/// These nodes share the global expression arena with executable bodies, but
/// their static-machine selections quantify a proof schema and must never
/// trigger runtime monomorphization.
pub(super) fn contract_expression_handles(program: &TypedTrees) -> Vec<ExpressionHandle> {
    let mut handles = Vec::new();
    for machine in program.machines() {
        for contract in program.machine_contracts(machine) {
            collect_contract_facts(program, contract.facts, &mut handles);
        }
        for state in program.machine_states(machine) {
            for contract in program.state_contracts(state) {
                collect_contract_facts(program, contract.facts, &mut handles);
            }
        }
    }
    handles
}

pub(super) fn collect_contract_facts(
    program: &TypedTrees,
    facts: arena::HandleSpan<ProofFact>,
    handles: &mut Vec<ExpressionHandle>,
) {
    for fact in program.proof_facts.span_or_empty(facts) {
        match fact {
            ProofFact::Expression(expression) => {
                collect_expression_tree(program, *expression, handles)
            }
            ProofFact::Membership(membership) => {
                collect_expression_tree(program, membership.value, handles)
            }
            ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    collect_expression_tree(program, *argument, handles);
                }
            }
        }
    }
}

pub(crate) fn collect_expression_tree(
    program: &TypedTrees,
    expression: ExpressionHandle,
    handles: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() || handles.contains(&expression) {
        return;
    }
    handles.push(expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            collect_expression_tree(program, dispatch.subject, handles);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_expression_tree(program, pattern, handles);
                }
                collect_expression_tree(program, arm.value, handles);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_tree(program, *value, handles);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_tree(program, atomic.value, handles);
            collect_expression_tree(program, atomic.result, handles);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_tree(program, binary.left, handles);
            collect_expression_tree(program, binary.right, handles);
        }
        ExpressionNode::Cast(cast) => collect_expression_tree(program, cast.value, handles),
        ExpressionNode::Call(call) => {
            collect_expression_tree(program, call.receiver, handles);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_tree(program, *argument, handles);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_tree(program, indexed.collection, handles);
            collect_expression_tree(program, indexed.index, handles);
        }
        ExpressionNode::Member(member) => {
            collect_expression_tree(program, member.receiver, handles)
        }
        ExpressionNode::Borrow(inner) => collect_expression_tree(program, inner.target, handles),
        ExpressionNode::Range(range) => {
            collect_expression_tree(program, range.start, handles);
            collect_expression_tree(program, range.end, handles);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_expression_tree(program, field.value, handles);
            }
        }
        ExpressionNode::Unary(unary) => collect_expression_tree(program, unary.operand, handles),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Ordinal of the statement enclosing one call site inside its state. A
/// statement call is its own site; an expression call belongs to the
/// statement whose expression tree contains it.
pub(super) fn enclosing_statement_ordinal(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    site: CallSite,
) -> Option<usize> {
    match site {
        CallSite::Statement(handle) => {
            let start = state.statement_nodes.start().arena_index() as usize;
            let ordinal = handle.arena_index() as usize;
            (ordinal >= start && ordinal < start + state.statement_nodes.count() as usize)
                .then_some(ordinal - start)
        }
        CallSite::Expression(handle) => program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
            .find_map(|(offset, statement)| {
                let mut roots = Vec::new();
                collect_statement_expression_trees(program, statement, &mut roots);
                roots.contains(&handle).then_some(offset)
            }),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn selection_for_call(
    program: &TypedTrees,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    site: CallSite,
    target_symbol: SymbolHandle,
    target_name: &str,
    machine_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    expected_return: Option<TypeReferenceHandle>,
    caller_is_generic: bool,
) -> Option<CallSelection> {
    let callee = resolve_callee(callee_states, target_symbol, target_name)?;
    let candidate = &candidates[callee.candidate_index];
    let mut machine_proposals = Vec::new();
    let mut evidence_proposals = Vec::new();
    let mut type_proposals = Vec::new();
    let mut const_proposals = Vec::new();
    let mut runtime_value_proposals = Vec::new();
    // A runtime `Value` subject resolves only against the locals the call can
    // see: the caller's parameters and `let` declarations before its own
    // statement.
    let scope_limit =
        enclosing_statement_ordinal(program, caller_state, site).unwrap_or(usize::MAX);
    collect_call_proposals(
        program,
        caller_machine,
        caller_state,
        candidates,
        callee_states,
        target_symbol,
        target_name,
        machine_arguments,
        arguments,
        expected_return,
        scope_limit,
        &mut machine_proposals,
        &mut evidence_proposals,
        &mut type_proposals,
        &mut const_proposals,
        &mut runtime_value_proposals,
    );
    Some(selection_from_proposals(
        program,
        site,
        callee,
        candidate,
        caller_is_generic,
        machine_proposals,
        evidence_proposals,
        type_proposals,
        const_proposals,
        runtime_value_proposals,
    ))
}

pub(super) fn selection_from_proposals(
    program: &TypedTrees,
    site: CallSite,
    callee: &CalleeState,
    candidate: &Candidate,
    caller_is_generic: bool,
    machine_proposals: Vec<(usize, usize, StaticMachineArgument)>,
    evidence_proposals: Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: Vec<(usize, usize, TypeReferenceHandle)>,
    runtime_value_proposals: Vec<(usize, usize, StaticMachineArgument)>,
) -> CallSelection {
    let mut selection = CallSelection {
        site,
        callee_symbol: callee.symbol,
        candidate_index: callee.candidate_index,
        caller_is_generic,
        unresolved_machine_parameters: false,
        unresolved_evidence_parameters: false,
        unresolved_const_parameters: false,
        type_bindings: vec![None; candidate.template.type_parameters.len()],
        const_bindings: vec![None; candidate.template.const_parameters.len()],
        runtime_value_bindings: vec![None; candidate.template.const_parameters.len()],
        machine_bindings: vec![None; candidate.template.machine_parameters.len()],
        evidence_bindings: vec![None; candidate.template.evidence_parameters.len()],
        conflicted: false,
    };
    for (_, parameter, argument) in runtime_value_proposals {
        // A runtime subject never conflicts: distinct argument values share
        // one specialization keyed by the binder's declared carrier.
        selection.runtime_value_bindings[parameter] = Some(argument);
    }
    for (_, parameter, binding) in type_proposals {
        if type_reference_is_any_generic_parameter(program, binding) {
            continue;
        }
        match selection.type_bindings[parameter] {
            None => selection.type_bindings[parameter] = Some(binding),
            Some(existing) if !same_type_identity(program, existing, binding) => {
                selection.conflicted = true
            }
            Some(_) => {}
        }
    }
    for (_, parameter, binding) in const_proposals {
        if !binding.is_valid() || type_reference_is_any_generic_parameter(program, binding) {
            // A concrete proposal from another occurrence cannot erase the
            // need to check this occurrence after its caller specializes.
            selection.unresolved_const_parameters = true;
            continue;
        }
        match selection.const_bindings[parameter] {
            None => selection.const_bindings[parameter] = Some(binding),
            Some(existing) if !same_type_identity(program, existing, binding) => {
                selection.conflicted = true
            }
            Some(_) => {}
        }
    }
    for (_, parameter, binding) in machine_proposals {
        if program.symbols.get(binding.symbol).kind == SymbolKind::MachineParameter {
            // A forwarded binder from another generic caller is no more
            // concrete than a recursive self-binding.
            selection.unresolved_machine_parameters = true;
        }
        match &selection.machine_bindings[parameter] {
            None => selection.machine_bindings[parameter] = Some(binding),
            Some(existing) if existing.symbol != binding.symbol => selection.conflicted = true,
            Some(_) => {}
        }
    }
    for (_, parameter, binding) in evidence_proposals {
        if program.symbols.get(binding.symbol).kind == SymbolKind::ConformanceParameter {
            selection.unresolved_evidence_parameters = true;
        }
        match &selection.evidence_bindings[parameter] {
            None => selection.evidence_bindings[parameter] = Some(binding),
            Some(existing)
                if existing.symbol != binding.symbol
                    || existing.display_name() != binding.display_name() =>
            {
                selection.conflicted = true
            }
            Some(_) => {}
        }
    }
    selection
}

pub(super) fn type_reference_is_any_generic_parameter(
    program: &TypedTrees,
    binding: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(binding)
    else {
        return false;
    };
    if symbol.is_valid() && program.symbols.get(*symbol).kind == SymbolKind::TypeParameter {
        return true;
    }
    program.machines().iter().any(|machine| {
        program
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Type
                        | TypeParameterKind::Const { .. }
                        | TypeParameterKind::Value { .. }
                ) && (parameter.symbol == *symbol
                    || (!parameter.symbol.is_valid()
                        && !symbol.is_valid()
                        && parameter.name.as_str() == name.as_str()))
            })
    })
}

pub(super) fn upsert_selection(selections: &mut Vec<CallSelection>, selection: CallSelection) {
    if let Some(existing) = selections
        .iter_mut()
        .find(|existing| existing.site == selection.site)
    {
        let existing_evidence = existing
            .type_bindings
            .iter()
            .filter(|item| item.is_some())
            .count()
            + existing
                .const_bindings
                .iter()
                .filter(|item| item.is_some())
                .count()
            + existing
                .machine_bindings
                .iter()
                .filter(|item| item.is_some())
                .count()
            + existing
                .evidence_bindings
                .iter()
                .filter(|item| item.is_some())
                .count();
        let new_evidence = selection
            .type_bindings
            .iter()
            .filter(|item| item.is_some())
            .count()
            + selection
                .const_bindings
                .iter()
                .filter(|item| item.is_some())
                .count()
            + selection
                .machine_bindings
                .iter()
                .filter(|item| item.is_some())
                .count()
            + selection
                .evidence_bindings
                .iter()
                .filter(|item| item.is_some())
                .count();
        if new_evidence >= existing_evidence {
            *existing = selection;
        }
    } else {
        selections.push(selection);
    }
}

pub(super) fn unique_complete_selections(
    program: &TypedTrees,
    selections: &[CallSelection],
    candidate_index: usize,
) -> Vec<(SpecializationKey, Vec<usize>)> {
    let mut groups: Vec<(SpecializationKey, Vec<usize>)> = Vec::new();
    for (selection_index, selection) in selections.iter().enumerate() {
        if selection.candidate_index != candidate_index
            || selection.caller_is_generic
            || !selection.is_complete()
        {
            continue;
        }
        let key = SpecializationKey {
            type_arguments: selection
                .type_bindings
                .iter()
                .map(|binding| {
                    program
                        .normalized_type_identity(binding.expect("complete selection"))
                        .into_string()
                })
                .collect(),
            const_arguments: selection
                .const_bindings
                .iter()
                .map(|binding| {
                    program
                        .normalized_type_identity(binding.expect("complete selection"))
                        .into_string()
                })
                .collect(),
            machine_arguments: selection
                .machine_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete selection").symbol)
                .collect(),
            evidence_arguments: selection
                .evidence_bindings
                .iter()
                .map(|binding| {
                    crate::conformance_applications::close_conformance_application(
                        program,
                        binding.as_ref().expect("complete selection"),
                    )
                    .expect("validated complete conformance application")
                    .report_fingerprint
                })
                .collect(),
        };
        if let Some((_, members)) = groups.iter_mut().find(|(existing, _)| *existing == key) {
            members.push(selection_index);
        } else {
            groups.push((key, vec![selection_index]));
        }
    }
    groups
}

pub(super) fn infer_static_bindings(
    program: &TypedTrees,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    type_parameters: &[(SymbolHandle, String)],
    const_parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    fixed_range_parameters: Option<&[usize]>,
    candidate_index: usize,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    if !required.is_valid() || !actual.is_valid() {
        return;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) =
            type_parameters
                .iter()
                .position(|(parameter_symbol, parameter_name)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                })
    {
        type_proposals.push((candidate_index, index, actual));
        return;
    }
    if let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
        && let Some(index) =
            const_parameters
                .iter()
                .position(|(parameter_symbol, parameter_name, _)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                })
    {
        const_proposals.push((candidate_index, index, actual));
        return;
    }

    match (
        program.type_reference_table.type_reference(required),
        program.type_reference_table.type_reference(actual),
    ) {
        // Data normalization names concrete instances; their retained generic
        // applications still carry the exact argument evidence.
        (TypeReferenceNode::Generic { .. }, TypeReferenceNode::Named { symbol, .. })
            if symbol.is_valid() =>
        {
            if let Some(application) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
                .and_then(|definition| definition.generic_instance)
                .filter(|application| {
                    matches!(
                        program.type_reference_table.type_reference(*application),
                        TypeReferenceNode::Generic { .. }
                    )
                })
            {
                infer_static_bindings(
                    program,
                    required,
                    application,
                    type_parameters,
                    const_parameters,
                    fixed_range_parameters,
                    candidate_index,
                    type_proposals,
                    const_proposals,
                );
            }
        }
        (
            TypeReferenceNode::Reference {
                referee: required, ..
            },
            TypeReferenceNode::Reference {
                referee: actual, ..
            },
        ) => infer_static_bindings(
            program,
            *required,
            *actual,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
        // Borrow syntax is carried by the CALL edge, not as a wrapper on the
        // place expression itself. Thus `f<T>(&T)` called as `f(&place)` sees
        // the declared type of `place` here. Peel the requirement-side borrow
        // and infer from that place type; ordinary call validation separately
        // checks that the authored borrow mode is legal.
        (
            TypeReferenceNode::Reference {
                referee: required, ..
            },
            _,
        ) => infer_static_bindings(
            program,
            *required,
            actual,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
        (
            TypeReferenceNode::Constrained {
                base_type: required_base,
                constraints: required_constraints,
            },
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                constraints: actual_constraints,
            },
        ) => {
            infer_static_bindings(
                program,
                *required_base,
                *actual_base,
                type_parameters,
                const_parameters,
                // Inspect the entire constrained shell once below. Peeling a
                // range or policy here must not expose a different endpoint.
                None,
                candidate_index,
                type_proposals,
                const_proposals,
            );
            if let Some(fixed_parameters) = fixed_range_parameters {
                range_arguments::infer(
                    program,
                    required,
                    actual,
                    const_parameters,
                    fixed_parameters,
                    candidate_index,
                    const_proposals,
                );
            }
            infer_domain_argument_bindings(
                program,
                *required_constraints,
                *actual_constraints,
                type_parameters,
                const_parameters,
                fixed_range_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
        }
        (TypeReferenceNode::Constrained { base_type, .. }, _) => infer_static_bindings(
            program,
            *base_type,
            validation::unwrapped_type_reference(program, actual).unwrap_or(actual),
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
        (
            TypeReferenceNode::Slice {
                element_type: required,
            },
            TypeReferenceNode::Slice {
                element_type: actual,
            },
        ) => infer_static_bindings(
            program,
            *required,
            *actual,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
            type_proposals,
            const_proposals,
        ),
        (
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                length: required_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
        ) => {
            infer_fixed_array_length_binding(
                program,
                required_length,
                actual_length,
                const_parameters,
                candidate_index,
                const_proposals,
            );
            infer_static_bindings(
                program,
                *required_element,
                *actual_element,
                type_parameters,
                const_parameters,
                fixed_range_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
        }
        (
            TypeReferenceNode::Generic {
                base_name: required_base,
                arguments: required_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_name: actual_base,
                arguments: actual_arguments,
                ..
            },
        ) if required_base == actual_base => {
            for (required, actual) in program
                .type_reference_table
                .type_reference_handles(*required_arguments)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*actual_arguments),
                )
            {
                infer_static_bindings(
                    program,
                    *required,
                    *actual,
                    type_parameters,
                    const_parameters,
                    fixed_range_parameters,
                    candidate_index,
                    type_proposals,
                    const_proposals,
                );
            }
        }
        _ => {}
    }
}

pub(super) fn infer_fixed_array_length_binding(
    program: &TypedTrees,
    required: &typed_trees::types::FixedArrayLength,
    actual: &typed_trees::types::FixedArrayLength,
    const_parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    candidate_index: usize,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } = required else {
        return;
    };
    let Some(parameter_index) =
        const_parameters
            .iter()
            .position(|(parameter_symbol, parameter_name, _)| {
                parameter_symbol == symbol
                    || (!parameter_symbol.is_valid()
                        && !symbol.is_valid()
                        && parameter_name == name.as_str())
            })
    else {
        return;
    };
    let binding = match actual {
        typed_trees::types::FixedArrayLength::Literal(value) => {
            let value = value.to_string();
            program
                .type_reference_table
                .named_references()
                .find(|(_, candidate_symbol, candidate_name)| {
                    !candidate_symbol.is_valid() && *candidate_name == value
                })
                .map(|(handle, _, _)| handle)
        }
        typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } => program
            .type_reference_table
            .named_references()
            .find(|(_, candidate_symbol, candidate_name)| {
                candidate_symbol == symbol
                    || (!candidate_symbol.is_valid()
                        && !symbol.is_valid()
                        && *candidate_name == name.as_str())
            })
            .map(|(handle, _, _)| handle),
        typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
    };
    if let Some(binding) = binding {
        const_proposals.push((candidate_index, parameter_index, binding));
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn infer_domain_argument_bindings(
    program: &TypedTrees,
    required_constraints: HandleSpan<TypeConstraintNode>,
    actual_constraints: HandleSpan<TypeConstraintNode>,
    type_parameters: &[(SymbolHandle, String)],
    const_parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    fixed_range_parameters: Option<&[usize]>,
    candidate_index: usize,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    for required in program
        .type_reference_table
        .constraints(required_constraints)
    {
        let TypeConstraintNode::Domain(required) = required else {
            continue;
        };
        let Some(actual) = program
            .type_reference_table
            .constraints(actual_constraints)
            .iter()
            .find_map(|constraint| {
                let TypeConstraintNode::Domain(actual) = constraint else {
                    return None;
                };
                same_domain_family(required, actual).then_some(actual)
            })
        else {
            continue;
        };
        for (required, actual) in required.arguments.iter().zip(&actual.arguments) {
            infer_static_bindings(
                program,
                *required,
                *actual,
                type_parameters,
                const_parameters,
                fixed_range_parameters,
                candidate_index,
                type_proposals,
                const_proposals,
            );
        }
    }
}

pub(super) fn same_domain_family(
    left: &typed_trees::types::DomainConstraint,
    right: &typed_trees::types::DomainConstraint,
) -> bool {
    if left.symbol.is_valid() && right.symbol.is_valid() {
        return left.symbol == right.symbol;
    }
    left.name == right.name
        || left.name.as_str().rsplit("::").next() == right.name.as_str().rsplit("::").next()
}

pub(super) fn same_type_identity(
    program: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    program.normalized_type_identity(left) == program.normalized_type_identity(right)
}

pub(super) fn approved_type_bounds(program: &TypedTrees, candidates: &[Candidate]) -> Vec<bool> {
    let mut symbol_diagnostics = Vec::new();
    let symbols = validation::TopLevelSymbols::build(program, &mut symbol_diagnostics);
    candidates
        .iter()
        .map(|candidate| {
            candidate
                .template
                .parameter_bounds
                .iter()
                .zip(candidate.type_bindings.iter())
                .all(|(bounds, binding)| {
                    let Some(binding) = binding else {
                        return true;
                    };
                    let Some(unwrapped) = validation::unwrapped_type_reference(program, *binding)
                    else {
                        return false;
                    };
                    bounds.iter().all(|property| {
                        validation::type_satisfies_declared_property(
                            program,
                            &symbols,
                            &[],
                            unwrapped,
                            *property,
                        )
                    })
                })
        })
        .collect()
}

pub(super) fn validate_candidate_conformance_bounds(
    program: &TypedTrees,
    candidate: &mut Candidate,
) -> Result<(), Vec<Diagnostic>> {
    candidate.inferred_conformance_arguments.clear();
    candidate.selected_bound_applications.clear();
    let mut diagnostics = Vec::new();
    for bound in &candidate.template.conformance_bounds {
        let Some(parameter_index) = candidate
            .template
            .type_parameters
            .iter()
            .position(|(symbol, _)| *symbol == bound.subject)
        else {
            continue;
        };
        let Some(binding) = candidate.type_bindings[parameter_index] else {
            continue;
        };
        let Some(type_name) = concrete_data_type_name(program, binding) else {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{}`, which is not a nominal data type and cannot satisfy conformance bound `{}`",
                candidate.template.template_name,
                bound.subject_name,
                program.display_type_reference(binding),
                bound.carrier_name,
            )));
            continue;
        };
        let type_identity = program.display_type_reference(binding);

        if let Some(binder) = bound.binder {
            let Some(evidence_index) = candidate
                .template
                .evidence_parameters
                .iter()
                .position(|parameter| parameter.binder == Some(binder))
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` lost explicit conformance binder `{}` from its specialization telescope",
                    candidate.template.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                )));
                continue;
            };
            let Some(selected_binding) = candidate.evidence_bindings[evidence_index].as_ref()
            else {
                continue;
            };
            let selected_symbol = selected_binding.symbol;
            let Some(selected) = program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == selected_symbol)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` binds `{}` to a symbol that is not a package-scoped conformance",
                    candidate.template.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                )));
                continue;
            };
            let application = match crate::conformance_applications::close_conformance_application(
                program,
                selected_binding,
            ) {
                Ok(application) => application,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let expected_trait = program
                .traits()
                .iter()
                .find(|definition| definition.symbol == bound.carrier);
            if application.subject_identity.as_deref() != Some(type_identity.as_str())
                || expected_trait
                    .is_none_or(|definition| application.trait_definition != definition.symbol)
                || !conformance_application_arguments_match_candidate(
                    program,
                    candidate,
                    bound,
                    &application,
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` cannot bind `{}` to conformance `{}`: expected a complete `{type_identity} satisfies {}` map with the instantiated trait arguments",
                    candidate.template.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                    selected
                        .alias
                        .as_ref()
                        .map_or("<unnamed>", |name| name.as_str()),
                    bound.carrier_name,
                )));
            }
            continue;
        }

        if let Some(selected) = &bound.selected_conformance {
            let application = match close_candidate_bound_application(program, candidate, selected)
            {
                Ok(application) => application,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if application.subject_identity.as_deref() != Some(type_identity.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` binds `{}` to `{type_name}`, but named conformance `{}::{}` belongs to `{}`",
                    candidate.template.template_name,
                    bound.subject_name,
                    bound.carrier_name,
                    bound
                        .selected_conformance_name()
                        .map_or("<missing>", |name| name.as_str()),
                    application.subject_identity.as_deref().unwrap_or("<subjectless>"),
                )));
                continue;
            }
            candidate.selected_bound_applications.push(application);
            continue;
        }

        let matches = program
            .conformances()
            .iter()
            .filter(|conformance| {
                conformance
                    .carrier_name()
                    .is_some_and(|carrier| carrier.as_str() == type_name)
                    && conformance.trait_name == bound.carrier_name
                    && conformance_arguments_match_candidate(program, candidate, bound, conformance)
            })
            .map(|conformance| conformance.symbol)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [selected] => candidate.inferred_conformance_arguments.push(*selected),
            [] => diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{type_name}`, which has no nominal conformance to `{}`",
                candidate.template.template_name, bound.subject_name, bound.carrier_name,
            ))),
            matches => diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{type_name}`, which has {count} conformances to `{}`; select one with `where {} satisfies {type_name}::Name`",
                candidate.template.template_name,
                bound.subject_name,
                bound.carrier_name,
                bound.subject_name,
                count = matches.len(),
            ))),
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) fn close_candidate_bound_application(
    program: &TypedTrees,
    candidate: &Candidate,
    selected: &StaticMachineArgument,
) -> Result<typed_trees::typed_trees::ClosedConformanceApplication, Diagnostic> {
    let rewrites = forwarded_static_argument_rewrites(program, candidate);
    let mut applications = [selected.clone()];
    substitute_forwarded_machine_arguments(&mut applications, &rewrites, &[]);
    crate::conformance_applications::close_conformance_application(program, &applications[0])
}

pub(super) fn concrete_data_type_name(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&str> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => concrete_data_type_name(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            concrete_data_type_name(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid() && program.symbols.get(*symbol).kind == SymbolKind::Data =>
        {
            Some(name.as_str())
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } if base_symbol.is_valid()
            && program.symbols.get(*base_symbol).kind == SymbolKind::Data =>
        {
            Some(base_name.as_str())
        }
        _ => None,
    }
}

pub(super) fn conformance_application_arguments_match_candidate(
    program: &TypedTrees,
    candidate: &Candidate,
    bound: &typed_trees::machine::GenericConformanceBound,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) -> bool {
    let substitutions = candidate
        .template
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
        .filter_map(|((symbol, _), binding)| {
            binding.map(|binding| (*symbol, program.display_type_reference(binding)))
        })
        .chain(
            candidate
                .template
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .filter_map(|((symbol, _, _), binding)| {
                    binding.map(|binding| (*symbol, program.display_type_reference(binding)))
                }),
        )
        .collect::<Vec<_>>();
    let machine_lifetimes = program.machines()[candidate.template.machine_index]
        .lifetime_parameters
        .iter()
        .map(|parameter| {
            (
                parameter.as_str().to_owned(),
                "__ordinary_call_region".to_owned(),
            )
        })
        .collect::<Vec<_>>();
    application.trait_arguments.len() == bound.arguments.len()
        && bound
            .arguments
            .iter()
            .zip(application.trait_arguments.iter())
            .all(|(required, actual)| {
                let required =
                    crate::conformance_applications::substituted_type_identity_with_lifetimes(
                        program,
                        *required,
                        &substitutions,
                        &machine_lifetimes,
                    );
                let actual = application.lifetime_arguments.iter().fold(
                    actual.clone(),
                    |identity, lifetime| {
                        identity.replace(&format!("'{lifetime}"), "'__ordinary_call_region")
                    },
                );
                required == actual
            })
}

pub(super) fn conformance_arguments_match_candidate(
    program: &TypedTrees,
    candidate: &Candidate,
    bound: &typed_trees::machine::GenericConformanceBound,
    conformance: &typed_trees::trait_definition::Conformance,
) -> bool {
    let actual = program
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    actual.len() == bound.arguments.len()
        && bound
            .arguments
            .iter()
            .zip(actual.iter())
            .all(|(required, actual)| {
                let required = candidate
                    .template
                    .type_parameters
                    .iter()
                    .zip(candidate.type_bindings.iter())
                    .find_map(|((symbol, _), binding)| {
                        let TypeReferenceNode::Named {
                            symbol: required_symbol,
                            ..
                        } = program.type_reference_table.type_reference(*required)
                        else {
                            return None;
                        };
                        (*symbol == *required_symbol)
                            .then_some(binding.as_ref().copied())
                            .flatten()
                    })
                    .unwrap_or(*required);
                same_type_identity(program, required, *actual)
            })
}
