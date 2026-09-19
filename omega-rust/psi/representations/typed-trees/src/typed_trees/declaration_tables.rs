//! The declaration tables of the typed trees: constants, data, domains,
//! proof facts, propositions, measures and operators.

use crate::typed_trees::{TypedTrees, proof_fact_source_span_index};
use crate::{data, domain, measure, proposition, signature};
use arena::{Handle, HandleSpan};

impl TypedTrees {
    pub fn retain_authored_declaration_selections(
        &mut self,
        selections: language_semantics::declaration_selection::AuthoredDeclarationSelections,
    ) {
        self.tables.authored_declaration_selections = selections;
    }

    pub fn authored_declaration_selections(
        &self,
    ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections {
        &self.tables.authored_declaration_selections
    }

    pub fn push_const_declaration(&mut self, declaration: crate::constant::ConstDeclaration) {
        self.tables
            .const_declarations
            .append_to_span(&mut self.roots.const_declarations, declaration);
    }

    pub fn const_declarations(&self) -> &[crate::constant::ConstDeclaration] {
        self.tables
            .const_declarations
            .span_or_empty(self.roots.const_declarations)
    }

    pub fn record_resolved_authored_declaration_selection_once(
        &mut self,
        source_span: source::SourceSpan,
        exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
        kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
        selected_symbol: symbols::SymbolHandle,
    ) -> Result<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionRecordError,
    > {
        if let Some(existing) = self
            .tables
            .authored_declaration_selections
            .iter()
            .find(|selection| {
                selection.source_span() == source_span
                    && selection.exposure() == exposure
                    && selection.kind() == kind
                    && selection.compiler_partition().is_none()
                    && matches!(
                        selection.target(),
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                            if target.selected_symbol() == selected_symbol
                    )
            })
        {
            return Ok(existing.occurrence_id());
        }
        self.tables.authored_declaration_selections.record_resolved(
            source_span,
            exposure,
            kind,
            selected_symbol,
        )
    }

    pub fn push_data_definition(&mut self, data_definition: data::DataDefinition) {
        self.tables
            .data_definitions
            .append_to_span(&mut self.roots.data_definitions, data_definition);
    }

    pub fn data_definitions(&self) -> &[data::DataDefinition] {
        self.tables
            .data_definitions
            .span_or_empty(self.roots.data_definitions)
    }

    pub fn push_data_type_parameter(
        &mut self,
        data_definition: &mut data::DataDefinition,
        type_parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut data_definition.type_parameters, type_parameter);
    }

    pub fn data_type_parameters(
        &self,
        data_definition: &data::DataDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(data_definition.type_parameters)
    }

    pub fn push_data_member(
        &mut self,
        data_definition: &mut data::DataDefinition,
        member: data::DataMember,
    ) {
        self.data_members
            .append_to_span(&mut data_definition.members, member);
    }

    pub fn data_members(&self, data_definition: &data::DataDefinition) -> &[data::DataMember] {
        self.data_members.span_or_empty(data_definition.members)
    }

    pub fn push_data_payload_field(
        &mut self,
        variant: &mut data::DataVariant,
        field: data::DataField,
    ) {
        self.data_payload_fields
            .append_to_span(&mut variant.payload, field);
    }

    pub fn data_payload_fields(&self, variant: &data::DataVariant) -> &[data::DataField] {
        self.data_payload_fields.span_or_empty(variant.payload)
    }

    pub fn push_domain_definition(&mut self, domain_definition: domain::DomainDefinition) {
        self.tables
            .domain_definitions
            .append_to_span(&mut self.roots.domain_definitions, domain_definition);
    }

