use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

type FloatLandingPair = (ExpressionHandle, TypeReferenceHandle);

mod call_destinations;
mod numeric_operands;
pub(crate) use numeric_operands::validate_anonymous_divisions;
pub(super) use numeric_operands::validate_numeric_tree_destination;

/// F2b -- float DESTINATION stamping (ch5 two-phase constants, the float
/// half): an UNSUFFIXED float literal initializing a declared `f32`/`f64`
/// place lands that format ON ITS VALUE. A wholly anonymous literal-arithmetic
/// tree is first evaluated as an exact rational and then replaced by one
/// landed literal, so every downstream reader -- native store, guard compare,
/// argument materialization, AND the interpreter -- consumes the same single
/// rounding. A nonconstant tree lands each complete anonymous operand subtree
/// once; its typed operations retain their own format semantics.
/// Authored operators retain their nodes and choose operand formats from their
/// signatures; the result destination is not authority to fold or stamp them.
/// Without this landing pass an anonymous literal at an f32 place takes the
/// transitional f64-then-narrow route (double rounding; the
/// 8388609.499999999999999 witness lands on the wrong side of the tie).
///
/// Declared storage, call arguments, and returned values supply destinations.
/// Suffix agreement is checked at storage/return sites and by the resolved
/// call validators. Already-landed (suffixed) literals are untouched: their landing
/// was chosen at the spelling, and the suffix-vs-destination check owns any
/// disagreement. Comparisons against a typed result, including a call or
/// computed operand, likewise land the opposite literal tree to that value's
/// format in every statement position. Declaration-owned named binders retain
/// their own scope. A callable contract's `result` root uses the exact return
/// type of its owning state, operator, or trait requirement. Runs on the
/// still-mutable typed tree BEFORE validation, so every downstream consumer
/// sees one stamped tree.
pub fn land_float_literal_destinations(program: &mut TypedTrees) {
    use numerics::literals::FloatFormat;

    let mut pairs: Vec<FloatLandingPair> = Vec::new();
    let mut direct_formats: Vec<(ExpressionHandle, FloatFormat)> = Vec::new();
    let mut anonymous_comparisons: Vec<ExpressionHandle> = Vec::new();

    for (handle, node) in program.expression_table.expression_entries() {
        match node {
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    let Some(field_type) =
                        crate::value_custody::struct_literals::selected_construction_field_type(
                            program,
                            literal,
                            field.field_symbol,
                        )
                    else {
                        continue;
                    };
                    pairs.push((field.value, field_type));
                }
            }
            ExpressionNode::Cast(cast) => {
                let format =
                    program
                        .primitive_type_reference(cast.target_type)
                        .and_then(|primitive| match primitive {
                            PrimitiveType::F32 => Some(FloatFormat::F32),
                            PrimitiveType::F64 => Some(FloatFormat::F64),
                            _ => None,
                        });
                if let Some(format) = format {
                    direct_formats.push((cast.value, format));
                }
            }
            ExpressionNode::Call(call) => {
                let arguments = program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied();
                if let Some(callee) = program.state_by_symbol(call.target_symbol) {
                    let parameters = program.state_parameters(callee).iter();
                    for (parameter, argument) in parameters.zip(arguments) {
                        if parameter.type_reference.is_valid() {
                            pairs.push((argument, parameter.type_reference));
                        }
                    }
                    continue;
                }
                // A legacy named operator (`F64::minimum(value, 2.5)`, or a
                // bound `min`/`clamp` shorthand) is a requirement, not a
                // stored state: its formals are the destination of each
                // literal operand. A receiver-spelled call supplies the first
                // formal through its receiver, exactly as its operand roster
                // does.
                let Some(operator) = program
                    .operators()
                    .iter()
                    .find(|operator| operator.symbol == call.target_symbol)
                else {
                    continue;
                };
                let parameters = program.operator_parameters(operator);
                let receiver_formal = usize::from(
                    call.receiver.is_valid() && (parameters.len() == arguments.len() + 1),
                );
                for (parameter, argument) in parameters.iter().skip(receiver_formal).zip(arguments)
                {
                    if parameter.type_reference.is_valid() {
                        pairs.push((argument, parameter.type_reference));
                    }
                }
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    typed_trees::expression::BinaryOperator::Equal
                        | typed_trees::expression::BinaryOperator::NotEqual
                        | typed_trees::expression::BinaryOperator::Less
                        | typed_trees::expression::BinaryOperator::LessOrEqual
                        | typed_trees::expression::BinaryOperator::Greater
                        | typed_trees::expression::BinaryOperator::GreaterOrEqual
                ) =>
            {
                // Definition predicates and proposition telescopes need not
                // belong to an executable state. Their exact named binders
                // still supply destinations; scoped traversal below also
                // resolves computed results in bodies and callable contracts.
                for (typed, anonymous) in [(binary.left, binary.right), (binary.right, binary.left)]
                {
                    if let ExpressionNode::Name(path) = program.expression_table.expression(typed)
                        && let Some(declared) =
                            crate::value_custody::expression_types::named_value_type_reference(
                                program, path,
                            )
                    {
                        pairs.push((anonymous, declared));
                    }
                }
                anonymous_comparisons.push(handle);
            }
            _ => {}
        }
    }

    for machine in program.machines() {
        if let Some(entry) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == "entry")
        {
            collect_result_contract_float_pairs(
                program,
                program.machine_contracts(machine),
                machine.symbol,
                program.state_parameters(entry),
                entry.return_type,
                &mut pairs,
            );
        }
        for state in program.machine_states(machine) {
            collect_result_contract_float_pairs(
                program,
                program.state_contracts(state),
                state.symbol,
                program.state_parameters(state),
                state.return_type,
                &mut pairs,
            );
            for statement in program.statement_table.statements(state.statement_nodes) {
                for (_, expression) in program.statement_table.expression_roots(statement) {
                    collect_comparison_pairs(
                        program,
                        expression,
                        &|operand| {
                            crate::value_custody::expression_types::expression_result_type_reference(
                                program, machine, state, operand,
                            )
                        },
                        &mut pairs,
                    );
                }
                match statement {
                    // A machine/state's direct terminal expression delivers
                    // its result without a TransitionTargetNode::Value wrapper.
                    StatementNode::Expression(value) if state.return_type.is_valid() => {
                        pairs.push((*value, state.return_type));
                    }
                    StatementNode::Call(call) => {
                        call_destinations::collect(program, machine, call, &mut pairs);
                    }
                    StatementNode::Assignment(assignment) => {
                        if let Some(declared) =
                            crate::value_custody::places::declared_place_type_raw(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                        {
                            pairs.push((assignment.value, declared));
                        }
                    }
                    StatementNode::LocalData(local) => {
                        if local.initial_value.is_valid() && local.type_reference.is_valid() {
                            pairs.push((local.initial_value, local.type_reference));
                        }
                    }
                    StatementNode::Transition(transition) => {
                        // A transition ARG adopts the TARGET state's declared
                        // param type (the arg IS a delivery into that
                        // destination -- same law as a `let`): `check(2.0e0 +
                        // tiny)` into `got: f32` folds/evaluates per-op at
                        // f32. Same-machine targets only (value-machine call
                        // args ride the Call statement, a later face).
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            let target_node = program.statement_table.transition_target(target);
                            if let typed_trees::statement::TransitionTargetNode::Value(value) =
                                target_node
                                && state.return_type.is_valid()
                            {
                                pairs.push((*value, state.return_type));
                                continue;
                            }
                            let typed_trees::statement::TransitionTargetNode::Named {
                                path,
                                arguments,
                                ..
                            } = target_node
                            else {
                                continue;
                            };
                            // The Named target's path members live in the
                            // STATEMENT table's identifier arena (the target
                            // node's home), not the expression table's.
                            let target_members =
                                program.statement_table.name_path_members(path.members);
                            let [target_name] = target_members else {
                                continue;
                            };
                            let Some(target_state) = program
                                .machine_states(machine)
                                .iter()
                                .find(|candidate| candidate.name.as_str() == target_name.as_str())
                            else {
                                continue;
                            };
                            // The `&mut self` receiver rides the param list but
                            // never pairs with a spelled argument -- zip only the
                            // value params.
                            let parameters = program.state_parameters(target_state).iter();
                            let argument_handles =
                                program.statement_table.expression_handles(*arguments);
                            for (parameter, argument) in
                                parameters.zip(argument_handles.iter().copied())
                            {
                                if parameter.type_reference.is_valid() {
                                    pairs.push((argument, parameter.type_reference));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        collect_result_contract_float_pairs(
            program,
            program.operator_contracts(operator),
            operator.symbol,
            program.operator_parameters(operator),
            operator.return_type,
            &mut pairs,
        );
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            collect_result_contract_float_pairs(
                program,
                program.state_signature_contracts(signature),
                signature.symbol,
                program.state_signature_parameters(signature),
                signature.return_type,
                &mut pairs,
            );
        }
    }

    for (value, format) in direct_formats {
        land_float_tree(program, value, format);
    }
    for (value, declared) in pairs {
        land_float_value_for_type(program, value, declared);
    }
    // A comparison with no typed operand is itself the first destination: it
    // produces bool. Its anonymous float operands therefore compare as exact
    // values (including format-independent NaN/infinity), rather than each
    // independently falling through the transitional f64 window.
    for comparison in anonymous_comparisons {
        fold_anonymous_float_comparison(program, comparison);
    }
}

fn collect_result_contract_float_pairs(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    return_type: TypeReferenceHandle,
    pairs: &mut Vec<FloatLandingPair>,
) {
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            collect_comparison_pairs(
                program,
                *expression,
                &|operand| {
                    if program.expression_table.names_reserved_result(operand) {
                        return return_type.is_valid().then_some(return_type);
                    }
                    crate::value_custody::expression_types::parameter_expression_result_type_reference(
                        program, owner, parameters, operand,
                    )
                },
                pairs,
            );
        }
    }
}

/// A comparison receives values, not source shapes. Its typed operand supplies
/// the anonymous peer's landing whether it is a place, a call, a cast or a
/// computed expression. The shared result query retains selected operator
/// results; inferring a format from an arbitrary descendant would cross a
/// conversion or impose an operand's type on a user-defined result.
///
/// Visit the same expression edges in initializers, fields, guards, arguments
/// and contracts. This only records destinations: landing below rounds complete
/// anonymous trees once and never changes an already-landed value.
fn collect_comparison_pairs(
    program: &TypedTrees,
    expression: ExpressionHandle,
    result_type: &impl Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    pairs: &mut Vec<FloatLandingPair>,
) {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Binary(binary) = node
            && matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Equal
                    | typed_trees::expression::BinaryOperator::NotEqual
                    | typed_trees::expression::BinaryOperator::Less
                    | typed_trees::expression::BinaryOperator::LessOrEqual
                    | typed_trees::expression::BinaryOperator::Greater
                    | typed_trees::expression::BinaryOperator::GreaterOrEqual
            )
        {
            for (typed, anonymous) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if let Some(declared) = result_type(typed) {
                    pairs.push((anonymous, declared));
                }
            }
        }
        crate::expression_traversal::children(program, node, |child| pending.push(child));
    }
}

fn land_float_value_for_type(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    declared: TypeReferenceHandle,
) {
    use typed_trees::types::TypeReferenceNode;

    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(value) {
        let values = program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .map(|arm| arm.value)
            .collect::<Vec<_>>();
        for value in values {
            land_float_value_for_type(program, value, declared);
        }
        return;
    }
    let declared_node = program
        .type_reference_table
        .type_reference(declared)
        .clone();
    match declared_node {
        TypeReferenceNode::Reference { referee, .. } => {
            land_float_value_for_type(program, value, referee);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            land_float_value_for_type(program, value, base_type);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            let ExpressionNode::ArrayLiteral(elements) =
                program.expression_table.expression(value).clone()
            else {
                return;
            };
            let elements = program
                .expression_table
                .expression_handles(elements)
                .to_vec();
            for element in elements {
                land_float_value_for_type(program, element, element_type);
            }
        }
        TypeReferenceNode::Named { .. } => {
            let format = match program.primitive_type_reference(declared) {
                Some(PrimitiveType::F32) => numerics::literals::FloatFormat::F32,
                Some(PrimitiveType::F64) => numerics::literals::FloatFormat::F64,
                _ => return,
            };
            land_float_tree(program, value, format);
        }
        TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => {}
    }
}

fn land_float_tree(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    format: numerics::literals::FloatFormat,
) {
    // Integer spellings are anonymous exact values too. Evaluate the complete
    // rational before requesting a float format; rounding each leaf would
    // change division and double-round large intermediate values. Typed
    // integer operands are deliberately declined by the shared evaluator.
    let mut builtin =
        |expression| super::integer_landing::has_anonymous_operator_meaning(program, expression);
    let exact = super::integer_landing::anonymous_integer_literal_tree_value(
        program,
        expression,
        &mut builtin,
    )
    .map(|evaluated| numerics::bignum::ExactFloat::Finite(evaluated.value))
    .or_else(|| anonymous_exact_float_tree(program, expression));
    if let Some(exact) = exact {
        let semantic_format = match format {
            numerics::literals::FloatFormat::F32 => {
                numerics::float_semantics::FloatFormat::BINARY32
            }
            numerics::literals::FloatFormat::F64 => {
                numerics::float_semantics::FloatFormat::BINARY64
            }
        };
        let value = numerics::float_semantics::FloatSemantics::round_exact(semantic_format, exact)
            .to_interpreter_value(semantic_format);
        *program.expression_table.expression_mut(expression) = ExpressionNode::Float(
            numerics::literals::FloatLiteral::from_f64(value).with_landing(format),
        );
    } else if numeric_operands::numeric_literal_tree(program, expression).is_none() {
        stamp_float_tree(program, expression, format);
    }
}

