//! Tables backing typed declarations, types, expressions and statements.
use crate::*;
use arena::{Handle, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct TypedTreeTables {
    pub runtime_callable_contracts:
        arena::RootedArena<crate::source_state::callable_values::RuntimeCallableContract>,
    pub runtime_callable_lowering: crate::source_state::callable_values::RuntimeCallableLowering,
    pub designated_static_type_arguments: arena::RootedArena<DesignatedStaticTypeArgument>,
    pub retained_product_application_handles: arena::RootedArena<Handle<ProductOperandApplication>>,
    pub product_input_selections:
        arena::RootedArena<crate::source_state::product_operand::ProductInputSelection>,
    pub product_input_argument_steps:
        arena::RootedArena<crate::source_state::product_operand::ProductArgumentStep>,
    pub product_operand_applications: arena::RootedArena<ProductOperandApplication>,
    pub(crate) authored_declaration_selections: crate::source_selections::AuthoredDeclarationSelections,
    pub const_declarations: arena::RootedArena<crate::source_state::constant::ConstDeclaration>,
    pub data_definitions: DataDefinitionTable,
    pub type_aliases: arena::RootedArena<crate::source_state::type_aliases::TypeAliasDefinition>,
    pub data_application_definitions: arena::RootedArena<DataApplicationView>,
    pub data_type_parameters: TypeParameterTable,
    pub declared_type_equations: arena::RootedArena<machine::DeclaredTypeEquation>,
    pub static_type_contexts: crate::source_state::type_system::type_predicates::StaticTypeContexts,
    pub data_members: arena::RootedArena<data::DataMember>,
    pub data_payload_fields: arena::RootedArena<data::DataField>,
    pub domain_definitions: SpelledDeclarationTable<domain::DomainDefinition>,
    pub proof_facts: arena::RootedArena<domain::ProofFact>,
    pub(crate) proof_fact_source_spans: arena::RetainedRows<Option<source_text::SourceSpan>>,
    pub propositions: arena::RootedArena<proposition::PropositionDefinition>,
    pub proposition_binders: arena::RootedArena<proposition::PropositionBinder>,
    pub mathematical_definitions: arena::RootedArena<mathematical::MathematicalDefinition>,
    pub mathematical_parameters: arena::RootedArena<mathematical::MathematicalParameter>,
    pub mathematical_types: arena::RootedArena<mathematical::MathematicalType>,
    pub domain_path_members: arena::RootedArena<crate::source_state::name::Identifier>,
    pub operator_path_members: arena::RootedArena<crate::source_state::name::Identifier>,
    pub machines: MachineDeclarationTable,
    pub measures: arena::RootedArena<measure::MeasureDefinition>,
    pub measure_path_members: arena::RootedArena<crate::source_state::name::Identifier>,
    pub operators: SpelledDeclarationTable<crate::source_state::operator::OperatorDefinition>,
    pub machine_owned_data: arena::RootedArena<machine::OwnedData>,
    pub machine_trait_conformances: arena::RootedArena<machine::TraitConformance>,
    pub machine_states: arena::RootedArena<crate::source_state::state::State>,
    pub state_parameters: arena::RootedArena<signature::StateParameter>,
    pub traits: arena::RootedArena<trait_definition::TraitDefinition>,
    pub conformances: arena::RootedArena<trait_definition::Conformance>,
    pub trait_requirements: arena::RootedArena<trait_definition::TraitRequirement>,
    pub trait_machine_signatures: arena::RootedArena<signature::StateSignature>,
    pub(crate) applied_callable_signatures: callable_applications::AppliedCallableTable,
    pub(crate) callable_substitutions:
        arena::RootedArena<callable_applications::CallableSubstitution>,
    pub(crate) trait_application_frames:
        arena::RootedArena<trait_definition::RetainedTraitApplicationFrame>,
    pub(crate) callable_argument_identities:
        arena::RootedArena<crate::source_state::type_identity::NormalizedTypeIdentity>,
    pub(crate) callable_arguments:
        arena::RootedArena<callable_applications::RequirementLocalArgument>,
    pub(crate) callable_lifetime_substitutions:
        arena::RootedArena<crate::source_state::static_applications::StaticMachineLifetimeBinding>,
    pub signature_invokes: arena::RootedArena<signature::AuthoredInvocation>,
    pub signature_contracts: arena::RootedArena<signature::SignatureContract>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::source_state::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub wire_schemas: arena::RootedArena<wire::WireSchema>,
    pub wire_members: arena::RootedArena<wire::WireMember>,
}

impl TypedTreeTables {
    /// Keep one declaration or completed body's expressions and statements
    /// together. A later source-application update may rewrite that owner,
    /// but must not detach unrelated bodies from the earlier completion batch.
    pub fn begin_declaration_output(&mut self) {
        self.expression_table.finish_output_page();
        self.statement_table.finish_output_page();
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct TypedTreeRoots {
    /// Stored by older compilers when the Build entry was only partly checked.
    #[doc(hidden)]
    pub partial_build_entry: Option<source_text::SourceSpan>,
    /// Retained to recognize that older snapshot format.
    #[doc(hidden)]
    pub partial_build_can_return: bool,
    pub retained_product_applications: HandleSpan<Handle<ProductOperandApplication>>,
    pub const_declarations: HandleSpan<crate::source_state::constant::ConstDeclaration>,
    pub data_definitions: HandleSpan<data::DataDefinition>,
    pub type_aliases: HandleSpan<crate::source_state::type_aliases::TypeAliasDefinition>,
    pub domain_definitions: HandleSpan<domain::DomainDefinition>,
    pub machines: HandleSpan<machine::Machine>,
    pub measures: HandleSpan<measure::MeasureDefinition>,
    pub operators: HandleSpan<crate::source_state::operator::OperatorDefinition>,
    pub machine_token_bindings: HandleSpan<crate::source_state::operator::OperatorDefinition>,
    pub propositions: HandleSpan<proposition::PropositionDefinition>,
    pub mathematical_definitions: HandleSpan<mathematical::MathematicalDefinition>,
    pub traits: HandleSpan<trait_definition::TraitDefinition>,
    pub conformances: HandleSpan<trait_definition::Conformance>,
    pub wire_schemas: HandleSpan<wire::WireSchema>,
}

impl TypedTreeRoots {
    pub fn with_roots(
        data_definitions: HandleSpan<data::DataDefinition>,
        domain_definitions: HandleSpan<domain::DomainDefinition>,
        machines: HandleSpan<machine::Machine>,
        operators: HandleSpan<crate::source_state::operator::OperatorDefinition>,
        traits: HandleSpan<trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            partial_build_entry: None,
            partial_build_can_return: false,
            retained_product_applications: HandleSpan::default(),
            const_declarations: HandleSpan::default(),
            data_definitions,
            type_aliases: HandleSpan::empty(),
            domain_definitions,
            machines,
            measures: HandleSpan::default(),
            operators,
            machine_token_bindings: HandleSpan::default(),
            propositions: HandleSpan::default(),
            mathematical_definitions: HandleSpan::default(),
            traits,
            conformances: HandleSpan::default(),
            wire_schemas: HandleSpan::default(),
        }
    }
}
