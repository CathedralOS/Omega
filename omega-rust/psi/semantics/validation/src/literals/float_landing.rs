use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

type FloatLandingPair = (ExpressionHandle, TypeReferenceHandle);

mod call_destinations;
mod integer_operands;
pub(super) use integer_operands::validate_integer_tree_destination;

/// F2b -- float DESTINATION stamping (ch5 two-phase constants, the float
/// half): an UNSUFFIXED float literal initializing a declared `f32`/`f64`
/// place lands that format ON ITS VALUE. A wholly anonymous literal-arithmetic
/// tree is first evaluated as an exact rational and then replaced by one
/// landed literal, so every downstream reader -- native store, guard compare,
/// argument materialization, AND the interpreter -- consumes the same single
/// rounding. A nonconstant tree instead stamps its anonymous literal leaves;
/// its runtime operations then execute at the destination format.
/// Without this landing pass an anonymous literal at an f32 place takes the
/// transitional f64-then-narrow route (double rounding; the
/// 8388609.499999999999999 witness lands on the wrong side of the tie).
///
/// Declared storage, call arguments, and returned values supply destinations.
/// Suffix agreement is checked at storage/return sites and by the resolved
/// call validators. Already-landed (suffixed) literals are untouched: their landing
/// was chosen at the spelling, and the suffix-vs-destination check owns any
/// disagreement. Comparisons against a typed named value, including a
/// proposition parameter, likewise land the opposite literal tree to that
/// value's format. A callable contract's `result` root uses the exact return
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
                let Some(data_definition) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == literal.type_name.as_str())
                else {
                    continue;
                };
                for field in program.expression_table.struct_fields(literal.fields) {
                    let Some(field_type) = crate::struct_literals::construction_field_type(
                        program,
                        data_definition,
                        literal.case_name.as_ref().map(|name| name.as_str()),
                        field.name.as_str(),
                    ) else {
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
                let Some(callee) = program.machines().iter().find_map(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == call.target_symbol)
                }) else {
                    continue;
                };
                let parameters = program
                    .state_parameters(callee)
                    .iter()
                    .filter(|parameter| !parameter.is_self);
                let arguments = program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied();
                for (parameter, argument) in parameters.zip(arguments) {
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
                if let ExpressionNode::Name(path) = program.expression_table.expression(binary.left)
                    && let Some(declared) =
                        crate::expression_types::named_value_type_reference(program, path)
                {
                    pairs.push((binary.right, declared));
                }
                if let ExpressionNode::Name(path) =
                    program.expression_table.expression(binary.right)
                    && let Some(declared) =
                        crate::expression_types::named_value_type_reference(program, path)
                {
                    pairs.push((binary.left, declared));
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
                entry.return_type,
                &mut pairs,
            );
        }
        for state in program.machine_states(machine) {
            collect_result_contract_float_pairs(
                program,
                program.state_contracts(state),
                state.return_type,
                &mut pairs,
            );
            for statement in program.statement_table.statements(state.statement_nodes) {
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
                        if let Some(declared) = crate::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        ) {
                            pairs.push((assignment.value, declared));
                        }
                    }
                    StatementNode::LocalData(local) => {
                        if local.initial_value.is_valid() && local.type_reference.is_valid() {
                            pairs.push((local.initial_value, local.type_reference));
                        }
                    }
                    // A float COMPARISON in a transition guard adopts the
                    // PLACE side's format (the operand-derived landing law,
                    // float flavor): `self.f == 16777216.0 + 1.0` with an f32
                    // place must fold/evaluate its literal side per-op at f32
                    // -- unstamped, the tree computed in the anonymous f64
                    // window and the engines diverged at the f32 precision
                    // cliff (2^24 + 1.0). Recursive like
                    // bless_equality_guard_literals: the multi-arm desugar
                    // wraps the spelled compare as `(subject) == true`, and
                    // conjunctions nest comparisons under And/Or legs.
                    // Suffixed literals keep their own landing (stamp-if-none
                    // in the shared loop below).
                    StatementNode::Transition(transition) => {
                        if let typed_trees::statement::TransitionGuardNode::When(guard) =
                            transition.guard
                        {
                            collect_guard_float_comparison_pairs(
                                program, machine, state, guard, &mut pairs,
                            );
                        }
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
                            let parameters = program
                                .state_parameters(target_state)
                                .iter()
                                .filter(|parameter| !parameter.is_self);
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
            operator.return_type,
            &mut pairs,
        );
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            collect_result_contract_float_pairs(
                program,
                program.state_signature_contracts(signature),
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
    return_type: TypeReferenceHandle,
    pairs: &mut Vec<FloatLandingPair>,
) {
    if !return_type.is_valid() {
        return;
    }
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            collect_result_comparison_pairs(program, *expression, return_type, pairs, 0);
        }
    }
}

