//! Destination compatibility consumes finalized operator selection. Operand
//! formats describe a builtin result only after selection retained that builtin.

use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus, CheckedValueOrigin};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn check(program: &TypedTrees, facts: &CheckFacts) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut check = |value, destination| {
                    check_destination(
                        program,
                        facts,
                        value,
                        destination,
                        state.symbol,
                        statement_index,
                        &mut diagnostics,
                    );
                };
                match statement {
                    StatementNode::Call(call) => {
                        if let Some(symbol) =
                            crate::flow::resolved_operator_statement_symbol(program, call)
                            && let Some(operator) =
                                typed_trees::operator::declaration_by_symbol(program, symbol)
                        {
                            let arguments =
                                program.statement_table.expression_handles(call.arguments);
                            for (argument, parameter) in
                                arguments.iter().zip(positional_operator_parameters(
                                    program.operator_parameters(operator),
                                    arguments.len(),
                                ))
                            {
                                check(*argument, parameter.type_reference);
                            }
                        }
                    }
                    StatementNode::LocalData(local) => {
                        check(local.initial_value, local.type_reference)
                    }
                    StatementNode::Assignment(assignment) => {
                        if let Some(destination) = validation::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        ) {
                            // A store through a reference delivers into its
                            // referent, not into the reference carrier.
                            let destination =
                                match program.type_reference_table.type_reference(destination) {
                                    TypeReferenceNode::Reference { referee, .. } => *referee,
                                    _ => destination,
                                };
                            check(assignment.value, destination);
                        }
                    }
                    StatementNode::Expression(value) => check(*value, state.return_type),
                    StatementNode::Transition(transition) => {
                        for target in [transition.target, transition.continuation] {
                            if target.is_valid()
                                && let TransitionTargetNode::Value(value) =
                                    program.statement_table.transition_target(target)
                            {
                                check(*value, state.return_type);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Flow owns exact call targets and evaluation order for statement calls,
    // nested expression calls, state transfers, and attached receivers.
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            let Some(site) = crate::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let Some(parameters) = crate::call_target_parameters(program, call.target_symbol)
            else {
                continue;
            };
            for (argument, parameter) in crate::call_site_argument_expressions(program, &site)
                .iter()
                .zip(parameters.iter().filter(|parameter| !parameter.is_self))
            {
                check_destination(
                    program,
                    facts,
                    *argument,
                    parameter.type_reference,
                    state.state_symbol,
                    call.statement_index,
                    &mut diagnostics,
                );
            }
        }
    }
    // Named operator calls have selected declaration facts instead of ordinary
    // machine-call flow rows. Their parameters are destinations too.
    for (_, selected) in facts.operators.named_uses.iter() {
        let CheckedValueOrigin::StateStatement {
            state_symbol,
            statement_index,
            ..
        } = selected.origin
        else {
            continue;
        };
        let ExpressionNode::Call(call) = program.expression_table.expression(selected.expression)
        else {
            continue;
        };
        let Some(operator) = typed_trees::operator::declaration_by_symbol(
            program,
            selected.selected_operator_symbol,
        ) else {
            continue;
        };
        let arguments = program.expression_table.expression_handles(call.arguments);
        for (argument, parameter) in arguments.iter().zip(positional_operator_parameters(
            program.operator_parameters(operator),
            arguments.len(),
        )) {
            check_destination(
                program,
                facts,
                *argument,
                parameter.type_reference,
                state_symbol,
                statement_index,
                &mut diagnostics,
            );
        }
    }
    diagnostics
}

fn positional_operator_parameters(
    parameters: &[typed_trees::signature::StateParameter],
    argument_count: usize,
) -> impl Iterator<Item = &typed_trees::signature::StateParameter> {
    // Named-call resolution has already checked this exact signature's arity.
    // Method form consumes either explicit self or the first ordinary parameter;
    // in the latter case it is the signature's one non-positional parameter.
    let receiver = parameters
        .iter()
        .position(|parameter| parameter.is_self)
        .or_else(|| (parameters.len().checked_sub(argument_count) == Some(1)).then_some(0));
    parameters
        .iter()
        .enumerate()
        .filter_map(move |(index, parameter)| (Some(index) != receiver).then_some(parameter))
}

