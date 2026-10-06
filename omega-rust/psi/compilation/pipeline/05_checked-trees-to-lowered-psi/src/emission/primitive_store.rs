//! Primitive-reference store emission shared by Unit and scalar-result bodies.
//!
//! An assignment selector keeps its exact indexed source occurrence until its
//! completed destination path mints the Terminal bound obligation. Checking
//! already decided that occurrence after operand effects; repeating certificate
//! search would lose the decision at the representation boundary. Only this
//! bound is source-checked: destination custody, declared value constraints and
//! stored-value formation remain their own admissions. Generated selectors have
//! no assignment occurrence and retain their existing evidence route. The
//! checked program retains the authored expression table for these identity
//! joins; reading it here neither checks source semantics nor rebuilds a plan.
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
    let (_, authored) = crate::declarations::state_declaration(checked, state)?;
    let Some(checked_trees::statement::StatementNode::Assignment(assignment)) = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(statement_index as usize)
    else {
        return unsupported("primitive store lost its authored assignment");
    };
    let indexed_occurrences =
        validation::assignment_target_indexes(&checked.expression_table, assignment.target);
    let decided_bounds = destination
        .path
        .iter()
        .map(|step| match step {
            ProjectionStep::AssignmentIndex { depth } => indexed_occurrences
                .get(*depth as usize)
                .map(|expression| checked.facts.scalar_index_is_proven(*expression))
                .ok_or(LoweringError::Unsupported(
                    "primitive store lost its authored indexed occurrence",
                )),
            _ => Ok(false),
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    let path = super::runtime_elements::complete_path(destination.path, indexes, calls)?;
    // complete_path maps every input step to exactly one segment. Refuse an
    // incomplete positional join if that contract changes; zip must not drop
    // a decision or pair it with a different destination step.
    if path.len() != decided_bounds.len() {
        return unsupported("primitive store path changed its selector positions");
    }
    // Path order is inner-first, but each AssignmentIndex retained its own
    // outside-in depth. Pair the decision with the exact completed step rather
    // than with a selector value or the assignment's outermost bound.
    for (segment, decided) in path.iter().zip(decided_bounds) {
        if decided
            && let terminal_psi::StructuralPathSegment::RuntimeIndex { obligation, .. } = segment
        {
            operations.source_checked_obligations.push(*obligation);
        }
    }
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        destination: destination.place,
        path,
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
    let value = lower_checked_scalar_expression(value)?;
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
    let (steps, scalar_type) = lower_destination_path(structural_type, path, types)?;
    Ok((
        super::runtime_elements::static_segments(steps)?,
        scalar_type,
    ))
}

/// The static prefix of a runtime-indexed primitive read ends at the fixed
/// array itself; the reader appends the `RuntimeIndex` segment once it has
/// evaluated the selector and allocated the segment's obligation. The element
/// must be a primitive scalar.
pub(crate) fn lower_indexed_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<terminal_psi::StructuralPathSegment>, ScalarType), LoweringError> {
    let (steps, walked) = walk_path(structural_type, path, types)?;
    let Walked::Declared(structural_type) = walked else {
        return unsupported("indexed primitive projection does not end at a fixed array");
    };
    let StructuralTypeShape::FixedArray { element, .. } =
        unique_type(types, structural_type)?.shape
    else {
        return unsupported("indexed primitive projection does not end at a fixed array");
    };
    let StructuralTypeShape::PrimitiveScalar(scalar_type) = unique_type(types, element)?.shape
    else {
        return unsupported("indexed primitive projection's element is not a primitive scalar");
    };
    Ok((
        super::runtime_elements::static_segments(steps)?,
        scalar_type,
    ))
}

/// The static prefix of a read that selects at run time once over `path`'s
/// fixed array and then `inner_depth` more times inside nested fixed-array
/// elements (`self.a[i][k]` has depth one). Each level must be a declared
/// fixed array and the last element a primitive scalar; the reader places one
/// `RuntimeIndex` segment per selector after the prefix, and the verifier
/// bounds each against the array its own prefix resolves to.
pub(crate) fn lower_nested_indexed_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    inner_depth: usize,
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<terminal_psi::StructuralPathSegment>, ScalarType), LoweringError> {
    let (steps, walked) = walk_path(structural_type, path, types)?;
    let Walked::Declared(mut selected) = walked else {
        return unsupported("nested indexed projection does not end at a fixed array");
    };
    for _ in 0..=inner_depth {
        let StructuralTypeShape::FixedArray { element, .. } = unique_type(types, selected)?.shape
        else {
            return unsupported("nested indexed projection selects inside a non-array element");
        };
        selected = element;
    }
    let StructuralTypeShape::PrimitiveScalar(scalar_type) = unique_type(types, selected)?.shape
    else {
        return unsupported("nested indexed projection's leaf is not a primitive scalar");
    };
    Ok((
        super::runtime_elements::static_segments(steps)?,
        scalar_type,
    ))
}

