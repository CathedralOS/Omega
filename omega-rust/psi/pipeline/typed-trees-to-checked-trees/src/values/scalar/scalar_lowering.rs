//! Lowering one scalar expression: state and unit scalars, returns, operand
//! landing, integer binary, bitwise and cast construction, and literal
//! landing and retagging.

use crate::values::scalar::boolean_lowering::lower_boolean_expression;
use crate::values::scalar::constant_array_projection;
use crate::values::scalar::expression_facts::{
    checked_integer_binary_kind, combine_arithmetic_domains, is_integer, local_position,
    operator_is_builtin, parameter_position, scalar_expression_type,
};
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::primitive_reference_read;
use crate::values::scalar::structural_fields;
use checked_trees::{
    CheckedIntegerRange, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedScalarExpression,
};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{IntegerLanding, LandedIntegerType};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};
pub(crate) use validation::integer_widen_is_total;

/// Lower one call argument in the caller state's checked scalar namespace.
/// Only the immutable scalar-prefix shape accepted by terminal scalar lowering
/// is represented; any wider state shape stays explicit as `None` so crash
/// refinement cannot claim a portable predicate it cannot later materialize.
pub(crate) fn lower_state_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    expected_type: PrimitiveType,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
    let parameters = program.state_parameters(state);
    let parameter_types = parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements.get(..before_statement)?;
    let mut locals = Vec::new();
    for statement in prefix {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        if local.is_mutable || !local.initial_value.is_valid() {
            return None;
        }
        let primitive_type = program.primitive_type_reference(local.type_reference)?;
        locals.push(ScalarLocal {
            is_mutable: false,
            symbol: local.symbol,
            name: local.name.as_str().to_owned(),
            primitive_type,
            arithmetic_domain: program.arithmetic_domain_for_type_reference(local.type_reference),
        });
    }
    lower_return_expression(
        program,
        operators,
        expression,
        parameters,
        program.state_parameters(state),
        &parameter_types,
        &locals,
        expected_type,
        exact_integer_casts,
    )
}

/// Lower one scalar argument inside a structural/Unit state. Structural
/// parameters retain their separate custody namespace; only primitive
/// parameters and earlier immutable primitive locals occupy scalar positions.
pub(crate) fn lower_unit_scalar_argument(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    expected_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| crate::values::scalar::occupies_scalar_position(program, parameter))
        .cloned()
        .collect::<Vec<_>>();
    let parameter_types = parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements.get(..before_statement)?;
    let mut locals = Vec::new();
    for statement in prefix {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        let Some(primitive_type) = program.primitive_type_reference(local.type_reference) else {
            continue;
        };
        if !local.initial_value.is_valid() {
            return None;
        }
        locals.push(ScalarLocal {
            is_mutable: local.is_mutable,
            symbol: local.symbol,
            name: local.name.as_str().to_owned(),
            primitive_type,
            arithmetic_domain: program.arithmetic_domain_for_type_reference(local.type_reference),
        });
    }
    lower_return_expression(
        program,
        operators,
        expression,
        &parameters,
        program.state_parameters(state),
        &parameter_types,
        &locals,
        expected_type,
        &[],
    )
}

/// Indexing consumes an integer value, not an expected u64 expression. Keep
/// typed arithmetic in its authored carrier; only wholly anonymous arithmetic
/// lands directly in the general count carrier. Terminal emission performs the
/// exact coordinate conversion after evaluating this expression.
pub(crate) fn lower_index_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
    if let Some(value) =
        land_anonymous_scalar_expression(program, operators, expression, PrimitiveType::U64)
    {
        return Some(value);
    }
    let (expression, _) = lower_scalar_expression(
        program,
        operators,
        expression,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )?;
    scalar_expression_type(&expression)
        .is_some_and(|primitive| primitive != PrimitiveType::Addr && is_integer(primitive))
        .then_some(expression)
}

pub(crate) fn lower_return_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    result_type: PrimitiveType,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedScalarExpression> {
    if let Some(value) =
        land_anonymous_scalar_expression(program, operators, expression, result_type)
    {
        return Some(value);
    }
    if result_type == PrimitiveType::Bool {
        return lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )
        .map(|expression| CheckedScalarExpression::Boolean(Box::new(expression)));
    }
    if let ExpressionNode::Float(literal) = program.expression_table.expression(expression) {
        let value = match (result_type, literal.landing()) {
            (PrimitiveType::F32, Some(numerics::literals::FloatFormat::F32)) => {
                semantic_vocabulary::IeeeFloatValue::Binary32(literal.f32_bits())
            }
            (PrimitiveType::F64, Some(numerics::literals::FloatFormat::F64)) => {
                semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
            }
            _ => return None,
        };
        return Some(CheckedScalarExpression::IeeeFloatLiteral { value });
    }
    let (expression, _) = lower_scalar_expression(
        program,
        operators,
        expression,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )?;
    match scalar_expression_type(&expression) {
        Some(actual_type) => (actual_type == result_type).then_some(expression),
        None => land_contextual_integer_literal(expression, result_type),
    }
}

