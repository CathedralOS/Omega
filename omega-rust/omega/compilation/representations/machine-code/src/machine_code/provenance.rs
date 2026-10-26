//! Semantic operations and edges attributed to exact native byte intervals.

/// Semantic operation or edge owning one exact emitted byte interval.
/// The emitted code site is the psi provenance verbatim.
pub type SemanticCodeSite = optimization_unit::PsiProvenance;

/// The boundary invocation's emitted provenance, verbatim.
pub fn semantic_code_site(site: semantic_vocabulary::InvocationSite) -> SemanticCodeSite {
    match site {
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            SemanticCodeSite::Operation(operation)
        }
        semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
        | semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
            SemanticCodeSite::Edge(edge)
        }
    }
}

/// Source-free semantic-to-code custody used by independent object replay.
/// It carries no runtime budget, charge, meter, or execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticCodeAttribution {
    pub site: SemanticCodeSite,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}