fn collect_result_comparison_pairs(
    program: &TypedTrees,
    expression: ExpressionHandle,
    return_type: TypeReferenceHandle,
    pairs: &mut Vec<FloatLandingPair>,
    depth: usize,
) {
    if !expression.is_valid() || depth >= 256 {
        return;
    }
    let recurse = |child, pairs: &mut Vec<_>| {
        collect_result_comparison_pairs(program, child, return_type, pairs, depth + 1)
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            recurse(atomic.value, pairs);
            recurse(atomic.result, pairs);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                recurse(*element, pairs);
            }
        }
        ExpressionNode::Binary(binary) => {
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Equal
                    | typed_trees::expression::BinaryOperator::NotEqual
                    | typed_trees::expression::BinaryOperator::Less
                    | typed_trees::expression::BinaryOperator::LessOrEqual
                    | typed_trees::expression::BinaryOperator::Greater
                    | typed_trees::expression::BinaryOperator::GreaterOrEqual
            ) {
                if expression_is_result(program, binary.left) {
                    pairs.push((binary.right, return_type));
                }
                if expression_is_result(program, binary.right) {
                    pairs.push((binary.left, return_type));
                }
            }
            recurse(binary.left, pairs);
            recurse(binary.right, pairs);
        }
        ExpressionNode::Borrow(borrow) => recurse(borrow.target, pairs),
        ExpressionNode::Cast(cast) => recurse(cast.value, pairs),
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                recurse(call.receiver, pairs);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, pairs);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection, pairs);
            recurse(indexed.index, pairs);
        }
        ExpressionNode::Member(member) => recurse(member.receiver, pairs),
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                recurse(range.start, pairs);
            }
            if range.end.is_valid() {
                recurse(range.end, pairs);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                recurse(field.value, pairs);
            }
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand, pairs),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn expression_is_result(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(path)
            if matches!(
                program.expression_table.name_path_members(path.members),
                [name] if name.as_str() == "result"
            )
    )
}

fn land_float_value_for_type(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    declared: TypeReferenceHandle,
) {
    use typed_trees::types::TypeReferenceNode;

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
    let exact = super::integer_landing::anonymous_numeric_value(program, expression, &mut builtin)
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
    } else {
        stamp_float_tree(program, expression, format);
    }
}

fn anonymous_exact_float_tree(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<numerics::bignum::ExactFloat> {
    use typed_trees::expression::BinaryOperator;

    match program.expression_table.expression(expression) {
        ExpressionNode::Float(literal) if literal.landing().is_none() => {
            numerics::bignum::ExactFloat::from_decimal_str(literal.text())
        }
        ExpressionNode::Borrow(inner) => anonymous_exact_float_tree(program, inner.target),
        ExpressionNode::Binary(binary) => {
            let left = anonymous_exact_float_tree(program, binary.left)?;
            let right = anonymous_exact_float_tree(program, binary.right)?;
            Some(match binary.operator {
                BinaryOperator::Add => left.add(&right),
                BinaryOperator::Subtract => left.sub(&right),
                BinaryOperator::Multiply => left.mul(&right),
                BinaryOperator::Divide => left.div(&right),
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

/// Collect (float-literal-tree, place-declared-type) pairs from every
/// comparison reachable in a guard expression: And/Or legs recurse, and each
/// Equal/NotEqual/ordering node pairs its literal side with its place side's
/// declared type (either orientation). The multi-arm desugar wraps the spelled
/// compare as `(subject) == true`, so comparisons nest inside equality legs --
/// recurse through comparison legs too, exactly like
/// bless_equality_guard_literals.
fn collect_guard_float_comparison_pairs(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    guard: ExpressionHandle,
    pairs: &mut Vec<(ExpressionHandle, typed_trees::types::TypeReferenceHandle)>,
) {
    if !guard.is_valid() {
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return;
    };
    use typed_trees::expression::BinaryOperator;
    match binary.operator {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            if let Some(declared) =
                crate::places::declared_place_type_raw(program, machine, Some(state), binary.left)
            {
                pairs.push((binary.right, declared));
            } else if let Some(declared) =
                crate::places::declared_place_type_raw(program, machine, Some(state), binary.right)
            {
                pairs.push((binary.left, declared));
            }
            collect_guard_float_comparison_pairs(program, machine, state, binary.left, pairs);
            collect_guard_float_comparison_pairs(program, machine, state, binary.right, pairs);
        }
        BinaryOperator::And | BinaryOperator::Or => {
            collect_guard_float_comparison_pairs(program, machine, state, binary.left, pairs);
            collect_guard_float_comparison_pairs(program, machine, state, binary.right, pairs);
        }
        _ => {}
    }
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
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            let inner = *inner;
            stamp_float_tree(program, inner.target, format);
        }
        ExpressionNode::Unary(unary) => {
            let operand = unary.operand;
            stamp_float_tree(program, operand, format);
        }
        ExpressionNode::Binary(binary) => {
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