/// The selected element's scalar projection retains either only static steps
/// or additional checked runtime selectors. Both emit one primitive read whose
/// fresh scalar receives the exact endpoint declaration's invariant.
pub(crate) enum IndexedElementLeaf {
    Flat(Vec<terminal_psi::StructuralPathSegment>),
    /// The element path's leaf is a runtime selection into a fixed-array
    /// member — `cells[i].data[j]` names `data`'s element. The path's
    /// static steps resolve exactly like `Flat`'s; its `RuntimeIndex` steps
    /// keep their checked coordinate, which the caller lowers into the
    /// read's evaluated selectors.
    RuntimeTail {
        element_path: Vec<CheckedUnitStructuralPathSegment>,
    },
}

/// Resolve the full selected-field path without copying its carrier. The read
/// retains every index and stored-reference crossing. Its declared scalar
/// invariant constrains only the fresh SSA result, not a canonical storage fact.
pub(crate) fn lower_indexed_element_field_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    element_path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<
    (
        Vec<terminal_psi::StructuralPathSegment>,
        IndexedElementLeaf,
        ScalarType,
    ),
    LoweringError,
> {
    let (steps, walked) = walk_path(structural_type, path, types)?;
    let Walked::Declared(structural_type) = walked else {
        return unsupported("indexed element field projection does not end at a fixed array");
    };
    let StructuralTypeShape::FixedArray { element, .. } =
        unique_type(types, structural_type)?.shape
    else {
        return unsupported("indexed element field projection does not end at a fixed array");
    };
    let Some((CheckedUnitStructuralPathSegment::Field(identity), carrier)) =
        element_path.split_last()
    else {
        return unsupported("indexed element field path does not end at a field");
    };
    let (carrier_steps, walked) = walk_path(element, carrier, types)?;
    let Walked::Declared(record) = walked else {
        return unsupported("indexed element field carrier is not a record");
    };
    let StructuralTypeShape::Record { fields } = &unique_type(types, record)?.shape else {
        return unsupported("indexed element field carrier is not a record");
    };
    let mut matching = fields.iter().filter(|field| field.identity == *identity);
    let field = matching.next().ok_or(LoweringError::Unsupported(
        "indexed element field is absent",
    ))?;
    if matching.next().is_some() || field.relevance.is_erased() {
        return unsupported("indexed element field is erased or ambiguous");
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
        _ => return unsupported("indexed element field is not an observable scalar declaration"),
    };
    let mut element_path = super::runtime_elements::static_segments(carrier_steps)?;
    element_path.push(terminal_psi::StructuralPathSegment::Field(
        field.identity.clone(),
    ));
    Ok((
        super::runtime_elements::static_segments(steps)?,
        IndexedElementLeaf::Flat(element_path),
        scalar_type,
    ))
}

/// The runtime-tail element path's leaf walk (`cells[i].data[j]`): the same
/// member-resolution walk `lower_indexed_element_field_path`'s carrier
/// takes, with each `RuntimeIndex` step selecting into a fixed array and
/// the path ending at one primitive scalar. The leaf keeps the checked
/// path verbatim — its runtime coordinates lower beside the read's other
/// selectors, each re-proven against the array it selects from.
pub(crate) fn lower_indexed_element_runtime_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    element_path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<
    (
        Vec<terminal_psi::StructuralPathSegment>,
        IndexedElementLeaf,
        ScalarType,
    ),
    LoweringError,
> {
    let (steps, walked) = walk_path(structural_type, path, types)?;
    let Walked::Declared(structural_type) = walked else {
        return unsupported("indexed element runtime projection does not end at a fixed array");
    };
    let StructuralTypeShape::FixedArray { element, .. } =
        unique_type(types, structural_type)?.shape
    else {
        return unsupported("indexed element runtime projection does not end at a fixed array");
    };
    let (_, walked) = walk_path(element, element_path, types)?;
    let Walked::Declared(structural_type) = walked else {
        return unsupported("indexed element runtime path does not end at a scalar carrier");
    };
    if !element_path
        .iter()
        .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::RuntimeIndex(_)))
    {
        return unsupported("indexed element runtime path has no runtime selector");
    }
    let StructuralTypeShape::PrimitiveScalar(scalar_type) =
        unique_type(types, structural_type)?.shape
    else {
        return unsupported("indexed element runtime path does not end at a scalar");
    };
    match scalar_type {
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => {}
        ScalarType::Integer(integer) if !integer.is_address() => {}
        _ => {
            return unsupported("indexed element runtime leaf is not a readable scalar");
        }
    }
    Ok((
        super::runtime_elements::static_segments(steps)?,
        IndexedElementLeaf::RuntimeTail {
            element_path: element_path.to_vec(),
        },
        scalar_type,
    ))
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
    let mut visited = Vec::new();
    for segment in path {
        let Walked::Declared(structural_type) = walked else {
            return unsupported("primitive projection descends below a byte element");
        };
        if visited.contains(&structural_type) {
            return unsupported("primitive projection has a cyclic carrier");
        }
        visited.push(structural_type);
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
