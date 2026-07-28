use crate::TypedTrees;
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub target_type: TypeReferenceHandle,
    pub is_public: bool,
    /// Authored transparent alias theory. Semantic consumers expand this
    /// record before normalization rather than treating the alias as evidence.
    pub alias: Option<DomainAliasDefinition>,
    /// Explicit predicate-body presence copied from the resolved theory.
    pub predicate_body: omega_core::semantics::DomainPredicateBody,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub body_token_count: usize,
    /// STR4 checked plans, slice 1: the normalized semantic identity from
    /// the program's SemanticDomainTable (populated ONCE at
    /// syntax->resolved, copied downstream; NULL only pre-lowering).
    pub semantic_id: omega_core::semantics::SemanticDomainId,
    /// Role-keyed semantic contributions copied from the resolved declaration
    /// without re-derivation.
    pub semantic_roles: omega_core::semantics::DomainSemanticRoles,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            target_type: TypeReferenceHandle::invalid(),
            is_public: false,
            alias: None,
            predicate_body: omega_core::semantics::DomainPredicateBody::Bodyless,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            body_token_count: 0,
            semantic_id: omega_core::semantics::SemanticDomainId::NULL,
            semantic_roles: omega_core::semantics::DomainSemanticRoles::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainAliasDefinition {
    pub constituents: Vec<DomainAliasConstituent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainAliasConstituent {
    pub domain: HandleSpan<Identifier>,
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
    pub domain: HandleSpan<Identifier>,
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

/// Whether one declared domain implies another by normalized semantic identity
/// or by an explicit domain-membership chain.
///
/// This relation belongs with the normalized typed domain graph rather than
/// with any one checker. Capacity-specialized declarations keep distinct
/// carrier-specific symbols so operator lookup can still select the declaration
/// for `[u8; 8]` versus `[u8; 16]`; their shared `semantic_id` is the proof
/// identity. Validation separately requires declarations sharing that identity
/// to have equal predicate bodies, semantic roles, and normalized fact sets.
pub fn declared_domain_implies(
    program: &TypedTrees,
    source_domain: SymbolHandle,
    target_domain: SymbolHandle,
) -> bool {
    fn inner(
        program: &TypedTrees,
        source_domain: SymbolHandle,
        target_domain: SymbolHandle,
        visited: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !source_domain.is_valid() || !target_domain.is_valid() {
            return false;
        }
        if source_domain == target_domain {
            return true;
        }
        if visited.contains(&source_domain) {
            return false;
        }
        visited.push(source_domain);

        let Some(source) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == source_domain)
        else {
            return false;
        };
        let Some(target) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == target_domain)
        else {
            return false;
        };
        if source.semantic_id.is_valid() && source.semantic_id == target.semantic_id {
            return true;
        }

        if source.alias.as_ref().is_some_and(|alias| {
            alias.constituents.iter().any(|constituent| {
                inner(program, constituent.domain_symbol, target_domain, visited)
            })
        }) {
            return true;
        }

        program.proof_facts(source).iter().any(|fact| match fact {
            ProofFact::Membership(membership) => {
                inner(program, membership.domain_symbol, target_domain, visited)
            }
            ProofFact::Expression(_) => false,
        })
    }

    inner(program, source_domain, target_domain, &mut Vec::new())
}
