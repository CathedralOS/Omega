use crate::native::abi::PlatformCallData;
use crate::native::control_flow::{OperationKind, StateFlow};
use crate::native::host_calls::{HostCall, HostCallArgumentKind};
use crate::native::plan::NativePlan;
use crate::native::platform_object::can_emit_target_object;
use crate::native::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::native::runtime_dispatch::branching::{
    RuntimeBranchCallExpansion, RuntimeBranchingCall,
};
use crate::native::runtime_dispatch::loop_plan::RuntimeDispatchLoopAction;
use crate::native::runtime_flow::RuntimeTransitionTarget;
use crate::native::runtime_text::{
    RuntimeTextSource, RuntimeTextUse, RuntimeTextWrite, RuntimeTextWriteKind,
};
use crate::native::state_calls::StateCallLowering;
use crate::native::state_guards::{StateGuardLowering, StateGuardOperator};
use crate::native::state_schedule::{build_entry_state_schedule, scheduled_state_contains};
use crate::native::state_storage::StateMutationLowering;
use crate::native::state_values::{StateValueKind, StateValueRole};
use crate::native::target::ObjectFormat;
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionPlan {
    pub object_format: ObjectFormat,
    pub entry_symbol: String,
    pub sections: usize,
    pub symbols: usize,
    pub host_bindings: usize,
    pub host_calls: usize,
    pub data_bytes: usize,
    pub selected_instructions: usize,
    pub instruction_operands: usize,
    pub machine_code_bytes: usize,
    pub encoded_machine_bytes: usize,
    pub relocations: usize,
    pub blockers: Arena<EmissionBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmissionBlocker {
    pub stage: String,
    pub reason: String,
}

pub fn build_emission_plan(native_plan: &NativePlan) -> EmissionPlan {
    let mut blockers = Arena::new();
    let (state_schedule, needs_runtime_dispatch) = match build_entry_state_schedule(native_plan) {
        Ok(state_schedule) => (state_schedule, false),
        Err(reason) => {
            if native_plan.runtime_dispatch_loop.needed {
                if !runtime_dispatch_loop_can_emit(native_plan) {
                    blockers.insert(runtime_dispatch_loop_blocker(native_plan));
                }
            } else {
                blockers.insert(blocker("state schedule", &reason));
                collect_runtime_dispatch_blockers(native_plan, &mut blockers);
            }
            (runtime_and_required_states(native_plan), true)
        }
    };

    if native_plan.machine_code.bytes.len() < native_plan.machine_code.byte_count {
        blockers.insert(blocker(
            "machine encoding",
            "not all selected native instructions are encoded into target bytes yet",
        ));
    }

    for (_, unsupported_call) in native_plan.host_calls.unsupported_calls.iter() {
        if !scheduled_state_contains(
            &state_schedule,
            &unsupported_call.machine,
            &unsupported_call.state,
        ) {
            continue;
        }

        blockers.insert(blocker(
            "host lowering",
            &format!(
                "{}.{} statement {} platform call `{}`: {}",
                unsupported_call.machine,
                unsupported_call.state,
                unsupported_call.statement_index,
                unsupported_call.platform_call,
                unsupported_call.reason
            ),
        ));
    }

    collect_host_argument_blockers(native_plan, &state_schedule, &mut blockers);
    collect_state_call_blockers(
        native_plan,
        &state_schedule,
        needs_runtime_dispatch,
        &mut blockers,
    );
    collect_state_storage_blockers(native_plan, needs_runtime_dispatch, &mut blockers);
    if needs_runtime_dispatch {
        collect_state_guard_blockers(native_plan, &mut blockers);
        collect_state_value_blockers(native_plan, &mut blockers);
    }
    collect_state_codegen_blockers(native_plan, &state_schedule, &mut blockers);

    if !can_emit_real_object(native_plan) {
        blockers.insert_many([
            blocker(
                "relocation encoding",
                "planned relocation records are not serialized into this target object format yet",
            ),
            blocker(
                "object writer",
                "this target still falls back to the Omega native object container",
            ),
        ]);
    }

    EmissionPlan {
        object_format: native_plan.target.object_format,
        entry_symbol: native_plan.object.entry_symbol.clone(),
        sections: native_plan.object.sections.len(),
        symbols: native_plan.object.symbols.len(),
        host_bindings: native_plan.host_abi.bindings.len(),
        host_calls: native_plan.host_calls.calls.len(),
        data_bytes: native_plan.data.bytes.len(),
        selected_instructions: native_plan.instructions.instructions.len(),
        instruction_operands: native_plan.instructions.operands.len(),
        machine_code_bytes: native_plan.machine_code.byte_count,
        encoded_machine_bytes: native_plan.machine_code.bytes.len(),
        relocations: native_plan.relocations.records.len(),
        blockers,
    }
}

fn collect_host_argument_blockers(
    native_plan: &NativePlan,
    state_schedule: &[crate::native::state_schedule::ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, host_call) in native_plan.host_calls.calls.iter() {
        if !scheduled_state_contains(state_schedule, &host_call.machine, &host_call.state) {
            continue;
        }

        let PlatformCallData::FirstTextArgument { .. } = host_call.data else {
            continue;
        };
        let Some(arguments) = native_plan.host_calls.arguments.span(host_call.arguments) else {
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{}.{} statement {} has an invalid argument span",
                    host_call.machine, host_call.state, host_call.statement_index
                ),
            ));
            continue;
        };
        let Some(first_argument) = arguments.first() else {
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{}.{} statement {} needs a text argument",
                    host_call.machine, host_call.state, host_call.statement_index
                ),
            ));
            continue;
        };

        if let HostCallArgumentKind::Expression(expression) = &first_argument.kind {
            let runtime_text_use = runtime_text_use_for_host_call(native_plan, host_call);
            if runtime_text_use
                .is_some_and(|text_use| runtime_text_use_has_input_buffer(native_plan, text_use))
            {
                continue;
            }
            blockers.insert(blocker(
                "host arguments",
                &runtime_text_use
                    .map(host_text_argument_blocker_reason)
                    .unwrap_or_else(|| {
                        format!(
                            "{}.{} statement {} text argument `{}` needs runtime text lowering",
                            host_call.machine,
                            host_call.state,
                            host_call.statement_index,
                            expression.display_name()
                        )
                    }),
            ));
        }
    }
}

