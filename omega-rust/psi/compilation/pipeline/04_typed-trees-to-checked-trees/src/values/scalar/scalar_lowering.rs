//! Lowering one scalar expression: state and unit scalars, returns, operand
//! landing, integer binary, bitwise and cast construction, and literal
//! landing and retagging.
//!
//! Indexed observations use one ordered projection plan for members and
//! selectors. A canonical place describes geometry but cannot retain an
//! executable selector's occurrence; read plans therefore keep the full
//! scalar operand and its exact indexing handle, independently of root location.
//!
//! Subtraction selects its result from operand roles, not storage context:
//! address offsets return addresses, while address differences return counts.
//! Anonymous offsets land as u64; named operands retain their source carriers.
//! Later Terminal reconstruction independently checks those signatures and
//! proves the mathematical difference fits its result before native lowering.

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
    CheckedBooleanExpression, CheckedIntegerRange, CheckedOperatorFacts,
    CheckedOperatorResolutionStatus, CheckedScalarExpression,
    CheckedStructuralPredicatePathSegment,
};
use language_semantics::declaration_selection::CollectionMeasure;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{IntegerLanding, LandedIntegerType};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
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
            is_reference: false,
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
    let (parameters, locals) = unit_scalar_context(program, state, before_statement)?;
    let parameter_types = parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;

    lower_return_expression(
        program,
        operators,
        expression,
        &parameters,
        program.state_parameters(state),
        &parameter_types,
        &locals,
        expected_type,
        &operators.exact_integer_casts,
    )
}

/// Lower the ordinary scalar algebra, then bind each storage observation in
/// that exact state namespace. The binder supplies its own custody policy;
/// local reads stay reads of the local value rather than its initializer.
pub(crate) fn lower_unit_scalar_argument_with_places(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    expected_type: PrimitiveType,
    mut bind: impl FnMut(&crate::flow::CanonicalPlace, PrimitiveType) -> Option<CheckedScalarExpression>,
) -> Option<CheckedScalarExpression> {
    let mut value = lower_unit_scalar_argument(
        program,
        operators,
        state,
        before_statement,
        expression,
        expected_type,
    )?;
    let symbols = unit_scalar_binding_symbols(program, state, before_statement)?;
    crate::facts::boolean_predicates::map_scalar(&mut value, &mut |operand| {
        let primitive = operand.primitive_type()?;
        let place = match operand {
            CheckedScalarExpression::Parameter { position, .. }
            | CheckedScalarExpression::Local { position, .. } => {
                crate::flow::canonical_place_from_symbol(*symbols.get(*position)?)?
            }
            CheckedScalarExpression::StorageRead { symbol, .. } => {
                crate::flow::canonical_place_from_symbol(*symbol)?
            }
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                let (root, segments, _, _) = crate::values::resolve_structural_parameter_path(
                    program,
                    program.state_parameters(state),
                    *parameter_position,
                    path,
                )?;
                crate::flow::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                }
            }
            _ => return None,
        };
        let bound = bind(&place, primitive)?;
        if bound.primitive_type()? != primitive {
            return None;
        }
        *operand = bound;
        Some(())
    })?;
    Some(value)
}

fn unit_scalar_context(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    before_statement: usize,
) -> Option<(Vec<StateParameter>, Vec<ScalarLocal>)> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| crate::values::scalar::occupies_scalar_position(program, parameter))
        .cloned()
        .collect::<Vec<_>>();
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements.get(..before_statement)?;
    let mut locals = Vec::new();
    for statement in prefix {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        let Some(primitive_type) = program.primitive_type_reference(local.type_reference) else {
            if let Some(binding) = primitive_reference_read::local(program, local) {
                locals.push(binding);
            }
            continue;
        };
        if !local.initial_value.is_valid()
            && crate::values::zeroed_scalar_local_initializer(program, local).is_none()
        {
            return None;
        }
        locals.push(ScalarLocal {
            is_mutable: local.is_mutable,
            symbol: local.symbol,
            name: local.name.as_str().to_owned(),
            primitive_type,
            arithmetic_domain: program.arithmetic_domain_for_type_reference(local.type_reference),
            is_reference: false,
        });
    }
    Some((parameters, locals))
}

