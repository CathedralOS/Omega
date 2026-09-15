//! Rejoin one consumer's actual by-value opaque demands to the exact rows and
//! immutable source instance of independently compiled producer policies.
//!
//! A consumer compiles against producer source, so its own projection always
//! agrees with the source it saw. Agreement across independently compiled
//! artifacts is only established when every foreign opaque, conformance, and
//! carrier the consumer actually crosses rejoins the producer's own projected
//! declaration and public availability rows, when the producer review is bound
//! to the immutable source instance the consumer compiled against, and when a
//! producer's own selected application for the crossed opaque equals the
//! consumer's actual selected application. Equal shapes, sizes, or compact
//! fingerprints never substitute for these exact nominal rows.

use super::PackagePolicyRepresentation;
use crate::record::{
    PackagePolicyCallingOpaqueUse, PackageReviewNominalIdentity, PackageReviewNominalOwner,
};
use package_compilation::PackageSourceConsumptionCommitment;
use semantic_vocabulary::PackageKeyIdentity;
use std::fmt;

/// The producer-side evidence one actual foreign demand must rejoin: the
/// producer's own projected policy bound to an immutable source instance.
///
/// `expected_source_instance` is the immutable source instance the consumer
/// compiled against for this producer — its retained dependency bundle's
/// source-consumption commitment. `reviewed_source_instance` is the immutable
/// source instance this producer review actually consumed — the review's own
/// source-consumption commitment. Both must be equal before any of the
/// producer's canonical rows may agree with the consumer's demand.
#[derive(Debug, Clone, Copy)]
pub struct PackagePolicyRepresentationProducerInstance<'a> {
    /// The independently compiled producer's own projected policy rows.
    pub policy: &'a PackagePolicyRepresentation,
    /// The immutable source instance the consumer compiled against for this
    /// producer.
    pub expected_source_instance: PackageSourceConsumptionCommitment,
    /// The immutable source instance this producer review consumed.
    pub reviewed_source_instance: PackageSourceConsumptionCommitment,
}

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
    /// The producer review consumed a different immutable source instance
    /// than the one the consumer compiled against for it.
    ProducerSourceInstanceMismatch {
        consumer: PackageKeyIdentity,
        producer: PackageKeyIdentity,
        opaque: PackageReviewNominalIdentity,
    },
    /// The producer's own selected application for the crossed opaque does not
    /// equal the consumer's actual selected application at the exchange.
    SelectedApplicationMismatch {
        consumer: PackageKeyIdentity,
        producer: PackageKeyIdentity,
        opaque: PackageReviewNominalIdentity,
    },
    /// The producer policy was projected for a different target than the consumer.
    TargetMismatch {
        consumer: PackageKeyIdentity,
        producer: PackageKeyIdentity,
    },
}

