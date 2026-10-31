use extents::{ExtentLoan, ProviderExistingContentGrant};

use crate::placements::access_admission::validate_access_admission;
use crate::{
    AccessPlanDiagnostic, AccessResourceCompatibility, DormantOwnedResident, ObservationModel,
    OwnedAccessAdmission, OwnedStableAdoptionError, ValidatedAccessPlan,
};

/// Establish provider-validated existing content through the Stable adoption
/// route.
///
/// The content grant was minted only while the corresponding provider root
/// authority was consumed. Adoption independently binds its admitted
/// interpretation to the actual normalized placement and rejects any drift in
/// origin, lineage, or geometry. External and Atomic observations use their
/// own future adoption routes and cannot pass through this Stable transition.
pub fn adopt_owned_stable(
    admission: OwnedAccessAdmission,
    content: ProviderExistingContentGrant,
) -> Result<DormantOwnedResident, OwnedStableAdoptionError> {
    if let Err(diagnostic) = validate_owned_stable_adoption(&admission, &content) {
        return Err(OwnedStableAdoptionError {
            admission,
            content,
            diagnostic,
        });
    }
    Ok(DormantOwnedResident { admission, content })
}

fn validate_owned_stable_adoption(
    admission: &OwnedAccessAdmission,
    content: &ProviderExistingContentGrant,
) -> Result<(), AccessPlanDiagnostic> {
    let replayed_resources = replay_owned_admission_resources(admission).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "Stable adoption could not replay the admitted resource profile: {diagnostic}"
        ))
    })?;
    if replayed_resources != admission.resources {
        return Err(AccessPlanDiagnostic(
            "Stable adoption replayed resource compatibility differs from the owned admission"
                .into(),
        ));
    }

    validate_owned_content_binding(admission, content)?;
    validate_resident_observation(
        &admission.access_plan,
        ObservationModel::Stable,
        "Stable adoption",
    )
}

pub(super) fn validate_owned_content_binding(
    admission: &OwnedAccessAdmission,
    content: &ProviderExistingContentGrant,
) -> Result<(), AccessPlanDiagnostic> {
    let extent = &admission.extent;
    let loan = extent.loan(0, extent.length()).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "owned content binding could not replay its whole-range loan: {diagnostic}"
        ))
    })?;
    validate_provider_content_binding(&admission.access_plan, &loan, content)
}

pub(super) fn validate_provider_content_binding(
    plan: &ValidatedAccessPlan,
    loan: &ExtentLoan<'_>,
    content: &ProviderExistingContentGrant,
) -> Result<(), AccessPlanDiagnostic> {
    if content.interpretation() != plan.content_interpretation() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content interpretation commitment does not match the admitted placement"
                .into(),
        ));
    }
    if content.origin() != loan.origin() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content origin does not match the admitted Extent".into(),
        ));
    }
    if content.lineage_root() != loan.lineage_root() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content lineage does not match the admitted Extent".into(),
        ));
    }
    if content.base() != loan.base() || content.length() != loan.length() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content geometry does not match the admitted Extent".into(),
        ));
    }
    if content.address_space() != loan.address_space() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content address space does not match the admitted Extent".into(),
        ));
    }
    if content.provenance() != loan.provenance() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content provenance does not match the admitted Extent".into(),
        ));
    }
    if content.era() != loan.era() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content mapping era does not match the admitted Extent".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_resident_observation(
    plan: &ValidatedAccessPlan,
    required: ObservationModel,
    route: &str,
) -> Result<(), AccessPlanDiagnostic> {
    if let Some(descriptor) = plan
        .access()
        .field_descriptors()
        .iter()
        .find(|descriptor| descriptor.observation() != required)
    {
        return Err(AccessPlanDiagnostic(format!(
            "field `{}` uses {:?} observation and cannot enter the {route}",
            descriptor.field(),
            descriptor.observation()
        )));
    }
    Ok(())
}

pub(super) fn replay_owned_admission_resources(
    admission: &OwnedAccessAdmission,
) -> Result<AccessResourceCompatibility, AccessPlanDiagnostic> {
    if admission.profile.receipt() != admission.profile_receipt {
        return Err(AccessPlanDiagnostic(
            "owned placement profile receipt differs from its retained admitted profile".into(),
        ));
    }
    let extent = &admission.extent;
    let loan = extent.loan(0, extent.length()).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "owned placement could not replay its whole-range loan: {diagnostic}"
        ))
    })?;
    validate_access_admission(&loan, &admission.access_plan, &admission.profile)
}

/// The owned-access authority replay every model carries: the admission's
/// resources replay verbatim, then the model's own `binding` (content grant
/// or schema correspondence), then its resident observation.
pub(super) fn validate_owned_admission_authority(
    admission: &OwnedAccessAdmission,
    model: ObservationModel,
    binding: impl FnOnce(&str) -> Result<(), AccessPlanDiagnostic>,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    let resources = replay_owned_admission_resources(admission).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained access authority: {diagnostic}"
        ))
    })?;
    if resources != admission.resources {
        return Err(AccessPlanDiagnostic(format!(
            "{transition} replayed resource compatibility differs from the retained admission"
        )));
    }
    binding(transition)?;
    validate_resident_observation(&admission.access_plan, model, transition)
}

pub(super) fn validate_owned_resident_authority(
    admission: &OwnedAccessAdmission,
    content: &ProviderExistingContentGrant,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    validate_owned_admission_authority(
        admission,
        ObservationModel::Stable,
        |transition| {
            validate_owned_content_binding(admission, content).map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "{transition} could not replay the retained provider content grant: {diagnostic}"
                ))
            })
        },
        transition,
    )
}
