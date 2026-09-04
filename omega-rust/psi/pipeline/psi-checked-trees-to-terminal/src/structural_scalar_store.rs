//! Shared checked custody replay for scalar stores through structural fields.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StoreAccessPolicy {
    MutableOnly,
    Exclusive,
}

pub(super) struct LoweredStructuralScalarStore {
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
    pub scalar_type: ScalarType,
}

pub(super) fn lower_structural_scalar_store_destination(
    store: &psi_checked_trees::CheckedStructuralScalarFieldStorePlan,
    expected_statement_index: u32,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[StructuralTypeDeclaration],
    scalar_parameters: &[psi_checked_trees::CheckedStructuralScalarParameterPlan],
    available_scalar_types: &[ScalarType],
    access_policy: StoreAccessPolicy,
) -> Result<LoweredStructuralScalarStore, LoweringError> {
    let access_matches = match access_policy {
        StoreAccessPolicy::MutableOnly => parameter.access == StructuralAccess::MutableBorrow,
        StoreAccessPolicy::Exclusive => matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ),
    };
    if !access_matches
        || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || store.statement_index != expected_statement_index
        || store.destination_parameter_position != parameter.position
        || !checked_store_source_matches(
            &store.value,
            store.primitive_type,
            scalar_parameters,
            available_scalar_types,
        )
    {
        return unsupported("structural scalar store lost exact exclusive custody");
    }
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    let declaration = structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
        .ok_or(LoweringError::Unsupported(
            "structural scalar store root type is absent",
        ))?;
    let mut field_owner = declaration;
    let mut path = Vec::with_capacity(store.carrier_path.len());
    for segment in &store.carrier_path {
        let CheckedUnitStructuralPathSegment::Field(identity) = segment else {
            return unsupported("structural scalar store carrier path is unsupported");
        };
        if identity.is_empty() {
            return unsupported("structural scalar store carrier path is unsupported");
        }
        let StructuralTypeShape::Record { fields } = &field_owner.shape else {
            return unsupported("structural scalar store carrier is not a record");
        };
        let carriers = fields
            .iter()
            .filter(|field| {
                field.identity == *identity
                    && !field.relevance.is_erased()
                    && matches!(field.field_type, StructuralFieldType::Structural(_))
            })
            .collect::<Vec<_>>();
        let [carrier] = carriers.as_slice() else {
            return unsupported("structural scalar store carrier is absent or ambiguous");
        };
        let StructuralFieldType::Structural(nested) = carrier.field_type else {
            unreachable!("carrier shape was checked above")
        };
        field_owner = structural_types
            .iter()
            .find(|candidate| candidate.id == nested)
            .ok_or(LoweringError::Unsupported(
                "structural scalar store nested carrier type is absent",
            ))?;
        path.push(StructuralPathSegment::Field(identity.clone()));
    }
    let StructuralTypeShape::Record { fields } = &field_owner.shape else {
        return unsupported("structural scalar store field owner is not a record");
    };
    let matching = fields
        .iter()
        .filter(|field| {
            field.identity == store.field_identity
                && !field.relevance.is_erased()
                && field.field_type == StructuralFieldType::Scalar(scalar_type)
        })
        .collect::<Vec<_>>();
    let [field] = matching.as_slice() else {
        return unsupported("structural scalar store field is absent or ambiguous");
    };
    Ok(LoweredStructuralScalarStore {
        path,
        field: field.id,
        scalar_type,
    })
}

fn checked_store_source_matches(
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
    scalar_parameters: &[psi_checked_trees::CheckedStructuralScalarParameterPlan],
    available_scalar_types: &[ScalarType],
) -> bool {
    if let CheckedScalarExpression::Local {
        position,
        primitive_type: source_type,
    } = value
    {
        return scalar_parameters.is_empty()
            && *position == 0
            && *source_type == primitive_type
            && terminal_scalar_type(*source_type).ok()
                == available_scalar_types.get(*position).copied()
            && available_scalar_types.len() == 1
            && matches!(
                primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            );
    }
    if scalar_parameters.is_empty() {
        return checked_store_literal_matches(value, primitive_type);
    }
    let (position, source_type) = match value {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => (*position, *primitive_type),
        CheckedScalarExpression::Boolean(boolean) => {
            let CheckedBooleanExpression::Parameter { position } = boolean.as_ref() else {
                return false;
            };
            (*position, PrimitiveType::Bool)
        }
        _ => return false,
    };
    scalar_parameters.get(position).is_some_and(|parameter| {
        Some(parameter.source_position) == authored_scalar_position(position)
            && parameter.primitive_type == primitive_type
            && source_type == primitive_type
    }) && scalar_parameters
        .iter()
        .enumerate()
        .all(|(index, parameter)| {
            Some(parameter.source_position) == authored_scalar_position(index)
        })
}

fn authored_scalar_position(dense_position: usize) -> Option<u32> {
    u32::try_from(dense_position).ok()?.checked_add(1)
}

pub(super) fn checked_store_literal_matches(
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> bool {
    match (value, primitive_type) {
        (CheckedScalarExpression::IntegerLiteral { .. }, primitive_type) => {
            primitive_type.accepts_integer_literal() && primitive_type != PrimitiveType::Addr
        }
        (CheckedScalarExpression::Boolean(boolean), PrimitiveType::Bool) => {
            matches!(boolean.as_ref(), CheckedBooleanExpression::Constant(_))
        }
        _ => false,
    }
}
