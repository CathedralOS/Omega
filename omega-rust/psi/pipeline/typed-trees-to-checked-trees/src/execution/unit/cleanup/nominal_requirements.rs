//! Nominal cleanup boolean, scalar and caller requirements with their
//! diagnostics.

use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralScalarIntegerBoundKind, CheckedStructuralScalarIntegerBoundPlan,
    CheckedStructuralScalarIntegerBoundRequirementPlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupRequirementPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape, ContractProofFactKind,
    ContractProofFactOwner, DataMember, Diagnostic, ExpressionNode, PrimitiveType, ProofFact,
    StateParameter, SymbolHandle, TypedTrees, parameter_root_symbol, terminal_field_identity,
};

pub(crate) fn nominal_cleanup_boolean_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    cleanup_machine: &typed_trees::machine::Machine,
    cleanup_state: &typed_trees::state::State,
    cleanup_receiver: &StateParameter,
) -> Option<Vec<CheckedUnitNominalAffineCleanupRequirementPlan>> {
    let checked_requires =
        checked_requires_expressions(program, facts, cleanup_machine.symbol, cleanup_state.symbol)?;
    let requirements = checked_requires
        .into_iter()
        .map(|expression| {
            direct_boolean_field_requirement(
                program,
                cleanup_state.symbol,
                cleanup_receiver,
                expression,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    Some(canonical_nominal_cleanup_requirements(requirements))
}

pub(crate) fn nominal_scalar_caller_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    source_parameters: &[StateParameter],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<(
    Vec<CheckedUnitNominalAffineCallerRequirementPlan>,
    Vec<CheckedStructuralScalarIntegerBoundRequirementPlan>,
)> {
    // Both accepted callers preserve checked entry requirements unchanged. The
    // Unit lane admits only direct Boolean root facts; the scalar lane also
    // retains direct fixed-width integer literal bounds and pairwise parameter
    // relations for exact arithmetic. Wider bodies must instead consult
    // path-specific exit contexts.
    let caller_requires =
        checked_requires_expressions(program, facts, caller_machine.symbol, caller_state.symbol)?;
    let mut structural_requirements = Vec::new();
    let mut scalar_requirements = Vec::new();
    for expression in caller_requires {
        if let Some(requirement) = source_parameters.iter().enumerate().find_map(
            |(source_parameter_index, source_parameter)| {
                let source_parameter_index = u32::try_from(source_parameter_index).ok()?;
                direct_boolean_field_requirement(
                    program,
                    caller_state.symbol,
                    source_parameter,
                    expression,
                )
                .map(
                    |requirement| CheckedUnitNominalAffineCallerRequirementPlan {
                        source_parameter_index,
                        field_identity: requirement.field_identity,
                        expected: requirement.expected,
                    },
                )
            },
        ) {
            structural_requirements.push(requirement);
            continue;
        }
        scalar_requirements.push(direct_integer_requirement(
            program,
            caller_machine.symbol,
            caller_state,
            source_parameters,
            scalar_parameters,
            expression,
        )?);
    }
    structural_requirements.sort_by(|left, right| {
        left.source_parameter_index
            .cmp(&right.source_parameter_index)
            .then(left.field_identity.cmp(&right.field_identity))
            .then(left.expected.cmp(&right.expected))
    });
    structural_requirements.dedup();
    Some((structural_requirements, scalar_requirements))
}

pub(crate) fn nominal_cleanup_caller_boolean_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    source_parameters: &[StateParameter],
) -> Option<Vec<CheckedUnitNominalAffineCallerRequirementPlan>> {
    let (structural, scalar) = nominal_scalar_caller_requirements(
        program,
        facts,
        caller_machine,
        caller_state,
        source_parameters,
        &[],
    )?;
    scalar.is_empty().then_some(structural)
}

pub(crate) fn direct_integer_requirement(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    source_parameters: &[StateParameter],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedStructuralScalarIntegerBoundRequirementPlan> {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let parameter = |parameter_expression| {
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            0,
            parameter_expression,
        )?;
        if !place.segments.is_empty() {
            return None;
        }
        let facts::PlaceRoot::Symbol(root) = place.root else {
            return None;
        };
        let source_position = source_parameters.iter().position(|parameter| {
            parameter_root_symbol(machine, parameter) == root || parameter.symbol == root
        })?;
        let parameter_position = scalar_parameters
            .iter()
            .position(|parameter| parameter.source_position as usize == source_position)?;
        let primitive_type = scalar_parameters.get(parameter_position)?.primitive_type;
        if !matches!(
            primitive_type,
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        ) {
            return None;
        }
        Some((u32::try_from(parameter_position).ok()?, primitive_type))
    };
    let (parameter_position, primitive_type, kind, bound) = match (
        binary.operator,
        program.expression_table.expression(binary.left),
        program.expression_table.expression(binary.right),
    ) {
        (BinaryOperator::LessOrEqual, _, ExpressionNode::Integer(bound)) => {
            let (position, primitive_type) = parameter(binary.left)?;
            (
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                CheckedStructuralScalarIntegerBoundPlan::Literal(bound.clone()),
            )
        }
        (BinaryOperator::LessOrEqual, ExpressionNode::Integer(bound), _) => {
            let (position, primitive_type) = parameter(binary.right)?;
            (
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Lower,
                CheckedStructuralScalarIntegerBoundPlan::Literal(bound.clone()),
            )
        }
        (BinaryOperator::LessOrEqual, _, ExpressionNode::Binary(bound))
            if bound.operator == BinaryOperator::Subtract =>
        {
            let (position, primitive_type) = parameter(binary.left)?;
            let (subtrahend, subtrahend_type) = parameter(bound.right)?;
            let ExpressionNode::Integer(maximum) = program.expression_table.expression(bound.left)
            else {
                return None;
            };
            let maximum_matches = match primitive_type {
                PrimitiveType::U8 => maximum.value_u64() == Some(u64::from(u8::MAX)),
                PrimitiveType::U16 => maximum.value_u64() == Some(u64::from(u16::MAX)),
                PrimitiveType::U32 => maximum.value_u64() == Some(u64::from(u32::MAX)),
                PrimitiveType::U64 => maximum.value_u64() == Some(u64::MAX),
                PrimitiveType::I8 => maximum.value_i64() == Some(i64::from(i8::MAX)),
                PrimitiveType::I16 => maximum.value_i64() == Some(i64::from(i16::MAX)),
                PrimitiveType::I32 => maximum.value_i64() == Some(i64::from(i32::MAX)),
                PrimitiveType::I64 => maximum.value_i64() == Some(i64::MAX),
                _ => false,
            };
            (maximum_matches && primitive_type == subtrahend_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                CheckedStructuralScalarIntegerBoundPlan::MaximumMinusParameter(subtrahend),
            ))?
        }
        (BinaryOperator::LessOrEqual, ExpressionNode::Binary(bound), _)
            if bound.operator == BinaryOperator::Subtract =>
        {
            let (position, primitive_type) = parameter(binary.right)?;
            let (subtrahend, subtrahend_type) = parameter(bound.right)?;
            let ExpressionNode::Integer(minimum) = program.expression_table.expression(bound.left)
            else {
                return None;
            };
            let minimum_matches = match primitive_type {
                PrimitiveType::I8 => minimum.value_i64() == Some(i64::from(i8::MIN)),
                PrimitiveType::I16 => minimum.value_i64() == Some(i64::from(i16::MIN)),
                PrimitiveType::I32 => minimum.value_i64() == Some(i64::from(i32::MIN)),
                PrimitiveType::I64 => minimum.value_i64() == Some(i64::MIN),
                _ => false,
            };
            (minimum_matches && primitive_type == subtrahend_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Lower,
                CheckedStructuralScalarIntegerBoundPlan::SignedMinimumMinusParameter(subtrahend),
            ))?
        }
        (BinaryOperator::LessOrEqual, ExpressionNode::Binary(bound), _)
            if bound.operator == BinaryOperator::Add =>
        {
            let (position, primitive_type) = parameter(binary.right)?;
            let (addend, addend_type, minimum) = match (
                program.expression_table.expression(bound.left),
                program.expression_table.expression(bound.right),
            ) {
                (ExpressionNode::Integer(minimum), _) => {
                    let (addend, addend_type) = parameter(bound.right)?;
                    (addend, addend_type, minimum)
                }
                (_, ExpressionNode::Integer(minimum)) => {
                    let (addend, addend_type) = parameter(bound.left)?;
                    (addend, addend_type, minimum)
                }
                _ => return None,
            };
            let minimum_matches = match primitive_type {
                PrimitiveType::I8 => minimum.value_i64() == Some(i64::from(i8::MIN)),
                PrimitiveType::I16 => minimum.value_i64() == Some(i64::from(i16::MIN)),
                PrimitiveType::I32 => minimum.value_i64() == Some(i64::from(i32::MIN)),
                PrimitiveType::I64 => minimum.value_i64() == Some(i64::MIN),
                _ => false,
            };
            (minimum_matches && primitive_type == addend_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Lower,
                CheckedStructuralScalarIntegerBoundPlan::SignedMinimumPlusParameter(addend),
            ))?
        }
        (BinaryOperator::LessOrEqual, _, ExpressionNode::Binary(bound))
            if bound.operator == BinaryOperator::Add =>
        {
            let (position, primitive_type) = parameter(binary.left)?;
            let (addend, addend_type, maximum) = match (
                program.expression_table.expression(bound.left),
                program.expression_table.expression(bound.right),
            ) {
                (ExpressionNode::Integer(maximum), _) => {
                    let (addend, addend_type) = parameter(bound.right)?;
                    (addend, addend_type, maximum)
                }
                (_, ExpressionNode::Integer(maximum)) => {
                    let (addend, addend_type) = parameter(bound.left)?;
                    (addend, addend_type, maximum)
                }
                _ => return None,
            };
            let maximum_matches = match primitive_type {
                PrimitiveType::I8 => maximum.value_i64() == Some(i64::from(i8::MAX)),
                PrimitiveType::I16 => maximum.value_i64() == Some(i64::from(i16::MAX)),
                PrimitiveType::I32 => maximum.value_i64() == Some(i64::from(i32::MAX)),
                PrimitiveType::I64 => maximum.value_i64() == Some(i64::MAX),
                _ => false,
            };
            (maximum_matches && primitive_type == addend_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                CheckedStructuralScalarIntegerBoundPlan::SignedMaximumPlusParameter(addend),
            ))?
        }
        (BinaryOperator::LessOrEqual, _, ExpressionNode::Binary(bound))
            if bound.operator == BinaryOperator::Divide =>
        {
            let (position, primitive_type) = parameter(binary.left)?;
            let (divisor, divisor_type) = parameter(bound.right)?;
            let ExpressionNode::Integer(boundary) = program.expression_table.expression(bound.left)
            else {
                return None;
            };
            let bound_plan = quotient_extremum_bound_plan(primitive_type, boundary, divisor)?;
            (primitive_type == divisor_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                bound_plan,
            ))?
        }
        (BinaryOperator::LessOrEqual, ExpressionNode::Binary(bound), _)
            if bound.operator == BinaryOperator::Divide =>
        {
            let (position, primitive_type) = parameter(binary.right)?;
            let (divisor, divisor_type) = parameter(bound.right)?;
            let ExpressionNode::Integer(boundary) = program.expression_table.expression(bound.left)
            else {
                return None;
            };
            let bound_plan = quotient_extremum_bound_plan(primitive_type, boundary, divisor)?;
            (primitive_type == divisor_type).then_some((
                position,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Lower,
                bound_plan,
            ))?
        }
        (BinaryOperator::LessOrEqual, _, _) => {
            let (left, primitive_type) = parameter(binary.left)?;
            let (right, right_type) = parameter(binary.right)?;
            (primitive_type == right_type).then_some((
                left,
                primitive_type,
                CheckedStructuralScalarIntegerBoundKind::Upper,
                CheckedStructuralScalarIntegerBoundPlan::Parameter(right),
            ))?
        }
        _ => return None,
    };
    Some(CheckedStructuralScalarIntegerBoundRequirementPlan {
        parameter_position,
        primitive_type,
        kind,
        bound,
    })
}

