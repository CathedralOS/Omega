//! Target-fixed physical process-entry contract.
//!
//! This carrier records what the launch environment supplies. It is distinct
//! from `ProgramStorageEntry::enter`, which is the semantic installation edge
//! used only after a target-authored bootstrap has established storage roots.

use calling_conventions::BoundaryEntryPlan;
use sha2::{Digest, Sha256};

mod exact_linux_x86_64;
mod exact_macos;
mod exact_uefi;
pub use exact_linux_x86_64::*;
pub use exact_macos::*;
pub use exact_uefi::*;

/// Domain-separated commitment to the exact source bytes of one closed
/// toolchain-owned physical-entry contract package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPackageSourceDigest {
    package: target::ProgramEntryPhysicalContractPackage,
    bytes: [u8; 32],
}

impl ProgramEntryPhysicalContractPackageSourceDigest {
    pub fn from_package_source(
        package: target::ProgramEntryPhysicalContractPackage,
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

    pub const fn package(&self) -> target::ProgramEntryPhysicalContractPackage {
        self.package
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPlan {
    target_slot: target::ProgramEntrySlotDeclaration,
    requirement_identity: String,
    target_package: target::ProgramEntryPhysicalContractPackage,
    target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest,
    non_authoritative_target_package_source_report_fingerprint: u64,
    parameter_type_identities: Vec<String>,
    result_type_identity: String,
    calling_plan_report_fingerprint: u64,
    boundary_entry_plan: BoundaryEntryPlan,
    /// Numeric entry-stack closure only where the target contract supplies
    /// one. The authored macOS contract deliberately declares no stack
    /// guarantee: `EntryStack::ProviderSelected` there requires independent
    /// backing, not a replayed toolchain number.
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
        // The contract shape is fixed by the target-owned physical package,
        // never inferred from parameter count. macOS ARM64 publishes the
        // four-register dyld arrival under AAPCS64 and deliberately retains
        // no numeric stack guarantee; UEFI x86-64 publishes the two-input
        // Microsoft-x64 arrival with its exact closed stack evidence.
        let (expected_policy, parameter_count, guaranteed_entry_stack) = match (
            target_slot.owner,
            target_slot.physical_calling_convention,
        ) {
            (
                target::TargetProfile::UefiX64,
                Some(target::ProgramEntryCallingConvention::MicrosoftX64),
            ) => {
                let application = target::TargetSemantics::guaranteed_entry_stack::<
                    target::UefiX86_64,
                >(target_slot.owner)
                .map_err(|error| error.to_string())?;
                let guarantee = target::TargetSemantics::close_guaranteed_entry_stack::<
                    target::UefiX86_64,
                >(target_slot.owner)
                .map_err(|error| error.to_string())?;
                if guarantee.application() != &application {
                    return Err(
                        "physical UEFI entry contract target-stack application and numeric closure drifted"
                            .into(),
                    );
                }
                (
                    calling_conventions::CallingPolicy::MicrosoftX64,
                    2,
                    Some(guarantee),
                )
            }
            (
                target::TargetProfile::MacosArm64,
                Some(target::ProgramEntryCallingConvention::Aapcs64),
            ) => (calling_conventions::CallingPolicy::Aapcs64, 4, None),
            // The Linux kernel enters through the initial process-stack image
            // in rsp and receives completion through exit_group; the authored
            // contract publishes no numeric stack guarantee.
            (
                target::TargetProfile::LinuxX64,
                Some(target::ProgramEntryCallingConvention::SystemVAMD64),
            ) => (calling_conventions::CallingPolicy::SystemVAMD64, 1, None),
            _ => {
                return Err(
                    "physical entry contract requires a target declaration with an authored physical calling convention"
                        .into(),
                );
            }
        };
        if requirement_identity.is_empty()
            || parameter_type_identities.len() != parameter_count
            || parameter_type_identities.iter().any(String::is_empty)
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
        Ok(Self {
            target_slot,
            requirement_identity,
            target_package,
            target_package_source_digest,
            non_authoritative_target_package_source_report_fingerprint,
            parameter_type_identities,
            result_type_identity,
            calling_plan_report_fingerprint,
            boundary_entry_plan,
            guaranteed_entry_stack,
        })
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn target_package(&self) -> target::ProgramEntryPhysicalContractPackage {
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

    pub const fn non_authoritative_target_package_source_report_fingerprint(&self) -> u64 {
        self.non_authoritative_target_package_source_report_fingerprint
    }

    pub fn parameter_type_identities(&self) -> &[String] {
        &self.parameter_type_identities
    }

    pub fn result_type_identity(&self) -> &str {
        &self.result_type_identity
    }

    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.calling_plan_report_fingerprint
    }

    pub const fn boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.boundary_entry_plan
    }

    /// Exact symbolic target-observation application retained as part of the
    /// physical contract's compatibility evidence where the target closes one.
    /// It carries no byte value, runtime stack observation, or address
    /// authority.
    pub const fn guaranteed_entry_stack_application(
        &self,
    ) -> Option<&target::SymbolicTargetObservationApplication> {
        match &self.guaranteed_entry_stack {
            Some(guarantee) => Some(guarantee.application()),
            None => None,
        }
    }

    /// Exact numeric target-contract closure of the symbolic entry-stack
    /// observation. Runtime conformance remains a separate arrival admission,
    /// and a target whose authored contract declares no numeric guarantee
    /// retains `None` rather than an invented bound.
    pub const fn guaranteed_entry_stack(&self) -> Option<&target::TargetEntryStackGuarantee> {
        self.guaranteed_entry_stack.as_ref()
    }
}

fn hash_digest_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::{
        ProgramEntryPhysicalContractPackageSourceDigest, ProgramEntryPhysicalContractPlan,
    };
    use calling_conventions::{
        CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
    };

    fn plan_for_source(source: &[u8], report_fingerprint: u64) -> ProgramEntryPhysicalContractPlan {
        let package = target::ProgramEntryPhysicalContractPackage::UefiX64;
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
            target::TargetProfile::UefiX64.program_entry_slot(),
            "UefiPhysicalEntry::enter".into(),
            package,
            ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(package, source),
            report_fingerprint,
            vec!["EfiImageHandle".into(), "&EfiSystemTable".into()],
            "EfiStatus".into(),
            calling_plan.contract_report_fingerprint(),
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
