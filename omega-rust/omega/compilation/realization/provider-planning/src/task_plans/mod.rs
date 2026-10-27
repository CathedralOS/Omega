//! Task activation plans: elaborated, settled and fingerprinted.
//! This file elaborates and settles the plans and computes their report
//! fingerprints. `specialization_commitments.rs` commits exact task machine
//! specializations, `runtime_requirements.rs` selects the task runtime
//! provider and requirement, `carry_crossings.rs` validates activation
//! carry crossings and translates them into plan values — including each
//! crossing's exact live frontier of places, claims and carry demands,
//! `start_selections.rs` selects task start targets and `stack_graphs.rs`
//! derives the whole-call-graph WCSU demand behind every fixed task stack.
//! `call_target_bindings.rs` turns a provider's admission-time callee
//! assignment into the `CallTargetBinding` rows `bind_call_targets`
//! consumes, deriving each bound subtree and its canonical crossings through
//! the same call-graph derivation a resolved call would have used.
mod carry_crossings;
mod runtime_requirements;
mod specialization_commitments;
mod stack_graphs;
mod start_selections;

use crate::task_plans::carry_crossings::{canonical_suspension_crossing, carry_obligations};
use crate::task_plans::runtime_requirements::{
    exact_task_machine_blocking, exact_task_machine_suspension, selected_task_runtime_provider,
};
use crate::task_plans::specialization_commitments::{
    exact_task_machine_contract, task_specialization_commitment,
};
use crate::task_plans::stack_graphs::task_call_graph;
use crate::task_plans::start_selections::{exact_task_activation_target, task_start_selections};
use diagnostics::Diagnostic;
use std::collections::BTreeMap;
use target::NativeTarget;
use task_plans::{
    ActivationPlanCandidate, CallingPlanId, MachineContractId, MachineEntryId,
    StackRepresentationId, TaskActivationPlanFact, TaskActivationPlanSet, ValueLayoutId,
    compose_task_stack_demand, project_wcsu_stack_plan, validate_wcsu_activation_plan,
};

/// Elaborate every concrete `TaskRuntime::{start,try_start}<M>` specialization
/// into a provider-independent activation demand. The source classifier is
/// deliberately nominal and closed: an unrelated method named `start` does
/// not become a task operation.
pub fn elaborate_task_activation_plans(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<TaskActivationPlanSet, Vec<Diagnostic>> {
    let selections = task_start_selections(typed)?;
    if selections.is_empty() {
        return Ok(TaskActivationPlanSet::default());
    }
    let mut activations = Vec::new();

    for selection in selections {
        let (selected_runtime, runtime_requirement) =
            selected_task_runtime_provider(typed, selected_provider_plans, &selection)?;
        let (target_machine, entry) = exact_task_activation_target(
            typed,
            selection.target_machine,
            selection.target_entry,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "TaskRuntime start specialization has an invalid retained target coordinate: {}",
                error.message()
            ))]
        })?;
        if !typed.machine_type_parameters(target_machine).is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "task activation target `{}` is still generic after specialization",
                target_machine.name
            ))]);
        }

        let contract = exact_task_machine_contract(
            check_facts,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let specialization_commitment = task_specialization_commitment(
            typed,
            check_facts,
            &selection,
            target_machine,
            entry,
            contract,
            &typed.callable_identity_text(runtime_requirement),
        )?;
        let suspension = exact_task_machine_suspension(
            check_facts,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let blocking = exact_task_machine_blocking(
            check_facts,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let machine_contract = normalized_id(
            contract.report_fingerprint,
            MachineContractId::from_normalized_identity,
        )?;
        let entry_report_fingerprint = entry_report_fingerprint(typed, target_machine, entry);
        let entry_id = normalized_id(
            entry_report_fingerprint,
            MachineEntryId::from_normalized_identity,
        )?;
        // The argument layout is more than a report coordinate: the
        // marshalling image packs each explicit parameter's concrete
        // size/alignment at its canonical offset, and the retained field
        // table is what the provider boundary checks a presented bundle
        // against.
        let argument_value_layouts = signature_value_layouts(
            typed,
            target,
            opaque_representation_selections,
            typed
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.type_reference),
        )?;
        let argument_layout_report_fingerprint =
            signature_layout_report_fingerprint(typed, &argument_value_layouts);
        let argument_layout = task_plans::TaskArgumentLayout::new(
            normalized_id(
                argument_layout_report_fingerprint,
                ValueLayoutId::from_normalized_identity,
            )?,
            &argument_value_layouts
                .iter()
                .map(|(_, layout)| (layout.size as u64, layout.alignment as u64))
                .collect::<Vec<_>>(),
        )
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        let outcome_value_layouts = signature_value_layouts(
            typed,
            target,
            opaque_representation_selections,
            std::iter::once(entry.return_type),
        )?;
        let outcome_layout_report_fingerprint =
            signature_layout_report_fingerprint(typed, &outcome_value_layouts);
        let terminal_outcome_layout = normalized_id(
            outcome_layout_report_fingerprint,
            ValueLayoutId::from_normalized_identity,
        )?;
        let calling_plan_report_fingerprint = calling_plan_report_fingerprint(
            target,
            entry_report_fingerprint,
            argument_layout_report_fingerprint,
            outcome_layout_report_fingerprint,
        );
        let calling_plan = normalized_id(
            calling_plan_report_fingerprint,
            CallingPlanId::from_normalized_identity,
        )?;

        let may_suspend = suspension.checked_may_suspend;
        let may_block = blocking.checked_may_block;
        // Stack and carry obligations follow the same checked call graph.
        let graph = task_call_graph(
            typed,
            check_facts,
            target,
            opaque_representation_selections,
            target_machine,
            entry,
        )?;
        let mut roster = BTreeMap::new();
        for crossing in &graph.crossings {
            let canonical = canonical_suspension_crossing(typed, crossing)?;
            roster.insert(canonical.identity, canonical);
        }
        let canonical_suspension_crossings = roster.into_values().collect::<Vec<_>>();
        let carry_obligations = carry_obligations(graph.carry);
        let stack_representation = normalized_id(
            stack_representation_report_fingerprint(target),
            StackRepresentationId::from_normalized_identity,
        )?;
        let demand = compose_task_stack_demand(graph.root, graph.frames)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        let projection = project_wcsu_stack_plan(&demand, stack_representation);

        let plan = validate_wcsu_activation_plan(
            ActivationPlanCandidate {
                machine_contract,
                entry: entry_id,
                argument_layout,
                terminal_outcome_layout,
                calling_plan,
                stack_plan: projection.stack_plan(),
                may_suspend,
                may_block,
                // Missing or locally unsafe crossings remain visible so the
                // plan validator rejects them fail-closed.
                canonical_suspension_crossings,
                carry_obligations,
                // `Task<T>` always carries cancellation-request authority. A
                // selected provider must establish that operation later.
                cancellation_required: true,
            },
            projection,
        )
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;

        activations.push(TaskActivationPlanFact {
            start_requirement: selection.requirement,
            target_machine: target_machine.symbol,
            target_entry: entry.symbol,
            specialization_report_fingerprint: selection.report_fingerprint,
            specialization_commitment,
            operation: selection.operation,
            selected_runtime,
            plan,
        });
    }

    Ok(TaskActivationPlanSet { activations })
}

