use crate::item::*;

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DomainDefinition {
    pub name: Identifier,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub target_type: crate::types::TypeReferenceHandle,
    pub index_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub is_public: bool,
    pub alias: Option<DomainAliasDefinition>,
    pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
    pub authored_routes: Vec<Vec<Identifier>>,
    /// The policy named after `access`: its plan states who may
    /// introduce the domain and who may read or write what it governs.
    pub access_policy: Option<crate::expression::StaticMachineArgument>,
    pub classification: Option<language_core::DomainClassification>,
    pub predicate_presence: language_core::DomainPredicatePresence,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<OperatorDefinition>,
    pub semantic_clause_token_count: usize,
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DomainAliasDefinition {
    pub constituents: Vec<HandleSpan<Identifier>>,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            name: Identifier::generated(""),
            type_parameters: HandleSpan::empty(),
            target_type: crate::types::TypeReferenceHandle::invalid(),
            index_arguments: HandleSpan::empty(),
            is_public: false,
            alias: None,
            cleanup_selection: None,
            authored_routes: Vec::new(),
            access_policy: None,
            classification: None,
            predicate_presence: language_core::DomainPredicatePresence::Absent,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            semantic_clause_token_count: 0,
        }
    }
}
