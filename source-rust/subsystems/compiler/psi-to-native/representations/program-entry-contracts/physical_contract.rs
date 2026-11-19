//! The validated target slot, arrival types and calling plan retained together.

use calling_conventions::{BoundaryEntryPlan, process_entry::physical_arrival};
use target::entry_contract::ProgramEntryPhysicalContractPackageSourceDigest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPlan {
    target_slot: target::ProgramEntrySlotDeclaration,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    identities: terminal_psi::TerminalIdentityTable,
    target_package: target::ProgramEntryPhysicalContractPackage,
    target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest,
    non_authoritative_target_package_source_report_fingerprint: u64,
    parameter_type_identities: Vec<terminal_psi::TerminalIdentityIndex>,
    result_type_identity: terminal_psi::TerminalIdentityIndex,
    calling_plan_report_fingerprint: u64,
    boundary_entry_plan: BoundaryEntryPlan,
    guaranteed_entry_stack: Option<target::TargetEntryStackGuarantee>,
}

impl ProgramEntryPhysicalContractPlan {
    pub fn new(
        target_slot: target::ProgramEntrySlotDeclaration,
        requirement_identity: String,
        target_package: target::ProgramEntryPhysicalContractPackage,
        target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest,
        non_authoritative_target_package_source_report_fingerprint: u64,
        parameter_type_identities: Vec<String>,
        result_type_identity: String,
        calling_plan_report_fingerprint: u64,
        boundary_entry_plan: BoundaryEntryPlan,
    ) -> Result<Self, String> {
        let Some(physical_requirement) = target_slot.physical_arrival_requirement else {
            return Err("program-entry physical contract has no target-fixed requirement".into());
        };
        if target_slot != target_slot.owner.program_entry_slot()
            || physical_requirement.is_empty()
            || target_slot.physical_contract_package != Some(target_package)
            || target_package_source_digest.package() != target_package
        {
            return Err(
                "physical entry contract drifted from its exact target slot declaration".into(),
            );
        }
        let arrival = physical_arrival(target_package);
        if target_slot.physical_calling_convention != Some(arrival.calling_convention()) {
            return Err(format!(
                "physical {physical_requirement} entry contract requires a target declaration with the package's authored physical calling convention"
            ));
        }
        let (expected_policy, parameter_count) = (arrival.policy(), arrival.parameter_count());
        let guaranteed_entry_stack = arrival.guaranteed_entry_stack(target_slot.owner)?;
        if requirement_identity.is_empty()
            || parameter_type_identities.len() != parameter_count
            || parameter_type_identities
                .iter()
                .any(|identity| identity.is_empty())
            || result_type_identity.is_empty()
            || calling_plan_report_fingerprint == 0
        {
            return Err(format!(
                "physical {physical_requirement} entry contract lost its exact {parameter_count} parameters, result, or calling-plan identity"
            ));
        }
        if boundary_entry_plan.call.policy != expected_policy
            || boundary_entry_plan.call.parameters.len() != parameter_count
            || boundary_entry_plan.call.result.is_none()
        {
            return Err(format!(
                "physical {physical_requirement} entry contract does not realize {parameter_count} {expected_policy:?} inputs and one result"
            ));
        }
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        Ok(Self {
            target_slot,
            requirement_identity: identities.intern(&requirement_identity),
            target_package,
            target_package_source_digest,
            non_authoritative_target_package_source_report_fingerprint,
            parameter_type_identities: parameter_type_identities
                .iter()
                .map(|identity| identities.intern(identity))
                .collect(),
            result_type_identity: identities.intern(&result_type_identity),
            calling_plan_report_fingerprint,
            boundary_entry_plan,
            guaranteed_entry_stack,
            identities: identities.build(),
        })
    }

    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    pub const fn target_package(&self) -> target::ProgramEntryPhysicalContractPackage {
        self.target_package
    }

    pub const fn target_package_source_digest(
        &self,
    ) -> ProgramEntryPhysicalContractPackageSourceDigest {
        self.target_package_source_digest
    }

    pub const fn non_authoritative_target_package_source_report_fingerprint(&self) -> u64 {
        self.non_authoritative_target_package_source_report_fingerprint
    }

    pub fn parameter_type_identities(&self) -> &[terminal_psi::TerminalIdentityIndex] {
        &self.parameter_type_identities
    }

    pub const fn result_type_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.result_type_identity
    }

    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.calling_plan_report_fingerprint
    }

    pub const fn boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.boundary_entry_plan
    }

    pub const fn guaranteed_entry_stack_application(
        &self,
    ) -> Option<&target::SymbolicTargetObservationApplication> {
        match &self.guaranteed_entry_stack {
            Some(guarantee) => Some(guarantee.application()),
            None => None,
        }
    }

    pub const fn guaranteed_entry_stack(&self) -> Option<&target::TargetEntryStackGuarantee> {
        self.guaranteed_entry_stack.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::process_entry::exact_hosted_physical_boundary_entry_plan;
    use target::entry_contract::{
        MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY,
        exact_physical_contract_package_source_digest,
    };
    use target::{ProgramEntryPhysicalContractPackage as Package, TargetProfile};

    #[test]
    fn physical_contract_retains_interned_arrival_types_and_rejects_incomplete_inputs() {
        let slot = TargetProfile::MacosArm64.program_entry_slot();
        let package = Package::MacosArm64;
        let calling = exact_hosted_physical_boundary_entry_plan(package).unwrap();
        let parameters = vec![
            MACOS_ARM64_I32_TYPE_IDENTITY.to_owned(),
            MACOS_ARM64_ADDRESS_TYPE_IDENTITY.to_owned(),
            MACOS_ARM64_ADDRESS_TYPE_IDENTITY.to_owned(),
            MACOS_ARM64_ADDRESS_TYPE_IDENTITY.to_owned(),
        ];
        let capture = |parameters, result| {
            ProgramEntryPhysicalContractPlan::new(
                slot,
                slot.physical_arrival_requirement.unwrap().to_owned(),
                package,
                exact_physical_contract_package_source_digest(package),
                1,
                parameters,
                result,
                calling.contract_report_fingerprint(),
                calling.plan().clone(),
            )
        };
        let plan = capture(parameters.clone(), MACOS_ARM64_I32_TYPE_IDENTITY.to_owned()).unwrap();
        assert_eq!(
            plan.identities().text(plan.requirement_identity()),
            slot.physical_arrival_requirement
        );
        for (&identity, expected) in plan.parameter_type_identities().iter().zip(&parameters) {
            assert_eq!(plan.identities().text(identity), Some(expected.as_str()));
        }
        assert_eq!(
            plan.parameter_type_identities()[0],
            plan.result_type_identity()
        );
        assert!(
            plan.parameter_type_identities()[1..]
                .iter()
                .all(|identity| *identity == plan.parameter_type_identities()[1])
        );
        assert!(capture(Vec::new(), MACOS_ARM64_I32_TYPE_IDENTITY.to_owned()).is_err());
        assert!(capture(parameters, String::new()).is_err());
    }
}