pub(crate) fn quotient_extremum_bound_plan(
    primitive_type: PrimitiveType,
    boundary: &numerics::literals::IntegerLiteral,
    divisor: u32,
) -> Option<CheckedStructuralScalarIntegerBoundPlan> {
    let maximum_matches = match primitive_type {
        PrimitiveType::U8 => boundary.value_u64() == Some(u64::from(u8::MAX)),
        PrimitiveType::U16 => boundary.value_u64() == Some(u64::from(u16::MAX)),
        PrimitiveType::U32 => boundary.value_u64() == Some(u64::from(u32::MAX)),
        PrimitiveType::U64 => boundary.value_u64() == Some(u64::MAX),
        PrimitiveType::I8 => boundary.value_i64() == Some(i64::from(i8::MAX)),
        PrimitiveType::I16 => boundary.value_i64() == Some(i64::from(i16::MAX)),
        PrimitiveType::I32 => boundary.value_i64() == Some(i64::from(i32::MAX)),
        PrimitiveType::I64 => boundary.value_i64() == Some(i64::MAX),
        _ => false,
    };
    if maximum_matches {
        return Some(CheckedStructuralScalarIntegerBoundPlan::MaximumDivideParameter(divisor));
    }
    let minimum_matches = match primitive_type {
        PrimitiveType::I8 => boundary.value_i64() == Some(i64::from(i8::MIN)),
        PrimitiveType::I16 => boundary.value_i64() == Some(i64::from(i16::MIN)),
        PrimitiveType::I32 => boundary.value_i64() == Some(i64::from(i32::MIN)),
        PrimitiveType::I64 => boundary.value_i64() == Some(i64::MIN),
        _ => false,
    };
    minimum_matches
        .then_some(CheckedStructuralScalarIntegerBoundPlan::SignedMinimumDivideParameter(divisor))
}

