//! Canonical path-indexed structural qualification roster validation.

use super::*;

pub(super) fn validate_projected_qualification_roster(
    place: PlaceId,
    root_type: StructuralTypeId,
    qualifications: &[terminal_psi::StructuralPathQualification],
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> Result<(), ModuleError> {
    if qualifications.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalProjectedStructuralQualifications(
            place,
        ));
    }
    for qualification in qualifications {
        let Some(projected_type) = (!qualification.path.is_empty())
            .then(|| {
                super::foundation::resolve_structural_path_in_types(
                    types,
                    root_type,
                    &qualification.path,
                )
            })
            .flatten()
        else {
            return Err(ModuleError::InvalidProjectedStructuralQualificationPath {
                place,
                path: qualification.path.clone(),
            });
        };
        let Some(domain) = domains.get(&qualification.domain) else {
            return Err(ModuleError::UnknownStructuralDomain(qualification.domain));
        };
        if domain.carrier != projected_type {
            return Err(
                ModuleError::ProjectedStructuralQualificationCarrierMismatch {
                    place,
                    path: qualification.path.clone(),
                    domain: domain.id,
                    expected: projected_type,
                    actual: domain.carrier,
                },
            );
        }
    }
    Ok(())
}