pub(crate) fn land_anonymous_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    destination: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    validation::land_anonymous_integer_expression(program, expression, destination, |expression| {
        match operators.expression_use(expression) {
            Some(operator) => operator.status == CheckedOperatorResolutionStatus::BuiltinFallback,
            None => validation::has_anonymous_operator_meaning(program, expression),
        }
    })
    .map(|literal| CheckedScalarExpression::IntegerLiteral { literal })
}

pub(crate) fn lower_scalar_operands(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    binary: &typed_trees::expression::TableBinaryExpression,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(
    (CheckedScalarExpression, ArithmeticDomain),
    (CheckedScalarExpression, ArithmeticDomain),
)> {
    let lower = |expression| {
        lower_scalar_expression(
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )
    };
    let mut left = lower(binary.left);
    let mut right = lower(binary.right);
    // Only the actual peer operand supplies a carrier. A wholly anonymous
    // subtree is evaluated exactly before landing; typed operations and calls
    // are rejected by this query and retain their original semantics.
    if let Some(destination) = right
        .as_ref()
        .and_then(|(value, _)| scalar_expression_type(value))
        && let Some(value) =
            land_anonymous_scalar_expression(program, operators, binary.left, destination)
    {
        left = Some((value, ArithmeticDomain::Exact));
    }
    if let Some(destination) = left
        .as_ref()
        .and_then(|(value, _)| scalar_expression_type(value))
        && let Some(value) =
            land_anonymous_scalar_expression(program, operators, binary.right, destination)
    {
        right = Some((value, ArithmeticDomain::Exact));
    }
    Some((left?, right?))
}

pub(crate) fn lower_scalar_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    if let Some(read) = primitive_reference_read::declared(program, authored_parameters, expression)
    {
        return Some(read);
    }
    if let Some((leaf, primitive)) =
        validation::closed_record_scalar_projection(program, expression)
    {
        let lowered = if is_integer(primitive) {
            let ExpressionNode::Integer(literal) = program.expression_table.expression(leaf) else {
                return None;
            };
            CheckedScalarExpression::IntegerLiteral {
                literal: if literal.landing().is_some() {
                    literal.clone()
                } else {
                    validation::land_anonymous_integer_expression(program, leaf, primitive, |_| {
                        false
                    })?
                },
            }
        } else {
            lower_scalar_expression(
                program,
                operators,
                leaf,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?
            .0
        };
        return (lowered.primitive_type() == Some(primitive))
            .then_some((lowered, ArithmeticDomain::Exact));
    }
    if let Some(leaf) = constant_array_projection::selected_leaf(
        program,
        operators,
        authored_parameters,
        expression,
    ) {
        return lower_scalar_expression(
            program,
            operators,
            leaf,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        );
    }
    if let Some(length) =
        structural_fields::structural_sequence_length(program, authored_parameters, expression)
    {
        return Some((length, ArithmeticDomain::Exact));
    }
    if let Some(length) = exact_inline_literal_subslice_length(program, expression) {
        return Some((length, ArithmeticDomain::Exact));
    }
    if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) {
        // Literal fixed-array leaves use the same checked projection as record
        // fields. Runtime byte indexing below keeps its separate bound proof.
        if let Some(field) = structural_fields::lower_structural_parameter_field(
            program,
            authored_parameters,
            expression,
        ) {
            let (_, _, collection_type) = structural_fields::structural_parameter_place(
                program,
                authored_parameters,
                indexed.collection,
            )?;
            return structural_fields::indexed_read_is_builtin(
                program,
                operators,
                authored_parameters,
                expression,
                collection_type,
                indexed.index,
            )
            .then_some(field);
        }
        let (parameter_position, path, mut collection_type) =
            structural_fields::structural_parameter_place(
                program,
                authored_parameters,
                indexed.collection,
            )?;
        if !structural_fields::indexed_read_is_builtin(
            program,
            operators,
            authored_parameters,
            expression,
            collection_type,
            indexed.index,
        ) {
            return None;
        }
        let element_type = loop {
            match program.type_reference_table.type_reference(collection_type) {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => collection_type = *referee,
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => break *element_type,
                _ => return None,
            }
        };
        let primitive_type = program.primitive_type_reference(element_type)?;
        if primitive_type != PrimitiveType::U8 {
            return None;
        }
        let index =
            land_anonymous_scalar_expression(program, operators, indexed.index, PrimitiveType::U64)
                .or_else(|| {
                    lower_scalar_expression(
                        program,
                        operators,
                        indexed.index,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                    )
                    .map(|(index, _)| index)
                })?;
        let index_type = scalar_expression_type(&index)?;
        if !is_integer(index_type) || index_type == PrimitiveType::Addr {
            return None;
        }
        return Some((
            CheckedScalarExpression::StructuralParameterIndexedRead {
                parameter_position,
                path,
                index: Box::new(index),
                primitive_type,
            },
            program.arithmetic_domain_for_type_reference(element_type),
        ));
    }
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(_) | ExpressionNode::Member(_)
    ) && let Some(field) = structural_fields::lower_structural_parameter_field(
        program,
        authored_parameters,
        expression,
    ) {
        return Some(field);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some(position) = parameter_position(program, path, parameters) {
                let parameter = &parameters[position];
                if parameter.is_mutable {
                    let primitive_type =
                        crate::values::mutable_scalar_parameter_type(program, parameter)?;
                    if path.symbol != parameter.symbol || path.head_symbol != parameter.symbol {
                        return None;
                    }
                    return Some((
                        CheckedScalarExpression::StorageRead {
                            symbol: parameter.symbol,
                            primitive_type,
                        },
                        program.arithmetic_domain_for_type_reference(parameter.type_reference),
                    ));
                }
                return Some((
                    CheckedScalarExpression::Parameter {
                        position,
                        primitive_type: parameter_types[position],
                    },
                    program
                        .arithmetic_domain_for_type_reference(parameters[position].type_reference),
                ));
            }
            // An erased formal has no runtime scalar position; it still names
            // a proof-only operand in the machine's erased roster so callers
            // can forward it as an erased actual.
            if let Some(authored) = parameter_position(program, path, authored_parameters) {
                let parameter = &authored_parameters[authored];
                if parameter.relevance.is_erased()
                    && let Some(primitive_type) =
                        program.primitive_type_reference(parameter.type_reference)
                {
                    let position = authored_parameters[..authored]
                        .iter()
                        .filter(|parameter| {
                            parameter.relevance.is_erased()
                                && program
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_some()
                        })
                        .count();
                    return Some((
                        CheckedScalarExpression::ErasedParameter {
                            position,
                            primitive_type,
                        },
                        program.arithmetic_domain_for_type_reference(parameter.type_reference),
                    ));
                }
            }
            let local_position = local_position(program, expression, path, locals)?;
            let local = &locals[local_position];
            if local.is_mutable {
                return Some((
                    CheckedScalarExpression::StorageRead {
                        symbol: local.symbol,
                        primitive_type: local.primitive_type,
                    },
                    local.arithmetic_domain,
                ));
            }
            let position = parameters.len().checked_add(
                locals[..local_position]
                    .iter()
                    .filter(|local| !local.is_mutable)
                    .count(),
            )?;
            Some((
                CheckedScalarExpression::Local {
                    position,
                    primitive_type: locals[local_position].primitive_type,
                },
                locals[local_position].arithmetic_domain,
            ))
        }
        ExpressionNode::Integer(literal) => Some((
            CheckedScalarExpression::IntegerLiteral {
                literal: literal.clone(),
            },
            literal
                .landing()
                .map(|landing| landing.domain)
                .unwrap_or(ArithmeticDomain::Exact),
        )),
        ExpressionNode::Cast(cast) if !cast.form.is_recast() && cast.semantic_domain.is_empty() => {
            let target_type = program.primitive_type_reference(cast.target_type)?;
            if !is_integer(target_type) {
                return None;
            }
            let operand =
                land_anonymous_scalar_expression(program, operators, cast.value, target_type)
                    .or_else(|| {
                        lower_scalar_expression(
                            program,
                            operators,
                            cast.value,
                            parameters,
                            authored_parameters,
                            parameter_types,
                            locals,
                            exact_integer_casts,
                        )
                        .map(|(value, _)| value)
                    })?;
            construct_integer_cast(program, expression, operand, exact_integer_casts)
        }
        ExpressionNode::Unary(unary)
            if unary.operator == UnaryOperator::BitwiseNot
                && operator_is_builtin(operators, expression) =>
        {
            let (operand, domain) = lower_scalar_expression(
                program,
                operators,
                unary.operand,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            construct_integer_bitwise_not(operand, domain)
        }
        ExpressionNode::Binary(binary) if operator_is_builtin(operators, expression) => {
            let ((left, left_domain), (right, right_domain)) = lower_scalar_operands(
                program,
                operators,
                binary,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            construct_integer_binary(binary.operator, left, left_domain, right, right_domain)
        }
        _ => None,
    }
}

