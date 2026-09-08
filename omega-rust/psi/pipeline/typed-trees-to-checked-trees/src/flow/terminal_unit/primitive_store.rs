//! Checked whole-primitive stores through exact exclusive reference parameters.

use super::*;

pub(super) fn build_write_only_primitive_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statements: &[StatementNode],
    scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
    selected_scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
) -> Option<CheckedUnitEffectOperationPlan> {
    let result_local = scalar_result_local.or(selected_scalar_result_local);
    let (statement_index, assignment) = match (result_local, statements) {
        (None, [StatementNode::Assignment(assignment)]) => (0, assignment),
        (
            Some(result),
            [
                StatementNode::LocalData(_),
                StatementNode::Assignment(assignment),
            ],
        ) if result.statement_index == 0 && result.binding_ordinal == 0 => (1, assignment),
        _ => return None,
    };
    let [destination] = structural_parameters else {
        return None;
    };
    if destination.is_self
        || destination.position != 0
        || destination.multiplicity != Multiplicity::Unrestricted
        || !matches!(
            destination.access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
    {
        return None;
    }
    let CheckedUnitStructuralTypeShape::PrimitiveScalar(destination_type) = shapes
        .types
        .get(&destination.type_identity)
        .map(|declaration| &declaration.shape)?
    else {
        return None;
    };
    let parameter = program
        .state_parameters(state)
        .get(usize::try_from(destination.position).ok()?)?;
    if program.state_parameters(state).len()
        != structural_parameters.len() + scalar_parameters.len()
    {
        return None;
    }
    if parameter.is_self || parameter.is_const || !parameter.is_mutable {
        return None;
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let expected_access = match access {
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
        language_semantics::ReferenceAccess::Shared => return None,
    };
    if destination.access != expected_access
        || program.primitive_type_reference(*referee) != Some(*destination_type)
    {
        return None;
    }
    let target = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        assignment.target,
    )?;
    if target.root != facts::PlaceRoot::Symbol(parameter.symbol) || !target.segments.is_empty() {
        return None;
    }
    let frame = &facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame;
    let expected_frame_path = format!("$P{}", destination.position);
    let exact_frame =
        matches!(frame.complete_paths(), Some([path]) if path == &expected_frame_path);
    // Before provider selection, a boundary-operator initializer makes the
    // source write frame opaque. The selected scalar lane replaces that one
    // initializer with a checked-body call whose complete scalar-only shape is
    // replayed below; the exact two-statement body leaves the destination
    // assignment as its only caller-visible write.
    let unresolved_selected_frame = selected_scalar_result_local.is_some()
        && frame.completeness() == facts::WriteFrameCompleteness::Opaque;
    if !exact_frame && !unresolved_selected_frame {
        return None;
    }
    let value = facts.values.scalar_expressions.expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
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
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Some((*position, *primitive_type)),
        CheckedScalarExpression::Boolean(expression) => match expression.as_ref() {
            checked_trees::CheckedBooleanExpression::Parameter { position } => {
                Some((*position, PrimitiveType::Bool))
            }
            _ => None,
        },
        _ => None,
    };
    let direct_parameter_is_exact = direct_parameter.is_some_and(|(position, primitive_type)| {
        scalar_parameters
            .get(position)
            .is_some_and(|parameter| parameter.primitive_type == primitive_type)
            && *destination_type == primitive_type
    });
    // The admitted result is binding zero, after the dense scalar inputs;
    // its source binding ordinal is not its scalar-expression position.
    let direct_result_is_exact = matches!(
        (result_local, value),
        (
            Some(result),
            CheckedScalarExpression::Local {
                position,
                primitive_type,
            },
        ) if *position == scalar_parameters.len()
            && *primitive_type == result.primitive_type
            && *destination_type == result.primitive_type
            && matches!(
                result.primitive_type,
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
    if !(direct_literal || direct_parameter_is_exact || direct_result_is_exact)
        || crate::values::scalar_expression_type(value) != Some(*destination_type)
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        statement_index,
        destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter {
            parameter_index: 0,
        },
        value: value.clone(),
    })
}

/// Find only initialized primitive storage established before this occurrence.
pub(super) fn primitive_local_before<'program>(
    program: &'program TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::statement::TableLocalData> {
    let mut locals = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement_index)?
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        });
    let local = locals.next()?;
    if locals.next().is_some()
        || !local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || !matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            TypeReferenceNode::Named { .. }
        )
        || program
            .primitive_type_reference(local.type_reference)
            .is_none()
    {
        return None;
    }
    Some(local)
}

/// The shared statement roster validates the complete state write frame.
pub(super) fn build_primitive_store_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<CheckedUnitEffectOperationPlan> {
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        assignment.target,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    if !place.segments.is_empty() {
        return None;
    }
    let (destination, primitive_type) = if let Some(local) = primitive_local_before(
        program,
        state,
        usize::try_from(statement_index).ok()?,
        symbol,
    ) {
        (
            checked_trees::CheckedPrimitiveStoreDestination::Local { symbol },
            program.primitive_type_reference(local.type_reference)?,
        )
    } else {
        let (parameter_index, destination) =
            structural_parameters
                .iter()
                .enumerate()
                .find(|(_, destination)| {
                    program
                        .state_parameters(state)
                        .get(destination.position as usize)
                        .is_some_and(|parameter| parameter.symbol == symbol)
                })?;
        if destination.is_self
            || destination.multiplicity != Multiplicity::Unrestricted
            || !destination.qualifications.is_empty()
            || !matches!(
                destination.access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            )
        {
            return None;
        }
        let parameter = program
            .state_parameters(state)
            .get(destination.position as usize)?;
        if parameter.is_const || !parameter.is_mutable {
            return None;
        }
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        let expected_access = match access {
            language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
            language_semantics::ReferenceAccess::WriteOnly => {
                CheckedStructuralAccess::WriteOnlyBorrow
            }
            language_semantics::ReferenceAccess::Shared => return None,
        };
        if destination.access != expected_access
            || !matches!(
                program.type_reference_table.type_reference(*referee),
                TypeReferenceNode::Named { .. }
            )
        {
            return None;
        }
        (
            checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            program.primitive_type_reference(*referee)?,
        )
    };
    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    if binding.expression != assignment.value
        || binding.destination != symbol
        || crate::values::scalar_expression_type(value) != Some(primitive_type)
        || !scalar_custody_is_exact(program, facts, state, binding, value, primitive_type)
        || matches!(value, CheckedScalarExpression::Boolean(expression) if checked_boolean_contains_short_circuit(expression))
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        statement_index,
        destination,
        value: value.clone(),
    })
}

pub(super) fn scalar_custody_is_exact(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> bool {
    let Ok(statement_index) = usize::try_from(binding.statement_ordinal) else {
        return false;
    };
    let Some(prefix) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement_index)
    else {
        return false;
    };
    let symbols = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .map(|parameter| parameter.symbol)
        .chain(prefix.iter().filter_map(|statement| {
            match statement {
                StatementNode::LocalData(local)
                    if !local.is_mutable
                        && program
                            .primitive_type_reference(local.type_reference)
                            .is_some() =>
                {
                    Some(local.symbol)
                }
                _ => None,
            }
        }));
    if !symbols.eq(facts
        .values
        .scalar_expressions
        .binding_symbols
        .span_or_empty(binding.symbols)
        .iter()
        .copied())
    {
        return false;
    }
    crate::values::lower_unit_scalar_argument(
        program,
        &facts.operators,
        state,
        statement_index,
        binding.expression,
        primitive_type,
    )
    .as_ref()
        == Some(value)
}
