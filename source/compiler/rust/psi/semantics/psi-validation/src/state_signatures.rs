use crate::proof_facts::{ProofFactOwner, validate_proof_facts};
use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::signature::{SignatureContract, StateParameter};
use psi_typed_trees::types::TypeReferenceHandle;
use std::fmt;

pub(crate) fn validate_callable_state_signatures(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let mut type_parameters = program.machine_type_parameters(machine).to_vec();
        let mut lifetime_parameters = machine.lifetime_parameters.clone();
        if let Some(attached_data) = &machine.attached_data
            && let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name == *attached_data)
        {
            for parameter in program.data_type_parameters(definition) {
                if !type_parameters
                    .iter()
                    .any(|existing| existing.name == parameter.name)
                {
                    type_parameters.push(parameter.clone());
                }
            }
            for parameter in &definition.lifetime_parameters {
                if !lifetime_parameters
                    .iter()
                    .any(|existing| existing == parameter)
                {
                    lifetime_parameters.push(parameter.clone());
                }
            }
        }
        validate_state_signature_types(
            program
                .machine_states(machine)
                .iter()
                .map(|state| StateSignatureView {
                    name: state.name.as_str(),
                    lifetime_parameters: &[],
                    parameters: program.state_parameters(state),
                    return_type: state.return_type,
                    contracts: program.state_contracts(state),
                }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Machine(machine.name.as_str()),
            &type_parameters,
            &lifetime_parameters,
        );
        validate_machine_parameter_signatures(
            program,
            symbols,
            diagnostics,
            program.machine_type_parameters(machine),
            &type_parameters,
            &lifetime_parameters,
            machine.name.as_str(),
        );
    }

    for definition in program.data_definitions() {
        let type_parameters = program.data_type_parameters(definition);
        validate_machine_parameter_signatures(
            program,
            symbols,
            diagnostics,
            type_parameters,
            type_parameters,
            &definition.lifetime_parameters,
            definition.name.as_str(),
        );
    }

    for trait_definition in program.traits() {
        for machine in program.trait_machine_signatures(trait_definition) {
            let mut type_parameters = program.trait_type_parameters(trait_definition).to_vec();
            type_parameters.extend_from_slice(program.state_signature_type_parameters(machine));
            for lifetime in &machine.lifetime_parameters {
                if trait_definition
                    .lifetime_parameters
                    .iter()
                    .any(|inherited| inherited == lifetime)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "trait requirement `{}` redeclares inherited lifetime `'{}'",
                        machine.name,
                        lifetime.as_str()
                    )));
                }
            }
            let mut lifetime_parameters = trait_definition.lifetime_parameters.clone();
            lifetime_parameters.extend_from_slice(&machine.lifetime_parameters);
            validate_state_signature_types(
                std::iter::once(StateSignatureView {
                    name: machine.name.as_str(),
                    lifetime_parameters: &machine.lifetime_parameters,
                    parameters: program.state_signature_parameters(machine),
                    return_type: machine.return_type,
                    contracts: program.state_signature_contracts(machine),
                }),
                program,
                symbols,
                diagnostics,
                StateSignatureOwner::Trait(trait_definition.name.as_str()),
                &type_parameters,
                &lifetime_parameters,
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StateSignatureView<'program> {
    name: &'program str,
    lifetime_parameters: &'program [Identifier],
    parameters: &'program [StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &'program [SignatureContract],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StateSignatureOwner<'program> {
    Machine(&'program str),
    Trait(&'program str),
    Requirement(&'program str),
}

impl fmt::Display for StateSignatureOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Machine(machine) => write!(formatter, "machine `{machine}`"),
            Self::Trait(trait_definition) => write!(formatter, "trait `{trait_definition}`"),
            Self::Requirement(parameter) => {
                write!(formatter, "machine-parameter requirement `{parameter}`")
            }
        }
    }
}

fn validate_machine_parameter_signatures<'program>(
    program: &'program TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    parameters: &'program [psi_typed_trees::data::TypeParameter],
    inherited_type_parameters: &[psi_typed_trees::data::TypeParameter],
    inherited_lifetime_parameters: &[Identifier],
    declaration: &'program str,
) {
    for parameter in parameters {
        let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        let contract = program
            .machine_parameter_contract_view(contract)
            .expect("typed machine-parameter contract must retain a valid requirement identity")
            .signature();
        let nested = program.state_signature_type_parameters(contract);
        for (index, nested_parameter) in nested.iter().enumerate() {
            if nested[..index]
                .iter()
                .any(|previous| previous.name == nested_parameter.name)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine-parameter requirement `{}` on `{declaration}` has duplicate generic parameter `{}`",
                    parameter.name, nested_parameter.name
                )));
            }
        }

        let mut local_type_parameters = inherited_type_parameters.to_vec();
        let mut local_lifetime_parameters = inherited_lifetime_parameters.to_vec();
        for nested_parameter in nested {
            if !local_type_parameters
                .iter()
                .any(|existing| existing.symbol == nested_parameter.symbol)
            {
                local_type_parameters.push(nested_parameter.clone());
            }
        }
        for nested_parameter in &contract.lifetime_parameters {
            if !local_lifetime_parameters
                .iter()
                .any(|existing| existing == nested_parameter)
            {
                local_lifetime_parameters.push(nested_parameter.clone());
            }
        }
        validate_state_signature_types(
            std::iter::once(StateSignatureView {
                name: contract.name.as_str(),
                lifetime_parameters: &contract.lifetime_parameters,
                parameters: program.state_signature_parameters(contract),
                return_type: contract.return_type,
                contracts: program.state_signature_contracts(contract),
            }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Requirement(parameter.name.as_str()),
            &local_type_parameters,
            &local_lifetime_parameters,
        );
        validate_machine_parameter_signatures(
            program,
            symbols,
            diagnostics,
            nested,
            &local_type_parameters,
            &local_lifetime_parameters,
            declaration,
        );
    }
}

