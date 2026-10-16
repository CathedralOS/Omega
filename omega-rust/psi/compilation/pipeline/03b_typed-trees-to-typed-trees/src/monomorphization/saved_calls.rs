//! Reconcile complete saved calls with instances already selected in this compilation.

use super::{CallSelection, CallSite, Candidate};
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::typed_trees::MachineSpecialization;

pub(super) fn replay(
    source: &mut TypedTrees,
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
) {
    let candidates = super::candidate::collect(source);
    let callees = super::candidate::callees(source, &candidates);
    let contracts = crate::monomorphization::selection::contract_expression_handles(source);
    let selections = crate::monomorphization::selection::collect_call_selections(
        source,
        &candidates,
        &callees,
        &contracts,
    );
    let mut sites = Vec::new();
    let mut expressions = Vec::new();
    for state in source.machine_states(machine) {
        for handle in
            crate::monomorphization::body_rewriting::statement_span_handles(state.statement_nodes)
        {
            sites.push(CallSite::Statement(handle));
            for root in super::operator_candidates::executable_statement_expression_roots(
                source,
                source.statement_table.statement(handle),
            ) {
                validation::expression_traversal::collect_expression_tree(
                    source,
                    root,
                    &mut expressions,
                );
            }
        }
    }
    sites.extend(expressions.into_iter().map(CallSite::Expression));

    for selection in selections {
        let candidate = &candidates[selection.candidate_index];
        if !sites.contains(&selection.site) || !selection.is_complete()
            // Lexical self-state remapping happens during cloning. An exact
            // cross-instance self call needs replay after that remapping.
            || candidate.template.template_symbol == machine.symbol
        {
            continue;
        }
        let concrete = super::candidate_for_selection(source, candidate, &selection);
        super::applications::retain_call(source, &concrete, &selection);
        let Some(instance) = selected_instance(source, program, candidate, &selection) else {
            continue;
        };
        let Some(state_offset) = candidate
            .template
            .state_symbols
            .iter()
            .position(|symbol| *symbol == selection.callee_symbol)
        else {
            continue;
        };
        let Some(instance_machine) = program.machine_by_symbol(instance.instance) else {
            continue;
        };
        let Some(state) = program.machine_states(instance_machine).get(state_offset) else {
            continue;
        };
        // A cloned callee may exist only in the live graph. Obtain its name
        // there, then rewrite only this saved executable occurrence.
        let arguments = selection
            .runtime_value_bindings
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        let subjects = crate::monomorphization::body_rewriting::runtime_value_operands(
            source,
            selection.lexical_state,
            &arguments,
        );
        crate::monomorphization::body_rewriting::rewrite_selected_call_with_name(
            source,
            selection.site,
            state.symbol,
            state.name.clone(),
            &subjects,
        );
    }
}

pub(super) fn selected_instance<'program>(
    source: &dyn ::validation::CheckingInput,
    program: &'program dyn ::validation::CheckingInput,
    candidate: &Candidate,
    selection: &CallSelection,
) -> Option<&'program MachineSpecialization> {
    let concrete = super::candidate_for_selection(source, candidate, selection);
    candidate_instance(source, program, &concrete)
}

pub(super) fn candidate_instance<'program>(
    source: &dyn ::validation::CheckingInput,
    program: &'program dyn ::validation::CheckingInput,
    candidate: &Candidate,
) -> Option<&'program MachineSpecialization> {
    let type_arguments = candidate
        .type_bindings
        .iter()
        .map(|binding| binding.map(|binding| source.normalized_type_identity_text(binding)))
        .collect::<Option<Vec<_>>>()?;
    let const_arguments = candidate
        .const_bindings
        .iter()
        .map(|binding| binding.map(|binding| source.normalized_type_identity_text(binding)))
        .collect::<Option<Vec<_>>>()?;
    let machine_arguments = candidate
        .machine_bindings
        .iter()
        .map(|binding| binding.as_ref().map(|binding| binding.symbol))
        .collect::<Option<Vec<_>>>()?;
    let evidence_arguments = candidate
        .evidence_bindings
        .iter()
        .map(|binding| binding.as_ref())
        .collect::<Option<Vec<_>>>()?;
    let applications = evidence_arguments
        .iter()
        .map(|binding| {
            validation::conformance_applications::close_conformance_application(source, binding)
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    program.machine_specializations.iter().find(|instance| {
        instance.template == candidate.template.template_symbol
            && instance.type_argument_identities == type_arguments
            && instance.const_argument_identities == const_arguments
            && instance.machine_arguments == machine_arguments
            && instance.conformance_arguments.iter().copied()
                .eq(evidence_arguments.iter().map(|binding| binding.symbol))
            // Compare explicit evidence applications by their complete records
            // rather than report fingerprints.
            && instance.conformance_applications == applications
    })
}
