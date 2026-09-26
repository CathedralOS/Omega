//! Snapshots of machines, traits, conformances, states, signatures,
//! contracts and parameters.

use crate::SymbolResolvedTrees;
use crate::machine::{Machine, OwnedData};
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::symbol_resolved_trees::inspection::snapshot::declaration_snapshots::domain_fact_snapshots;
use crate::symbol_resolved_trees::inspection::snapshot::statement_and_expression_snapshots::{
    snapshot_static_argument, statement_snapshot, table_expression_snapshot,
};
use crate::symbol_resolved_trees::inspection::snapshot::type_snapshots::type_reference_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::{
    ExpressionSnapshot, ProofFactSnapshot, StatementSnapshot, StaticArgumentSnapshot,
    TypeReferenceSnapshot,
};
use crate::trait_definition::TraitDefinition;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConformanceSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_symbol: Option<u32>,
    pub trait_name: String,
    pub trait_symbol: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub trait_lifetime_arguments: Vec<String>,
    pub arguments: Vec<TypeReferenceSnapshot>,
    pub implementation: &'static str,
    pub rows: Vec<ConformanceRowSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConformanceRowSnapshot {
    pub declaring_trait: u32,
    pub requirement: u32,
    pub realization_machine: u32,
    pub realization_state: u32,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MachineSnapshot {
    pub name: String,
    pub attached_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spelling: Option<&'static str>,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
    pub supply: MachineSupplySnapshot,
    pub body_is_present: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub structural_type_equations_pending: bool,
    pub termination: TerminationInterfaceSnapshot,
    pub ranking_subjects: Vec<ExpressionSnapshot>,
    pub ranking_view: Vec<String>,
    pub invokes: Vec<String>,
    pub service_reach: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub service_reach_is_installation_bound: bool,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: Vec<SignatureContractSnapshot>,
    pub owned_data: Vec<OwnedDataSnapshot>,
    pub states: Vec<StateSnapshot>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GenericConformanceBoundSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binder_symbol: Option<u32>,
    pub subject: String,
    pub subject_symbol: u32,
    pub carrier: String,
    pub carrier_symbol: u32,
    pub arguments: Vec<TypeReferenceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_conformance: Option<StaticArgumentSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_conformance_symbol: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MachineSupplySnapshot {
    CheckedBody,
    Requirement,
    TopLevelRequirement,
    Boundary,
    AdmissionClaim,
    ExternalRealization {
        #[serde(skip_serializing_if = "Option::is_none")]
        binding: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mechanism: Option<&'static str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "interface", rename_all = "snake_case")]
pub enum TerminationInterfaceSnapshot {
    InternalDerived,
    Published {
        guarantee: TerminationGuaranteeSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminationGuaranteeSnapshot {
    NoGuarantee,
    Terminates {
        premises: Vec<ProgressPremiseSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgressPremiseSnapshot {
    pub profile: u32,
    pub subject_root: u32,
    pub subject_projections: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnedDataSnapshot {
    pub name: String,
    pub type_reference: TypeReferenceSnapshot,
    pub initial_value: Option<ExpressionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraitSnapshot {
    pub name: String,
    pub is_boundary: bool,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
    pub requires: Vec<String>,
    pub machines: Vec<StateSignatureSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSnapshot {
    pub name: String,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
    pub contracts: Vec<SignatureContractSnapshot>,
    pub statements: Vec<StatementSnapshot>,
    pub table_statement_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSignatureSnapshot {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spelling: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    pub is_default: bool,
    pub parameters: Vec<StateParameterSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub native_callback_parameters: Vec<NativeCallbackParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
    pub invokes: Vec<String>,
    pub service_reach: Vec<String>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: Vec<SignatureContractSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativeCallbackParameterSnapshot {
    pub name: String,
    pub binder: String,
    pub native_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignatureContractSnapshot {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash_cause: Option<&'static str>,
    pub facts: Vec<ProofFactSnapshot>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateParameterSnapshot {
    pub name: String,
    pub type_reference: TypeReferenceSnapshot,
    pub is_mutable: bool,
    pub is_self: bool,
    pub relevance: &'static str,
}

pub(crate) fn machine_snapshot(
    program: &SymbolResolvedTrees,
    machine: &Machine,
) -> MachineSnapshot {
    MachineSnapshot {
        name: machine.name.to_string(),
        attached_data: machine.attached_data.as_ref().map(ToString::to_string),
        spelling: machine.spelling.map(|spelling| spelling.symbol()),
        is_public: machine.is_public,
        lifetime_parameters: machine
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .machine_type_parameters(machine)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        conformance_bounds: machine
            .conformance_bounds
            .iter()
            .map(|bound| GenericConformanceBoundSnapshot {
                binder: bound.binder_name.as_ref().map(ToString::to_string),
                binder_symbol: bound.binder.map(|symbol| symbol.arena_index()),
                subject: bound.subject_name.to_string(),
                subject_symbol: bound.subject.arena_index(),
                carrier: bound.carrier_name.to_string(),
                carrier_symbol: bound.carrier.arena_index(),
                arguments: program
                    .child_type_references(bound.arguments)
                    .iter()
                    .map(|argument| type_reference_snapshot(program, argument))
                    .collect(),
                selected_conformance: bound
                    .selected_conformance
                    .as_ref()
                    .map(|argument| snapshot_static_argument(program, argument)),
                selected_conformance_symbol: bound
                    .selected_conformance
                    .as_ref()
                    .map(|selected| selected.symbol.arena_index()),
            })
            .collect(),
        supply: machine_supply_snapshot(machine.supply_mode),
        body_is_present: machine.body_is_present,
        structural_type_equations_pending: machine.structural_type_equations_pending,
        termination: termination_interface_snapshot(&machine.termination_plan.interface),
        ranking_subjects: program
            .tables
            .bodies
            .expressions
            .expression_handles(machine.ranking_subjects)
            .iter()
            .map(|handle| table_expression_snapshot(program, *handle))
            .collect(),
        ranking_view: program
            .machine_ranking_view(machine.ranking_view)
            .iter()
            .map(ToString::to_string)
            .collect(),
        invokes: program
            .machine_invokes(machine)
            .iter()
            .map(ToString::to_string)
            .collect(),
        service_reach: service_reach_names(program, machine.service_reach_row),
        service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
        suspends: machine.suspends,
        blocks: machine.blocks,
        contracts: program
            .machine_contracts(machine)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
        owned_data: program
            .machine_owned_data(machine.owned_data)
            .iter()
            .map(|owned| owned_data_snapshot(program, owned))
            .collect(),
        states: program
            .machine_state_handles(machine.states)
            .iter()
            .map(|state| program.machine_state(*state))
            .map(|state| state_snapshot(program, state))
            .collect(),
    }
}

fn machine_supply_snapshot(supply: language_semantics::MachineSupplyMode) -> MachineSupplySnapshot {
    use language_semantics::MachineSupplyMode;
    match supply {
        MachineSupplyMode::CheckedBody => MachineSupplySnapshot::CheckedBody,
        MachineSupplyMode::Requirement => MachineSupplySnapshot::Requirement,
        MachineSupplyMode::TopLevelRequirement => MachineSupplySnapshot::TopLevelRequirement,
        MachineSupplyMode::Boundary => MachineSupplySnapshot::Boundary,
        MachineSupplyMode::AdmissionClaim => MachineSupplySnapshot::AdmissionClaim,
        MachineSupplyMode::ExternalRealization { binding, mechanism } => {
            MachineSupplySnapshot::ExternalRealization {
                binding: binding.map(|binding| binding.0),
                mechanism: mechanism.map(|mechanism| mechanism.as_str()),
            }
        }
    }
}

fn termination_interface_snapshot(
    interface: &language_semantics::TerminationInterface,
) -> TerminationInterfaceSnapshot {
    use language_semantics::{TerminationGuarantee, TerminationInterface};
    match interface {
        TerminationInterface::InternalDerived => TerminationInterfaceSnapshot::InternalDerived,
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => {
            TerminationInterfaceSnapshot::Published {
                guarantee: TerminationGuaranteeSnapshot::NoGuarantee,
            }
        }
        TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
            TerminationInterfaceSnapshot::Published {
                guarantee: TerminationGuaranteeSnapshot::Terminates {
                    premises: premises
                        .iter()
                        .map(|premise| ProgressPremiseSnapshot {
                            profile: premise.profile.0,
                            subject_root: premise.subject.root.arena_index(),
                            subject_projections: premise
                                .subject
                                .projections
                                .iter()
                                .map(|symbol| symbol.arena_index())
                                .collect(),
                        })
                        .collect(),
                },
            }
        }
    }
}

fn owned_data_snapshot(program: &SymbolResolvedTrees, owned: &OwnedData) -> OwnedDataSnapshot {
    OwnedDataSnapshot {
        name: owned.name.to_string(),
        type_reference: type_reference_snapshot(program, &owned.type_reference),
        initial_value: owned
            .initial_value
            .is_valid()
            .then(|| table_expression_snapshot(program, owned.initial_value)),
    }
}

pub(crate) fn trait_definition_snapshot(
    program: &SymbolResolvedTrees,
    trait_definition: &TraitDefinition,
) -> TraitSnapshot {
    TraitSnapshot {
        name: trait_definition.name.to_string(),
        is_boundary: trait_definition.is_boundary,
        is_public: trait_definition.is_public,
        lifetime_parameters: trait_definition
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .trait_type_parameters(trait_definition)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        conformance_bounds: trait_definition
            .conformance_bounds
            .iter()
            .map(|bound| GenericConformanceBoundSnapshot {
                binder: bound.binder_name.as_ref().map(ToString::to_string),
                binder_symbol: bound.binder.map(|symbol| symbol.arena_index()),
                subject: bound.subject_name.to_string(),
                subject_symbol: bound.subject.arena_index(),
                carrier: bound.carrier_name.to_string(),
                carrier_symbol: bound.carrier.arena_index(),
                arguments: program
                    .child_type_references(bound.arguments)
                    .iter()
                    .map(|argument| type_reference_snapshot(program, argument))
                    .collect(),
                selected_conformance: bound
                    .selected_conformance
                    .as_ref()
                    .map(|argument| snapshot_static_argument(program, argument)),
                selected_conformance_symbol: bound
                    .selected_conformance
                    .as_ref()
                    .map(|selected| selected.symbol.arena_index()),
            })
            .collect(),
        requires: program
            .trait_requirements(trait_definition.requires)
            .iter()
            .map(|requirement| {
                let arguments = program.child_type_references(requirement.arguments);
                if requirement.lifetime_arguments.is_empty() && arguments.is_empty() {
                    requirement.name.to_string()
                } else {
                    let lifetime_arguments = requirement
                        .lifetime_arguments
                        .iter()
                        .map(|lifetime| format!("'{lifetime}"));
                    let type_arguments = arguments.iter().map(|argument| argument.display_name());
                    format!(
                        "{}<{}>",
                        requirement.name,
                        lifetime_arguments
                            .chain(type_arguments)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            })
            .collect(),
        machines: program
            .trait_machine_signatures(trait_definition.machines)
            .iter()
            .map(|signature| state_signature_snapshot(program, signature))
            .collect(),
    }
}

pub(crate) fn conformance_snapshot(
    program: &SymbolResolvedTrees,
    conformance: &crate::trait_definition::Conformance,
) -> ConformanceSnapshot {
    let (implementation, rows) = match &conformance.implementation {
        crate::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
            ("attached_requirement_machines", Vec::new())
        }
        crate::trait_definition::ConformanceImplementation::Closed { rows } => (
            "closed",
            rows.iter()
                .map(|row| ConformanceRowSnapshot {
                    declaring_trait: row.declaring_trait.arena_index(),
                    requirement: row.requirement.arena_index(),
                    realization_machine: row.realization_machine.arena_index(),
                    realization_state: row.realization_state.arena_index(),
                    source: match row.source {
                        crate::trait_definition::ConformanceRowSource::Inline => "inline",
                        crate::trait_definition::ConformanceRowSource::Reference => "reference",
                        crate::trait_definition::ConformanceRowSource::TraitDefault => {
                            "trait_default"
                        }
                    },
                })
                .collect(),
        ),
    };
    ConformanceSnapshot {
        has_symbol: conformance.symbol.is_valid(),
        name: conformance.alias.as_ref().map_or_else(
            || program.symbols.display_path(conformance.symbol, "::"),
            ToString::to_string,
        ),
        is_public: conformance.is_public,
        lifetime_parameters: conformance
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .data_type_parameters(conformance.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        subject: conformance.carrier_name().map(ToString::to_string),
        subject_symbol: conformance
            .carrier_symbol
            .is_valid()
            .then(|| conformance.carrier_symbol.arena_index()),
        trait_name: conformance.trait_name.to_string(),
        trait_symbol: conformance.trait_symbol.arena_index(),
        trait_lifetime_arguments: conformance
            .trait_lifetime_arguments
            .iter()
            .map(ToString::to_string)
            .collect(),
        arguments: program
            .child_type_references(conformance.arguments)
            .iter()
            .map(|argument| type_reference_snapshot(program, argument))
            .collect(),
        implementation,
        rows,
    }
}

fn state_snapshot(program: &SymbolResolvedTrees, state: &State) -> StateSnapshot {
    StateSnapshot {
        name: state.name.to_string(),
        parameters: program
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: state
            .return_type
            .as_ref()
            .map(|type_reference| type_reference_snapshot(program, type_reference)),
        contracts: program
            .signature_contracts(state.contracts)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
        statements: program
            .state_statements(state.statements)
            .iter()
            .map(|statement| statement_snapshot(program, statement))
            .collect(),
        table_statement_count: state.statement_nodes.count() as usize,
    }
}

fn state_signature_snapshot(
    program: &SymbolResolvedTrees,
    signature: &StateSignature,
) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: signature.name.to_string(),
        spelling: signature.spelling.map(|spelling| spelling.symbol()),
        lifetime_parameters: signature
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .data_type_parameters(signature.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        is_default: signature.is_default,
        parameters: program
            .state_parameters(signature.parameters)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        native_callback_parameters: signature
            .native_callback_parameters
            .iter()
            .map(|parameter| NativeCallbackParameterSnapshot {
                name: parameter.name.to_string(),
                binder: parameter.binder.to_string(),
                native_ordinal: parameter.native_ordinal,
            })
            .collect(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| type_reference_snapshot(program, type_reference)),
        invokes: program
            .signature_invokes(signature.invokes)
            .iter()
            .map(ToString::to_string)
            .collect(),
        service_reach: service_reach_names(program, signature.service_reach_row),
        suspends: signature.suspends,
        blocks: signature.blocks,
        contracts: program
            .signature_contracts(signature.contracts)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
    }
}

fn service_reach_names(
    program: &SymbolResolvedTrees,
    row: language_semantics::ServiceReachRowId,
) -> Vec<String> {
    program
        .service_reach_rows
        .services(row)
        .iter()
        .filter_map(|service| program.service_reaches.definition(*service))
        .map(|definition| definition.name.clone())
        .collect()
}

fn signature_contract_snapshot(
    program: &SymbolResolvedTrees,
    contract: &crate::signature::SignatureContract,
) -> SignatureContractSnapshot {
    let (kind, crash_cause) = match &contract.kind {
        crate::signature::SignatureContractKind::Requires => ("requires", None),
        crate::signature::SignatureContractKind::Ensures => ("ensures", None),
        crate::signature::SignatureContractKind::EnsuresForResultCase { .. } => {
            ("ensures_for_result_case", None)
        }
        crate::signature::SignatureContractKind::Crashes { cause } => {
            return SignatureContractSnapshot {
                kind: "crashes",
                binding: contract.binding.as_ref().map(ToString::to_string),
                crash_cause: Some(match cause {
                    crate::signature::CrashCause::Trap => "Trap",
                    crate::signature::CrashCause::Abort => "Abort",
                }),
                facts: domain_fact_snapshots(program, contract.facts),
                token_count: contract.token_count,
            };
        }
    };
    SignatureContractSnapshot {
        kind,
        binding: contract.binding.as_ref().map(ToString::to_string),
        crash_cause,
        facts: domain_fact_snapshots(program, contract.facts),
        token_count: contract.token_count,
    }
}

pub(crate) fn state_parameter_snapshot(
    program: &SymbolResolvedTrees,
    parameter: &StateParameter,
) -> StateParameterSnapshot {
    StateParameterSnapshot {
        name: parameter.name.to_string(),
        type_reference: type_reference_snapshot(program, &parameter.type_reference),
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
        relevance: super::declaration_snapshots::snapshot_binding_relevance(parameter.relevance),
    }
}