fn runtime_text_use_for_host_call<'plan>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan RuntimeTextUse> {
    native_plan
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| {
            text_use.machine == host_call.machine
                && text_use.state == host_call.state
                && text_use.statement_index == host_call.statement_index
                && text_use.platform_call == host_call.platform_call
        })
        .map(|(_, text_use)| text_use)
}

fn runtime_text_use_has_input_buffer(native_plan: &NativePlan, text_use: &RuntimeTextUse) -> bool {
    native_plan.runtime_text.slots.iter().any(|(_, slot)| {
        slot.place.display_name() == text_use.expression.display_name() && slot.has_input_buffer
    })
}

fn host_text_argument_blocker_reason(text_use: &RuntimeTextUse) -> String {
    let lowering_need = match text_use.source {
        RuntimeTextSource::StoredPlace => "runtime string storage lowering",
        RuntimeTextSource::GeneratedString => "runtime string builder lowering",
        RuntimeTextSource::MutablePlace => "runtime mutable string place lowering",
        RuntimeTextSource::OtherExpression => "runtime string expression lowering",
    };

    format!(
        "{}.{} statement {} text argument `{}` needs {lowering_need}",
        text_use.machine,
        text_use.state,
        text_use.statement_index,
        text_use.expression.display_name()
    )
}