fn stack_representation_report_fingerprint(target: NativeTarget) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.byte(0x53);
    hash.string_terminated("fixed-nonmoving-stack-v1");
    hash.bytes(&target.wire_identity_bytes());
    hash.finish_or(1)
}

/// The concrete per-parameter layouts a start signature marshals under, in
/// source parameter order.
fn signature_value_layouts(
    typed: &checked_trees::typed_trees::TypedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    types: impl IntoIterator<Item = checked_trees::types::TypeReferenceHandle>,
) -> Result<
    Vec<(
        checked_trees::types::TypeReferenceHandle,
        layout::TypeLayout,
    )>,
    Vec<Diagnostic>,
> {
    types
        .into_iter()
        .map(|type_reference| {
            layout::layout_type_reference(
                typed,
                target,
                opaque_representation_selections,
                type_reference,
            )
            .map(|layout| (type_reference, layout))
            .map_err(|error| vec![error])
        })
        .collect()
}

fn signature_layout_report_fingerprint(
    typed: &checked_trees::typed_trees::TypedTrees,
    layouts: &[(
        checked_trees::types::TypeReferenceHandle,
        layout::TypeLayout,
    )],
) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.byte(0x51);
    for (type_reference, layout) in layouts {
        hash.string_terminated(&typed.normalized_type_identity_text(*type_reference));
        hash.usize(layout.size);
        hash.usize(layout.alignment);
        hash.byte(0xff);
    }
    hash.finish_or(1)
}

fn entry_report_fingerprint(
    typed: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::machine::Machine,
    entry: &checked_trees::state::State,
) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.byte(0x45);
    hash.string_terminated(machine.name.as_str());
    hash.string_terminated(entry.name.as_str());
    for parameter in typed.state_parameters(entry) {
        hash.byte(u8::from(parameter.is_mutable));
        hash.string_terminated(&typed.normalized_type_identity_text(parameter.type_reference));
    }
    hash.byte(0xfe);
    hash.string_terminated(&typed.normalized_type_identity_text(entry.return_type));
    hash.finish_or(1)
}

fn calling_plan_report_fingerprint(
    target: NativeTarget,
    entry_report_fingerprint: u64,
    arguments_report_fingerprint: u64,
    outcome_report_fingerprint: u64,
) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.bytes(&target.wire_identity_bytes());
    hash.u64(entry_report_fingerprint);
    hash.u64(arguments_report_fingerprint);
    hash.u64(outcome_report_fingerprint);
    hash.finish_or(1)
}

fn normalized_id<T>(
    identity: u64,
    constructor: impl FnOnce(u64) -> Result<T, task_plans::TaskPlanDiagnostic>,
) -> Result<T, Vec<Diagnostic>> {
    constructor(identity.max(1)).map_err(|error| vec![Diagnostic::error(error.to_string())])
}
