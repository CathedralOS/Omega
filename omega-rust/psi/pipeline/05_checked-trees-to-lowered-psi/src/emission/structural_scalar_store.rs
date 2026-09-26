//! Shared checked custody replay for scalar stores through structural fields.

use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedUnitStructuralPathSegment,
    LoweringError, OperationKind, PlaceId, PrimitiveType, ScalarType, StructuralAccess,
    StructuralFieldId, StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    ValueId, allocate_dense, obligation_id, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::runtime_elements::ProjectionStep;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreAccessPolicy {
    MutableOnly,
    Exclusive,
}

pub(crate) struct LoweredStructuralScalarStore {
    /// Carrier steps from the destination root; a runtime element's selector
    /// is evaluated by the emitter before the stored value.
    pub steps: Vec<ProjectionStep>,
    pub field: StructuralFieldId,
    pub scalar_type: ScalarType,
    pub requires_range_obligation: bool,
}

impl LoweredStructuralScalarStore {
    /// The static carrier path, for routes that evaluate no selector.
    pub(crate) fn static_path(&self) -> Result<Vec<StructuralPathSegment>, LoweringError> {
        crate::emission::runtime_elements::static_segments(self.steps.clone())
    }

    /// Each replacement requests its own declaration-derived range proof against
    /// the completed RHS. Neither an earlier read nor the root's initial validity
    /// can stand in for proving the value about to be stored. `indexes` are the
    /// carrier's evaluated runtime elements, in path order.
    pub(crate) fn into_operation(
        self,
        destination: PlaceId,
        indexes: Vec<ValueId>,
        value: ValueId,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<OperationKind, LoweringError> {
        let range_obligation = if self.requires_range_obligation {
            Some(obligation_id(allocate_dense(
                &mut calls.next_obligation_identity,
            )?))
        } else {
            None
        };
        Ok(OperationKind::StructuralScalarFieldStore {
            destination,
            path: crate::emission::runtime_elements::complete_path(self.steps, indexes, calls)?,
            field: self.field,
            value,
            range_obligation,
        })
    }
}

pub(crate) fn lower_structural_scalar_store_destination(
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    expected_statement_index: u32,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[StructuralTypeDeclaration],
    scalar_parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
    available_scalar_types: &[ScalarType],
    access_policy: StoreAccessPolicy,
) -> Result<LoweredStructuralScalarStore, LoweringError> {
    if !checked_store_source_matches(
        store.value.as_pure().ok_or(LoweringError::Unsupported(
            "this structural scalar store route requires a pure RHS",
        ))?,
        store.primitive_type,
        scalar_parameters,
        available_scalar_types,
    ) {
        return unsupported("structural scalar store lost exact exclusive custody");
    }
    lower_structural_scalar_store_place(
        store,
        expected_statement_index,
        parameter,
        structural_types,
        access_policy,
    )
}

/// Resolve the destination independently of the ordered source expression.
/// Callers must validate and emit that expression in its current scalar namespace.
pub(crate) fn lower_structural_scalar_store_place(
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    expected_statement_index: u32,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[StructuralTypeDeclaration],
    access_policy: StoreAccessPolicy,
) -> Result<LoweredStructuralScalarStore, LoweringError> {
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    let (steps, field) = lower_structural_field_place_steps(
        store.statement_index,
        expected_statement_index,
        store
            .destination
            .parameter_position()
            .ok_or(LoweringError::Unsupported(
                "parameter store route cannot substitute a local destination",
            ))?,
        &store.carrier_path,
        &store.field_identity,
        parameter,
        structural_types,
        access_policy,
    )?;
    if field.field_type.scalar_type() != Some(scalar_type) {
        return unsupported("structural scalar store field has a different type");
    }
    Ok(LoweredStructuralScalarStore {
        steps,
        field: field.id,
        scalar_type,
        requires_range_obligation: matches!(
            field.field_type,
            StructuralFieldType::BoundedInteger(_)
        ),
    })
}

/// Reconstruct the exact carrier and relevant field independently of payload
/// type, for routes whose carrier names no runtime element.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_structural_field_place<'a>(
    statement_index: u32,
    expected_statement_index: u32,
    destination_parameter_position: u32,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    parameter: &StructuralParameterDeclaration,
    structural_types: &'a [StructuralTypeDeclaration],
    access_policy: StoreAccessPolicy,
) -> Result<
    (
        Vec<StructuralPathSegment>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let (steps, field) = lower_structural_field_place_steps(
        statement_index,
        expected_statement_index,
        destination_parameter_position,
        carrier_path,
        field_identity,
        parameter,
        structural_types,
        access_policy,
    )?;
    Ok((
        crate::emission::runtime_elements::static_segments(steps)?,
        field,
    ))
}

