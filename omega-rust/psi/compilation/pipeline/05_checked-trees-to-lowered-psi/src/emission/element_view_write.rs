//! Runtime-indexed element stores through exact mutable element views.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, Operation, OperationKind,
    OperationResult, PrimitiveType, StructuralTypeDeclaration, StructuralTypeShape,
    ValueDeclaration, allocate_dense, direct_expression_contains_short_circuit,
    emit_direct_expression, obligation_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use checked_trees::{CheckedElementViewWritePlan, expression::ExpressionNode};

/// The store's value under the scalar-field store's value vocabulary: an
/// authored expression rejoined at `AssignmentValue`, or the scalar the
/// same statement's call just established. Computation payloads keep no
/// direct lowering yet; the plan only mints them beside a record field.
pub(crate) fn element_store_scalar_value(
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement_index: u32,
    value: &checked_trees::CheckedStructuralScalarFieldStoreValue,
    element: PrimitiveType,
    values: &[ValueDeclaration],
) -> Result<LoweredDirectExpression, LoweringError> {
    match value {
        checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(_) => bindings.expression_at(
            checked,
            state,
            statement_index,
            CheckedScalarExpressionRole::AssignmentValue,
        ),
        checked_trees::CheckedStructuralScalarFieldStoreValue::ScalarResult { position } => {
            let position = usize::try_from(*position).map_err(|_| {
                LoweringError::Unsupported("element-view store result position exceeds usize")
            })?;
            if position.checked_add(1) != Some(values.len()) {
                return unsupported(
                    "element-view store call result is not this statement's established scalar result",
                );
            }
            let scalar_type = terminal_scalar_type(element)?;
            if values[position].scalar_type != scalar_type {
                return unsupported("element-view store call result differs from its element type");
            }
            Ok(LoweredDirectExpression::Local {
                position,
                scalar_type,
            })
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(_) => {
            unsupported("element-view write keeps a computation value")
        }
    }
}

/// A mutable-view local's authored establishment: the statement binding its
/// symbol must still carry a `&mut` declared type, the same rejoin a
/// `&mut`-borrowed destination parameter gets through its authored parameter.
fn authored_view_local(
    checked: &CheckedTrees,
    statements: &[typed_trees::statement::StatementNode],
    symbol: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let mut found = false;
    for statement in statements {
        let checked_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.symbol != symbol {
            continue;
        }
        if found {
            return unsupported("element-view write names a duplicated view local");
        }
        found = true;
        if !matches!(
            checked
                .type_reference_table
                .type_reference(local.type_reference),
            checked_trees::types::TypeReferenceNode::Reference {
                access: language_core::ReferenceAccess::Mutable,
                ..
            }
        ) {
            return unsupported("element-view write local lost its authored mutability");
        }
    }
    if !found {
        return unsupported("element-view write names an unknown view local");
    }
    Ok(())
}

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    write: &CheckedElementViewWritePlan,
) -> Result<(), LoweringError> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(assignment.target)
    else {
        return unsupported("element-view write lost its authored index");
    };
    let (destination_root, source_root, source_path_is_empty) = match write.destination {
        checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter { position } => {
            let parameter_symbol = checked
                .state_parameters(state)
                .get(position as usize)
                .map(|parameter| parameter.symbol)
                .ok_or(LoweringError::Unsupported(
                    "element-view write has no authored parameter",
                ))?;
            let source =
                crate::emission::call_source_custody::projected_receivers::store_destination(
                    checked,
                    machine,
                    state_symbol,
                    None,
                    indexed.collection,
                )?;
            (parameter_symbol, source.root, source.path.is_empty())
        }
        checked_trees::CheckedStructuralScalarFieldStoreDestination::Local { symbol } => {
            authored_view_local(
                checked,
                checked.statement_table.statements(state.statement_nodes),
                symbol,
            )?;
            // The view local is the destination's own carrier: the indexed
            // collection must be its bare authored name, which the projected
            // receiver walk has no arm for since it is neither owned storage
            // nor a shared-borrow loan.
            let ExpressionNode::Name(name) =
                checked.expression_table.expression(indexed.collection)
            else {
                return unsupported("element-view write substituted its view local collection");
            };
            if !name.symbol.is_valid()
                || name.head_symbol != name.symbol
                || name.symbol != symbol
                || checked
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
            {
                return unsupported("element-view write substituted its view local collection");
            }
            (symbol, symbol, true)
        }
    };
    if owner.symbol != machine
        || source_root != destination_root
        || !source_path_is_empty
        || !validation::place_has_builtin_coordinates(
            &checked.typed,
            owner,
            Some(state),
            assignment.target,
        )
    {
        return unsupported("element-view write changed its authored mutable destination");
    }
    if checked.facts.operators.uses.iter().any(|(_, selected)| {
        selected.expression == assignment.target
            && (selected.spelling != language_core::OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
    }) {
        return unsupported("element-view write changed its selected index meaning");
    }
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state_symbol,
            write.statement_index,
            CheckedScalarExpressionRole::AssignmentIndex { depth: 0 },
        )
        .ok_or(LoweringError::Unsupported(
            "element-view write lost its operand source",
        ))?;
    if binding.expression != indexed.index || expression != &write.index {
        return unsupported("element-view write substituted an authored operand");
    }
    crate::expression_preparation::source_custody::validate_pure(
        checked,
        binding,
        terminal_scalar_type(
            crate::expression_preparation::source_custody::locate(
                checked,
                state_symbol,
                write.statement_index,
                CheckedScalarExpressionRole::AssignmentIndex { depth: 0 },
            )?
            .primitive_type,
        )?,
    )?;
    let value_expressions = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .filter(|expression| {
            expression.state == state_symbol
                && expression.statement_ordinal == write.statement_index
                && expression.role == CheckedScalarExpressionRole::AssignmentValue
        })
        .collect::<Vec<_>>();
    match &write.value {
        checked_trees::CheckedStructuralScalarFieldStoreValue::ScalarResult { .. } => {
            if !value_expressions.is_empty() {
                return unsupported("element-view write replaced a selected RHS with a result");
            }
            if !matches!(
                checked.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            ) {
                return unsupported("element-view write call result has no authored call");
            }
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(retained) => {
            let (binding, expression) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(
                    state_symbol,
                    write.statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )
                .ok_or(LoweringError::Unsupported(
                    "element-view write lost its operand source",
                ))?;
            if binding.expression != assignment.value || expression != retained {
                return unsupported("element-view write substituted an authored operand");
            }
            crate::expression_preparation::source_custody::validate_pure(
                checked,
                binding,
                terminal_scalar_type(write.element)?,
            )?;
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(_) => {
            return unsupported("element-view write kept a computation value");
        }
    }
    Ok(())
}

pub(crate) fn emit(
    destination_place: semantic_vocabulary::PlaceId,
    destination_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
    index: &LoweredDirectExpression,
    value: &LoweredDirectExpression,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let element = structural_types
        .iter()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::ElementView { element } if declaration.id == destination_type => {
                Some(element)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "element-view write destination is not an element view",
        ))?;
    let element_scalar = structural_types
        .iter()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) if declaration.id == element => {
                Some(scalar_type)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "element-view write element is not a scalar leaf",
        ))?;
    if !matches!(
        index.scalar_type(),
        semantic_vocabulary::ScalarType::Integer(_)
    ) || value.scalar_type() != element_scalar
        || direct_expression_contains_short_circuit(index)
        || direct_expression_contains_short_circuit(value)
    {
        return unsupported("element-view write requires scalar operands");
    }
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    validate_direct_parameter_types(index, &types)?;
    validate_direct_parameter_types(value, &types)?;
    let index = super::emit_byte_index(index, values, next_value, next_obligation, operations)?;
    let value = emit_direct_expression(value, values, next_value, operations);
    let length = value_id(allocate_dense(next_value)?);
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: length,
            scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
        }),
        kind: OperationKind::ElementViewLength {
            source: destination_place,
        },
    });
    Ok(OperationKind::ElementViewWrite {
        destination: destination_place,
        index,
        value,
        length,
        obligation: obligation_id(allocate_dense(next_obligation)?),
    })
}
