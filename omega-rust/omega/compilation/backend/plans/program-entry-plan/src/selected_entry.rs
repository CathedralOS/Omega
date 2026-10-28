//! Start here. The selected program storage entry plan; its root role,
//! diagnostic, derived boundary storage and fused service establishment
//! receipt live beside it.

pub(crate) mod diagnostic;
pub(crate) mod root_role;

const PROGRAM_STORAGE_ENTRY_OWNER: &str = "ProgramStorageEntry";
const PROGRAM_STORAGE_ENTRY_METHOD: &str = "enter";

use crate::{ProgramEntryPhysicalContractPlan, ProgramStorageEntryDiagnostic};

/// Exact target-owned environment-to-program slot and its normalized source
/// schema. This is deliberately not a provider plan: `ProgramEntry` accepts an
/// environment root and does not model an outbound service conformance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramStorageEntryPlan {
    target_slot: target::ProgramEntrySlotDeclaration,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    schema: effects::provider_plan::ServiceSchema,
    physical_contract: Option<ProgramEntryPhysicalContractPlan>,
    /// Identity text the plan's indices resolve against — minted beside the
    /// checked program before a Terminal module exists.
    identities: terminal_psi::TerminalIdentityTable,
}

impl SelectedProgramStorageEntryPlan {
    pub fn from_target_slot(
        slot: target::ProgramEntrySlotDeclaration,
        schema: effects::provider_plan::ServiceSchema,
        requirement_identity: String,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        // The semantic continuation is `ProgramStorageEntry::enter` under
        // every admitted entry contract. Where its two roots become visible
        // is fixed by the slot's declared shape: a freestanding
        // `ProgramStorageApplication` exposes them as authored parameters,
        // while a hosted `HostedApplication` keeps them as internal bridge
        // inputs and exposes no source-visible parameters. Any other shape
        // pairing is drift, not a third contract.
        let declares_exact_continuation = match slot.schema {
            target::ProgramEntrySchema::ProgramStorageApplication => {
                slot.visible_parameters
                    == target::ProgramEntryVisibleParameters::ImageAndInitialStorage
            }
            target::ProgramEntrySchema::HostedApplication => {
                slot.visible_parameters == target::ProgramEntryVisibleParameters::None
            }
        };
        if slot != slot.owner.program_entry_slot()
            || !declares_exact_continuation
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
        if schema.identity_text(schema.trait_name) != boundary_schema {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` requires boundary schema `{boundary_schema}`, not `{}`",
                slot.owner.root_slot_owner_name(),
                slot.slot_name,
                schema.identity_text(schema.trait_name)
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
            .filter(|method| {
                schema.identities.id_at(&requirement_identity) == Some(method.requirement_identity)
            })
            .collect::<Vec<_>>();
        let [method] = matching_methods.as_slice() else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target program-storage entry schema retains {} copies of exact arrival requirement `{}`",
                matching_methods.len(),
                requirement_identity,
            )));
        };
        if method.requirement_owner != PROGRAM_STORAGE_ENTRY_OWNER
            || method.name != PROGRAM_STORAGE_ENTRY_METHOD
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target program-storage arrival requirement `{requirement_identity}` drifted from `{PROGRAM_STORAGE_ENTRY_OWNER}::{PROGRAM_STORAGE_ENTRY_METHOD}`",
            )));
        }

        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let requirement_identity = identities.intern(&requirement_identity);
        Ok(Self {
            target_slot: slot,
            requirement_identity,
            schema,
            physical_contract: None,
            identities: identities.build(),
        })
    }

    /// The plan-local table this plan's requirement index resolves against.
    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
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

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn schema(&self) -> &effects::provider_plan::ServiceSchema {
        &self.schema
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    pub const fn physical_contract(&self) -> Option<&ProgramEntryPhysicalContractPlan> {
        self.physical_contract.as_ref()
    }
}
