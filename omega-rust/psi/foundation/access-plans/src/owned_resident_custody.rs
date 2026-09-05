use extents::{ExtentLoan, ProviderExistingContentGrant};

use super::{
    AccessPlanDiagnostic, DormantOwnedResident, ObservationModel, OwnedPlacementAdmission,
    OwnedStableAdoptionError, PlacementResourceCompatibility, ValidatedPlacementPlan,
    validate_placement_admission,
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
    admission: OwnedPlacementAdmission,
    content: ProviderExistingContentGrant,
) -> Result<DormantOwnedResident, OwnedStableAdoptionError> {
    let diagnostic = validate_owned_stable_adoption(&admission, &content);
    if let Err(diagnostic) = diagnostic {
        return Err(OwnedStableAdoptionError {
            admission,
            content,
            diagnostic,
        });
    }
    Ok(DormantOwnedResident { admission, content })
}

fn validate_owned_stable_adoption(
    admission: &OwnedPlacementAdmission,
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
        &admission.placement_plan,
        ObservationModel::Stable,
        "Stable adoption",
    )
}

pub(super) fn validate_owned_content_binding(
    admission: &OwnedPlacementAdmission,
    content: &ProviderExistingContentGrant,
) -> Result<(), AccessPlanDiagnostic> {
    let extent = &admission.extent;
    let loan = extent.loan(0, extent.length()).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "owned content binding could not replay its whole-range loan: {diagnostic}"
        ))
    })?;
    validate_provider_content_binding(&admission.placement_plan, &loan, content)
}

pub(super) fn validate_provider_content_binding(
    plan: &ValidatedPlacementPlan,
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
    plan: &ValidatedPlacementPlan,
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
    admission: &OwnedPlacementAdmission,
) -> Result<PlacementResourceCompatibility, AccessPlanDiagnostic> {
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
    validate_placement_admission(&loan, &admission.placement_plan, &admission.profile)
}

pub(super) fn validate_owned_resident_authority(
    admission: &OwnedPlacementAdmission,
    content: &ProviderExistingContentGrant,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    let resources = replay_owned_admission_resources(admission).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained placement authority: {diagnostic}"
        ))
    })?;
    if resources != admission.resources {
        return Err(AccessPlanDiagnostic(format!(
            "{transition} replayed resource compatibility differs from the retained admission"
        )));
    }
    validate_owned_content_binding(admission, content).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained provider content grant: {diagnostic}"
        ))
    })?;
    validate_resident_observation(
        &admission.placement_plan,
        ObservationModel::Stable,
        transition,
    )
}
