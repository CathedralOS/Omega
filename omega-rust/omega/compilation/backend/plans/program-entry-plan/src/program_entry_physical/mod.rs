//! Target-fixed physical process-entry contract.
//!
//! This carrier records what the launch environment supplies. It is distinct
//! from `ProgramStorageEntry::enter`, which is the semantic installation edge
//! used only after a target-authored bootstrap has established storage roots.

use calling_conventions::{
    BoundaryEntryPlan, EntryStack, MachineRegime, MachineState, MachineStateSet, Preemption,
    StatePlan, ValidatedBoundaryEntryPlan,
};

/// The shared state plan for closed target-authored entry replays: the
/// provider selects the entry stack, nothing is interrupted, saved, or
/// restored, and the transitive regime may use the full ordinary machine
/// state.
fn process_entry_state_plan(initial_regime: MachineRegime) -> StatePlan {
    StatePlan {
        initial_regime,
        interrupted_state: MachineStateSet::default(),
        saved_state: MachineStateSet::default(),
        restored_state: MachineStateSet::default(),
        permitted_transitive_use: MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
            MachineState::Flags,
        ]),
        stack: EntryStack::ProviderSelected,
        preemption: Preemption::NotApplicable,
    }
}

mod arrival;
mod identity;
mod package_source;
pub use identity::{
    LINUX_ARM64_I32_TYPE_IDENTITY, LINUX_ARM64_U64_TYPE_IDENTITY,
    LINUX_X86_64_ADDRESS_TYPE_IDENTITY, LINUX_X86_64_I32_TYPE_IDENTITY,
    MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY,
    MACOS_X86_64_ADDRESS_TYPE_IDENTITY, MACOS_X86_64_I32_TYPE_IDENTITY,
    WINDOWS_X86_64_U32_TYPE_IDENTITY,
};

/// Independently reconstruct and validate the boundary entry plan the closed
/// target package authored. `None` where the package publishes no hosted
/// replay: the UEFI contract's plan is authored and admitted by provider
/// planning, never reconstructed beside it.
pub fn exact_hosted_physical_boundary_entry_plan(
    package: target::ProgramEntryPhysicalContractPackage,
) -> Option<ValidatedBoundaryEntryPlan> {
    arrival::physical_arrival(package).replay()
}

/// Domain-separated commitment to the exact source bytes of one closed
/// toolchain-owned physical-entry contract package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPackageSourceDigest {
    package: target::ProgramEntryPhysicalContractPackage,
    bytes: [u8; 32],
}

/// Strong commitment to the exact source bytes of one closed
/// toolchain-owned physical-entry contract package.
///
/// The package selects its own bundled document (`package_source`), so a
/// caller compares the digest its compiled tree produced against this one
/// without naming the target again.
pub fn exact_physical_contract_package_source_digest(
    package: target::ProgramEntryPhysicalContractPackage,
) -> ProgramEntryPhysicalContractPackageSourceDigest {
    ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        package,
        package_source::exact_package_source(package),
    )
}

impl ProgramEntryPhysicalContractPackageSourceDigest {
    /// Recover a digest claim, without authenticating the package bytes. Native
    /// entry admission still compares it with the independently selected source.
    pub const fn from_retained_digest(
        package: target::ProgramEntryPhysicalContractPackage,
        bytes: [u8; 32],
    ) -> Self {
        Self { package, bytes }
    }

    pub fn from_package_source(
        package: target::ProgramEntryPhysicalContractPackage,
        source: &[u8],
    ) -> Self {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"");
        hash_digest_field(
            &mut digest,
            b"omega.program-entry-physical-contract-package-source.v1",
        );
        hash_digest_field(&mut digest, package.manifest_identity().as_bytes());
        hash_digest_field(&mut digest, package.package_relative_source().as_bytes());
        hash_digest_field(&mut digest, source);
        Self {
            package,
            bytes: digest.finish().into(),
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
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    /// Identity text the contract's indices resolve against — minted beside
    /// the checked program before a Terminal module exists.
    identities: terminal_psi::TerminalIdentityTable,
    target_package: target::ProgramEntryPhysicalContractPackage,
    target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest,
    non_authoritative_target_package_source_report_fingerprint: u64,
    parameter_type_identities: Vec<terminal_psi::TerminalIdentityIndex>,
    result_type_identity: terminal_psi::TerminalIdentityIndex,
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
        // never inferred from parameter count: the package's arrival row names
        // the authored convention, the exact arity, and — where the contract
        // publishes one — the numeric entry-stack closure.
        let arrival = arrival::physical_arrival(target_package);
        if target_slot.physical_calling_convention != Some(arrival.calling_convention) {
            return Err(format!(
                "physical {physical_requirement} entry contract requires a target declaration with the package's authored physical calling convention"
            ));
        }
        let (expected_policy, parameter_count) = (arrival.policy, arrival.parameter_count());
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

    /// The plan-local table this contract's identity indices resolve against.
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

fn hash_digest_field(digest: &mut crate::digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    digest.bytes(bytes);
}
