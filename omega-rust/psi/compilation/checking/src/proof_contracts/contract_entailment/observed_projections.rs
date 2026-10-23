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
        let owned = super::structural_terms::owned_observation_type(program, &value).is_some();
        if owned && !super::structural_terms::scalar_operand_is_observed(program, reference, &value)
        {
            return None;
        }
        readable_projection_crossings(program, reference, &value)?;
        let projected = crate::value_custody::places::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        )?;
        match segment {
            PlaceSegment::Field { symbol }
                if symbol.is_valid() && program.any_field_owner(*symbol).is_some() =>
            {
                value = project_field(
                    if owned {
                        value.into_semantic_value()
                    } else {
                        value
                    },
                    program.symbols.name(*symbol),
                )?;
                if owned {
                    value = super::structural_terms::owned_projection_observation(
                        program, projected, value,
                    );
                }
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
    // This untyped helper cannot authorize crossing an observation shell.
    // The typed path owner above unwraps owned inline storage and masks the
    // selected reference child before any subsequent projection.
    if matches!(value, StructuralTerm::ObservedValue { .. }) {
        return None;
    }
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
        StructuralTerm::CollectionMeasure { .. }
        | StructuralTerm::Integer(_)
        | StructuralTerm::Float(_)
        | StructuralTerm::ScalarBinary { .. }
        | StructuralTerm::MathematicalBinary { .. }
        | StructuralTerm::IntegerEmbedding { .. }
        | StructuralTerm::ObservedValue { .. } => None,
    }
}

/// A substituted contract retains the owned argument's declared shell. Resolve
/// its field through that declaration just as an executing member expression
/// does; the untyped field algebra cannot authorize this crossing.
pub(super) fn project_declared_field(
    program: &dyn crate::CheckingInput,
    value: StructuralTerm,
    field_name: &str,
) -> Option<StructuralTerm> {
    let Some(reference) = super::structural_terms::owned_observation_type(program, &value) else {
        return project_field(value, field_name);
    };
    let data = crate::value_custody::places::data_definition_for_type(program, reference)?;
    let mut fields = program
        .data_member_fields(program.data_members(data))
        .filter(|field| field.name.as_str() == field_name);
    let (Some(field), None) = (fields.next(), fields.next()) else {
        return None;
    };
    project_observed_value(
        program,
        reference,
        value,
        &[PlaceSegment::Field {
            symbol: field.symbol,
        }],
    )
}

/// Observe an authenticated builtin element of a completed collection value.
/// The caller owns the captured selector and storage revision. A descriptor
/// whose referent was not observed cannot supply contents by projection.
pub(super) fn project_index(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    value: StructuralTerm,
    index: StructuralTerm,
) -> Option<StructuralTerm> {
    readable_projection_crossings(program, reference, &value)?;
    if !super::structural_terms::scalar_operand_is_observed(program, reference, &value) {
        return None;
    }
    let element_type =
        crate::value_custody::places::collection_element_type_reference(program, reference)?;
    let owned = super::structural_terms::owned_observation_type(program, &value).is_some();
    let selected = StructuralTerm::IndexedValue {
        element_type,
        element_identity: program.normalized_type_identity(element_type),
        collection: Box::new(if owned {
            value.into_semantic_value()
        } else {
            value
        }),
        index: Box::new(index),
    };
    Some(if owned {
        super::structural_terms::owned_projection_observation(program, element_type, selected)
    } else {
        selected
    })
}

/// A captured coordinate identifies storage, not permission to observe it.
/// Fields and elements retain the same readable-reference and origin fences.
fn readable_projection_crossings(
    program: &dyn crate::CheckingInput,
    mut crossing: TypeReferenceHandle,
    value: &StructuralTerm,
) -> Option<()> {
    let mut crossed = Vec::new();
    while let Some(shell) = crate::reference_shell(program, crossing) {
        if crossed.contains(&crossing) {
            return None;
        }
        crossed.push(crossing);
        if !shell.access.is_readable()
            || !super::structural_terms::scalar_operand_is_observed(program, crossing, value)
            || shell.referee == crossing
        {
            return None;
        }
        crossing = shell.referee;
    }
    Some(())
}

/// Authenticate an actual scalar data field; collection metadata is not a field.
/// Integers and addresses keep their numeric read epochs. IEEE floats join that
/// roster because a float has no integer relation: the completed read is the
/// observation an exclusive loan must retire, and only an overlapping loan does.
/// This observation carries neither a native-width bound nor storage authority.
pub(super) fn declared_scalar_field_read(
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
    (primitive.is_fixed_integer()
        || primitive == typed_trees::types::PrimitiveType::Addr
        || matches!(
            primitive,
            typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64
        ))
    .then_some(primitive)
}

/// Integer and address subset of [`declared_scalar_field_read`]. Arithmetic
/// observations still exclude IEEE equality and its NaN cases.
pub(super) fn declared_integer_field_read(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    declared_scalar_field_read(program, machine, state, expression).filter(|primitive| {
        primitive.is_fixed_integer() || *primitive == typed_trees::types::PrimitiveType::Addr
    })
}
