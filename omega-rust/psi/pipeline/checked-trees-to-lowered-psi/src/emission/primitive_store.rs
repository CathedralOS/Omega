//! Primitive-reference store emission shared by Unit and scalar-result bodies.
use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, LoweringError,
    OperationKind, PlaceId, PrimitiveType, ScalarType, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, ValueDeclaration,
    direct_expression_contains_short_circuit, emit_direct_expression,
    lower_checked_scalar_expression, terminal_scalar_type, unsupported,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;

/// The resolved primitive endpoint is shared by ordinary and composed bodies.
/// Their namespaces choose a parameter or live local; this owner validates the
/// exclusive parameter projection and evaluates exactly one authored RHS.
pub(crate) struct Destination {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    pub(crate) scalar_type: ScalarType,
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
    let (path, scalar_type) = lower_path(parameter.structural_type, path, types)?;
    Ok(Destination {
        place: parameter.place,
        path,
        scalar_type,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    destination: Destination,
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
    let value = evaluation.source_value(
        checked,
        machine,
        state,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
        value,
        source_value_count,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        calls,
    )?;
    if value.scalar_type != destination.scalar_type || !value.qualifications.is_empty() {
        return unsupported("primitive store RHS differs from its destination carrier");
    }
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        destination: destination.place,
        path: destination.path,
        value: value.id,
    })
}

/// The resolved array-carrier endpoint for a runtime-indexed store: the
/// retained path stops at the fixed array itself, so its element type is the
/// store's scalar carrier and the runtime index is a separate operand.
pub(crate) struct IndexedDestination {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    pub(crate) scalar_type: ScalarType,
}

/// A runtime-indexed store is always a projected write into exclusive
/// borrowed storage; it never stores the parameter whole.
pub(crate) fn indexed_parameter_destination(
    parameter: &StructuralParameterDeclaration,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<IndexedDestination, LoweringError> {
    if !matches!(
        parameter.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
    {
        return unsupported("indexed primitive store parameter lost its exclusive custody");
    }
    let (path, scalar_type) = lower_indexed_path(parameter.structural_type, path, types)?;
    Ok(IndexedDestination {
        place: parameter.place,
        path,
        scalar_type,
    })
}

/// Emit a runtime-indexed primitive store: the retained index lowers to the
/// `u64` coordinate (with the ordinary exact-cast obligation when the carrier
/// needs conversion), the RHS shares `emit_assignment`'s source evaluation,
/// and the bounds obligation certifies the checked `index < extent` proof.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_indexed_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    destination: IndexedDestination,
    index: &CheckedScalarExpression,
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
    let index = lower_checked_scalar_expression(index)?;
    if direct_expression_contains_short_circuit(&index) {
        return unsupported("indexed primitive store index has unexpanded control");
    }
    let index = super::emit_byte_index(
        &index,
        values,
        next_value,
        &mut calls.next_obligation_identity,
        operations,
    )?;
    let value = evaluation.source_value(
        checked,
        machine,
        state,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
        value,
        source_value_count,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        calls,
    )?;
    if value.scalar_type != destination.scalar_type || !value.qualifications.is_empty() {
        return unsupported("primitive store RHS differs from its destination carrier");
    }
    Ok(OperationKind::WriteOnlyIndexedPrimitiveStore {
        destination: destination.place,
        path: destination.path,
        index,
        value: value.id,
        obligation: calls.allocate_requirement()?,
    })
}

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    destination: &CheckedUnitStructuralParameterPlan,
    path: &[CheckedUnitStructuralPathSegment],
    value: &checked_trees::CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let parameter = checked
        .state_parameters(state)
        .get(destination.position as usize)
        .ok_or(LoweringError::Unsupported(
            "primitive store lost its authored destination",
        ))?;
    validate_symbol_assignment(
        checked,
        state_symbol,
        statement_index,
        parameter.symbol,
        path,
        value,
    )
}

