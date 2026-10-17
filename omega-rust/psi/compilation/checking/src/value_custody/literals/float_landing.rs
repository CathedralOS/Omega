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
                crate::value_custody::struct_literals::each_selected_literal_field(
                    program,
                    literal,
                    |value, field_type| pairs.push((value, field_type)),
                );
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
                // A named operator is a requirement, not a stored state.
                // Use the same ambiguity-checked selection as call checking:
                // contract calls need not carry an early target symbol, but
                // their selected formals still supply the destination of each
                // literal operand. A receiver-spelled call supplies the first
                // formal through its receiver, exactly as its operand roster
                // does.
                let Some(operator) =
                    typed_trees::operator::resolve_named_expression_call(program, call)
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
                    if let Some(path) = program.expression_table.name_path_node(typed)
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
                for (_, expression) in program.tables.expression_roots(statement) {
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
                        // Every resolved named target supplies its declared
                        // destination, whether it is an internal state or a
                        // machine invocation. Stamp the operand once in that
                        // format; this supplies no call or runtime authority.
                        for target in transition.targets() {
                            let target_node = program.statement_table.transition_target(target);
                            if let typed_trees::statement::TransitionTargetNode::Value(value) =
                                target_node
                                && state.return_type.is_valid()
                            {
                                pairs.push((*value, state.return_type));
                                continue;
                            }
                            let Some(named_arguments) =
                                super::named_transition_arguments(program, target_node)
                            else {
                                continue;
                            };
                            for (parameter, argument) in named_arguments {
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

    for operator in program.all_operators() {
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
    program: &dyn crate::CheckingInput,
    contracts: &[typed_trees::signature::SignatureContract],
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    return_type: TypeReferenceHandle,
    pairs: &mut Vec<FloatLandingPair>,
) {
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(expression) = fact.as_expression() else {
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    result_type: &impl Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    pairs: &mut Vec<FloatLandingPair>,
) {
    crate::expression_traversal::visit_pre_order(program, vec![expression], |_, node| {
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
        false
    });
}

fn land_float_value_for_type(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    declared: TypeReferenceHandle,
) {
    use typed_trees::types::TypeReferenceNode;

    if let Some(dispatch) = program.expression_table.match_node(value) {
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
    // A constructor delivers its fields into the receiving application's
    // prepared telescope. Its authored owner may still be the open template,
    // whose field type alone cannot choose a float format. Translate through
    // the exact constructor/member correspondence before visiting operands;
    // this supplies rounding destinations, not construction or loan authority.
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value) {
        let fields = if literal.case_symbol.is_some() {
            crate::structural_case_constructor(program, value, declared)
                .map(|constructor| {
                    constructor
                        .fields
                        .into_iter()
                        .map(|(_, value, reference)| (value, reference))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            let Some(reference) = crate::unwrapped_type_reference(program, declared) else {
                return;
            };
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .filter_map(|field| {
                    program
                        .closed_data_constructor_field(
                            reference,
                            literal.type_symbol,
                            field.field_symbol,
                        )
                        .map(|selected| (field.value, selected.type_reference))
                })
                .collect()
        };
        for (value, reference) in fields {
            land_float_value_for_type(program, value, reference);
        }
        return;
    }
    let declared_node = program
        .type_reference_table
        .type_reference(declared)
        .clone();
    match declared_node {
        TypeReferenceNode::Reference { referee, .. } => {
            let source = match program.expression_table.expression(value) {
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => value,
            };
            land_float_value_for_type(program, source, referee);
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
        retain_folded_subtree_occurrences(program, expression);
        *program.expression_table.expression_mut(expression) = ExpressionNode::Float(
            numerics::literals::FloatLiteral::from_f64(value).with_landing(format),
        );
    } else if numeric_operands::numeric_literal_tree(program, expression).is_none() {
        stamp_float_tree(program, expression, format);
    }
}

fn anonymous_exact_float_tree(
    program: &dyn crate::CheckingInput,
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
    retain_folded_subtree_occurrences(program, expression);
    *program.expression_table.expression_mut(expression) = ExpressionNode::Boolean(value);
}

/// A successfully evaluated anonymous builtin tree becomes one literal, but
/// every authored operation still belongs to that live value. Retain child
/// occurrence identities before removing their edges. Otherwise a later private
/// projection sees the detached child ledger rows as orphan obligations and
/// cannot exclude them with their owning declaration. This moves custody only:
/// the caller already established builtin numeric meaning, and ordinary checked
/// finalization still resolves each retained occurrence on the literal result.
fn retain_folded_subtree_occurrences(program: &mut TypedTrees, root: ExpressionHandle) {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut occurrences = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        occurrences.extend(
            program
                .expression_table
                .authored_selection_occurrences(expression),
        );
        crate::expression_traversal::children(
            program,
            program.expression_table.expression(expression),
            |child| pending.push(child),
        );
    }
    program
        .expression_table
        .attach_authored_selection_occurrences(root, occurrences);
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