/// The exact binding namespace used by `lower_unit_scalar_argument`. Stored
/// local values retain their own symbols rather than replaying initializers.
pub(crate) fn unit_scalar_binding_symbols(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    before_statement: usize,
) -> Option<Vec<symbols::SymbolHandle>> {
    let (parameters, locals) = unit_scalar_context(program, state, before_statement)?;
    Some(
        parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(
                locals
                    .iter()
                    .filter(|local| local.is_value_binding())
                    .map(|local| local.symbol),
            )
            .collect(),
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
    // Boolean predicates retain their established form, but not every Boolean
    // value is a predicate tree. Storage observations (including runtime
    // element selection) use the common typed scalar read below.
    if result_type == PrimitiveType::Bool
        && let Some(expression) = lower_boolean_expression(
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )
    {
        return Some(CheckedScalarExpression::Boolean(Box::new(expression)));
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
        && let Some(value) = land_anonymous_scalar_expression(
            program,
            operators,
            binary.right,
            subtraction_count_context(binary.operator, destination),
        )
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
    // A recast operand `&place as &T` / `&mut place as &mut T` re-views its
    // source place under the stated target carrier (§5b address identity):
    // the checked form retains the source place's own read at its declared
    // carrier beneath the re-view (`view_place_read`). Validation already
    // proved the representation relation and the borrow walk attributes the
    // view to the source place, so only whole-place sources compose here --
    // a byte-region view over an indexed source keeps element-unit reads
    // that cannot carry the target width and stays unlowered (loud) rather
    // than misattributing bytes.
    if let Some((source, target_type)) = recast_view_operand(program, expression) {
        let primitive_type = program.primitive_type_reference(target_type)?;
        let (read, _) = lower_scalar_expression(
            program,
            operators,
            source,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        )?;
        return Some((
            view_place_read(read, primitive_type)?,
            program.arithmetic_domain_for_type_reference(target_type),
        ));
    }
    // A landed floating leaf has the same bits whether it is the whole return
    // or selected from a closed constructor. The caller still checks its exact
    // destination carrier; this does not choose meaning for float operations.
    if let ExpressionNode::Float(literal) = program.expression_table.expression(expression) {
        let value = match literal.landing()? {
            numerics::literals::FloatFormat::F32 => {
                semantic_vocabulary::IeeeFloatValue::Binary32(literal.f32_bits())
            }
            numerics::literals::FloatFormat::F64 => {
                semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
            }
        };
        return Some((
            CheckedScalarExpression::IeeeFloatLiteral { value },
            ArithmeticDomain::Exact,
        ));
    }
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
        return indexed_scalar_read(
            program,
            operators,
            expression,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            exact_integer_casts,
        );
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
    if let Some(read) = indexed_scalar_read(
        program,
        operators,
        expression,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    ) {
        return Some(read);
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
            if local.is_mutable || local.is_reference {
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
                    .filter(|local| local.is_value_binding())
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
            if matches!(target_type, PrimitiveType::F32 | PrimitiveType::F64)
                && cast.domain == ArithmeticDomain::Exact
            {
                // Arithmetic policy changes later operations, not stored bits.
                // Explicit same-format erasure therefore retains the ordinary
                // operand read. Format conversions still need their selected
                // boundary operation; never reinterpret a different carrier.
                let (operand, _) = lower_scalar_expression(
                    program,
                    operators,
                    cast.value,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )?;
                return (scalar_expression_type(&operand) == Some(target_type))
                    .then_some((operand, ArithmeticDomain::Exact));
            }
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
            construct_integer_binary(
                binary.operator,
                left,
                left_domain,
                right,
                right_domain,
                operators.formation_is_decided(expression),
            )
        }
        _ => None,
    }
}

/// An anonymous offset beside an address denotes a count, not another address.
/// Named operands keep their checked carrier, so address differences remain distinct.
pub(crate) fn subtraction_count_context(
    operator: BinaryOperator,
    peer: PrimitiveType,
) -> PrimitiveType {
    if operator == BinaryOperator::Subtract && peer == PrimitiveType::Addr {
        PrimitiveType::U64
    } else {
        peer
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
    formation_decided: bool,
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
        right = land_contextual_integer_literal(
            right,
            subtraction_count_context(operator, left_type?),
        )?;
        right_type = scalar_expression_type(&right);
    }
    let left_type = left_type?;
    let right_type = right_type?;
    let primitive_type = if operator == BinaryOperator::Subtract && left_type == PrimitiveType::Addr
    {
        if domain != ArithmeticDomain::Exact {
            return None;
        }
        match right_type {
            PrimitiveType::Addr => PrimitiveType::U64,
            PrimitiveType::U64 => PrimitiveType::Addr,
            _ => return None,
        }
    } else {
        if !shift && right_type != left_type {
            return None;
        }
        left_type
    };
    if !is_integer(primitive_type) || !is_integer(right_type) {
        return None;
    }
    Some((
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left: Box::new(left),
            right: Box::new(right),
            formation_decided,
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
                formation_decided: true,
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
    // A saturating conversion composes for every fixed-integer pair: the
    // clamp is spelled with total arithmetic on the source carrier and the
    // lowered stage routes the clamped value through the wrapping
    // conversion's mask-and-halves crossing, whose exact casts carry their
    // own bound evidence.
    if cast.domain == ArithmeticDomain::Saturating
        && is_integer(source_type)
        && source_type != PrimitiveType::Addr
        && target_type != PrimitiveType::Addr
    {
        return Some((
            CheckedScalarExpression::IntegerSaturatingCast {
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
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
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
    if !range.start.is_valid() || !range.end.is_valid() {
        return None;
    }
    let ExpressionNode::Integer(start) = program.expression_table.expression(range.start) else {
        return None;
    };
    let ExpressionNode::Integer(end) = program.expression_table.expression(range.end) else {
        return None;
    };
    let length = end
        .value_u64()?
        .checked_add(u64::from(range.end_inclusive))?
        .checked_sub(start.value_u64()?)?;
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

/// Peel the authored `&`/`&mut` spellings around one recast view operand,
/// returning the whole-place source and the cast's stated target type.
/// `&x as &T` parses as `Borrow { Cast }` -- the unary `&` wraps the postfix
/// chain with the authored access -- while parentheses can leave the borrow
/// around the cast's SOURCE (`&(x) as &T`). Sources are name, member and
/// indexed places; `view_place_read` admits an indexed source only as a whole
/// element or as the first byte of a `u8` region, and deeper view chains have
/// no scalar place read to re-view.
pub(crate) fn recast_view_operand(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
    let mut expression = expression;
    while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
        expression = borrow.target;
    }
    let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
        return None;
    };
    if !cast.form.is_recast() {
        return None;
    }
    let mut source = cast.value;
    while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(source) {
        source = borrow.target;
    }
    matches!(
        program.expression_table.expression(source),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    )
    .then_some((source, cast.target_type))
}

/// One whole scalar place's storage site, recovered from its checked read.
/// A recast view re-reads the same site through its target carrier.
enum PlaceReadSite {
    Storage(symbols::SymbolHandle),
    Parameter(usize),
    Local(usize),
    StructuralField(u32, Vec<CheckedStructuralPredicatePathSegment>),
}

/// Recover the storage site of one checked read when it is a whole scalar
/// place. Boolean place leaves unwrap to their site as well; any composed
/// value (operations, literals, indexed reads) is not a place.
fn place_read_site(read: &CheckedScalarExpression) -> Option<PlaceReadSite> {
    match read {
        CheckedScalarExpression::StorageRead { symbol, .. } => {
            Some(PlaceReadSite::Storage(*symbol))
        }
        CheckedScalarExpression::Parameter { position, .. } => {
            Some(PlaceReadSite::Parameter(*position))
        }
        CheckedScalarExpression::Local { position, .. } => Some(PlaceReadSite::Local(*position)),
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } => Some(PlaceReadSite::StructuralField(
            *parameter_position,
            path.clone(),
        )),
        CheckedScalarExpression::Boolean(inner) => match inner.as_ref() {
            CheckedBooleanExpression::StorageRead { symbol } => {
                Some(PlaceReadSite::Storage(*symbol))
            }
            CheckedBooleanExpression::Parameter { position } => {
                Some(PlaceReadSite::Parameter(*position))
            }
            CheckedBooleanExpression::Local { position } => Some(PlaceReadSite::Local(*position)),
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } => Some(PlaceReadSite::StructuralField(
                *parameter_position,
                path.clone(),
            )),
            _ => None,
        },
        _ => None,
    }
}

/// The checked read of one whole place re-viewed as `primitive_type`. A
/// same-carrier view changes facts, not representation, and is the place's
/// own read. A carrier change retains the read at its declared carrier inside
/// `RepresentationView`, so later stages replay the carrier relation instead
/// of meeting a field read whose carrier its declaration does not name. A
/// Boolean view keeps the retagged Boolean leaf its guard positions consume;
/// its native replay is still owed a Boolean re-view form.
///
/// A runtime-selected `u8` element is the first byte of a byte-region view
/// (`&self.buf[offset] as &u32`): the recast judgment proved the whole
/// footprint in bounds, and the retained view keeps that one authored element
/// read so stage 05 can replay the region byte by byte, each byte owing its
/// own index obligation. Any other width change is not a whole-place view and
/// stays unlowered.
fn view_place_read(
    read: CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let byte_region = matches!(
        &read,
        CheckedScalarExpression::StructuralParameterIndexedRead {
            element_path,
            primitive_type: PrimitiveType::U8,
            ..
        } if element_path.is_empty()
    );
    let indexed_element = matches!(
        read,
        CheckedScalarExpression::StructuralParameterIndexedRead { .. }
    );
    if let CheckedScalarExpression::RepresentationView {
        primitive_type: view_type,
        ..
    } = &read
    {
        // A recast-spelled place already lowers to its referee carrier's
        // view (`&b as &u8` mints the RepresentationView itself): naming the
        // same carrier keeps that read rather than meeting a raw place the
        // site walk cannot reach.
        if *view_type == primitive_type {
            return Some(read);
        }
    }
    if primitive_type == PrimitiveType::Bool
        && let CheckedScalarExpression::Boolean(_) = read
    {
        // A Boolean source already has the requested carrier.
        return Some(read);
    }
    if !indexed_element {
        place_read_site(&read)?;
    }
    let source = read.primitive_type()?;
    if source == primitive_type {
        return Some(read);
    }
    let region = byte_region && is_integer(primitive_type);
    if !region && scalar_byte_width(source)? != scalar_byte_width(primitive_type)? {
        return None;
    }
    Some(CheckedScalarExpression::RepresentationView {
        primitive_type,
        operand: Box::new(read),
    })
}

/// The storage width of one primitive scalar carrier, in bytes.
fn scalar_byte_width(primitive_type: PrimitiveType) -> Option<u32> {
    Some(match primitive_type {
        PrimitiveType::Bool | PrimitiveType::U8 | PrimitiveType::I8 => 1,
        PrimitiveType::U16 | PrimitiveType::I16 => 2,
        PrimitiveType::U32 | PrimitiveType::I32 | PrimitiveType::F32 => 4,
        PrimitiveType::U64 | PrimitiveType::I64 | PrimitiveType::F64 => 8,
        PrimitiveType::Addr => return None,
    })
}

/// Re-read one place site through the bool carrier -- the checked form of
/// `&place as &bool` in a Boolean position.
fn place_read_site_as_boolean(site: PlaceReadSite) -> CheckedBooleanExpression {
    match site {
        PlaceReadSite::Storage(symbol) => CheckedBooleanExpression::StorageRead { symbol },
        PlaceReadSite::Parameter(position) => CheckedBooleanExpression::Parameter { position },
        PlaceReadSite::Local(position) => CheckedBooleanExpression::Local { position },
        PlaceReadSite::StructuralField(parameter_position, path) => {
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            }
        }
    }
}

/// A Boolean place keeps its ordinary Boolean read. A different source
/// carrier remains a scalar representation view so its landing obligation
/// reaches Terminal Psi through the ordinary scalar computation path.
pub(crate) fn lower_recast_boolean_operand(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let (source, target_type) = recast_view_operand(program, expression)?;
    if program.primitive_type_reference(target_type) != Some(PrimitiveType::Bool) {
        return None;
    }
    let (read, _) = lower_scalar_expression(
        program,
        operators,
        source,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )?;
    if read.primitive_type()? != PrimitiveType::Bool {
        return None;
    }
    Some(place_read_site_as_boolean(place_read_site(&read)?))
}

/// One builtin element selection `collection[index]` over current structural
/// storage: the root and projection the collection names, the selected
/// element's type, whether the collection is a borrowed view, and the lowered
/// selector. A whole view local reads the view its establishment published,
/// the same element selection a whole view parameter takes.
pub(super) struct ElementSelection {
    pub(super) root: checked_trees::CheckedStorageRoot,
    pub(super) path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    pub(super) element_type: typed_trees::types::TypeReferenceHandle,
    pub(super) through_view: bool,
    pub(super) index: checked_trees::CheckedScalarSelector,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn selected_element(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<ElementSelection> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let (root, path, mut collection_type) = match structural_fields::structural_parameter_place(
        program,
        authored_parameters,
        indexed.collection,
    ) {
        Some((position, path, collection_type)) => (
            checked_trees::CheckedStorageRoot::Parameter { index: position },
            path,
            collection_type,
        ),
        None => {
            if let Some((symbol, path, collection_type)) = structural_fields::view_local_place(
                program,
                authored_parameters,
                indexed.collection,
            ) {
                (
                    checked_trees::CheckedStorageRoot::ViewLocal { symbol },
                    path,
                    collection_type,
                )
            } else {
                let (symbol, path, collection_type) = structural_fields::owned_local_place(
                    program,
                    authored_parameters,
                    indexed.collection,
                )?;
                (
                    checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
                    path,
                    collection_type,
                )
            }
        }
    };
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
    let (element_type, through_view) = loop {
        match program.type_reference_table.type_reference(collection_type) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => collection_type = *referee,
            TypeReferenceNode::FixedArray { element_type, .. } => break (*element_type, false),
            TypeReferenceNode::Slice { element_type } => break (*element_type, true),
            _ => return None,
        }
    };
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
    Some(ElementSelection {
        root,
        path,
        element_type,
        through_view,
        index: checked_trees::CheckedScalarSelector {
            expression: index,
            occurrence: expression,
        },
    })
}

/// Prepare one scalar indexed observation independently of the arrangement of
/// indexes and fields after its first collection selection. Every runtime
/// selector retains its full scalar value and exact indexing occurrence.
/// Storage roots and declared carriers, not source nesting families, determine
/// how the observation is emitted. Static literal geometry remains explicit.
#[allow(clippy::too_many_arguments)]
fn indexed_scalar_read(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    enum Projection {
        Field(symbols::SymbolHandle),
        Index {
            occurrence: ExpressionHandle,
            operand: ExpressionHandle,
        },
    }
    let mut projections = Vec::new();
    let mut cursor = expression;
    let first_selection = loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Indexed(indexed) => {
                if structural_fields::structural_parameter_place(
                    program,
                    authored_parameters,
                    indexed.collection,
                )
                .is_some()
                    || structural_fields::view_local_place(
                        program,
                        authored_parameters,
                        indexed.collection,
                    )
                    .is_some()
                    || structural_fields::owned_local_place(
                        program,
                        authored_parameters,
                        indexed.collection,
                    )
                    .is_some()
                {
                    break cursor;
                }
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    return None;
                }
                projections.push(Projection::Index {
                    occurrence: cursor,
                    operand: indexed.index,
                });
                cursor = indexed.collection;
            }
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() || !member.member_symbol.is_valid() {
                    return None;
                }
                projections.push(Projection::Field(member.member_symbol));
                cursor = member.receiver;
            }
            _ => return None,
        }
    };
    let selection = selected_element(
        program,
        operators,
        first_selection,
        parameters,
        authored_parameters,
        parameter_types,
        locals,
        exact_integer_casts,
    )?;
    let mut element_type = selection.element_type;
    let mut element_path = Vec::with_capacity(projections.len());
    for projection in projections.into_iter().rev() {
        match projection {
            Projection::Field(symbol) => {
                let (fields, leaf) = structural_fields::element_field_path(
                    program,
                    element_type,
                    &[facts::PlaceSegment::Field { symbol }],
                )?;
                element_path.extend(
                    fields
                        .into_iter()
                        .map(checked_trees::CheckedScalarReadProjection::Static),
                );
                element_type = leaf;
            }
            Projection::Index {
                occurrence,
                operand,
            } => {
                if !structural_fields::indexed_read_is_builtin(
                    program,
                    operators,
                    authored_parameters,
                    occurrence,
                    element_type,
                    operand,
                ) {
                    return None;
                }
                if let Some(literal) = program
                    .expression_table
                    .constant_integer_value(operand)
                    .and_then(|literal| u64::try_from(literal).ok())
                {
                    element_type = structural_fields::fixed_index_element_type(
                        program,
                        element_type,
                        literal,
                    )?;
                    element_path.push(checked_trees::CheckedScalarReadProjection::Static(
                        CheckedStructuralPredicatePathSegment::FixedIndex(literal),
                    ));
                } else {
                    let selector = land_anonymous_scalar_expression(
                        program,
                        operators,
                        operand,
                        PrimitiveType::U64,
                    )
                    .or_else(|| {
                        lower_scalar_expression(
                            program,
                            operators,
                            operand,
                            parameters,
                            authored_parameters,
                            parameter_types,
                            locals,
                            exact_integer_casts,
                        )
                        .map(|(selector, _)| selector)
                    })?;
                    let carrier = scalar_expression_type(&selector)?;
                    if !is_integer(carrier) || carrier == PrimitiveType::Addr {
                        return None;
                    }
                    element_type =
                        structural_fields::runtime_index_element_type(program, element_type)?;
                    element_path.push(checked_trees::CheckedScalarReadProjection::RuntimeIndex(
                        checked_trees::CheckedScalarSelector {
                            expression: selector,
                            occurrence,
                        },
                    ));
                }
            }
        }
    }
    let primitive_type = program.primitive_type_reference(element_type)?;
    if !is_integer(primitive_type)
        && !matches!(
            primitive_type,
            PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
        )
        || primitive_type == PrimitiveType::Addr
    {
        return None;
    }
    Some((
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root: selection.root,
            path: selection.path,
            index: Box::new(selection.index),
            element_path,
            primitive_type,
        },
        program.arithmetic_domain_for_type_reference(element_type),
    ))
}
