//! Serializable snapshots of typed trees for inspection.
//!
//! This file owns the whole-program snapshot and its roots and tables.
//! `declaration_snapshots.rs`, `machine_snapshots.rs`,
//! `statement_and_expression_snapshots.rs` and `type_snapshots.rs` own the
//! snapshots of each vocabulary.

mod declaration_snapshots;
mod machine_snapshots;
mod statement_and_expression_snapshots;
#[cfg(test)]
mod tests;
mod type_snapshots;

pub use declaration_snapshots::{
    ConstDeclarationSnapshot, DataDefinitionSnapshot, DataMemberSnapshot, DataPayloadFieldSnapshot,
    DomainAliasConstituentSnapshot, DomainDefinitionSnapshot, DomainEstablishmentRouteSnapshot,
    DomainSemanticRolesSnapshot, DomainTypeParameterSnapshot, MathematicalBinderSnapshot,
    MathematicalBodySnapshot, MathematicalDefinitionSnapshot, MathematicalParameterSnapshot,
    MathematicalTypeSnapshot, OperatorDefinitionSnapshot, ProofFactSnapshot,
    PropositionBinderSnapshot, PropositionBodySnapshot, PropositionFormulaSnapshot,
    PropositionSnapshot, QuotientDefinitionSnapshot, QuotientEquivalenceSelectionSnapshot,
};
pub use machine_snapshots::{
    ConformanceRowSnapshot, ConformanceSnapshot, GenericConformanceBoundSnapshot, MachineSnapshot,
    MachineSupplySnapshot, NativeCallbackParameterSnapshot, OwnedDataSnapshot,
    ProgressPremiseSnapshot, RankRangeSnapshot, RankingWitnessSnapshot, SignatureContractSnapshot,
    StateParameterSnapshot, StateSignatureSnapshot, StateSnapshot, TerminationGuaranteeSnapshot,
    TerminationInterfaceSnapshot, TraitSnapshot,
};
pub use statement_and_expression_snapshots::{
    ExpressionSnapshot, MatchArmSnapshot, MatchPatternSnapshot, StatementSnapshot,
    StaticArgumentSnapshot, StructLiteralFieldSnapshot, TransitionGuardSnapshot,
    TransitionTargetSnapshot,
};
pub use type_snapshots::{
    DomainConstraintSubjectSnapshot, OpenIndexNormalizationSnapshot, OpenIndexOperationSnapshot,
    TypeConstraintSnapshot, TypeReferenceSnapshot, WireMemberSnapshot, WireSchemaSnapshot,
};

