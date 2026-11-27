//! Typed access to the record spans and machine bodies in the resolved forest.
use crate::constant::ConstDeclaration;
use crate::data::{DataDefinition, TypeParameter};
use crate::domain::{DomainDefinition, ProofFact};
use crate::machine::{Machine, OwnedData};
use crate::mathematical::{MathematicalType, MathematicalTypeHandle};
use crate::name::DiagnosticName;
use crate::signature::{SignatureContract, StateParameter, StateSignature};
use crate::source_symbols::JoinedPath;
use crate::state::State;
use crate::statement::Statement;
use crate::trait_definition::{TraitDefinition, TraitRequirement};
use crate::types::TypeReference;
use crate::{
    SymbolResolvedDeclarationStorage, SymbolResolvedTrees, data, domain, expression, mathematical,
    operator, signature, state, statement, wire,
};
use arena::{Handle, HandleSpan};
use source_text::SourceSpan;
use symbols::SymbolHandle;

impl SymbolResolvedTrees {
    pub(crate) fn expression_table(&self) -> &expression::ExpressionTable {
        &self.tables.bodies.expressions
    }

    pub(crate) fn expression_table_mut(&mut self) -> &mut expression::ExpressionTable {
        &mut self.tables.bodies.expressions
    }

    pub(crate) fn statement_table(&self) -> &statement::StatementTable {
        &self.tables.bodies.statements
    }

    pub(crate) fn statement_table_mut(&mut self) -> &mut statement::StatementTable {
        &mut self.tables.bodies.statements
    }

    pub(crate) fn declarations(&self) -> &SymbolResolvedDeclarationStorage {
        &self.tables.declarations
    }

    pub(crate) fn declarations_mut(&mut self) -> &mut SymbolResolvedDeclarationStorage {
        &mut self.tables.declarations
    }

    pub fn domain_definition_by_symbol(&self, symbol: SymbolHandle) -> Option<&DomainDefinition> {
        self.domain_definitions
            .iter()
            .find(|definition| definition.symbol == symbol)
    }

    pub fn data_definition_by_symbol(&self, symbol: SymbolHandle) -> Option<&DataDefinition> {
        self.data_definitions
            .iter()
            .find(|definition| definition.symbol == symbol)
    }

    pub fn authored_service_reach_rows_for(
        &self,
        owner: symbols::SymbolHandle,
    ) -> impl Iterator<Item = &signature::AuthoredServiceReachRow> {
        self.authored_service_reach_rows
            .iter()
            .filter(move |row| row.owner == owner)
    }

    pub fn data_members(&self, span: HandleSpan<data::DataMember>) -> &[data::DataMember] {
        self.declarations().data_members.span_or_empty(span)
    }

    pub fn data_payload_fields(&self, span: HandleSpan<data::DataField>) -> &[data::DataField] {
        self.declarations().data_payload_fields.span_or_empty(span)
    }

    pub fn data_type_parameters(&self, span: HandleSpan<TypeParameter>) -> &[TypeParameter] {
        self.declarations().data_type_parameters.span_or_empty(span)
    }

    pub fn proof_facts(&self, span: HandleSpan<domain::ProofFact>) -> &[domain::ProofFact] {
        self.declarations().proof_facts.span_or_empty(span)
    }

    pub fn mathematical_type(&self, handle: MathematicalTypeHandle) -> &MathematicalType {
        self.declarations().mathematical_types.get(handle)
    }

    pub fn mathematical_parameters(
        &self,
        span: HandleSpan<mathematical::MathematicalParameter>,
    ) -> &[mathematical::MathematicalParameter] {
        self.declarations()
            .mathematical_parameters
            .span_or_empty(span)
    }

    pub fn proof_fact_source_span(&self, handle: Handle<ProofFact>) -> Option<SourceSpan> {
        self.declarations()
            .proof_fact_source_spans
            .get(proof_fact_source_span_index(handle))
            .copied()
            .flatten()
    }

    pub fn set_proof_fact_source_span(
        &mut self,
        handle: Handle<domain::ProofFact>,
        source_span: source_text::SourceSpan,
    ) {
        let index = proof_fact_source_span_index(handle);
        self.declarations_mut()
            .proof_fact_source_spans
            .resize(index + 1, None);
        self.declarations_mut().proof_fact_source_spans[index] = Some(source_span);
    }

