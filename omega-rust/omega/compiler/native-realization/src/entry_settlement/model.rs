use super::calling_plans::validate_paired_calling_plans;
use terminal_production::CheckedProgramEntryTerminalReceipt;

/// Exact build-owned source-entry custody carried into native realization.
/// This is declaration and calling-contract evidence only: it owns no runtime
/// roots and cannot authorize a physical bootstrap, image, or publication.
#[derive(Debug, Clone, Copy)]
pub struct NativeProgramEntrySettlement<'entry> {
    pub(crate) source: &'entry program_entry_plan::SelectedProgramEntrySourceSignature,
    pub(crate) semantic_boundary_entry_plan: Option<&'entry calling_conventions::BoundaryEntryPlan>,
    pub(crate) storage_entry: Option<&'entry program_entry_plan::SelectedProgramStorageEntryPlan>,
    pub(crate) fused_service_establishments:
        &'entry [program_entry_plan::ProgramEntryFusedServiceEstablishment],
}

impl<'entry> NativeProgramEntrySettlement<'entry> {
    pub const fn new(
        source: &'entry program_entry_plan::SelectedProgramEntrySourceSignature,
        calling_plans: Option<(
            &'entry calling_conventions::BoundaryEntryPlan,
            &'entry program_entry_plan::SelectedProgramStorageEntryPlan,
        )>,
        fused_service_establishments: &'entry [
            program_entry_plan::ProgramEntryFusedServiceEstablishment
        ],
    ) -> Self {
        let (semantic_boundary_entry_plan, storage_entry) = match calling_plans {
            Some((semantic, storage)) => (Some(semantic), Some(storage)),
            None => (None, None),
        };
        Self {
            source,
            semantic_boundary_entry_plan,
            storage_entry,
            fused_service_establishments,
        }
    }

    pub const fn source(self) -> &'entry program_entry_plan::SelectedProgramEntrySourceSignature {
        self.source
    }

    pub const fn semantic_boundary_entry_plan(
        self,
    ) -> Option<&'entry calling_conventions::BoundaryEntryPlan> {
        self.semantic_boundary_entry_plan
    }

    pub const fn storage_entry(
        self,
    ) -> Option<&'entry program_entry_plan::SelectedProgramStorageEntryPlan> {
        self.storage_entry
    }

    pub const fn fused_service_establishments(
        self,
    ) -> &'entry [program_entry_plan::ProgramEntryFusedServiceEstablishment] {
        self.fused_service_establishments
    }

    pub(crate) fn validate_for_target(self, target: target::NativeTarget) -> Result<(), String> {
        let slot = self.source.target_slot();
        if slot.owner.native_target() != target {
            return Err(format!(
                "selected ProgramEntry target profile `{}` does not own native target {target:?}",
                slot.owner.target_name(),
            ));
        }
        self.validate_fused_service_establishments_for_target()?;
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

    pub(crate) fn validate_fused_service_establishments_for_target(self) -> Result<(), String> {
        let source_identity = self.source.identity();
        let slot = self.source.target_slot();
        let receiver_identity = self.source.receiver().normalized_type_identity();
        let mut previous_field = None;
        for establishment in self.fused_service_establishments {
            if establishment.source_signature_identity() != source_identity
                || establishment.target_slot() != slot
                || receiver_identity
                    .is_none_or(|receiver| establishment.receiver_type_identity() != receiver)
                || previous_field.is_some_and(|field| field >= establishment.field_identity())
            {
                return Err(
                    "Fused root establishments drifted from canonical ProgramEntry custody".into(),
                );
            }
            previous_field = Some(establishment.field_identity());
        }
        Ok(())
    }
}

/// Owned, independently replayed source-entry settlement for one canonical
/// Terminal artifact. It remains declaration and calling-contract custody;
/// it grants no semantic wrapper, physical process entry, image installation,
/// or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNativeProgramEntrySettlement {
    pub(crate) checked_entry: CheckedProgramEntryTerminalReceipt,
    pub(crate) target: target::NativeTarget,
    pub(crate) source: program_entry_plan::SelectedProgramEntrySourceSignature,
    pub(crate) semantic_boundary_entry_plan: Option<calling_conventions::BoundaryEntryPlan>,
    pub(crate) storage_entry: Option<program_entry_plan::SelectedProgramStorageEntryPlan>,
    pub(crate) fused_service_establishments:
        Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
}

impl ValidatedNativeProgramEntrySettlement {
    pub const fn checked_entry(&self) -> &CheckedProgramEntryTerminalReceipt {
        &self.checked_entry
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    pub const fn source(&self) -> &program_entry_plan::SelectedProgramEntrySourceSignature {
        &self.source
    }

    pub const fn semantic_boundary_entry_plan(
        &self,
    ) -> Option<&calling_conventions::BoundaryEntryPlan> {
        self.semantic_boundary_entry_plan.as_ref()
    }

    pub const fn storage_entry(
        &self,
    ) -> Option<&program_entry_plan::SelectedProgramStorageEntryPlan> {
        self.storage_entry.as_ref()
    }

    pub fn fused_service_establishments(
        &self,
    ) -> &[program_entry_plan::ProgramEntryFusedServiceEstablishment] {
        &self.fused_service_establishments
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
    FusedServiceEstablishmentDrift,
}

impl std::fmt::Display for NativeProgramEntrySettlementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeProgramEntrySettlementError {}
