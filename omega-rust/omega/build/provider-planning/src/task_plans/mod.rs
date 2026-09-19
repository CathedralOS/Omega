//! Task activation plans: elaborated, settled and fingerprinted.
//! This file elaborates and settles the plans and computes their report
//! fingerprints. `specialization_commitments.rs` commits exact task machine
//! specializations, `runtime_requirements.rs` selects the task runtime
//! provider and requirement, `carry_crossings.rs` validates activation
//! carry crossings and translates them into plan values — including each
//! crossing's exact live frontier of places, claims and carry demands,
//! `start_selections.rs` selects task start targets and `stack_graphs.rs`
//! derives the whole-call-graph WCSU demand behind every fixed task stack.

mod carry_crossings;
mod runtime_requirements;
mod specialization_commitments;
mod stack_graphs;
mod start_selections;
#[cfg(test)]
mod tests;

use crate::task_plans::carry_crossings::{
    activation_carry_crossings, canonical_suspension_crossing, carry_obligations,
    exact_activation_wide_carry,
};
use crate::task_plans::runtime_requirements::{
    exact_task_machine_blocking, exact_task_machine_suspension, selected_task_runtime_provider,
};
use crate::task_plans::specialization_commitments::{
    exact_task_machine_contract, task_specialization_commitment,
};
use crate::task_plans::stack_graphs::task_call_graph;
use crate::task_plans::start_selections::{exact_task_activation_target, task_start_selections};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::collections::BTreeMap;
use std::sync::Arc;
use target::{Architecture, NativeTarget, ObjectFormat};
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
    program: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<TaskActivationPlanSet, Vec<Diagnostic>> {
    let selections = task_start_selections(program)?;
    if selections.is_empty() {
        return Ok(TaskActivationPlanSet::default());
    }
    let layouts = layout::build_layout_plan(program, target, opaque_representation_selections)
        .map_err(|error| vec![error])?;
    let mut activations = Vec::new();

    for selection in selections {
        let selected_runtime =
            selected_task_runtime_provider(program, selected_provider_plans, &selection)?;
        let (target_machine, entry) = exact_task_activation_target(
            program,
            selection.target_machine,
            selection.target_entry,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "TaskRuntime start specialization has an invalid retained target coordinate: {}",
                error.message()
            ))]
        })?;
        if !program.machine_type_parameters(target_machine).is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "task activation target `{}` is still generic after specialization",
                target_machine.name
            ))]);
        }

        let contract = exact_task_machine_contract(
            program,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let specialization_commitment = task_specialization_commitment(
            program,
            &selection,
            target_machine,
            entry,
            contract,
            selected_runtime.requirement_identity.as_str(),
        )?;
        let suspension = exact_task_machine_suspension(
            program,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let blocking = exact_task_machine_blocking(
            program,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let machine_contract = normalized_id(
            contract.report_fingerprint,
            MachineContractId::from_normalized_identity,
        )?;
        let entry_report_fingerprint = entry_report_fingerprint(program, target_machine, entry);
        let entry_id = normalized_id(
            entry_report_fingerprint,
            MachineEntryId::from_normalized_identity,
        )?;
        // The argument layout is more than a report coordinate: the
        // marshalling image packs each non-self parameter's concrete
        // size/alignment at its canonical offset, and the retained field
        // table is what the provider boundary checks a presented bundle
        // against.
        let argument_value_layouts = signature_value_layouts(
            program,
            target,
            opaque_representation_selections,
            program
                .state_parameters(entry)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| parameter.type_reference),
        )?;
        let argument_layout_report_fingerprint =
            signature_layout_report_fingerprint(program, &argument_value_layouts);
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
            program,
            target,
            opaque_representation_selections,
            std::iter::once(entry.return_type),
        )?;
        let outcome_layout_report_fingerprint =
            signature_layout_report_fingerprint(program, &outcome_value_layouts);
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
        let crossings = activation_carry_crossings(program, target_machine.symbol)?;
        // The authoritative crossing roster covers the containment subtree
        // and every suspension crossing reached inside the checked call
        // graph, deduplicated by canonical identity.
        let graph = task_call_graph(
            program,
            target,
            opaque_representation_selections,
            &layouts,
            target_machine,
            entry,
        )?;
        let mut roster = BTreeMap::new();
        for crossing in crossings
            .subtree
            .iter()
            .copied()
            .chain(graph.crossings.iter().copied())
        {
            let canonical = canonical_suspension_crossing(program, crossing)?;
            roster.insert(canonical.identity, canonical);
        }
        let canonical_suspension_crossings = roster.into_values().collect::<Vec<_>>();
        let activation_wide_carry = exact_activation_wide_carry(
            program,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let carry_obligations = carry_obligations(activation_wide_carry.effective);
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

/// Derive and commit the complete compiler-owned task-activation sidecar.
/// The checked program and selected providers remain shared read-only inputs;
/// a failed derivation leaves the previously retained sidecar unchanged.
pub fn settle_task_activation_plans(
    retained: &mut Arc<TaskActivationPlanSet>,
    program: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<(), Vec<Diagnostic>> {
    let task_activations = elaborate_task_activation_plans(
        program,
        selected_provider_plans,
        target,
        opaque_representation_selections,
    )?;
    *retained = Arc::new(task_activations);
    Ok(())
}

fn stack_representation_report_fingerprint(target: NativeTarget) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(0x53);
    hash.string("fixed-nonmoving-stack-v1");
    hash.byte(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    hash.byte(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    hash.usize(target.pointer_size);
    hash.usize(target.pointer_alignment);
    hash.finish()
}

/// The concrete per-parameter layouts a start signature marshals under, in
/// source parameter order.
fn signature_value_layouts(
    program: &CheckedTrees,
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
                program,
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
    program: &CheckedTrees,
    layouts: &[(
        checked_trees::types::TypeReferenceHandle,
        layout::TypeLayout,
    )],
) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(0x51);
    for (type_reference, layout) in layouts {
        hash.string(program.normalized_type_identity(*type_reference).as_str());
        hash.usize(layout.size);
        hash.usize(layout.alignment);
        hash.byte(0xff);
    }
    hash.finish()
}

fn entry_report_fingerprint(
    program: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    entry: &checked_trees::state::State,
) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(0x45);
    hash.string(machine.name.as_str());
    hash.string(entry.name.as_str());
    for parameter in program.state_parameters(entry) {
        hash.byte(u8::from(parameter.is_self));
        hash.byte(u8::from(parameter.is_mutable));
        hash.string(
            program
                .normalized_type_identity(parameter.type_reference)
                .as_str(),
        );
    }
    hash.byte(0xfe);
    hash.string(program.normalized_type_identity(entry.return_type).as_str());
    hash.finish()
}

fn calling_plan_report_fingerprint(
    target: NativeTarget,
    entry_report_fingerprint: u64,
    arguments_report_fingerprint: u64,
    outcome_report_fingerprint: u64,
) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    hash.byte(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    hash.usize(target.pointer_size);
    hash.usize(target.pointer_alignment);
    hash.u64(entry_report_fingerprint);
    hash.u64(arguments_report_fingerprint);
    hash.u64(outcome_report_fingerprint);
    hash.finish()
}

fn normalized_id<T>(
    identity: u64,
    constructor: impl FnOnce(u64) -> Result<T, task_plans::TaskPlanDiagnostic>,
) -> Result<T, Vec<Diagnostic>> {
    constructor(identity.max(1)).map_err(|error| vec![Diagnostic::error(error.to_string())])
}

struct StableHash(u64);

impl StableHash {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.byte(*byte);
        }
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
        self.byte(0);
    }

    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0.max(1)
    }
}
