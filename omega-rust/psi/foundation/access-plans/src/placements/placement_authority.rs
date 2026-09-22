//! Private normalization and replay of the authority behind a placed access.

use extents::{LoanPolarity, ResidentClaimId};

use crate::placements::owned_resident_custody::replay_owned_admission_resources;
use crate::placements::owned_resident_custody::validate_owned_content_binding;
use crate::placements::owned_resident_custody::validate_owned_resident_authority;
use crate::placements::owned_resident_custody::validate_provider_content_binding;
use crate::placements::owned_resident_custody::validate_resident_observation;
use crate::placements::placement_admission::validate_placement_admission;
use crate::{
    AccessPlanDiagnostic, AdmittedResourceProfile, AdmittedSchemaDeviceCorrespondence,
    BorrowPolarity, EstablishedBorrowedAtomicResidentPlacement,
    EstablishedBorrowedResidentPlacement, EstablishedOwnedAtomicPlacement,
    EstablishedOwnedExternalPlacement, EstablishedOwnedPlacement, PlacedOccurrenceId, PlacedView,
    PlacementAdmissionId, PlacementResourceCompatibility, ResourceProfileReceiptId,
    ValidatedPlacementPlan,
};

/// Private lifetime witness for the exact authority that justified a placed
/// access. Owned Stable access retains the whole established carrier rather
/// than reducing provider content custody to a bare Extent reference.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum PlacementAuthorityRef<'view, 'extent> {
    Borrowed(&'view PlacedView<'extent>),
    CorrespondedBorrowed(
        &'view crate::placements::schema_correspondence::SchemaCorrespondedPlacedView<'extent>,
    ),
    BorrowedResident(&'view EstablishedBorrowedResidentPlacement<'extent>),
    BorrowedAtomicResident(&'view EstablishedBorrowedAtomicResidentPlacement<'extent>),
    EstablishedOwned(&'view EstablishedOwnedPlacement),
    EstablishedOwnedAtomic(&'view EstablishedOwnedAtomicPlacement),
    OwnedCorrespondedExternal(&'view EstablishedOwnedExternalPlacement),
}

impl<'view, 'extent> PlacementAuthorityRef<'view, 'extent> {
    pub(crate) const fn base(self) -> u64 {
        match self {
            Self::Borrowed(view) => view.loan.base(),
            Self::CorrespondedBorrowed(view) => view.view().loan.base(),
            Self::BorrowedResident(established) => established.base(),
            Self::BorrowedAtomicResident(established) => established.base(),
            Self::EstablishedOwned(established) => established.extent().base(),
            Self::EstablishedOwnedAtomic(established) => established.extent().base(),
            Self::OwnedCorrespondedExternal(established) => established.extent().base(),
        }
    }

    pub(crate) const fn placement_plan(self) -> &'view ValidatedPlacementPlan {
        match self {
            Self::Borrowed(view) => &view.plan,
            Self::CorrespondedBorrowed(view) => &view.view().plan,
            Self::BorrowedResident(established) => established.placement_plan(),
            Self::BorrowedAtomicResident(established) => established.placement_plan(),
            Self::EstablishedOwned(established) => established.placement_plan(),
            Self::EstablishedOwnedAtomic(established) => established.placement_plan(),
            Self::OwnedCorrespondedExternal(established) => established.placement_plan(),
        }
    }

    pub(crate) const fn profile_receipt(self) -> ResourceProfileReceiptId {
        match self {
            Self::Borrowed(view) => view.profile_receipt,
            Self::CorrespondedBorrowed(view) => view.view().profile_receipt,
            Self::BorrowedResident(established) => established.profile_receipt(),
            Self::BorrowedAtomicResident(established) => established.profile_receipt(),
            Self::EstablishedOwned(established) => established.profile_receipt(),
            Self::EstablishedOwnedAtomic(established) => established.profile_receipt(),
            Self::OwnedCorrespondedExternal(established) => established.profile_receipt(),
        }
    }

    pub(crate) const fn profile(self) -> &'view AdmittedResourceProfile {
        match self {
            Self::Borrowed(view) => &view.profile,
            Self::CorrespondedBorrowed(view) => &view.view().profile,
            Self::BorrowedResident(established) => established.profile(),
            Self::BorrowedAtomicResident(established) => established.profile(),
            Self::EstablishedOwned(established) => &established.admission.profile,
            Self::EstablishedOwnedAtomic(established) => established.profile(),
            Self::OwnedCorrespondedExternal(established) => &established.admission.profile,
        }
    }

    pub(crate) fn replay_resources(
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
            Self::OwnedCorrespondedExternal(established) => {
                replay_owned_admission_resources(&established.admission)
            }
        }
    }

    pub(crate) fn replay_resident_content(
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
                    crate::access_plan::ObservationModel::Stable,
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
                    crate::access_plan::ObservationModel::Atomic,
                    transition,
                )
            }),
            Self::EstablishedOwned(established) => {
                validate_owned_content_binding(&established.admission, &established.content)
                    .and_then(|()| {
                        validate_resident_observation(
                            established.placement_plan(),
                            crate::access_plan::ObservationModel::Stable,
                            transition,
                        )
                    })
            }
            Self::EstablishedOwnedAtomic(established) => validate_owned_resident_authority(
                &established.admission,
                &established.content,
                crate::access_plan::ObservationModel::Atomic,
                transition,
            ),
            Self::OwnedCorrespondedExternal(established) => {
                established.validate_external_observation(transition)
            }
        };
        replay.map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "{transition} could not replay the retained resident content grant: {diagnostic}"
            ))
        })
    }

    pub(crate) fn replay_correspondence(
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
            Self::OwnedCorrespondedExternal(established) => {
                established.validate_correspondence(transition)
            }
            _ => Ok(()),
        }
    }

    pub(crate) const fn correspondence(self) -> Option<&'view AdmittedSchemaDeviceCorrespondence> {
        match self {
            Self::CorrespondedBorrowed(view) => Some(view.correspondence()),
            Self::OwnedCorrespondedExternal(established) => Some(established.correspondence()),
            _ => None,
        }
    }

    pub(crate) const fn resources(self) -> &'view PlacementResourceCompatibility {
        match self {
            Self::Borrowed(view) => &view.resources,
            Self::CorrespondedBorrowed(view) => &view.view().resources,
            Self::BorrowedResident(established) => established.resources(),
            Self::BorrowedAtomicResident(established) => established.resources(),
            Self::EstablishedOwned(established) => established.resources(),
            Self::EstablishedOwnedAtomic(established) => established.resources(),
            Self::OwnedCorrespondedExternal(established) => established.resources(),
        }
    }

    pub(crate) const fn admission(self) -> PlacementAdmissionId {
        match self {
            Self::Borrowed(view) => view.admission,
            Self::CorrespondedBorrowed(view) => view.view().admission,
            Self::BorrowedResident(established) => established.admission(),
            Self::BorrowedAtomicResident(established) => established.admission(),
            Self::EstablishedOwned(established) => established.admission(),
            Self::EstablishedOwnedAtomic(established) => established.admission(),
            Self::OwnedCorrespondedExternal(established) => established.admission(),
        }
    }

    pub(crate) const fn source_loan(self) -> BorrowPolarity {
        let polarity = match self {
            Self::Borrowed(view) => view.loan.polarity(),
            Self::CorrespondedBorrowed(view) => view.view().loan.polarity(),
            Self::BorrowedResident(established) => established.loan_polarity(),
            Self::BorrowedAtomicResident(established) => established.loan_polarity(),
            Self::EstablishedOwned(_)
            | Self::EstablishedOwnedAtomic(_)
            | Self::OwnedCorrespondedExternal(_) => LoanPolarity::Exclusive,
        };
        match polarity {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        }
    }

    pub(crate) const fn resident_claim(self) -> Option<ResidentClaimId> {
        match self {
            Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => None,
            Self::BorrowedResident(established) => Some(established.resident_claim()),
            Self::BorrowedAtomicResident(established) => Some(established.resident_claim()),
            Self::EstablishedOwned(established) => Some(established.resident_claim()),
            Self::EstablishedOwnedAtomic(established) => Some(established.resident_claim()),
            Self::OwnedCorrespondedExternal(_) => None,
        }
    }

    pub(crate) const fn placed_occurrence(self) -> Option<PlacedOccurrenceId> {
        match self {
            Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => None,
            Self::BorrowedResident(established) => Some(established.occurrence()),
            Self::BorrowedAtomicResident(established) => Some(established.occurrence()),
            Self::EstablishedOwned(established) => Some(established.occurrence),
            Self::EstablishedOwnedAtomic(established) => Some(established.occurrence()),
            Self::OwnedCorrespondedExternal(established) => Some(established.occurrence()),
        }
    }
}
