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
    if program
        .expression_table
        .call_node(expression)
        .and_then(|call| call.native_code_layout.as_ref())
        .is_some_and(|query| query.discards_result)
    {
        return true;
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
    matches!(program.statement_table.statements(state.statement_nodes).last(),
        Some(typed_trees::statement::StatementNode::Expression(tail)) if *tail == expression)
        && call_completes_unit_state(program, machine, state, expression)
}

/// Signature compatibility for a direct Unit completion. The caller supplies
/// the actual statement/terminal position; nested operands never inherit it.
pub(crate) fn call_completes_unit_state(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    unit_type(program, state.return_type)
        && program.expression_table.expression_is_valid(expression)
        && call_returns_unit(program, machine, expression)
}

enum DeclaredCallResult<'program> {
    IntrinsicUnit,
    Signature(TypeReferenceHandle),
    Source {
        machine: &'program Machine,
        state: &'program State,
    },
}

/// One logical result query for statement, terminal and wrapper inference.
/// Public requirement custody takes precedence over any private executable.
fn declared_call_result<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<DeclaredCallResult<'program>> {
    let call = program.expression_table.call_node(expression)?;
    if admitted_build_declaration_call(program, expression) {
        return Some(DeclaredCallResult::IntrinsicUnit);
    }
    if !call.target_symbol.is_valid()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
        || call.layout_query.is_some()
        || call.native_code_layout.is_some()
    {
        return None;
    }
    if let Some(signature) = program.requirement_call_signature(machine.symbol, call) {
        return Some(DeclaredCallResult::Signature(signature.return_type));
    }
    if call.static_requirement_dispatch.is_some() {
        return None;
    }
    if let Some((machine, state)) = super::find_machine_head(program, call.target_symbol)
        .or_else(|| super::find_state(program, call.target_symbol))
    {
        return Some(DeclaredCallResult::Source { machine, state });
    }
    if let Some((owner, signature)) = program.machine_parameter_signature(call.target_symbol) {
        return (owner.symbol == machine.symbol)
            .then_some(DeclaredCallResult::Signature(signature.return_type));
    }
    super::call_target_return_type(program, call.target_symbol).map(DeclaredCallResult::Signature)
}

pub(in crate::machine_calls::calls) fn call_returns_unit(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    match declared_call_result(program, machine, expression) {
        Some(DeclaredCallResult::IntrinsicUnit) => true,
        Some(DeclaredCallResult::Signature(reference)) => unit_type(program, reference),
        Some(DeclaredCallResult::Source { machine, state }) => {
            unit_type(program, state.return_type)
                && (state.return_type.is_valid() || !has_inferred_value_return(program, machine))
        }
        None => false,
    }
}

/// The checker establishes the Build namespace and explicit argument before validation;
/// the target owner separately admits default declaration operations. Neither
/// token authorizes execution on a fabricated Build value.
pub(super) fn admitted_build_declaration_call(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let (intrinsic, static_count, value_count) =
        match BuildOperation::from_call_target(call.target.as_str()) {
            Some(BuildOperation::AssumedBindingContract) => (
                AuthoredDeclarationSelectionIntrinsic::BuildAssumedBindingContract,
                2..=2,
                1..=1,
            ),
            Some(BuildOperation::ProviderSelection) => (
                AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection,
                2..=2,
                1..=2,
            ),
            Some(BuildOperation::ServiceExclusion) => (
                AuthoredDeclarationSelectionIntrinsic::BuildServiceExclusion,
                1..=1,
                1..=1,
            ),
            Some(BuildOperation::ServiceAudit) => (
                AuthoredDeclarationSelectionIntrinsic::BuildServiceAudit,
                1..=1,
                1..=1,
            ),
            Some(BuildOperation::BoundaryAcceptance) => (
                AuthoredDeclarationSelectionIntrinsic::BuildBoundaryAcceptance,
                1..=1,
                1..=1,
            ),
            Some(BuildOperation::RepresentationSelection) => (
                AuthoredDeclarationSelectionIntrinsic::BuildRepresentationSelection,
                2..=2,
                1..=1,
            ),
            Some(BuildOperation::WireCompatibilityRequest) => (
                AuthoredDeclarationSelectionIntrinsic::BuildWireCompatibilityRequest,
                5..=usize::MAX,
                1..=1,
            ),
            _ => return false,
        };
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

/// The complete callable result of an ordinary machine. An omitted result is
/// represented by the type table's Unit sentinel; it is not a missing scalar
/// carrier. Explicit non-Unit state results and inferred value-return edges
/// still prevent treating the machine as a resultless citation.
pub fn machine_has_unit_result(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    let states = program.machine_states(machine);
    !states.is_empty()
        && states
            .iter()
            .all(|state| unit_type(program, state.return_type))
        && !has_inferred_value_return(program, machine)
}

// A finite dependency walk follows logical results, not Value-node syntax.
// A cycle introduces no value by itself; termination and successful completion
// remain independent checking obligations, never inferred from this query.
fn has_inferred_value_return(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    use typed_trees::statement::TransitionTargetNode;
    let mut pending = vec![machine];
    let mut visited = Vec::new();
    while let Some(machine) = pending.pop() {
        if visited.contains(&machine.symbol) {
            continue;
        }
        visited.push(machine.symbol);
        for state in program.machine_states(machine) {
            if !unit_type(program, state.return_type) {
                return true;
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                for target in transition.targets() {
                    let TransitionTargetNode::Value(expression) =
                        program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    match declared_call_result(program, machine, *expression) {
                        Some(DeclaredCallResult::IntrinsicUnit) => {}
                        Some(DeclaredCallResult::Signature(reference))
                            if unit_type(program, reference) => {}
                        Some(DeclaredCallResult::Source { machine, state })
                            if unit_type(program, state.return_type) =>
                        {
                            if !state.return_type.is_valid() {
                                pending.push(machine);
                            }
                        }
                        _ => return true,
                    }
                }
            }
        }
    }
    false
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
