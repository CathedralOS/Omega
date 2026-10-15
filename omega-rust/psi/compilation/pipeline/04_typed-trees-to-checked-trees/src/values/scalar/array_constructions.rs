//! Array actuals use the captured call occurrence and authored formal position.
//! Discovery does not schedule evaluation or create a synthetic call. Pure and
//! computed leaves share this roster after flow capture, while statement-owned
//! arrays retain their existing pre-flow value production.
use checked_trees::{CheckedArrayConstructionSource, FlowFacts, FlowStateFact};
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Copy)]
pub(crate) struct CallArrayConstruction {
    pub source: CheckedArrayConstructionSource,
    pub expression: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
}

/// The one flow fact captured for `state`; `None` when there is none or more
/// than one. A per-statement walk resolves it once per state rather than
/// rescanning every state fact at each statement.
pub(crate) fn unique_state_flow<'flow>(
    flow: &'flow FlowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<&'flow FlowStateFact> {
    flow.control.unique_state(machine.symbol, state.symbol)
}

/// Array arguments discovered within the unique flow fact already resolved
/// by the caller.
pub(crate) fn call_array_constructions_in(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    captured: &FlowStateFact,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Vec<CallArrayConstruction> {
    let Some(calls) = flow.control.calls.span(captured.calls) else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for call in calls
        .iter()
        .filter(|call| call.statement_index == statement_index)
    {
        if calls
            .iter()
            .filter(|other| {
                other.statement_index == statement_index && other.call_ordinal == call.call_ordinal
            })
            .count()
            != 1
        {
            continue;
        }
        let Some(site) = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            call.call_ordinal,
        ) else {
            continue;
        };
        let target_symbol = match &site {
            validation::CallSite::Expression { call, .. } => {
                validation::call_target_symbol(program, call)
            }
            validation::CallSite::Statement(call) => call.target_symbol,
            validation::CallSite::TransitionNamed { path, .. } => path.symbol,
        };
        if target_symbol != call.target_symbol {
            continue;
        }
        let Some(parameters) = validation::call_target_parameters(program, target_symbol) else {
            continue;
        };
        let return_type = match site {
            // A named invocation may retain its machine head. Keep that
            // authored identity above; its resolved entry owns the result.
            // Internal state transfers keep their successor-owned operands.
            validation::CallSite::TransitionNamed { .. } => {
                validation::terminal_callable_target(program, machine, target_symbol)
                    .map(|(_, return_type)| return_type)
            }
            _ => validation::call_target_return_type(program, target_symbol),
        };
        let Some(return_type) = return_type else {
            continue;
        };
        // A scalar computation owns its nested structural operands. Giving
        // those leaves independent statement roots would schedule them twice
        // and could move construction out of a selective branch.
        let direct_root = match program.statement_table.statement_at(state, statement_index) {
            Some(StatementNode::Call(_)) => {
                matches!(site, validation::CallSite::Statement(_))
            }
            Some(StatementNode::LocalData(local)) => {
                call.authored_expression == local.initial_value
            }
            // A whole boundary application is the store's result operation;
            // ordinary scalar-returning calls belong to its computation instead.
            Some(StatementNode::Assignment(assignment)) => {
                super::call_lowering::call_is_boundary(program, target_symbol)
                    && call.authored_expression == assignment.value
            }
            Some(StatementNode::Expression(expression)) => call.authored_expression == *expression,
            // A terminal invocation owns its argument preparation on the
            // selected arm. Guard and operand expression calls remain under
            // their scalar computation rather than acquiring another owner.
            Some(StatementNode::Transition(_)) => {
                matches!(site, validation::CallSite::TransitionNamed { .. })
            }
            _ => false,
        };
        if !direct_root && program.primitive_type_reference(return_type).is_some() {
            continue;
        }
        // Construction belongs to the caller, independent of the callee's
        // implementation. The common signature lookup retains the original
        // requirement's formals; it never selects or fabricates a provider.
        let arguments = validation::call_site_argument_expressions(program, &site);
        let explicit_parameters = parameters.iter().enumerate();
        if parameters.len() != arguments.len() {
            continue;
        }
        for ((position, parameter), expression) in explicit_parameters.zip(arguments) {
            if (parameter.is_const)
                || parameter.is_mutable
                || !validation::is_closed_primitive_array_type(program, parameter.type_reference)
                || validation::scalar_array_elements(
                    program,
                    machine.symbol,
                    *expression,
                    parameter.type_reference,
                )
                .is_none()
            {
                continue;
            }
            let (Ok(call_ordinal), Ok(parameter_position)) =
                (u32::try_from(call.call_ordinal), u32::try_from(position))
            else {
                continue;
            };
            output.push(CallArrayConstruction {
                source: CheckedArrayConstructionSource::CallArgument {
                    call_ordinal,
                    parameter_position,
                },
                expression: *expression,
                type_reference: parameter.type_reference,
            });
        }
    }
    output
}