fn collect_state_call_blockers(
    native_plan: &NativePlan,
    state_schedule: &[crate::native::state_schedule::ScheduledState],
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if needs_runtime_dispatch {
        collect_runtime_body_state_call_blockers(native_plan, blockers);
        collect_unresolved_state_call_blockers(native_plan, blockers);
        return;
    }

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        if state_call.target_machine.is_empty() {
            blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls unresolved state `{}` through `{}`",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_state,
                    state_call.receiver
                ),
            ));
            continue;
        }

        if matches!(
            state_call.lowering,
            StateCallLowering::InlineLeaf
                | StateCallLowering::InlineBranching
                | StateCallLowering::InlineExpansion
        ) && !needs_runtime_dispatch
            && scheduled_state_contains(
                state_schedule,
                &state_call.source_machine,
                &state_call.source_state,
            )
            && scheduled_state_contains(
                state_schedule,
                &state_call.target_machine,
                &state_call.target_state,
            )
        {
            continue;
        }

        match state_call.lowering {
            StateCallLowering::InlineLeaf => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls leaf state {}.{} with {} argument(s); native emission needs leaf state-call inlining",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_machine,
                    state_call.target_state,
                    state_call.argument_count
                ),
            )),
            StateCallLowering::InlineBranching => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls branching state {}.{} with {} argument(s); native emission needs guarded state-call expansion",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_machine,
                    state_call.target_state,
                    state_call.argument_count
                ),
            )),
            StateCallLowering::InlineExpansion => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls {}.{} with {} argument(s); native emission needs inline state-call expansion",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_machine,
                    state_call.target_state,
                    state_call.argument_count
                ),
            )),
            StateCallLowering::Unresolved => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls unresolved state `{}` through `{}`",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_state,
                    state_call.receiver
                ),
            )),
        };
    }
}

fn collect_runtime_body_state_call_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let mut grouped_blockers = Vec::<RuntimeBodyStateCallBlocker>::new();

    for (_, body) in native_plan.runtime_bodies.bodies.iter() {
        let Some(operations) = native_plan.runtime_bodies.operations.span(body.operations) else {
            blockers.insert(blocker(
                "runtime bodies",
                &format!(
                    "#{} {}.{} has an invalid runtime body operation span",
                    body.dispatch_index, body.machine, body.state
                ),
            ));
            continue;
        };

        for operation in operations {
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_machine,
                target_state,
                argument_count,
                lowering,
            } = &operation.kind
            else {
                continue;
            };

            push_runtime_body_state_call_blocker(
                &mut grouped_blockers,
                RuntimeBodyStateCallBlocker {
                    dispatch_index: body.dispatch_index,
                    source_machine: operation.source_machine.clone(),
                    source_state: operation.source_state.clone(),
                    first_statement_index: operation.statement_index,
                    target_machine: target_machine.clone(),
                    target_state: target_state.clone(),
                    argument_count: *argument_count,
                    lowering: *lowering,
                    count: 1,
                },
            );
        }
    }

    for grouped_blocker in grouped_blockers {
        if runtime_body_state_call_has_planned_expansion(native_plan, &grouped_blocker) {
            continue;
        }

        let expansion_reason =
            runtime_body_state_call_expansion_reason(native_plan, &grouped_blocker);
        blockers.insert(blocker(
            "state calls",
            &format!(
                "#{} {}.{} statement {} calls {}.{} with {} argument(s){}; runtime dispatch body needs {expansion_reason}",
                grouped_blocker.dispatch_index,
                grouped_blocker.source_machine,
                grouped_blocker.source_state,
                grouped_blocker.first_statement_index,
                grouped_blocker.target_machine,
                grouped_blocker.target_state,
                grouped_blocker.argument_count,
                repeated_count_suffix(grouped_blocker.count),
            ),
        ));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeBodyStateCallBlocker {
    dispatch_index: u32,
    source_machine: String,
    source_state: String,
    first_statement_index: usize,
    target_machine: String,
    target_state: String,
    argument_count: usize,
    lowering: StateCallLowering,
    count: usize,
}

fn push_runtime_body_state_call_blocker(
    grouped_blockers: &mut Vec<RuntimeBodyStateCallBlocker>,
    blocker: RuntimeBodyStateCallBlocker,
) {
    if let Some(existing) = grouped_blockers.iter_mut().find(|existing| {
        existing.dispatch_index == blocker.dispatch_index
            && existing.source_machine == blocker.source_machine
            && existing.source_state == blocker.source_state
            && existing.target_machine == blocker.target_machine
            && existing.target_state == blocker.target_state
            && existing.argument_count == blocker.argument_count
            && existing.lowering == blocker.lowering
    }) {
        existing.count += 1;
        return;
    }

    grouped_blockers.push(blocker);
}

fn runtime_body_state_call_has_planned_expansion(
    native_plan: &NativePlan,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> bool {
    if grouped_blocker.lowering != StateCallLowering::InlineBranching {
        return false;
    }

    let mut matching_calls = native_plan
        .runtime_branching_calls
        .calls
        .iter()
        .filter_map(|(_, call)| {
            runtime_branching_call_matches_grouped_blocker(call, grouped_blocker).then_some(call)
        })
        .peekable();

    if matching_calls.peek().is_none() {
        return false;
    }

    matching_calls.all(|call| runtime_branching_call_has_planned_expansion(native_plan, call))
}

fn runtime_branching_call_has_planned_expansion(
    native_plan: &NativePlan,
    call: &RuntimeBranchingCall,
) -> bool {
    match call.expansion {
        RuntimeBranchCallExpansion::GuardedLeaf => {
            runtime_branching_call_leaf_expansion_count(native_plan, call) > 0
        }
        RuntimeBranchCallExpansion::NeedsStraightLineTarget => {
            runtime_branching_call_leaf_expansion_count(native_plan, call) > 0
                && runtime_branching_call_straight_line_expansion_count(native_plan, call) > 0
        }
        RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
        | RuntimeBranchCallExpansion::NeedsNestedBranchTarget
        | RuntimeBranchCallExpansion::UnknownTarget
        | RuntimeBranchCallExpansion::Unplanned => false,
    }
}

fn runtime_branching_call_leaf_expansion_count(
    native_plan: &NativePlan,
    call: &RuntimeBranchingCall,
) -> usize {
    native_plan
        .runtime_branching_calls
        .leaf_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == call.dispatch_index
                && expansion.source_machine == call.source_machine
                && expansion.source_state == call.source_state
                && expansion.statement_index == call.statement_index
                && expansion.branch_machine == call.target_machine
                && expansion.branch_state == call.target_state
        })
        .count()
}

