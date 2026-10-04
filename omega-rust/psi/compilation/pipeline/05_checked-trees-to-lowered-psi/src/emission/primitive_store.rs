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
use crate::emission::runtime_elements::ProjectionStep;

/// The resolved primitive endpoint is shared by ordinary and composed bodies.
/// Their namespaces choose a parameter or live local; this owner validates the
/// exclusive parameter projection and evaluates exactly one authored RHS.
pub(crate) struct Destination {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<ProjectionStep>,
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
    let (path, scalar_type) = lower_destination_path(parameter.structural_type, path, types)?;
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
    if value.scalar_type != destination.scalar_type || !value.qualifications.is_empty() {
        return unsupported("primitive store RHS differs from its destination carrier");
    }
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        destination: destination.place,
        path: super::runtime_elements::complete_path(destination.path, indexes, calls)?,
        value: value.id,
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
    let (assignment, target) = authored_store(checked, state_symbol, statement_index)?;
    if !destination.is_valid() || target.root != destination {
        return unsupported("primitive store destination differs from its authored parameter");
    }
    match value_role_at(checked, state_symbol, assignment, &target.path, path)? {
        CheckedScalarExpressionRole::AssignmentValue => {
            validate_assignment_value(checked, state_symbol, statement_index, assignment, value)
        }
        role => validate_array_element_value(
            checked,
            state_symbol,
            statement_index,
            assignment,
            role,
            value,
        ),
    }
}

/// Where a primitive store's value was checked. A store whose path is its
/// authored target's own path stores the assignment's value; one whose path
/// extends it by literal element indices is one element store of a whole
/// array-literal replacement and stores that element's row.
pub(crate) fn value_role(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    path: &[CheckedUnitStructuralPathSegment],
) -> Result<CheckedScalarExpressionRole, LoweringError> {
    let (assignment, target) = authored_store(checked, state_symbol, statement_index)?;
    value_role_at(checked, state_symbol, assignment, &target.path, path)
}

/// Every statement with more than one primitive store is a whole
/// array-literal replacement: its stores, in operation order, write the
/// literal's row-major elements, each exactly once and all of them.
pub(crate) fn validate_element_rosters(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    operations: &[checked_trees::CheckedUnitEffectOperationPlan],
) -> Result<(), LoweringError> {
    let mismatch =
        || LoweringError::Unsupported("array literal element stores are not its complete roster");
    let mut rosters: Vec<(u32, Vec<&[CheckedUnitStructuralPathSegment]>)> = Vec::new();
    for operation in operations {
        let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            path,
            ..
        } = operation
        else {
            continue;
        };
        match rosters
            .iter_mut()
            .find(|(statement, _)| statement == statement_index)
        {
            Some((_, paths)) => paths.push(path),
            None => rosters.push((*statement_index, vec![path])),
        }
    }
    for (statement_index, paths) in rosters {
        if paths.len() == 1 {
            continue;
        }
        let (assignment, target) = authored_store(checked, state_symbol, statement_index)?;
        let (machine, state) =
            crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
        let elements = validation::declared_place_type_raw(
            &checked.typed,
            machine,
            Some(state),
            assignment.target,
        )
        .and_then(|declared| validation::closed_array_store_type(&checked.typed, declared))
        .and_then(|expected| {
            validation::scalar_array_elements(
                &checked.typed,
                machine.symbol,
                assignment.value,
                expected,
            )
        })
        .ok_or_else(mismatch)?;
        if elements.elements.len() != paths.len() {
            return Err(mismatch());
        }
        for (ordinal, path) in paths.into_iter().enumerate() {
            let expected = CheckedScalarExpressionRole::ArrayElement {
                source: checked_trees::CheckedArrayConstructionSource::Statement,
                element_ordinal: u32::try_from(ordinal).map_err(|_| mismatch())?,
            };
            if value_role_at(checked, state_symbol, assignment, &target.path, path)? != expected {
                return Err(mismatch());
            }
        }
    }
    Ok(())
}

