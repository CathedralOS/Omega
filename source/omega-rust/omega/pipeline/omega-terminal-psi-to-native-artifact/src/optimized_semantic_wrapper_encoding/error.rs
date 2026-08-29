use omega_isa_x86_64::X86_64SemanticUnitWrapperEncodingError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperEncodingError {
    InvalidSemanticPlan,
    SemanticStepShapeMismatch,
    Target(X86_64SemanticUnitWrapperEncodingError),
    TemplateMismatch,
}

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized ProgramStorage semantic wrapper encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedProgramStorageSemanticWrapperEncodingError {}