fn runtime_branching_call_straight_line_expansion_count(
    native_plan: &NativePlan,
    call: &RuntimeBranchingCall,
) -> usize {
    native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == call.dispatch_index
                && expansion.source_machine == call.source_machine
                && expansion.source_state == call.source_state
                && expansion.statement_index == call.statement_index
                && expansion.branch_machine == call.target_machine
                && expansion.branch_state == call.target_state
        })
        .count()
}

fn repeated_count_suffix(count: usize) -> String {
    if count <= 1 {
        String::new()
    } else {
        format!(" ({count} sites)")
    }
}

fn runtime_body_state_call_expansion_reason(
    native_plan: &NativePlan,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> String {
    match grouped_blocker.lowering {
        StateCallLowering::InlineLeaf => "leaf state-call expansion".to_owned(),
        StateCallLowering::InlineExpansion => "straight-line state-call expansion".to_owned(),
        StateCallLowering::Unresolved => "unresolved state-call expansion".to_owned(),
        StateCallLowering::InlineBranching => {
            runtime_branching_call_expansion_reason(native_plan, grouped_blocker)
        }
    }
}

fn runtime_branching_call_expansion_reason(
    native_plan: &NativePlan,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> String {
    let mut matching_calls = native_plan
        .runtime_branching_calls
        .calls
        .iter()
        .filter_map(|(_, call)| {
            runtime_branching_call_matches_grouped_blocker(call, grouped_blocker).then_some(call)
        })
        .peekable();

    if matching_calls.peek().is_none() {
        return "guarded state-call expansion".to_owned();
    }

    let mut expansion = RuntimeBranchCallExpansion::GuardedLeaf;

    for call in matching_calls {
        expansion = strongest_branch_expansion(expansion, call.expansion);
    }

    runtime_branch_expansion_reason(expansion).to_owned()
}

fn runtime_branching_call_matches_grouped_blocker(
    call: &RuntimeBranchingCall,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> bool {
    call.dispatch_index == grouped_blocker.dispatch_index
        && call.source_machine == grouped_blocker.source_machine
        && call.source_state == grouped_blocker.source_state
        && call.target_machine == grouped_blocker.target_machine
        && call.target_state == grouped_blocker.target_state
        && call.argument_count == grouped_blocker.argument_count
}

fn strongest_branch_expansion(
    current: RuntimeBranchCallExpansion,
    next: RuntimeBranchCallExpansion,
) -> RuntimeBranchCallExpansion {
    if branch_expansion_rank(next) > branch_expansion_rank(current) {
        next
    } else {
        current
    }
}

fn branch_expansion_rank(expansion: RuntimeBranchCallExpansion) -> u8 {
    match expansion {
        RuntimeBranchCallExpansion::GuardedLeaf => 0,
        RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards => 1,
        RuntimeBranchCallExpansion::NeedsStraightLineTarget => 2,
        RuntimeBranchCallExpansion::NeedsNestedBranchTarget => 3,
        RuntimeBranchCallExpansion::UnknownTarget => 4,
        RuntimeBranchCallExpansion::Unplanned => 5,
    }
}

fn runtime_branch_expansion_reason(expansion: RuntimeBranchCallExpansion) -> &'static str {
    match expansion {
        RuntimeBranchCallExpansion::GuardedLeaf => "guarded leaf branch expansion",
        RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards => {
            "guarded leaf branch expansion with complex guards"
        }
        RuntimeBranchCallExpansion::NeedsStraightLineTarget => {
            "guarded branch expansion with straight-line target"
        }
        RuntimeBranchCallExpansion::NeedsNestedBranchTarget => "nested guarded branch expansion",
        RuntimeBranchCallExpansion::UnknownTarget => {
            "guarded branch expansion with unknown target lowering"
        }
        RuntimeBranchCallExpansion::Unplanned => "guarded state-call expansion",
    }
}

fn collect_unresolved_state_call_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if !state_call.required || !state_call.target_machine.is_empty() {
            continue;
        }

        blockers.insert(blocker(
            "state calls",
            &format!(
                "{}.{} statement {} calls unresolved state `{}` through `{}`",
                state_call.source_machine,
                state_call.source_state,
                state_call.statement_index,
                state_call.target_state,
                state_call.receiver
            ),
        ));
    }
}

