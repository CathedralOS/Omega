//! Serializable snapshots of symbol-resolved trees for inspection.
//!
//! This file owns the whole-program snapshot and its roots and tables.
//! `declaration_snapshots.rs`, `machine_snapshots.rs`,
//! `statement_and_expression_snapshots.rs` and `type_snapshots.rs` own the
//! snapshots of each vocabulary.

mod declaration_snapshots;
mod machine_snapshots;
mod statement_and_expression_snapshots;
#[cfg(test)]
mod termination_vocabulary_tests;
#[cfg(test)]
mod tests;
mod type_snapshots;

pub use declaration_snapshots::{
    ConstDeclarationSnapshot, DataDefinitionSnapshot, DataMemberSnapshot, DataPayloadFieldSnapshot,
    DomainAliasConstituentSnapshot, DomainDefinitionSnapshot, DomainEstablishmentRouteSnapshot,
    DomainSemanticRolesSnapshot, MathematicalBinderSnapshot, MathematicalBodySnapshot,
    MathematicalDefinitionSnapshot, MathematicalParameterSnapshot, MathematicalTypeSnapshot,
    MeasureDefinitionSnapshot, OperatorDefinitionSnapshot, ProofFactSnapshot,
    PropositionBinderSnapshot, PropositionBodySnapshot, PropositionSnapshot,
    QuotientDefinitionSnapshot, QuotientEquivalenceSelectionSnapshot,
};
pub use machine_snapshots::{
    ConformanceRowSnapshot, ConformanceSnapshot, GenericConformanceBoundSnapshot, MachineSnapshot,
    MachineSupplySnapshot, NativeCallbackParameterSnapshot, OwnedDataSnapshot,
    ProgressPremiseSnapshot, SignatureContractSnapshot, StateParameterSnapshot,
    StateSignatureSnapshot, StateSnapshot, TerminationGuaranteeSnapshot,
    TerminationInterfaceSnapshot, TraitSnapshot,
};
pub use statement_and_expression_snapshots::{
    ExpressionSnapshot, MatchArmSnapshot, MatchPatternSnapshot, StatementSnapshot,
    StaticArgumentSnapshot, StructLiteralFieldSnapshot, TransitionGuardSnapshot,
    TransitionTargetSnapshot,
};
pub use type_snapshots::{
    TypeConstraintSnapshot, TypeReferenceSnapshot, WireMemberSnapshot, WireSchemaSnapshot,
};

