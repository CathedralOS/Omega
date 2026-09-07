//! Reconcile complete saved calls with instances already selected in this compilation.

use super::{CallSelection, CallSite, Candidate};
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::typed_trees::MachineSpecialization;

pub(super) fn replay(source: &mut TypedTrees, program: &TypedTrees, machine: &Machine) {
    let candidates = super::candidate::collect(source);
    let callees = super::candidate::callees(source, &candidates);
    let contracts = super::contract_expression_handles(source);
    let selections = super::collect_call_selections(source, &candidates, &callees, &contracts);
    let mut sites = Vec::new();
    let mut expressions = Vec::new();
    for state in source.machine_states(machine) {
        for handle in super::statement_span_handles(state.statement_nodes) {
            sites.push(CallSite::Statement(handle));
            for root in super::selected_operator_providers::executable_statement_expression_roots(
                source,
                source.statement_table.statement(handle),
            ) {
                super::collect_expression_tree(source, root, &mut expressions);
            }
        }
    }
    sites.extend(expressions.into_iter().map(CallSite::Expression));

    for selection in selections {
        let candidate = &candidates[selection.candidate_index];
        if !sites.contains(&selection.site) || !selection.is_complete()
            // Lexical self-state remapping happens during cloning. An exact
            // cross-instance self call needs replay after that remapping.
            || candidate.template_symbol == machine.symbol
        {
            continue;
        }
        let Some(instance) = selected_instance(source, program, candidate, &selection) else {
            continue;
        };
        let Some(state_offset) = candidate
            .state_symbols
            .iter()
            .position(|symbol| *symbol == selection.callee_symbol)
        else {
            continue;
        };
        let Some(instance_machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == instance.instance)
        else {
            continue;
        };
        let Some(state) = program.machine_states(instance_machine).get(state_offset) else {
            continue;
        };
        // A cloned callee may exist only in the live graph. Obtain its name
        // there, then rewrite only this saved executable occurrence.
        super::rewrite_selected_call_with_name(
            source,
            selection.site,
            state.symbol,
            state.name.clone(),
        );
    }
}

fn selected_instance<'program>(
    source: &TypedTrees,
    program: &'program TypedTrees,
    candidate: &Candidate,
    selection: &CallSelection,
) -> Option<&'program MachineSpecialization> {
    let type_arguments = selection
        .type_bindings
        .iter()
        .map(|binding| {
            binding.map(|binding| source.normalized_type_identity(binding).into_string())
        })
        .collect::<Option<Vec<_>>>()?;
    let const_arguments = selection
        .const_bindings
        .iter()
        .map(|binding| {
            binding.map(|binding| source.normalized_type_identity(binding).into_string())
        })
        .collect::<Option<Vec<_>>>()?;
    let machine_arguments = selection
        .machine_bindings
        .iter()
        .map(|binding| binding.as_ref().map(|binding| binding.symbol))
        .collect::<Option<Vec<_>>>()?;
    let evidence_arguments = selection
        .evidence_bindings
        .iter()
        .map(|binding| binding.as_ref())
        .collect::<Option<Vec<_>>>()?;
    let mut applications = evidence_arguments
        .iter()
        .map(|binding| {
            crate::conformance_applications::close_conformance_application(source, binding)
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let mut concrete = super::candidate_for_selection(candidate, selection);
    super::validate_candidate_conformance_bounds(source, &mut concrete).ok()?;
    applications.extend(concrete.selected_bound_applications);
    program.machine_specializations.iter().find(|instance| {
        instance.template == candidate.template_symbol
            && instance.type_argument_identities == type_arguments
            && instance.const_argument_identities == const_arguments
            && instance.machine_arguments == machine_arguments
            && instance.conformance_arguments.iter().copied()
                .eq(evidence_arguments.iter().map(|binding| binding.symbol))
            && instance.inferred_conformance_arguments == concrete.inferred_conformance_arguments
            // Compare explicit arguments and selected bound applications, including
            // their complete records rather than report fingerprints.
            && instance.conformance_applications == applications
    })
}

#[cfg(test)]
mod tests;