fn collect_state_value_blockers(native_plan: &NativePlan, blockers: &mut Arena<EmissionBlocker>) {
    for (_, value) in native_plan.state_values.values.iter() {
        if !value.required || value.kind != StateValueKind::Binary {
            continue;
        }

        if value.role == StateValueRole::TransitionGuard {
            continue;
        }

        if state_value_is_static_assignment(native_plan, value) {
            continue;
        }

        if state_value_has_planned_text_builder(native_plan, value) {
            continue;
        }

        blockers.insert(blocker(
            "state values",
            &runtime_value_blocker_reason(native_plan, value),
        ));
    }
}

fn state_value_has_planned_text_builder(
    native_plan: &NativePlan,
    value: &crate::native::state_values::StateValueUse,
) -> bool {
    runtime_text_write_for_statement(
        native_plan,
        &value.machine,
        &value.state,
        value.statement_index,
    )
    .is_some_and(|text_write| {
        text_write.kind == RuntimeTextWriteKind::GeneratedString
            && runtime_text_builder_for_write(native_plan, text_write).is_some()
    })
}

fn runtime_value_blocker_reason(
    native_plan: &NativePlan,
    value: &crate::native::state_values::StateValueUse,
) -> String {
    if let Some(text_write) = runtime_text_write_for_statement(
        native_plan,
        &value.machine,
        &value.state,
        value.statement_index,
    ) {
        return format!(
            "{}.{} statement {} text write `{}` = `{}` needs {}",
            text_write.machine,
            text_write.state,
            text_write.statement_index,
            text_write.target.display_name(),
            text_write.value.display_name(),
            runtime_text_write_lowering_name(text_write)
        );
    }

    format!(
        "{}.{} statement {} {:?} binary expression `{}` needs runtime value lowering",
        value.machine,
        value.state,
        value.statement_index,
        value.role,
        value.expression.display_name()
    )
}

fn runtime_text_write_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    machine: &str,
    state: &str,
    statement_index: usize,
) -> Option<&'plan RuntimeTextWrite> {
    native_plan
        .runtime_text
        .writes
        .iter()
        .find(|(_, text_write)| {
            text_write.machine == machine
                && text_write.state == state
                && text_write.statement_index == statement_index
        })
        .map(|(_, text_write)| text_write)
}

fn runtime_text_builder_for_write<'plan>(
    native_plan: &'plan NativePlan,
    text_write: &RuntimeTextWrite,
) -> Option<&'plan crate::native::runtime_text::RuntimeTextBuilder> {
    native_plan
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.machine == text_write.machine
                && builder.state == text_write.state
                && builder.statement_index == text_write.statement_index
                && builder.target.display_name() == text_write.target.display_name()
        })
        .map(|(_, builder)| builder)
}

