#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownSelectedLoweringCompletionStatus(u8),
    UnknownX86BranchRelaxationStatus(u8),
    UnknownPostAllocationMachineOptimizationStatus(u8),
    UnknownPostAllocationMachineOptimization(u8),
    ActionCountOverflow,
    ConflictingPhysicalTransformations,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    UnknownLayoutPolicy(u8),
    UnknownScope(u8),
    UnknownFrameDisposition(u8),
    UnknownUnavailableStatus(u8),
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for FunctionRelativeOptimizationRealizationManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid function-relative realization manifest: {self:?}"
        )
    }
}

impl std::error::Error for FunctionRelativeOptimizationRealizationManifestDecodeError {}
