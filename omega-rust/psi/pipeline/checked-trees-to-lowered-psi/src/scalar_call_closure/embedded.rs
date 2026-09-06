//! Scalar helper catalogs embedded beneath an existing source-owned root.

use super::*;
use checked_trees::CheckedScalarComputationKind;

mod discovery;
pub(crate) use discovery::{
    checked_scalar_call_closure, checked_scalar_call_closure_with_structural_roots,
};

pub(crate) struct EmbeddedScalarCalls {
    pub(crate) machine_ids: Vec<(symbols::SymbolHandle, MachineId)>,
    pub(crate) requirement_counts: Vec<(symbols::SymbolHandle, usize)>,
    pub(crate) next_call_obligation: u64,
    prepared: Vec<PreparedScalarMachine>,
}

impl EmbeddedScalarCalls {
    pub(crate) fn from_shared(
        machine_ids: Vec<(symbols::SymbolHandle, MachineId)>,
        requirement_counts: Vec<(symbols::SymbolHandle, usize)>,
        next_call_obligation: u64,
    ) -> Self {
        Self {
            machine_ids,
            requirement_counts,
            next_call_obligation,
            prepared: Vec::new(),
        }
    }

    pub(crate) fn emission_context(&self) -> CallEmissionContext<'_> {
        CallEmissionContext {
            machine_ids: &self.machine_ids,
            requirement_counts: &self.requirement_counts,
            next_obligation_identity: self.next_call_obligation,
            obligation_limit: u64::MAX,
        }
    }

    /// Dynamic dispatch selects its realization prefix after the leaf catalog.
    /// Reserve that complete prefix before emitting any helper call identity.
    pub(crate) fn reserve_machine_prefix(&mut self, count: usize) -> Result<(), LoweringError> {
        for (index, (_, identity)) in self.machine_ids.iter_mut().enumerate() {
            let index = count.checked_add(index).ok_or(LoweringError::Unsupported(
                "embedded scalar helper machine count overflows usize",
            ))?;
            *identity = machine_id(dense_identity(index)?);
        }
        Ok(())
    }

    pub(crate) fn append_to(self, lowered: &mut LoweredPsi) -> Result<(), LoweringError> {
        for machine in self.prepared {
            let terminal_machine = lookup_machine_id(&self.machine_ids, machine.source_machine)?;
            if lowered
                .semantic_module
                .machines
                .iter()
                .any(|existing| existing.id == terminal_machine)
            {
                return unsupported("embedded scalar helper identity overlaps an existing machine");
            }
            let identity_base = (terminal_machine.get() - 1)
                .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
                .ok_or(LoweringError::Unsupported(
                    "embedded scalar helper identity range overflows",
                ))?;
            let mut helper = build_scalar_graph_module(
                &machine.states,
                machine.result_type,
                machine.contract,
                machine.crash_routes,
                machine.identity_reshuffles,
                machine.partition_compositions,
                terminal_machine,
                identity_base,
                &self.machine_ids,
                &self.requirement_counts,
            )?;
            lowered
                .semantic_module
                .machines
                .append(&mut helper.semantic_module.machines);
            lowered
                .proof_bundle
                .evidence
                .append(&mut helper.proof_bundle.evidence);
            lowered
                .source_call_occurrences
                .append(&mut helper.source_call_occurrences);
            lowered
                .selected_ieee_float_fma_occurrences
                .append(&mut helper.selected_ieee_float_fma_occurrences);
        }
        Ok(())
    }
}

impl EmbeddedScalarCalls {
    /// The caller first validates exact operation/source custody. Only those
    /// retained argument roots participate in helper selection.
    pub(crate) fn prepare_computations(
        checked: &CheckedTrees,
        roots: &[checked_trees::CheckedScalarComputationHandle],
        excluded_sources: &[symbols::SymbolHandle],
        reserved_prefix: usize,
    ) -> Result<Self, LoweringError> {
        let targets = computation_targets(checked, roots)?;
        Self::prepare_targets(checked, &targets, excluded_sources, reserved_prefix)
    }

    pub(crate) fn prepare_targets(
        checked: &CheckedTrees,
        roots: &[symbols::SymbolHandle],
        excluded_sources: &[symbols::SymbolHandle],
        reserved_prefix: usize,
    ) -> Result<Self, LoweringError> {
        let closure = checked_scalar_call_closure(checked, roots)?;
        if closure
            .iter()
            .any(|symbol| excluded_sources.contains(symbol))
        {
            return unsupported("embedded scalar helper overlaps an excluded source owner");
        }
        let prepared = closure
        .iter()
        .map(|symbol| {
            let graph = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(*symbol)
                .ok_or(LoweringError::Unsupported(
                    "embedded scalar helper has no checked graph",
                ))?;
            let prepared = if roots.contains(symbol) {
                prepare_embedded_scalar_graph_machine(checked, *symbol, graph)?
            } else {
                prepare_scalar_graph_machine(checked, *symbol, graph)?
            };
            if !prepared.identity_reshuffles.structural_places.is_empty()
                || !prepared.identity_reshuffles.entry_claims.is_empty()
                || !prepared.identity_reshuffles.reshuffles.is_empty()
                || !prepared.partition_compositions.structural_places.is_empty()
                || !prepared.partition_compositions.compositions.is_empty()
            {
                return unsupported(
                    "embedded scalar helper structural effects require a dedicated terminal slice",
                );
            }
            Ok(prepared)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
        let requirement_counts = prepared
            .iter()
            .map(|machine| (machine.source_machine, machine.contract.requirement_count()))
            .collect();
        let machine_ids = closure
            .into_iter()
            .map(|symbol| (symbol, machine_id(1)))
            .collect();
        let mut selected = EmbeddedScalarCalls {
            machine_ids,
            requirement_counts,
            prepared,
            next_call_obligation: TERMINAL_UNIT_CALL_OBLIGATION_BASE,
        };
        selected.reserve_machine_prefix(reserved_prefix)?;
        Ok(selected)
    }
}

pub(crate) fn computation_targets(
    checked: &CheckedTrees,
    roots: &[checked_trees::CheckedScalarComputationHandle],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut pending = roots.to_vec();
    let mut visited = Vec::new();
    let mut targets = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        if !plans.nodes.is_valid(handle) {
            return unsupported("embedded scalar computation contains a stale node");
        }
        visited.push(handle);
        match &plans.nodes.get(handle).kind {
            CheckedScalarComputationKind::Value(_) => {}
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
            } => pending.extend([*condition, *when_true, *when_false]),
            CheckedScalarComputationKind::Call {
                target_machine,
                arguments,
                ..
            } => {
                if !targets.contains(target_machine) {
                    targets.push(*target_machine);
                }
                pending.extend(plans.operands.span(*arguments).ok_or(
                    LoweringError::Unsupported("embedded scalar call arguments are stale"),
                )?);
            }
            CheckedScalarComputationKind::Apply { operands, .. } => pending.extend(
                plans
                    .operands
                    .span(*operands)
                    .ok_or(LoweringError::Unsupported(
                        "embedded scalar operands are stale",
                    ))?,
            ),
        }
    }
    Ok(targets)
}
