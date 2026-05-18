use crate::host_calls::lowering::{
    find_platform_call_lowering, host_operation, lower_host_call_arguments, platform_call_name,
    platform_call_receiver_type,
};
use crate::host_calls::static_values::{
    StaticValues, apply_call_static_effects, apply_static_assignment, initial_static_values,
};
use crate::{HostCall, HostCallPlan, UnsupportedHostCall, UnsupportedHostCallReason};
use omega_calling_conventions::HostAbiPlan;
use omega_checked_trees::Program;
use omega_checked_trees::machine::Machine;
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{StatementNode, TableCall};
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_target::NativeTarget;

pub(super) fn collect_machine_host_calls(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    for state in program.machine_states(machine) {
        collect_state_host_calls(program, target, host_abi, machine, state, plan)?;
    }

    Ok(())
}

fn collect_state_host_calls(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    state: &State,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    let mut static_values = initial_static_values(program, machine, &mut plan.expressions);

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        match statement {
            StatementNode::Assignment(assignment) => {
                apply_static_assignment(
                    &mut static_values,
                    program,
                    &mut plan.expressions,
                    *assignment,
                );
                continue;
            }
            StatementNode::Call(table_call) => {
                collect_call_host_lowering(
                    program,
                    target,
                    host_abi,
                    machine,
                    state,
                    statement_index,
                    table_call,
                    &static_values,
                    plan,
                )?;
                apply_call_static_effects(&mut static_values, program, table_call);
            }
            _ => {}
        }
    }

    Ok(())
}

fn collect_call_host_lowering(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    call: &TableCall,
    static_values: &StaticValues,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    let Some(platform_name) = platform_call_receiver_type(program, machine, call) else {
        return Ok(());
    };

    let Some((lowering_handle, lowering)) =
        find_platform_call_lowering(host_abi, &platform_name, call)
    else {
        let platform_call = platform_call_name(program, call);
        plan.unsupported_calls.insert(UnsupportedHostCall {
            source_key: state_key(machine, state),
            statement_index,
            platform_call,
            reason: UnsupportedHostCallReason::NoNativeLowering { target },
        });
        return Ok(());
    };

    let operations = host_abi
        .host_operations
        .span(lowering.operations)
        .map(|operations| {
            plan.operations.insert_many(
                operations
                    .iter()
                    .map(|operation| host_operation(operation.key)),
            )
        })
        .unwrap_or_else(HandleSpan::empty);
    let arguments = lower_host_call_arguments(
        program,
        call,
        static_values,
        &mut plan.expressions,
        &mut plan.arguments,
    );
    plan.calls.insert(HostCall {
        source_key: state_key(machine, state),
        statement_index,
        lowering: lowering_handle,
        data: lowering.data,
        operations,
        arguments,
    });
    Ok(())
}

fn state_key(machine: &Machine, state: &State) -> StateKey {
    StateKey {
        machine: machine.symbol,
        state: state.symbol,
        segment_index: 0,
    }
}