/// Construct one selected builtin operation from operands whose evaluation
/// order has already been retained by either the pure tree or computation plan.
pub(crate) fn construct_integer_binary(
    operator: BinaryOperator,
    mut left: CheckedScalarExpression,
    left_domain: ArithmeticDomain,
    mut right: CheckedScalarExpression,
    right_domain: ArithmeticDomain,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let shift = matches!(
        operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    let domain = if shift {
        left_domain
    } else {
        combine_arithmetic_domains(left_domain, right_domain)?
    };
    let kind = checked_integer_binary_kind(operator, domain)?;
    let mut left_type = scalar_expression_type(&left);
    let mut right_type = scalar_expression_type(&right);
    // Unary `-value` is parsed as a compiler-generated anonymous
    // `0 - value`. That zero has no parse-site suffix from which the
    // ordinary literal stamper can learn a carrier, so retain the
    // binary expression's already-checked operand carrier here. This
    // is contextual literal landing, not a new negation meaning.
    if operator == BinaryOperator::Subtract && left_type.is_none() {
        left = land_anonymous_zero(left, right_type?)?;
        left_type = scalar_expression_type(&left);
    }
    if left_type.is_none() {
        left = land_contextual_integer_literal(left, right_type?)?;
        left_type = scalar_expression_type(&left);
    }
    if right_type.is_none() {
        right = land_contextual_integer_literal(right, left_type?)?;
        right_type = scalar_expression_type(&right);
    }
    let primitive_type = left_type?;
    let right_type = right_type?;
    if !is_integer(primitive_type)
        || !is_integer(right_type)
        || (!shift && right_type != primitive_type)
    {
        return None;
    }
    Some((
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left: Box::new(left),
            right: Box::new(right),
        },
        domain,
    ))
}

