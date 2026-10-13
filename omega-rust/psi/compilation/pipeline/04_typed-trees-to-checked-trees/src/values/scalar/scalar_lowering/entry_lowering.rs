//! entry lowering slice of scalar_lowering.

use super::{element_selection::*, integers::*, recast_reads::*};
use crate::values::scalar::boolean_lowering::lower_boolean_expression;
use crate::values::scalar::constant_array_projection;
use crate::values::scalar::expression_facts::{
    is_integer, local_position, operator_is_builtin, parameter_position, scalar_expression_type,
};
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::primitive_reference_read;
use crate::values::scalar::structural_fields;
use checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedScalarExpression,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// Lower one call argument in the caller state's checked scalar namespace.
/// Only the immutable scalar-prefix shape accepted by terminal scalar lowering
/// is represented; any wider state shape stays explicit as `None` so crash
/// refinement cannot claim a portable predicate it cannot later materialize.
pub(crate) fn lower_state_scalar_expression(
    program: &dyn ::validation::CheckingInput,
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
        let local = statement.as_local_data()?;
        if local.is_mutable
            || (!local.initial_value.is_valid()
                && crate::values::zeroed_scalar_local_initializer(program, local).is_none())
        {
            return None;
        }
        let primitive_type = program.primitive_type_reference(local.type_reference)?;
        locals.push(ScalarLocal::from_local_data(program, local, primitive_type));
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
    program: &dyn ::validation::CheckingInput,
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
pub(crate) fn lower_unit_scalar_argument_for_places(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    expected_type: PrimitiveType,
    mut bind: impl FnMut(&validation::CanonicalPlace, PrimitiveType) -> Option<CheckedScalarExpression>,
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
                validation::CanonicalPlace {
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

pub(crate) fn unit_scalar_context(
    program: &dyn ::validation::CheckingInput,
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
        let Some(local) = statement.as_local_data() else {
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
        locals.push(ScalarLocal::from_local_data(program, local, primitive_type));
    }
    Some((parameters, locals))
}

/// The exact binding namespace used by `lower_unit_scalar_argument`. Stored
/// local values retain their own symbols rather than replaying initializers.
pub(crate) fn unit_scalar_binding_symbols(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    destination: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    validation::land_anonymous_integer_expression(
        program,
        expression,
        destination,
        &[],
        |expression| match operators.expression_use(expression) {
            Some(operator) => operator.status == CheckedOperatorResolutionStatus::BuiltinFallback,
            None => validation::has_anonymous_operator_meaning(program, expression),
        },
    )
    .map(|(literal, _)| CheckedScalarExpression::IntegerLiteral { literal })
}

pub(crate) fn lower_scalar_operands(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    // view to the source place. An element source is a selection whether its
    // index is a literal or a runtime value: the view starts at that element,
    // and a wider view covers the byte region the recast judgment proved in
    // bounds. Folding a literal index into a field path would leave a single
    // element leaf that cannot carry the target width.
    if let Some((source, target_type)) = recast_view_operand(program, expression) {
        let primitive_type = program.primitive_type_reference(target_type)?;
        let element = program.expression_table.indexed_node(source).and_then(|_| {
            indexed_scalar_read(
                program,
                operators,
                source,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )
        });
        let (read, _) = match element {
            Some(element) => element,
            None => lower_scalar_expression(
                program,
                operators,
                source,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?,
        };
        return Some((
            view_place_read(read, primitive_type)?,
            program.arithmetic_domain_for_type_reference(target_type),
        ));
    }
    // A landed floating leaf has the same bits whether it is the whole return
    // or selected from a closed constructor. The caller still checks its exact
    // destination carrier; this does not choose meaning for float operations.
    if let ExpressionNode::Float(literal) = program.expression_table.expression(expression) {
        let value = typed_trees::expression::ieee_float_value(literal, literal.landing()?);
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
            let literal = program.expression_table.integer_node(leaf)?;
            CheckedScalarExpression::IntegerLiteral {
                literal: if literal.landing().is_some() {
                    literal.clone()
                } else {
                    validation::land_anonymous_integer_expression(
                        program,
                        leaf,
                        primitive,
                        &[],
                        |_| false,
                    )?
                    .0
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
    if let Some(length) = structural_fields::structural_sequence_length(
        program,
        operators,
        authored_parameters,
        expression,
        |_, operand| {
            super::element_selection::pure_selector(
                program,
                operators,
                operand,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )
        },
    ) {
        return Some((length, ArithmeticDomain::Exact));
    }
    if let Some(length) = exact_inline_literal_subslice_length(program, expression) {
        return Some((length, ArithmeticDomain::Exact));
    }
    if let Some(indexed) = program.expression_table.indexed_node(expression) {
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
            // `Implementation as dyn Trait<Args>`: checking admitted the exact
            // named implementation; the selector names its table.
            if target_type == PrimitiveType::Selector {
                let Some(name) = program.expression_table.name_path_node(cast.value) else {
                    return None;
                };
                return Some((
                    CheckedScalarExpression::ImplementationSelection {
                        implementation: name.symbol,
                        interface_identity: program.normalized_type_identity(cast.target_type),
                    },
                    ArithmeticDomain::Exact,
                ));
            }
            if matches!(target_type, PrimitiveType::F32 | PrimitiveType::F64) {
                // Every numeric qualification cast preserves stored denotation.
                // Retain the same-format payload with its authored policy so
                // pure argument custody and future selected operations share
                // the ordinary scalar route. Format conversion still needs
                // its selected boundary operation.
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
                    .then_some((operand, cast.domain));
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
