//! Callee resolution, machine proposals, contract facts and expression
//! trees.

use validation::expression_traversal::{
    collect_contract_facts, collect_statement_expression_trees,
};

use crate::monomorphization::selection::static_bindings::infer_static_bindings;
use crate::monomorphization::{
    CallSite, CalleeState, Candidate, ExpressionHandle, StaticMachineArgument, SymbolHandle,
    SymbolKind, TypeReferenceHandle, const_arguments,
};
use typed_trees::const_integer::{self, ConstIntegerExpression};
use typed_trees::types::TypeReferenceNode;

/// An explicit argument has no binder slot of its selected kind. Omitted
/// arguments can still be inferred; excess arguments cannot be discarded.
pub(crate) struct StaticArgumentOverflow;

pub(crate) fn collect_machine_proposals_for_callee(
    program: &dyn ::validation::CheckingInput,
    candidates: &[Candidate],
    callee: &CalleeState,
    machine_arguments: &[StaticMachineArgument],
    runtime_scope: Option<(&typed_trees::state::State, usize)>,
    machine_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    evidence_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
    type_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    const_proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
    runtime_value_proposals: &mut Vec<(usize, usize, StaticMachineArgument)>,
) -> Result<(), StaticArgumentOverflow> {
    let candidate = &candidates[callee.candidate_index];
    let mut type_index = 0usize;
    let mut const_index = 0usize;
    let mut machine_index = 0usize;
    let mut evidence_index = 0usize;
    for selected in machine_arguments {
        if let Some(expression) = const_expression_argument(program, selected) {
            if const_index >= candidate.template.const_parameters.len() {
                return Err(StaticArgumentOverflow);
            }
            // An open `Degree - 1` waits for the caller to specialize. A
            // closed integer uses the same named binding as a decimal literal.
            match const_integer::classify(
                &program.expression_table,
                expression,
                &|symbol, spelling| const_binder(program, symbol, spelling),
            ) {
                Some(ConstIntegerExpression::Closed(value)) => {
                    if let Some((handle, _, _)) = program
                        .type_reference_table
                        .named_references()
                        .find(|(_, symbol, name)| !symbol.is_valid() && *name == value.to_string())
                    {
                        const_proposals.push((callee.candidate_index, const_index, handle));
                    }
                    const_index += 1;
                    continue;
                }
                Some(ConstIntegerExpression::Open) => {
                    const_index += 1;
                    continue;
                }
                None => {}
            }
        }
        let runtime_expression = const_arguments::runtime_subject_expression(program, selected);
        let runtime_field = runtime_scope.and_then(|(state, _)| {
            selected
                .evidence_projection
                .as_ref()
                .and_then(|projection| {
                    validation::runtime_field_subject(
                        program,
                        state,
                        &projection.term,
                        &projection.member,
                    )
                })
        });
        let value_slot = const_index < candidate.template.const_parameters.len()
            && candidate
                .template
                .value_const_parameters
                .contains(&const_index);
        if selected.type_reference.is_valid() && !(value_slot && runtime_expression.is_some()) {
            if type_index >= candidate.template.type_parameters.len() {
                return Err(StaticArgumentOverflow);
            }
            type_proposals.push((callee.candidate_index, type_index, selected.type_reference));
            type_index += 1;
            continue;
        }
        // A forwarded binder occupies its explicit slot even before it has a
        // closed value. Keep this call incomplete until the caller specializes;
        // neither range inference nor a later explicit argument may fill it.
        if const_arguments::forwarded_type(program, selected).is_valid() {
            if const_index >= candidate.template.const_parameters.len() {
                return Err(StaticArgumentOverflow);
            }
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
            if const_index >= candidate.template.const_parameters.len() {
                return Err(StaticArgumentOverflow);
            }
            if let Some((handle, _, _)) = program
                .type_reference_table
                .named_references()
                .find(|(_, symbol, name)| !symbol.is_valid() && *name == literal)
            {
                const_proposals.push((callee.candidate_index, const_index, handle));
            }
            const_index += 1;
            continue;
        }
        // A `Value` binder admits an ordinary runtime subject. Its slot binds
        // the declared carrier so every runtime argument of that carrier shares
        // one specialization; the resolved argument later becomes an appended
        // ordinary call argument.
        if const_index >= candidate.template.const_parameters.len()
            && (runtime_expression.is_some()
                || runtime_field.is_some()
                || runtime_scope.is_some_and(|(scope_state, scope_limit)| {
                    const_arguments::resolve_runtime_subject(
                        program,
                        scope_state,
                        scope_limit,
                        selected,
                    )
                    .is_some()
                }))
        {
            return Err(StaticArgumentOverflow);
        }
        if value_slot {
            if let Some((scope_state, scope_limit)) = runtime_scope
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
                runtime_value_proposals.push((
                    callee.candidate_index,
                    const_index,
                    subject_argument,
                ));
                const_index += 1;
                continue;
            }
            if runtime_expression.is_some() || runtime_field.is_some() {
                const_proposals.push((
                    callee.candidate_index,
                    const_index,
                    candidate.template.const_parameters[const_index].2,
                ));
                runtime_value_proposals.push((
                    callee.candidate_index,
                    const_index,
                    selected.clone(),
                ));
                const_index += 1;
                continue;
            }
        }
        if !selected.symbol.is_valid() {
            continue;
        }
        let kind = program.symbols.get(selected.symbol).kind;
        if matches!(
            kind,
            SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
        ) {
            if type_index >= candidate.template.type_parameters.len() {
                return Err(StaticArgumentOverflow);
            }
            if let Some((handle, _, _)) = program
                .type_reference_table
                .named_references()
                .find(|(_, symbol, _)| *symbol == selected.symbol)
            {
                type_proposals.push((callee.candidate_index, type_index, handle));
            }
            type_index += 1;
            continue;
        }
        if matches!(
            kind,
            SymbolKind::Conformance | SymbolKind::ConformanceParameter
        ) {
            if evidence_index >= candidate.template.evidence_parameters.len() {
                return Err(StaticArgumentOverflow);
            }
            evidence_proposals.push((callee.candidate_index, evidence_index, selected.clone()));
            evidence_index += 1;
            continue;
        }
        if !matches!(kind, SymbolKind::State | SymbolKind::MachineParameter) {
            continue;
        }
        if machine_index >= candidate.template.machine_parameters.len() {
            return Err(StaticArgumentOverflow);
        }
        machine_proposals.push((callee.candidate_index, machine_index, selected.clone()));
        let requirement = &candidate.template.machine_parameters[machine_index].2;
        machine_index += 1;
        let Some(actual_state) = state_by_symbol(program, selected.symbol) else {
            continue;
        };
        let first_type_proposal = type_proposals.len();
        let first_const_proposal = const_proposals.len();
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
        // Provider-local binders are not operands in the selecting caller's
        // telescope. Contextual closure owns their application; an open
        // provider cannot propose its arbitrary binder as a caller argument.
        let provider = program.machines().iter().find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == selected.symbol)
        });
        if let Some(provider) = provider {
            let provider_binders: Vec<_> = program
                .machine_type_parameters(provider)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect();
            let owns_binder = |binding| {
                super::super::contextual_calls::names_any(
                    program.typed_program(),
                    binding,
                    &provider_binders,
                )
            };
            let mut ordinal = 0usize;
            type_proposals.retain(|(_, _, binding)| {
                let keep = ordinal < first_type_proposal || !owns_binder(*binding);
                ordinal += 1;
                keep
            });
            let mut ordinal = 0usize;
            const_proposals.retain(|(_, _, binding)| {
                let keep = ordinal < first_const_proposal || !owns_binder(*binding);
                ordinal += 1;
                keep
            });
        }
    }
    Ok(())
}

