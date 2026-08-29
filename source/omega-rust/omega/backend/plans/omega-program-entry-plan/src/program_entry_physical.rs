//! Target-fixed physical process-entry contract.
//!
//! This carrier records what the launch environment supplies. It is distinct
//! from `ProgramStorageEntry::enter`, which is the semantic installation edge
//! used only after a target-authored bootstrap has established storage roots.

use omega_calling_conventions::BoundaryEntryPlan;
use sha2::{Digest, Sha256};

/// Domain-separated commitment to the exact source bytes of one closed
/// toolchain-owned physical-entry contract package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPackageSourceDigest {
    package: omega_target::ProgramEntryPhysicalContractPackage,
    bytes: [u8; 32],
}

impl ProgramEntryPhysicalContractPackageSourceDigest {
    pub fn from_package_source(
        package: omega_target::ProgramEntryPhysicalContractPackage,
        source: &[u8],
    ) -> Self {
        let mut digest = Sha256::new();
        hash_digest_field(
            &mut digest,
            b"omega.program-entry-physical-contract-package-source.v1",
        );
        hash_digest_field(&mut digest, package.manifest_identity().as_bytes());
        hash_digest_field(&mut digest, package.package_relative_source().as_bytes());
        hash_digest_field(&mut digest, source);
        Self {
            package,
            bytes: digest.finalize().into(),
        }
    }

    pub const fn package(&self) -> omega_target::ProgramEntryPhysicalContractPackage {
        self.package
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPlan {
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    requirement_identity: String,
    target_package: omega_target::ProgramEntryPhysicalContractPackage,
    target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest,
    non_authoritative_target_package_source_report_fingerprint: u64,
    parameter_type_identities: Vec<String>,
    result_type_identity: String,
    calling_plan_fingerprint: u64,
    boundary_entry_plan: BoundaryEntryPlan,
}

impl ProgramEntryPhysicalContractPlan {
    pub fn new(
        target_slot: omega_target::ProgramEntrySlotDeclaration,
        requirement_identity: String,
        target_package: omega_target::ProgramEntryPhysicalContractPackage,
        target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest,
        non_authoritative_target_package_source_report_fingerprint: u64,
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
            || target_package_source_digest.package() != target_package
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
            target_package_source_digest,
            non_authoritative_target_package_source_report_fingerprint,
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

    pub const fn target_package_source_digest(
        &self,
    ) -> ProgramEntryPhysicalContractPackageSourceDigest {
        self.target_package_source_digest
    }

    pub fn target_package_source_matches(&self, source: &[u8]) -> bool {
        self.target_package_source_digest
            == ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                self.target_package,
                source,
            )
    }

    /// Compatibility accessor for the compact, non-authoritative source report
    /// coordinate.
    pub const fn target_package_fingerprint(&self) -> u64 {
        self.non_authoritative_target_package_source_report_fingerprint
    }

    pub const fn non_authoritative_target_package_source_report_fingerprint(&self) -> u64 {
        self.non_authoritative_target_package_source_report_fingerprint
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

fn hash_digest_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
    };

    use super::*;

    fn plan_for_source(source: &[u8], report_fingerprint: u64) -> ProgramEntryPhysicalContractPlan {
        let package = omega_target::ProgramEntryPhysicalContractPackage::UefiX64;
        let pointer = ValueShape::integer(8, 8);
        let calling_plan = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![pointer, pointer],
                result: Some(pointer),
            },
        )
        .expect("physical calling plan");
        ProgramEntryPhysicalContractPlan::new(
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            "UefiPhysicalEntry::enter".into(),
            package,
            ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(package, source),
            report_fingerprint,
            vec!["EfiImageHandle".into(), "&EfiSystemTable".into()],
            "EfiStatus".into(),
            calling_plan.contract_fingerprint(),
            calling_plan.plan().clone(),
        )
        .expect("physical contract")
    }

    #[test]
    fn compact_equal_package_source_substitution_is_rejected_by_strong_commitment() {
        let trusted_source = b"machine UefiPhysicalEntry::enter { trusted body }";
        let substituted_source = b"machine UefiPhysicalEntry::enter { substituted body }";
        let trusted = plan_for_source(trusted_source, 0);
        let substituted = plan_for_source(substituted_source, 0);

        assert_eq!(
            trusted.non_authoritative_target_package_source_report_fingerprint(),
            substituted.non_authoritative_target_package_source_report_fingerprint(),
        );
        assert_ne!(
            trusted.target_package_source_digest(),
            substituted.target_package_source_digest(),
        );
        assert!(trusted.target_package_source_matches(trusted_source));
        assert!(!trusted.target_package_source_matches(substituted_source));
        assert_ne!(trusted, substituted);
    }
}
