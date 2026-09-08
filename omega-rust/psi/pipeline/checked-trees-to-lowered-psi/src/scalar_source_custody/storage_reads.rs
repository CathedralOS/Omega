//! Rejoin storage observations with authored expressions and operand positions.

use super::*;

/// Mutable reads retain their occurrence, operand position, and exact place.
/// Unary and cast wrappers do not change the read's identity. Greater-than
/// comparisons use the same operand normalization as the checked producer.
pub(super) fn validate(
    checked: &CheckedTrees,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    source: &SourceRoot,
) -> Result<(), LoweringError> {
    let (_, state) = authored_state(checked, binding.state)?;
    let (_, retained) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(binding.state, binding.statement_ordinal, binding.role)
        .ok_or(LoweringError::Unsupported(
            "storage read has no exact scalar expression",
        ))?;
    let mut authored_reads = Vec::new();
    collect_authored_storage_reads(
        checked,
        state,
        binding.statement_ordinal,
        source.expression,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut authored_reads,
    )?;
    let mut retained_reads = Vec::new();
    collect_scalar_storage_reads(retained, &mut Vec::new(), &mut retained_reads);
    if retained_reads != authored_reads {
        return unsupported("scalar storage read differs from its authored mutable place");
    }
    Ok(())
}

type StorageReadOccurrence = (Vec<usize>, symbols::SymbolHandle, PrimitiveType);

fn authored_storage_read(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    before: u32,
    name: &checked_trees::expression::TableNamePath,
) -> Result<Option<(symbols::SymbolHandle, PrimitiveType)>, LoweringError> {
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
        return Ok(local
            .is_mutable
            .then(|| {
                checked
                    .primitive_type_reference(local.type_reference)
                    .map(|primitive| (local.symbol, primitive))
            })
            .flatten());
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
    Ok(parameter
        .is_mutable
        .then(|| {
            checked
                .primitive_type_reference(parameter.type_reference)
                .map(|primitive| (parameter.symbol, primitive))
        })
        .flatten())
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
            if let Some((symbol, primitive)) = authored_storage_read(checked, state, before, name)?
            {
                reads.push((path.clone(), symbol, primitive));
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
    path: &mut Vec<usize>,
    reads: &mut Vec<StorageReadOccurrence>,
) {
    match expression {
        CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type,
        } => {
            reads.push((path.clone(), *symbol, *primitive_type));
        }
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_scalar_storage_reads(operand, path, reads);
                path.pop();
            }
        }
        CheckedScalarExpression::StructuralParameterIndexedRead { index, .. } => {
            path.push(0);
            collect_scalar_storage_reads(index, path, reads);
            path.pop();
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerTrappingCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. } => {
            collect_scalar_storage_reads(operand, path, reads);
        }
        CheckedScalarExpression::Boolean(expression) => {
            collect_boolean_storage_reads(expression, path, reads);
        }
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. } => {}
    }
}

fn collect_boolean_storage_reads(
    expression: &CheckedBooleanExpression,
    path: &mut Vec<usize>,
    reads: &mut Vec<StorageReadOccurrence>,
) {
    match expression {
        CheckedBooleanExpression::StorageRead { symbol } => {
            reads.push((path.clone(), *symbol, PrimitiveType::Bool));
        }
        CheckedBooleanExpression::Not(operand) => {
            collect_boolean_storage_reads(operand, path, reads);
        }
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_boolean_storage_reads(operand, path, reads);
                path.pop();
            }
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_scalar_storage_reads(operand, path, reads);
                path.pop();
            }
        }
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::Local { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
    }
}
