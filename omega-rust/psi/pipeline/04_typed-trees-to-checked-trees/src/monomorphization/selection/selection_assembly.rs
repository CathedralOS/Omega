//! Assembling and deduplicating selections from proposals.

use crate::monomorphization::selection::callee_proposals::{
    enclosing_statement_ordinal, resolve_callee,
};
use crate::monomorphization::selection::collect_call_proposals;
use crate::monomorphization::selection::static_bindings::same_type_identity;
use crate::monomorphization::{
    CallSelection, CallSite, CalleeState, Candidate, ExpressionHandle, SpecializationKey,
    StaticMachineArgument, SymbolHandle, SymbolKind, TypeParameterKind, TypeReferenceHandle,
    TypeReferenceNode, TypedTrees,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn selection_for_call(
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
    receiver_type: Option<TypeReferenceHandle>,
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
    let explicit_arguments = collect_call_proposals(
        program,
        caller_machine,
        caller_state,
        candidates,
        callee_states,
        target_symbol,
        target_name,
        machine_arguments,
        arguments,
        receiver_type,
        expected_return,
        scope_limit,
        &mut machine_proposals,
        &mut evidence_proposals,
        &mut type_proposals,
        &mut const_proposals,
        &mut runtime_value_proposals,
    );
    let mut selection = selection_from_proposals(
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
    );
    selection.explicit_argument_overflow =
        explicit_arguments == super::ExplicitArgumentCapacity::Exceeded;
    Some(selection)
}

pub(crate) fn selection_from_proposals(
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
        explicit_argument_overflow: false,
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

pub(crate) fn type_reference_is_any_generic_parameter(
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

pub(crate) fn upsert_selection(selections: &mut Vec<CallSelection>, selection: CallSelection) {
    if let Some(existing) = selections
        .iter_mut()
        .find(|existing| existing.site == selection.site)
    {
        let explicit_argument_overflow =
            existing.explicit_argument_overflow || selection.explicit_argument_overflow;
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
        existing.explicit_argument_overflow = explicit_argument_overflow;
    } else {
        selections.push(selection);
    }
}

pub(crate) fn unique_complete_selections(
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
                    crate::conformance::conformance_applications::close_conformance_application(
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
