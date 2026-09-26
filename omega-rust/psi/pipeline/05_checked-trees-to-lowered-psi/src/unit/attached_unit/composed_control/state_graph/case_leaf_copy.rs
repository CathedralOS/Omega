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
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration};

/// Emit the copy into `operations` and return the owned place declaration the
/// successor argument names directly. `source` is the subject's current place
/// after edge staging and `root_type` its declared structural type; the leaf
/// type the walk resolves must declare the target parameter's exact identity.
pub(super) fn emit(
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
    root_type: StructuralTypeId,
    subject_path: &[CheckedUnitStructuralPathSegment],
    subject_identity: &str,
    case_identity: &str,
    field_identity: &str,
    payload_path: &[CheckedUnitStructuralPathSegment],
    target: &CheckedUnitStructuralParameterPlan,
    destination: PlaceId,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut canonical = Vec::with_capacity(subject_path.len() + payload_path.len() + 2);
    let mut visited = Vec::new();
    let projected = walk_fields(types, &mut canonical, &mut visited, root_type, subject_path)?;
    // The projected subject must name the declared sum the transfer planned
    // against, and the case step enters exactly the proven case's payloads.
    let declaration = unique_type(types, projected)?;
    if declaration.identity != subject_identity {
        return unsupported("Unit graph case-payload subject type drifted");
    }
    let cases = match &declaration.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return unsupported("Unit graph case-payload subject is not a sum"),
    };
    let selected = cases
        .iter()
        .find(|case| case.identity == case_identity)
        .ok_or(LoweringError::Unsupported(
            "Unit graph case-payload case is absent",
        ))?;
    canonical.push(CanonicalStructuralPathSegment::Case(selected.id));
    let payload = selected
        .fields
        .iter()
        .find(|field| field.identity == field_identity && !field.relevance.is_erased())
        .ok_or(LoweringError::Unsupported(
            "Unit graph case-payload field is absent",
        ))?;
    canonical.push(CanonicalStructuralPathSegment::Field(payload.id));
    let StructuralFieldType::Structural(leaf) = payload.field_type else {
        return unsupported("Unit graph case payload is not a structural leaf");
    };
    let leaf = walk_fields(types, &mut canonical, &mut visited, leaf, payload_path)?;
    // The resolved leaf must be the exact `Unrestricted` type the target
    // state declared — the op mints an unqualified owned copy of that type.
    if unique_type(types, leaf)?.identity != target.type_identity
        || target.multiplicity != Multiplicity::Unrestricted
        || !target.qualifications.is_empty()
        || !target.projected_qualifications.is_empty()
    {
        return unsupported("Unit graph case-payload target drifted");
    }
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
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
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
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = matching.next().ok_or(LoweringError::Unsupported(
                    "Unit graph case-payload field is absent",
                ))?;
                if matching.next().is_some() || field.relevance.is_erased() {
                    return unsupported("Unit graph case-payload field is erased or ambiguous");
                }
                canonical.push(CanonicalStructuralPathSegment::Field(field.id));
                let StructuralFieldType::Structural(child) = field.field_type else {
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
