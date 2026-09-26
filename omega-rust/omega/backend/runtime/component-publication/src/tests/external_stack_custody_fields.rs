//! Substitutable-field inventories for the external stack provision custody
//! matrices driven by `tests.rs`'s
//! `external_stack_provision_rejects_every_one_field_substitution`.
//!
//! Two record families carry the provider-owned stack supply: the
//! `AdmittedExternalStackDomainLease` a provider admits for one domain of one
//! installed occurrence, and the `ProvisionedExternalStackSet` sealing those
//! leases that the runnable component retains. Each declared lane is one
//! representable field whose substitution the family's independent seams
//! reject: the seal binding each lease's occurrence triple, and the
//! install-time provision gate re-authenticating the retained set's
//! occurrence triple and each demanded domain's supply.
//!
//! Deliberately not lanes: a lease's `provisioner` and `validation_receipt`
//! are provider-minted provenance carried verbatim, and a lease row restated
//! inside an already-sealed set — its occurrence triple, its domain under
//! the sealed key, or supply over the composed demand — is carried because
//! sealing is the lease-binding seam and the coverage join is a supply
//! inequality. The test names those carried axes explicitly.

use super::{AdmittedExternalStackDomainLease, ProvisionedExternalStackSet};
use executable_installation::InstalledCodeContext;
use external_roots::{ExternalRootDiagnostic, StackDomain};

optimization_core::custody_field_inventory! {
    /// One substitutable lane of an `AdmittedExternalStackDomainLease`
    /// before it is sealed; checked by the seal plus install-time provision
    /// gate in `external_stack_provision_rejects_every_one_field_substitution`.
    pub enum ExternalStackDomainLeaseFieldForTest {
        InstalledCode,
        InstalledCodeContext,
        Artifact,
        Domain,
        CapacityBelowDemand,
        AlignmentBelowDemand,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable lane of the retained `ProvisionedExternalStackSet`;
    /// checked by the install-time provision gate in
    /// `external_stack_provision_rejects_every_one_field_substitution`.
    pub enum ProvisionedExternalStackSetFieldForTest {
        InstalledCode,
        InstalledCodeContext,
        Artifact,
        DemandedLeaseDropped,
        DemandedLeaseCapacityBelowDemand,
        DemandedLeaseCapacityZeroed,
        DemandedLeaseAlignmentBelowDemand,
        DemandedLeaseAlignmentZeroed,
    }
}

/// The named rejection an external stack custody seam reports.
/// `ExternalRootDiagnostic` carries prose, so the matrices compare the kind
/// its exact diagnostic fragment names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalStackCustodyRejection {
    /// The seal refuses a lease bound to another installed occurrence.
    SealOccurrence,
    /// Component admission refuses a set bound to another occurrence.
    AdmissionOccurrence,
    /// The install-time gate refuses a retained set naming another
    /// occurrence.
    RetainedOccurrence,
    /// No admitted lease provisions a domain the root demands.
    UncoveredDomain,
    /// A demanded domain's lease capacity is below the composed demand.
    CapacityBelowDemand,
    /// A demanded domain's lease alignment is below the composed demand.
    AlignmentBelowDemand,
}

impl ExternalStackCustodyRejection {
    /// The exact diagnostic fragment each rejection kind names.
    pub const fn fragment(self) -> &'static str {
        match self {
            Self::SealOccurrence => {
                "different installed-code occurrence than the provision set being sealed"
            }
            Self::AdmissionOccurrence => {
                "different installed-code occurrence than the retained runnable component"
            }
            Self::RetainedOccurrence => {
                "retained external stack provision names a different installed-code occurrence"
            }
            Self::UncoveredDomain => "no admitted stack lease provisions domain",
            Self::CapacityBelowDemand => "below the composed 2048-byte demand",
            Self::AlignmentBelowDemand => "below the composed alignment",
        }
    }

    const ALL: [Self; 6] = [
        Self::SealOccurrence,
        Self::AdmissionOccurrence,
        Self::RetainedOccurrence,
        Self::UncoveredDomain,
        Self::CapacityBelowDemand,
        Self::AlignmentBelowDemand,
    ];
}

/// Classify a seam's diagnostic into the family's named vocabulary; any
/// other diagnostic means the substitution produced a rejection the family
/// does not declare.
pub fn classify_external_stack_rejection(
    diagnostic: &ExternalRootDiagnostic,
) -> ExternalStackCustodyRejection {
    let text = diagnostic.to_string();
    ExternalStackCustodyRejection::ALL
        .into_iter()
        .find(|rejection| text.contains(rejection.fragment()))
        .unwrap_or_else(|| {
            panic!("external stack rejection outside the declared vocabulary: {text}")
        })
}