fn validate_state_signature_types<'program>(
    signatures: impl Iterator<Item = StateSignatureView<'program>>,
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: StateSignatureOwner<'program>,
    type_parameters: &[psi_typed_trees::data::TypeParameter],
    inherited_lifetime_parameters: &[Identifier],
) {
    for signature in signatures {
        let mut lifetime_parameters = inherited_lifetime_parameters.to_vec();
        for parameter in signature.lifetime_parameters {
            if !lifetime_parameters
                .iter()
                .any(|existing| existing == parameter)
            {
                lifetime_parameters.push(parameter.clone());
            }
        }
        validate_state_parameter_names(signature, owner, diagnostics);
        validate_state_signature_contracts(program, signature, owner, diagnostics);

        for parameter in signature.parameters {
            if parameter.is_self {
                continue;
            }

            validate_type_reference_handle_with_type_parameters(
                program,
                parameter.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateParameter {
                    owner,
                    state: signature.name,
                    parameter: parameter.name.as_str(),
                    generic_depth: 0,
                },
                type_parameters,
                &lifetime_parameters,
            );
        }

        if signature.return_type.is_valid() {
            validate_type_reference_handle_with_type_parameters(
                program,
                signature.return_type,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateReturn {
                    owner,
                    state: signature.name,
                    generic_depth: 0,
                },
                type_parameters,
                &lifetime_parameters,
            );
        }
    }
}

fn validate_state_signature_contracts(
    program: &TypedTrees,
    signature: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in signature.contracts {
        if matches!(owner, StateSignatureOwner::Machine(_))
            && contract.kind != psi_typed_trees::signature::SignatureContractKind::Requires
        {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` admits only arrival `requires` contracts",
                signature.name
            )));
            continue;
        }
        validate_named_evidence_binding(
            program,
            &format!("{owner} state `{}`", signature.name),
            contract,
            diagnostics,
        );
        validate_crash_route_shapes(program, contract, diagnostics);
        validate_proof_facts(
            program,
            program.proof_facts.span_or_empty(contract.facts),
            diagnostics,
            ProofFactOwner::StateSignatureContract {
                owner,
                state: signature.name,
                kind: contract_kind_label(&contract.kind),
            },
        );
    }
}

pub(crate) fn validate_machine_contracts(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in program.machine_contracts(machine) {
        validate_named_evidence_binding(
            program,
            &format!("machine `{}`", machine.name),
            contract,
            diagnostics,
        );
        validate_crash_route_shapes(program, contract, diagnostics);
        validate_proof_facts(
            program,
            program.proof_facts.span_or_empty(contract.facts),
            diagnostics,
            ProofFactOwner::MachineContract {
                machine: machine.name.as_str(),
                kind: contract_kind_label(&contract.kind),
            },
        );
    }
}

fn validate_named_evidence_binding(
    program: &TypedTrees,
    owner: &str,
    contract: &psi_typed_trees::signature::SignatureContract,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(binding) = contract.binding.as_ref() else {
        return;
    };
    let facts = program.proof_facts.span_or_empty(contract.facts);
    let [psi_typed_trees::domain::ProofFact::Proposition(application)] = facts else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` must bind exactly one proposition application",
            contract_kind_label(&contract.kind),
        )));
        return;
    };
    let Some(normalized) = program.normalize_nominal_proposition_application(application) else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` does not resolve to one nominal proposition endpoint",
            contract_kind_label(&contract.kind),
        )));
        return;
    };
    if !matches!(
        normalized.classification,
        psi_typed_trees::proposition::PropositionEvidenceClassification::Witness { .. }
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` binds fact-only proposition `{}`; only a witness-bearing proposition has a projectable evidence term",
            contract_kind_label(&contract.kind),
            normalized.name,
        )));
    }
}

fn validate_crash_route_shapes(
    program: &TypedTrees,
    contract: &psi_typed_trees::signature::SignatureContract,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let psi_typed_trees::signature::SignatureContractKind::Crashes { cause } = &contract.kind
    else {
        return;
    };
    for fact in program.proof_facts.span_or_empty(contract.facts) {
        if !matches!(fact, psi_typed_trees::domain::ProofFact::Expression(_)) {
            diagnostics.push(Diagnostic::error(format!(
                "`crashes {cause:?}` routes must be Boolean expressions; domain memberships and proposition applications are proof facts, not runtime-refinable crash routes",
            )));
        }
    }
}

fn contract_kind_label(kind: &psi_typed_trees::signature::SignatureContractKind) -> &'static str {
    match kind {
        psi_typed_trees::signature::SignatureContractKind::Requires => "requires",
        psi_typed_trees::signature::SignatureContractKind::Ensures => "ensures",
        psi_typed_trees::signature::SignatureContractKind::Crashes { .. } => "crashes",
    }
}

fn validate_state_parameter_names(
    state: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (parameter_index, parameter) in state.parameters.iter().enumerate() {
        if state.parameters[..parameter_index]
            .iter()
            .any(|previous| previous.name == parameter.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` has duplicate parameter `{}`",
                state.name, parameter.name
            )));
        }
    }
}
