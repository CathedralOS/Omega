//! Failures owned by target-catalog admission and effect analysis.
use omega_isa_aarch64::Aarch64MachineEffectCatalogValidationError;
use omega_isa_x86_64::X86_64MachineEffectCatalogValidationError;
use omega_machine_optimizer::MachineEffectError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineEffectStageError {
    X86_64Catalog(X86_64MachineEffectCatalogValidationError),
    Aarch64Catalog(Aarch64MachineEffectCatalogValidationError),
    Analysis(MachineEffectError),
    ReceiptMismatch,
}

impl std::fmt::Display for MachineEffectStageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "machine-effect analysis failed: {self:?}")
    }
}

impl std::error::Error for MachineEffectStageError {}
