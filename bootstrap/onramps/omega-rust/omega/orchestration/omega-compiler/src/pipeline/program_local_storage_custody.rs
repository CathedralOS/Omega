//! Sealed custody joining program-local program-storage stages to their root
//! account registry.
//!
//! Passive `Extent` origins describe lineage but do not own it. Every stage
//! that retains program-local roots therefore remains inseparable from the
//! non-copyable registry that owns their established accounts. Public code may
//! observe either half, but only compiler pipeline transitions may consume the
//! pair.

use super::ProgramStorageEntryDiagnostic;
use omega_external_roots::ProgramLocalExtentRegistry;

#[derive(Debug)]
pub struct ProgramLocalStorageCustody<'root, 'code, Stage> {
    stage: Stage,
    registry: ProgramLocalExtentRegistry<'root, 'code>,
}

impl<'root, 'code, Stage> ProgramLocalStorageCustody<'root, 'code, Stage> {
    pub const fn stage(&self) -> &Stage {
        &self.stage
    }

    pub const fn registry(&self) -> &ProgramLocalExtentRegistry<'root, 'code> {
        &self.registry
    }

    pub(crate) const fn new(
        stage: Stage,
        registry: ProgramLocalExtentRegistry<'root, 'code>,
    ) -> Self {
        Self { stage, registry }
    }

    pub(crate) fn into_parts(self) -> (Stage, ProgramLocalExtentRegistry<'root, 'code>) {
        (self.stage, self.registry)
    }

    pub(crate) fn stage_mut(&mut self) -> &mut Stage {
        &mut self.stage
    }
}

/// A rejected program-local stage transition. The prior stage and its account
/// owner remain joined so callers may correct the input and retry without
/// manufacturing or abandoning lineage.
#[derive(Debug)]
pub struct ProgramLocalStorageCustodyError<'root, 'code, Stage> {
    custody: ProgramLocalStorageCustody<'root, 'code, Stage>,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl<'root, 'code, Stage> ProgramLocalStorageCustodyError<'root, 'code, Stage> {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_custody(self) -> ProgramLocalStorageCustody<'root, 'code, Stage> {
        self.custody
    }

    pub(crate) const fn new(
        custody: ProgramLocalStorageCustody<'root, 'code, Stage>,
        diagnostic: ProgramStorageEntryDiagnostic,
    ) -> Self {
        Self {
            custody,
            diagnostic,
        }
    }
}

impl<Stage> std::fmt::Display for ProgramLocalStorageCustodyError<'_, '_, Stage> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl<Stage: std::fmt::Debug> std::error::Error for ProgramLocalStorageCustodyError<'_, '_, Stage> {}