pub(crate) fn nominal_cleanup_missing_requirement(
    source_parameter_index: u32,
    caller_requirements: &[CheckedUnitNominalAffineCallerRequirementPlan],
    required: &[CheckedUnitNominalAffineCleanupRequirementPlan],
) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
    required
        .iter()
        .find(|requirement| {
            !caller_requirements.iter().any(|caller| {
                caller.source_parameter_index == source_parameter_index
                    && caller.field_identity == requirement.field_identity
                    && caller.expected == requirement.expected
            })
        })
        .cloned()
}

pub(crate) fn canonical_nominal_cleanup_requirements(
    mut requirements: Vec<CheckedUnitNominalAffineCleanupRequirementPlan>,
) -> Vec<CheckedUnitNominalAffineCleanupRequirementPlan> {
    requirements.sort_by(|left, right| {
        left.field_identity
            .cmp(&right.field_identity)
            .then(left.expected.cmp(&right.expected))
    });
    requirements.dedup();
    requirements
}

pub(crate) fn nominal_cleanup_missing_requirement_diagnostic(
    program: &TypedTrees,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    source_parameter: &StateParameter,
    cleanup_machine: &typed_trees::machine::Machine,
    missing: CheckedUnitNominalAffineCleanupRequirementPlan,
) -> Diagnostic {
    let edge = format!(
        "automatic cleanup requires at Unit return edge from {} state {} after statement 0",
        crate::labels::machine_name(program, caller_machine.symbol),
        crate::labels::symbol_name(program, caller_state.symbol),
    );
    Diagnostic::error(format!(
        "cannot prove {edge}: missing {}.{} == {} required by {}",
        source_parameter.name.as_str(),
        missing.field_identity,
        missing.expected,
        crate::labels::machine_name(program, cleanup_machine.symbol),
    ))
}

