use crate::StructuralPathSegment;
use psi_core::{
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
