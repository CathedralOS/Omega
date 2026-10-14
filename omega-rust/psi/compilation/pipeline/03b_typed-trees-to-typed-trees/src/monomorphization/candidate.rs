use super::{CalleeState, Candidate, CandidateTemplate};
use typed_trees::data::TypeParameterKind;

pub(super) fn from_machine(
    program: &dyn ::validation::CheckingInput,
    machine_index: usize,
) -> Candidate<'static> {
    let machine = &program.machines()[machine_index];
    from_parameters(
        program,
        machine_index,
        program.machine_type_parameters(machine),
        machine.conformance_bounds.clone(),
    )
}

pub(super) fn from_capture(
    program: &dyn ::validation::CheckingInput,
    machine_index: usize,
    conformance: &typed_trees::trait_definition::Conformance,
) -> Candidate<'static> {
    from_parameters(
        program,
        machine_index,
        program.conformance_type_parameters(conformance),
        Vec::new(),
    )
}

fn from_parameters(
    program: &dyn ::validation::CheckingInput,
    machine_index: usize,
    parameters: &[typed_trees::data::TypeParameter],
    evidence_parameters: Vec<typed_trees::machine::GenericConformanceBound>,
) -> Candidate<'static> {
    let machine = &program.machines()[machine_index];
    let mut type_parameters = Vec::new();
    let mut const_parameters = Vec::new();
    let mut value_const_parameters = Vec::new();
    let mut machine_parameters = Vec::new();
    for parameter in parameters {
        match &parameter.kind {
            TypeParameterKind::Type => {
                type_parameters.push((parameter.symbol, parameter.name.as_str().to_owned()));
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
            // Implementation and proposition parameters belong to trait abstraction surfaces,
            // not executable machines.
            TypeParameterKind::Implementation | TypeParameterKind::Proposition { .. } => {}
        }
    }
    Candidate {
        binding_application:
            typed_trees::static_applications::StaticMachineApplicationHandle::invalid(),
        type_bindings: vec![None; type_parameters.len()],
        const_bindings: vec![None; const_parameters.len()],
        runtime_value_bindings: vec![None; const_parameters.len()],
        machine_bindings: vec![None; machine_parameters.len()],
        evidence_bindings: vec![None; evidence_parameters.len()],
        template: std::borrow::Cow::Owned(CandidateTemplate {
            machine_index,
            template_symbol: machine.symbol,
            template_name: machine.name.as_str().to_owned(),
            state_symbols: program
                .machine_states(machine)
                .iter()
                .map(|state| state.symbol)
                .collect(),
            type_parameters,
            const_parameters,
            value_const_parameters,
            machine_parameters,
            evidence_parameters,
        }),
        conflicted: false,
    }
}

pub(super) fn collect(program: &dyn ::validation::CheckingInput) -> Vec<Candidate<'static>> {
    program
        .machines()
        .iter()
        .enumerate()
        .filter(|(_, machine)| machine.has_specialization_parameters())
        .map(|(machine_index, _)| from_machine(program, machine_index))
        .collect()
}

pub(super) fn callees(
    program: &dyn ::validation::CheckingInput,
    candidates: &[Candidate],
) -> Vec<CalleeState> {
    candidates
        .iter()
        .enumerate()
        .flat_map(|(candidate_index, candidate)| {
            program
                .machine_states(&program.machines()[candidate.template.machine_index])
                .iter()
                .map(move |state| {
                    let parameters = program.state_parameters(state);
                    CalleeState {
                        symbol: state.symbol,
                        candidate_index,
                        return_type: state.return_type,
                        parameter_types: parameters
                            .iter()
                            .map(|parameter| parameter.type_reference)
                            .collect(),
                    }
                })
        })
        .collect()
}
