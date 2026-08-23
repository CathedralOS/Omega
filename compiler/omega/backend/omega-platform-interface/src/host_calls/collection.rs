use super::lowering::{
    call_parameter_expects_address, call_parameter_expects_reference,
    expression_platform_receiver_type, find_platform_call_lowering_by_target,
    lower_host_call_argument,
};
use crate::host_calls::lowering::{
    find_platform_call_lowering, host_operation, lower_host_call_arguments, platform_call_name,
    platform_call_receiver_type,
};
use crate::host_calls::static_values::{
    StaticValues, apply_call_static_effects, apply_static_assignment, initial_static_values,
};
use crate::{HostCall, HostCallPlan, UnsupportedHostCall, UnsupportedHostCallReason};
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::StateKey;
use omega_target::NativeTarget;
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::state::State;
use psi_checked_trees::statement::{StatementNode, TableCall};
use psi_diagnostics::Diagnostic;

pub(super) fn collect_machine_host_calls(
    program: &CheckedTrees,
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
    program: &CheckedTrees,
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
                collect_assignment_result_host_lowering(
                    program,
                    target,
                    host_abi,
                    machine,
                    state,
                    statement_index,
                    assignment,
                    &static_values,
                    plan,
                )?;
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
            StatementNode::LocalData(local_data) => {
                // The local-initializer host-call shape: `let fd = self.fs.open(..)`.
                // Mirror the assignment case, but the RESULT place (argument[0]) is
                // the LOCAL, whose place expression we synthesize into `plan.
                // expressions` (a single-symbol Name). Without this a `let`-bound
                // boundary call is never collected -> dropped in selection (file
                // created, never written) -- the ergonomic-wrapper shape. Non-call
                // initializers fall through unchanged.
                collect_local_result_host_lowering(
                    program,
                    target,
                    host_abi,
                    machine,
                    state,
                    statement_index,
                    local_data,
                    &static_values,
                    plan,
                )?;
            }
            _ => {}
        }
    }

    Ok(())
}

/// The assignment-RHS host-call shape: `self.t = self.clock.tick_count()`.
/// When the assignment VALUE is a call into a boundary trait with a known
/// platform lowering, insert a `HostCall` whose single ARGUMENT is the
/// assignment TARGET place -- the value-returning encoder reads it as the
/// result storage operand. A non-host call value simply falls through (the
/// ordinary mutation machinery, or its clean blocker, handles it).
#[allow(clippy::too_many_arguments)]
fn collect_assignment_result_host_lowering(
    program: &CheckedTrees,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    assignment: &psi_checked_trees::statement::TableAssignment,
    static_values: &StaticValues,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    let psi_checked_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(assignment.value)
    else {
        return Ok(());
    };
    let call = call.clone();
    let Some(platform_name) = expression_platform_receiver_type(program, call.target_symbol) else {
        return Ok(());
    };
    let Some((lowering_handle, lowering)) = find_platform_call_lowering_by_target(
        program,
        host_abi,
        &platform_name,
        &call.target,
        call.target_symbol,
    ) else {
        // A boundary call with no native lowering on this target: record it as
        // unsupported so the report explains the gap.
        plan.unsupported_calls.insert(UnsupportedHostCall {
            source_key: state_key(machine, state),
            statement_index,
            call_ordinal: 0,
            platform_call: format!("{platform_name}.{}", call.target.as_str()),
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
                    .map(|operation| host_operation(host_abi, operation.key)),
            )
        })
        .unwrap_or_else(HandleSpan::empty);

    // The RESULT place travels as argument[0]: the same argument lowering and
    // operand resolution the scalar-argument ops use, read by the encoder as
    // the store destination.
    let mut argument_span = HandleSpan::empty();
    plan.arguments.append_to_span(
        &mut argument_span,
        crate::HostCallArgument {
            kind: lower_host_call_argument(
                program,
                assignment.target,
                static_values,
                &mut plan.expressions,
            ),
            is_borrowed: false,
            expects_reference: false,
            expects_address: false,
        },
    );
    // The call's REAL arguments follow the result (e.g. key_state(vk)).
    for (index, argument) in program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        plan.arguments.append_to_span(
            &mut argument_span,
            crate::HostCallArgument {
                kind: lower_host_call_argument(
                    program,
                    *argument,
                    static_values,
                    &mut plan.expressions,
                ),
                is_borrowed: matches!(
                    program.expression_table.expression(*argument),
                    psi_checked_trees::expression::ExpressionNode::Borrow(_)
                ),
                expects_reference: call_parameter_expects_reference(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    index,
                ),
                expects_address: call_parameter_expects_address(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    index,
                ),
            },
        );
    }

    plan.calls.insert(HostCall {
        source_key: state_key(machine, state),
        statement_index,
        call_ordinal: 0,
        lowering: lowering_handle,
        data: lowering.data,
        operations,
        arguments: argument_span,
        has_result: true,
    });
    Ok(())
}

