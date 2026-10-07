//! Discover all real operation bodies and scalar helpers before allocating identities.
//!
//! A scalar helper can itself own an ordered structural body or graph statements
//! that call Unit bodies, whose calls and providers reveal further helpers.
//! Close those dependencies to a
//! fixed point, then partition owners: operation bodies use the existing ordered
//! emitter exactly once, while the remaining scalar bodies use their own emitter.
//! Classification grants no custody; each body and call is validated by assembly.

use super::bodies::UnitPlans;
use super::{
    CheckedScalarCallee, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitProviderCandidate, LoweringError, ProviderBody, UnitBody,
    checked_scalar_call_closure_with_structural_roots, checked_unit_call_closure_including,
    checked_unit_provider_candidates, reject_recursive_unit_closure, shared_closure, unsupported,
};
pub(super) struct CheckedCallCatalog {
    pub(super) operations: Vec<symbols::SymbolHandle>,
    pub(super) providers: Vec<CheckedUnitProviderCandidate>,
    pub(super) scalars: Vec<symbols::SymbolHandle>,
    pub(super) boundaries: Vec<symbols::SymbolHandle>,
}

pub(super) fn discover(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    entry: symbols::SymbolHandle,
    unit_roots: &[symbols::SymbolHandle],
    external: Option<&shared_closure::ExternalUnitRoots<'_>>,
    scalar_entry: bool,
) -> Result<CheckedCallCatalog, LoweringError> {
    let mut retained_roots = unit_roots.to_vec();
    let mut family_scalar_roots = Vec::new();
    let mut family_boundary_roots = Vec::new();
    // Target families are portable product surface, not entry reachability.
    // Discover every implementation before allocating the shared namespaces;
    // independently lowered modules cannot be merged without changing IDs.
    for member in checked
        .typed
        .target_machine_families
        .iter()
        .filter(|family| family.purpose == typed_trees::machine::MachinePurpose::Runtime)
        .flat_map(|family| &family.members)
    {
        let machine =
            checked
                .typed
                .machine_by_symbol(member.machine)
                .ok_or(LoweringError::Unsupported(
                    "target-family body is absent from checked input",
                ))?;
        if !machine.supplies_checked_body() {
            super::unique_unit_boundary(plans, member.machine).map_err(|error| match error {
                LoweringError::Unsupported(reason) => LoweringError::InvalidUnitMachinePlan {
                    machine: machine.name.as_str().to_owned(),
                    reason,
                    gap: checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .gap_diagnostic(member.machine, &checked.symbols),
                },
                error => error,
            })?;
            family_boundary_roots.push(member.machine);
            continue;
        }
        if UnitBody::contains(plans, member.machine)
            && UnitBody::find(plans, member.machine)?
                .scalar_result_type()
                .is_none()
        {
            if !retained_roots.contains(&member.machine) {
                retained_roots.push(member.machine);
            }
        } else {
            CheckedScalarCallee::find_for_unit_call(checked, member.machine)?;
            if !family_scalar_roots.contains(&member.machine) {
                family_scalar_roots.push(member.machine);
            }
        }
    }
    // Scalar implementations a selection made in the closure can dispatch to.
    let mut dispatched_scalar_roots = Vec::new();
    loop {
        let closure = match retained_roots.split_first() {
            Some((root, additional)) => {
                checked_unit_call_closure_including(checked, plans, *root, additional)?
            }
            None => Vec::new(),
        };
        if external.is_some() && closure.contains(&entry) {
            return unsupported("external composed entry overlaps its ordinary Unit closure");
        }
        let candidates = checked_unit_provider_candidates(checked, plans, &closure)?;
        // This catalog contains checked Omega bodies, not opaque realizations.
        // Their stores and atomic operations keep the same checked access and
        // custody as ordinary calls, and their transitive calls join this
        // closure. A non-observing store is also legal through mutable access
        // or into owned local storage; its operation kind does not imply an
        // opaque write-only boundary. Such boundaries retain their separate
        // source admission and provider-installation evidence requirements.
        let mut new_roots = candidates
            .iter()
            .filter(|candidate| candidate.body == ProviderBody::Callable)
            .map(|candidate| candidate.candidate)
            .filter(|candidate| !closure.contains(candidate))
            .collect::<Vec<_>>();
        let mut ordinary_scalar_roots =
            external.map_or_else(Vec::new, |roots| roots.scalar_roots.to_vec());
        let mut structural_scalar_roots = Vec::new();
        for root in &family_scalar_roots {
            if !ordinary_scalar_roots.contains(root) {
                ordinary_scalar_roots.push(*root);
            }
            if !CheckedScalarCallee::find_for_unit_call(checked, *root)?
                .structural_parameters()
                .is_empty()
            {
                structural_scalar_roots.push(*root);
            }
        }
        // A scalar provider joins the scalar closure as its call would; one
        // taking structural operands needs the structural call custody too.
        for candidate in candidates
            .iter()
            .filter(|candidate| candidate.body == ProviderBody::ScalarCallee)
        {
            if !ordinary_scalar_roots.contains(&candidate.candidate) {
                ordinary_scalar_roots.push(candidate.candidate);
            }
            if !CheckedScalarCallee::find_for_unit_call(checked, candidate.candidate)?
                .structural_parameters()
                .is_empty()
                && !structural_scalar_roots.contains(&candidate.candidate)
            {
                structural_scalar_roots.push(candidate.candidate);
            }
        }
        if scalar_entry {
            ordinary_scalar_roots.push(entry);
            structural_scalar_roots.push(entry);
        }
        for root in &dispatched_scalar_roots {
            if !ordinary_scalar_roots.contains(root) {
                ordinary_scalar_roots.push(*root);
            }
            if !CheckedScalarCallee::find_for_unit_call(checked, *root)?
                .structural_parameters()
                .is_empty()
                && !structural_scalar_roots.contains(root)
            {
                structural_scalar_roots.push(*root);
            }
        }
        for machine_symbol in &closure {
            let computed_structural_roots =
                crate::expression_preparation::computation_graph::structural_call_targets(
                    checked,
                    *machine_symbol,
                )?;
            for target in crate::expression_preparation::computation_graph::call_targets(
                checked,
                *machine_symbol,
            )? {
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
                // An ordinary scalar call roots its scalar callee.
                if let Some(call) = operation
                    .call()
                    .filter(|call| call.is_module() && call.scalar_result().is_some())
                {
                    let target_machine = call.target.machine;
                    if !ordinary_scalar_roots.contains(&target_machine) {
                        ordinary_scalar_roots.push(target_machine);
                    }
                    if (!call.structural_arguments.is_empty() || !call.claim_transfers().is_empty())
                        && !structural_scalar_roots.contains(&target_machine)
                    {
                        structural_scalar_roots.push(target_machine);
                    }
                }
            }
        }
        let scalar_roots = ordinary_scalar_roots;
        let mut scalar_closure = checked_scalar_call_closure_with_structural_roots(
            checked,
            &scalar_roots,
            &structural_scalar_roots,
        )?;

        for source in closure.iter().chain(&scalar_closure) {
            // An expression's dying owner invokes its selected hook even
            // when no ordinary operation calls that hook. Close those
            // dependencies for every body before assigning callable IDs;
            // scalar helpers and Unit callers share the same action roster.
            for cleanup in crate::unit::unit_cleanup::computation_cleanups(checked, *source)? {
                let target = cleanup.cleanup_machine;
                if !closure.contains(&target) && !new_roots.contains(&target) {
                    new_roots.push(target);
                }
            }
            for operation in
                crate::expression_preparation::computation_graph::value_call_operations(
                    checked, *source,
                )?
            {
                if let Some(call) = operation
                    .call()
                    .filter(|call| call.is_module() && call.scalar_result().is_none())
                {
                    let target = call.target.machine;
                    if !closure.contains(&target) && !new_roots.contains(&target) {
                        new_roots.push(target);
                    }
                }
            }
        }
        for source in &scalar_closure {
            if let Some(graph) = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(*source)
            {
                for operation in graph
                    .states
                    .iter()
                    .flat_map(|state| &state.unit_operations)
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                {
                    if matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                    ) {
                        // Store RHS calls are source-checked computation roots
                        // in the scalar closure above. The write adds no callee;
                        // ordered lowering independently validates its destination.
                        continue;
                    }
                    if matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                    ) {
                        // Construction adds storage; its retained call dependencies
                        // are separate entries in this same operation inventory.
                        continue;
                    }
                    // Non-scalar call results belong to ordinary operation bodies.
                    let Some(call) = operation
                        .call()
                        .filter(|call| call.is_module() && call.scalar_result().is_none())
                    else {
                        return unsupported("scalar graph retained an unsupported Unit effect");
                    };
                    let target_machine = call.target.machine;
                    if !closure.contains(&target_machine) && !new_roots.contains(&target_machine) {
                        new_roots.push(target_machine);
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
        // A selection's table rows are reachable only through dispatch, never
        // through a call edge: every implementation a closure body selects
        // joins the closure, so the dispatch catalog can name each row.
        let mut dispatched = false;
        for source in closure.iter().chain(&scalar_closure) {
            for selection in checked
                .facts
                .dynamic_conformances
                .selections
                .iter()
                .filter(|selection| selection.machine == *source)
            {
                for row in &selection.rows {
                    let realization = row.realization_machine;
                    if UnitBody::contains(plans, realization) {
                        if !closure.contains(&realization) && !new_roots.contains(&realization) {
                            new_roots.push(realization);
                        }
                    } else if !scalar_closure.contains(&realization)
                        && !dispatched_scalar_roots.contains(&realization)
                    {
                        dispatched_scalar_roots.push(realization);
                        dispatched = true;
                    }
                }
            }
        }
        if !new_roots.is_empty() || dispatched {
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
            boundaries: family_boundary_roots,
        });
    }
}
