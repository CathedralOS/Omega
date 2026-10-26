//! A `[copy]` case payload (or a subtree beneath it) transferred into a
//! successor state mints one `StructuralCaseLeafCopy` on the selected edge:
//! the canonical op path walks the subject's own projection, enters the case
//! the dominating control flow proved, and resolves the payload subtree to an
//! owned `Unrestricted` leaf. The borrowed subject's custody stays intact —
//! copying observes contents like a scalar field read.
use super::super::super::{
    Operation, OperationKind, OperationResult, PlaceId, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, unsupported,
};
use super::super::LoweringError;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::structural_observations;
use checked_trees::{
    CheckedMemberIdentity, CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
};
use language_semantics::Multiplicity;
use terminal_psi::StructuralTypeDeclaration;

/// Emit the copy into `operations` and return the owned place declaration the
/// successor argument names directly. `source` is the subject's current place
/// after edge staging and `root_type` its declared structural type.
pub(super) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    types: &[StructuralTypeDeclaration],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    source: PlaceId,
    root_type: StructuralTypeId,
    subject_path: &[CheckedUnitStructuralPathSegment],
    case_identity: &CheckedMemberIdentity,
    field_identity: &CheckedMemberIdentity,
    payload_path: &[CheckedUnitStructuralPathSegment],
    target: &CheckedUnitStructuralParameterPlan,
    destination: PlaceId,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut path = Vec::with_capacity(subject_path.len() + payload_path.len() + 2);
    path.extend_from_slice(subject_path);
    path.push(CheckedUnitStructuralPathSegment::Case(
        case_identity.clone(),
    ));
    path.push(CheckedUnitStructuralPathSegment::Field(
        field_identity.clone(),
    ));
    path.extend_from_slice(payload_path);
    // Resolve the whole proven case path once. Intermediate reference loads
    // stay explicit; a reference-valued payload endpoint keeps its descriptor.
    let selected = structural_observations::stored_value_path(identities, root_type, &path, types)?;
    let canonical = selected
        .static_observation_path()
        .ok_or(LoweringError::Unsupported(
            "Unit graph case-payload copy requires static observation coordinates",
        ))?;
    let leaf = selected
        .field_type()?
        .as_structural()
        .ok_or(LoweringError::Unsupported(
            "Unit graph case payload is not a structural leaf",
        ))?;
    // Affine payloads use rooted projected transfers, preserving their one
    // ownership obligation. This operation only materializes copyable data.
    if target.multiplicity != Multiplicity::Unrestricted {
        return unsupported("case leaf observation requires copyable payload custody");
    }
    // A payload's memberships are already carried under its selected case
    // path. Relocate those exact source rows onto the new copy. The receiving
    // parameter cannot supply them, and an empty result roster would erase
    // them before the ordinary state-argument binder checks its contract.
    let (whole, projected) = if let Some(parameter) = parameters
        .iter()
        .find(|parameter| parameter.place == source)
    {
        (
            &parameter.qualifications,
            &parameter.projected_qualifications,
        )
    } else {
        let result =
            operations
                .structural_result_contract(source)
                .ok_or(LoweringError::Unsupported(
                    "case leaf copy lost its source membership contract",
                ))?;
        (&result.qualifications, &result.projected_qualifications)
    };
    let storage_path =
        terminal_semantics::canonical_structural_storage_path(types.iter(), root_type, &canonical)
            .ok_or(LoweringError::Unsupported(
                "case leaf copy lost its exact storage path",
            ))?;
    let (qualifications, projected_qualifications) =
        terminal_psi::StructuralPathQualification::select(whole, projected, &storage_path);
    let producer = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: destination,
            structural_type: leaf,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications,
            projected_qualifications,
            claims: Vec::new(),
        }),
        kind: OperationKind::StructuralCaseLeafCopy {
            source,
            path: canonical,
        },
    });
    Ok(StructuralPlaceDeclaration {
        id: destination,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type: leaf,
        },
    })
}
