use crate::proof_facts::{ProofFactOwner, validate_proof_facts};
use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::signature::{SignatureContract, StateParameter};
use omega_typed_trees::types::TypeReferenceHandle;
use std::fmt;

pub(crate) fn validate_callable_state_signatures(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let mut type_parameters = program.machine_type_parameters(machine).to_vec();
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
        }
        validate_state_signature_types(
            program
                .machine_states(machine)
                .iter()
                .map(|state| StateSignatureView {
                    name: state.name.as_str(),
                    parameters: program.state_parameters(state),
                    return_type: state.return_type,
                    effects: &[],
                    contracts: program.state_contracts(state),
                }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Machine(machine.name.as_str()),
            &type_parameters,
        );
    }

    for trait_definition in program.traits() {
        validate_proof_facts(
            program,
            program.trait_invariants(trait_definition),
            diagnostics,
            ProofFactOwner::TraitInvariant {
                trait_definition: trait_definition.name.as_str(),
            },
        );
        validate_state_signature_types(
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .map(|machine| StateSignatureView {
                    name: machine.name.as_str(),
                    parameters: program.state_signature_parameters(machine),
                    return_type: machine.return_type,
                    effects: program.state_signature_effects(machine),
                    contracts: program.state_signature_contracts(machine),
                }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Trait(trait_definition.name.as_str()),
            program.trait_type_parameters(trait_definition),
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct StateSignatureView<'program> {
    name: &'program str,
    parameters: &'program [StateParameter],
    return_type: TypeReferenceHandle,
    effects: &'program [Identifier],
    contracts: &'program [SignatureContract],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StateSignatureOwner<'program> {
    Machine(&'program str),
    Trait(&'program str),
}

impl fmt::Display for StateSignatureOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Machine(machine) => write!(formatter, "machine `{machine}`"),
            Self::Trait(trait_definition) => write!(formatter, "trait `{trait_definition}`"),
        }
    }
}

fn validate_state_signature_types<'program>(
    signatures: impl Iterator<Item = StateSignatureView<'program>>,
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: StateSignatureOwner<'program>,
    type_parameters: &[omega_typed_trees::data::TypeParameter],
) {
    for signature in signatures {
        validate_state_parameter_names(signature, owner, diagnostics);
        validate_state_signature_effects(signature, owner, diagnostics);
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
            );
        }
    }
}

fn validate_state_signature_effects(
    signature: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for effect in signature.effects {
        if !omega_effects::is_standard_effect_name(effect.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` declares unknown effect `{}`",
                signature.name, effect
            )));
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
            && contract.kind != omega_typed_trees::signature::SignatureContractKind::Requires
        {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` admits only arrival `requires` contracts",
                signature.name
            )));
            continue;
        }
        validate_proof_facts(
            program,
            program.proof_facts.span_or_empty(contract.facts),
            diagnostics,
            ProofFactOwner::StateSignatureContract {
                owner,
                state: signature.name,
                kind: contract_kind_label(contract.kind),
            },
        );
    }
}

pub(crate) fn validate_machine_effects(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for effect in program.machine_effects(machine) {
        if !omega_effects::is_standard_effect_name(effect.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` declares unknown effect `{}`",
                machine.name, effect
            )));
        }
    }
}

pub(crate) fn validate_machine_contracts(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in program.machine_contracts(machine) {
        validate_proof_facts(
            program,
            program.proof_facts.span_or_empty(contract.facts),
            diagnostics,
            ProofFactOwner::MachineContract {
                machine: machine.name.as_str(),
                kind: contract_kind_label(contract.kind),
            },
        );
    }
}

fn contract_kind_label(kind: omega_typed_trees::signature::SignatureContractKind) -> &'static str {
    match kind {
        omega_typed_trees::signature::SignatureContractKind::Requires => "requires",
        omega_typed_trees::signature::SignatureContractKind::Ensures => "ensures",
        omega_typed_trees::signature::SignatureContractKind::Boundary => "boundary",
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
