//! Close domain-selected applications with the ordinary static tuple machinery.
//! These are declaration-owned semantic edges, not synthetic source calls.
//! Open tuples retain their exact bindings for universal checking; only closed
//! tuples select executable instances through the shared specialization cache.

use super::{CalleeState, Diagnostic, SymbolHandle, TypedTrees};

pub(super) fn materialize(program: &mut TypedTrees) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let requests = program
        .data_definitions()
        .iter()
        .filter_map(|data| data.cleanup_selection)
        .chain(
            program
                .domain_definitions()
                .iter()
                .filter_map(|domain| domain.cleanup_selection),
        )
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut created = Vec::new();
    for request in requests {
        let application = program
            .expression_table
            .static_machine_application(request.application)
            .clone();
        let Some(machine_index) =
            program.machine_position_by_symbol(application.registration_operation)
        else {
            return Err(vec![Diagnostic::error(
                "cleanup application does not select an independent machine",
            )]);
        };
        let machine = program.machines()[machine_index].clone();
        let Some(entry) = program.machine_states(&machine).first().cloned() else {
            return Err(vec![Diagnostic::error(
                "selected cleanup machine has no entry",
            )]);
        };
        let mut candidate = super::candidate::from_machine(program, machine_index);
        candidate.binding_application = request.application;
        let arguments = program
            .expression_table
            .static_machine_application_arguments(request.application)
            .to_vec();
        let callee = CalleeState {
            symbol: entry.symbol,
            candidate_index: 0,
            return_type: entry.return_type,
            parameter_types: program
                .state_parameters(&entry)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect(),
        };
        let mut proposals = super::selection::StaticProposals::default();
        super::selection::collect_machine_proposals_for_callee(
            program,
            std::slice::from_ref(&candidate),
            &callee,
            &arguments,
            None,
            &mut proposals,
        )
        .map_err(|_| {
            vec![Diagnostic::error(
                "cleanup application supplies arguments beyond its declared telescope",
            )]
        })?;
        let bindings = super::selection::proposal_bindings(program, &candidate, proposals);
        let open = bindings.unresolved_type_parameters
            || bindings.unresolved_const_parameters
            || bindings.unresolved_machine_parameters
            || bindings.unresolved_evidence_parameters;
        candidate.type_bindings = bindings.type_bindings;
        candidate.const_bindings = bindings.const_bindings;
        candidate.runtime_value_bindings = bindings.runtime_value_bindings;
        candidate.machine_bindings = bindings.machine_bindings;
        candidate.evidence_bindings = bindings.evidence_bindings;
        candidate.conflicted = bindings.conflicted;
        if candidate.conflicted
            || candidate.type_bindings.iter().any(Option::is_none)
            || candidate.const_bindings.iter().any(Option::is_none)
            || candidate.machine_bindings.iter().any(Option::is_none)
            || candidate.evidence_bindings.iter().any(Option::is_none)
        {
            return Err(vec![Diagnostic::error(format!(
                "cleanup application `{}` must supply a complete consistent static tuple",
                machine.name.as_str(),
            ))]);
        }
        if candidate.runtime_value_bindings.iter().any(Option::is_some) {
            return Err(vec![Diagnostic::error(
                "cleanup application cannot require runtime arguments beyond its explicit value parameter",
            )]);
        }
        let retained = super::applications::candidate_bindings(program, &candidate);
        program
            .expression_table
            .set_static_machine_application_bindings(request.application, &retained);
        if open {
            selected.push((request.application, SymbolHandle::invalid()));
            continue;
        }
        let executable = if program.machine_type_parameters(&machine).is_empty()
            && machine.conformance_bounds.is_empty()
        {
            machine.symbol
        } else if let Some(instance) =
            super::saved_calls::candidate_instance(program, program, &candidate)
        {
            instance.instance
        } else {
            let ordinal = program
                .machine_specializations
                .iter()
                .filter(|instance| instance.template == machine.symbol)
                .count();
            let identity =
                validation::contract_identities::normalized_machine_identity(program, &machine)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "cleanup template has no normalized callable identity",
                        )]
                    })?;
            let commitment = validation::contract_identities::accepted_template_commitment(
                program,
                machine_index,
            );
            let states = super::body_cloning::clone_specialized_machine(
                None, program, &candidate, ordinal, identity, commitment,
            )?;
            let entry = states
                .iter()
                .find_map(|(source, target)| (*source == entry.symbol).then_some(*target))
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "cleanup specialization lost its entry correspondence",
                    )]
                })?;
            let instance = program
                .machine_holding_state(entry)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "cleanup specialization entry lost its machine",
                    )]
                })?
                .symbol;
            created.push(instance);
            instance
        };
        selected.push((request.application, executable));
    }
    let definitions = program.roots.data_definitions;
    for data in program
        .tables
        .data_definitions
        .span_mut_or_empty(definitions)
    {
        if let Some(selection) = &mut data.cleanup_selection
            && let Some((_, executable)) = selected
                .iter()
                .find(|(application, _)| *application == selection.application)
        {
            selection.executable = *executable;
        }
    }
    let definitions = program.roots.domain_definitions;
    for domain in program
        .tables
        .domain_definitions
        .span_mut_or_empty(definitions)
    {
        if let Some(selection) = &mut domain.cleanup_selection
            && let Some((_, executable)) = selected
                .iter()
                .find(|(application, _)| *application == selection.application)
        {
            selection.executable = *executable;
        }
    }
    Ok(created)
}
