use super::calling_plans::validate_paired_calling_plans;
use psi_checked_trees_to_terminal::CheckedProgramEntryTerminalReceipt;

/// Exact build-owned source-entry custody carried into native realization.
/// This is declaration and calling-contract evidence only: it owns no runtime
/// roots and cannot authorize a physical bootstrap, image, or publication.
#[derive(Debug, Clone, Copy)]
pub struct NativeProgramEntrySettlement<'entry> {
    pub(crate) source: &'entry omega_program_entry_plan::SelectedProgramEntrySourceSignature,
    pub(crate) semantic_boundary_entry_plan:
        Option<&'entry omega_calling_conventions::BoundaryEntryPlan>,
    pub(crate) storage_entry:
        Option<&'entry omega_program_entry_plan::SelectedProgramStorageEntryPlan>,
}

impl<'entry> NativeProgramEntrySettlement<'entry> {
    pub const fn new(
        source: &'entry omega_program_entry_plan::SelectedProgramEntrySourceSignature,
        calling_plans: Option<(
            &'entry omega_calling_conventions::BoundaryEntryPlan,
            &'entry omega_program_entry_plan::SelectedProgramStorageEntryPlan,
        )>,
    ) -> Self {
        let (semantic_boundary_entry_plan, storage_entry) = match calling_plans {
            Some((semantic, storage)) => (Some(semantic), Some(storage)),
            None => (None, None),
        };
        Self {
            source,
            semantic_boundary_entry_plan,
            storage_entry,
        }
    }

    pub const fn source(
        self,
    ) -> &'entry omega_program_entry_plan::SelectedProgramEntrySourceSignature {
        self.source
    }

    pub const fn semantic_boundary_entry_plan(
        self,
    ) -> Option<&'entry omega_calling_conventions::BoundaryEntryPlan> {
        self.semantic_boundary_entry_plan
    }

    pub const fn storage_entry(
        self,
    ) -> Option<&'entry omega_program_entry_plan::SelectedProgramStorageEntryPlan> {
        self.storage_entry
    }

    pub(crate) fn validate_for_target(
        self,
        target: omega_target::NativeTarget,
    ) -> Result<(), String> {
        let slot = self.source.target_slot();
        if slot.owner.native_target() != target {
            return Err(format!(
                "selected ProgramEntry target profile `{}` does not own native target {target:?}",
                slot.owner.target_name(),
            ));
        }
        let declares_two_surfaces = slot.boundary_schema.is_some()
            || slot.physical_arrival_requirement.is_some()
            || slot.physical_contract_package.is_some()
            || slot.physical_calling_convention.is_some()
            || slot.semantic_calling_convention.is_some();
        match (
            declares_two_surfaces,
            self.semantic_boundary_entry_plan,
            self.storage_entry,
        ) {
            (false, None, None) => Ok(()),
            (true, Some(semantic), Some(storage)) => {
                validate_paired_calling_plans(self.source, semantic, storage)
            }
            _ => Err(
                "selected ProgramEntry lost its exact paired semantic/physical calling-plan custody"
                    .into(),
            ),
        }
    }
}

/// Owned, independently replayed source-entry settlement for one canonical
/// Terminal artifact. It remains declaration and calling-contract custody;
/// it grants no semantic wrapper, physical process entry, image installation,
/// or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNativeProgramEntrySettlement {
    pub(crate) checked_entry: CheckedProgramEntryTerminalReceipt,
    pub(crate) target: omega_target::NativeTarget,
    pub(crate) source: omega_program_entry_plan::SelectedProgramEntrySourceSignature,
    pub(crate) semantic_boundary_entry_plan: Option<omega_calling_conventions::BoundaryEntryPlan>,
    pub(crate) storage_entry: Option<omega_program_entry_plan::SelectedProgramStorageEntryPlan>,
}

impl ValidatedNativeProgramEntrySettlement {
    pub const fn checked_entry(&self) -> &CheckedProgramEntryTerminalReceipt {
        &self.checked_entry
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn source(&self) -> &omega_program_entry_plan::SelectedProgramEntrySourceSignature {
        &self.source
    }

    pub const fn semantic_boundary_entry_plan(
        &self,
    ) -> Option<&omega_calling_conventions::BoundaryEntryPlan> {
        self.semantic_boundary_entry_plan.as_ref()
    }

    pub const fn storage_entry(
        &self,
    ) -> Option<&omega_program_entry_plan::SelectedProgramStorageEntryPlan> {
        self.storage_entry.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeProgramEntrySettlementError {
    TargetDrift,
    CallingPlanPairingDrift,
    SourceSignatureSubstitution,
    SourceMachineSubstitution,
    CanonicalArtifactReplay(String),
    TerminalPsiSubstitution,
    TerminalEntrySubstitution,
    TerminalEntryMultiplicity(usize),
}

impl std::fmt::Display for NativeProgramEntrySettlementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeProgramEntrySettlementError {}
