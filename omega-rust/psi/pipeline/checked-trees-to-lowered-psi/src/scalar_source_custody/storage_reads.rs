//! Rejoin scalar reads with authored bindings, storage, and operand positions.

use super::*;

/// Reads retain their occurrence, operand position, and exact binding or place.
/// Unary and cast wrappers do not change the read's identity. Greater-than
/// comparisons use the same operand normalization as the checked producer.
pub(super) fn validate(
    checked: &CheckedTrees,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    source: &SourceRoot,
) -> Result<(), LoweringError> {
    let (_, retained) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(binding.state, binding.statement_ordinal, binding.role)
        .ok_or(LoweringError::Unsupported(
            "storage read has no exact scalar expression",
        ))?;
    let expression = if binding.role == CheckedScalarExpressionRole::Guard {
        guard_subject(checked, source.expression)
    } else {
        source.expression
    };
    validate_expression(
        checked,
        binding.state,
        binding.statement_ordinal,
        expression,
        retained,
    )
}

/// Rejoin fixed-integer and Boolean reads to their exact source scope.
/// Computation operands use this same check without inventing a pure-plan row.
pub(crate) fn validate_expression(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    retained: &CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let (_, state) = authored_state(checked, state)?;
    let mut authored_reads = Vec::new();
    collect_authored_storage_reads(
        checked,
        state,
        statement,
        expression,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut authored_reads,
    )?;
    let namespace = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            checked
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .map(|parameter| parameter.symbol)
        .chain(
            checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement as usize)
                .filter_map(|statement| match statement {
                    StatementNode::LocalData(local)
                        if !local.is_mutable
                            && local.initial_value.is_valid()
                            && checked
                                .primitive_type_reference(local.type_reference)
                                .is_some() =>
                    {
                        Some(local.symbol)
                    }
                    _ => None,
                }),
        )
        .collect::<Vec<_>>();
    let mut retained_reads = Vec::new();
    collect_scalar_storage_reads(retained, &namespace, &mut Vec::new(), &mut retained_reads);
    if retained_reads != authored_reads {
        return unsupported("scalar read differs from its authored binding or mutable place");
    }
    Ok(())
}

/// Guard lowering erases the builtin `subject == true` wrapper, not its subject.
fn guard_subject(checked: &CheckedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return expression;
    };
    if binary.operator != checked_trees::expression::BinaryOperator::Equal
        || checked
            .facts
            .operators
            .expression_use(expression)
            .is_some_and(|operator| {
                operator.status != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            })
    {
        return expression;
    }
    match (
        checked.expression_table.expression(binary.left),
        checked.expression_table.expression(binary.right),
    ) {
        (ExpressionNode::Boolean(true), _) => binary.right,
        (_, ExpressionNode::Boolean(true)) => binary.left,
        _ => expression,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadKind {
    Storage,
    Parameter,
    Local,
}

type StorageReadOccurrence = (Vec<usize>, symbols::SymbolHandle, PrimitiveType, ReadKind);

fn authored_storage_read(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    before: u32,
    name: &checked_trees::expression::TableNamePath,
) -> Result<Option<(symbols::SymbolHandle, PrimitiveType, ReadKind)>, LoweringError> {
    if !name.symbol.is_valid()
        || name.symbol != name.head_symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return Ok(None);
    }
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(before as usize)
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == name.symbol => Some(local),
            _ => None,
        });
    if let Some(local) = locals.next() {
        if locals.next().is_some() {
            return unsupported("scalar storage read has duplicate authored declarations");
        }
        return Ok(checked
            .primitive_type_reference(local.type_reference)
            .filter(|primitive| local.is_mutable || supported_mutable_parameter(*primitive))
            .map(|primitive| {
                (
                    local.symbol,
                    primitive,
                    if local.is_mutable {
                        ReadKind::Storage
                    } else {
                        ReadKind::Local
                    },
                )
            }));
    }
    let mut parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == name.symbol);
    let Some(parameter) = parameters.next() else {
        return Ok(None);
    };
    if parameters.next().is_some() {
        return unsupported("scalar storage read has duplicate authored parameters");
    }
    Ok(checked
        .primitive_type_reference(parameter.type_reference)
        .filter(|primitive| parameter.is_mutable || supported_mutable_parameter(*primitive))
        .map(|primitive| {
            (
                parameter.symbol,
                primitive,
                if parameter.is_mutable {
                    ReadKind::Storage
                } else {
                    ReadKind::Parameter
                },
            )
        }))
}

fn collect_authored_storage_reads(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    before: u32,
    expression: ExpressionHandle,
    path: &mut Vec<usize>,
    active: &mut Vec<ExpressionHandle>,
    reads: &mut Vec<StorageReadOccurrence>,
) -> Result<(), LoweringError> {
    use checked_trees::expression::BinaryOperator;
    if !checked.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return unsupported("scalar storage source contains a stale or cyclic expression");
    }
    active.push(expression);
    match checked.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            if let Some((symbol, primitive, kind)) =
                authored_storage_read(checked, state, before, name)?
            {
                reads.push((path.clone(), symbol, primitive, kind));
            }
        }
        ExpressionNode::Binary(binary) => {
            let (left, right) = if matches!(
                binary.operator,
                BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
            ) {
                (binary.right, binary.left)
            } else {
                (binary.left, binary.right)
            };
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_authored_storage_reads(
                    checked, state, before, operand, path, active, reads,
                )?;
                path.pop();
            }
        }
        ExpressionNode::Unary(unary) => {
            collect_authored_storage_reads(
                checked,
                state,
                before,
                unary.operand,
                path,
                active,
                reads,
            )?;
        }
        ExpressionNode::Cast(cast) => {
            collect_authored_storage_reads(
                checked, state, before, cast.value, path, active, reads,
            )?;
        }
        ExpressionNode::Indexed(indexed) => {
            path.push(0);
            collect_authored_storage_reads(
                checked,
                state,
                before,
                indexed.index,
                path,
                active,
                reads,
            )?;
            path.pop();
        }
        // These source forms are owned by literal, structural, or computation
        // custody. Their roots are not primitive scalar content reads.
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    active.pop();
    Ok(())
}

fn collect_scalar_storage_reads(
    expression: &CheckedScalarExpression,
    namespace: &[symbols::SymbolHandle],
    path: &mut Vec<usize>,
    reads: &mut Vec<StorageReadOccurrence>,
) {
    match expression {
        CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type,
        } => {
            reads.push((path.clone(), *symbol, *primitive_type, ReadKind::Storage));
        }
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_scalar_storage_reads(operand, namespace, path, reads);
                path.pop();
            }
        }
        CheckedScalarExpression::StructuralParameterIndexedRead { index, .. } => {
            path.push(0);
            collect_scalar_storage_reads(index, namespace, path, reads);
            path.pop();
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerTrappingCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. } => {
            collect_scalar_storage_reads(operand, namespace, path, reads);
        }
        CheckedScalarExpression::Boolean(expression) => {
            collect_boolean_storage_reads(expression, namespace, path, reads);
        }
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        }
        | CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => {
            if supported_mutable_parameter(*primitive_type) {
                reads.push((
                    path.clone(),
                    namespace.get(*position).copied().unwrap_or_default(),
                    *primitive_type,
                    if matches!(expression, CheckedScalarExpression::Parameter { .. }) {
                        ReadKind::Parameter
                    } else {
                        ReadKind::Local
                    },
                ));
            }
        }
        CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. } => {}
    }
}

fn collect_boolean_storage_reads(
    expression: &CheckedBooleanExpression,
    namespace: &[symbols::SymbolHandle],
    path: &mut Vec<usize>,
    reads: &mut Vec<StorageReadOccurrence>,
) {
    match expression {
        CheckedBooleanExpression::StorageRead { symbol } => {
            reads.push((
                path.clone(),
                *symbol,
                PrimitiveType::Bool,
                ReadKind::Storage,
            ));
        }
        CheckedBooleanExpression::Not(operand) => {
            collect_boolean_storage_reads(operand, namespace, path, reads);
        }
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_boolean_storage_reads(operand, namespace, path, reads);
                path.pop();
            }
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_scalar_storage_reads(operand, namespace, path, reads);
                path.pop();
            }
        }
        CheckedBooleanExpression::Parameter { position }
        | CheckedBooleanExpression::Local { position } => {
            reads.push((
                path.clone(),
                namespace.get(*position).copied().unwrap_or_default(),
                PrimitiveType::Bool,
                if matches!(expression, CheckedBooleanExpression::Parameter { .. }) {
                    ReadKind::Parameter
                } else {
                    ReadKind::Local
                },
            ));
        }
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
    }
}
