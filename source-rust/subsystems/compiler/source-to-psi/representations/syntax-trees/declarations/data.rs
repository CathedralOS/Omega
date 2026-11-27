use crate::item::*;

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct DataDefinition {
    pub name: Identifier,
    pub is_public: bool,
    pub supply_mode: language_core::DataSupplyMode,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub generic_instance: Option<crate::types::TypeReferenceHandle>,
    pub conformance_bounds: Vec<crate::item::GenericConformanceBound>,
    pub properties: DataProperties,
    pub quotient: Option<QuotientDefinition>,
    pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
    pub requires_facts: HandleSpan<ProofFact>,
    pub members: HandleSpan<DataMember>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct QuotientDefinition {
    pub carrier: crate::types::TypeReferenceHandle,
    pub relation: HandleSpan<Identifier>,
    /// `same_length<T>` after `%`: the relation's generic arguments.
    pub relation_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub equivalence: Option<QuotientEquivalenceSelection>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct QuotientEquivalenceSelection {
    pub relation: HandleSpan<Identifier>,
    pub relation_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub trait_name: Identifier,
    pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub conformance_name: Identifier,
    /// `SameLength<T>` after `as`: the conformance's generic arguments.
    pub conformance_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct DataProperties {
    pub multiplicity: language_core::Multiplicity,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
    Retired(u64),
}

impl Default for DataMember {
    fn default() -> Self {
        Self::Variant(DataVariant::default())
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct DataField {
    pub identity: Option<u64>,
    pub name: Identifier,
    pub relevance: language_core::BindingRelevance,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct DataVariant {
    pub identity: Option<u64>,
    pub name: Identifier,
    pub payload: HandleSpan<DataField>,
    pub requires_facts: HandleSpan<ProofFact>,
    pub retired_payload_identities: Vec<u64>,
}
