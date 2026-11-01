//! Join each emitted scalar field to its exact checked receiving type.
//! The source type plan and Terminal declaration already own the association;
//! do not rediscover domain identity from a numeric restriction or field name.

use super::{CheckedTrees, CheckedUnitStructuralTypeShape, LoweringError, UnitPlans};
use crate::expression_preparation::qualifications::FieldType;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTableBuilder};
use typed_trees::type_identity::TypeIdentityId;

pub(in crate::unit::attached_unit) fn scalar_field_types(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    type_ids: &[(TypeIdentityId, StructuralTypeId)],
    declarations: &[StructuralTypeDeclaration],
) -> Result<Vec<FieldType>, LoweringError> {
    let mut result = Vec::new();
    for (source, target) in type_ids {
        // Pointer transport can name an identity-only referent. It owns no
        // stored fields; expanding it here would undo finite type collection.
        let Some(declaration) = declarations.iter().find(|row| row.id == *target) else {
            continue;
        };
        let plan = super::selected_declaration(plans, *source)?;
        let (fields, cases) = match &plan.shape {
            CheckedUnitStructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
            CheckedUnitStructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                (fields.as_slice(), cases.as_slice())
            }
            _ => continue,
        };
        let (emitted, emitted_cases) = match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
            StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases.as_slice()),
            _ => {
                return Err(LoweringError::Unsupported(
                    "scalar field owner changed shape",
                ));
            }
        };
        for field in fields
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
        {
            if field.relevance.is_erased()
                || checked
                    .primitive_type_reference(field.type_reference)
                    .is_none()
            {
                continue;
            }
            let identity = identities.intern(&field.identity.spelling());
            let emitted = emitted
                .iter()
                .chain(emitted_cases.iter().flat_map(|case| &case.fields))
                .find(|row| row.identity == identity)
                .ok_or(LoweringError::Unsupported(
                    "scalar field lost its exact declaration",
                ))?;
            if emitted.field_type.scalar_type().is_some() {
                result.push((emitted.id, field.type_reference));
            }
        }
    }
    Ok(result)
}
