use super::calling_plans::validate_paired_calling_plans;
use provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;
use terminal_psi::CheckedProgramEntryTerminalReceipt;
use terminal_psi_to_abstract_operations::TerminalPlacedViewEstablishment;

/// Exact build-owned source-entry custody carried into native realization.
/// This is declaration and calling-contract evidence only: it owns no runtime
/// roots and cannot authorize a physical bootstrap, image, or publication.
#[derive(Debug, Clone, Copy)]
pub struct NativeProgramEntrySettlement<'entry> {
    pub(crate) checked_entry: Option<&'entry CheckedProgramEntryTerminalReceipt>,
    pub(crate) source: &'entry program_entry_plan::SelectedProgramEntrySourceSignature,
    pub(crate) semantic_calling_application: Option<&'entry BoundaryCallingPlanRealization>,
    pub(crate) physical_calling_application: Option<&'entry BoundaryCallingPlanRealization>,
    pub(crate) storage_entry: Option<&'entry program_entry_plan::SelectedProgramStorageEntryPlan>,
    pub(crate) fused_service_establishments:
        &'entry [program_entry_plan::ProgramEntryFusedServiceEstablishment],
}

impl<'entry> NativeProgramEntrySettlement<'entry> {
    pub const fn new(
        source: &'entry program_entry_plan::SelectedProgramEntrySourceSignature,
        calling_plans: Option<(
            &'entry BoundaryCallingPlanRealization,
            &'entry BoundaryCallingPlanRealization,
            &'entry program_entry_plan::SelectedProgramStorageEntryPlan,
        )>,
        fused_service_establishments: &'entry [
            program_entry_plan::ProgramEntryFusedServiceEstablishment
        ],
    ) -> Self {
        let (semantic_calling_application, physical_calling_application, storage_entry) =
            match calling_plans {
                Some((semantic, physical, storage)) => {
                    (Some(semantic), Some(physical), Some(storage))
                }
                None => (None, None, None),
            };
        Self {
            checked_entry: None,
            source,
            semantic_calling_application,
            physical_calling_application,
            storage_entry,
            fused_service_establishments,
        }
    }

    /// Retain source-owned entry evidence for replay against the exact artifact.
    /// Attaching this receipt does not validate it or provision a receiver.
    pub const fn with_checked_entry(
        mut self,
        checked_entry: &'entry CheckedProgramEntryTerminalReceipt,
    ) -> Self {
        self.checked_entry = Some(checked_entry);
        self
    }

    pub const fn source(self) -> &'entry program_entry_plan::SelectedProgramEntrySourceSignature {
        self.source
    }

    pub const fn semantic_boundary_entry_plan(
        self,
    ) -> Option<&'entry calling_conventions::BoundaryEntryPlan> {
        match self.semantic_calling_application {
            Some(application) => Some(&application.boundary_entry_plan),
            None => None,
        }
    }

    pub const fn semantic_calling_application(
        self,
    ) -> Option<&'entry BoundaryCallingPlanRealization> {
        self.semantic_calling_application
    }

    pub const fn physical_calling_application(
        self,
    ) -> Option<&'entry BoundaryCallingPlanRealization> {
        self.physical_calling_application
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

    /// Check the target and calling-contract custody before native lowering.
    /// This validates declarations only; Terminal replay, storage provisioning,
    /// and installation authority remain independent requirements.
    pub fn validate_for_target(self, target: target::NativeTarget) -> Result<(), String> {
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
            self.semantic_calling_application,
            self.physical_calling_application,
            self.storage_entry,
        ) {
            (false, None, None, None) => Ok(()),
            (true, Some(semantic), Some(physical), Some(storage)) => {
                validate_paired_calling_plans(self.source, semantic, physical, storage)
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
                // Strictly increasing by the field ROUTE, not the leaf name:
                // a service carrier contributes one establishment at whatever
                // depth the receiver's record-field tree places it, so two
                // bindings differing only in their owning record share a leaf
                // name without being out of order or repeated.
                || previous_field.is_some_and(|field| field >= establishment.field_path())
            {
                return Err(
                    "Fused root establishments drifted from canonical ProgramEntry custody".into(),
                );
            }
            previous_field = Some(establishment.field_path());
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
    pub(crate) semantic_calling_application: Option<BoundaryCallingPlanRealization>,
    pub(crate) physical_calling_application: Option<BoundaryCallingPlanRealization>,
    pub(crate) storage_entry: Option<program_entry_plan::SelectedProgramStorageEntryPlan>,
    pub(crate) fused_service_establishments:
        Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
    /// Provider establishments bound to the entry machine's placed-view
    /// roster rows, in roster order. The executable input boundary joined
    /// each supply to its declared row before settlement replayed the
    /// artifact; the settlement retains the exact loans so the emitted entry
    /// boundary is the custody the binder must satisfy. Empty when the entry
    /// declared no placed-view inputs.
    pub(crate) placed_view_establishments: Vec<TerminalPlacedViewEstablishment>,
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
        match self.semantic_calling_application.as_ref() {
            Some(application) => Some(&application.boundary_entry_plan),
            None => None,
        }
    }

    pub const fn semantic_calling_application(&self) -> Option<&BoundaryCallingPlanRealization> {
        self.semantic_calling_application.as_ref()
    }

    pub const fn physical_calling_application(&self) -> Option<&BoundaryCallingPlanRealization> {
        self.physical_calling_application.as_ref()
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

    /// The provider establishments bound to this entry's placed-view roster
    /// rows, in roster order. Empty when the entry declared no placed-view
    /// inputs.
    pub fn placed_view_establishments(&self) -> &[TerminalPlacedViewEstablishment] {
        &self.placed_view_establishments
    }

    /// Attach the placed-view establishments the executable input boundary
    /// already joined to this artifact's roster. Settlement retains the exact
    /// loans as boundary custody; it does not re-validate them.
    pub(crate) fn with_placed_view_establishments(
        mut self,
        establishments: &[TerminalPlacedViewEstablishment],
    ) -> Self {
        self.placed_view_establishments = establishments.to_vec();
        self
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
    ReceiverEligibilityDrift,
    FusedServiceEstablishmentDrift,
}

impl std::fmt::Display for NativeProgramEntrySettlementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeProgramEntrySettlementError {}
