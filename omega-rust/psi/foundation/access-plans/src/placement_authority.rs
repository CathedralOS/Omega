//! Private normalization and replay of the authority behind a placed access.

use extents::{LoanPolarity, ResidentClaimId};

use super::owned_atomic_resident_custody::validate_owned_atomic_resident_authority;
use super::owned_resident_custody::validate_resident_observation;
use super::{
    AccessPlanDiagnostic, AdmittedResourceProfile, AdmittedSchemaDeviceCorrespondence,
    BorrowPolarity, EstablishedBorrowedAtomicResidentPlacement,
    EstablishedBorrowedResidentPlacement, EstablishedOwnedAtomicPlacement,
    EstablishedOwnedPlacement, PlacedOccurrenceId, PlacedView, PlacementAdmissionId,
    PlacementResourceCompatibility, ResourceProfileReceiptId, ValidatedPlacementPlan,
    replay_owned_admission_resources, validate_owned_content_binding, validate_placement_admission,
    validate_provider_content_binding,
};

/// Private lifetime witness for the exact authority that justified a placed
/// access. Owned Stable access retains the whole established carrier rather
/// than reducing provider content custody to a bare Extent reference.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(super) enum PlacementAuthorityRef<'view, 'extent> {
    Borrowed(&'view PlacedView<'extent>),
    CorrespondedBorrowed(&'view super::SchemaCorrespondedPlacedView<'extent>),
    BorrowedResident(&'view EstablishedBorrowedResidentPlacement<'extent>),
    BorrowedAtomicResident(&'view EstablishedBorrowedAtomicResidentPlacement<'extent>),
    EstablishedOwned(&'view EstablishedOwnedPlacement),
    EstablishedOwnedAtomic(&'view EstablishedOwnedAtomicPlacement),
}

impl<'view, 'extent> PlacementAuthorityRef<'view, 'extent> {
    pub(super) const fn base(self) -> u64 {
        match self {
            Self::Borrowed(view) => view.loan.base(),
            Self::CorrespondedBorrowed(view) => view.view().loan.base(),
            Self::BorrowedResident(established) => established.base(),
            Self::BorrowedAtomicResident(established) => established.base(),
            Self::EstablishedOwned(established) => established.extent().base(),
            Self::EstablishedOwnedAtomic(established) => established.extent().base(),
        }
    }

    pub(super) const fn placement_plan(self) -> &'view ValidatedPlacementPlan {
        match self {
            Self::Borrowed(view) => &view.plan,
            Self::CorrespondedBorrowed(view) => &view.view().plan,
            Self::BorrowedResident(established) => established.placement_plan(),
            Self::BorrowedAtomicResident(established) => established.placement_plan(),
            Self::EstablishedOwned(established) => established.placement_plan(),
            Self::EstablishedOwnedAtomic(established) => established.placement_plan(),
        }
    }

    pub(super) const fn profile_receipt(self) -> ResourceProfileReceiptId {
        match self {
            Self::Borrowed(view) => view.profile_receipt,
            Self::CorrespondedBorrowed(view) => view.view().profile_receipt,
            Self::BorrowedResident(established) => established.profile_receipt(),
            Self::BorrowedAtomicResident(established) => established.profile_receipt(),
            Self::EstablishedOwned(established) => established.profile_receipt(),
            Self::EstablishedOwnedAtomic(established) => established.profile_receipt(),
        }
    }

    pub(super) const fn profile(self) -> &'view AdmittedResourceProfile {
        match self {
            Self::Borrowed(view) => &view.profile,
            Self::CorrespondedBorrowed(view) => &view.view().profile,
            Self::BorrowedResident(established) => established.profile(),
            Self::BorrowedAtomicResident(established) => established.profile(),
            Self::EstablishedOwned(established) => &established.admission.profile,
            Self::EstablishedOwnedAtomic(established) => established.profile(),
        }
    }

    pub(super) fn replay_resources(
        self,
    ) -> Result<PlacementResourceCompatibility, AccessPlanDiagnostic> {
        match self {
            Self::Borrowed(view) => {
                validate_placement_admission(&view.loan, &view.plan, &view.profile)
            }
            Self::CorrespondedBorrowed(view) => validate_placement_admission(
                &view.view().loan,
                &view.view().plan,
                &view.view().profile,
            ),
            Self::BorrowedResident(established) => validate_placement_admission(
                established.loan(),
                established.placement_plan(),
                established.profile(),
            ),
            Self::BorrowedAtomicResident(established) => {
                established.validate_lender_binding("borrowed Atomic resident resource replay")?;
                validate_placement_admission(
                    established.loan(),
                    established.placement_plan(),
                    established.profile(),
                )
            }
            Self::EstablishedOwned(established) => {
                replay_owned_admission_resources(&established.admission)
            }
            Self::EstablishedOwnedAtomic(established) => {
                replay_owned_admission_resources(&established.admission)
            }
        }
    }

    pub(super) fn replay_resident_content(
        self,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        let replay = match self {
            Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => return Ok(()),
            Self::BorrowedResident(established) => validate_provider_content_binding(
                established.placement_plan(),
                established.loan(),
                established.content(),
            )
            .and_then(|()| {
                validate_resident_observation(
                    established.placement_plan(),
                    super::ObservationModel::Stable,
                    transition,
                )
            }),
            Self::BorrowedAtomicResident(established) => validate_provider_content_binding(
                established.placement_plan(),
                established.loan(),
                established.content(),
            )
            .and_then(|()| {
                validate_resident_observation(
                    established.placement_plan(),
                    super::ObservationModel::Atomic,
                    transition,
                )
            }),
            Self::EstablishedOwned(established) => {
                validate_owned_content_binding(&established.admission, &established.content)
                    .and_then(|()| {
                        validate_resident_observation(
                            established.placement_plan(),
                            super::ObservationModel::Stable,
                            transition,
                        )
                    })
            }
            Self::EstablishedOwnedAtomic(established) => validate_owned_atomic_resident_authority(
                &established.admission,
                &established.content,
                transition,
            ),
        };
        replay.map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "{transition} could not replay the retained resident content grant: {diagnostic}"
            ))
        })
    }

    pub(super) fn replay_correspondence(
        self,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        match self {
            Self::CorrespondedBorrowed(view) => {
                view.validate_correspondence().map_err(|diagnostic| {
                    AccessPlanDiagnostic(format!(
                        "{transition} could not replay the retained schema/device correspondence: {diagnostic}"
                    ))
                })
            }
            _ => Ok(()),
        }
    }

    pub(super) const fn correspondence(self) -> Option<&'view AdmittedSchemaDeviceCorrespondence> {
        match self {
            Self::CorrespondedBorrowed(view) => Some(view.correspondence()),
            _ => None,
        }
    }

    pub(super) const fn resources(self) -> &'view PlacementResourceCompatibility {
        match self {
            Self::Borrowed(view) => &view.resources,
            Self::CorrespondedBorrowed(view) => &view.view().resources,
            Self::BorrowedResident(established) => established.resources(),
            Self::BorrowedAtomicResident(established) => established.resources(),
            Self::EstablishedOwned(established) => established.resources(),
            Self::EstablishedOwnedAtomic(established) => established.resources(),
        }
    }

    pub(super) const fn admission(self) -> PlacementAdmissionId {
        match self {
            Self::Borrowed(view) => view.admission,
            Self::CorrespondedBorrowed(view) => view.view().admission,
            Self::BorrowedResident(established) => established.admission(),
            Self::BorrowedAtomicResident(established) => established.admission(),
            Self::EstablishedOwned(established) => established.admission(),
            Self::EstablishedOwnedAtomic(established) => established.admission(),
        }
    }

    pub(super) const fn source_loan(self) -> BorrowPolarity {
        let polarity = match self {
            Self::Borrowed(view) => view.loan.polarity(),
            Self::CorrespondedBorrowed(view) => view.view().loan.polarity(),
            Self::BorrowedResident(established) => established.loan_polarity(),
            Self::BorrowedAtomicResident(established) => established.loan_polarity(),
            Self::EstablishedOwned(_) | Self::EstablishedOwnedAtomic(_) => LoanPolarity::Exclusive,
        };
        match polarity {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        }
    }

    pub(super) const fn resident_claim(self) -> Option<ResidentClaimId> {
        match self {
            Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => None,
            Self::BorrowedResident(established) => Some(established.resident_claim()),
            Self::BorrowedAtomicResident(established) => Some(established.resident_claim()),
            Self::EstablishedOwned(established) => Some(established.resident_claim()),
            Self::EstablishedOwnedAtomic(established) => Some(established.resident_claim()),
        }
    }

    pub(super) const fn placed_occurrence(self) -> Option<PlacedOccurrenceId> {
        match self {
            Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => None,
            Self::BorrowedResident(established) => Some(established.occurrence()),
            Self::BorrowedAtomicResident(established) => Some(established.occurrence()),
            Self::EstablishedOwned(established) => Some(established.occurrence),
            Self::EstablishedOwnedAtomic(established) => Some(established.occurrence()),
        }
    }
}
