//! Pruning of unreachable private machines.

use std::collections::{BTreeMap, BTreeSet};

use optimization_core::{
    AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
};
use optimization_unit::{
    NodeLocation, ProvenanceRewrite, PrunedMachineCustody, PsiOptimizationUnit, PsiRealizationSite,
    PsiRewriteCandidate, UnreachablePrivateMachinesRewrite,
};
use semantic_vocabulary::MachineId;

use crate::rules::control_flow_cleanup::provenance_proven_unreachable_at;
use crate::{
    AnalysisProduct, CallGraphAnalysis, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
};

psi_rule_contract!(
    UnreachablePrivateMachinePruneRule,
    crate::rules::control_flow_cleanup::call_graph_control_flow_cleanup_rule_contract(
        b"omega.psi-rule.unreachable-private-machine-pruning.v1",
        1,
        AnalysisSet::new([AnalysisKind::CallGraph]),
        OptimizationSafetyClass::StructuralIdentity,
    ),
);

psi_rule_impl!(UnreachablePrivateMachinePruneRule, |unit, analyses| {
    required_analysis!(analyses, AnalysisKind::CallGraph => Some(AnalysisProduct::CallGraph(call_graph)));
    let machines = rule_unreachable_private_machine_complement(unit, call_graph);
    if machines.is_empty() {
        return Ok(Vec::new());
    }
    let ordinals = rule_active_source_ordinals(unit);
    let custody = machines
        .iter()
        .map(|machine| PrunedMachineCustody {
            machine: *machine,
            source_ordinal: ordinals[machine],
        })
        .collect::<Vec<_>>();
    let Some(provenance) = rule_pruned_machine_provenance(unit, &machines) else {
        return Ok(Vec::new());
    };
    Ok(vec![
        PsiRewriteCandidate::new_unreachable_private_machines(
            unit.identity,
            Self::contract(),
            // The pre-roster constructor injected an empty affected-blocks list for this
            // rule; the pruned machines travel in the rewrite payload. Keep that value.
            Vec::new(),
            provenance,
            -i64::try_from(machines.len()).unwrap_or(i64::MAX),
            UnreachablePrivateMachinesRewrite { machines: custody },
        )
        .map_err(RuleProposalError::InvalidCandidate)?,
    ])
});

fn rule_active_source_ordinals(unit: &PsiOptimizationUnit) -> BTreeMap<MachineId, u32> {
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    let mut active = unit.functions.iter();
    let mut result = BTreeMap::new();
    let total = unit.functions.len() + unit.pruned_machines.len();
    for ordinal in 0..total {
        let ordinal = u32::try_from(ordinal).expect("function ordinal fits u32");
        if !pruned.contains_key(&ordinal) {
            let function = active
                .next()
                .expect("validated roster has active source member");
            result.insert(function.machine, ordinal);
        }
    }
    result
}

pub(in crate::rules) fn rule_unreachable_private_machine_complement(
    unit: &PsiOptimizationUnit,
    call_graph: &CallGraphAnalysis,
) -> Vec<MachineId> {
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut reachable = BTreeSet::from([unit.entry]);
    reachable.extend(
        unit.dynamic_dispatch
            .tables
            .iter()
            .flat_map(|table| table.rows.iter().copied().flatten()),
    );
    reachable.extend(
        unit.provider_candidates
            .iter()
            .map(|candidate| candidate.candidate),
    );
    reachable.extend(
        unit.functions
            .iter()
            .filter(|function| function.attachment.is_some())
            .map(|function| function.machine),
    );
    let mut references = call_graph
        .callees
        .iter()
        .map(|(machine, callees)| (*machine, callees.iter().copied().collect::<BTreeSet<_>>()))
        .collect::<BTreeMap<_, _>>();
    // Native metadata and implementation selections retain executable bodies
    // without invocation. Do not put these edges in recursion/effect analysis.
    for function in &unit.functions {
        let selected = references.entry(function.machine).or_default();
        for node in function.block_nodes() {
            selected.extend(unit.operation_machine_references(&node.operation));
        }
    }
    let mut work = reachable.iter().copied().collect::<Vec<_>>();
    while let Some(machine) = work.pop() {
        for callee in references.get(&machine).into_iter().flatten().copied() {
            if active.contains(&callee) && reachable.insert(callee) {
                work.push(callee);
            }
        }
    }
    active.difference(&reachable).copied().collect()
}

fn rule_pruned_machine_provenance(
    unit: &PsiOptimizationUnit,
    machines: &[MachineId],
) -> Option<Vec<ProvenanceRewrite>> {
    let machines = machines.iter().copied().collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for function in unit
        .functions
        .iter()
        .filter(|function| machines.contains(&function.machine))
    {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let input = PsiRealizationSite::Node(NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                });
                if !node.provenance.is_empty() {
                    rows.push(provenance_proven_unreachable_at(
                        input,
                        input,
                        &node.provenance,
                        &node.fuel,
                    ));
                }
                for edge in &node.successors {
                    let input = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    if !edge.provenance.is_empty() {
                        rows.push(provenance_proven_unreachable_at(
                            input,
                            input,
                            &edge.provenance,
                            &edge.fuel,
                        ));
                    }
                }
            }
        }
    }
    rows.sort_by_key(ProvenanceRewrite::canonical_sort_key);
    Some(rows)
}
