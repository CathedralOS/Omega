use extents::{Extent, ExtentLoan};

use crate::ResourceProfileReceiptId;
use crate::normalized_identity;
use crate::{
    AccessPlanDiagnostic, AccessResourceCompatibility, AccessView, AdmittedResourceProfile,
    OwnedAccessAdmission, OwnedAccessRejection, ValidatedAccessPlan, validate_access_resources,
};

normalized_identity!(AccessAdmissionId, "placement-admission identity cannot be zero", pub(crate));

normalized_identity!(
    AccessOccurrenceId,
    "placed-occurrence identity cannot be zero"
);

/// One accepted placement that owns the exact extent loan checked by the
/// provider. It cannot be reused to admit another range or another loan.
#[derive(Debug)]
pub struct AccessAdmission<'extent> {
    pub(crate) identity: AccessAdmissionId,
    pub(crate) access_plan: ValidatedAccessPlan,
    pub(crate) profile_receipt: ResourceProfileReceiptId,
    pub(crate) profile: AdmittedResourceProfile,
    pub(crate) resources: AccessResourceCompatibility,
    pub(crate) loan: ExtentLoan<'extent>,
}

#[derive(Debug)]
pub struct AccessRejection<'extent> {
    loan: ExtentLoan<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

/// Failed borrowed access-view establishment returns the highest valid
/// loan-bearing admission intact for corrected retry or withdrawal.
#[derive(Debug)]
pub struct PlaceEstablishmentError<'extent> {
    admission: AccessAdmission<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> AccessAdmission<'extent> {
    pub const fn identity(&self) -> AccessAdmissionId {
        self.identity
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn resources(&self) -> &AccessResourceCompatibility {
        &self.resources
    }

    /// Cancel permission-only admission and recover the exact source loan.
    ///
    /// No placed content has been established at this stage, so withdrawal
    /// makes no content, destruction, vacancy, or allocator-release claim.
    pub fn withdraw(self) -> ExtentLoan<'extent> {
        self.loan
    }
}

impl<'extent> PlaceEstablishmentError<'extent> {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (AccessAdmission<'extent>, AccessPlanDiagnostic) {
        (self.admission, self.diagnostic)
    }
}

impl<'extent> AccessRejection<'extent> {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (ExtentLoan<'extent>, AccessPlanDiagnostic) {
        (self.loan, self.diagnostic)
    }
}

pub fn admit_access<'extent>(
    identity: AccessAdmissionId,
    loan: ExtentLoan<'extent>,
    plan: &ValidatedAccessPlan,
    profile: &AdmittedResourceProfile,
) -> Result<AccessAdmission<'extent>, AccessRejection<'extent>> {
    let validation = validate_access_admission(&loan, plan, profile);
    match validation {
        Ok(resources) => Ok(AccessAdmission {
            identity,
            access_plan: plan.clone(),
            profile_receipt: profile.receipt(),
            profile: profile.clone(),
            resources,
            loan,
        }),
        Err(diagnostic) => Err(AccessRejection { loan, diagnostic }),
    }
}

/// Admit one complete owned Extent without manufacturing an owned loan or a
/// second authority root.
///
/// Validation borrows the full range only for the duration of the check. The
/// accepted carrier then retains the original Extent; rejection returns that
/// same value with its sealed origin and lineage unchanged.
pub fn admit_owned_access(
    identity: AccessAdmissionId,
    extent: Extent,
    plan: &ValidatedAccessPlan,
    profile: &AdmittedResourceProfile,
) -> Result<OwnedAccessAdmission, OwnedAccessRejection> {
    let validation = match extent.loan(0, extent.length()) {
        Ok(loan) => validate_access_admission(&loan, plan, profile),
        Err(diagnostic) => Err(AccessPlanDiagnostic(format!(
            "owned extent could not produce its internal whole-range loan: {diagnostic}"
        ))),
    };
    match validation {
        Ok(resources) => Ok(OwnedAccessAdmission {
            identity,
            access_plan: plan.clone(),
            profile_receipt: profile.receipt(),
            profile: profile.clone(),
            resources,
            extent,
        }),
        Err(diagnostic) => Err(OwnedAccessRejection { extent, diagnostic }),
    }
}

pub(super) fn validate_access_admission(
    loan: &ExtentLoan<'_>,
    plan: &ValidatedAccessPlan,
    profile: &AdmittedResourceProfile,
) -> Result<AccessResourceCompatibility, AccessPlanDiagnostic> {
    let restricted = profile.restrict_to_loan(loan)?;
    let compatibility = validate_access_resources(plan, &restricted)?;
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

/// Establish one borrowed access view only after independently replaying the
/// retained placement, admitted profile, and exact resource compatibility.
/// Rejection returns the complete loan-bearing admission unchanged.
pub fn place<'extent>(
    admission: AccessAdmission<'extent>,
) -> Result<AccessView<'extent>, PlaceEstablishmentError<'extent>> {
    let diagnostic = if admission.profile.receipt() != admission.profile_receipt {
        Some(AccessPlanDiagnostic(
            "borrowed placement profile receipt differs from its retained admitted profile".into(),
        ))
    } else {
        match validate_access_admission(
            &admission.loan,
            &admission.access_plan,
            &admission.profile,
        ) {
            Ok(resources) if resources == admission.resources => None,
            Ok(_) => Some(AccessPlanDiagnostic(
                "borrowed placement replayed resource compatibility differs from the retained admission"
                    .into(),
            )),
            Err(diagnostic) => Some(AccessPlanDiagnostic(format!(
                "borrowed access-view establishment could not replay the admitted resource profile: {diagnostic}"
            ))),
        }
    };
    if let Some(diagnostic) = diagnostic {
        return Err(PlaceEstablishmentError {
            admission,
            diagnostic,
        });
    }
    Ok(AccessView {
        loan: admission.loan,
        plan: admission.access_plan,
        profile_receipt: admission.profile_receipt,
        profile: admission.profile,
        resources: admission.resources,
        admission: admission.identity,
    })
}