pub(crate) fn validate_symbol_assignment(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    destination: symbols::SymbolHandle,
    path: &[CheckedUnitStructuralPathSegment],
    value: &checked_trees::CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    use checked_trees::statement::StatementNode;
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let Some(StatementNode::Assignment(assignment)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)
    else {
        return unsupported("primitive store has no authored assignment");
    };
    let target = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine.symbol,
        state_symbol,
        Some(statement_index as usize),
        assignment.target,
    )?;
    if !destination.is_valid() || target.root != destination || target.path != path {
        return unsupported("primitive store destination differs from its authored parameter");
    }
    validate_assignment_value(checked, state_symbol, statement_index, assignment, value)
}

/// Source custody for a runtime-indexed primitive store. The authored target
/// is one `Indexed` whose collection resolves to the destination parameter's
/// retained static path; the retained `AssignmentIndex` binding must carry
/// exactly the plan's scalar operand, and the RHS shares the ordinary
/// assignment-value checks.
pub(crate) fn validate_indexed_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    destination: &CheckedUnitStructuralParameterPlan,
    path: &[CheckedUnitStructuralPathSegment],
    index: &CheckedScalarExpression,
    value: &checked_trees::CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    use checked_trees::{expression::ExpressionNode, statement::StatementNode};
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let Some(StatementNode::Assignment(assignment)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)
    else {
        return unsupported("indexed primitive store has no authored assignment");
    };
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(assignment.target)
    else {
        return unsupported("indexed primitive store lost its authored indexed target");
    };
    if owner.symbol != machine
        || matches!(
            checked.expression_table.expression(indexed.index),
            ExpressionNode::Integer(_) | ExpressionNode::Range(_)
        )
        || !validation::place_has_builtin_coordinates(
            &checked.typed,
            owner,
            Some(state),
            assignment.target,
        )
    {
        return unsupported("indexed primitive store requires exact builtin indexed custody");
    }
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement_index as usize),
        indexed.collection,
    )?;
    let parameter = checked
        .state_parameters(state)
        .get(destination.position as usize)
        .ok_or(LoweringError::Unsupported(
            "indexed primitive store lost its authored destination",
        ))?;
    if source.root != parameter.symbol || source.path != path {
        return unsupported(
            "indexed primitive store destination differs from its authored parameter",
        );
    }
    let (index_binding, retained_index) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state_symbol,
            statement_index,
            CheckedScalarExpressionRole::AssignmentIndex,
        )
        .ok_or(LoweringError::Unsupported(
            "indexed primitive store lost a scalar source binding",
        ))?;
    if index_binding.expression != indexed.index || retained_index != index {
        return unsupported("indexed primitive store substituted its evaluated index operand");
    }
    crate::expression_preparation::source_custody::validate_pure(
        checked,
        index_binding,
        terminal_scalar_type(
            crate::expression_preparation::source_custody::locate(
                checked,
                state_symbol,
                statement_index,
                CheckedScalarExpressionRole::AssignmentIndex,
            )?
            .primitive_type,
        )?,
    )?;
    validate_assignment_value(checked, state_symbol, statement_index, assignment, value)
}

