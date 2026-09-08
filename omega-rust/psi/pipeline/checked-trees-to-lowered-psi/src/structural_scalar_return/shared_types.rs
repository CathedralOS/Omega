//! Rejoin a selected callee's type closure in the caller's allocated namespace.

use super::*;

pub(super) fn validate(
    plans: &[CheckedUnitStructuralTypePlan],
    shared: &[StructuralTypeDeclaration],
    callee: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<(), LoweringError> {
    let mut pending = callee
        .attachment_type_identity
        .iter()
        .map(String::as_str)
        .chain(
            callee
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.as_str()),
        )
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let type_ids = shared
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    while let Some(identity) = pending.pop() {
        if !visited.insert(identity) {
            continue;
        }
        let mut selected = plans.iter().filter(|plan| plan.identity == identity);
        let mut declarations = shared
            .iter()
            .filter(|declaration| declaration.identity == identity);
        let (Some(plan), Some(declaration)) = (selected.next(), declarations.next()) else {
            return unsupported(
                "shared Unit structural catalog is missing a scalar realization type",
            );
        };
        if identity.is_empty() || selected.next().is_some() || declarations.next().is_some() {
            return unsupported("scalar realization type identity is ambiguous");
        }
        // Reuse shape lowering with the shared type IDs and this declaration's
        // field/case allocation. Independently numbered catalogs are not equal
        // merely because their local IDs coincide.
        let (mut next_field, mut next_case) = match &declaration.shape {
            StructuralTypeShape::Record { fields } => {
                (fields.first().map(|field| field.id.get()).unwrap_or(1), 1)
            }
            StructuralTypeShape::Sum { cases } => (
                cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .next()
                    .map(|field| field.id.get())
                    .unwrap_or(1),
                cases.first().map(|case| case.id.get()).unwrap_or(1),
            ),
            StructuralTypeShape::Mixed { fields, cases } => (
                fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                    .next()
                    .map(|field| field.id.get())
                    .unwrap_or(1),
                cases.first().map(|case| case.id.get()).unwrap_or(1),
            ),
            _ => (1, 1),
        };
        let expected = match &plan.shape {
            CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive) => {
                StructuralTypeShape::PrimitiveScalar(terminal_scalar_type(*primitive)?)
            }
            CheckedUnitStructuralTypeShape::ByteSequence(carrier) => {
                StructuralTypeShape::ByteSequence(terminal_byte_sequence_carrier(*carrier))
            }
            CheckedUnitStructuralTypeShape::Record { fields } => {
                retain_fields(fields, &mut pending);
                StructuralTypeShape::Record {
                    fields: lower_mixed_fields(fields, &type_ids, &mut next_field)?,
                }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } => {
                pending.push(element_type_identity);
                StructuralTypeShape::FixedArray {
                    element: lookup_type_id(&type_ids, element_type_identity)?,
                    length: *length,
                }
            }
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                for case in cases {
                    retain_fields(&case.fields, &mut pending);
                }
                StructuralTypeShape::Sum {
                    cases: lower_mixed_cases(cases, &type_ids, &mut next_field, &mut next_case)?,
                }
            }
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                retain_fields(fields, &mut pending);
                for case in cases {
                    retain_fields(&case.fields, &mut pending);
                }
                StructuralTypeShape::Mixed {
                    fields: lower_mixed_fields(fields, &type_ids, &mut next_field)?,
                    cases: lower_mixed_cases(cases, &type_ids, &mut next_field, &mut next_case)?,
                }
            }
        };
        if expected != declaration.shape {
            return unsupported(
                "shared Unit structural catalog disagrees with a scalar realization type",
            );
        }
    }
    Ok(())
}

fn retain_fields<'plan>(
    fields: &'plan [checked_trees::CheckedUnitStructuralFieldPlan],
    pending: &mut Vec<&'plan str>,
) {
    for field in fields {
        if let CheckedUnitStructuralFieldType::Structural { type_identity } = &field.field_type {
            pending.push(type_identity);
        }
    }
}
