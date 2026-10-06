//! Primitive-reference store emission shared by Unit and scalar-result bodies.
//! A runtime element's bound travels from range checking through the shared
//! destination path (`runtime_elements.rs`), as for every other store route.
use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitStructuralPathSegment, LoweringError, OperationKind, PlaceId, PrimitiveType,
    ScalarType, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, ValueDeclaration, direct_expression_contains_short_circuit,
    emit_direct_expression, lower_checked_scalar_expression, terminal_scalar_type, unsupported,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::runtime_elements::ProjectionStep;

/// The resolved primitive endpoint is shared by ordinary and composed bodies.
/// Their namespaces choose a parameter or live local; this owner validates the
/// exclusive parameter projection and evaluates exactly one authored RHS.
pub(crate) struct Destination {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<ProjectionStep>,
}

pub(crate) fn parameter_destination(
    parameter: &StructuralParameterDeclaration,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<Destination, LoweringError> {
    if !matches!(
        parameter.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || parameter.multiplicity == StructuralMultiplicity::Linear
        || (path.is_empty() && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
        || !parameter.qualifications.is_empty()
    {
        return unsupported("primitive store parameter lost its exclusive custody");
    }
    let (path, _) = lower_destination_path(parameter.structural_type, path, types)?;
    Ok(Destination {
        place: parameter.place,
        path,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    destination: Destination,
    role: CheckedScalarExpressionRole,
    value: &checked_trees::CheckedCallScalarArgument,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    source_value_count: usize,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<OperationKind, LoweringError> {
    // The runtime elements' selectors are evaluated before the stored value.
    let indexes = super::runtime_elements::evaluate_selectors(
        &destination.path,
        checked,
        machine,
        state,
        statement_index,
        evaluation,
        source_value_count,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        calls,
    )?;
    let value = evaluation.source_value(
        checked,
        machine,
        state,
        statement_index,
        role,
        value,
        source_value_count,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        calls,
    )?;
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        destination: destination.place,
        path: super::runtime_elements::complete_path(destination.path, indexes, calls, operations)?,
        value: value.id,
    })
}

pub(crate) fn emit(
    destination_parameter_index: u32,
    value: &CheckedScalarExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    scalar_parameter_count: usize,
    scalar_values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let destination = parameters
        .get(usize::try_from(destination_parameter_index).map_err(|_| {
            LoweringError::Unsupported("write-only store parameter index exceeds usize")
        })?)
        .ok_or(LoweringError::Unsupported(
            "write-only store names an unknown structural parameter",
        ))?;
    if !matches!(
        destination.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
    {
        return unsupported(
            "write-only store destination lost its exclusive unrestricted unqualified custody",
        );
    }
    let destination_shape = structural_types
        .iter()
        .find(|declaration| declaration.id == destination.structural_type)
        .map(|declaration| &declaration.shape)
        .ok_or(LoweringError::Unsupported(
            "write-only store destination type is absent",
        ))?;
    let StructuralTypeShape::PrimitiveScalar(destination_type) = destination_shape else {
        return unsupported("write-only store destination is not a primitive scalar root");
    };
    let direct_literal = matches!(value, CheckedScalarExpression::IntegerLiteral { .. })
        || matches!(value, CheckedScalarExpression::IeeeFloatLiteral { .. })
        || matches!(
            value,
            CheckedScalarExpression::Boolean(expression)
                if matches!(
                    expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Constant(_)
                )
        );
    let direct_parameter = match value {
        CheckedScalarExpression::Parameter { position, .. } => *position < scalar_parameter_count,
        CheckedScalarExpression::Boolean(expression) => match expression.as_ref() {
            checked_trees::CheckedBooleanExpression::Parameter { position } => {
                *position < scalar_parameter_count
            }
            _ => false,
        },
        _ => false,
    };
    let direct_result_home = matches!(
        value,
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } if *position >= scalar_parameter_count
            && position.checked_add(1) == Some(scalar_values.len())
            && terminal_scalar_type(*primitive_type).ok() == Some(*destination_type)
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
            )
    );
    if !direct_literal && !direct_parameter && !direct_result_home {
        return unsupported("write-only store value is outside the admitted direct scalar rung");
    }
    let value = lower_checked_scalar_expression(value, &[])?;
    if value.scalar_type() != *destination_type {
        return unsupported("write-only store value type disagrees with its destination");
    }
    emit_value(
        destination.place,
        *destination_type,
        &value,
        scalar_values,
        next_value,
        operations,
    )
}

pub(crate) fn emit_value(
    destination: PlaceId,
    destination_type: ScalarType,
    value: &LoweredDirectExpression,
    scalar_values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    if value.scalar_type() != destination_type || direct_expression_contains_short_circuit(value) {
        return unsupported("primitive store RHS has an incompatible type or unexpanded control");
    }
    let source_types = scalar_values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    validate_direct_parameter_types(value, &source_types)?;
    let value = emit_direct_expression(value, scalar_values, next_value, operations);
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        destination,
        path: Vec::new(),
        value,
    })
}

/// Resolve a destination's complete storage projection against the emitted
/// declarations. Unlike a scalar-field operation this endpoint is the
/// primitive itself, so an array element needs no synthetic field identity.
/// Fields, literal elements, and the assignment's runtime elements compose in
/// one walk.
pub(crate) fn lower_destination_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<ProjectionStep>, ScalarType), LoweringError> {
    let (steps, walked) = walk_path(structural_type, path, types)?;
    let scalar_type = match walked {
        Walked::Declared(structural_type) => {
            let StructuralTypeShape::PrimitiveScalar(scalar_type) =
                unique_type(types, structural_type)?.shape
            else {
                return unsupported("primitive projection does not end at a primitive scalar");
            };
            scalar_type
        }
        Walked::ByteElement => terminal_scalar_type(PrimitiveType::U8)?,
        Walked::ScalarField(scalar) => scalar,
    };
    Ok((steps, scalar_type))
}