/// The authored-RHS custody shared by static and runtime-indexed stores.
fn validate_assignment_value(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    assignment: &checked_trees::statement::TableAssignment,
    value: &checked_trees::CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    use checked_trees::expression::ExpressionNode;
    let (machine, _state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let binding_destination = match checked.expression_table.expression(assignment.target) {
        ExpressionNode::Name(name) => name.symbol,
        _ => symbols::SymbolHandle::invalid(),
    };
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let computations = &checked.facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state_symbol
                && root.statement_ordinal == statement_index
                && root.role == role
        });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("primitive store has duplicate RHS computation roots");
    }
    let value = match value {
        checked_trees::CheckedCallScalarArgument::Computation(handle) => {
            let root = root.ok_or(LoweringError::Unsupported(
                "primitive store lost its RHS computation root",
            ))?;
            if root.machine != machine.symbol
                || root.root != *handle
                || !computations.nodes.is_valid(*handle)
            {
                return unsupported("primitive store RHS computation has different custody");
            }
            let source = crate::expression_preparation::source_custody::locate(
                checked,
                state_symbol,
                statement_index,
                role,
            )?;
            let node = computations.nodes.get(*handle);
            if source.machine != machine.symbol
                || source.destination != binding_destination
                || source.expression != assignment.value
                || node.authored_root != assignment.value
                || node.primitive_type != source.primitive_type
                || checked
                    .facts
                    .values
                    .scalar_expressions
                    .expressions
                    .iter()
                    .any(|expression| {
                        expression.state == state_symbol
                            && expression.statement_ordinal == statement_index
                            && expression.role == role
                    })
                || checked
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .iter()
                    .any(|(_, binding)| {
                        binding.state == state_symbol
                            && binding.statement_ordinal == statement_index
                            && binding.role == role
                    })
            {
                return unsupported("primitive store computation differs from its authored RHS");
            }
            crate::expression_preparation::source_custody::validate_computation_calls(
                checked,
                machine.symbol,
                state_symbol,
                statement_index,
                *handle,
                assignment.value,
            )?;
            return crate::expression_preparation::source_custody::value_correspondence::validate(
                checked,
                state_symbol,
                statement_index,
                assignment.value,
                source.primitive_type,
                value,
            );
        }
        checked_trees::CheckedCallScalarArgument::Pure(value) => {
            if root.is_some() {
                return unsupported(
                    "primitive store replaced its RHS computation with a pure value",
                );
            }
            value
        }
    };
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state_symbol,
            statement_index,
            CheckedScalarExpressionRole::AssignmentValue,
        )
        .ok_or(LoweringError::Unsupported(
            "primitive store lost its unique RHS binding",
        ))?;
    if binding.expression != assignment.value
        || binding.destination != binding_destination
        || expression != value
    {
        return unsupported("primitive store RHS differs from its authored assignment");
    }
    crate::expression_preparation::source_custody::validate_namespace(checked, binding)
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

/// Resolve the complete storage projection against the emitted declarations.
/// Unlike a scalar-field operation this endpoint is the primitive itself, so
/// an array element needs no synthetic field identity.
pub(crate) fn lower_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<
    (
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        ScalarType,
    ),
    LoweringError,
> {
    let (result, structural_type) = walk_path(structural_type, path, types)?;
    let StructuralTypeShape::PrimitiveScalar(scalar_type) =
        unique_type(types, structural_type)?.shape
    else {
        return unsupported("primitive projection does not end at a primitive scalar");
    };
    Ok((result, scalar_type))
}

/// The static prefix of a runtime-indexed store ends at the fixed array
/// itself; the runtime index is an operand, never a path segment. The
/// element must be a primitive scalar — a record or nested-array element
/// keeps its own store owners.
pub(crate) fn lower_indexed_path(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<
    (
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        ScalarType,
    ),
    LoweringError,
> {
    let (result, structural_type) = walk_path(structural_type, path, types)?;
    let StructuralTypeShape::FixedArray { element, .. } =
        unique_type(types, structural_type)?.shape
    else {
        return unsupported("indexed primitive projection does not end at a fixed array");
    };
    let StructuralTypeShape::PrimitiveScalar(scalar_type) = unique_type(types, element)?.shape
    else {
        return unsupported("indexed primitive projection's element is not a primitive scalar");
    };
    Ok((result, scalar_type))
}

/// Walk literal field/index segments from the destination root, returning the
/// canonical path and the structural type it selects.
fn walk_path(
    mut structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<
    (
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        StructuralTypeId,
    ),
    LoweringError,
> {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Segment;
    let mut result = Vec::with_capacity(path.len());
    let mut visited = Vec::new();
    for segment in path {
        if visited.contains(&structural_type) {
            return unsupported("primitive projection has a cyclic carrier");
        }
        visited.push(structural_type);
        let declaration = unique_type(types, structural_type)?;
        structural_type = match (segment, &declaration.shape) {
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
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return unsupported("primitive projection requires a structural carrier field");
                };
                result.push(Segment::Field(field.id));
                child
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                result.push(Segment::FixedIndex(*index));
                *element
            }
            _ => {
                return unsupported(
                    "primitive projection has an incompatible or out-of-bounds path",
                );
            }
        };
    }
    Ok((result, structural_type))
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
