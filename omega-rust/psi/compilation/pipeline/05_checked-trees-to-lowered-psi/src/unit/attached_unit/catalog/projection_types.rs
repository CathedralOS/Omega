//! Concrete declarations demanded by finite checked paths, whether an operation
//! uses the path or a published signature retains its qualification. Pointer
//! transport alone stays identity-only: following every reference declaration
//! would expand recursive type families without bound. This inventory supplies
//! geometry only, never access, membership, bounds or lifetime authority.
use super::*;

pub(super) fn collect_parameter(
    plans: UnitPlans<'_>,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
) -> Result<(), LoweringError> {
    roots.push(parameter.type_identity);
    collect_qualifications(
        plans,
        parameter.type_identity,
        &parameter.projected_qualifications,
        roots,
    )
}

pub(super) fn collect_qualifications(
    plans: UnitPlans<'_>,
    root: typed_trees::type_identity::TypeIdentityId,
    qualifications: &[checked_trees::CheckedStructuralPathQualification],
    roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
) -> Result<(), LoweringError> {
    for qualification in qualifications {
        collect(plans, root, &qualification.path, roots)?;
    }
    Ok(())
}

pub(super) fn collect(
    plans: UnitPlans<'_>,
    mut identity: typed_trees::type_identity::TypeIdentityId,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
    roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
) -> Result<(), LoweringError> {
    use checked_trees::{
        CheckedUnitStructuralPathSegment as Step, CheckedUnitStructuralTypeShape as Shape,
    };
    let mut selected_case = None;
    roots.push(identity);
    for (position, step) in path.iter().enumerate() {
        let declaration = selected_declaration(plans, identity)?;
        match step {
            Step::Case(selected) => {
                let cases = match &declaration.shape {
                    Shape::Sum { cases } | Shape::Mixed { cases, .. } => cases,
                    _ => return unsupported("projection case lost its declared namespace"),
                };
                selected_case = Some(
                    cases
                        .iter()
                        .find(|case| case.identity == *selected)
                        .ok_or(LoweringError::Unsupported(
                            "projection case declaration missing",
                        ))?
                        .fields
                        .as_slice(),
                );
                continue;
            }
            Step::Field(selected) => {
                let fields = match selected_case.take() {
                    Some(fields) => fields,
                    None => match &declaration.shape {
                        Shape::Record { fields, .. } | Shape::Mixed { fields, .. } => fields,
                        _ => return unsupported("projection field lost its declared container"),
                    },
                };
                let field = fields
                    .iter()
                    .find(|field| field.identity == *selected)
                    .ok_or(LoweringError::Unsupported(
                        "projection field declaration missing",
                    ))?;
                match field.field_type.structural_type_identity() {
                    Some(field_type) => identity = field_type,
                    // Inline scalars have no separate storage declaration, and
                    // cannot contain another projection. Keep the parent shape.
                    None if position + 1 == path.len() => return Ok(()),
                    None => return unsupported("projection crosses a nonstructural field"),
                }
            }
            Step::Referent => {
                let Shape::Reference {
                    referent_identity, ..
                } = &declaration.shape
                else {
                    return unsupported("projection pointer crossing lost its descriptor");
                };
                identity = *referent_identity;
            }
            Step::FixedIndex(_) | Step::AllElements => {
                identity = match &declaration.shape {
                    Shape::FixedArray {
                        element_type_identity,
                        ..
                    }
                    | Shape::BorrowedSliceView {
                        element_type_identity,
                        ..
                    } => *element_type_identity,
                    _ => return unsupported("projection index lost its collection type"),
                };
            }
            Step::RepresentationView { type_identity, .. } => identity = *type_identity,
            _ => return unsupported("projection type demand has no static storage path"),
        }
        roots.push(identity);
    }
    Ok(())
}
