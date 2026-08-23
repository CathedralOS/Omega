use crate::{
    data, domain, expression, measure, operator, proposition, signature, snapshot, state,
    statement, tables, types, wire,
};
use psi_arena::{Arena, Handle, HandleSpan, OrderedRootArena};
use psi_diagnostics::PhaseSnapshot;
use psi_symbols::SymbolTable;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTrees {
    pub roots: SymbolResolvedRoots,
    pub tables: SymbolResolvedTableStorage,
    pub symbols: SymbolTable,
    /// EFX: boundary-trait symbols normalized into service identities after
    /// symbol assignment. This is the source of truth for service reach.
    pub service_reaches: psi_language_semantics::ServiceReachTable,
    pub service_reach_rows: psi_language_semantics::ServiceReachRowTable,
    /// STR4 checked plans, slice 1: the deterministic semantic-domain
    /// interner (declared-name identity, declaration order).
    pub semantic_domains: psi_language_semantics::SemanticDomainTable,
    /// PRV4: normalized `via` bindings, interned once at lowering.
    pub external_bindings: psi_language_semantics::ExternalBindingTable,
    /// Erased evidence assignments removed from runtime statement spans.
    pub evidence_forwardings: Vec<statement::EvidenceForwarding>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedRoots {
    pub data_definitions: OrderedRootArena<crate::data::DataDefinition>,
    pub domain_definitions: OrderedRootArena<crate::domain::DomainDefinition>,
    pub invariant_definitions: OrderedRootArena<crate::invariant::InvariantDefinition>,
    pub machines: OrderedRootArena<crate::machine::Machine>,
    pub measures: OrderedRootArena<measure::MeasureDefinition>,
    pub operators: OrderedRootArena<operator::OperatorDefinition>,
    pub propositions: OrderedRootArena<proposition::PropositionDefinition>,
    pub traits: OrderedRootArena<crate::trait_definition::TraitDefinition>,
    pub conformances: OrderedRootArena<crate::trait_definition::Conformance>,
    pub wire_schemas: OrderedRootArena<wire::WireSchema>,
}

impl SymbolResolvedRoots {
    pub fn with_roots(
        data_definitions: OrderedRootArena<crate::data::DataDefinition>,
        domain_definitions: OrderedRootArena<crate::domain::DomainDefinition>,
        invariant_definitions: OrderedRootArena<crate::invariant::InvariantDefinition>,
        machines: OrderedRootArena<crate::machine::Machine>,
        operators: OrderedRootArena<operator::OperatorDefinition>,
        traits: OrderedRootArena<crate::trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            data_definitions,
            domain_definitions,
            invariant_definitions,
            machines,
            measures: OrderedRootArena::default(),
            operators,
            propositions: OrderedRootArena::default(),
            traits,
            conformances: OrderedRootArena::default(),
            wire_schemas: OrderedRootArena::default(),
        }
    }
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
    pub data_payload_fields: Arena<data::DataField>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub proof_facts: Arena<domain::ProofFact>,
    pub domain_path_members: Arena<crate::name::DiagnosticName>,
    pub operator_path_members: Arena<crate::name::DiagnosticName>,
    pub operator_definitions: Arena<operator::OperatorDefinition>,
    pub proposition_binders: Arena<proposition::PropositionBinder>,
    pub machine_owned_data: Arena<crate::machine::OwnedData>,
    pub machine_trait_conformances: Arena<crate::machine::TraitConformance>,
    pub machine_state_handles: Arena<Handle<state::State>>,
    pub machine_states: Arena<state::State>,
    pub trait_requirements: Arena<crate::trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub ranking_views: Arena<crate::name::DiagnosticName>,
    pub signature_invokes: Arena<crate::name::DiagnosticName>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub statement_path_members: Arena<crate::name::DiagnosticName>,
    pub state_statements: Arena<statement::Statement>,
    pub child_type_references: Arena<types::TypeReference>,
    pub wire_members: Arena<wire::WireMember>,
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
    pub fn with_roots(
        roots: SymbolResolvedRoots,
        tables: SymbolResolvedTableStorage,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            roots,
            tables,
            symbols,
            service_reaches: psi_language_semantics::ServiceReachTable::default(),
            service_reach_rows: psi_language_semantics::ServiceReachRowTable::default(),
            semantic_domains: psi_language_semantics::SemanticDomainTable::default(),
            external_bindings: psi_language_semantics::ExternalBindingTable::default(),
            evidence_forwardings: Vec::new(),
        }
    }

    pub fn data_members(&self, span: HandleSpan<data::DataMember>) -> &[data::DataMember] {
        self.tables.declarations.data_members.span_or_empty(span)
    }

    pub fn data_payload_fields(&self, span: HandleSpan<data::DataField>) -> &[data::DataField] {
        self.tables
            .declarations
            .data_payload_fields
            .span_or_empty(span)
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

    pub fn operator_path_members(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .operator_path_members
            .span_or_empty(span)
    }

    pub fn operator_definitions(
        &self,
        span: HandleSpan<operator::OperatorDefinition>,
    ) -> &[operator::OperatorDefinition] {
        self.tables
            .declarations
            .operator_definitions
            .span_or_empty(span)
    }

    pub fn measure_path_members(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .operator_path_members
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

    /// Project a machine-parameter contract to its single published signature
    /// while retaining whether that signature is structurally owned or an
    /// exact nominal trait requirement. Nominal ownership is revalidated here
    /// so a mismatched symbol pair fails closed.
    pub fn machine_parameter_contract_view<'program>(
        &'program self,
        contract: &'program data::MachineParameterContract,
    ) -> Option<data::MachineParameterContractView<'program>> {
        match contract {
            data::MachineParameterContract::Structural(signature) => {
                Some(data::MachineParameterContractView::Structural(signature))
            }
            data::MachineParameterContract::AuthoredNominal { .. } => None,
            data::MachineParameterContract::Nominal {
                trait_definition,
                requirement,
            } => {
                let trait_definition = self
                    .traits
                    .iter()
                    .find(|candidate| candidate.symbol == *trait_definition)?;
                let requirement = self
                    .trait_machine_signatures(trait_definition.machines)
                    .iter()
                    .find(|candidate| candidate.symbol == *requirement)?;
                Some(data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                })
            }
        }
    }

    pub fn trait_type_parameters(
        &self,
        trait_definition: &crate::trait_definition::TraitDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters(trait_definition.type_parameters)
    }

    pub fn trait_invariants(
        &self,
        trait_definition: &crate::trait_definition::TraitDefinition,
    ) -> &[domain::ProofFact] {
        self.proof_facts(trait_definition.invariants)
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

    pub fn signature_invokes(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .signature_invokes
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

    pub fn machine_invokes(
        &self,
        machine: &crate::machine::Machine,
    ) -> &[crate::name::DiagnosticName] {
        self.signature_invokes(machine.invokes)
    }

    pub fn machine_type_parameters(
        &self,
        machine: &crate::machine::Machine,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters(machine.type_parameters)
    }

    pub fn machine_ranking_view(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables.declarations.ranking_views.span_or_empty(span)
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

    pub fn wire_members(&self, span: HandleSpan<wire::WireMember>) -> &[wire::WireMember] {
        self.tables.declarations.wire_members.span_or_empty(span)
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

#[cfg(test)]
mod tests {
    use crate::{
        SymbolResolvedRoots, SymbolResolvedTableStorage, SymbolResolvedTrees, data, domain,
        invariant, machine, name::DiagnosticName, operator, trait_definition,
    };
    use psi_arena::{HandleSpan, OrderedRootArena};
    use psi_symbols::SymbolTable;

    #[test]
    fn symbol_resolved_roots_constructor_keeps_top_level_roots_explicit() {
        let data_definitions = OrderedRootArena::<data::DataDefinition>::default();
        let domain_definitions = OrderedRootArena::<domain::DomainDefinition>::default();
        let invariant_definitions = OrderedRootArena::<invariant::InvariantDefinition>::default();
        let machines = OrderedRootArena::<machine::Machine>::default();
        let operators = OrderedRootArena::<operator::OperatorDefinition>::default();
        let traits = OrderedRootArena::<trait_definition::TraitDefinition>::default();

        let roots = SymbolResolvedRoots::with_roots(
            data_definitions.clone(),
            domain_definitions.clone(),
            invariant_definitions.clone(),
            machines.clone(),
            operators.clone(),
            traits.clone(),
        );

        assert_eq!(roots.data_definitions, data_definitions);
        assert_eq!(roots.domain_definitions, domain_definitions);
        assert_eq!(roots.invariant_definitions, invariant_definitions);
        assert_eq!(roots.machines, machines);
        assert_eq!(roots.operators, operators);
        assert_eq!(roots.traits, traits);
    }

    #[test]
    fn symbol_resolved_trees_constructor_keeps_roots_tables_and_symbols_explicit() {
        let roots = SymbolResolvedRoots::default();
        let tables = SymbolResolvedTableStorage::default();
        let symbols = SymbolTable::default();

        let trees = SymbolResolvedTrees::with_roots(roots.clone(), tables.clone(), symbols.clone());

        assert_eq!(trees.roots, roots);
        assert_eq!(trees.tables, tables);
        assert_eq!(trees.symbols, symbols);
    }

    #[test]
    fn ranking_views_and_signature_invokes_use_independent_arenas() {
        let mut trees = SymbolResolvedTrees::default();
        let mut ranking_view = HandleSpan::empty();
        let mut signature_invokes = HandleSpan::empty();

        trees
            .tables
            .declarations
            .ranking_views
            .append_to_span(&mut ranking_view, DiagnosticName::generated("remaining"));
        trees.tables.declarations.signature_invokes.append_to_span(
            &mut signature_invokes,
            DiagnosticName::generated("Console.write"),
        );

        assert_eq!(
            trees.machine_ranking_view(ranking_view)[0].as_str(),
            "remaining"
        );
        assert_eq!(
            trees.signature_invokes(signature_invokes)[0].as_str(),
            "Console.write"
        );
    }
}
