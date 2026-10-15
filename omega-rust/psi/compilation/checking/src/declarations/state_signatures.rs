//! Signature validation keeps the owning machine through nested type checks.
//! Its display name is diagnostic metadata: declarations in separate modules
//! may share it, so dependent bounds must use the retained owner directly.

use crate::declarations::symbols::TopLevelSymbols;
use crate::proof_contracts::proof_facts::{ProofFactOwner, validate_proof_facts};
use crate::value_custody::type_references::{
    TypeParameterScope, TypeReferenceOwner, validate_type_reference_handle,
};
use diagnostics::Diagnostic;
use std::fmt;
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::{SignatureContract, StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

mod value_scope;

pub(crate) fn validate_callable_state_signatures(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        value_scope::validate(program, machine, symbols, diagnostics);
        let (type_parameters, lifetime_parameters) = program
            .machine_type_reference_scope(machine, |existing, parameter| {
                existing.name == parameter.name
            });
        crate::value_custody::type_references::validate_const_binder_carriers(
            program,
            symbols,
            &format!("machine `{}`", machine.name),
            program.machine_type_parameters(machine),
            TypeParameterScope {
                operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                lexical_owner: machine.symbol,
                type_parameters: &type_parameters,
                lifetime_parameters: &lifetime_parameters,
            },
            diagnostics,
        );
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
            StateSignatureOwner::Machine(machine),
            TypeParameterScope {
                operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                lexical_owner: machine.symbol,
                type_parameters: &type_parameters,
                lifetime_parameters: &lifetime_parameters,
            },
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
        validate_machine_state_result_agreement(program, machine, diagnostics);
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
        for machine in program
            .trait_machine_signatures(trait_definition)
            .iter()
            .chain(
                program
                    .closed_requirement_applications()
                    .filter(|application| application.declaring_trait == trait_definition.symbol)
                    .map(|application| application.signature),
            )
        {
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
                TypeParameterScope {
                    operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                    lexical_owner: machine.symbol,
                    type_parameters: &type_parameters,
                    lifetime_parameters: &lifetime_parameters,
                },
            );
            validate_machine_parameter_signatures(
                program,
                symbols,
                diagnostics,
                program.state_signature_type_parameters(machine),
                &type_parameters,
                &lifetime_parameters,
                machine.name.as_str(),
            );
        }
    }
}

