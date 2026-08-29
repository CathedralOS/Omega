const PROGRAM_STORAGE_ENTRY_OWNER: &str = "ProgramStorageEntry";
const PROGRAM_STORAGE_ENTRY_METHOD: &str = "enter";

use crate::{ProgramEntryPhysicalContractPlan, ProgramStorageEntryDiagnostic};

/// Exact target-owned environment-to-program slot and its normalized source
/// schema. This is deliberately not a provider plan: `ProgramEntry` accepts an
/// environment root and does not model an outbound service conformance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramStorageEntryPlan {
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    requirement_identity: String,
    schema: omega_effects::provider_plan::ServiceSchema,
    physical_contract: Option<ProgramEntryPhysicalContractPlan>,
}

impl SelectedProgramStorageEntryPlan {
    pub fn from_target_slot(
        slot: omega_target::ProgramEntrySlotDeclaration,
        schema: omega_effects::provider_plan::ServiceSchema,
        requirement_identity: String,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        if slot != slot.owner.program_entry_slot()
            || slot.schema != omega_target::ProgramEntrySchema::ProgramStorageApplication
            || slot.visible_parameters
                != omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
            || slot.semantic_arrival_requirement
                != format!("{PROGRAM_STORAGE_ENTRY_OWNER}::{PROGRAM_STORAGE_ENTRY_METHOD}")
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` does not declare the exact program-storage entry contract",
                slot.owner.root_slot_owner_name(),
                slot.slot_name
            )));
        }
        let Some(boundary_schema) = slot.boundary_schema else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` has no source boundary schema",
                slot.owner.root_slot_owner_name(),
                slot.slot_name
            )));
        };
        if schema.trait_name != boundary_schema {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` requires boundary schema `{boundary_schema}`, not `{}`",
                slot.owner.root_slot_owner_name(),
                slot.slot_name,
                schema.trait_name
            )));
        }
        if requirement_identity.is_empty() {
            return Err(ProgramStorageEntryDiagnostic(
                "target program-storage entry has no exact arrival requirement identity".into(),
            ));
        }
        let matching_methods = schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == requirement_identity)
            .collect::<Vec<_>>();
        let [method] = matching_methods.as_slice() else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target program-storage entry schema retains {} copies of exact arrival requirement `{requirement_identity}`",
                matching_methods.len(),
            )));
        };
        if method.requirement_owner != PROGRAM_STORAGE_ENTRY_OWNER
            || method.name != PROGRAM_STORAGE_ENTRY_METHOD
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target program-storage arrival requirement `{requirement_identity}` drifted from `{PROGRAM_STORAGE_ENTRY_OWNER}::{PROGRAM_STORAGE_ENTRY_METHOD}`",
            )));
        }

        Ok(Self {
            target_slot: slot,
            requirement_identity,
            schema,
            physical_contract: None,
        })
    }

    pub fn with_physical_contract(
        mut self,
        physical_contract: ProgramEntryPhysicalContractPlan,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        if physical_contract.target_slot() != self.target_slot {
            return Err(ProgramStorageEntryDiagnostic(
                "physical entry contract belongs to a different target slot".into(),
            ));
        }
        if self.physical_contract.is_some() {
            return Err(ProgramStorageEntryDiagnostic(
                "selected program-storage entry already has a physical contract".into(),
            ));
        }
        self.physical_contract = Some(physical_contract);
        Ok(self)
    }

    pub const fn target_slot(&self) -> omega_target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn schema(&self) -> &omega_effects::provider_plan::ServiceSchema {
        &self.schema
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn physical_contract(&self) -> Option<&ProgramEntryPhysicalContractPlan> {
        self.physical_contract.as_ref()
    }
}
