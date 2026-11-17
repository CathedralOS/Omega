//! Closed semantic records and their issuance-time admission.
pub use native_vocabulary::emission::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticEmissionFacts {
    pub admission: crate::EmissionAdmission,
    pub program: EmissionProgramFacts,
    pub projection: OptimizedProgramProjection,
}
