//! Discover all real operation bodies and scalar helpers before allocating identities.
//!
//! A scalar helper can itself own an ordered structural body or graph statements
//! that call Unit bodies, whose calls and providers reveal further helpers.
//! Close those dependencies to a
//! fixed point, then partition owners: operation bodies use the existing ordered
//! emitter exactly once, while the remaining scalar bodies use their own emitter.
//! Classification grants no custody; each body and call is validated by assembly.

use super::*;

pub(super) struct CheckedCallCatalog {
    pub(super) operations: Vec<symbols::SymbolHandle>,
    pub(super) providers: Vec<CheckedUnitProviderCandidate>,
    pub(super) scalars: Vec<symbols::SymbolHandle>,
}

pub(super) fn discover(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
    unit_roots: &[symbols::SymbolHandle],
    external: Option<&shared_closure::ExternalUnitRoots<'_>>,
    scalar_entry: bool,
) -> Result<CheckedCallCatalog, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut retained_roots = unit_roots.to_vec();
    loop {
        let closure = match retained_roots.split_first() {
            Some((root, additional)) => {
                checked_unit_call_closure_including(checked, *root, additional)?
            }
            None => Vec::new(),
        };
        if external.is_some() && closure.contains(&entry) {
            return unsupported("external composed entry overlaps its ordinary Unit closure");
        }
        let candidates = checked_unit_provider_candidates(checked, &closure)?;
        for candidate in candidates
            .iter()
            .filter(|candidate| candidate.body == ProviderBody::Callable)
        {
            if UnitBody::find(plans, candidate.candidate)?
                .operations()
                .any(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                    )
                })
            {
                return unsupported(
                    "write-only stores in opaque provider candidates require a pinned non-observation judgment",
                );
            }
        }

        let mut new_roots = candidates
            .iter()
            .filter(|candidate| candidate.body == ProviderBody::Callable)
            .map(|candidate| candidate.candidate)
            .filter(|candidate| !closure.contains(candidate))
            .collect::<Vec<_>>();
        let mut ordinary_scalar_roots =
            external.map_or_else(Vec::new, |roots| roots.scalar_roots.to_vec());
        let mut selected_scalar_roots = Vec::new();
        let mut structural_scalar_roots = Vec::new();
        if scalar_entry {
            ordinary_scalar_roots.push(entry);
            structural_scalar_roots.push(entry);
        }
        for machine_symbol in &closure {
            let computed_structural_roots =
                crate::scalar_computations::structural_call_targets(checked, *machine_symbol)?;
            for target in crate::scalar_computations::call_targets(checked, *machine_symbol)? {
                if computed_structural_roots.contains(&target) {
                    CheckedScalarCallee::find_for_unit_call(checked, target)?;
                    if !structural_scalar_roots.contains(&target) {
                        structural_scalar_roots.push(target);
                    }
                } else {
                    CheckedScalarCallee::find(checked, target)?;
                }
                if !ordinary_scalar_roots.contains(&target) {
                    ordinary_scalar_roots.push(target);
                }
            }
            for operation in UnitBody::find(plans, *machine_symbol)?.operations() {
                match operation {
                    CheckedUnitEffectOperationPlan::ScalarCall {
                        target_machine,
                        structural_arguments,
                        claim_transfers,
                        ..
                    } => {
                        if !ordinary_scalar_roots.contains(target_machine) {
                            ordinary_scalar_roots.push(*target_machine);
                        }
                        if (!structural_arguments.is_empty() || !claim_transfers.is_empty())
                            && !structural_scalar_roots.contains(target_machine)
                        {
                            structural_scalar_roots.push(*target_machine);
                        }
                    }
                    CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                        realization_machine,
                        ..
                    } if !selected_scalar_roots.contains(realization_machine) => {
                        selected_scalar_roots.push(*realization_machine);
                    }
                    _ => {}
                }
            }
        }
        let scalar_roots = ordinary_scalar_roots
            .iter()
            .chain(&selected_scalar_roots)
            .copied()
            .fold(Vec::new(), |mut roots, root| {
                if !roots.contains(&root) {
                    roots.push(root);
                }
                roots
            });
        let mut scalar_closure = checked_scalar_call_closure_with_structural_roots(
            checked,
            &scalar_roots,
            &structural_scalar_roots,
        )?;

        for source in &scalar_closure {
            if let Some(graph) = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(*source)
            {
                for operation in graph.states.iter().flat_map(|state| &state.unit_operations) {
                    if matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                    ) {
                        // Store RHS calls are source-checked computation roots
                        // in the scalar closure above. The write adds no callee;
                        // ordered lowering independently validates its destination.
                        continue;
                    }
                    if matches!(operation, CheckedUnitEffectOperationPlan::EstablishStructuralValue { calls, .. } if calls.is_empty())
                    {
                        // Scalar field calls already belong to the computation
                        // closure. A fresh record adds storage, not a callee.
                        continue;
                    }
                    let CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } = operation
                    else {
                        return unsupported("scalar graph retained an unsupported Unit effect");
                    };
                    if !closure.contains(target_machine) && !new_roots.contains(target_machine) {
                        new_roots.push(*target_machine);
                    }
                }
            }
            if matches!(
                CheckedScalarCallee::find_for_unit_call(checked, *source)?,
                CheckedScalarCallee::Operations(_)
            ) && !closure.contains(source)
                && !new_roots.contains(source)
            {
                new_roots.push(*source);
            }
        }
        if !new_roots.is_empty() {
            retained_roots.extend(new_roots);
            continue;
        }
        // Every overlapping owner must be the operation body we intentionally
        // routed above. Never hide an ambiguous graph/operation owner by merely
        // deleting duplicate source identities.
        for source in scalar_closure
            .iter()
            .filter(|source| closure.contains(source))
        {
            if !matches!(
                CheckedScalarCallee::find_for_unit_call(checked, *source)?,
                CheckedScalarCallee::Operations(_)
            ) {
                return unsupported(
                    "embedded scalar call overlaps the attached Unit machine closure",
                );
            }
        }
        scalar_closure.retain(|source| !closure.contains(source));
        if external.is_some() && scalar_closure.contains(&entry) {
            return unsupported("embedded scalar call overlaps the attached Unit machine closure");
        }
        reject_recursive_unit_closure(plans, &closure)?;
        return Ok(CheckedCallCatalog {
            operations: closure,
            providers: candidates,
            scalars: scalar_closure,
        });
    }
}