/// Specialization consumes resolved declaration identity; it never resolves
/// names. An unresolved qualified call may denote an intrinsic or an invalid
/// member, not a same-spelled generic machine from another owner.
pub(crate) fn resolve_callee(
    callee_states: &[CalleeState],
    target_symbol: SymbolHandle,
) -> Option<&CalleeState> {
    if !target_symbol.is_valid() {
        return None;
    }
    callee_states
        .iter()
        .find(|callee| callee.symbol == target_symbol)
}

pub(crate) fn state_by_symbol(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    program.state_by_symbol(symbol)
}

/// Every expression node reachable from an ordinary machine/state contract.
/// These nodes share the global expression arena with executable bodies, but
/// their static-machine selections quantify a proof schema and must never
/// trigger runtime monomorphization.
pub(crate) fn contract_expression_handles(
    program: &dyn ::validation::CheckingInput,
) -> Vec<ExpressionHandle> {
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

/// Ordinal of the statement enclosing one call site inside its state. A
/// statement call is its own site; an expression call belongs to the
/// statement whose expression tree contains it.
pub(crate) fn enclosing_statement_ordinal(
    program: &dyn ::validation::CheckingInput,
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
            .indexed_statements(state.statement_nodes)
            .find_map(|(offset, statement)| {
                let mut roots = Vec::new();
                collect_statement_expression_trees(program, statement, &mut roots);
                roots.contains(&handle).then_some(offset)
            }),
    }
}

fn const_expression_argument(
    program: &dyn ::validation::CheckingInput,
    argument: &StaticMachineArgument,
) -> Option<ExpressionHandle> {
    if !argument.type_reference.is_valid() {
        return None;
    }
    match program
        .type_reference_table
        .type_reference(argument.type_reference)
    {
        TypeReferenceNode::ConstExpression(expression) => Some(*expression),
        _ => None,
    }
}

fn const_binder(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
    spelling: &str,
) -> bool {
    program.machines().iter().any(|machine| {
        program
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| {
                parameter.kind.is_const_or_value()
                    && ((symbol.is_valid() && parameter.symbol == symbol)
                        || parameter.name.as_str() == spelling)
            })
    })
}
