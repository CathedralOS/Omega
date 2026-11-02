//! Source-visible program entry requirements supplied by a target's slot catalog.
//! Physical arrival, calling conventions and storage realization are not source
//! compatibility inputs and remain with the target owner.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntrySchema {
    /// Hosted launch hides physical image/storage arrival from source.
    HostedApplication,
    /// Freestanding launch exposes the image and initial-storage roots.
    ProgramStorageApplication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntryVisibleParameters {
    None,
    ImageAndInitialStorage,
}

/// The source contract of an already recognized target-owned entry slot.
/// Names locate the pinned core requirement or label diagnostics; the checker
/// resolves exact source identities rather than treating these as authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntrySourceContract {
    pub owner: &'static str,
    pub slot_name: &'static str,
    pub schema: ProgramEntrySchema,
    pub semantic_arrival_requirement: &'static str,
    pub visible_parameters: ProgramEntryVisibleParameters,
}

/// Target catalog access without source programs, declarations or judgments.
/// Psi asks only after lexical visibility succeeds, preserving query diagnostics.
pub trait ProgramEntrySourceCatalog: Sync {
    fn source_contract(&self, slot: &str) -> Result<ProgramEntrySourceContract, String>;
}