/// A static projection to a primitive leaf, for observations that name no
/// runtime element.
pub(crate) fn lower_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<terminal_psi::StructuralPathSegment>, ScalarType), LoweringError> {
    if let Some((CheckedUnitStructuralPathSegment::Field(identity), carrier)) = path.split_last() {
        // Reads may observe declaration-constrained scalar fields through a
        // pointer. Such a field is not raw primitive store storage. Reuse the
        // carrier walk (including its bounds), then query read-only leaf typing.
        let (steps, _) = walk_path(structural_type, carrier, types)?;
        let mut path = super::runtime_elements::static_segments(steps)?;
        path.push(terminal_psi::StructuralPathSegment::Field(identity.clone()));
        let scalar_type = terminal_semantics::primitive_read_projection_type(
            types.iter(),
            structural_type,
            &path,
        )
        .ok_or(LoweringError::Unsupported(
            "primitive observation has no readable scalar endpoint",
        ))?;
        return Ok((path, scalar_type));
    }
    let (steps, scalar_type) = lower_destination_path(structural_type, path, types)?;
    Ok((
        super::runtime_elements::static_segments(steps)?,
        scalar_type,
    ))
}

/// Resolve one scalar endpoint after any ordered element projections. A field
/// retains its declared scalar carrier and invariant; array elements retain
/// their declared primitive type. Runtime selectors change neither.
pub(crate) fn indexed_element_scalar_type(
    element: StructuralTypeId,
    element_path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<ScalarType, LoweringError> {
    if let Some((CheckedUnitStructuralPathSegment::Field(identity), carrier)) =
        element_path.split_last()
    {
        let (_, walked) = walk_path(element, carrier, types)?;
        let Walked::Declared(record) = walked else {
            return unsupported("indexed field carrier is not a record");
        };
        let StructuralTypeShape::Record { fields } = &unique_type(types, record)?.shape else {
            return unsupported("indexed field carrier is not a record");
        };
        let mut matching = fields.iter().filter(|field| field.identity == *identity);
        let (Some(field), None) = (matching.next(), matching.next()) else {
            return unsupported("indexed field is absent or ambiguous");
        };
        if field.relevance.is_erased() {
            return unsupported("indexed field is erased");
        }
        let scalar_type = match &field.field_type {
            declared if terminal_semantics::scalar_field_requires_evidence(declared) => {
                declared.scalar_type().ok_or(LoweringError::Unsupported(
                    "indexed field invariant has no scalar carrier",
                ))?
            }
            StructuralFieldType::Scalar(ScalarType::Boolean) => ScalarType::Boolean,
            StructuralFieldType::Scalar(ScalarType::Integer(integer)) if !integer.is_address() => {
                ScalarType::Integer(*integer)
            }
            StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => *scalar,
            StructuralFieldType::IeeeFloat(format) => ScalarType::IeeeFloat(*format),
            _ => {
                return unsupported(
                    "indexed element field is not an observable scalar declaration",
                );
            }
        };
        return Ok(scalar_type);
    }
    let (_, walked) = walk_path(element, element_path, types)?;
    let scalar_type = match walked {
        Walked::ScalarField(scalar_type) => scalar_type,
        Walked::Declared(identity) => match unique_type(types, identity)?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => scalar_type,
            _ => return unsupported("indexed element path does not end at a scalar"),
        },
        Walked::ByteElement => {
            return unsupported("indexed element path requires its byte carrier");
        }
    };
    match scalar_type {
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => Ok(scalar_type),
        ScalarType::Integer(integer) if !integer.is_address() => Ok(scalar_type),
        _ => unsupported("indexed element leaf is not a readable scalar"),
    }
}

