//! Closed semantic records and their issuance-time admission.
pub use target_vocabulary::publication::{
    DescriptorProducer, DescriptorProducerFact, EmissionFunctionFacts, EmissionOperationProvenance,
    EmissionProgramFacts, OptimizedProgramProjection, PrimitiveLocalFact, RamObligation, RamSource,
    ScalarCallOrigin, ScalarCallResultFact,
};

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SemanticEmissionFacts {
    pub admission: crate::EmissionAdmission,
    pub program: EmissionProgramFacts,
    pub projection: OptimizedProgramProjection,
    target_audit: Option<Arc<()>>,
}

impl SemanticEmissionFacts {
    pub fn new(
        admission: crate::EmissionAdmission,
        program: EmissionProgramFacts,
        projection: OptimizedProgramProjection,
    ) -> Self {
        Self {
            admission,
            program,
            projection,
            target_audit: None,
        }
    }
}

// Explicit replay custody is a per-compilation capability. Semantic equality
// still compares the same three portable facts across independent compiles.
impl PartialEq for SemanticEmissionFacts {
    fn eq(&self, other: &Self) -> bool {
        self.admission == other.admission
            && self.program == other.program
            && self.projection == other.projection
    }
}

impl Eq for SemanticEmissionFacts {}

pub mod target_replay;
