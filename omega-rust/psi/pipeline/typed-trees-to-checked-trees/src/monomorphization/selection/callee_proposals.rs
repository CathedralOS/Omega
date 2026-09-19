//! Callee resolution, machine proposals, contract facts and expression
//! trees.

use crate::monomorphization::selection::static_bindings::infer_static_bindings;
use crate::monomorphization::{
    CallSite, CalleeState, Candidate, ExpressionHandle, ExpressionNode, ProofFact,
    StaticMachineArgument, SymbolHandle, SymbolKind, TypeReferenceHandle, TypedTrees,
    collect_statement_expression_trees, const_arguments,
};
#[cfg(test)]
use crate::monomorphization::{
    candidate, collect_call_selections, materialize_static_argument_types,
};

pub(crate) fn collect_machine_proposals_for_callee(
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
        if selected.type_reference.is_valid() {
            if type_index < candidate.template.type_parameters.len() {
                type_proposals.push((callee.candidate_index, type_index, selected.type_reference));
            }
            type_index += 1;
            continue;
        }
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

pub(crate) fn resolve_callee<'a>(
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

pub(crate) fn state_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.symbol == symbol)
}

#[test]
pub(crate) fn discarded_call_inference_retains_fixed_array_const_proposal() {
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
pub(crate) fn contract_expression_handles(program: &TypedTrees) -> Vec<ExpressionHandle> {
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

pub(crate) fn collect_contract_facts(
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
pub(crate) fn enclosing_statement_ordinal(
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