/// A machine composes its states into one callable graph: every authored
/// `->` must restate the machine's result, which the first (implicit entry)
/// state carries after the parser fills it with the machine head's declared
/// result. A state declaring a different result cannot be fused into that
/// graph — diagnose here instead of declining deep in terminal lowering.
fn validate_machine_state_result_agreement(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let states = program.machine_states(machine);
    let Some(result_state) = states.first() else {
        return;
    };
    let result = program.normalized_type_identity(result_state.return_type);
    for state in &states[1..] {
        if program.normalized_type_identity(state.return_type) == result {
            continue;
        }
        let declared = if state.return_type.is_valid() {
            format!(
                "result `{}`",
                program.normalized_type_identity_text(state.return_type)
            )
        } else {
            "no result".to_string()
        };
        let expected = if result_state.return_type.is_valid() {
            format!(
                "result `{}`",
                program.normalized_type_identity_text(result_state.return_type)
            )
        } else {
            "no result".to_string()
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` declares {declared}, which differs from the machine's {expected}",
            machine.name,
            state.name.as_str(),
        )));
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
    Machine(&'program Machine),
    Trait(&'program str),
    Requirement(&'program str),
}

impl fmt::Display for StateSignatureOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Machine(machine) => write!(formatter, "machine `{}`", machine.name),
            Self::Trait(trait_definition) => write!(formatter, "trait `{trait_definition}`"),
            Self::Requirement(parameter) => {
                write!(formatter, "machine-parameter requirement `{parameter}`")
            }
        }
    }
}

fn validate_machine_parameter_signatures<'program>(
    program: &'program dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    parameters: &'program [typed_trees::data::TypeParameter],
    inherited_type_parameters: &[typed_trees::data::TypeParameter],
    inherited_lifetime_parameters: &[Identifier],
    declaration: &'program str,
) {
    for parameter in parameters {
        let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
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
            TypeParameterScope {
                operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                lexical_owner: contract.symbol,
                type_parameters: &local_type_parameters,
                lifetime_parameters: &local_lifetime_parameters,
            },
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
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: StateSignatureOwner<'program>,
    type_parameter_scope: TypeParameterScope<'_>,
) {
    for signature in signatures {
        let mut lifetime_parameters = type_parameter_scope.lifetime_parameters.to_vec();
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
            if matches!(owner, StateSignatureOwner::Machine(_))
                && typed_trees::service::service_requirement_declaration(
                    program,
                    parameter.type_reference,
                )
                .is_some()
                && (parameter.is_const
                    || parameter.is_mutable
                    || !type_parameter_scope.type_parameters.is_empty()
                    || !signature.lifetime_parameters.is_empty())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} state `{}` parameter `{}` uses the routed `Binding<R>` carrier outside the first immutable, nongeneric, lifetime-free owned-parameter rung",
                    signature.name, parameter.name,
                )));
            }

            validate_type_reference_handle(
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
                TypeParameterScope {
                    lifetime_parameters: &lifetime_parameters,
                    ..type_parameter_scope
                },
            );
        }

        if signature.return_type.is_valid() {
            validate_type_reference_handle(
                program,
                signature.return_type,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateReturn {
                    owner,
                    state: signature.name,
                    generic_depth: 0,
                },
                TypeParameterScope {
                    lifetime_parameters: &lifetime_parameters,
                    ..type_parameter_scope
                },
            );
        }
    }
}

fn validate_state_signature_contracts(
    program: &dyn crate::CheckingInput,
    signature: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in signature.contracts {
        if matches!(owner, StateSignatureOwner::Machine(_))
            && contract.kind != typed_trees::signature::SignatureContractKind::Requires
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

/// `(required symbol, required name, actual name)` for each signature
/// parameter zipped with the machine's entry-state parameters — the
/// substitution table conformance and law walks build identically.
pub(crate) fn signature_entry_substitutions(
    program: &dyn crate::CheckingInput,
    requirement: &StateSignature,
    entry_state: &State,
) -> Vec<(symbols::SymbolHandle, String, String)> {
    program
        .state_signature_parameters(requirement)
        .iter()
        .zip(program.state_parameters(entry_state))
        .map(|(required, actual)| {
            (
                required.symbol,
                required.name.as_str().to_owned(),
                actual.name.as_str().to_owned(),
            )
        })
        .collect()
}

pub(crate) fn validate_machine_contracts(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    owner: &str,
    contract: &typed_trees::signature::SignatureContract,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(binding) = contract.binding.as_ref() else {
        return;
    };
    let facts = program.proof_facts.span_or_empty(contract.facts);
    let [typed_trees::domain::ProofFact::Proposition(application)] = facts else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` must bind exactly one proposition application",
            contract_kind_label(&contract.kind),
        )));
        return;
    };
    let Some(normalized) = program.normalize_nominal_proposition_application(application, None)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` does not resolve to one nominal proposition endpoint",
            contract_kind_label(&contract.kind),
        )));
        return;
    };
    if !matches!(
        normalized.classification,
        typed_trees::proposition::PropositionEvidenceClassification::Witness { .. }
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` binds fact-only proposition `{}`; only a witness-bearing proposition has a projectable evidence term",
            contract_kind_label(&contract.kind),
            normalized.name,
        )));
    }
}

fn validate_crash_route_shapes(
    program: &dyn crate::CheckingInput,
    contract: &typed_trees::signature::SignatureContract,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let typed_trees::signature::SignatureContractKind::Crashes { cause } = &contract.kind else {
        return;
    };
    for fact in program.proof_facts.span_or_empty(contract.facts) {
        if !matches!(fact, typed_trees::domain::ProofFact::Expression(_)) {
            diagnostics.push(Diagnostic::error(format!(
                "`crashes {cause:?}` routes must be Boolean expressions; domain memberships and proposition applications are proof facts, not runtime-refinable crash routes",
            )));
        }
    }
}

fn contract_kind_label(kind: &typed_trees::signature::SignatureContractKind) -> &'static str {
    match kind {
        typed_trees::signature::SignatureContractKind::Requires => "requires",
        typed_trees::signature::SignatureContractKind::Ensures => "ensures",
        typed_trees::signature::SignatureContractKind::EnsuresForResultCase { .. } => {
            "outcome-specific ensures"
        }
        typed_trees::signature::SignatureContractKind::Crashes { .. } => "crashes",
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
