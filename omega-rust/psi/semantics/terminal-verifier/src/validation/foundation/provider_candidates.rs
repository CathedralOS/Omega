//! The module's provider candidates: ordering, subjects, requirement
//! identities and signatures.

use super::super::{
    BTreeMap, BTreeSet, BoundaryContentGuarantee, BoundaryMachineId, ModuleError, StructuralTypeId,
    TerminalModule,
};
use super::provider_signature_parameter;
use terminal_psi::StructuralTypeDeclaration;

/// Validates the module's provider candidates: rows are canonically
/// ordered, each names a known boundary and candidate machine, subjects
/// are distinct, requirement identities agree per boundary, and signatures
/// name known types.
pub(super) fn validate_provider_candidates(
    module: &TerminalModule,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), ModuleError> {
    if let Some(pair) = module.provider_candidates.windows(2).find(|pair| {
        (
            pair[0].boundary,
            pair[0].provider_identity.as_str(),
            pair[0].candidate_identity.as_str(),
            pair[0].candidate,
        ) >= (
            pair[1].boundary,
            pair[1].provider_identity.as_str(),
            pair[1].candidate_identity.as_str(),
            pair[1].candidate,
        )
    }) {
        let row = &pair[1];
        return Err(ModuleError::InvalidProviderCandidate {
            boundary: row.boundary,
            candidate: row.candidate,
        });
    }
    let mut provider_subjects = BTreeSet::new();
    let mut requirement_identities = BTreeMap::<BoundaryMachineId, &str>::new();
    for row in &module.provider_candidates {
        let Some(boundary) = module
            .boundary_machines
            .iter()
            .find(|boundary| boundary.id == row.boundary)
        else {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        };
        let Some(candidate) = module
            .machines
            .iter()
            .find(|candidate| candidate.id == row.candidate)
        else {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        };
        if row.requirement_identity.is_empty()
            || row.provider_identity.is_empty()
            || row.candidate_identity.is_empty()
            || boundary.identity != row.requirement_identity
            || boundary
                .content_guarantees
                .iter()
                .any(|guarantee| matches!(guarantee, BoundaryContentGuarantee::RetainedBorrow(_)))
            || !provider_subjects.insert((
                row.boundary,
                row.provider_identity.as_str(),
                row.candidate_identity.as_str(),
                row.candidate,
            ))
            || requirement_identities
                .insert(row.boundary, row.requirement_identity.as_str())
                .is_some_and(|identity| identity != row.requirement_identity)
        {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        }
        let Some(attachment) = candidate
            .attachment
            .and_then(|attachment| types.get(&attachment))
        else {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        };
        let boundary_signature = boundary
            .structural_parameters
            .iter()
            .map(provider_signature_parameter)
            .collect::<Vec<_>>();
        let candidate_signature = candidate
            .structural_parameters
            .iter()
            .map(provider_signature_parameter)
            .collect::<Vec<_>>();
        let expected_positions = (0..boundary_signature.len())
            .map(|index| terminal_psi::ProviderParameterRefinement {
                boundary_index: u32::try_from(index).unwrap_or(u32::MAX),
                candidate_index: u32::try_from(index).unwrap_or(u32::MAX),
            })
            .collect::<Vec<_>>();
        let scalar_signature_matches = boundary.scalar_parameters.len()
            == candidate.parameters.len()
            && boundary
                .scalar_parameters
                .iter()
                .zip(&candidate.parameters)
                .all(|(boundary, candidate)| *boundary == candidate.scalar_type);
        if attachment.identity.is_empty()
            || !scalar_signature_matches
            // A crash-free checked candidate refines any may-crash ceiling;
            // guarded candidate routes need the same positional substitution
            // and ceiling coverage that call continuations use.
            || !super::super::crash::provider_crash_routes_refine_boundary(boundary, candidate)
            || !super::provider_result::matches(boundary, candidate)
            || row.signature.parameters != boundary_signature
            || row.signature.parameters != candidate_signature
            || row.refinement.positional_parameters != expected_positions
            || row.refinement.required_domains != boundary.requires
            || row.refinement.realized_service_ceiling != candidate.published_service_ceiling
            || row
                .refinement
                .realized_service_ceiling
                .iter()
                .any(|service| !boundary.published_service_ceiling.contains(service))
        {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        }
    }
    Ok(())
}
