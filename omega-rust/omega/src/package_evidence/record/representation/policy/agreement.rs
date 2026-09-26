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
use crate::package_compilation::PackageSourceConsumptionCommitment;
use crate::package_evidence::record::{
    PackagePolicyCallingOpaqueUse, PackageReviewNominalIdentity, PackageReviewNominalOwner,
};
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
mod tests;