/// The lease family's honest-recomputation hook: mutate exactly the declared
/// field of an admitted lease. `donor` is an authentic lease admitted for a
/// distinct occurrence and supplies the foreign installed-code and artifact
/// identities; `foreign_context` is the receipt context of a placement whose
/// compact identities collide with the honest one, so the context lane moves
/// the exact occurrence alone. Supply lanes take fixed values below the
/// callback root's composed 2048-byte, 16-aligned interrupted demand.
pub fn substitute_external_stack_lease_for_test(
    lease: &mut AdmittedExternalStackDomainLease,
    field: ExternalStackDomainLeaseFieldForTest,
    donor: &AdmittedExternalStackDomainLease,
    foreign_context: &InstalledCodeContext,
) {
    use ExternalStackDomainLeaseFieldForTest as Leg;
    let mut donor = donor.clone();
    match field {
        Leg::InstalledCode => {
            *lease.installed_code_mut_for_test() = *donor.installed_code_mut_for_test()
        }
        Leg::InstalledCodeContext => {
            *lease.installed_code_context_mut_for_test() = foreign_context.clone();
        }
        Leg::Artifact => *lease.artifact_mut_for_test() = *donor.artifact_mut_for_test(),
        Leg::Domain => *lease.domain_mut_for_test() = StackDomain::Dedicated { class: 7 },
        Leg::CapacityBelowDemand => *lease.capacity_bytes_mut_for_test() = 1024,
        Leg::AlignmentBelowDemand => *lease.alignment_mut_for_test() = 8,
    }
}

/// The exact seam rejection each declared lease lane must produce: the
/// occurrence triple rejects at the seal, and supply lanes seal honestly
/// but leave the demanded interrupted domain uncovered at install.
pub fn external_stack_lease_custody_outcome(
    field: ExternalStackDomainLeaseFieldForTest,
) -> optimization_core::MutationOutcome<ExternalStackCustodyRejection> {
    use ExternalStackCustodyRejection as Rejection;
    use ExternalStackDomainLeaseFieldForTest as Leg;
    optimization_core::MutationOutcome::ExactError(match field {
        Leg::InstalledCode | Leg::InstalledCodeContext | Leg::Artifact => Rejection::SealOccurrence,
        Leg::Domain => Rejection::UncoveredDomain,
        Leg::CapacityBelowDemand => Rejection::CapacityBelowDemand,
        Leg::AlignmentBelowDemand => Rejection::AlignmentBelowDemand,
    })
}

/// The retained set's lease row for the interrupted domain the callback
/// root demands.
pub fn demanded_lease(
    set: &mut ProvisionedExternalStackSet,
) -> &mut AdmittedExternalStackDomainLease {
    set.leases_mut_for_test()
        .get_mut(&StackDomain::Interrupted)
        .expect("interrupted lease")
}

/// The set family's honest-recomputation hook: mutate exactly the declared
/// field of a sealed set, including one field of its demanded interrupted
/// lease row. `donor` is an authentic set sealed for a distinct occurrence;
/// `foreign_context` plays the same role as in the lease hook.
pub fn substitute_provisioned_external_stack_set_for_test(
    set: &mut ProvisionedExternalStackSet,
    field: ProvisionedExternalStackSetFieldForTest,
    donor: &ProvisionedExternalStackSet,
    foreign_context: &InstalledCodeContext,
) {
    use ProvisionedExternalStackSetFieldForTest as Leg;
    let mut donor = donor.clone();
    match field {
        Leg::InstalledCode => {
            *set.installed_code_mut_for_test() = *donor.installed_code_mut_for_test()
        }
        Leg::InstalledCodeContext => {
            *set.installed_code_context_mut_for_test() = foreign_context.clone();
        }
        Leg::Artifact => *set.artifact_mut_for_test() = *donor.artifact_mut_for_test(),
        Leg::DemandedLeaseDropped => {
            set.leases_mut_for_test().remove(&StackDomain::Interrupted);
        }
        Leg::DemandedLeaseCapacityBelowDemand => {
            *demanded_lease(set).capacity_bytes_mut_for_test() = 1024;
        }
        Leg::DemandedLeaseCapacityZeroed => {
            *demanded_lease(set).capacity_bytes_mut_for_test() = 0;
        }
        Leg::DemandedLeaseAlignmentBelowDemand => {
            *demanded_lease(set).alignment_mut_for_test() = 8;
        }
        Leg::DemandedLeaseAlignmentZeroed => {
            *demanded_lease(set).alignment_mut_for_test() = 0;
        }
    }
}

/// The exact install-gate rejection each declared set lane must produce.
pub fn provisioned_external_stack_set_custody_outcome(
    field: ProvisionedExternalStackSetFieldForTest,
) -> optimization_core::MutationOutcome<ExternalStackCustodyRejection> {
    use ExternalStackCustodyRejection as Rejection;
    use ProvisionedExternalStackSetFieldForTest as Leg;
    optimization_core::MutationOutcome::ExactError(match field {
        Leg::InstalledCode | Leg::InstalledCodeContext | Leg::Artifact => {
            Rejection::RetainedOccurrence
        }
        Leg::DemandedLeaseDropped => Rejection::UncoveredDomain,
        Leg::DemandedLeaseCapacityBelowDemand | Leg::DemandedLeaseCapacityZeroed => {
            Rejection::CapacityBelowDemand
        }
        Leg::DemandedLeaseAlignmentBelowDemand | Leg::DemandedLeaseAlignmentZeroed => {
            Rejection::AlignmentBelowDemand
        }
    })
}