fn runtime_text_write_lowering_name(text_write: &RuntimeTextWrite) -> &'static str {
    match text_write.kind {
        RuntimeTextWriteKind::StaticText => "runtime text literal storage",
        RuntimeTextWriteKind::StoredCopy => "runtime text copy lowering",
        RuntimeTextWriteKind::GeneratedString => "runtime string builder lowering",
        RuntimeTextWriteKind::OtherExpression => "runtime text expression lowering",
    }
}

fn collect_state_guard_blockers(native_plan: &NativePlan, blockers: &mut Arena<EmissionBlocker>) {
    for (_, guard) in native_plan.state_guards.guards.iter() {
        if matches!(
            guard.lowering,
            StateGuardLowering::NoOp | StateGuardLowering::CompareStaticValue
        ) {
            continue;
        }

        blockers.insert(blocker(
            "state guards",
            &format!(
                "#{} {}.{} edge {} -> #{} {} {:?}/{:?} `{}` needs runtime guard lowering",
                guard.source_dispatch_index,
                guard.source_machine,
                guard.source_state,
                guard.statement_order,
                guard.target_dispatch_index,
                runtime_transition_target_name(&guard.target),
                guard.kind,
                guard.lowering,
                guard.expression.display_name()
            ),
        ));
    }
}

fn state_value_is_static_assignment(
    native_plan: &NativePlan,
    value: &crate::native::state_values::StateValueUse,
) -> bool {
    if value.role != crate::native::state_values::StateValueRole::AssignmentValue {
        return false;
    }
    let Some(state) = state_flow(native_plan, &value.machine, &value.state) else {
        return false;
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return false;
    };

    operations.iter().any(|operation| {
        operation.statement_index == value.statement_index
            && matches!(operation.kind, OperationKind::StaticAssignment { .. })
    })
}

fn collect_state_storage_blockers(
    native_plan: &NativePlan,
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if needs_runtime_dispatch {
        collect_runtime_body_storage_blockers(native_plan, blockers);
        return;
    }

    for (_, local) in native_plan.state_storage.locals.iter() {
        if !local.required {
            continue;
        }

        blockers.insert(blocker(
            "state storage",
            &format!(
                "{}.{} statement {} local `{}`: {} needs stack/local storage lowering",
                local.machine, local.state, local.statement_index, local.name, local.type_name
            ),
        ));
    }

    for (_, mutation) in native_plan.state_storage.mutations.iter() {
        if !mutation.required {
            continue;
        }

        if mutation.lowering == StateMutationLowering::AlreadyLowered {
            continue;
        }

        blockers.insert(blocker(
            "state mutation",
            &format!(
                "{}.{} statement {} {:?}/{:?} `{}` = `{}` needs mutation lowering",
                mutation.machine,
                mutation.state,
                mutation.statement_index,
                mutation.mutation_kind,
                mutation.lowering,
                mutation.target.display_name(),
                mutation.value.display_name()
            ),
        ));
    }
}

fn collect_runtime_body_storage_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, slot) in native_plan.runtime_storage.frame_slots.iter() {
        if slot.byte_size > 0 {
            continue;
        }

        blockers.insert(blocker(
            "state storage",
            &format!(
                "#{} {}.{} statement {} local `{}`: {} needs runtime frame slot layout",
                slot.dispatch_index,
                slot.source_machine,
                slot.source_state,
                slot.statement_index,
                slot.name,
                slot.type_name
            ),
        ));
    }

    for (_, write) in native_plan.runtime_storage.writes.iter() {
        if runtime_storage_write_has_planned_text_write(native_plan, write) {
            continue;
        }

        blockers.insert(blocker(
            "state mutation",
            &format!(
                "#{} {}.{} statement {} {:?}/{:?} `{}` = `{}` needs runtime storage write lowering",
                write.dispatch_index,
                write.source_machine,
                write.source_state,
                write.statement_index,
                write.mutation_kind,
                write.lowering,
                write.target.display_name(),
                write.value.display_name()
            ),
        ));
    }
}

