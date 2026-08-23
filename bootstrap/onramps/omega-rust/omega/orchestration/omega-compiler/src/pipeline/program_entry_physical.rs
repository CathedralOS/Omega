//! Target-fixed physical process-entry contract.
//!
//! This carrier records what the launch environment supplies. It is distinct
//! from `ProgramStorageEntry::enter`, which is the semantic installation edge
//! used only after a target-authored bootstrap has established storage roots.

use omega_calling_conventions::BoundaryEntryPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPlan {
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    requirement_identity: String,
    target_package: omega_target::ProgramEntryPhysicalContractPackage,
    target_package_fingerprint: u64,
    parameter_type_identities: Vec<String>,
    result_type_identity: String,
    calling_plan_fingerprint: u64,
    boundary_entry_plan: BoundaryEntryPlan,
}

impl ProgramEntryPhysicalContractPlan {
    pub(crate) fn new(
        target_slot: omega_target::ProgramEntrySlotDeclaration,
        requirement_identity: String,
        target_package: omega_target::ProgramEntryPhysicalContractPackage,
        target_package_fingerprint: u64,
        parameter_type_identities: Vec<String>,
        result_type_identity: String,
        calling_plan_fingerprint: u64,
        boundary_entry_plan: BoundaryEntryPlan,
    ) -> Result<Self, String> {
        let Some(physical_requirement) = target_slot.physical_arrival_requirement else {
            return Err("program-entry physical contract has no target-fixed requirement".into());
        };
        if target_slot.owner != omega_target::TargetProfile::UefiX64
            || physical_requirement != "UefiPhysicalEntry::enter"
            || target_slot.physical_contract_package != Some(target_package)
            || target_slot.physical_calling_convention
                != Some(omega_target::ProgramEntryCallingConvention::MicrosoftX64)
        {
            return Err(
                "physical entry contract is restricted to the exact UEFI x86-64 target declaration"
                    .into(),
            );
        }
        if requirement_identity.is_empty()
            || parameter_type_identities.len() != 2
            || parameter_type_identities.iter().any(String::is_empty)
            || result_type_identity.is_empty()
            || target_package_fingerprint == 0
            || calling_plan_fingerprint == 0
        {
            return Err(
                "physical UEFI entry contract lost its exact two parameters, result, or calling-plan identity"
                    .into(),
            );
        }
        if boundary_entry_plan.call.policy != omega_calling_conventions::CallingPolicy::MicrosoftX64
            || boundary_entry_plan.call.parameters.len() != 2
            || boundary_entry_plan.call.result.is_none()
        {
            return Err(
                "physical UEFI entry contract does not realize two Microsoft-x64 inputs and one result"
                    .into(),
            );
        }
        Ok(Self {
            target_slot,
            requirement_identity,
            target_package,
            target_package_fingerprint,
            parameter_type_identities,
            result_type_identity,
            calling_plan_fingerprint,
            boundary_entry_plan,
        })
    }

    pub const fn target_slot(&self) -> omega_target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn target_package(&self) -> omega_target::ProgramEntryPhysicalContractPackage {
        self.target_package
    }

    pub const fn target_package_identity(&self) -> &'static str {
        self.target_package.manifest_identity()
    }

    pub const fn target_package_fingerprint(&self) -> u64 {
        self.target_package_fingerprint
    }

    pub fn parameter_type_identities(&self) -> &[String] {
        &self.parameter_type_identities
    }

    pub fn result_type_identity(&self) -> &str {
        &self.result_type_identity
    }

    pub const fn calling_plan_fingerprint(&self) -> u64 {
        self.calling_plan_fingerprint
    }

    pub const fn boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.boundary_entry_plan
    }
}
