//! Unit calls retain their exact statement or return use and declaration.

use super::{ExpressionHandle, Machine, State, TypeReferenceHandle, TypeReferenceNode};
use language_semantics::declaration_selection::BuildOperation;
/// A standalone call may perform Unit work before later statements. The final
/// expression still has the state's return contract. Call this for a direct
/// statement root; recursive validators must also retain the current use
/// position, since a reused handle does not make a nested operand a statement.
pub fn unit_statement_call_is_admitted(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut occurrences = statements.iter().enumerate().filter_map(|(position, statement)| {
        matches!(statement, typed_trees::statement::StatementNode::Expression(root) if *root == expression)
            .then_some(position)
    });
    let Some(position) = occurrences.next() else {
        return false;
    };
    if occurrences.next().is_some() {
        return false;
    }
    if position + 1 == statements.len() {
        return unit_return_call_is_admitted(program, machine, state, expression);
    }
    call_returns_unit(program, machine, expression)
}

pub fn unit_return_call_is_admitted(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if !unit_type(program, state.return_type)
        || !program.expression_table.expression_is_valid(expression)
        || !matches!(
            program.statement_table.statements(state.statement_nodes).last(),
            Some(typed_trees::statement::StatementNode::Expression(tail)) if *tail == expression
        )
    {
        return false;
    }
    call_returns_unit(program, machine, expression)
}

pub(in crate::machine_calls::calls) fn call_returns_unit(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    let Some(call) = program.expression_table.source_call_node(expression) else {
        return false;
    };
    if admitted_build_declaration_call(program, expression) {
        return true;
    }
    if !call.target_symbol.is_valid()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return false;
    }
    // State spans are disjoint append-only ranges, so a state symbol's
    // holder is unique — no second-machine witness can exist.
    if let Some(target) = program.machine_holding_state(call.target_symbol) {
        let Some(target_state) = program
            .machine_states(target)
            .iter()
            .find(|candidate| candidate.symbol == call.target_symbol)
        else {
            return false;
        };
        return unit_type(program, target_state.return_type)
            && (target_state.return_type.is_valid()
                || !has_inferred_value_return(program, target));
    }
    let requirement = match program.machine_parameter_signature(call.target_symbol) {
        Some((owner, signature)) if owner.symbol == machine.symbol => signature.symbol,
        Some(_) => return false,
        None => call.target_symbol,
    };
    program
        .selected_trait_requirement_signature(requirement, call.authored_machine_application)
        .is_some_and(|(definition, signature)| {
            definition.is_boundary && unit_type(program, signature.return_type)
        })
}

/// The checker establishes the Build namespace and explicit argument before validation;
/// the target owner separately admits default declaration operations. Neither
/// token authorizes execution on a fabricated Build value.
pub(super) fn admitted_build_declaration_call(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let Some(operation) = BuildOperation::from_call_target(call.target.as_str()) else {
        return false;
    };
    let Some((static_count, value_count)) = operation.declaration_operand_counts() else {
        return false;
    };
    let intrinsic = operation.intrinsic();
    if call.target_symbol.is_valid()
        || !call.receiver.is_valid()
        || !static_count.contains(&call.machine_arguments.len())
        || !value_count.contains(
            &program
                .expression_table
                .expression_handles(call.arguments)
                .len(),
        )
        || !call.evidence_arguments.is_empty()
        || !call.selects_only_nominal_route()
    {
        return false;
    }
    crate::compiler_operations::authored_call_intrinsic(
        program,
        typed_trees::references::StaticArgumentCall::Expression(expression, call),
    ) == Some(intrinsic)
}

fn has_inferred_value_return(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    use typed_trees::statement::TransitionTargetNode;
    program.machine_states(machine).iter().any(|state| {
        (state.return_type.is_valid() && !unit_type(program, state.return_type))
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    let Some(transition) = statement.as_transition() else {
                        return false;
                    };
                    transition.targets().any(|target| {
                        matches!(
                            program.statement_table.transition_target(target),
                            TransitionTargetNode::Value(_)
                        )
                    })
                })
    })
}

fn unit_type(program: &dyn crate::CheckingInput, mut reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Unit => return true,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return false,
        }
    }
}
