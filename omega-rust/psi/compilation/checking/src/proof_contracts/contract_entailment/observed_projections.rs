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
        let mut crossing = reference;
        let mut crossed = Vec::new();
        while let Some(shell) = crate::reference_shell(program, crossing) {
            if crossed.contains(&crossing) {
                return None;
            }
            crossed.push(crossing);
            if !shell.access.is_readable()
                || !super::structural_terms::scalar_operand_is_observed(program, crossing, &value)
                || shell.referee == crossing
            {
                return None;
            }
            crossing = shell.referee;
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
                let (owner, _) = program.variant_owner(*variant)?;
                let carrier =
                    crate::value_custody::places::unwrapped_type_reference(program, reference)?;
                if crate::value_custody::type_shapes::head_data_symbol(program, carrier)
                    != Some(owner.symbol)
                    || case.as_str() != program.symbols.name(*variant)
                {
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
        StructuralTerm::Variable(name) => Some(StructuralTerm::Projection {
            subject: Box::new(StructuralTerm::Variable(name)),
            path: field_name.to_owned(),
        }),
        StructuralTerm::Opaque(name) => {
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
        | StructuralTerm::CallProjection { .. }
        | StructuralTerm::IndexedValue { .. }) => Some(StructuralTerm::Projection {
            subject: Box::new(value),
            path: field_name.to_owned(),
        }),
        StructuralTerm::Integer(_)
        | StructuralTerm::Float(_)
        | StructuralTerm::ScalarBinary { .. }
        | StructuralTerm::MathematicalBinary { .. }
        | StructuralTerm::IntegerEmbedding { .. }
        | StructuralTerm::ObservedValue { .. } => None,
    }
}

/// Authenticate an actual integer data field; collection metadata is not a field.
pub(super) fn declared_integer_field_read(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    let typed_trees::expression::ExpressionNode::Member(member) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    let receiver =
        crate::expression_result_type_reference(program, machine, state, member.receiver)?;
    let data = crate::value_custody::places::data_definition_for_type(program, receiver)?;
    let field = crate::value_custody::places::exact_data_member_field(
        program,
        data,
        member.member_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|case| case.as_str()),
    )?;
    if !field.symbol.is_valid()
        || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
    {
        return None;
    }
    let reference = crate::expression_result_type_reference(program, machine, state, expression)?;
    let primitive = program.primitive_type_reference(reference)?;
    primitive.is_fixed_integer().then_some(primitive)
}