/// The authored assignment at a primitive store's statement, with the
/// storage its target names.
fn authored_store(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
) -> Result<
    (
        &checked_trees::statement::TableAssignment,
        crate::emission::call_source_custody::projected_receivers::ReceiverSource,
    ),
    LoweringError,
> {
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
    let mut target = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine.symbol,
        state_symbol,
        Some(statement_index as usize),
        assignment.target,
    )?;
    target.path = authored_storage_path(checked, machine, state, &target)?;
    Ok((assignment, target))
}

/// Replay the implicit dereferences from declarations, not from the producer's
/// proposed path. The authored root remains the carrier whose pointer is read;
/// no static origin can replace that address. Ordinary fields and array elements
/// retain their original steps around each explicit crossing.
fn authored_storage_path(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    target: &crate::emission::call_source_custody::projected_receivers::ReceiverSource,
) -> Result<Vec<CheckedUnitStructuralPathSegment>, LoweringError> {
    use checked_trees::types::TypeReferenceNode;
    let invalid =
        || LoweringError::Unsupported("primitive store lost its declared storage projection");
    let parameter = checked
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == target.root);
    let (mut selected, mut owner) = if let Some(parameter) = parameter {
        let TypeReferenceNode::Reference { referee, .. } = checked
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return Err(invalid());
        };
        (
            *referee,
            if parameter.is_self {
                checked
                    .typed
                    .unique_data_definition_by_symbol(machine.attached_data_symbol)
            } else {
                None
            },
        )
    } else {
        let (_, local) = checked
            .state_local_declaration(state, target.root)
            .ok_or_else(invalid)?;
        // A reference-typed local stores into its referent: the authored
        // root folds its own shell into the access, so the walk starts from
        // the referee exactly as the borrowed parameter's does.
        (
            validation::reference_shell(&checked.typed, local.type_reference)
                .map(|shell| shell.referee)
                .unwrap_or(local.type_reference),
            None,
        )
    };
    let mut path = Vec::with_capacity(target.path.len());
    for (position, segment) in target.path.iter().enumerate() {
        if let Some((_, declared)) = target
            .alias_bridges()
            .iter()
            .find(|(at, _)| *at == position)
        {
            // The captured prefix ends at the view's own address: `view.value`
            // reads the fields of the alias's declared record, not of the
            // element the folded place selected. Swap the declared referee
            // in and drop the root's owner scope for the rest of the walk.
            selected = *declared;
            owner = None;
        }
        selected = match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                let declaration = owner
                    .or_else(|| checked.typed.closed_data_definition(selected))
                    .ok_or_else(invalid)?;
                let mut fields = checked
                    .data_members(declaration)
                    .iter()
                    .filter_map(|member| {
                        let checked_trees::data::DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.path_identity() == *identity && !field.relevance.is_erased())
                            .then_some(field)
                    });
                let field = fields.next().ok_or_else(invalid)?;
                if fields.next().is_some() {
                    return Err(invalid());
                }
                field.type_reference
            }
            CheckedUnitStructuralPathSegment::FixedIndex(_)
            | CheckedUnitStructuralPathSegment::RuntimeIndex(_) => {
                let carrier = validation::unwrapped_type_reference(&checked.typed, selected)
                    .ok_or_else(invalid)?;
                match checked.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => *element_type,
                    _ => return Err(invalid()),
                }
            }
            _ => return Err(invalid()),
        };
        path.push(segment.clone());
        let mut reference_types = Vec::new();
        while let TypeReferenceNode::Reference {
            referee, access, ..
        } = checked.type_reference_table.type_reference(selected)
        {
            if reference_types.contains(&selected) {
                return Err(invalid());
            }
            reference_types.push(selected);
            if matches!(
                checked.type_reference_table.type_reference(*referee),
                TypeReferenceNode::Slice { .. }
            ) {
                break;
            }
            if *access != language_semantics::ReferenceAccess::Mutable {
                return Err(invalid());
            }
            path.push(CheckedUnitStructuralPathSegment::Referent);
            selected = *referee;
        }
        owner = None;
    }
    Ok(path)
}

