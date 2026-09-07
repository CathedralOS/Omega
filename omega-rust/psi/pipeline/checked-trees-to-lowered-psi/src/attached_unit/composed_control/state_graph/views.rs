//! Descriptor identities follow the acyclic graph, not authored state order.

use super::*;

pub(super) struct ViewBindings {
    pub(super) state_roots: Vec<Vec<usize>>,
    pub(super) derived: Vec<DerivedView>,
}

/// One selected edge establishes this descriptor before its target can use it.
pub(super) struct DerivedView {
    pub(super) state: symbols::SymbolHandle,
    pub(super) statement_ordinal: u32,
    pub(super) target_parameter_index: u32,
    pub(super) source_root: usize,
}

pub(super) fn bindings(
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<ViewBindings, LoweringError> {
    let mut derived = Vec::new();
    let mut incoming = vec![0_usize; plan.states.len()];
    for state in &plan.states {
        for successor in successors(state) {
            let target = plan
                .states
                .iter()
                .position(|state| state.state == successor.target_state)
                .ok_or(LoweringError::Unsupported("Unit graph edge has no target"))?;
            incoming[target] += 1;
        }
    }
    if incoming[0] != 0 || incoming[1..].contains(&0) {
        return unsupported("Unit graph is cyclic or has unreachable states");
    }
    let mut roots = vec![None; plan.states.len()];
    roots[0] = Some((0..plan.states[0].structural_parameters.len()).collect::<Vec<_>>());
    let mut ready = vec![0];
    let mut next = 0;
    while let Some(source) = ready.get(next).copied() {
        next += 1;
        for successor in successors(&plan.states[source]) {
            let target = plan
                .states
                .iter()
                .position(|state| state.state == successor.target_state)
                .ok_or(LoweringError::Unsupported("Unit graph target disappeared"))?;
            let source_roots = roots[source]
                .as_ref()
                .ok_or(LoweringError::Unsupported("Unit graph view roots missing"))?;
            let aliases = successor
                .transfers
                .iter()
                .map(|transfer| {
                    match transfer.source {
                        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                            source_roots.get(index as usize).copied().ok_or(
                                LoweringError::Unsupported("Unit graph view transfer source missing"),
                            )
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { parameter_index, .. } => {
                            let source_root = *source_roots.get(parameter_index as usize).ok_or(
                                LoweringError::Unsupported("Unit graph subslice source missing"),
                            )?;
                            let root = plan.states[0].structural_parameters.len() + derived.len();
                            derived.push(DerivedView {
                                state: plan.states[source].state,
                                statement_ordinal: successor.statement_ordinal,
                                target_parameter_index: transfer.target_parameter_index,
                                source_root,
                            });
                            Ok(root)
                        }
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            if roots[target]
                .as_ref()
                .is_some_and(|previous| previous != &aliases)
            {
                return unsupported("Unit graph join needs structural descriptor rebinding");
            }
            roots[target] = Some(aliases);
            incoming[target] -= 1;
            if incoming[target] == 0 {
                ready.push(target);
            }
        }
    }
    if next != plan.states.len() {
        return unsupported("Unit graph cyclic safety is not retained");
    }
    let state_roots = roots
        .into_iter()
        .map(|roots| {
            roots.ok_or(LoweringError::Unsupported(
                "Unit graph state is unreachable",
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ViewBindings {
        state_roots,
        derived,
    })
}
