//! Check and identify the terminal authority leaves reviewed against a physical
//! classification and an optional receiving permission policy. The native
//! reviewer supplies the reachable leaves from its admitted execution.
#![forbid(unsafe_code)]

use semantic_vocabulary::{TerminalAuthorityClass, TerminalAuthorityDisposition};

mod mechanisms;
pub use mechanisms::{
    CheckedPhysicalOperationIdentity, CheckedPhysicalTerminalMechanismIdentity,
    CheckedSyscallArgumentContractIdentity, NormalizedForeignArgumentContract,
    NormalizedForeignTerminalMechanismIdentity, SyscallTerminalMechanismIdentity,
    TableBindingAuthority, TerminalMechanismIdentity, terminal_mechanism_identity_bytes,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAuthorityPolicyIdentity {
    version: u32,
    commitment: [u8; 32],
}

impl TerminalAuthorityPolicyIdentity {
    pub const fn from_parts(version: u32, commitment: [u8; 32]) -> Self {
        Self {
            version,
            commitment,
        }
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn commitment(self) -> [u8; 32] {
        self.commitment
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalAuthorityPermissionPolicyIdentity {
    version: u32,
    commitment: [u8; 32],
}

impl TerminalAuthorityPermissionPolicyIdentity {
    pub const fn from_parts(version: u32, commitment: [u8; 32]) -> Self {
        Self {
            version,
            commitment,
        }
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn commitment(self) -> [u8; 32] {
        self.commitment
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalAuthorityClosureLeafOrigin {
    SelectedBoundary,
    CheckedBoundary {
        machine: semantic_vocabulary::MachineId,
        site: semantic_vocabulary::InvocationSite,
        context: [u8; 32],
    },
    CheckedPhysical {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    CheckedIntrinsic {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
    },
}

impl TerminalAuthorityClosureLeafOrigin {
    pub const fn checked_site(
        self,
    ) -> Option<(
        semantic_vocabulary::MachineId,
        semantic_vocabulary::OperationId,
    )> {
        match self {
            Self::SelectedBoundary | Self::CheckedBoundary { .. } => None,
            Self::CheckedPhysical { machine, operation }
            | Self::CheckedIntrinsic { machine, operation } => Some((machine, operation)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAuthorityClosureCustody {
    SelectedRequirement {
        service_schema: build_providers::plans::ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        declared_requirement_identity: terminal_psi::TerminalIdentityIndex,
        provider_plan: build_providers::plans::ProviderPlanDigest,
    },
    NativeBinding {
        service_schema: build_providers::plans::ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        declared_requirement_identity: terminal_psi::TerminalIdentityIndex,
        receipt: [u8; 32],
    },
    ProgramRoot {
        service_identity: terminal_psi::TerminalIdentityIndex,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureLeaf {
    custody: TerminalAuthorityClosureCustody,
    mechanism: TerminalMechanismIdentity,
    origin: TerminalAuthorityClosureLeafOrigin,
    exercised: Option<TerminalAuthorityDisposition>,
    permitted: Option<TerminalAuthorityDisposition>,
}

impl TerminalAuthorityClosureLeaf {
    pub fn new(
        service_schema: build_providers::plans::ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        declared_requirement_identity: terminal_psi::TerminalIdentityIndex,
        provider_plan: build_providers::plans::ProviderPlanDigest,
        mechanism: TerminalMechanismIdentity,
        origin: TerminalAuthorityClosureLeafOrigin,
        exercised: Option<TerminalAuthorityDisposition>,
        permitted: Option<TerminalAuthorityDisposition>,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        let leaf = Self {
            custody: TerminalAuthorityClosureCustody::SelectedRequirement {
                service_schema,
                requirement_identity,
                declared_requirement_identity,
                provider_plan,
            },
            mechanism,
            origin,
            exercised,
            permitted,
        };
        leaf.validate()?;
        Ok(leaf)
    }

    pub fn native_binding(
        service_schema: build_providers::plans::ServiceSchemaDigest,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
        declared_requirement_identity: terminal_psi::TerminalIdentityIndex,
        receipt: [u8; 32],
        mechanism: TerminalMechanismIdentity,
        origin: TerminalAuthorityClosureLeafOrigin,
        exercised: Option<TerminalAuthorityDisposition>,
        permitted: Option<TerminalAuthorityDisposition>,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        let leaf = Self {
            custody: TerminalAuthorityClosureCustody::NativeBinding {
                service_schema,
                requirement_identity,
                declared_requirement_identity,
                receipt,
            },
            mechanism,
            origin,
            exercised,
            permitted,
        };
        leaf.validate()?;
        Ok(leaf)
    }

    pub fn program_root(
        service_identity: terminal_psi::TerminalIdentityIndex,
        mechanism: TerminalMechanismIdentity,
        origin: TerminalAuthorityClosureLeafOrigin,
        exercised: TerminalAuthorityDisposition,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        let leaf = Self {
            custody: TerminalAuthorityClosureCustody::ProgramRoot { service_identity },
            mechanism,
            origin,
            exercised: Some(exercised),
            permitted: None,
        };
        leaf.validate()?;
        Ok(leaf)
    }

    fn validate(&self) -> Result<(), TerminalAuthorityClosureReviewBuildError> {
        if let TerminalAuthorityClosureLeafOrigin::CheckedBoundary { site, context, .. } =
            self.origin
            && (context == [0; 32]
                || matches!(site, semantic_vocabulary::InvocationSite::InvokeNoNormal(_))
                || !matches!(
                    self.mechanism,
                    TerminalMechanismIdentity::NormalizedForeign(foreign)
                        if matches!(foreign.argument_contract(),
                            NormalizedForeignArgumentContract::Checked(checked)
                                if !checked.is_zero())
                ))
        {
            return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
        }
        if matches!(
            self.mechanism,
            TerminalMechanismIdentity::CheckedPhysical(_)
        ) != matches!(
            self.origin,
            TerminalAuthorityClosureLeafOrigin::CheckedPhysical { .. }
        ) {
            return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
        }
        if matches!(
            self.origin,
            TerminalAuthorityClosureLeafOrigin::CheckedIntrinsic { .. }
        ) && !matches!(
            self.mechanism,
            TerminalMechanismIdentity::CompilerIntrinsic(_)
        ) {
            return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
        }
        if matches!(self.custody, TerminalAuthorityClosureCustody::SelectedRequirement { declared_requirement_identity, .. } | TerminalAuthorityClosureCustody::NativeBinding { declared_requirement_identity, .. } if declared_requirement_identity.is_absent())
            || self.subject_identity().is_absent()
        {
            return Err(TerminalAuthorityClosureReviewBuildError::EmptyRequirement);
        }
        if matches!(
            self.custody,
            TerminalAuthorityClosureCustody::ProgramRoot { .. }
        ) {
            if self.origin.checked_site().is_none() {
                return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
            }
            if self.exercised.is_none() {
                return Err(TerminalAuthorityClosureReviewBuildError::UnclassifiedAuthority);
            }
            if self.permitted.is_some() {
                return Err(TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent);
            }
        }
        if matches!(self.custody, TerminalAuthorityClosureCustody::NativeBinding { receipt, .. } if receipt == [0; 32])
        {
            return Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch);
        }
        if let Some(permitted) = &self.permitted {
            let exercised = self
                .exercised
                .as_ref()
                .ok_or(TerminalAuthorityClosureReviewBuildError::UnclassifiedAuthority)?;
            if let Some(class) = permitted.first_missing_class(exercised) {
                return Err(
                    TerminalAuthorityClosureReviewBuildError::ExercisedAuthorityNotPermitted {
                        class,
                    },
                );
            }
        }
        Ok(())
    }

    pub const fn custody(&self) -> TerminalAuthorityClosureCustody {
        self.custody
    }

    pub const fn subject_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        match self.custody {
            TerminalAuthorityClosureCustody::SelectedRequirement {
                requirement_identity,
                ..
            }
            | TerminalAuthorityClosureCustody::NativeBinding {
                requirement_identity,
                ..
            } => requirement_identity,
            TerminalAuthorityClosureCustody::ProgramRoot { service_identity } => service_identity,
        }
    }

    pub const fn service_schema(&self) -> Option<build_providers::plans::ServiceSchemaDigest> {
        match self.custody {
            TerminalAuthorityClosureCustody::SelectedRequirement { service_schema, .. }
            | TerminalAuthorityClosureCustody::NativeBinding { service_schema, .. } => {
                Some(service_schema)
            }
            TerminalAuthorityClosureCustody::ProgramRoot { .. } => None,
        }
    }

    pub const fn provider_plan(&self) -> Option<build_providers::plans::ProviderPlanDigest> {
        match self.custody {
            TerminalAuthorityClosureCustody::SelectedRequirement { provider_plan, .. } => {
                Some(provider_plan)
            }
            TerminalAuthorityClosureCustody::ProgramRoot { .. }
            | TerminalAuthorityClosureCustody::NativeBinding { .. } => None,
        }
    }

    pub const fn native_binding_receipt(&self) -> Option<[u8; 32]> {
        match self.custody {
            TerminalAuthorityClosureCustody::NativeBinding { receipt, .. } => Some(receipt),
            _ => None,
        }
    }

    pub const fn origin(&self) -> TerminalAuthorityClosureLeafOrigin {
        self.origin
    }

    pub const fn mechanism(&self) -> TerminalMechanismIdentity {
        self.mechanism
    }

    pub const fn exercised(&self) -> Option<&TerminalAuthorityDisposition> {
        self.exercised.as_ref()
    }

    pub const fn permitted(&self) -> Option<&TerminalAuthorityDisposition> {
        self.permitted.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityClosureReviewReceipt {
    terminal_artifact_identity: [u8; 32],
    target: target::NativeTarget,
    selected_provider_closure: build_providers::selections::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
    leaves: Vec<TerminalAuthorityClosureLeaf>,
    identities: terminal_psi::TerminalIdentityTable,
    identity: [u8; 32],
}

impl TerminalAuthorityClosureReviewReceipt {
    pub fn from_reviewed_leaves(
        terminal_artifact_identity: [u8; 32],
        target: target::NativeTarget,
        selected_provider_closure: build_providers::selections::SelectedProviderClosureDigest,
        physical_policy: TerminalAuthorityPolicyIdentity,
        permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
        mut leaves: Vec<TerminalAuthorityClosureLeaf>,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, TerminalAuthorityClosureReviewBuildError> {
        leaves.sort_by(|left, right| compare_closure_leaves(left, right, identities));
        if leaves
            .windows(2)
            .any(|rows| same_closure_leaf_key(&rows[0], &rows[1]))
        {
            return Err(TerminalAuthorityClosureReviewBuildError::DuplicateRequirementLeaf);
        }
        if leaves
            .iter()
            .any(|leaf| leaf.permitted.is_some() != permission_policy.is_some())
        {
            return Err(TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent);
        }
        for leaf in &leaves {
            leaf.validate()?;
            if identities
                .text(leaf.subject_identity())
                .is_none_or(str::is_empty)
                || matches!(leaf.custody, TerminalAuthorityClosureCustody::SelectedRequirement { declared_requirement_identity, .. } | TerminalAuthorityClosureCustody::NativeBinding { declared_requirement_identity, .. } if identities.text(declared_requirement_identity).is_none_or(str::is_empty))
            {
                return Err(TerminalAuthorityClosureReviewBuildError::EmptyRequirement);
            }
        }
        let identity = terminal_authority_closure_review_identity(
            terminal_artifact_identity,
            target,
            selected_provider_closure,
            physical_policy,
            permission_policy,
            &leaves,
            identities,
        );
        Ok(Self {
            terminal_artifact_identity,
            target,
            selected_provider_closure,
            physical_policy,
            permission_policy,
            leaves,
            identities: identities.clone(),
            identity,
        })
    }

    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }

    pub const fn terminal_artifact_identity(&self) -> [u8; 32] {
        self.terminal_artifact_identity
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    pub const fn selected_provider_closure(
        &self,
    ) -> build_providers::selections::SelectedProviderClosureDigest {
        self.selected_provider_closure
    }

    pub const fn physical_policy(&self) -> TerminalAuthorityPolicyIdentity {
        self.physical_policy
    }

    pub const fn permission_policy(&self) -> Option<TerminalAuthorityPermissionPolicyIdentity> {
        self.permission_policy
    }

    pub fn leaves(&self) -> &[TerminalAuthorityClosureLeaf] {
        &self.leaves
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAuthorityClosureReviewBuildError {
    MechanismOriginMismatch,
    EmptyRequirement,
    DuplicateRequirementLeaf,
    AdmissionAxisInconsistent,
    UnclassifiedAuthority,
    ExercisedAuthorityNotPermitted { class: TerminalAuthorityClass },
}

fn compare_closure_leaves(
    left: &TerminalAuthorityClosureLeaf,
    right: &TerminalAuthorityClosureLeaf,
    identities: &terminal_psi::TerminalIdentityTable,
) -> std::cmp::Ordering {
    if matches!(
        left.custody,
        TerminalAuthorityClosureCustody::ProgramRoot { .. }
    ) && matches!(
        right.custody,
        TerminalAuthorityClosureCustody::ProgramRoot { .. }
    ) {
        return left
            .origin
            .checked_site()
            .cmp(&right.origin.checked_site())
            .then_with(|| {
                identities
                    .text(left.subject_identity())
                    .cmp(&identities.text(right.subject_identity()))
            })
            .then_with(|| {
                terminal_mechanism_identity_bytes(left.mechanism)
                    .cmp(&terminal_mechanism_identity_bytes(right.mechanism))
            });
    }
    left.service_schema()
        .map(|schema| *schema.as_bytes())
        .cmp(&right.service_schema().map(|schema| *schema.as_bytes()))
        .then_with(|| {
            identities
                .text(left.subject_identity())
                .cmp(&identities.text(right.subject_identity()))
        })
        .then_with(|| {
            left.provider_plan()
                .map(|plan| *plan.as_bytes())
                .cmp(&right.provider_plan().map(|plan| *plan.as_bytes()))
        })
        .then_with(|| {
            left.native_binding_receipt()
                .cmp(&right.native_binding_receipt())
        })
        .then_with(|| left.origin.cmp(&right.origin))
        .then_with(|| {
            terminal_mechanism_identity_bytes(left.mechanism)
                .cmp(&terminal_mechanism_identity_bytes(right.mechanism))
        })
}

fn same_closure_leaf_key(
    left: &TerminalAuthorityClosureLeaf,
    right: &TerminalAuthorityClosureLeaf,
) -> bool {
    match (left.custody, right.custody) {
        (
            TerminalAuthorityClosureCustody::ProgramRoot { .. },
            TerminalAuthorityClosureCustody::ProgramRoot { .. },
        ) => left.origin.checked_site() == right.origin.checked_site(),
        (
            TerminalAuthorityClosureCustody::SelectedRequirement {
                service_schema: left_schema,
                requirement_identity: left_requirement,
                provider_plan: left_plan,
                ..
            },
            TerminalAuthorityClosureCustody::SelectedRequirement {
                service_schema: right_schema,
                requirement_identity: right_requirement,
                provider_plan: right_plan,
                ..
            },
        ) => {
            left_schema == right_schema
                && left_requirement == right_requirement
                && left_plan == right_plan
                && left.origin == right.origin
        }
        (
            TerminalAuthorityClosureCustody::NativeBinding {
                service_schema: left_schema,
                requirement_identity: left_requirement,
                receipt: left_receipt,
                ..
            },
            TerminalAuthorityClosureCustody::NativeBinding {
                service_schema: right_schema,
                requirement_identity: right_requirement,
                receipt: right_receipt,
                ..
            },
        ) => {
            left_schema == right_schema
                && left_requirement == right_requirement
                && left_receipt == right_receipt
                && left.origin == right.origin
        }
        _ => false,
    }
}

fn terminal_authority_closure_review_identity(
    terminal_artifact_identity: [u8; 32],
    target: target::NativeTarget,
    selected_provider_closure: build_providers::selections::SelectedProviderClosureDigest,
    physical_policy: TerminalAuthorityPolicyIdentity,
    permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
    leaves: &[TerminalAuthorityClosureLeaf],
    identities: &terminal_psi::TerminalIdentityTable,
) -> [u8; 32] {
    let mut digest = wire::digest_encoder::DomainDigestEncoder::new(
        b"omega.terminal-authority.closure-review.v6\0",
    );
    digest.raw(&terminal_artifact_identity);
    digest.byte(match target.architecture {
        target::Architecture::Aarch64 => 0,
        target::Architecture::X86_64 => 1,
    });
    digest.byte(match target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
    });
    digest.raw(&(target.pointer_size as u64).to_be_bytes());
    digest.raw(&(target.pointer_alignment as u64).to_be_bytes());
    digest.raw(selected_provider_closure.as_bytes());
    digest.raw(&physical_policy.version().to_be_bytes());
    digest.raw(&physical_policy.commitment());
    match permission_policy {
        None => digest.byte(0),
        Some(permission_policy) => {
            digest.byte(1);
            digest.raw(&permission_policy.version().to_be_bytes());
            digest.raw(&permission_policy.commitment());
        }
    }
    digest.raw(&(leaves.len() as u64).to_be_bytes());
    for leaf in leaves {
        match leaf.custody {
            TerminalAuthorityClosureCustody::SelectedRequirement {
                service_schema,
                declared_requirement_identity,
                ..
            } => {
                digest.byte(0);
                digest.raw(service_schema.as_bytes());
                let declared = identities
                    .text(declared_requirement_identity)
                    .unwrap_or_default();
                digest.raw(&(declared.len() as u64).to_be_bytes());
                digest.raw(declared.as_bytes());
            }
            TerminalAuthorityClosureCustody::NativeBinding {
                service_schema,
                declared_requirement_identity,
                receipt,
                ..
            } => {
                digest.byte(2);
                digest.raw(service_schema.as_bytes());
                let declared = identities
                    .text(declared_requirement_identity)
                    .unwrap_or_default();
                digest.raw(&(declared.len() as u64).to_be_bytes());
                digest.raw(declared.as_bytes());
                digest.raw(&receipt);
            }
            TerminalAuthorityClosureCustody::ProgramRoot { .. } => digest.byte(1),
        }
        let requirement = identities.text(leaf.subject_identity()).unwrap_or_default();
        digest.raw(&(requirement.len() as u64).to_be_bytes());
        digest.raw(requirement.as_bytes());
        if let Some(provider_plan) = leaf.provider_plan() {
            digest.raw(provider_plan.as_bytes());
        }
        match leaf.origin {
            TerminalAuthorityClosureLeafOrigin::SelectedBoundary => digest.byte(0),
            TerminalAuthorityClosureLeafOrigin::CheckedBoundary {
                machine,
                site,
                context,
            } => {
                digest.byte(3);
                digest.raw(&machine.get().to_be_bytes());
                match site {
                    semantic_vocabulary::InvocationSite::Operation(operation) => {
                        digest.byte(0);
                        digest.raw(&operation.get().to_be_bytes());
                    }
                    semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                        digest.byte(1);
                        digest.raw(&edge.get().to_be_bytes());
                    }
                    semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                        digest.byte(2);
                        digest.raw(&edge.get().to_be_bytes());
                    }
                    semantic_vocabulary::InvocationSite::Cleanup {
                        edge,
                        action_ordinal,
                    } => {
                        digest.byte(3);
                        digest.raw(&edge.get().to_be_bytes());
                        digest.raw(&action_ordinal.to_be_bytes());
                    }
                }
                digest.raw(&context);
            }
            TerminalAuthorityClosureLeafOrigin::CheckedPhysical { machine, operation } => {
                digest.byte(1);
                digest.raw(&machine.get().to_be_bytes());
                digest.raw(&operation.get().to_be_bytes());
            }
            TerminalAuthorityClosureLeafOrigin::CheckedIntrinsic { machine, operation } => {
                digest.byte(2);
                digest.raw(&machine.get().to_be_bytes());
                digest.raw(&operation.get().to_be_bytes());
            }
        }
        let mechanism = terminal_mechanism_identity_bytes(leaf.mechanism);
        digest.raw(&(mechanism.len() as u64).to_be_bytes());
        digest.raw(&mechanism);
        match &leaf.exercised {
            None => digest.byte(0),
            Some(exercised) => {
                digest.byte(1);
                encode_authority_classes(&mut digest, exercised);
            }
        }
        match &leaf.permitted {
            None => digest.byte(0),
            Some(permitted) => {
                digest.byte(1);
                encode_authority_classes(&mut digest, permitted);
            }
        }
    }
    digest.finish()
}

fn encode_authority_classes(
    digest: &mut wire::digest_encoder::DomainDigestEncoder,
    disposition: &TerminalAuthorityDisposition,
) {
    digest.raw(&(disposition.classes().len() as u64).to_be_bytes());
    for class in disposition.classes() {
        digest.byte(class.canonical_tag());
    }
}

#[cfg(test)]
mod physical_leaf_tests {
    use super::*;

    #[test]
    fn physical_classification_and_receiver_permission_are_independent() {
        let mut identities = terminal_psi::TerminalIdentityTable::default().into_builder();
        let requirement = identities.intern("PortSequence::emit");
        let identities = identities.build();
        let profile = target::TargetProfile::LinuxX64;
        let port_io = TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::PortIo]);
        let leaf = |exercised, permitted| {
            TerminalAuthorityClosureLeaf::new(
                build_providers::plans::ServiceSchemaDigest::from_digest([1; 32]),
                requirement,
                requirement,
                build_providers::plans::ProviderPlanDigest::from_digest([2; 32]),
                CheckedPhysicalTerminalMechanismIdentity::port_write(profile, 0x3f8).into(),
                TerminalAuthorityClosureLeafOrigin::CheckedPhysical {
                    machine: semantic_vocabulary::MachineId::new(1).unwrap(),
                    operation: semantic_vocabulary::OperationId::new(2).unwrap(),
                },
                exercised,
                permitted,
            )
        };
        let review = |leaf, permission| {
            TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
                [3; 32],
                profile.native_target(),
                build_providers::selections::SelectedProviderPlanFacts::default().identity_digest(),
                TerminalAuthorityPolicyIdentity::from_parts(1, [5; 32]),
                permission,
                vec![leaf],
                &identities,
            )
        };
        let classified = leaf(Some(port_io.clone()), None).unwrap();
        let granted = leaf(Some(port_io.clone()), Some(port_io.clone())).unwrap();
        let policy = Some(TerminalAuthorityPermissionPolicyIdentity::from_parts(
            1, [6; 32],
        ));
        assert_ne!(
            review(classified.clone(), None).unwrap().identity(),
            review(granted.clone(), policy).unwrap().identity(),
        );
        assert_eq!(
            review(classified, policy),
            Err(TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent)
        );
        assert_eq!(
            review(granted, None),
            Err(TerminalAuthorityClosureReviewBuildError::AdmissionAxisInconsistent)
        );
        assert_eq!(
            leaf(None, Some(port_io.clone())),
            Err(TerminalAuthorityClosureReviewBuildError::UnclassifiedAuthority)
        );
        assert_eq!(
            leaf(
                Some(port_io),
                Some(TerminalAuthorityDisposition::from_classes([]))
            ),
            Err(
                TerminalAuthorityClosureReviewBuildError::ExercisedAuthorityNotPermitted {
                    class: TerminalAuthorityClass::PortIo
                }
            ),
        );
    }

    #[test]
    fn physical_sites_distinguish_events_without_allowing_mechanism_substitution() {
        let mut identities = terminal_psi::TerminalIdentityTable::default().into_builder();
        let requirement = identities.intern("PortSequence::emit");
        let identities = identities.build();
        let machine = semantic_vocabulary::MachineId::new(1).unwrap();
        let operation = semantic_vocabulary::OperationId::new(2).unwrap();
        let schema = build_providers::plans::ServiceSchemaDigest::from_digest([1; 32]);
        let plan = build_providers::plans::ProviderPlanDigest::from_digest([2; 32]);
        let profile = target::TargetProfile::LinuxX64;
        let leaf = |operation, port| {
            TerminalAuthorityClosureLeaf::new(
                schema,
                requirement,
                requirement,
                plan,
                CheckedPhysicalTerminalMechanismIdentity::port_write(profile, port).into(),
                TerminalAuthorityClosureLeafOrigin::CheckedPhysical { machine, operation },
                None,
                None,
            )
            .unwrap()
        };
        let receipt = |leaves| {
            TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
                [3; 32],
                profile.native_target(),
                build_providers::selections::SelectedProviderPlanFacts::default().identity_digest(),
                TerminalAuthorityPolicyIdentity::from_parts(1, [5; 32]),
                None,
                leaves,
                &identities,
            )
        };
        let first = leaf(operation, 0x3f8);
        let next_operation = semantic_vocabulary::OperationId::new(3).unwrap();
        let second = leaf(next_operation, 0x2f8);
        let ordered = receipt(vec![first.clone(), second.clone()]).unwrap();
        // The published v5 digest binds the sorted leaves to this selected closure.
        assert_eq!(
            ordered.identity(),
            [
                0xab, 0xc7, 0x10, 0x11, 0x85, 0xa4, 0x39, 0xe8, 0xb7, 0xd5, 0xef, 0x6c, 0xc3, 0xab,
                0x45, 0xec, 0xf8, 0x7f, 0xe1, 0x61, 0xf1, 0xe1, 0x2d, 0x77, 0x4e, 0x82, 0x3d, 0x25,
                0x6e, 0x0a, 0xaf, 0x6f
            ]
        );
        assert_eq!(
            receipt(vec![]).unwrap().identity(),
            [
                0x66, 0xfd, 0x0b, 0xeb, 0xf1, 0xaf, 0x9a, 0xc2, 0x43, 0x2f, 0x91, 0x5f, 0xd0, 0x82,
                0x91, 0x3e, 0x0e, 0xec, 0x57, 0xdc, 0x18, 0x46, 0x15, 0x46, 0xd7, 0x2f, 0xd7, 0xdd,
                0x21, 0x78, 0x65, 0xe9
            ]
        );
        assert_eq!(ordered, receipt(vec![second, first.clone()]).unwrap());
        assert_ne!(
            ordered.identity(),
            receipt(vec![first.clone()]).unwrap().identity()
        );
        assert_eq!(
            receipt(vec![first, leaf(operation, 0x2f8)]),
            Err(TerminalAuthorityClosureReviewBuildError::DuplicateRequirementLeaf)
        );
        assert_eq!(
            TerminalAuthorityClosureLeaf::new(
                schema,
                requirement,
                requirement,
                plan,
                CheckedPhysicalTerminalMechanismIdentity::port_write(profile, 0x3f8).into(),
                TerminalAuthorityClosureLeafOrigin::SelectedBoundary,
                None,
                None,
            ),
            Err(TerminalAuthorityClosureReviewBuildError::MechanismOriginMismatch)
        );
    }
}