fn value_role_at(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    target_path: &[CheckedUnitStructuralPathSegment],
    path: &[CheckedUnitStructuralPathSegment],
) -> Result<CheckedScalarExpressionRole, LoweringError> {
    use checked_trees::types::{FixedArrayLength, TypeReferenceNode};
    if target_path == path {
        return Ok(CheckedScalarExpressionRole::AssignmentValue);
    }
    let mismatch = || {
        LoweringError::Unsupported(
            "primitive store destination differs from its authored parameter",
        )
    };
    let indices = path
        .strip_prefix(target_path)
        .filter(|suffix| !suffix.is_empty())
        .ok_or_else(mismatch)?;
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let mut reference = validation::declared_place_type_raw(
        &checked.typed,
        machine,
        Some(state),
        assignment.target,
    )
    .and_then(|declared| validation::closed_array_store_type(&checked.typed, declared))
    .ok_or_else(mismatch)?;
    let mut ordinal = 0_u64;
    let mut indices = indices.iter();
    while let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Literal(length),
    } = checked.type_reference_table.type_reference(reference)
    {
        let Some(CheckedUnitStructuralPathSegment::FixedIndex(index)) = indices.next() else {
            return Err(mismatch());
        };
        let extent = u64::try_from(*length).map_err(|_| mismatch())?;
        if *index >= extent {
            return Err(mismatch());
        }
        ordinal = ordinal
            .checked_mul(extent)
            .and_then(|ordinal| ordinal.checked_add(*index))
            .ok_or_else(mismatch)?;
        reference = *element_type;
    }
    if indices.next().is_some() {
        return Err(mismatch());
    }
    Ok(CheckedScalarExpressionRole::ArrayElement {
        source: checked_trees::CheckedArrayConstructionSource::Statement,
        element_ordinal: u32::try_from(ordinal).map_err(|_| mismatch())?,
    })
}

/// The authored-RHS custody of one element store of a whole array-literal
/// replacement: the store's value is the element's own checked row, bound
/// to exactly that authored element of the literal.
fn validate_array_element_value(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    assignment: &checked_trees::statement::TableAssignment,
    role: CheckedScalarExpressionRole,
    value: &checked_trees::CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    let mismatch =
        || LoweringError::Unsupported("array element store differs from its authored literal");
    let CheckedScalarExpressionRole::ArrayElement {
        element_ordinal, ..
    } = role
    else {
        return Err(mismatch());
    };
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let expected = validation::declared_place_type_raw(
        &checked.typed,
        machine,
        Some(state),
        assignment.target,
    )
    .and_then(|declared| validation::closed_array_store_type(&checked.typed, declared))
    .ok_or_else(mismatch)?;
    let element = validation::scalar_array_elements(
        &checked.typed,
        machine.symbol,
        assignment.value,
        expected,
    )
    .and_then(|array| array.elements.get(element_ordinal as usize).copied())
    .map(|(element, _)| element)
    .ok_or_else(mismatch)?;
    let checked_trees::CheckedCallScalarArgument::Pure(value) = value else {
        return Err(mismatch());
    };
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(state_symbol, statement_index, role)
        .ok_or_else(mismatch)?;
    if binding.expression != element || binding.destination.is_valid() || expression != value {
        return Err(mismatch());
    }
    crate::expression_preparation::source_custody::validate_namespace(checked, binding)
}

/// The authored-RHS custody of a primitive store.
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