fn anonymous_exact_float_tree(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<numerics::bignum::ExactFloat> {
    use typed_trees::expression::BinaryOperator;

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) if literal.landing().is_none() => {
            Some(numerics::bignum::ExactFloat::Finite(
                numerics::bignum::BigRational::from_integer(literal.value_bignum()?),
            ))
        }
        ExpressionNode::Float(literal) if literal.landing().is_none() => {
            numerics::bignum::ExactFloat::from_decimal_str(literal.text())
        }
        ExpressionNode::Borrow(inner) => anonymous_exact_float_tree(program, inner.target),
        ExpressionNode::Binary(binary)
            if super::integer_landing::has_anonymous_operator_meaning(program, expression) =>
        {
            let left = anonymous_exact_float_tree(program, binary.left)?;
            let right = anonymous_exact_float_tree(program, binary.right)?;
            Some(match binary.operator {
                BinaryOperator::Add => left.add(&right),
                BinaryOperator::Subtract => left.sub(&right),
                BinaryOperator::Multiply => left.mul(&right),
                BinaryOperator::Divide => {
                    if matches!(&right, numerics::bignum::ExactFloat::Finite(value) if value.is_zero())
                    {
                        return None;
                    }
                    left.div(&right)
                }
                _ => return None,
            })
        }
        _ => None,
    }
}

