//! Pruning of unreachable private machines.

use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct UnreachablePrivateMachinePruneRule;

impl UnreachablePrivateMachinePruneRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.unreachable-private-machine-pruning.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::CallGraph]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::CallGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for UnreachablePrivateMachinePruneRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::CallGraph(call_graph)) = analyses.get(AnalysisKind::CallGraph)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::CallGraph));
        };
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
                provenance,
                -i64::try_from(machines.len()).unwrap_or(i64::MAX),
                UnreachablePrivateMachinesRewrite { machines: custody },
            )
            .map_err(RuleProposalError::InvalidCandidate)?,
        ])
    }
}

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

pub(in crate::rules::passes) fn rule_unreachable_private_machine_complement(
    unit: &PsiOptimizationUnit,
    call_graph: &crate::CallGraphAnalysis,
) -> Vec<MachineId> {
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut reachable = BTreeSet::from([unit.entry]);
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
    for function in &unit.functions {
        let function_references = references.entry(function.machine).or_default();
        for operation in function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
        {
            match operation {
                O::Return {
                    cleanup_actions, ..
                }
                | O::ReturnUnit {
                    cleanup_actions, ..
                } => {
                    function_references.extend(cleanup_actions.iter().filter_map(|action| {
                        match action {
                            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                                Some(cleanup.cleanup_machine)
                            }
                            _ => None,
                        }
                    }));
                }
                _ => {}
            }
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
                    rows.push(ProvenanceRewrite {
                        input,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let input = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    if !edge.provenance.is_empty() {
                        rows.push(ProvenanceRewrite {
                            input,
                            disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                            sources: edge.provenance.clone(),
                            fuel: edge.fuel.clone(),
                        });
                    }
                }
            }
        }
    }
    rows.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some(rows)
}
