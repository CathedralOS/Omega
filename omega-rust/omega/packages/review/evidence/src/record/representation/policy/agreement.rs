//! Rejoin one consumer's actual by-value opaque demands to the exact rows of
//! independently compiled producer policies.
//!
//! A consumer compiles against producer source, so its own projection always
//! agrees with the source it saw. Agreement across independently compiled
//! artifacts is only established when every foreign opaque, conformance, and
//! carrier the consumer actually crosses rejoins the producer's own projected
//! declaration and public availability rows. Equal shapes, sizes, or compact
//! fingerprints never substitute for these exact nominal rows.

use super::PackagePolicyRepresentation;
use crate::record::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
use semantic_vocabulary::PackageKeyIdentity;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyRepresentationAgreementError {
    /// A foreign nominal owner has no independently compiled producer policy.
    ProducerPolicyAbsent {
        consumer: PackageKeyIdentity,
        producer: PackageKeyIdentity,
        opaque: PackageReviewNominalIdentity,
    },
    /// The producer policy does not declare the opaque the consumer crosses.
    OpaqueUndeclared {
        consumer: PackageKeyIdentity,
        producer: PackageKeyIdentity,
        opaque: PackageReviewNominalIdentity,
    },
    /// The producer policy publishes no availability row with exactly this
    /// opaque, named conformance, and carrier.
    AvailabilityUnpublished {
        consumer: PackageKeyIdentity,
        producer: PackageKeyIdentity,
        opaque: PackageReviewNominalIdentity,
        conformance: PackageReviewNominalIdentity,
        carrier: PackageReviewNominalIdentity,
    },
    /// The producer policy was projected for a different target than the consumer.
    TargetMismatch {
        consumer: PackageKeyIdentity,
        producer: PackageKeyIdentity,
    },
}

