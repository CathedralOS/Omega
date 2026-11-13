//! Source preparation gives anonymous float expressions their actual declared
//! destinations. Exact evaluation and numeric formation stay in typed source
//! queries; this owner only records the resulting source representation.

use crate::tables::expressions_of as expression_table;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::source_numeric::{anonymous_exact_float_tree, numeric_literal_tree};
use typed_trees::statement::StatementNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(crate) type FloatLandingPair = (ExpressionHandle, TypeReferenceHandle);

pub(crate) mod call_destinations {
    use super::FloatLandingPair;
    use typed_trees::machine::Machine;
    use typed_trees::signature::StateParameter;
    use typed_trees::statement::TableCall;

    pub(crate) fn collect(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        call: &TableCall,
        pairs: &mut Vec<FloatLandingPair>,
    ) {
        if !call.target_symbol.is_valid() {
            return;
        }
        let Some(parameters) = parameters(program, machine, call) else {
            return;
        };
        let arguments = program.statement_table.expression_handles(call.arguments);
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if parameter.type_reference.is_valid() {
                pairs.push((*argument, parameter.type_reference));
            }
        }
    }

    fn parameters<'program>(
        program: &'program dyn validation::CheckingInput,
        machine: &Machine,
        call: &TableCall,
    ) -> Option<&'program [StateParameter]> {
        if let Some(signature) = program.machine_parameter_signature_in(machine, call.target_symbol)
        {
            return Some(program.state_signature_parameters(signature));
        }
        for candidate in program.machines() {
            if let Some(state) = program.state_in_machine(candidate, call.target_symbol) {
                return Some(program.state_parameters(state));
            }
        }
        for trait_definition in program.traits() {
            if let Some(signature) =
                program.trait_machine_signature_by_symbol(trait_definition, call.target_symbol)
            {
                return Some(program.state_signature_parameters(signature));
            }
        }
        program
            .all_operators()
            .find(|operator| operator.symbol == call.target_symbol)
            .map(|operator| program.operator_parameters(operator))
    }
}

