//! Content postconditions before the module assigns identities to domain declarations.
use super::DomainInstance;
use semantic_vocabulary::StructuralTypeId;
/// A guarantee over qualified structural places. The checked domain catalogue
/// assigns each projection its module identity after reading every declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentEquation {
    pub algebra: semantic_vocabulary::ContentAlgebra,
    pub left: ProjectedContent,
    pub right: ProjectedContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedContent {
    Projection {
        domain: DomainInstance,
        carrier: StructuralTypeId,
        expression: semantic_vocabulary::ContentProjectionExpression,
        subject: semantic_vocabulary::ContentStructuralPlace,
    },
    Separate(Vec<Self>),
}
