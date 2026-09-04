use psi_extents::{Extent, ExtentLoan};

use super::{
    AccessPlanDiagnostic, AdmittedResourceProfile, OwnedPlacementAdmission,
    OwnedPlacementRejection, PlaceEstablishmentError, PlacedView, PlacementAdmission,
    PlacementAdmissionId, PlacementRejection, PlacementResourceCompatibility,
    ValidatedPlacementPlan, validate_placement_resources,
};

pub fn admit_placement<'extent>(
    identity: PlacementAdmissionId,
    loan: ExtentLoan<'extent>,
    plan: &ValidatedPlacementPlan,
    profile: &AdmittedResourceProfile,
) -> Result<PlacementAdmission<'extent>, PlacementRejection<'extent>> {
    let validation = validate_placement_admission(&loan, plan, profile);
    match validation {
        Ok(resources) => Ok(PlacementAdmission {
            identity,
            placement_plan: plan.clone(),
            profile_receipt: profile.receipt(),
            profile: profile.clone(),
            resources,
            loan,
        }),
        Err(diagnostic) => Err(PlacementRejection { loan, diagnostic }),
    }
}

/// Admit one complete owned Extent without manufacturing an owned loan or a
/// second authority root.
///
/// Validation borrows the full range only for the duration of the check. The
/// accepted carrier then retains the original Extent; rejection returns that
/// same value with its sealed origin and lineage unchanged.
pub fn admit_owned_placement(
    identity: PlacementAdmissionId,
    extent: Extent,
    plan: &ValidatedPlacementPlan,
    profile: &AdmittedResourceProfile,
) -> Result<OwnedPlacementAdmission, OwnedPlacementRejection> {
    let validation = match extent.loan(0, extent.length()) {
        Ok(loan) => validate_placement_admission(&loan, plan, profile),
        Err(diagnostic) => Err(AccessPlanDiagnostic(format!(
            "owned extent could not produce its internal whole-range loan: {diagnostic}"
        ))),
    };
    match validation {
        Ok(resources) => Ok(OwnedPlacementAdmission {
            identity,
            placement_plan: plan.clone(),
            profile_receipt: profile.receipt(),
            profile: profile.clone(),
            resources,
            extent,
        }),
        Err(diagnostic) => Err(OwnedPlacementRejection { extent, diagnostic }),
    }
}

pub(super) fn validate_placement_admission(
    loan: &ExtentLoan<'_>,
    plan: &ValidatedPlacementPlan,
    profile: &AdmittedResourceProfile,
) -> Result<PlacementResourceCompatibility, AccessPlanDiagnostic> {
    let restricted = profile.restrict_to_loan(loan)?;
    let compatibility = validate_placement_resources(plan, &restricted)?;
    if !compatibility.base.admits(loan.base()) {
        return Err(AccessPlanDiagnostic(format!(
            "extent loan base {} does not satisfy placement base congruence: base mod {} must equal {}",
            loan.base(),
            compatibility.base.modulus,
            compatibility.base.residue
        )));
    }
    Ok(compatibility)
}

/// Establish one borrowed placed view only after independently replaying the
/// retained placement, admitted profile, and exact resource compatibility.
/// Rejection returns the complete loan-bearing admission unchanged.
pub fn place<'extent>(
    admission: PlacementAdmission<'extent>,
) -> Result<PlacedView<'extent>, PlaceEstablishmentError<'extent>> {
    let diagnostic = if admission.profile.receipt() != admission.profile_receipt {
        Some(AccessPlanDiagnostic(
            "borrowed placement profile receipt differs from its retained admitted profile".into(),
        ))
    } else {
        match validate_placement_admission(
            &admission.loan,
            &admission.placement_plan,
            &admission.profile,
        ) {
            Ok(resources) if resources == admission.resources => None,
            Ok(_) => Some(AccessPlanDiagnostic(
                "borrowed placement replayed resource compatibility differs from the retained admission"
                    .into(),
            )),
            Err(diagnostic) => Some(AccessPlanDiagnostic(format!(
                "borrowed placed-view establishment could not replay the admitted resource profile: {diagnostic}"
            ))),
        }
    };
    if let Some(diagnostic) = diagnostic {
        return Err(PlaceEstablishmentError {
            admission,
            diagnostic,
        });
    }
    Ok(PlacedView {
        loan: admission.loan,
        plan: admission.placement_plan,
        profile_receipt: admission.profile_receipt,
        profile: admission.profile,
        resources: admission.resources,
        admission: admission.identity,
    })
}