pub fn land_float_literal_destinations(typed: &mut TypedTrees) {
    use numerics::literals::FloatFormat;

    let checking = validation::CheckingContext::new(&*typed);
    let program: &dyn validation::CheckingInput = &checking;

    let mut pairs: Vec<FloatLandingPair> = Vec::new();
    let mut direct_formats: Vec<(ExpressionHandle, FloatFormat)> = Vec::new();
    let mut anonymous_comparisons: Vec<ExpressionHandle> = Vec::new();

    for (handle, node) in expression_table(program).expression_entries() {
        match node {
            ExpressionNode::StructLiteral(literal) => {
                each_selected_literal_field(program, handle, literal, |value, field_type| {
                    pairs.push((value, field_type))
                });
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
                let arguments = expression_table(program)
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
                for (typed, anonymous) in [(binary.left, binary.right), (binary.right, binary.left)]
                {
                    if let Some(path) = expression_table(program).name_path_node(typed)
                        && let Some(declared) =
                            typed_trees::source_operator_types::named_value_type_reference(
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
        if let Some(entry) = program.machine_states(machine).first() {
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
                            crate::source_type_queries::expression_result_type_reference(
                                program, machine, state, operand,
                            )
                        },
                        &mut pairs,
                    );
                }
                match statement {
                    StatementNode::Expression(value) if state.return_type.is_valid() => {
                        pairs.push((*value, state.return_type));
                    }
                    StatementNode::Call(call) => {
                        call_destinations::collect(program, machine, call, &mut pairs);
                    }
                    StatementNode::Assignment(assignment) => {
                        if let Some(declared) = validation::declared_place_type_raw(
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
                    StatementNode::Transition(transition) => {
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
                                named_transition_arguments(program, target_node)
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

    drop(checking);
    let program = typed;

    for (value, format) in direct_formats {
        land_float_tree(program, value, format);
    }
    for (value, declared) in pairs {
        land_float_value_for_type(program, value, declared);
    }
    for comparison in anonymous_comparisons {
        fold_anonymous_float_comparison(program, comparison);
    }
}

fn collect_result_contract_float_pairs(
    program: &dyn validation::CheckingInput,
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
                    if expression_table(program).names_reserved_result(operand) {
                        return return_type.is_valid().then_some(return_type);
                    }
                    crate::source_type_queries::parameter_expression_result_type_reference(
                        program, owner, parameters, operand,
                    )
                },
                pairs,
            );
        }
    }
}

fn collect_comparison_pairs(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
    result_type: &impl Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    pairs: &mut Vec<FloatLandingPair>,
) {
    validation::expression_traversal::visit_pre_order(program, vec![expression], |_, node| {
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

    if let Some(dispatch) = expression_table(program).match_node(value) {
        let values = expression_table(program)
            .match_arms(dispatch.arms)
            .iter()
            .map(|arm| arm.value)
            .collect::<Vec<_>>();
        for value in values {
            land_float_value_for_type(program, value, declared);
        }
        return;
    }
    if let ExpressionNode::StructLiteral(literal) = expression_table(program).expression(value) {
        let fields = if literal.case_symbol.is_some() {
            validation::structural_case_constructor(program, value, declared)
                .map(|constructor| {
                    constructor
                        .fields
                        .into_iter()
                        .map(|(_, value, reference)| (value, reference))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            let Some(reference) = validation::unwrapped_type_reference(program, declared) else {
                return;
            };
            expression_table(program)
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
            let source = match expression_table(program).expression(value) {
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
            let elements = expression_table(program)
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
        TypeReferenceNode::Callable { .. }
        | TypeReferenceNode::Slice { .. }
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
    let mut builtin = |expression| {
        typed_trees::closed_numeric::has_anonymous_operator_meaning(program, expression)
    };
    let exact = typed_trees::closed_numeric::evaluate_anonymous_value::<false>(
        program,
        expression,
        &[],
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
    } else if numeric_literal_tree(program, expression).is_none() {
        stamp_float_tree(program, expression, format);
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
    if !typed_trees::closed_numeric::has_builtin_anonymous_operands(program, expression, spelling) {
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

fn retain_folded_subtree_occurrences(program: &mut TypedTrees, root: ExpressionHandle) {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut occurrences = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        occurrences.extend(expression_table(program).authored_selection_occurrences(expression));
        validation::expression_traversal::children(
            program,
            expression_table(program).expression(expression),
            |child| pending.push(child),
        );
    }
    program
        .expression_table
        .attach_authored_selection_occurrences(root, occurrences);
}

fn stamp_float_tree(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    format: numerics::literals::FloatFormat,
) {
    if numeric_literal_tree(program, expression).is_some() {
        land_float_tree(program, expression, format);
        return;
    }
    match expression_table(program).expression(expression) {
        ExpressionNode::Borrow(inner) => {
            let inner = *inner;
            stamp_float_tree(program, inner.target, format);
        }
        ExpressionNode::Unary(unary) => {
            let operand = unary.operand;
            stamp_float_tree(program, operand, format);
        }
        ExpressionNode::Binary(binary)
            if typed_trees::closed_numeric::has_anonymous_operator_meaning(program, expression) =>
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

fn each_selected_literal_field(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
    literal: &typed_trees::expression::TableStructLiteral,
    mut each: impl FnMut(ExpressionHandle, TypeReferenceHandle),
) {
    for field in program.expression_table.struct_fields(literal.fields) {
        if let Some(field_type) = validation::selected_construction_field_type(
            program,
            expression,
            literal,
            field.field_symbol,
        ) {
            each(field.value, field_type);
        }
    }
}

fn named_transition_arguments<'program>(
    program: &'program dyn validation::CheckingInput,
    target: &typed_trees::statement::TransitionTargetNode,
) -> Option<
    Vec<(
        &'program typed_trees::signature::StateParameter,
        ExpressionHandle,
    )>,
> {
    let typed_trees::statement::TransitionTargetNode::Named {
        path, arguments, ..
    } = target
    else {
        return None;
    };
    let parameters =
        typed_trees::calls::declaration_queries::call_target_parameters(program, path.symbol)?;
    Some(
        parameters
            .iter()
            .zip(
                program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .copied(),
            )
            .collect(),
    )
}
