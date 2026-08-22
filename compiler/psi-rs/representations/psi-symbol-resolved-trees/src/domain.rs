use crate::name::DiagnosticName;
use crate::types::TypeReference;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub target_type: TypeReference,
    pub index_arguments: HandleSpan<TypeReference>,
    pub is_public: bool,
    /// Authored transparent alias theory, independent from predicate facts.
    pub alias: Option<DomainAliasDefinition>,
    /// Authored exact trait-requirement paths. Normalization resolves these
    /// once into `establishment_routes` after top-level symbols exist.
    pub authored_routes: Vec<Vec<DiagnosticName>>,
    /// Explicit closed domain classification, copied from syntax and never
    /// inferred from the declaration's shape or uses.
    pub classification: Option<psi_language_semantics::DomainClassification>,
    /// Explicit predicate-body presence from the source declaration.
    pub predicate_body: psi_language_semantics::DomainPredicateBody,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub semantic_clause_token_count: usize,
    /// STR4 checked plans, slice 1: the normalized semantic identity from
    /// the program's SemanticDomainTable (populated ONCE at
    /// syntax->resolved, copied downstream; NULL only pre-lowering).
    pub semantic_id: psi_language_semantics::SemanticDomainId,
    /// Role-keyed semantic contributions, populated once at syntax->resolved
    /// and copied downstream. Predicate membership remains independent in
    /// `predicate_body`.
    pub semantic_roles: psi_language_semantics::DomainSemanticRoles,
    /// Normalized authored relationships that may introduce membership.
    /// Populated once after symbol assignment and copied downstream.
    pub establishment_routes: Vec<psi_language_semantics::DomainEstablishmentRoute>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainAliasDefinition {
    /// Nonempty by grammar. Symbols are assigned after all domain declarations
    /// have received their top-level symbols.
    pub constituents: Vec<DomainAliasConstituent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainAliasConstituent {
    pub domain: HandleSpan<DiagnosticName>,
    pub domain_symbol: SymbolHandle,
}

impl Default for DomainAliasConstituent {
    fn default() -> Self {
        Self {
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofFact {
    Expression(crate::expression::ExpressionHandle),
    Membership(ProofMembershipFact),
}

impl Default for ProofFact {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofMembershipFact {
    pub value: crate::expression::ExpressionHandle,
    pub domain: HandleSpan<DiagnosticName>,
    pub domain_symbol: SymbolHandle,
}

impl Default for ProofMembershipFact {
    fn default() -> Self {
        Self {
            value: crate::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::invalid(),
        }
    }
}