pub(crate) fn scalar_nominal_cleanup_missing_requirement_diagnostic(
    program: &TypedTrees,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    return_statement_ordinal: u32,
    source_parameter: &StateParameter,
    cleanup_machine: &typed_trees::machine::Machine,
    missing: CheckedUnitNominalAffineCleanupRequirementPlan,
) -> Diagnostic {
    let edge = format!(
        "automatic cleanup requires at scalar return edge from {} state {} after statement {}",
        crate::labels::machine_name(program, caller_machine.symbol),
        crate::labels::symbol_name(program, caller_state.symbol),
        return_statement_ordinal,
    );
    Diagnostic::error(format!(
        "cannot prove {edge}: missing {}.{} == {} required by {}",
        source_parameter.name.as_str(),
        missing.field_identity,
        missing.expected,
        crate::labels::machine_name(program, cleanup_machine.symbol),
    ))
}

pub(crate) fn checked_requires_expressions(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
    let mut expressions = Vec::new();
    for (_, checked) in facts.proof.contract_facts.iter().filter(|(_, checked)| {
        matches!(checked.owner, ContractProofFactOwner::Machine { machine_symbol } if machine_symbol == machine)
            || matches!(checked.owner, ContractProofFactOwner::MachineState { machine_symbol, state_symbol } if machine_symbol == machine && state_symbol == state)
    }) {
        if checked.kind != ContractProofFactKind::Requires {
            return None;
        }
        let ProofFact::Expression(expression) = program.proof_facts.get(checked.fact) else {
            return None;
        };
        expressions.push(*expression);
    }
    Some(expressions)
}