pub(crate) fn construct_integer_bitwise_not(
    operand: CheckedScalarExpression,
    domain: ArithmeticDomain,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let primitive_type = scalar_expression_type(&operand)?;
    is_integer(primitive_type).then_some((
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand: Box::new(operand),
        },
        domain,
    ))
}

/// Retain cast meaning and any partial-conversion proof at the original source
/// occurrence even when the operand is a completed computation-plan value.
pub(crate) fn construct_integer_cast(
    program: &TypedTrees,
    expression: ExpressionHandle,
    operand: CheckedScalarExpression,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
        return None;
    };
    if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
        return None;
    }
    let target_type = program.primitive_type_reference(cast.target_type)?;
    if !is_integer(target_type) {
        return None;
    }
    let source_type = scalar_expression_type(&operand);
    if source_type.is_none()
        && cast.domain == ArithmeticDomain::Exact
        && target_type != PrimitiveType::Addr
        && let Some(literal) = retag_exact_integer_literal(&operand, target_type)
    {
        return Some((literal, cast.domain));
    }
    let source_type = source_type?;
    if source_type == target_type {
        return Some((operand, cast.domain));
    }
    // A compile-known exact conversion does not need a runtime cast
    // operation or a carried flow assumption: validation has already
    // proved the spelling denotes a target value, and the checked
    // carrier can retain that value directly at its new landing. Keep
    // address conversions out of this fixed-integer slice; addr is a
    // distinct carrier even when its current representation is u64.
    if cast.domain == ArithmeticDomain::Exact
        && source_type != PrimitiveType::Addr
        && target_type != PrimitiveType::Addr
        && let Some(literal) = retag_exact_integer_literal(&operand, target_type)
    {
        return Some((literal, cast.domain));
    }
    // A full-carrier inclusion needs no occurrence proof. Preserve it
    // as widening even when validation also retained a bounded range
    // for this spelling; exact-cast obligations are only necessary for
    // partial fixed-integer conversions.
    if integer_widen_is_total(source_type, target_type) {
        return Some((
            CheckedScalarExpression::IntegerWiden {
                primitive_type: target_type,
                operand: Box::new(operand),
            },
            cast.domain,
        ));
    }
    if cast.domain == ArithmeticDomain::Exact
        && let Some(fact) = exact_integer_casts
            .iter()
            .find(|fact| fact.expression == expression)
        && fact.source_type == source_type
        && fact.target_type == target_type
    {
        return Some((
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: target_type,
                operand: Box::new(operand),
                range: CheckedIntegerRange {
                    minimum: fact.minimum.clone(),
                    maximum: fact.maximum.clone(),
                },
            },
            cast.domain,
        ));
    }
    if cast.domain == ArithmeticDomain::Wrapping
        && is_integer(source_type)
        && source_type != PrimitiveType::Addr
        && target_type != PrimitiveType::Addr
    {
        return Some((
            CheckedScalarExpression::IntegerWrappingCast {
                primitive_type: target_type,
                operand: Box::new(operand),
            },
            cast.domain,
        ));
    }
    if cast.domain == ArithmeticDomain::Trapping
        && is_integer(source_type)
        && source_type != PrimitiveType::Addr
        && target_type != PrimitiveType::Addr
    {
        return Some((
            CheckedScalarExpression::IntegerTrappingCast {
                primitive_type: target_type,
                operand: Box::new(operand),
            },
            cast.domain,
        ));
    }
    // All remaining cast shapes fail closed at this source-independent
    // boundary: no total conversion and no retained occurrence proof.
    None
}

