//! Prepare the same selected payload carrier for pure and computed reads.
//!
//! A scalar payload can use its dispatch snapshot directly. A scalar beneath
//! a structural payload instead observes a retained carrier, with the remaining
//! record path relative to that carrier. Canonical source identities resolve
//! the carrier here; its copy operation still owes live storage and selected
//! case authority at execution. This never grants authored affine duplication.

use semantic_vocabulary::CanonicalStructuralPathSegment;

use super::{LoweringError, PlaceId, StructuralTypeDeclaration, StructuralTypeId, unsupported};
use crate::emission::operation_emission::expressions::PrimitiveReadSource;

pub(crate) struct PreparedCaseObservation {
    pub(crate) source: PrimitiveReadSource,
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
    pub(crate) captured_prefix_length: usize,
}

pub(crate) fn prepare(
    source: PlaceId,
    root_type: StructuralTypeId,
    carrier_path: &[CanonicalStructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Result<PreparedCaseObservation, LoweringError> {
    let first_case = carrier_path
        .iter()
        .position(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)));
    let Some(case_position) = first_case.filter(|position| position + 1 < carrier_path.len())
    else {
        return Ok(PreparedCaseObservation {
            source: source.into(),
            path: carrier_path.to_vec(),
            captured_prefix_length: 0,
        });
    };
    if !matches!(
        carrier_path[case_position + 1],
        CanonicalStructuralPathSegment::Field(_)
    ) {
        return unsupported("case observation requires its declared payload field");
    }
    let captured_prefix_length = case_position + 2;
    let captured_path = &carrier_path[..captured_prefix_length];
    let structural_type =
        terminal_semantics::canonical_case_leaf_type(declarations.iter(), root_type, captured_path)
            .ok_or(LoweringError::Unsupported(
                "case observation lost its exact structural payload carrier",
            ))?;
    Ok(PreparedCaseObservation {
        source: PrimitiveReadSource::CaseLeaf {
            source,
            path: captured_path.to_vec(),
            structural_type,
        },
        path: carrier_path[captured_prefix_length..].to_vec(),
        captured_prefix_length,
    })
}
