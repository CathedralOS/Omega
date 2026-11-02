//! Failures owned by target-catalog admission and effect analysis.
use crate::MachineEffectError;
use isa_aarch64::Aarch64MachineEffectCatalogValidationError;
use isa_x86_64::X86_64MachineEffectCatalogValidationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineEffectStageError {
    X86_64Catalog(X86_64MachineEffectCatalogValidationError),
    Aarch64Catalog(Aarch64MachineEffectCatalogValidationError),
    Analysis(MachineEffectError),
}

failed_display!(MachineEffectStageError, "machine-effect analysis failed");