    pub fn domain_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.declarations().domain_path_members.span_or_empty(span)
    }

    pub fn operator_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.declarations()
            .operator_path_members
            .span_or_empty(span)
    }

    pub fn operator_path_string(
        &self,
        span: HandleSpan<crate::source_resolved::name::DiagnosticName>,
    ) -> String {
        self.operator_path_members(span).joined_path()
    }

    pub fn operator_definitions(
        &self,
        span: HandleSpan<operator::OperatorDefinition>,
    ) -> &[operator::OperatorDefinition] {
        self.declarations().operator_definitions.span_or_empty(span)
    }

    pub fn measure_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.operator_path_members(span)
    }

    pub fn trait_machine_signatures(&self, span: HandleSpan<StateSignature>) -> &[StateSignature] {
        self.declarations()
            .trait_machine_signatures
            .span_or_empty(span)
    }

    pub fn machine_parameter_contract_view<'program>(
        &'program self,
        contract: &'program data::MachineParameterContract,
    ) -> Option<data::MachineParameterContractView<'program>> {
        match contract {
            data::MachineParameterContract::Inferred
            | data::MachineParameterContract::RequirementIdentity => None,
            data::MachineParameterContract::Structural(signature) => {
                Some(data::MachineParameterContractView::Structural(signature))
            }
            data::MachineParameterContract::AuthoredNominal { .. } => None,
            data::MachineParameterContract::Nominal {
                trait_definition,
                requirement,
                ..
            } => {
                let trait_definition = self.trait_definition(*trait_definition)?;
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

    pub fn trait_type_parameters(&self, trait_definition: &TraitDefinition) -> &[TypeParameter] {
        self.data_type_parameters(trait_definition.type_parameters)
    }

    pub fn trait_requirements(&self, span: HandleSpan<TraitRequirement>) -> &[TraitRequirement] {
        self.declarations().trait_requirements.span_or_empty(span)
    }

    pub fn state_parameters(&self, span: HandleSpan<StateParameter>) -> &[StateParameter] {
        self.declarations().state_parameters.span_or_empty(span)
    }

    pub fn signature_invokes(
        &self,
        span: HandleSpan<signature::InvocationReference>,
    ) -> &[signature::InvocationReference] {
        self.declarations().signature_invokes.span_or_empty(span)
    }

    pub fn signature_contracts(&self, span: HandleSpan<SignatureContract>) -> &[SignatureContract] {
        self.declarations().signature_contracts.span_or_empty(span)
    }

    pub fn trait_definition(&self, symbol: symbols::SymbolHandle) -> Option<&TraitDefinition> {
        self.roots.trait_definition(symbol)
    }

    pub fn const_declaration(&self, symbol: symbols::SymbolHandle) -> Option<&ConstDeclaration> {
        self.roots
            .const_declarations
            .iter()
            .find(|declaration| declaration.symbol == symbol)
    }

    pub fn const_declaration_at_source_span(&self, span: SourceSpan) -> Option<&ConstDeclaration> {
        self.roots
            .const_declarations
            .iter()
            .find(|declaration| self.symbols.symbol_source_span(declaration.symbol) == Some(span))
    }

    pub fn machine(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&crate::source_resolved::machine::Machine> {
        self.machines.by_symbol(symbol)
    }

    pub fn machine_invokes(&self, machine: &Machine) -> &[signature::InvocationReference] {
        self.signature_invokes(machine.invokes)
    }

    pub fn machine_type_parameters(&self, machine: &Machine) -> &[data::TypeParameter] {
        self.data_type_parameters(machine.type_parameters)
    }

    pub fn machine_declared_type_equations(
        &self,
        machine: &Machine,
    ) -> &[crate::source_resolved::machine::DeclaredTypeEquation] {
        self.declarations()
            .declared_type_equations
            .span_or_empty(machine.declared_type_equations)
    }

    pub fn machine_ranking_view(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.declarations().ranking_views.span_or_empty(span)
    }

    pub fn machine_contracts(&self, machine: &Machine) -> &[signature::SignatureContract] {
        self.signature_contracts(machine.contracts)
    }

    pub fn machine_state_handles(&self, span: HandleSpan<Handle<State>>) -> &[Handle<State>] {
        self.declarations()
            .machine_state_handles
            .span_or_empty(span)
    }

    pub fn machine_state(&self, handle: Handle<state::State>) -> &state::State {
        self.declarations().machine_states.get(handle)
    }

    pub fn machine_owned_data(&self, span: HandleSpan<OwnedData>) -> &[OwnedData] {
        self.declarations().machine_owned_data.span_or_empty(span)
    }

    pub fn machine_trait_conformances(
        &self,
        span: HandleSpan<crate::source_resolved::machine::TraitConformance>,
    ) -> &[crate::source_resolved::machine::TraitConformance] {
        self.declarations()
            .machine_trait_conformances
            .span_or_empty(span)
    }

    /// Statements under construction, before the resolver publishes the executable body.
    pub fn construction_statements(&self, span: HandleSpan<Statement>) -> &[Statement] {
        self.declarations().state_statements.span_or_empty(span)
    }

    pub fn child_type_references(&self, span: HandleSpan<TypeReference>) -> &[TypeReference] {
        self.declarations()
            .child_type_references
            .span_or_empty(span)
    }

    pub fn child_type_reference(&self, handle: Handle<TypeReference>) -> &TypeReference {
        self.declarations().child_type_references.get(handle)
    }

    pub fn wire_members(&self, span: HandleSpan<wire::WireMember>) -> &[wire::WireMember] {
        self.declarations().wire_members.span_or_empty(span)
    }
}

fn proof_fact_source_span_index(handle: Handle<domain::ProofFact>) -> usize {
    usize::try_from(handle.arena_index())
        .expect("proof fact source-span index exceeds usize")
        .checked_sub(1)
        .expect("proof fact source-span handle must be valid")
}