impl PackagePolicyRepresentation {
    /// `producers` resolves a foreign package identity to its independently
    /// compiled producer instance: the producer's own projected policy bound
    /// to the immutable source instance it reviewed, alongside the immutable
    /// source instance the consumer compiled against for it. `None` means no
    /// such review exists in the closure.
    pub fn rejoin_foreign_demands<'a>(
        &self,
        mut producers: impl FnMut(
            PackageKeyIdentity,
        ) -> Option<PackagePolicyRepresentationProducerInstance<'a>>,
    ) -> Result<(), PackagePolicyRepresentationAgreementError> {
        for demand in &self.demands {
            for use_ in demand.calling.opaque_uses() {
                let opaque = use_.opaque();
                let conformance = use_.application().declaration();
                let carrier = use_.carrier();

                if let PackageReviewNominalOwner::Package(producer) = opaque.owner()
                    && producer != self.package
                {
                    let producer_policy = resolve_producer(self, &mut producers, producer, use_)?;
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
                    let producer_policy = resolve_producer(self, &mut producers, producer, use_)?;
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
    producers: &mut impl FnMut(
        PackageKeyIdentity,
    ) -> Option<PackagePolicyRepresentationProducerInstance<'a>>,
    producer: PackageKeyIdentity,
    use_: &PackagePolicyCallingOpaqueUse,
) -> Result<&'a PackagePolicyRepresentation, PackagePolicyRepresentationAgreementError> {
    let opaque = use_.opaque();
    let Some(instance) = producers(producer) else {
        return Err(
            PackagePolicyRepresentationAgreementError::ProducerPolicyAbsent {
                consumer: consumer.package,
                producer,
                opaque: opaque.clone(),
            },
        );
    };
    let policy = instance.policy;
    // A resolved policy owned by another package is not the producer's own
    // instance policy, so the demanded producer's policy remains absent.
    if policy.package != producer {
        return Err(
            PackagePolicyRepresentationAgreementError::ProducerPolicyAbsent {
                consumer: consumer.package,
                producer,
                opaque: opaque.clone(),
            },
        );
    }
    if instance.reviewed_source_instance != instance.expected_source_instance {
        return Err(
            PackagePolicyRepresentationAgreementError::ProducerSourceInstanceMismatch {
                consumer: consumer.package,
                producer,
                opaque: opaque.clone(),
            },
        );
    }
    if policy.target != consumer.target {
        return Err(PackagePolicyRepresentationAgreementError::TargetMismatch {
            consumer: consumer.package,
            producer,
        });
    }
    // Strong selected-application equality at the actual exchange: when the
    // producer's own activation selected an application for this opaque, the
    // consumer's actual application must equal it. The selection owner is
    // custody, not application identity, so it is excluded.
    if let Some(selected) = policy
        .selected_availability
        .iter()
        .find(|selected| selected.opaque == *opaque)
        && (selected.application != *use_.application()
            || selected.carrier != *use_.carrier()
            || selected.lifecycle != use_.lifecycle()
            || selected.copy_disposition != use_.copy_disposition()
            || selected.origin != use_.origin())
    {
        return Err(
            PackagePolicyRepresentationAgreementError::SelectedApplicationMismatch {
                consumer: consumer.package,
                producer,
                opaque: opaque.clone(),
            },
        );
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
            Self::ProducerSourceInstanceMismatch {
                producer, opaque, ..
            } => write!(
                formatter,
                "producer policy `{:?}` for opaque `{}` was reviewed from a different immutable source instance than the consumer compiled against",
                producer,
                opaque.path()
            ),
            Self::SelectedApplicationMismatch {
                producer, opaque, ..
            } => write!(
                formatter,
                "producer policy `{:?}` selected a different application for opaque `{}` than the consumer's actual application at this exchange",
                producer,
                opaque.path()
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
    use super::{PackageKeyIdentity, PackageSourceConsumptionCommitment};
    use crate::record::PackagePolicyRepresentationAgreementError;
    use crate::record::PackagePolicyRepresentationProducerInstance;
    use crate::record::PackageReviewNominalIdentity;
    use crate::record::PackageReviewNominalOwner;
    use crate::record::{
        PackagePolicyCallbacks, PackagePolicyCallingOpaqueUse, PackagePolicyCallingPlan,
        PackagePolicyClosedConformanceApplication, PackagePolicyConformanceShape,
        PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
        PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
        PackagePolicyPreemption, PackagePolicyRepresentation,
        PackagePolicyRepresentationAvailability, PackagePolicyRepresentationDemand,
        PackagePolicyRepresentationSelection, PackagePolicyStatePlan,
        PackageReviewBoundaryCallingPolicy, PackageReviewEvidenceInterface,
        PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
        PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
    };

    const SOURCE_INSTANCE: PackageSourceConsumptionCommitment =
        PackageSourceConsumptionCommitment::for_test([5; 32]);

    fn package(byte: u8) -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([byte; 32]).unwrap()
    }

    fn nominal(owner: PackageReviewNominalOwner, path: &str) -> PackageReviewNominalIdentity {
        PackageReviewNominalIdentity {
            owner,
            path: path.to_owned(),
        }
    }

    fn producer_instance<'a>(
        policy: &'a PackagePolicyRepresentation,
        expected_source_instance: PackageSourceConsumptionCommitment,
        reviewed_source_instance: PackageSourceConsumptionCommitment,
    ) -> PackagePolicyRepresentationProducerInstance<'a> {
        PackagePolicyRepresentationProducerInstance {
            policy,
            expected_source_instance,
            reviewed_source_instance,
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
                .rejoin_foreign_demands(|identity| (identity == producer_id)
                    .then(|| producer_instance(&producer, SOURCE_INSTANCE, SOURCE_INSTANCE)))
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
            consumer.rejoin_foreign_demands(|_| Some(producer_instance(
                &producer,
                SOURCE_INSTANCE,
                SOURCE_INSTANCE
            ))),
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
            consumer.rejoin_foreign_demands(|_| Some(producer_instance(
                &producer,
                SOURCE_INSTANCE,
                SOURCE_INSTANCE
            ))),
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
            consumer.rejoin_foreign_demands(|_| Some(producer_instance(
                &producer,
                SOURCE_INSTANCE,
                SOURCE_INSTANCE
            ))),
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

    #[test]
    fn rejects_resolved_policy_owned_by_another_package() {
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
        let other = policy(
            package(3),
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            true,
            true,
        );
        assert!(matches!(
            consumer.rejoin_foreign_demands(|_| Some(producer_instance(
                &other,
                SOURCE_INSTANCE,
                SOURCE_INSTANCE
            ))),
            Err(PackagePolicyRepresentationAgreementError::ProducerPolicyAbsent { .. })
        ));
    }

    #[test]
    fn rejects_producer_source_instance_mismatch() {
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
            true,
            true,
        );
        assert!(matches!(
            consumer.rejoin_foreign_demands(|_| Some(producer_instance(
                &producer,
                SOURCE_INSTANCE,
                PackageSourceConsumptionCommitment::for_test([9; 32])
            ))),
            Err(PackagePolicyRepresentationAgreementError::ProducerSourceInstanceMismatch { .. })
        ));
    }

    #[test]
    fn accepts_producer_selection_equal_to_the_actual_application() {
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
        let mut producer = policy(
            producer_id,
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            true,
            true,
        );
        let use_ = &consumer.demands()[0].calling().opaque_uses()[0];
        producer
            .selected_availability
            .push(PackagePolicyRepresentationSelection {
                opaque: use_.opaque().clone(),
                carrier: use_.carrier().clone(),
                selection_owner: PackageReviewNominalOwner::Package(producer_id),
                application: use_.application().clone(),
                origin: use_.origin(),
                lifecycle: use_.lifecycle(),
                copy_disposition: use_.copy_disposition(),
            });
        assert!(
            consumer
                .rejoin_foreign_demands(|_| Some(producer_instance(
                    &producer,
                    SOURCE_INSTANCE,
                    SOURCE_INSTANCE
                )))
                .is_ok(),
            "an equal producer selection with a different selection owner agrees"
        );
    }

    #[test]
    fn rejects_producer_selection_mismatch_at_the_exchange() {
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
        let mut producer = policy(
            producer_id,
            target,
            PackageReviewNominalOwner::Package(producer_id),
            PackageReviewNominalOwner::Package(producer_id),
            true,
            true,
        );
        let use_ = &consumer.demands()[0].calling().opaque_uses()[0];
        let mut application = use_.application().clone();
        application.type_arguments = Vec::new();
        application
            .type_arguments
            .push(crate::record::PackageReviewTypeIdentity {
                canonical: "Different".to_owned(),
            });
        producer
            .selected_availability
            .push(PackagePolicyRepresentationSelection {
                opaque: use_.opaque().clone(),
                carrier: use_.carrier().clone(),
                selection_owner: PackageReviewNominalOwner::Package(producer_id),
                application,
                origin: use_.origin(),
                lifecycle: use_.lifecycle(),
                copy_disposition: use_.copy_disposition(),
            });
        assert!(matches!(
            consumer.rejoin_foreign_demands(|_| Some(producer_instance(
                &producer,
                SOURCE_INSTANCE,
                SOURCE_INSTANCE
            ))),
            Err(PackagePolicyRepresentationAgreementError::SelectedApplicationMismatch { .. })
        ));

        let mut disposition = producer.clone();
        disposition.selected_availability.clear();
        disposition.selected_availability.push(
            PackagePolicyRepresentationSelection {
                opaque: use_.opaque().clone(),
                carrier: use_.carrier().clone(),
                selection_owner: PackageReviewNominalOwner::Package(producer_id),
                application: use_.application().clone(),
                origin: use_.origin(),
                lifecycle: use_.lifecycle(),
                copy_disposition:
                    crate::record::PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
            },
        );
        assert!(matches!(
            consumer.rejoin_foreign_demands(|_| Some(producer_instance(
                &disposition,
                SOURCE_INSTANCE,
                SOURCE_INSTANCE
            ))),
            Err(PackagePolicyRepresentationAgreementError::SelectedApplicationMismatch { .. })
        ));
    }
}