fn fold_anonymous_float_comparison(program: &mut TypedTrees, expression: ExpressionHandle) {
    use std::cmp::Ordering;
    use typed_trees::expression::BinaryOperator;

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression).clone()
    else {
        return;
    };
    let spelling = match binary.operator {
        BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
        BinaryOperator::NotEqual => language_core::OperatorSpelling::NotEqual,
        BinaryOperator::Greater => language_core::OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => language_core::OperatorSpelling::GreaterEqual,
        BinaryOperator::Less => language_core::OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => language_core::OperatorSpelling::LessEqual,
        _ => return,
    };
    if !super::integer_landing::has_builtin_anonymous_operands(program, expression, spelling) {
        return;
    }
    let Some(left) = anonymous_exact_float_tree(program, binary.left) else {
        return;
    };
    let Some(right) = anonymous_exact_float_tree(program, binary.right) else {
        return;
    };
    let value = match binary.operator {
        BinaryOperator::Equal => left.equal_value(&right),
        BinaryOperator::NotEqual => !left.equal_value(&right),
        BinaryOperator::Greater => left.partial_cmp_value(&right) == Some(Ordering::Greater),
        BinaryOperator::GreaterOrEqual => matches!(
            left.partial_cmp_value(&right),
            Some(Ordering::Greater | Ordering::Equal)
        ),
        BinaryOperator::Less => left.partial_cmp_value(&right) == Some(Ordering::Less),
        BinaryOperator::LessOrEqual => matches!(
            left.partial_cmp_value(&right),
            Some(Ordering::Less | Ordering::Equal)
        ),
        _ => return,
    };
    *program.expression_table.expression_mut(expression) = ExpressionNode::Boolean(value);
}

/// Stamp every UNSTAMPED float literal reachable through Mutable/Unary/Binary
/// wrappers with `format` when the tree is not wholly anonymous and constant.
/// This is the runtime-expression case: operations execute per-op at the
/// landed width. Suffixed literals keep their own landing (disagreement is
/// validate_suffix_landings' domain). Deliberately does NOT descend into
/// calls/indexes/places.
fn stamp_float_tree(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    format: numerics::literals::FloatFormat,
) {
    // A typed peer requests a landing of the complete anonymous operand,
    // not separate rounding of its leaves. Failed anonymous evaluation must
    // remain unlanded so the formation check can still reject it.
    if numeric_operands::numeric_literal_tree(program, expression).is_some() {
        land_float_tree(program, expression, format);
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            let inner = *inner;
            stamp_float_tree(program, inner.target, format);
        }
        ExpressionNode::Unary(unary) => {
            let operand = unary.operand;
            stamp_float_tree(program, operand, format);
        }
        ExpressionNode::Binary(binary)
            if super::integer_landing::has_anonymous_operator_meaning(program, expression) =>
        {
            let (left, right) = (binary.left, binary.right);
            stamp_float_tree(program, left, format);
            stamp_float_tree(program, right, format);
        }
        ExpressionNode::Float(literal) if literal.landing().is_none() => {
            let landed = literal.with_landing(format);
            *program.expression_table.expression_mut(expression) = ExpressionNode::Float(landed);
        }
        _ => {}
    }
}