use crate::SymbolResolvedTrees;
use crate::symbol_resolved_trees::inspection::snapshot::declaration_snapshots::data_definition_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::declaration_snapshots::domain_definition_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::declaration_snapshots::mathematical_definition_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::declaration_snapshots::measure_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::declaration_snapshots::operator_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::declaration_snapshots::proposition_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::machine_snapshots::conformance_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::machine_snapshots::machine_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::machine_snapshots::trait_definition_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::type_snapshots::type_reference_snapshot;
use crate::symbol_resolved_trees::inspection::snapshot::type_snapshots::wire_schema_snapshot;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolResolvedTreesSnapshot {
    pub roots: SymbolResolvedRootsSnapshot,
    pub tables: ResolvedTableSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence_forwardings: Vec<EvidenceForwardingSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceForwardingSnapshot {
    pub machine_symbol: u32,
    pub state_symbol: u32,
    pub statement_index: usize,
    pub target: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_conformance: Option<u32>,
}

impl SymbolResolvedTreesSnapshot {
    pub fn from_symbol_resolved_trees(symbol_resolved_trees: &SymbolResolvedTrees) -> Self {
        Self {
            roots: SymbolResolvedRootsSnapshot {
                const_declarations: symbol_resolved_trees
                    .const_declarations
                    .iter()
                    .map(|declaration| ConstDeclarationSnapshot {
                        has_symbol: declaration.symbol.is_valid(),
                        name: symbol_resolved_trees
                            .symbols
                            .display_path(declaration.symbol, "::"),
                        is_public: declaration.is_public,
                        declared_type: type_reference_snapshot(
                            symbol_resolved_trees,
                            &declaration.declared_type,
                        ),
                        canonical_value_encoding: declaration.canonical_value_encoding.clone(),
                    })
                    .collect(),
                conformances: symbol_resolved_trees
                    .conformances
                    .iter()
                    .map(|conformance| conformance_snapshot(symbol_resolved_trees, conformance))
                    .collect(),
                data_definitions: symbol_resolved_trees
                    .data_definitions
                    .iter()
                    .map(|data| data_definition_snapshot(symbol_resolved_trees, data))
                    .collect(),
                domain_definitions: symbol_resolved_trees
                    .domain_definitions
                    .iter()
                    .map(|domain| domain_definition_snapshot(symbol_resolved_trees, domain))
                    .collect(),
                machines: symbol_resolved_trees
                    .machines
                    .iter()
                    .map(|machine| machine_snapshot(symbol_resolved_trees, machine))
                    .collect(),
                mathematical_definitions: symbol_resolved_trees
                    .mathematical_definitions
                    .iter()
                    .map(|definition| {
                        mathematical_definition_snapshot(symbol_resolved_trees, definition)
                    })
                    .collect(),
                measures: symbol_resolved_trees
                    .measures
                    .iter()
                    .map(|measure| measure_snapshot(symbol_resolved_trees, measure))
                    .collect(),
                operators: symbol_resolved_trees
                    .operators
                    .iter()
                    .map(|operator| operator_snapshot(symbol_resolved_trees, operator))
                    .collect(),
                propositions: symbol_resolved_trees
                    .propositions
                    .iter()
                    .map(|proposition| proposition_snapshot(symbol_resolved_trees, proposition))
                    .collect(),
                traits: symbol_resolved_trees
                    .traits
                    .iter()
                    .map(|trait_definition| {
                        trait_definition_snapshot(symbol_resolved_trees, trait_definition)
                    })
                    .collect(),
                wire_schemas: symbol_resolved_trees
                    .wire_schemas
                    .iter()
                    .map(|wire_schema| wire_schema_snapshot(symbol_resolved_trees, wire_schema))
                    .collect(),
            },
            tables: ResolvedTableSnapshot {
                type_constraint_count: symbol_resolved_trees.tables.types.constraints.len(),
                expression_count: symbol_resolved_trees
                    .tables
                    .bodies
                    .expressions
                    .expression_count(),
                statement_count: symbol_resolved_trees
                    .tables
                    .bodies
                    .statements
                    .statement_count(),
                type_reference_count: symbol_resolved_trees
                    .tables
                    .types
                    .references
                    .type_reference_count(),
            },
            evidence_forwardings: symbol_resolved_trees
                .evidence_forwardings
                .iter()
                .map(|forwarding| EvidenceForwardingSnapshot {
                    machine_symbol: forwarding.machine_symbol.arena_index(),
                    state_symbol: forwarding.state_symbol.arena_index(),
                    statement_index: forwarding.statement_index,
                    target: forwarding.target.to_string(),
                    source: forwarding.source.to_string(),
                    source_conformance: forwarding
                        .source_conformance
                        .map(|symbol| symbol.arena_index()),
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
pub struct SymbolResolvedRootsSnapshot {
    pub const_declarations: Vec<ConstDeclarationSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformances: Vec<ConformanceSnapshot>,
    pub data_definitions: Vec<DataDefinitionSnapshot>,
    pub domain_definitions: Vec<DomainDefinitionSnapshot>,
    pub machines: Vec<MachineSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mathematical_definitions: Vec<MathematicalDefinitionSnapshot>,
    pub measures: Vec<MeasureDefinitionSnapshot>,
    pub operators: Vec<OperatorDefinitionSnapshot>,
    pub propositions: Vec<PropositionSnapshot>,
    pub traits: Vec<TraitSnapshot>,
    pub wire_schemas: Vec<WireSchemaSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedTableSnapshot {
    pub type_constraint_count: usize,
    pub expression_count: usize,
    pub statement_count: usize,
    pub type_reference_count: usize,
}