/// Walk the checked segments from the destination root, resolving each field
/// against its declared record, and return the steps and the structural
/// type they select. Each runtime element names a distinct selector of the
/// assignment's target.
/// The walk's current position: a declared structural type, or a byte
/// carrier's element — the u8 scalar that rides no declared identity.
enum Walked {
    Declared(StructuralTypeId),
    ScalarField(ScalarType),
    ByteElement,
}

fn walk_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<ProjectionStep>, Walked), LoweringError> {
    use terminal_psi::StructuralPathSegment as Segment;
    let mut walked = Walked::Declared(structural_type);
    let mut result = Vec::with_capacity(path.len());
    for segment in path {
        let Walked::Declared(structural_type) = walked else {
            return unsupported("primitive projection descends below a byte element");
        };
        let declaration = unique_type(types, structural_type)?;
        walked = match (segment, &declaration.shape) {
            (
                CheckedUnitStructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, .. },
            ) => {
                result.push(ProjectionStep::Static(Segment::Referent));
                Walked::Declared(*referent)
            }
            (
                CheckedUnitStructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields },
            ) => {
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = matching.next().ok_or(LoweringError::Unsupported(
                    "primitive projection field is absent",
                ))?;
                if matching.next().is_some()
                    || field.relevance.is_erased()
                    || fields
                        .iter()
                        .filter(|candidate| candidate.id == field.id)
                        .count()
                        != 1
                {
                    return unsupported("primitive projection field is erased or ambiguous");
                }
                result.push(ProjectionStep::Static(Segment::Field(
                    field.identity.clone(),
                )));
                match field.field_type {
                    StructuralFieldType::Structural(child) => Walked::Declared(child),
                    StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => {
                        Walked::ScalarField(scalar)
                    }
                    StructuralFieldType::IeeeFloat(format) => {
                        Walked::ScalarField(ScalarType::IeeeFloat(format))
                    }
                    _ => {
                        return unsupported(
                            "primitive projection requires a plain IEEE or structural field",
                        );
                    }
                }
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                result.push(ProjectionStep::Static(Segment::FixedIndex(*index)));
                Walked::Declared(*element)
            }
            (
                CheckedUnitStructuralPathSegment::RuntimeIndex(runtime),
                StructuralTypeShape::FixedArray { element, .. },
            ) => {
                // Every checked selector vocabulary walks a fixed array the
                // same way: assignment indexes, parameter and local
                // positions, and stored-field snapshots each name a distinct
                // runtime element of the projection.
                result.push(super::runtime_elements::selector_step(runtime));
                Walked::Declared(*element)
            }
            (
                segment @ (CheckedUnitStructuralPathSegment::FixedIndex(_)
                | CheckedUnitStructuralPathSegment::RuntimeIndex(_)),
                StructuralTypeShape::ByteSequence(carrier),
            ) => {
                match segment {
                    CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                        if let terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } =
                            carrier
                            && *index >= *capacity
                        {
                            return unsupported(
                                "primitive projection has an incompatible or out-of-bounds path",
                            );
                        }
                        result.push(ProjectionStep::Static(Segment::FixedIndex(*index)));
                    }
                    CheckedUnitStructuralPathSegment::RuntimeIndex(runtime) => {
                        // A repeated operand still selects a new element of
                        // this carrier and owns its own positional bound.
                        result.push(super::runtime_elements::selector_step(runtime));
                    }
                    _ => {}
                }
                Walked::ByteElement
            }
            _ => {
                return unsupported(
                    "primitive projection has an incompatible or out-of-bounds path",
                );
            }
        };
    }
    Ok((result, walked))
}

fn unique_type(
    types: &[StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Result<&StructuralTypeDeclaration, LoweringError> {
    let mut matching = types
        .iter()
        .filter(|declaration| declaration.id == identity);
    let declaration = matching.next().ok_or(LoweringError::Unsupported(
        "primitive projection type is absent",
    ))?;
    if matching.next().is_some() {
        return unsupported("primitive projection type is ambiguous");
    }
    Ok(declaration)
}
