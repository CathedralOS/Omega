use crate::{data, domain, expression, signature, snapshot, state, statement, tables, types};
use omega_core::arena::{Arena, Handle, HandleSpan, OrderedRootArena};
use omega_core::diagnostics::PhaseSnapshot;
use omega_core::symbols::SymbolTable;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTrees {
    pub roots: SymbolResolvedRoots,
    pub tables: SymbolResolvedTableStorage,
    pub symbols: SymbolTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedRoots {
    pub data_definitions: OrderedRootArena<crate::data::DataDefinition>,
    pub domain_definitions: OrderedRootArena<crate::domain::DomainDefinition>,
    pub invariant_definitions: OrderedRootArena<crate::invariant::InvariantDefinition>,
    pub machines: OrderedRootArena<crate::machine::Machine>,
    pub platforms: OrderedRootArena<crate::platform::Platform>,
    pub traits: OrderedRootArena<crate::trait_definition::TraitDefinition>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTableStorage {
    pub declarations: SymbolResolvedDeclarationStorage,
    pub bodies: SymbolResolvedBodyStorage,
    pub types: SymbolResolvedTypeStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedDeclarationStorage {
    pub data_members: Arena<data::DataMember>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub proof_facts: Arena<domain::ProofFact>,
    pub domain_path_members: Arena<crate::name::DiagnosticName>,
    pub machine_contained_objects: Arena<crate::machine::ContainedObject>,
    pub machine_owned_data: Arena<crate::machine::OwnedData>,
    pub machine_trait_conformances: Arena<crate::machine::TraitConformance>,
    pub machine_state_handles: Arena<Handle<state::State>>,
    pub machine_states: Arena<state::State>,
    pub platform_state_signatures: Arena<signature::StateSignature>,
    pub trait_requirements: Arena<crate::trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub signature_effects: Arena<crate::name::DiagnosticName>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub statement_path_members: Arena<crate::name::DiagnosticName>,
    pub state_statements: Arena<statement::Statement>,
    pub child_type_references: Arena<types::TypeReference>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTypeStorage {
    pub constraints: Arena<types::TypeConstraint>,
    pub references: types::TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedBodyStorage {
    pub expressions: expression::ExpressionTable,
    pub statements: statement::StatementTable,
}

impl SymbolResolvedTrees {
    pub fn data_members(&self, span: HandleSpan<data::DataMember>) -> &[data::DataMember] {
        self.tables.declarations.data_members.span_or_empty(span)
    }

    pub fn data_type_parameters(
        &self,
        span: HandleSpan<data::TypeParameter>,
    ) -> &[data::TypeParameter] {
        self.tables
            .declarations
            .data_type_parameters
            .span_or_empty(span)
    }

    pub fn proof_facts(&self, span: HandleSpan<domain::ProofFact>) -> &[domain::ProofFact] {
        self.tables.declarations.proof_facts.span_or_empty(span)
    }

    pub fn domain_path_members(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .domain_path_members
            .span_or_empty(span)
    }

    pub fn platform_state_signatures(
        &self,
        span: HandleSpan<signature::StateSignature>,
    ) -> &[signature::StateSignature] {
        self.tables
            .declarations
            .platform_state_signatures
            .span_or_empty(span)
    }

    pub fn trait_machine_signatures(
        &self,
        span: HandleSpan<signature::StateSignature>,
    ) -> &[signature::StateSignature] {
        self.tables
            .declarations
            .trait_machine_signatures
            .span_or_empty(span)
    }

    pub fn trait_requirements(
        &self,
        span: HandleSpan<crate::trait_definition::TraitRequirement>,
    ) -> &[crate::trait_definition::TraitRequirement] {
        self.tables
            .declarations
            .trait_requirements
            .span_or_empty(span)
    }

    pub fn state_parameters(
        &self,
        span: HandleSpan<signature::StateParameter>,
    ) -> &[signature::StateParameter] {
        self.tables
            .declarations
            .state_parameters
            .span_or_empty(span)
    }

    pub fn signature_effects(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .signature_effects
            .span_or_empty(span)
    }

    pub fn signature_contracts(
        &self,
        span: HandleSpan<signature::SignatureContract>,
    ) -> &[signature::SignatureContract] {
        self.tables
            .declarations
            .signature_contracts
            .span_or_empty(span)
    }

    pub fn machine_effects(
        &self,
        machine: &crate::machine::Machine,
    ) -> &[crate::name::DiagnosticName] {
        self.signature_effects(machine.effects)
    }

    pub fn machine_decrease_order(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .signature_effects
            .span_or_empty(span)
    }

    pub fn machine_contracts(
        &self,
        machine: &crate::machine::Machine,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts(machine.contracts)
    }

    pub fn machine_state_handles(
        &self,
        span: HandleSpan<Handle<state::State>>,
    ) -> &[Handle<state::State>] {
        self.tables
            .declarations
            .machine_state_handles
            .span_or_empty(span)
    }

    pub fn machine_state(&self, handle: Handle<state::State>) -> &state::State {
        self.tables.declarations.machine_states.get(handle)
    }

    pub fn machine_contained_objects(
        &self,
        span: HandleSpan<crate::machine::ContainedObject>,
    ) -> &[crate::machine::ContainedObject] {
        self.tables
            .declarations
            .machine_contained_objects
            .span_or_empty(span)
    }

    pub fn machine_owned_data(
        &self,
        span: HandleSpan<crate::machine::OwnedData>,
    ) -> &[crate::machine::OwnedData] {
        self.tables
            .declarations
            .machine_owned_data
            .span_or_empty(span)
    }

    pub fn machine_trait_conformances(
        &self,
        span: HandleSpan<crate::machine::TraitConformance>,
    ) -> &[crate::machine::TraitConformance] {
        self.tables
            .declarations
            .machine_trait_conformances
            .span_or_empty(span)
    }

    pub fn state_statements(
        &self,
        span: HandleSpan<statement::Statement>,
    ) -> &[statement::Statement] {
        self.tables
            .declarations
            .state_statements
            .span_or_empty(span)
    }

    pub fn child_type_references(
        &self,
        span: HandleSpan<types::TypeReference>,
    ) -> &[types::TypeReference] {
        self.tables
            .declarations
            .child_type_references
            .span_or_empty(span)
    }

    pub fn child_type_reference(
        &self,
        handle: Handle<types::TypeReference>,
    ) -> &types::TypeReference {
        self.tables.declarations.child_type_references.get(handle)
    }

    pub fn rebuild_tables(&mut self) {
        let tables =
            tables::SymbolResolvedTreeTables::from_symbol_resolved_trees_with_state_spans(self);
        self.tables.bodies.expressions = tables.bodies.expressions;
        self.tables.bodies.statements = tables.bodies.statements;
        self.tables.types.references = tables.types.references;
    }

    pub fn snapshot(&self) -> snapshot::SymbolResolvedTreesSnapshot {
        snapshot::SymbolResolvedTreesSnapshot::from_symbol_resolved_trees(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }
}

impl PhaseSnapshot for SymbolResolvedTrees {
    type Snapshot = snapshot::SymbolResolvedTreesSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        SymbolResolvedTrees::snapshot(self)
    }
}

impl Deref for SymbolResolvedTrees {
    type Target = SymbolResolvedRoots;

    fn deref(&self) -> &Self::Target {
        &self.roots
    }
}

impl DerefMut for SymbolResolvedTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.roots
    }
}
