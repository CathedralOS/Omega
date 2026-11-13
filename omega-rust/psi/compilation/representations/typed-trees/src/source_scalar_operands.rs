//! Determine source scalar carriers and literal landing without runtime proof state.
use crate::{
    TypedTrees,
    expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
        TableCallExpression,
    },
    types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
};
use diagnostics::Diagnostic;
use numerics::{arithmetic::ArithmeticDomain, bignum::BigInt};

pub fn readable_observation_type_reference(
    program: &TypedTrees,
    declared: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let mut reference = declared;
    let mut observed = declared;
    for _ in 0..=program.type_reference_table.type_reference_count() {
        if !program.contains_type_reference(reference) {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() => {
                reference = *referee;
                observed = *referee;
            }
            TypeReferenceNode::Reference { .. } => return None,
            _ => return Some(observed),
        }
    }
    None
}
pub fn scalar_observation_meaning(
    program: &TypedTrees,
    declared: TypeReferenceHandle,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    let observed = readable_observation_type_reference(program, declared)?;
    Some((
        program.primitive_type_reference(observed)?,
        program.arithmetic_domain_for_type_reference(observed),
    ))
}

pub fn anonymous_integer_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<BigInt> {
    crate::closed_numeric::evaluate_anonymous_value::<true>(
        program,
        expression,
        &[],
        &mut |expression| {
            crate::closed_numeric::has_anonymous_operator_meaning(program, expression)
        },
    )?
    .value
    .to_integer_exact()
}

pub fn anonymous_integer_math_literal(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<semantic_vocabulary::IntegerMathLiteral> {
    let value = anonymous_integer_value(program, expression)?;
    let (high, low) = value.abs().div_rem(&BigInt::from_u128(1u128 << 64))?;
    let magnitude = (u128::from(high.to_u64()?) << 64) | u128::from(low.to_u64()?);
    semantic_vocabulary::IntegerMathLiteral::new(value.is_negative(), magnitude).ok()
}

/// This is an integer destination query. Address-width obligations remain separate.
pub fn anonymous_integer_fits(
    program: &TypedTrees,
    primitive: PrimitiveType,
    expression: ExpressionHandle,
) -> Result<bool, Diagnostic> {
    fn visit(
        program: &TypedTrees,
        primitive: PrimitiveType,
        expression: ExpressionHandle,
        active: &mut Vec<ExpressionHandle>,
    ) -> Result<bool, Diagnostic> {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return Err(Diagnostic::error(
                "anonymous integer landing has a stale or cyclic source expression",
            ));
        }
        if let Some(dispatch) = program.expression_table.match_node(expression) {
            let arms = program.expression_table.match_arms(dispatch.arms);
            if arms.is_empty() {
                return Ok(false);
            }
            active.push(expression);
            let mut all_fit = true;
            for arm in arms {
                all_fit &= visit(program, primitive, arm.value, active)?;
            }
            active.pop();
            return Ok(all_fit);
        }
        if !primitive.accepts_integer_literal() {
            return Ok(false);
        }
        let Some(evaluated) = crate::closed_numeric::evaluate_anonymous_value::<true>(
            program,
            expression,
            &[],
            &mut |expression| {
                crate::closed_numeric::has_anonymous_operator_meaning(program, expression)
            },
        ) else {
            return Ok(false);
        };
        let value = evaluated.value.to_integer_exact().ok_or_else(|| {
            Diagnostic::error(format!(
                "anonymous value `{}` is not an integer and cannot land in `{}`",
                evaluated.value,
                primitive.name()
            ))
            .with_source_span(program.expression_table.source_span(expression))
        })?;
        if (primitive == PrimitiveType::Addr && value.to_u64().is_some())
            || crate::closed_numeric::land_integer(&value, primitive).is_some()
        {
            return Ok(true);
        }
        Err(Diagnostic::error(format!(
            "anonymous integer value `{value}` does not fit destination `{}`",
            primitive.name()
        ))
        .with_source_span(program.expression_table.source_span(expression)))
    }
    visit(program, primitive, expression, &mut Vec::new())
}

pub fn validate_anonymous_integer_primitive_range(
    program: &TypedTrees,
    primitive: PrimitiveType,
    expression: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Err(mut diagnostic) = anonymous_integer_fits(program, primitive, expression) {
        diagnostic.message.push_str(&format!(" in {owner}"));
        diagnostics.push(diagnostic);
    }
}
pub fn validate_anonymous_integer_range(
    program: &TypedTrees,
    destination: TypeReferenceHandle,
    expression: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(primitive) = program.primitive_type_reference(destination) {
        validate_anonymous_integer_primitive_range(
            program,
            primitive,
            expression,
            owner,
            diagnostics,
        );
    }
}

pub fn builtin_scalar_operand_references(
    program: &TypedTrees,
    binary: &TableBinaryExpression,
    references: [Option<TypeReferenceHandle>; 2],
    expected: Option<TypeReferenceHandle>,
    mut lands: impl FnMut(ExpressionHandle, PrimitiveType) -> bool,
) -> Option<[TypeReferenceHandle; 2]> {
    let mut reference = |index: usize| {
        if let Some(reference) = references[index] {
            return Some(reference);
        }
        let destination = references[1 - index].or(expected)?;
        let (primitive, _) = scalar_observation_meaning(program, destination)?;
        let primitive = if references[1 - index].is_some() {
            crate::operator::integer_peer_operand_carrier(binary.operator, primitive, index == 1)
        } else {
            primitive
        };
        let expression = [binary.left, binary.right][index];
        if primitive == PrimitiveType::Addr {
            if !binary.operator.is_comparison()
                || references[1 - index].is_none()
                || !anonymous_integer_fits(program, primitive, expression).ok()?
            {
                return None;
            }
        } else if !primitive.is_integer() || !lands(expression, primitive) {
            return None;
        }
        crate::source_operator_types::builtin_type_reference(
            program,
            crate::types::builtin_type_atom(primitive.landed_landing()?),
        )
    };
    Some([reference(0)?, reference(1)?])
}

pub fn proof_integer_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    declared: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    embedding: &dyn Fn(&TableCallExpression) -> bool,
) -> bool {
    fn visit(
        program: &TypedTrees,
        expression: ExpressionHandle,
        declared: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
        embedding: &dyn Fn(&TableCallExpression) -> bool,
        active: &mut Vec<ExpressionHandle>,
    ) -> bool {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return false;
        }
        active.push(expression);
        let answer = match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => embedding(call),
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                ) =>
            {
                let left = visit(program, binary.left, declared, embedding, active);
                let right = visit(program, binary.right, declared, embedding, active);
                (left || right)
                    && (left || anonymous_integer_value(program, binary.left).is_some())
                    && (right || anonymous_integer_value(program, binary.right).is_some())
            }
            _ => declared(expression).is_some_and(|mut reference| {
                for _ in 0..=program.type_reference_table.type_reference_count() {
                    if !program.contains_type_reference(reference) {
                        return false;
                    }
                    match program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                        TypeReferenceNode::Named { symbol, .. } => {
                            return symbol.is_valid()
                                && program
                                    .symbols
                                    .builtin_type_symbol(symbols::BuiltinType::Int)
                                    == Some(*symbol);
                        }
                        _ => return false,
                    }
                }
                false
            }),
        };
        active.pop();
        answer
    }
    visit(program, expression, declared, embedding, &mut Vec::new())
}
