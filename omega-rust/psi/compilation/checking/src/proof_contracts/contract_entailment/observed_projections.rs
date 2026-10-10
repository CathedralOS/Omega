//! Declared projections of already completed values.
//!
//! Field symbols select the ordinary qualified projection before the shared
//! term algebra reads its contents. An owned result can expose owned fields;
//! a returned reference descriptor cannot supply a referent observation.

use super::structural_judgment::StructuralTerm;
use facts::PlaceSegment;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn project_observed_value(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    mut value: StructuralTerm,
    path: &[PlaceSegment],
) -> Option<StructuralTerm> {
    for segment in path {
        if crate::reference_shell(program, reference).is_some()
            && !super::structural_terms::scalar_operand_is_observed(program, reference, &value)
        {
            return None;
        }
        let projected = crate::value_custody::places::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        )?;
        match segment {
            PlaceSegment::Field { symbol }
                if symbol.is_valid() && program.any_field_owner(*symbol).is_some() =>
            {
                value = project_field(value, program.symbols.name(*symbol))?;
            }
            PlaceSegment::Case { variant }
                if variant.is_valid() && program.variant_owner(*variant).is_some() =>
            {
                let StructuralTerm::Constructor { case, .. } = value.semantic_value() else {
                    return None;
                };
                if case != program.symbols.name(*variant) {
                    return None;
                }
            }
            _ => return None,
        }
        reference = projected;
    }
    Some(value)
}

pub(super) fn project_field(value: StructuralTerm, field_name: &str) -> Option<StructuralTerm> {
    match value.into_semantic_value() {
        StructuralTerm::Constructor { fields, .. } => fields
            .into_iter()
            .find_map(|(name, value)| (name == field_name).then_some(value)),
        StructuralTerm::Variable(name) | StructuralTerm::Opaque(name) => {
            Some(StructuralTerm::Opaque(format!("{name}.{field_name}")))
        }
        StructuralTerm::BoundValue(subject) => Some(StructuralTerm::BoundProjection {
            subject,
            path: field_name.to_owned(),
        }),
        StructuralTerm::BoundProjection { subject, path } => {
            Some(StructuralTerm::BoundProjection {
                subject,
                path: format!("{path}.{field_name}"),
            })
        }
        StructuralTerm::Projection { subject, path } => Some(StructuralTerm::Projection {
            subject,
            path: format!("{path}.{field_name}"),
        }),
        value @ (StructuralTerm::CapturedValue { .. }
        | StructuralTerm::Application { .. }
        | StructuralTerm::CallProjection { .. }) => Some(StructuralTerm::Projection {
            subject: Box::new(value),
            path: field_name.to_owned(),
        }),
        StructuralTerm::Integer(_)
        | StructuralTerm::Float(_)
        | StructuralTerm::ScalarBinary { .. }
        | StructuralTerm::ObservedValue { .. } => None,
    }
}
