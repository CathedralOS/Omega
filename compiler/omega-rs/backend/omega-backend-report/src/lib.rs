mod codegen;
mod format;
mod host;
mod identity;
mod input;
mod object;
mod proof;
mod runtime_bodies;
mod runtime_branching;
mod runtime_dispatch;
mod runtime_flow;
mod runtime_text;
mod schedule;
mod semantic_spine;
mod source_surface;
mod state_calls;
mod stats;
mod storage;

use omega_backend_report_types::BackendSurfaceReport;
use omega_control_flow::StateKey;
use omega_state_calls::StateCall;
use omega_state_graph::RuntimeTransitionTarget;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};

use crate::host::host_call_display_name;

pub use input::{BackendReportInput, BackendReportPhaseTiming};

pub fn backend_report_text(
    backend_surface: &BackendSurfaceReport,
    backend_plan: &BackendReportInput<'_>,
) -> String {
    let mut output = String::new();

    output.push_str("# Omega Backend Plan\n\n");
    output.push_str(&format!("target: {:?}\n", backend_plan.target));
    output.push_str(&format!(
        "entry: {}.{} as `{}`\n\n",
        backend_plan.entry_machine_name(),
        backend_plan.entry_state_name(),
        omega_object_file::object_entry_symbol_name(backend_plan.object)
    ));

    stats::write_backend_phase_timings(&mut output, backend_plan);
    stats::write_backend_string_storage(&mut output, backend_plan);
    proof::write_checked_semantics_section(&mut output, backend_plan);
    semantic_spine::write_artifact_semantic_spine(&mut output, backend_plan);

    host::write_host_sections(&mut output, backend_plan);
    state_calls::write_state_call_sections(&mut output, backend_plan);
    storage::write_storage_sections(&mut output, backend_plan);

    runtime_text::write_runtime_text_sections(&mut output, backend_plan);

    codegen::write_codegen_sections(&mut output, backend_plan);
    source_surface::write_source_native_surface(&mut output, backend_surface);
    schedule::write_state_schedule(&mut output, backend_plan);
    runtime_flow::write_runtime_flow_sections(&mut output, backend_plan);
    runtime_dispatch::write_runtime_dispatch_sections(&mut output, backend_plan);
    runtime_bodies::write_runtime_bodies_section(&mut output, backend_plan);
    runtime_branching::write_runtime_branching_sections(&mut output, backend_plan);

    object::write_layout_object_sections(&mut output, backend_plan);
    output
}

pub(crate) fn transition_guard_expression_name(
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> String {
    if guard.is_valid() {
        format!("when {}", expressions.display_name(guard))
    } else {
        "always".to_owned()
    }
}

pub(crate) fn runtime_transition_target_name(
    backend_plan: &BackendReportInput<'_>,
    target: &RuntimeTransitionTarget,
) -> String {
    match target {
        RuntimeTransitionTarget::State { key, .. } => backend_state_name(backend_plan, *key),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown => "unknown".to_owned(),
    }
}

pub(crate) fn backend_state_name(backend_plan: &BackendReportInput<'_>, key: StateKey) -> String {
    backend_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}

fn backend_state_call_receiver_name(
    backend_plan: &BackendReportInput<'_>,
    call: &StateCall,
) -> String {
    backend_plan
        .control_flow
        .receiver_name_by_symbol(call.source_key, call.receiver_symbol)
        .or_else(|| {
            backend_plan
                .control_flow
                .call_receiver_name_by_statement(call.source_key, call.statement_index)
        })
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            if call.receiver_symbol.is_valid() {
                "<unknown>".to_owned()
            } else {
                "self".to_owned()
            }
        })
}

pub(crate) fn host_call_name_for_statement<'plan>(
    backend_plan: &'plan BackendReportInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> String {
    backend_plan
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            state_key_matches_statement_source(host_call.source_key, source_key)
                && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call_display_name(backend_plan, host_call))
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}
