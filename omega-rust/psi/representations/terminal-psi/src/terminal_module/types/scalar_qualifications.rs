//! Source-free scalar membership vocabulary and exact edge coercions.
//!
//! This closed catalog carries predicate-free, route-free scalar tags only.
//! An edge coercion adds strict same-carrier membership; it does not change
//! payload bits, erase meaning, execute an operation, or prove a predicate.

use semantic_vocabulary::{
    DomainSemanticId, EdgeId, MachineId, ScalarDomainId, ScalarQualificationSetId, ScalarType,
    ValueId,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarQualificationCatalog {
    pub domains: Vec<ScalarDomainDeclaration>,
    pub sets: Vec<ScalarQualificationSet>,
    pub coercions: Vec<ScalarQualificationCoercion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarDomainDeclaration {
    pub id: ScalarDomainId,
    pub semantic_domain: DomainSemanticId,
    pub identity: String,
    pub carrier: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarQualificationSet {
    pub id: ScalarQualificationSetId,
    pub domains: Vec<ScalarDomainId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarQualificationCoercion {
    pub machine: MachineId,
    pub edge: EdgeId,
    pub argument_ordinal: u32,
    pub source: ValueId,
    pub destination: ValueId,
}