fn runtime_storage_write_has_planned_text_write(
    native_plan: &NativePlan,
    write: &crate::native::runtime_storage::RuntimeStorageWrite,
) -> bool {
    runtime_text_write_for_statement(
        native_plan,
        &write.source_machine,
        &write.source_state,
        write.statement_index,
    )
    .is_some_and(|text_write| {
        text_write.target.display_name() == write.target.display_name()
            && runtime_text_write_is_planned(native_plan, text_write)
    })
}

fn runtime_text_write_is_planned(native_plan: &NativePlan, text_write: &RuntimeTextWrite) -> bool {
    match text_write.kind {
        RuntimeTextWriteKind::StaticText | RuntimeTextWriteKind::StoredCopy => true,
        RuntimeTextWriteKind::GeneratedString => {
            runtime_text_builder_for_write(native_plan, text_write).is_some()
        }
        RuntimeTextWriteKind::OtherExpression => false,
    }
}

fn collect_state_codegen_blockers(
    native_plan: &NativePlan,
    state_schedule: &[crate::native::state_schedule::ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for scheduled_state in state_schedule {
        let Some(state_flow) = state_flow(
            native_plan,
            &scheduled_state.machine,
            &scheduled_state.state,
        ) else {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "scheduled state {}.{} was not present in the control-flow plan",
                    scheduled_state.machine, scheduled_state.state
                ),
            ));
            continue;
        };

        let Some(operations) = native_plan
            .control_flow
            .operations
            .span(state_flow.operations)
        else {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "{}.{} has an invalid operation span",
                    scheduled_state.machine, scheduled_state.state
                ),
            ));
            continue;
        };

        for operation in operations {
            match operation.kind {
                OperationKind::Call { .. }
                    if state_statement_has_host_call(
                        native_plan,
                        &scheduled_state.machine,
                        &scheduled_state.state,
                        operation.statement_index,
                    ) || state_statement_has_state_call(
                        native_plan,
                        &scheduled_state.machine,
                        &scheduled_state.state,
                        operation.statement_index,
                    ) => {}
                OperationKind::Call { .. } => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} is a call that is not lowered to a native host operation",
                            scheduled_state.machine,
                            scheduled_state.state,
                            operation.statement_index
                        ),
                    ));
                }
                OperationKind::ConstantIntegerAssignment
                | OperationKind::StaticAssignment { .. } => {}
                OperationKind::Assignment { .. }
                    if state_statement_has_storage_mutation(
                        native_plan,
                        &scheduled_state.machine,
                        &scheduled_state.state,
                        operation.statement_index,
                    ) => {}
                OperationKind::Assignment { .. } => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} Assignment is not supported by native emission yet",
                            scheduled_state.machine,
                            scheduled_state.state,
                            operation.statement_index
                        ),
                    ));
                }
                OperationKind::LocalData
                    if state_statement_has_local_storage(
                        native_plan,
                        &scheduled_state.machine,
                        &scheduled_state.state,
                        operation.statement_index,
                    ) => {}
                _ => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} {:?} is not supported by native emission yet",
                            scheduled_state.machine,
                            scheduled_state.state,
                            operation.statement_index,
                            operation.kind
                        ),
                    ));
                }
            };
        }
    }
}

fn state_statement_has_local_storage(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan.state_storage.locals.iter().any(|(_, local)| {
        local.machine == machine_name
            && local.state == state_name
            && local.statement_index == statement_index
    })
}

fn state_statement_has_storage_mutation(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan
        .state_storage
        .mutations
        .iter()
        .any(|(_, mutation)| {
            mutation.machine == machine_name
                && mutation.state == state_name
                && mutation.statement_index == statement_index
        })
}

fn state_statement_has_state_call(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan.state_calls.calls.iter().any(|(_, state_call)| {
        state_call.source_machine == machine_name
            && state_call.source_state == state_name
            && state_call.statement_index == statement_index
    })
}

fn runtime_and_required_states(
    native_plan: &NativePlan,
) -> Vec<crate::native::state_schedule::ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in native_plan.runtime_flow.states.iter() {
        push_scheduled_state(&mut states, &state.machine, &state.state);
    }

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if state_call.required {
            push_scheduled_state(
                &mut states,
                &state_call.source_machine,
                &state_call.source_state,
            );

            if !state_call.target_machine.is_empty() {
                push_scheduled_state(
                    &mut states,
                    &state_call.target_machine,
                    &state_call.target_state,
                );
            }
        }
    }

    states
}

