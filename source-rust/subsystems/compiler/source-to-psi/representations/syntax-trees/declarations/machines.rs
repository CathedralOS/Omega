use crate::item::*;

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct Machine {
    pub name: Identifier,
    pub generic_data_template: Identifier,
    pub default_body_trait: Identifier,
    pub attached_data: Option<Identifier>,
    pub spelling: Option<OperatorSpelling>,
    pub is_public: bool,
    pub bodyless: bool,
    pub target_family_interface: bool,
    pub target: Option<Identifier>,
    pub authored_target: Option<Identifier>,
    pub boundary: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub satisfies: HandleSpan<SatisfiesClause>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub terminates_guarantee: bool,
    pub ranking_subjects: HandleSpan<crate::expression::ExpressionHandle>,
    pub ranking_view: HandleSpan<Identifier>,
    pub ranking_view_arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub ranking_range: crate::expression::ExpressionHandle,
    pub service_reach_keyword_source_spans: Vec<source_text::SourceSpan>,
    pub service_reaches: HandleSpan<crate::types::TypeReferenceHandle>,
    pub service_reach_is_installation_bound: bool,
    pub invokes: HandleSpan<crate::types::TypeReferenceHandle>,
    pub suspends_keyword_source_spans: Vec<source_text::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source_text::SourceSpan>,
    pub suspends: bool,
    pub blocks: bool,
    pub admitted_same_stack_bytes: u64,
    /// The header wrote `crashes none`, which publishes no row in
    /// `contracts`. `wiki/spec/language/effects.md`: an explicit
    /// empty value "promises the empty value and rejects any body or
    /// selection that violates it", so on a private body it is not
    /// the omitted clause that requests inference.
    pub declares_no_crash_route: bool,
    pub contracts: HandleSpan<CapabilityContract>,
    pub states: HandleSpan<StateHandle>,
    pub nested_machines: HandleSpan<Item>,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct State {
    pub assembly_target_requirement: language_core::inline_assembly::AsmTargetApplicability,
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<CapabilityContract>,
    pub statements: HandleSpan<crate::statement::StatementHandle>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TraitDefinition {
    pub is_boundary: bool,
    pub is_public: bool,
    pub name: Identifier,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub parents: HandleSpan<crate::types::TypeReferenceHandle>,
    pub requires: HandleSpan<Identifier>,
    pub machines: HandleSpan<StateSignatureHandle>,
    pub refines: Option<crate::types::TypeReferenceHandle>,
    pub refinement_clauses: Vec<TraitRefinementClause>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TraitRefinementClause {
    pub requirement: Option<Identifier>,
    pub signature: StateSignature,
}

/// Executable statements followed by the declarations visible within this body.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct MachineBody {
    pub assembly_target_requirement: language_core::inline_assembly::AsmTargetApplicability,
    pub statements: HandleSpan<crate::statement::StatementHandle>,
    pub nested_machines: HandleSpan<Item>,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct StateSignature {
    pub name: Identifier,
    pub spelling: Option<crate::operator_spelling::OperatorSpelling>,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub native_callback_parameters: Vec<NativeCallbackParameterNode>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub service_reach_is_installation_bound: bool,
    pub service_reach_keyword_source_spans: Vec<source_text::SourceSpan>,
    pub service_reaches: HandleSpan<crate::types::TypeReferenceHandle>,
    pub invokes: HandleSpan<crate::types::TypeReferenceHandle>,
    pub suspends_keyword_source_spans: Vec<source_text::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source_text::SourceSpan>,
    pub suspends: bool,
    pub blocks: bool,
    pub admitted_same_stack_bytes: u64,
    pub contracts: HandleSpan<CapabilityContract>,
    pub default_body: Option<MachineBody>,
    pub terminates_guarantee: bool,
    /// The requirement wrote `crashes none`. That clause publishes no
    /// contract row, so this is the only record that the empty crash
    /// set was authored rather than omitted, which for a requirement
    /// defers to the selected member's routes.
    pub declares_no_crash_route: bool,
}