use crate::TypedTrees;
use crate::typed_trees::inspection::snapshot::declaration_snapshots::data_definition_snapshot;
use crate::typed_trees::inspection::snapshot::declaration_snapshots::domain_definition_snapshot;
use crate::typed_trees::inspection::snapshot::declaration_snapshots::mathematical_definition_snapshot;
use crate::typed_trees::inspection::snapshot::declaration_snapshots::operator_snapshot;
use crate::typed_trees::inspection::snapshot::declaration_snapshots::proposition_snapshot;
use crate::typed_trees::inspection::snapshot::machine_snapshots::conformance_snapshot;
use crate::typed_trees::inspection::snapshot::machine_snapshots::machine_snapshot;
use crate::typed_trees::inspection::snapshot::machine_snapshots::trait_definition_snapshot;
use crate::typed_trees::inspection::snapshot::statement_and_expression_snapshots::expression_snapshot;
use crate::typed_trees::inspection::snapshot::type_snapshots::type_reference_snapshot;
use crate::typed_trees::inspection::snapshot::type_snapshots::wire_schema_snapshot;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedTreesSnapshot {
    pub roots: TypedRootsSnapshot,
    pub tables: TypedTableSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub external_bindings: Vec<ExternalBindingSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence_forwardings: Vec<EvidenceForwardingSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub proof_output_calls: Vec<ProofOutputCallSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceForwardingSnapshot {
    pub machine_symbol: u32,
    pub state_symbol: u32,
    pub statement_index: usize,
    pub source_statement_index: usize,
    pub target: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_conformance: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProofOutputCallSnapshot {
    pub machine_symbol: u32,
    pub state_symbol: u32,
    pub statement_index: usize,
    pub source_statement_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_call_statement_index: Option<usize>,
    pub bindings: Vec<(String, String)>,
    pub call: ExpressionSnapshot,
}

impl TypedTreesSnapshot {
    pub fn from_typed_trees(program: &TypedTrees) -> Self {
        Self {
            roots: TypedRootsSnapshot {
                const_declarations: program
                    .const_declarations()
                    .iter()
                    .map(|declaration| ConstDeclarationSnapshot {
                        has_symbol: declaration.symbol.is_valid(),
                        name: program.symbols.display_path(declaration.symbol, "::"),
                        is_public: declaration.is_public,
                        declared_type: type_reference_snapshot(program, declaration.declared_type),
                        canonical_value_encoding: declaration.canonical_value_encoding.clone(),
                    })
                    .collect(),
                conformances: program
                    .conformances()
                    .iter()
                    .map(|conformance| conformance_snapshot(program, conformance))
                    .collect(),
                data_definitions: program
                    .data_definitions()
                    .iter()
                    .map(|data| data_definition_snapshot(program, data))
                    .collect(),
                domain_definitions: program
                    .domain_definitions()
                    .iter()
                    .map(|domain| domain_definition_snapshot(program, domain))
                    .collect(),
                machines: program
                    .machines()
                    .iter()
                    .map(|machine| machine_snapshot(program, machine))
                    .collect(),
                mathematical_definitions: program
                    .mathematical_definitions()
                    .iter()
                    .map(|definition| mathematical_definition_snapshot(program, definition))
                    .collect(),
                operators: program
                    .operators()
                    .iter()
                    .map(|operator| operator_snapshot(program, operator))
                    .collect(),
                machine_token_bindings: program
                    .machine_token_bindings()
                    .iter()
                    .map(|operator| operator_snapshot(program, operator))
                    .collect(),
                propositions: program
                    .propositions()
                    .iter()
                    .map(|proposition| proposition_snapshot(program, proposition))
                    .collect(),
                traits: program
                    .traits()
                    .iter()
                    .map(|trait_definition| trait_definition_snapshot(program, trait_definition))
                    .collect(),
                wire_schemas: program
                    .wire_schemas()
                    .iter()
                    .map(|wire_schema| wire_schema_snapshot(program, wire_schema))
                    .collect(),
            },
            tables: TypedTableSnapshot {
                data_definition_count: program.data_definitions.len(),
                data_type_parameter_count: program.data_type_parameters.len(),
                data_member_count: program.data_members.len(),
                domain_definition_count: program.domain_definitions.len(),
                machine_count: program.machines.len(),
                operator_count: program.operators.len(),
                proposition_count: program.propositions.len(),
                proposition_binder_count: program.proposition_binders.len(),
                machine_owned_data_count: program.machine_owned_data.len(),
                machine_state_count: program.machine_states.len(),
                state_parameter_count: program.state_parameters.len(),
                trait_count: program.traits.len(),
                conformance_count: program.conformances.len(),
                trait_requirement_count: program.trait_requirements.len(),
                trait_machine_signature_count: program.trait_machine_signatures.len(),
                expression_count: program.expression_table.expression_count(),
                expression_struct_field_count: program.expression_table.struct_field_count(),
                statement_count: program.statement_table.statement_count(),
                transition_target_count: program.statement_table.transition_target_count(),
                type_reference_count: program.type_reference_table.type_reference_count(),
                type_constraint_count: program.type_reference_table.constraint_count(),
            },
            external_bindings: program
                .external_bindings
                .identities()
                .map(|(identity, binding)| ExternalBindingSnapshot {
                    identity: identity.0,
                    binding: match binding {
                        language_semantics::ExternalBindingIdentity::Syscall { number } => {
                            ExternalBindingValueSnapshot::Syscall { number: *number }
                        }
                        language_semantics::ExternalBindingIdentity::CompilerIntrinsic => {
                            ExternalBindingValueSnapshot::CompilerIntrinsic
                        }
                        language_semantics::ExternalBindingIdentity::VtableSlot { index } => {
                            ExternalBindingValueSnapshot::VtableSlot { index: *index }
                        }
                        language_semantics::ExternalBindingIdentity::VtableField { field } => {
                            ExternalBindingValueSnapshot::VtableField {
                                field: field.clone(),
                            }
                        }
                        language_semantics::ExternalBindingIdentity::TableFunction { field } => {
                            ExternalBindingValueSnapshot::TableFunction {
                                field: field.clone(),
                            }
                        }
                    },
                })
                .collect(),
            evidence_forwardings: program
                .evidence_forwardings
                .iter()
                .map(|forwarding| EvidenceForwardingSnapshot {
                    machine_symbol: forwarding.machine_symbol.arena_index(),
                    state_symbol: forwarding.state_symbol.arena_index(),
                    statement_index: forwarding.statement_index,
                    source_statement_index: forwarding.source_statement_index,
                    target: forwarding.target.to_string(),
                    source: forwarding.source.to_string(),
                    source_conformance: forwarding
                        .source_conformance
                        .map(|symbol| symbol.arena_index()),
                })
                .collect(),
            proof_output_calls: program
                .proof_output_calls
                .iter()
                .map(|package| ProofOutputCallSnapshot {
                    machine_symbol: package.machine_symbol.arena_index(),
                    state_symbol: package.state_symbol.arena_index(),
                    statement_index: package.statement_index,
                    source_statement_index: package.source_statement_index,
                    runtime_call_statement_index: package.runtime_call_statement_index,
                    bindings: package
                        .bindings
                        .iter()
                        .map(|binding| {
                            (
                                binding.output_field.to_string(),
                                binding.binding.to_string(),
                            )
                        })
                        .collect(),
                    call: expression_snapshot(program, package.call),
                })
                .collect(),
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternalBindingSnapshot {
    pub identity: u32,
    #[serde(flatten)]
    pub binding: ExternalBindingValueSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExternalBindingValueSnapshot {
    Syscall { number: i64 },
    CompilerIntrinsic,
    VtableSlot { index: i64 },
    VtableField { field: String },
    TableFunction { field: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedRootsSnapshot {
    pub const_declarations: Vec<ConstDeclarationSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformances: Vec<ConformanceSnapshot>,
    pub data_definitions: Vec<DataDefinitionSnapshot>,
    pub domain_definitions: Vec<DomainDefinitionSnapshot>,
    pub machines: Vec<MachineSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mathematical_definitions: Vec<MathematicalDefinitionSnapshot>,
    pub operators: Vec<OperatorDefinitionSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub machine_token_bindings: Vec<OperatorDefinitionSnapshot>,
    pub propositions: Vec<PropositionSnapshot>,
    pub traits: Vec<TraitSnapshot>,
    pub wire_schemas: Vec<WireSchemaSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedTableSnapshot {
    pub data_definition_count: usize,
    pub data_type_parameter_count: usize,
    pub data_member_count: usize,
    pub domain_definition_count: usize,
    pub machine_count: usize,
    pub operator_count: usize,
    pub proposition_count: usize,
    pub proposition_binder_count: usize,
    pub machine_owned_data_count: usize,
    pub machine_state_count: usize,
    pub state_parameter_count: usize,
    pub trait_count: usize,
    pub conformance_count: usize,
    pub trait_requirement_count: usize,
    pub trait_machine_signature_count: usize,
    pub expression_count: usize,
    pub expression_struct_field_count: usize,
    pub statement_count: usize,
    pub transition_target_count: usize,
    pub type_reference_count: usize,
    pub type_constraint_count: usize,
}