impl PackagePolicyRepresentation {
    /// `producers` resolves a foreign package identity to its independently
    /// compiled policy; `None` means no such review exists in the closure.
    pub fn rejoin_foreign_demands<'a>(
        &self,
        mut producers: impl FnMut(PackageKeyIdentity) -> Option<&'a PackagePolicyRepresentation>,
    ) -> Result<(), PackagePolicyRepresentationAgreementError> {
        for demand in &self.demands {
            for use_ in demand.calling.opaque_uses() {
                let opaque = use_.opaque();
                let conformance = use_.application().declaration();
                let carrier = use_.carrier();

                if let PackageReviewNominalOwner::Package(producer) = opaque.owner()
                    && producer != self.package
                {
                    let producer_policy = resolve_producer(self, &mut producers, producer, opaque)?;
                    if !producer_policy.declarations.contains(opaque) {
                        return Err(
                            PackagePolicyRepresentationAgreementError::OpaqueUndeclared {
                                consumer: self.package,
                                producer,
                                opaque: opaque.clone(),
                            },
                        );
                    }
                }

                if let PackageReviewNominalOwner::Package(producer) = conformance.owner()
                    && producer != self.package
                {
                    let producer_policy = resolve_producer(self, &mut producers, producer, opaque)?;
                    if !producer_policy.producer_availability.iter().any(|row| {
                        row.opaque == *opaque
                            && row.conformance.identity() == conformance
                            && row.carrier == *carrier
                    }) {
                        return Err(
                            PackagePolicyRepresentationAgreementError::AvailabilityUnpublished {
                                consumer: self.package,
                                producer,
                                opaque: opaque.clone(),
                                conformance: conformance.clone(),
                                carrier: carrier.clone(),
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

fn resolve_producer<'a>(
    consumer: &PackagePolicyRepresentation,
    producers: &mut impl FnMut(PackageKeyIdentity) -> Option<&'a PackagePolicyRepresentation>,
    producer: PackageKeyIdentity,
    opaque: &PackageReviewNominalIdentity,
) -> Result<&'a PackagePolicyRepresentation, PackagePolicyRepresentationAgreementError> {
    let Some(policy) = producers(producer) else {
        return Err(
            PackagePolicyRepresentationAgreementError::ProducerPolicyAbsent {
                consumer: consumer.package,
                producer,
                opaque: opaque.clone(),
            },
        );
    };
    if policy.target != consumer.target {
        return Err(PackagePolicyRepresentationAgreementError::TargetMismatch {
            consumer: consumer.package,
            producer,
        });
    }
    Ok(policy)
}

impl fmt::Display for PackagePolicyRepresentationAgreementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProducerPolicyAbsent {
                producer, opaque, ..
            } => write!(
                formatter,
                "opaque `{}` requires independently compiled producer policy `{:?}`, but none was available",
                opaque.path(),
                producer
            ),
            Self::OpaqueUndeclared {
                producer, opaque, ..
            } => write!(
                formatter,
                "producer policy `{:?}` does not declare opaque `{}`",
                producer,
                opaque.path()
            ),
            Self::AvailabilityUnpublished {
                producer,
                opaque,
                conformance,
                carrier,
                ..
            } => write!(
                formatter,
                "producer policy `{:?}` publishes no availability for opaque `{}`, conformance `{}`, and carrier `{}`",
                producer,
                opaque.path(),
                conformance.path(),
                carrier.path()
            ),
            Self::TargetMismatch {
                consumer, producer, ..
            } => write!(
                formatter,
                "consumer package `{:?}` and producer package `{:?}` use different representation targets",
                consumer, producer
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{
        PackagePolicyCallbacks, PackagePolicyCallingOpaqueUse, PackagePolicyCallingPlan,
        PackagePolicyClosedConformanceApplication, PackagePolicyConformanceShape,
        PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
        PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
        PackagePolicyPreemption, PackagePolicyRepresentation,
        PackagePolicyRepresentationAvailability, PackagePolicyRepresentationDemand,
        PackagePolicyStatePlan, PackageReviewBoundaryCallingPolicy, PackageReviewEvidenceInterface,
        PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
        PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
    };

    fn package(byte: u8) -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([byte; 32]).unwrap()
    }

    fn nominal(owner: PackageReviewNominalOwner, path: &str) -> PackageReviewNominalIdentity {
        PackageReviewNominalIdentity {
            owner,
            path: path.to_owned(),
        }
    }

    fn target(
        profile: PackageReviewRepresentationTargetProfile,
    ) -> PackageReviewRepresentationTarget {
        PackageReviewRepresentationTarget {
            profile,
            architecture: PackageReviewRepresentationArchitecture::X86_64,
            object_format: PackageReviewRepresentationObjectFormat::Coff,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }

    fn opaque_use(
        opaque: PackageReviewNominalIdentity,
        conformance: PackageReviewNominalIdentity,
        carrier: PackageReviewNominalIdentity,
    ) -> PackagePolicyCallingOpaqueUse {
        PackagePolicyCallingOpaqueUse {
            opaque,
            carrier,
            selection_owner: PackageReviewNominalOwner::Package(package(1)),
            application: PackagePolicyClosedConformanceApplication {
                declaration: conformance,
                lifetime_arguments: Vec::new(),
                type_arguments: Vec::new(),
                const_arguments: Vec::new(),
                machine_arguments: Vec::new(),
                subject: None,
                trait_identity: nominal(
                    PackageReviewNominalOwner::ToolchainSource(
                        crate::record::PackageReviewToolchainSourceIdentity { digest: [7; 32] },
                    ),
                    "OpaqueRepresentation",
                ),
                trait_lifetime_arguments: Vec::new(),
                trait_arguments: Vec::new(),
                rows: Vec::new(),
            },
            origin:
                crate::record::PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance,
            lifecycle: crate::record::PackageReviewOpaqueRepresentationLifecycleDisposition::Inert,
            copy_disposition:
                crate::record::PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly,
            occurrences: Vec::new(),
        }
    }

    fn policy(
        consumer: PackageKeyIdentity,
        target: PackageReviewRepresentationTarget,
        opaque_owner: PackageReviewNominalOwner,
        conformance_owner: PackageReviewNominalOwner,
        availability: bool,
        declaration: bool,
    ) -> PackagePolicyRepresentation {
        let opaque = nominal(opaque_owner, "Token");
        let conformance = nominal(conformance_owner, "TokenRepresentation");
        let carrier = nominal(conformance_owner, "Carrier");
        let use_ = opaque_use(opaque.clone(), conformance.clone(), carrier.clone());
        let demand_opaque = use_.opaque().clone();
        let state = PackagePolicyStatePlan {
            initial_regime: PackagePolicyMachineRegime::X86Long64,
            interrupted_state: PackagePolicyMachineStateSet::default(),
            saved_state: PackagePolicyMachineStateSet::default(),
            restored_state: PackagePolicyMachineStateSet::default(),
            permitted_transitive_use: PackagePolicyMachineStateSet::default(),
            stack: PackagePolicyEntryStack::Dedicated { class: 0 },
            preemption: PackagePolicyPreemption::NotApplicable,
        };
        let calling = PackagePolicyCallingPlan {
            boundary_trait: nominal(
                PackageReviewNominalOwner::ToolchainSource(
                    crate::record::PackageReviewToolchainSourceIdentity { digest: [8; 32] },
                ),
                "Calling",
            ),
            boundary_arguments: Vec::new(),
            boundary_lifetime_parameter_count: 0,
            requirement: nominal(
                PackageReviewNominalOwner::ToolchainSource(
                    crate::record::PackageReviewToolchainSourceIdentity { digest: [9; 32] },
                ),
                "Calling::call",
            ),
            requirement_trait: nominal(
                PackageReviewNominalOwner::ToolchainSource(
                    crate::record::PackageReviewToolchainSourceIdentity { digest: [8; 32] },
                ),
                "Calling",
            ),
            requirement_arguments: Vec::new(),
            requirement_lifetime_arguments: Vec::new(),
            requirement_lifetime_parameter_count: 0,
            static_parameters: Vec::new(),
            target,
            shape_graph: crate::record::PackageReviewBoundaryShapeGraph {
                shapes: Vec::new(),
                fields: Vec::new(),
                parameters: Vec::new(),
                result: None,
            },
            semantic_parameters: Vec::new(),
            semantic_result: None,
            native_parameters: Vec::new(),
            callbacks: PackagePolicyCallbacks {
                binders: Vec::new(),
                demands: Vec::new(),
                materializations: Vec::new(),
                layouts: Vec::new(),
            },
            opaque_uses: vec![use_],
            physical: PackagePolicyPhysicalCallingContract {
                policy: PackageReviewBoundaryCallingPolicy::MicrosoftX64,
                parameters: Vec::new(),
                result: None,
                ordinary_clobbers: Vec::new(),
                stack_alignment: 16,
                shadow_bytes: 32,
                entry_control: PackagePolicyEntryControl::CallReturn,
                state,
            },
        };
        PackagePolicyRepresentation {
            package: consumer,
            target,
            declarations: declaration.then_some(opaque.clone()).into_iter().collect(),
            producer_availability: availability
                .then_some(PackagePolicyRepresentationAvailability {
                    opaque,
                    conformance: PackagePolicyConformanceShape {
                        identity: conformance,
                        lifetime_parameter_count: 0,
                        type_parameters: Vec::new(),
                        subject: crate::record::PackageReviewConformanceSubject::Nominal(
                            carrier.clone(),
                        ),
                        interface: PackageReviewEvidenceInterface {
                            trait_identity: nominal(
                                PackageReviewNominalOwner::ToolchainSource(
                                    crate::record::PackageReviewToolchainSourceIdentity {
                                        digest: [7; 32],
                                    },
                                ),
                                "OpaqueRepresentation",
                            ),
                            lifetime_arguments: Vec::new(),
                            arguments: Vec::new(),
                            requirements: Vec::new(),
                        },
                    },
                    carrier,
                })
                .into_iter()
                .collect(),
            selected_availability: Vec::new(),
            demands: vec![PackagePolicyRepresentationDemand {
                opaque: demand_opaque,
                calling,
            }],
        }
    }

    #[test]
    fn joins_exact_foreign_declaration_and_availability() {
        let consumer_id = package(1);
        let producer_id = package(2);
        let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
        let consumer = policy(
            consumer_id,
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            false,
            false,
        );
        let producer = policy(
            producer_id,
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            true,
            true,
        );
        assert!(
            consumer
                .rejoin_foreign_demands(|identity| (identity == producer_id).then_some(&producer))
                .is_ok()
        );
    }

    #[test]
    fn rejects_absent_producer_policy() {
        let producer_id = package(2);
        let consumer = policy(
            package(1),
            target(PackageReviewRepresentationTargetProfile::WindowsX64),
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            false,
            false,
        );
        assert!(matches!(
            consumer.rejoin_foreign_demands(|_| None),
            Err(PackagePolicyRepresentationAgreementError::ProducerPolicyAbsent { .. })
        ));
    }

    #[test]
    fn rejects_undeclared_opaque() {
        let producer_id = package(2);
        let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
        let consumer = policy(
            package(1),
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            false,
            false,
        );
        let producer = policy(
            producer_id,
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            false,
            false,
        );
        assert!(matches!(
            consumer.rejoin_foreign_demands(|_| Some(&producer)),
            Err(PackagePolicyRepresentationAgreementError::OpaqueUndeclared { .. })
        ));
    }

    #[test]
    fn rejects_unpublished_availability() {
        let producer_id = package(2);
        let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
        let consumer = policy(
            package(1),
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            false,
            false,
        );
        let producer = policy(
            producer_id,
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            false,
            true,
        );
        assert!(matches!(
            consumer.rejoin_foreign_demands(|_| Some(&producer)),
            Err(PackagePolicyRepresentationAgreementError::AvailabilityUnpublished { .. })
        ));
    }

    #[test]
    fn rejects_target_mismatch() {
        let producer_id = package(2);
        let consumer = policy(
            package(1),
            target(PackageReviewRepresentationTargetProfile::WindowsX64),
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            false,
            false,
        );
        let producer = policy(
            producer_id,
            target(PackageReviewRepresentationTargetProfile::LinuxX64),
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            true,
            true,
        );
        assert!(matches!(
            consumer.rejoin_foreign_demands(|_| Some(&producer)),
            Err(PackagePolicyRepresentationAgreementError::TargetMismatch { .. })
        ));
    }

    #[test]
    fn local_owner_needs_no_producer_policy() {
        let consumer_id = package(1);
        let target = target(PackageReviewRepresentationTargetProfile::WindowsX64);
        let consumer = policy(
            consumer_id,
            target,
            PackageReviewNominalOwner::Package(consumer_id),
            PackageReviewNominalOwner::Package(consumer_id),
            false,
            true,
        );
        assert!(consumer.rejoin_foreign_demands(|_| None).is_ok());
    }
}
