use super::{CalleeState, Candidate};
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;

pub(super) fn from_machine(program: &TypedTrees, machine_index: usize) -> Candidate {
    let machine = &program.machines()[machine_index];
    let parameters = program.machine_type_parameters(machine);
    let mut type_parameters = Vec::new();
    let mut parameter_bounds = Vec::new();
    let mut const_parameters = Vec::new();
    let mut value_const_parameters = Vec::new();
    let mut machine_parameters = Vec::new();
    for parameter in parameters {
        match &parameter.kind {
            TypeParameterKind::Type => {
                type_parameters.push((parameter.symbol, parameter.name.as_str().to_owned()));
                parameter_bounds.push(validation::declared_property_requirements(
                    &parameter.bounds,
                ));
            }
            TypeParameterKind::Const { type_reference } => const_parameters.push((
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                *type_reference,
            )),
            // A `Value` binder binds through the static const path while its
            // argument is statically known; dynamic realization is deferred.
            TypeParameterKind::Value { type_reference } => {
                value_const_parameters.push(const_parameters.len());
                const_parameters.push((
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    *type_reference,
                ));
            }
            TypeParameterKind::Machine { contract } => {
                let signature = program
                    .machine_parameter_contract_view(contract)
                    .expect("typed machine parameter has a contract")
                    .signature();
                machine_parameters.push((
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    signature.clone(),
                ));
            }
            // Proposition parameters belong to trait abstraction surfaces,
            // not executable machines.
            TypeParameterKind::Proposition { .. } => {}
        }
    }
    let evidence_parameters = machine
        .conformance_bounds
        .iter()
        .filter(|bound| bound.binder.is_some())
        .cloned()
        .collect::<Vec<_>>();
    Candidate {
        machine_index,
        template_symbol: machine.symbol,
        template_name: machine.name.as_str().to_owned(),
        state_symbols: program
            .machine_states(machine)
            .iter()
            .map(|state| state.symbol)
            .collect(),
        type_bindings: vec![None; type_parameters.len()],
        const_bindings: vec![None; const_parameters.len()],
        machine_bindings: vec![None; machine_parameters.len()],
        evidence_bindings: vec![None; evidence_parameters.len()],
        type_parameters,
        parameter_bounds,
        conformance_bounds: machine.conformance_bounds.clone(),
        const_parameters,
        value_const_parameters,
        machine_parameters,
        evidence_parameters,
        inferred_conformance_arguments: Vec::new(),
        selected_bound_applications: Vec::new(),
        conflicted: false,
    }
}

pub(super) fn collect(program: &TypedTrees) -> Vec<Candidate> {
    program
        .machines()
        .iter()
        .enumerate()
        .filter(|(_, machine)| !program.machine_type_parameters(machine).is_empty())
        .map(|(machine_index, _)| from_machine(program, machine_index))
        .collect()
}

pub(super) fn callees(program: &TypedTrees, candidates: &[Candidate]) -> Vec<CalleeState> {
    candidates
        .iter()
        .enumerate()
        .flat_map(|(candidate_index, candidate)| {
            program
                .machine_states(&program.machines()[candidate.machine_index])
                .iter()
                .map(move |state| CalleeState {
                    symbol: state.symbol,
                    name: state.name.as_str().to_owned(),
                    candidate_index,
                    return_type: state.return_type,
                    parameter_types: program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| parameter.type_reference)
                        .collect(),
                })
        })
        .collect()
}