/// How a resolved runtime-indexed element field read reaches its leaf: the
/// flat `RuntimeIndex`-inserted path for leaves the runtime read admits, or
/// an element hoist whose `StructuralLeafCopy` materializes the selected
/// record so the canonical field read supplies a bound the runtime path
/// cannot carry.
pub(crate) enum IndexedElementLeaf {
    Flat(Vec<terminal_psi::StructuralPathSegment>),
    Hoist {
        element: StructuralTypeId,
        carrier: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
    },
    /// The element path's leaf is a runtime selection into a fixed-array
    /// member — `cells[i].data[j]` names `data`'s element. The path's
    /// static steps resolve exactly like `Flat`'s; its `RuntimeIndex` steps
    /// keep their checked coordinate, which the caller lowers into the
    /// read's evaluated selectors.
    RuntimeTail {
        element_path: Vec<CheckedUnitStructuralPathSegment>,
    },
}

/// The static prefix and leaf of a scalar field inside a runtime-selected
/// record element (`cells[i].hp`). The prefix ends at the fixed array, as
/// for a primitive element; the element path walks the selected record
/// through structural fields and literal indexes and ends at one relevant,
/// non-address integer, Boolean, or plain IEEE scalar field. The reader
/// places its `RuntimeIndex` segment between the two. Independent read
/// verification checks the exact leaf type and index bound; accepting IEEE
/// here does not erase bounded-field refinements. A bounded leaf instead
/// hoists: the copy materializes the element and the canonical field read
/// on the fresh place supplies its declared range.
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
    if terminal_semantics::scalar_field_requires_evidence(&field.field_type) {
        let carrier = canonical_carrier(record, carrier, types)?;
        return Ok((
            super::runtime_elements::static_segments(steps)?,
            IndexedElementLeaf::Hoist {
                element,
                carrier,
                field: field.id,
            },
            field
                .field_type
                .scalar_type()
                .ok_or(LoweringError::Unsupported(
                    "restricted indexed element field has no scalar carrier",
                ))?,
        ));
    }
    let scalar_type = match field.field_type {
        StructuralFieldType::Scalar(ScalarType::Boolean) => ScalarType::Boolean,
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) if !integer.is_address() => {
            ScalarType::Integer(integer)
        }
        StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => scalar,
        StructuralFieldType::IeeeFloat(format) => ScalarType::IeeeFloat(format),
        _ => {
            return unsupported(
                "indexed element field is not a non-address integer, Boolean, or plain IEEE scalar",
            );
        }
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