fn push_scheduled_state(
    states: &mut Vec<crate::native::state_schedule::ScheduledState>,
    machine: &str,
    state: &str,
) {
    if states
        .iter()
        .any(|scheduled_state| scheduled_state.machine == machine && scheduled_state.state == state)
    {
        return;
    }

    states.push(crate::native::state_schedule::ScheduledState {
        machine: machine.to_owned(),
        state: state.to_owned(),
    });
}

fn collect_runtime_dispatch_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, cycle) in native_plan.runtime_flow.cycles.iter() {
        let Some(states) = native_plan.runtime_flow.cycle_states.span(cycle.states) else {
            blockers.insert(blocker(
                "runtime dispatch",
                "invalid runtime cycle span in native flow plan",
            ));
            continue;
        };
        let cycle_path = states
            .iter()
            .map(|state| format!("{}.{}", state.machine, state.state))
            .collect::<Vec<_>>()
            .join(" -> ");

        blockers.insert(blocker(
            "runtime dispatch",
            &format!("cycle {cycle_path} needs generated state dispatch before native emission"),
        ));
    }
}

fn runtime_dispatch_loop_blocker(native_plan: &NativePlan) -> EmissionBlocker {
    if let Some(guard_lowering) = first_unsupported_dispatch_guard(native_plan) {
        return blocker(
            "runtime dispatch",
            &format!(
                "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); guard lowering {guard_lowering:?} needs runtime state comparison byte emission",
                native_plan.runtime_dispatch_loop.cases.len(),
                native_plan.runtime_dispatch_loop.edges.len(),
                native_plan.runtime_flow.cycles.len()
            ),
        );
    }

    blocker(
        "runtime dispatch",
        &format!(
            "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); native emission needs dispatch loop byte emission",
            native_plan.runtime_dispatch_loop.cases.len(),
            native_plan.runtime_dispatch_loop.edges.len(),
            native_plan.runtime_flow.cycles.len()
        ),
    )
}

fn runtime_dispatch_loop_can_emit(native_plan: &NativePlan) -> bool {
    native_plan
        .runtime_dispatch_loop
        .edges
        .iter()
        .all(|(_, edge)| {
            dispatch_loop_guard_can_emit(edge) && edge.action != RuntimeDispatchLoopAction::Unknown
        })
}

fn first_unsupported_dispatch_guard(native_plan: &NativePlan) -> Option<StateGuardLowering> {
    native_plan
        .runtime_dispatch_loop
        .edges
        .iter()
        .find(|(_, edge)| !dispatch_loop_guard_can_emit(edge))
        .map(|(_, edge)| edge.guard_lowering)
}

fn dispatch_loop_guard_can_emit(
    edge: &crate::native::runtime_dispatch::loop_plan::RuntimeDispatchLoopEdge,
) -> bool {
    match edge.guard_lowering {
        StateGuardLowering::NoOp => true,
        StateGuardLowering::CompareStaticValue => {
            edge.guard_has_storage
                && matches!(
                    edge.guard_operator,
                    StateGuardOperator::Equal | StateGuardOperator::NotEqual
                )
                && matches!(edge.guard_byte_size, 1 | 4)
        }
        StateGuardLowering::CompareRuntimeValue | StateGuardLowering::NeedsRuntimeExpression => {
            false
        }
    }
}

fn runtime_transition_target_name(target: &RuntimeTransitionTarget) -> String {
    match target {
        RuntimeTransitionTarget::State { machine, state } => format!("{machine}.{state}"),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
    }
}

fn state_flow<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Option<&'plan StateFlow> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
}

fn state_statement_has_host_call(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.machine == machine_name
            && host_call.state == state_name
            && host_call.statement_index == statement_index
    })
}

fn can_emit_real_object(native_plan: &NativePlan) -> bool {
    can_emit_target_object(native_plan.target)
        && native_plan.machine_code.bytes.len() == native_plan.machine_code.byte_count
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    EmissionBlocker {
        stage: stage.to_owned(),
        reason: reason.to_owned(),
    }
}