/// Reclose the one source-level slice view whose length is already fixed by
/// two literal bounds. This is a value fact, not a backend fold: retaining the
/// landed `u64` here lets later checked control replay the exact initializer
/// without consulting a target representation.
fn exact_inline_literal_subslice_length(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<CheckedScalarExpression> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.member.as_str() != "len"
        || member.member_symbol.is_valid()
        || member.case_variant.is_some()
    {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(member.receiver)
    else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    if range.end_inclusive || !range.start.is_valid() || !range.end.is_valid() {
        return None;
    }
    let ExpressionNode::Integer(start) = program.expression_table.expression(range.start) else {
        return None;
    };
    let ExpressionNode::Integer(end) = program.expression_table.expression(range.end) else {
        return None;
    };
    let length = end.value_u64()?.checked_sub(start.value_u64()?)?;
    let length = i64::try_from(length).ok()?;
    Some(CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(length).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::U64,
                domain: ArithmeticDomain::Exact,
            },
        ),
    })
}

fn land_anonymous_zero(
    expression: CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    if literal.landing().is_some() || literal.value_i64() != Some(0) {
        return None;
    }
    let landed_type = match primitive_type {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        PrimitiveType::Addr => LandedIntegerType::Addr,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    Some(CheckedScalarExpression::IntegerLiteral {
        literal: literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    })
}

pub(crate) fn retag_exact_integer_literal(
    expression: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    let landed_type = landed_for_primitive(primitive_type)?;
    let fits = if landed_type.is_signed() {
        let value = literal.value_i64()?;
        let bits = landed_type.bit_width();
        let minimum = -(1_i128 << (bits - 1));
        let maximum = (1_i128 << (bits - 1)) - 1;
        let value = i128::from(value);
        minimum <= value && value <= maximum
    } else {
        let value = literal.value_u64()?;
        let bits = landed_type.bit_width();
        let maximum = if bits == 64 {
            u64::MAX
        } else {
            (1_u64 << bits) - 1
        };
        value <= maximum
    };
    fits.then(|| CheckedScalarExpression::IntegerLiteral {
        literal: literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    })
}

pub(crate) fn land_contextual_integer_literal(
    expression: CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    if literal.landing().is_some() {
        return None;
    }
    let landed_type = match primitive_type {
        PrimitiveType::Addr => LandedIntegerType::Addr,
        primitive_type => landed_for_primitive(primitive_type)?,
    };
    let fits = if landed_type.is_signed() {
        let value = i128::from(literal.value_i64()?);
        let bits = landed_type.bit_width();
        let minimum = -(1_i128 << (bits - 1));
        let maximum = (1_i128 << (bits - 1)) - 1;
        minimum <= value && value <= maximum
    } else {
        let value = literal.value_u64()?;
        let bits = landed_type.bit_width();
        let maximum = if bits == 64 {
            u64::MAX
        } else {
            (1_u64 << bits) - 1
        };
        value <= maximum
    };
    fits.then(|| CheckedScalarExpression::IntegerLiteral {
        literal: literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    })
}

pub(crate) fn landed_for_primitive(primitive_type: PrimitiveType) -> Option<LandedIntegerType> {
    Some(match primitive_type {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        PrimitiveType::Addr | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return None;
        }
    })
}
