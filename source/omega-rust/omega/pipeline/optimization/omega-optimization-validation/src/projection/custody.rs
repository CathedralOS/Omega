//! Exact source occurrence and logical-fuel custody replay.

use super::*;

fn source_occurrence_map(
    unit: &PsiOptimizationUnit,
) -> Option<BTreeMap<(psi_core::MachineId, PsiProvenance), BTreeMap<PsiRealizationSite, u64>>> {
    let mut result = BTreeMap::<_, BTreeMap<_, _>>::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let node_site = PsiRealizationSite::Node(omega_optimization_unit::NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                });
                for settlement in &node.fuel {
                    if result
                        .entry((function.machine, settlement.site))
                        .or_default()
                        .insert(node_site, settlement.units)
                        .is_some()
                    {
                        return None;
                    }
                }
                for edge in &node.successors {
                    let edge_site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    for settlement in &edge.fuel {
                        if result
                            .entry((function.machine, settlement.site))
                            .or_default()
                            .insert(edge_site, settlement.units)
                            .is_some()
                        {
                            return None;
                        }
                    }
                }
            }
        }
    }
    Some(result)
}

pub(super) fn validate_source_custody(
    initial: &PsiOptimizationUnit,
    final_unit: &PsiOptimizationUnit,
    ledger: &PsiTransformationLedger,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    let mut ledger_pruned = ledger
        .records()
        .iter()
        .flat_map(|record| record.pruned_machines.iter().copied())
        .collect::<Vec<_>>();
    ledger_pruned.sort_unstable();
    if ledger_pruned != final_unit.pruned_machines {
        return Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch);
    }
    let mut current = source_occurrence_map(initial)
        .ok_or(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)?;
    let final_sources = source_occurrence_map(final_unit)
        .ok_or(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)?;
    for record in ledger.records() {
        let mut by_input = BTreeMap::<PsiRealizationSite, Vec<_>>::new();
        for row in &record.provenance {
            by_input.entry(row.input).or_default().push(row);
        }
        for (input_site, rows) in &by_input {
            let machine = input_site.machine();
            let expected = rows[0]
                .fuel
                .iter()
                .map(|settlement| (settlement.site, settlement.units))
                .collect::<BTreeMap<_, _>>();
            if rows.iter().any(|row| {
                row.input != *input_site
                    || row.sources.iter().copied().collect::<BTreeSet<_>>()
                        != expected.keys().copied().collect()
                    || row
                        .fuel
                        .iter()
                        .map(|settlement| (settlement.site, settlement.units))
                        .collect::<BTreeMap<_, _>>()
                        != expected
            }) || expected.iter().any(|(source, units)| {
                current
                    .get(&(machine, *source))
                    .and_then(|occurrences| occurrences.get(input_site))
                    != Some(units)
            }) {
                return Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch);
            }
            for source in expected.keys() {
                let occurrences = current
                    .get_mut(&(machine, *source))
                    .expect("input occurrence was checked");
                occurrences.remove(input_site);
                if occurrences.is_empty() {
                    current.remove(&(machine, *source));
                }
            }
        }
        for row in &record.provenance {
            let ProvenanceDisposition::RealizedAt(output_site) = row.disposition else {
                continue;
            };
            let machine = output_site.machine();
            for settlement in &row.fuel {
                if current
                    .entry((machine, settlement.site))
                    .or_default()
                    .insert(output_site, settlement.units)
                    .is_some()
                {
                    return Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch);
                }
            }
        }
    }
    if current != final_sources {
        return Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch);
    }
    Ok(())
}
