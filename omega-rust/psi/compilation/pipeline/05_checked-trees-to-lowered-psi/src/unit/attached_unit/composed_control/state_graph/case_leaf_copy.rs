//! A `[copy]` case payload (or a subtree beneath it) transferred into a
//! successor state mints one `StructuralCaseLeafCopy` on the selected edge:
//! the canonical op path walks the subject's own projection, enters the case
//! the dominating control flow proved, and resolves the payload subtree to an
//! owned `Unrestricted` leaf. The borrowed subject's custody stays intact —
//! copying observes contents like a scalar field read.
use super::super::super::{
    Operation, OperationKind, OperationResult, PlaceId, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    unsupported,
};
use super::super::LoweringError;
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::{CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment};
use language_semantics::Multiplicity;
use semantic_vocabulary::CanonicalStructuralPathSegment;
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
    case_identity: &str,
    field_identity: &str,
    payload_path: &[CheckedUnitStructuralPathSegment],
    target: &CheckedUnitStructuralParameterPlan,
    destination: PlaceId,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut canonical = Vec::with_capacity(subject_path.len() + payload_path.len() + 2);
    let mut visited = Vec::new();
    let projected = walk_fields(
        identities,
        types,
        &mut canonical,
        &mut visited,
        root_type,
        subject_path,
    )?;
    // The case step enters exactly the proven case's payloads.
    let declaration = unique_type(types, projected)?;
    let cases = match &declaration.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return unsupported("Unit graph case-payload subject is not a sum"),
    };
    let selected =
        terminal_psi::structural_case_by_identity(cases, identities.intern(&case_identity)).ok_or(
            LoweringError::Unsupported("Unit graph case-payload case is absent"),
        )?;
    canonical.push(CanonicalStructuralPathSegment::Case(selected.id));
    let payload =
        terminal_psi::live_field_by_identity(&selected.fields, identities.intern(&field_identity))
            .ok_or(LoweringError::Unsupported(
                "Unit graph case-payload field is absent",
            ))?;
    canonical.push(CanonicalStructuralPathSegment::Field(payload.id));
    let Some(leaf) = payload.field_type.as_structural() else {
        return unsupported("Unit graph case payload is not a structural leaf");
    };
    let leaf = walk_fields(
        identities,
        types,
        &mut canonical,
        &mut visited,
        leaf,
        payload_path,
    )?;
    unique_type(types, leaf)?;
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

/// Walk literal record-field and fixed-index segments, appending the resolved
/// canonical steps and returning the structural type the path selects. A sum
/// carrier mid-path is rejected: entering a payload requires the explicit
/// `Case` step only the proven edge can mint.
fn walk_fields(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    types: &[StructuralTypeDeclaration],
    canonical: &mut Vec<CanonicalStructuralPathSegment>,
    visited: &mut Vec<StructuralTypeId>,
    mut structural_type: StructuralTypeId,
    segments: &[CheckedUnitStructuralPathSegment],
) -> Result<StructuralTypeId, LoweringError> {
    for segment in segments {
        if visited.contains(&structural_type) {
            return unsupported("Unit graph case-payload path has a cyclic carrier");
        }
        visited.push(structural_type);
        let declaration = unique_type(types, structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                CheckedUnitStructuralPathSegment::Field(identity),
                terminal_psi::record_or_mixed_shape_pattern!(fields),
            ) => {
                let mut matching = fields
                    .iter()
                    .filter(|field| field.identity == identities.intern(&identity.spelling()));
                let field = matching.next().ok_or(LoweringError::Unsupported(
                    "Unit graph case-payload field is absent",
                ))?;
                if matching.next().is_some() || field.relevance.is_erased() {
                    return unsupported("Unit graph case-payload field is erased or ambiguous");
                }
                canonical.push(CanonicalStructuralPathSegment::Field(field.id));
                let Some(child) = field.field_type.as_structural() else {
                    return unsupported(
                        "Unit graph case-payload field is not a structural carrier",
                    );
                };
                child
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                canonical.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                *element
            }
            _ => {
                return unsupported("Unit graph case-payload path is incompatible");
            }
        };
    }
    Ok(structural_type)
}

fn unique_type(
    types: &[StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Result<&StructuralTypeDeclaration, LoweringError> {
    let mut matching = types
        .iter()
        .filter(|declaration| declaration.id == identity);
    let declaration = matching.next().ok_or(LoweringError::Unsupported(
        "Unit graph case-payload type is absent",
    ))?;
    if matching.next().is_some() {
        return unsupported("Unit graph case-payload type is ambiguous");
    }
    Ok(declaration)
}