/// Canonicalize the element path's structural prefix for a hoisted read:
/// each carrier field resolves to its declared field identity, and a
/// literal fixed-array index carries over verbatim. The lowered leaf walk
/// already accepted the path, so every carrier step is structural here.
fn canonical_carrier(
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<semantic_vocabulary::CanonicalStructuralPathSegment>, LoweringError> {
    let mut result = Vec::with_capacity(path.len());
    let mut structural_type = structural_type;
    for segment in path {
        let declaration = unique_type(types, structural_type)?;
        match (segment, &declaration.shape) {
            (
                CheckedUnitStructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields },
            ) => {
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = matching.next().ok_or(LoweringError::Unsupported(
                    "indexed element carrier field is absent",
                ))?;
                if matching.next().is_some() {
                    return unsupported("indexed element carrier field is ambiguous");
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return unsupported("indexed element carrier descends through a scalar field");
                };
                result.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    field.id,
                ));
                structural_type = child;
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                result
                    .push(semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = *element;
            }
            _ => {
                return unsupported("indexed element carrier is not structural");
            }
        }
    }
    Ok(result)
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
    let mut structural_type = element;
    let mut runtime_segments = 0_usize;
    for segment in element_path {
        let declaration = unique_type(types, structural_type)?;
        match (segment, &declaration.shape) {
            (
                CheckedUnitStructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields },
            ) => {
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = matching.next().ok_or(LoweringError::Unsupported(
                    "indexed element runtime carrier field is absent",
                ))?;
                if matching.next().is_some() || field.relevance.is_erased() {
                    return unsupported(
                        "indexed element runtime carrier field is erased or ambiguous",
                    );
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return unsupported(
                        "indexed element runtime carrier descends through a scalar field",
                    );
                };
                structural_type = child;
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                structural_type = *element;
            }
            (
                CheckedUnitStructuralPathSegment::RuntimeIndex(_),
                StructuralTypeShape::FixedArray { element, .. },
            ) => {
                runtime_segments += 1;
                structural_type = *element;
            }
            _ => {
                return unsupported("indexed element runtime path is not structural");
            }
        }
    }
    if runtime_segments == 0 {
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
                let step = super::runtime_elements::selector_step(runtime);
                if result.contains(&step) {
                    return unsupported(
                        "primitive projection has an incompatible or out-of-bounds path",
                    );
                }
                result.push(step);
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
                        // Indexed stores through a borrowed byte view select
                        // with the same selector vocabulary a fixed array
                        // uses; a repeated step is already this projection's
                        // selector and is simply not pushed again.
                        let step = super::runtime_elements::selector_step(runtime);
                        if !result.contains(&step) {
                            result.push(step);
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const CARRIER_STORE: &str = r#"
        data Window { padding: u64; destination: &mut u64; }
        data Carrier { prefix: u64; window: Window; count: u64; }
        machine Carrier::set_count(carrier: &mut Carrier) { carrier.count = 7; }
        data Main { scratch: u64; }
        machine Main::inspect(destination: &mut u64, observed: &u64) -> u64 {
            let mut carrier: Carrier = Carrier {
                prefix: 3,
                window: Window { padding: 5, destination: destination },
                count: 0
            };
            Carrier::set_count(&mut carrier);
            carrier.window.destination = 11;
            match observed == 59 && carrier.count == 7 { true -> 70, false -> 71 }
        }
    "#;

    #[test]
    fn primitive_store_source_replays_carrier_and_reference_crossing() {
        let checked = crate::front_end::checked_program(CARRIER_STORE);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::inspect")
            .expect("inspect machine");
        let state = &checked.machine_states(machine)[0];
        let checked_trees::statement::StatementNode::LocalData(carrier) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("carrier declaration");
        };
        let (_, target) = authored_store(&checked, state.symbol, 2)
            .expect("source replay retains the established carrier");
        let path = vec![
            CheckedUnitStructuralPathSegment::Field("window".into()),
            CheckedUnitStructuralPathSegment::Field("destination".into()),
            CheckedUnitStructuralPathSegment::Referent,
        ];
        assert_eq!(target.root, carrier.symbol);
        assert_eq!(target.path, path);
        let source = crate::expression_preparation::source_custody::locate(
            &checked,
            state.symbol,
            2,
            CheckedScalarExpressionRole::AssignmentValue,
        )
        .expect("the assignment value is the primitive referent, not its descriptor");
        assert_eq!(source.primitive_type, PrimitiveType::U64);
        let value = checked_trees::CheckedCallScalarArgument::Pure(
            CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(11),
            },
        );
        validate_symbol_assignment(&checked, state.symbol, 2, carrier.symbol, &path, &value)
            .expect("the declared crossing and authored value agree");
        assert!(
            validate_symbol_assignment(
                &checked,
                state.symbol,
                2,
                checked.state_parameters(state)[0].symbol,
                &path,
                &value,
            )
            .is_err(),
            "the pointer's static origin cannot replace its actual carrier"
        );
        assert!(
            validate_symbol_assignment(
                &checked,
                state.symbol,
                2,
                carrier.symbol,
                &path[..2],
                &value,
            )
            .is_err(),
            "the pointer slot cannot replace the referent destination"
        );
    }

    #[test]
    fn primitive_store_source_rejects_shared_leaf_permission() {
        let source = CARRIER_STORE.replace("&mut u64", "&u64");
        let checked = crate::front_end::checked_program(&source);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::inspect")
            .expect("inspect machine");
        let state = &checked.machine_states(machine)[0];
        assert!(authored_store(&checked, state.symbol, 2).is_err());
    }
}
