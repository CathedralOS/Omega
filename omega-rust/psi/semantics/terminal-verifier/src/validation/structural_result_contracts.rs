//! Exact qualification custody shared by structural calls and returns.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct StructuralResultSignature<'a> {
    pub(super) structural_type: StructuralTypeId,
    pub(super) multiplicity: StructuralMultiplicity,
    pub(super) qualifications: &'a [StructuralDomainId],
    pub(super) projected_qualifications: &'a [terminal_psi::StructuralPathQualification],
}

pub(super) fn operation_signature(
    result: &terminal_psi::StructuralOperationResult,
) -> StructuralResultSignature<'_> {
    StructuralResultSignature {
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        qualifications: &result.qualifications,
        projected_qualifications: &result.projected_qualifications,
    }
}

pub(super) fn source_signature(
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<StructuralResultSignature<'_>> {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .map(|parameter| StructuralResultSignature {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            qualifications: &parameter.qualifications,
            projected_qualifications: &parameter.projected_qualifications,
        })
        .or_else(|| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| {
                    operation.result.structural().and_then(|result| {
                        (result.place == source).then(|| operation_signature(result))
                    })
                })
        })
}

pub(super) fn matches_function_result(
    signature: StructuralResultSignature<'_>,
    result: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    signature.structural_type == result.structural_type
        && signature.multiplicity == result.multiplicity
        && signature.qualifications == result.qualifications
        && signature.projected_qualifications == result.projected_qualifications
}

pub(super) fn call_result_matches(
    result: &terminal_psi::StructuralOperationResult,
    callee: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    matches_function_result(operation_signature(result), callee)
}

pub(super) fn has_empty_qualification_rosters(
    qualifications: &[StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
) -> bool {
    qualifications.is_empty() && projected.is_empty()
}