/// The LOCAL-initializer host-call shape: `let fd = self.fs.open(path, flags)`.
/// Identical to `collect_assignment_result_host_lowering` except the RESULT place
/// (argument[0]) is the LOCAL declared by this `let` -- which, unlike an assignment
/// target, has no place expression in the program tree. We SYNTHESIZE a single-symbol
/// `Name` for it directly into `plan.expressions` (the same table the collected
/// arguments live in, so instruction-selection resolves it to the local's frame slot
/// by symbol). A non-call initializer (`let n = 5`) matches nothing and is a no-op.
#[allow(clippy::too_many_arguments)]
fn collect_local_result_host_lowering(
    program: &CheckedTrees,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    local_data: &psi_checked_trees::statement::TableLocalData,
    static_values: &StaticValues,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    let psi_checked_trees::expression::ExpressionNode::Call(call) = program
        .expression_table
        .expression(local_data.initial_value)
    else {
        if std::env::var_os("OMEGA_DEBUG_HOSTCALL").is_some() {
            eprintln!(
                "HOSTCALL: local `{}` initializer is not a Call node",
                local_data.name.as_str()
            );
        }
        return Ok(());
    };
    let call = call.clone();
    let Some(platform_name) = expression_platform_receiver_type(program, call.target_symbol) else {
        if std::env::var_os("OMEGA_DEBUG_HOSTCALL").is_some() {
            eprintln!(
                "HOSTCALL: local `{}` call target `{}` has no platform receiver type",
                local_data.name.as_str(),
                call.target.as_str()
            );
        }
        return Ok(());
    };
    let Some((lowering_handle, lowering)) = find_platform_call_lowering_by_target(
        program,
        host_abi,
        &platform_name,
        &call.target,
        call.target_symbol,
    ) else {
        plan.unsupported_calls.insert(UnsupportedHostCall {
            source_key: state_key(machine, state),
            statement_index,
            call_ordinal: 0,
            platform_call: format!("{platform_name}.{}", call.target.as_str()),
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
                    .map(|operation| host_operation(host_abi, operation.key)),
            )
        })
        .unwrap_or_else(HandleSpan::empty);

    // Synthesize `Name(local)` into plan.expressions as the result place (arg[0]).
    let mut members = HandleSpan::empty();
    plan.expressions
        .push_name_path_member(&mut members, local_data.name.clone());
    let mut member_symbols = HandleSpan::empty();
    plan.expressions
        .push_name_path_member_symbol(&mut member_symbols, local_data.symbol);
    let result_place =
        plan.expressions
            .insert(psi_checked_trees::expression::ExpressionNode::Name(
                psi_checked_trees::expression::TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: local_data.symbol,
                    symbol: local_data.symbol,
                },
            ));

    let mut argument_span = HandleSpan::empty();
    plan.arguments.append_to_span(
        &mut argument_span,
        crate::HostCallArgument {
            kind: crate::HostCallArgumentKind::Expression(result_place),
            is_borrowed: false,
            expects_reference: false,
            expects_address: false,
        },
    );
    for (index, argument) in program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        plan.arguments.append_to_span(
            &mut argument_span,
            crate::HostCallArgument {
                kind: lower_host_call_argument(
                    program,
                    *argument,
                    static_values,
                    &mut plan.expressions,
                ),
                is_borrowed: matches!(
                    program.expression_table.expression(*argument),
                    psi_checked_trees::expression::ExpressionNode::Borrow(_)
                ),
                expects_reference: call_parameter_expects_reference(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    index,
                ),
                expects_address: call_parameter_expects_address(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    index,
                ),
            },
        );
    }

    plan.calls.insert(HostCall {
        source_key: state_key(machine, state),
        statement_index,
        call_ordinal: 0,
        lowering: lowering_handle,
        data: lowering.data,
        operations,
        arguments: argument_span,
        has_result: true,
    });
    Ok(())
}

fn collect_call_host_lowering(
    program: &CheckedTrees,
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
    // Host collection currently lowers statement-level platform calls. Borrow
    // facts are preferred when the call also participates in ordinary semantic
    // dispatch; otherwise the statement-level host call is ordinal 0.
    let call_ordinal =
        call_ordinal_for_statement_call(program, machine, state, statement_index, call)
            .unwrap_or(0);

    let Some((lowering_handle, lowering)) =
        find_platform_call_lowering(program, host_abi, &platform_name, call)
    else {
        let platform_call = platform_call_name(program, call);
        plan.unsupported_calls.insert(UnsupportedHostCall {
            source_key: state_key(machine, state),
            statement_index,
            call_ordinal,
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
                    .map(|operation| host_operation(host_abi, operation.key)),
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
        call_ordinal,
        lowering: lowering_handle,
        data: lowering.data,
        operations,
        arguments,
        has_result: false,
    });
    Ok(())
}

fn call_ordinal_for_statement_call(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    call: &TableCall,
) -> Option<usize> {
    program
        .facts
        .borrow
        .states
        .iter()
        .find_map(|(_, borrow_state)| {
            (borrow_state.machine_symbol == machine.symbol
                && borrow_state.state_symbol == state.symbol)
                .then_some(borrow_state)
        })
        .and_then(|borrow_state| {
            program
                .facts
                .borrow
                .calls
                .span(borrow_state.calls)
                .and_then(|calls| {
                    calls.iter().find_map(|borrow_call| {
                        (borrow_call.statement_index == statement_index
                            && borrow_call.receiver_symbol == call.receiver_symbol
                            && borrow_call.target_symbol == call.target_symbol)
                            .then_some(borrow_call.call_ordinal)
                    })
                })
        })
}

fn state_key(machine: &Machine, state: &State) -> StateKey {
    StateKey {
        machine: machine.symbol,
        state: state.symbol,
        segment_index: 0,
    }
}