    pub fn push_domain_type_parameter(
        &mut self,
        domain: &mut domain::DomainDefinition,
        parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut domain.type_parameters, parameter);
    }

    pub fn domain_type_parameters(
        &self,
        domain: &domain::DomainDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(domain.type_parameters)
    }

    pub fn push_domain_operator(
        &mut self,
        domain: &mut domain::DomainDefinition,
        operator: crate::operator::OperatorDefinition,
    ) {
        self.operators
            .append_to_span(&mut domain.operators, operator);
    }

    pub fn domain_definitions(&self) -> &[domain::DomainDefinition] {
        self.tables
            .domain_definitions
            .span_or_empty(self.roots.domain_definitions)
    }

    pub fn domain_operators(
        &self,
        domain: &domain::DomainDefinition,
    ) -> &[crate::operator::OperatorDefinition] {
        self.operators.span_or_empty(domain.operators)
    }

    pub fn proof_facts(&self, domain: &domain::DomainDefinition) -> &[domain::ProofFact] {
        self.proof_facts.span_or_empty(domain.facts)
    }

    pub fn proof_fact_source_span(
        &self,
        handle: Handle<domain::ProofFact>,
    ) -> Option<source::SourceSpan> {
        self.tables
            .proof_fact_source_spans
            .get(proof_fact_source_span_index(handle))
            .copied()
            .flatten()
    }

    pub fn set_proof_fact_source_span(
        &mut self,
        handle: Handle<domain::ProofFact>,
        source_span: source::SourceSpan,
    ) {
        let index = proof_fact_source_span_index(handle);
        self.tables.proof_fact_source_spans.resize(index + 1, None);
        self.tables.proof_fact_source_spans[index] = Some(source_span);
    }

    pub fn push_proposition(&mut self, proposition: proposition::PropositionDefinition) {
        self.tables
            .propositions
            .append_to_span(&mut self.roots.propositions, proposition);
    }

    pub fn propositions(&self) -> &[proposition::PropositionDefinition] {
        self.tables
            .propositions
            .span_or_empty(self.roots.propositions)
    }

    pub fn proposition_binders(
        &self,
        proposition: &proposition::PropositionDefinition,
    ) -> &[proposition::PropositionBinder] {
        self.tables
            .proposition_binders
            .span_or_empty(proposition.binders)
    }

    pub fn push_proposition_binder(
        &mut self,
        proposition: &mut proposition::PropositionDefinition,
        binder: proposition::PropositionBinder,
    ) {
        self.tables
            .proposition_binders
            .append_to_span(&mut proposition.binders, binder);
    }

    pub fn proposition_parameters(
        &self,
        proposition: &proposition::PropositionDefinition,
    ) -> &[signature::StateParameter] {
        self.tables
            .state_parameters
            .span_or_empty(proposition.parameters)
    }

    pub fn push_proposition_parameter(
        &mut self,
        proposition: &mut proposition::PropositionDefinition,
        parameter: signature::StateParameter,
    ) {
        self.tables
            .state_parameters
            .append_to_span(&mut proposition.parameters, parameter);
    }

    pub fn push_mathematical_definition(
        &mut self,
        definition: crate::mathematical::MathematicalDefinition,
    ) {
        self.tables
            .mathematical_definitions
            .append_to_span(&mut self.roots.mathematical_definitions, definition);
    }

    pub fn mathematical_definitions(&self) -> &[crate::mathematical::MathematicalDefinition] {
        self.tables
            .mathematical_definitions
            .span_or_empty(self.roots.mathematical_definitions)
    }

    pub fn push_mathematical_parameter(
        &mut self,
        definition: &mut crate::mathematical::MathematicalDefinition,
        parameter: crate::mathematical::MathematicalParameter,
    ) {
        self.tables
            .mathematical_parameters
            .append_to_span(&mut definition.parameters, parameter);
    }

    pub fn mathematical_parameters(
        &self,
        span: HandleSpan<crate::mathematical::MathematicalParameter>,
    ) -> &[crate::mathematical::MathematicalParameter] {
        self.tables.mathematical_parameters.span_or_empty(span)
    }

    pub fn insert_mathematical_type(
        &mut self,
        ty: crate::mathematical::MathematicalType,
    ) -> crate::mathematical::MathematicalTypeHandle {
        self.tables.mathematical_types.insert(ty)
    }

    pub fn mathematical_type(
        &self,
        handle: crate::mathematical::MathematicalTypeHandle,
    ) -> &crate::mathematical::MathematicalType {
        self.tables.mathematical_types.get(handle)
    }

    pub fn domain_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.domain_path_members.span_or_empty(span)
    }

    pub fn push_measure(&mut self, measure: measure::MeasureDefinition) {
        self.tables
            .measures
            .append_to_span(&mut self.roots.measures, measure);
    }

    pub fn measures(&self) -> &[measure::MeasureDefinition] {
        self.tables.measures.span_or_empty(self.roots.measures)
    }

    pub fn push_measure_path_member(
        &mut self,
        measure: &mut measure::MeasureDefinition,
        member: crate::name::Identifier,
    ) {
        self.measure_path_members
            .append_to_span(&mut measure.name, member);
    }

    pub fn measure_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.measure_path_members.span_or_empty(span)
    }

    pub fn push_operator(&mut self, operator: crate::operator::OperatorDefinition) {
        self.tables
            .operators
            .append_to_span(&mut self.roots.operators, operator);
    }

    pub fn operators(&self) -> &[crate::operator::OperatorDefinition] {
        self.tables.operators.span_or_empty(self.roots.operators)
    }

    /// Append one token-bearing machine's operator-signature view. The view
    /// shares the machine's symbol and its parameter, contract, and type
    /// parameter spans; it is never a second declaration.
    pub fn push_machine_token_binding(&mut self, view: crate::operator::OperatorDefinition) {
        self.tables
            .operators
            .append_to_span(&mut self.roots.machine_token_bindings, view);
    }

    /// The operator-signature views of every token-bearing machine, in
    /// machine declaration order.
    pub fn machine_token_bindings(&self) -> &[crate::operator::OperatorDefinition] {
        self.tables
            .operators
            .span_or_empty(self.roots.machine_token_bindings)
    }

    pub fn push_operator_path_member(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        member: crate::name::Identifier,
    ) {
        self.operator_path_members
            .append_to_span(&mut operator.name, member);
    }

    pub fn push_operator_type_parameter(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut operator.type_parameters, parameter);
    }

    pub fn operator_type_parameters(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(operator.type_parameters)
    }

    pub fn push_operator_parameter(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut operator.parameters, parameter);
    }

    pub fn operator_parameters(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(operator.parameters)
    }

    pub fn push_operator_contract(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut operator.contracts, contract);
    }

    pub fn operator_contracts(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(operator.contracts)
    }

    pub fn operator_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.operator_path_members.span_or_empty(span)
    }
}