fn check_destination(
    program: &TypedTrees,
    facts: &CheckFacts,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() || !destination.is_valid() {
        return;
    }
    let destination = match program.type_reference_table.type_reference(destination) {
        TypeReferenceNode::Constrained { base_type, .. } => *base_type,
        _ => destination,
    };
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value)
        && let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == literal.type_symbol)
    {
        for field in program.expression_table.struct_fields(literal.fields) {
            let field_type = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(declared)
                        if declared.symbol == field.field_symbol =>
                    {
                        Some(declared.type_reference)
                    }
                    typed_trees::data::DataMember::Variant(variant)
                        if Some(variant.symbol) == literal.case_symbol =>
                    {
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|declared| declared.symbol == field.field_symbol)
                            .map(|declared| declared.type_reference)
                    }
                    _ => None,
                });
            if let Some(field_type) = field_type {
                check_destination(
                    program,
                    facts,
                    field.value,
                    field_type,
                    state,
                    statement,
                    diagnostics,
                );
            }
        }
        return;
    }
    if let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value)
        && let TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(destination)
    {
        for element in program.expression_table.expression_handles(*elements) {
            check_destination(
                program,
                facts,
                *element,
                *element_type,
                state,
                statement,
                diagnostics,
            );
        }
        return;
    }
    let Some(target @ (PrimitiveType::F32 | PrimitiveType::F64)) = primitive(program, destination)
    else {
        return;
    };
    let mut expression = value;
    loop {
        expression = match program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => atomic.value,
            ExpressionNode::Unary(unary) => unary.operand,
            _ => break,
        };
    }
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) {
        return;
    }
    if let Some(source @ (PrimitiveType::F32 | PrimitiveType::F64)) =
        result_primitive(program, facts, expression, state, statement)
        && source != target
    {
        diagnostics.push(Diagnostic::error(format!(
            "operator expression `{}` delivers a `{}` result to a `{}` destination; changing a landed float's format requires an explicit conversion",
            program.expression_table.display_name(expression), source.name(), target.name(),
        )));
    }
}

fn result_primitive(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: ExpressionHandle,
    state: symbols::SymbolHandle,
    statement: usize,
) -> Option<PrimitiveType> {
    let origin = CheckedValueOrigin::StateStatement {
        machine_symbol: program.symbols.get(state).parent,
        state_symbol: state,
        statement_index: statement,
        role: checked_trees::CheckedValueStatementRole::Expression,
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            result_primitive(program, facts, atomic.value, state, statement)
        }
        ExpressionNode::Unary(unary) => {
            result_primitive(program, facts, unary.operand, state, statement)
        }
        ExpressionNode::Binary(binary) => {
            let selected = facts.operators.uses.iter().map(|(_, operator)| operator).find(|operator| {
                operator.expression == expression && matches!(operator.origin, CheckedValueOrigin::StateStatement { state_symbol, statement_index, .. } if state_symbol == state && statement_index == statement)
            });
            if let Some(selected) = selected {
                match selected.status {
                    CheckedOperatorResolutionStatus::Resolved => {
                        return facts
                            .operators
                            .selected_candidate(selected)
                            .and_then(|candidate| primitive(program, candidate.return_type));
                    }
                    CheckedOperatorResolutionStatus::BuiltinFallback => {}
                    _ => return None,
                }
            }
            match binary.operator {
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo => {
                    let left = result_primitive(program, facts, binary.left, state, statement);
                    let right = result_primitive(program, facts, binary.right, state, statement);
                    match (left, right) {
                        (Some(left), Some(right)) if left == right => Some(left),
                        (Some(value), None) | (None, Some(value)) => Some(value),
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        ExpressionNode::Float(literal) => literal.landing().map(|format| match format {
            numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
            numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
        }),
        ExpressionNode::Call(call) => {
            typed_trees::operator::resolve_named_expression_call(program, call)
                .map(|operator| operator.return_type)
                .or_else(|| {
                    crate::semantic_calls::find_state(program, call.target_symbol)
                        .map(|state| state.return_type)
                })
                .or_else(|| {
                    program
                        .machine_parameter_signature(call.target_symbol)
                        .map(|(_, signature)| signature.return_type)
                })
                .or_else(|| {
                    program
                        .traits()
                        .iter()
                        .flat_map(|definition| program.trait_machine_signatures(definition))
                        .find(|signature| signature.symbol == call.target_symbol)
                        .map(|signature| signature.return_type)
                })
                .and_then(|reference| primitive(program, reference))
        }
        _ => crate::operators::expression_type_reference_for_origin(program, expression, origin)
            .and_then(|reference| primitive(program, reference)),
    }
}

fn primitive(program: &TypedTrees, reference: TypeReferenceHandle) -> Option<PrimitiveType> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => primitive(program, *base_type),
        _ => program.primitive_type_reference(reference),
    }
}