/// Reconstruct the exact carrier and relevant field independently of payload type.
#[allow(clippy::too_many_arguments)]
fn lower_structural_field_place_steps<'a>(
    statement_index: u32,
    expected_statement_index: u32,
    destination_parameter_position: u32,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    parameter: &StructuralParameterDeclaration,
    structural_types: &'a [StructuralTypeDeclaration],
    access_policy: StoreAccessPolicy,
) -> Result<
    (
        Vec<ProjectionStep>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
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
        || statement_index != expected_statement_index
        || destination_parameter_position != parameter.position
    {
        return unsupported("structural scalar store lost exact exclusive custody");
    }
    lower_structural_field_steps(
        parameter.structural_type,
        carrier_path,
        field_identity,
        structural_types,
    )
}

/// Shared geometry follows exact declarations; callers separately establish the
/// root's current ownership or exclusive-borrow authority. The carrier names
/// no runtime element.
pub(crate) fn lower_structural_field_path<'a>(
    root_type: StructuralTypeId,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<StructuralPathSegment>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let (steps, field) =
        lower_structural_field_steps(root_type, carrier_path, field_identity, structural_types)?;
    Ok((
        crate::emission::runtime_elements::static_segments(steps)?,
        field,
    ))
}

/// Walk a carrier path of record fields and fixed-array elements, literal or
/// the assignment's runtime elements, in any order, to the record that
/// declares `field_identity`.
fn lower_structural_field_steps<'a>(
    root_type: StructuralTypeId,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<ProjectionStep>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let declaration = structural_types
        .iter()
        .find(|declaration| declaration.id == root_type)
        .ok_or(LoweringError::Unsupported(
            "structural scalar store root type is absent",
        ))?;
    let mut field_owner = declaration;
    let mut steps = Vec::with_capacity(carrier_path.len());
    for segment in carrier_path {
        let nested = match segment {
            CheckedUnitStructuralPathSegment::Field(identity) if !identity.is_empty() => {
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
                steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                    identity.clone(),
                )));
                nested
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = &field_owner.shape else {
                    return unsupported("structural scalar store carrier is not a fixed array");
                };
                if *index >= *length {
                    return unsupported("structural scalar store fixed index is out of bounds");
                }
                steps.push(ProjectionStep::Static(StructuralPathSegment::FixedIndex(
                    *index,
                )));
                *element
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
            ) if !steps.contains(&ProjectionStep::AssignmentIndex { depth: *depth }) => {
                let StructuralTypeShape::FixedArray { element, .. } = &field_owner.shape else {
                    return unsupported("structural scalar store carrier is not a fixed array");
                };
                steps.push(ProjectionStep::AssignmentIndex { depth: *depth });
                *element
            }
            _ => return unsupported("structural scalar store carrier path is unsupported"),
        };
        field_owner = structural_types
            .iter()
            .find(|candidate| candidate.id == nested)
            .ok_or(LoweringError::Unsupported(
                "structural scalar store nested carrier type is absent",
            ))?;
    }
    let StructuralTypeShape::Record { fields } = &field_owner.shape else {
        return unsupported("structural scalar store field owner is not a record");
    };
    let matching = fields
        .iter()
        .filter(|field| field.identity == field_identity && !field.relevance.is_erased())
        .collect::<Vec<_>>();
    let [field] = matching.as_slice() else {
        return unsupported("structural scalar store field is absent or ambiguous");
    };
    Ok((steps, field))
}

fn checked_store_source_matches(
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
    scalar_parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
    available_scalar_types: &[ScalarType],
) -> bool {
    if checked_store_literal_matches(value, primitive_type) {
        return true;
    }
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

pub(crate) fn checked_store_literal_matches(
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> bool {
    match (value, primitive_type) {
        (CheckedScalarExpression::IeeeFloatLiteral { value }, primitive_type) => {
            terminal_scalar_type(primitive_type).ok() == Some(ScalarType::IeeeFloat(value.format()))
        }
        (CheckedScalarExpression::IntegerLiteral { .. }, primitive_type) => {
            primitive_type.accepts_integer_literal()
        }
        (CheckedScalarExpression::Boolean(boolean), PrimitiveType::Bool) => {
            matches!(boolean.as_ref(), CheckedBooleanExpression::Constant(_))
        }
        _ => false,
    }
}