pub(crate) fn direct_boolean_field_requirement(
    program: &TypedTrees,
    state: SymbolHandle,
    root_parameter: &StateParameter,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};

    let (field_expression, expected) = match program.expression_table.expression(expression) {
        ExpressionNode::Member(_) => (expression, true),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            if !matches!(
                program.expression_table.expression(unary.operand),
                ExpressionNode::Member(_)
            ) {
                return None;
            }
            (unary.operand, false)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let (field, literal) = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), ExpressionNode::Member(_)) => {
                    (binary.right, *literal)
                }
                (ExpressionNode::Member(_), ExpressionNode::Boolean(literal)) => {
                    (binary.left, *literal)
                }
                _ => return None,
            };
            (
                field,
                if binary.operator == BinaryOperator::Equal {
                    literal
                } else {
                    !literal
                },
            )
        }
        _ => return None,
    };
    let place =
        crate::flow::canonical_place_from_expression_in_state(program, state, 0, field_expression)?;
    let [facts::PlaceSegment::Field { symbol }] = place.segments.as_slice() else {
        return None;
    };
    if place.root != facts::PlaceRoot::Symbol(root_parameter.symbol)
        || !program.data_definitions().iter().any(|data| {
            program.data_members(data).iter().any(|member| {
                matches!(member, DataMember::Field(field)
                    if field.symbol == *symbol
                        && program.primitive_type_reference(field.type_reference)
                            == Some(PrimitiveType::Bool))
            })
        })
    {
        return None;
    }
    Some(CheckedUnitNominalAffineCleanupRequirementPlan {
        field_identity: terminal_field_identity(program, *symbol)?,
        expected,
    })
}

pub(crate) fn is_bounded_nominal_cleanup_record(shape: &CheckedUnitStructuralTypeShape) -> bool {
    match shape {
        CheckedUnitStructuralTypeShape::Record { fields } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && matches!(
                    &field.field_type,
                    CheckedUnitStructuralFieldType::Scalar(
                        PrimitiveType::Bool
                            | PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::Addr
                    )
                )
        }),
        CheckedUnitStructuralTypeShape::Reference { .. }
        | CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
        | CheckedUnitStructuralTypeShape::ByteSequence(_)
        | CheckedUnitStructuralTypeShape::FixedArray { .. }
        | CheckedUnitStructuralTypeShape::BorrowedSliceView { .. }
        | CheckedUnitStructuralTypeShape::Sum { .. }
        | CheckedUnitStructuralTypeShape::Mixed { .. } => false,
    }
}
