use crate::StructuralPathSegment;
use semantic_vocabulary::{
    ContentAlgebra, ContentProjectionExpression, ContentProjectionIdentity, DomainSemanticId,
    StructuralDomainId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralDomainDeclaration {
    pub id: StructuralDomainId,
    /// Stable source-free semantic-domain identity, distinct from this
    /// module-local dense declaration ID.
    pub semantic_domain: DomainSemanticId,
    pub identity: String,
    /// Exact carrier accepted by this domain. Qualification never changes the
    /// runtime carrier and never authorizes its own establishment.
    pub carrier: StructuralTypeId,
    /// Owner-unique normalized `Content<A>` definition, when this
    /// qualification is content-bearing. This row is independent of any
    /// boundary route that may introduce a program-local occurrence; those
    /// routes must replay this exact definition rather than restating one.
    pub content_projection: Option<StructuralContentProjection>,
    /// Domain-owned catalog of routes authorized to establish this
    /// qualification, strictly ordered. Consumers can neither append routes
    /// nor substitute a same-spelled issuer, and private issuer identities
    /// are retained here without granting consumer call, conformance, or
    /// private-type access.
    pub establishment_routes: Vec<StructuralEstablishmentRoute>,
}

/// One route authorized by a domain declaration to establish its
/// qualification. The retained identity is the replay authority: naming a
/// machine is neither trait sealing nor an implicit grant to other machines
/// in its package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEstablishmentRoute {
    /// An exact invocation through this requirement under a valid selected
    /// conformance; `requirement` is the requirement's canonical identity.
    Requirement { requirement: String },
    /// That exact machine's invocation — not another conformer of a
    /// requirement it happens to satisfy. `machine` is the machine's
    /// canonical callable identity.
    ExactMachine { machine: String },
    /// An installed external-root invocation admitted under this boundary
    /// requirement; `requirement` is its canonical boundary identity.
    BoundaryRequirement { requirement: String },
}

/// Establishment evidence binding one domain membership on an operation
/// result to the authorized route that issued it. The establishing
/// occurrence is the enclosing operation itself: replay resolves that
/// operation's callee identity and compares it to the authorized route
/// declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResultQualificationEstablishment {
    /// Domain membership established on this result. Must name a member of
    /// the result's `qualifications` or a projected row's domain.
    pub domain: StructuralDomainId,
    /// Index into that domain declaration's `establishment_routes` catalog.
    pub route: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralContentProjection {
    pub identity: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
    pub expression: ContentProjectionExpression,
}

/// One exact qualification carried by a nonempty structural path beneath a
/// parameter root. The path is occurrence-relative, not a type-wide rule: a
/// qualification on one field never qualifies a sibling, prefix, or root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPathQualification {
    pub path: Vec<StructuralPathSegment>,
    pub domain: StructuralDomainId,
}
