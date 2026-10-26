//! Discover all real operation bodies, scalar helpers included, before
//! allocating identities.
//!
//! A scalar helper is a Unit body with a scalar completion; its calls and
//! providers reveal further helpers. Close those dependencies to a fixed
//! point: every body then uses the one ordered emitter exactly once.
//! Classification grants no custody; each body and call is validated by assembly.

use super::bodies::UnitPlans;
use super::{
    CheckedScalarCallee, CheckedTrees, CheckedUnitProviderCandidate, LoweringError, UnitBody,
    checked_scalar_call_closure_with_structural_roots, checked_unit_call_closure_including,
    checked_unit_provider_candidates, unsupported,
};
pub(super) struct CheckedCallCatalog {
    pub(super) operations: Vec<symbols::SymbolHandle>,
    pub(super) source_machines: Vec<symbols::SymbolHandle>,
    pub(super) providers: Vec<CheckedUnitProviderCandidate>,
    pub(super) boundaries: Vec<symbols::SymbolHandle>,
}

pub(super) fn discover(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    entry: symbols::SymbolHandle,
    unit_roots: &[symbols::SymbolHandle],
    program_roots: crate::TerminalProgramRoots<'_>,
    scalar_entry: bool,
) -> Result<CheckedCallCatalog, LoweringError> {
    let mut retained_roots = unit_roots.to_vec();
    let mut family_scalar_roots = Vec::new();
    // Authored selection subjects are interface roots, not runtime calls.
    // Keep the complete supplied roster, even methods no body invokes; a
    // missing checked plan rejects rather than narrowing a whole-trait default.
    let mut retained_boundary_roots = Vec::new();
    for boundary in program_roots.boundaries {
        if let Some((_, signature)) = checked.trait_machine_signature(*boundary) {
            retained_boundary_roots.extend(
                checked
                    .requirement_machine_instances(signature)
                    .map(|instance| instance.symbol),
            );
        } else {
            retained_boundary_roots.push(*boundary);
        }
    }
    for boundary in &retained_boundary_roots {
        super::unique_unit_boundary(plans, *boundary).map_err(
            crate::lowering_error::unit_machine_plan_error(
                checked,
                checked.symbols.name(*boundary),
                *boundary,
            ),
        )?;
    }
    // Authored roots and target families are portable product surface, not
    // reachability from whichever application happens to be requested first.
    // Discover each contract head and implementation before allocating shared
    // namespaces. An independent head retains its opaque checked plan even
    // when no call demands it; it supplies no implementation or authority.
    let native_roots = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            let owner = checked.typed.machine_by_symbol(node.machine)?;
            if checked.typed.callable_body_has_open_parameters(owner) {
                return None;
            }
            let checked_trees::CheckedStructuralValueKind::NativeCodeLayout { selected_state } =
                node.kind
            else {
                return None;
            };
            validation::find_machine_by_entry_state(&checked.typed, selected_state)
                .map(|(machine, _)| machine.symbol)
        });
    for callable in program_roots
        .machines
        .iter()
        .copied()
        .chain(native_roots)
        .chain(
            super::target_families::runtime_families(checked)?
                .into_iter()
                .flat_map(|family| {
                    std::iter::once(family.canonical).chain(
                        family
                            .members
                            .iter()
                            .filter(|member| member.machine != family.canonical)
                            .map(|member| member.machine),
                    )
                }),
        )
    {
        let machine =
            checked
                .typed
                .machine_by_symbol(callable)
                .ok_or(LoweringError::Unsupported(
                    "program-root callable is absent from checked input",
                ))?;
        // A retained body's satisfaction edges are interface roots even
        // when no operation invokes those requirements. The checked planner
        // owns their contracts; this join never fabricates a boundary plan
        // from an implementation's private signature or inferred effects.
        // Only its published concrete plans can join this catalog; open
        // generic and named-family applications keep their separate frontier.
        for conformance in checked.typed.machine_trait_conformances(machine) {
            if plans
                .boundary_machines()
                .iter()
                .any(|boundary| boundary.machine == conformance.requirement_symbol)
                && !retained_boundary_roots.contains(&conformance.requirement_symbol)
            {
                retained_boundary_roots.push(conformance.requirement_symbol);
            }
        }
        if !machine.supplies_checked_body() {
            super::unique_unit_boundary(plans, callable).map_err(
                crate::lowering_error::unit_machine_plan_error(
                    checked,
                    machine.name.as_str(),
                    callable,
                ),
            )?;
            if !retained_boundary_roots.contains(&callable) {
                retained_boundary_roots.push(callable);
            }
            continue;
        }
        if UnitBody::contains(plans, callable)
            && UnitBody::find(plans, callable)?
                .scalar_result_type()
                .is_none()
        {
            if !retained_roots.contains(&callable) {
                retained_roots.push(callable);
            }
        } else {
            CheckedScalarCallee::find_for_unit_call(checked, callable)?;
            if !family_scalar_roots.contains(&callable) {
                family_scalar_roots.push(callable);
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
        let source_machines = plans.source_machines(checked, &closure)?;
        for source in &source_machines {
            for boundary in crate::expression_preparation::computation_graph::boundary_targets(
                checked, *source,
            )? {
                if !retained_boundary_roots.contains(&boundary) {
                    retained_boundary_roots.push(boundary);
                }
            }
        }
        let candidates = checked_unit_provider_candidates(
            checked,
            plans,
            &closure,
            &mut retained_boundary_roots,
        )?;
        // This catalog contains checked Omega bodies, not opaque realizations.
        // Their stores and atomic operations keep the same checked access and
        // custody as ordinary calls, and their transitive calls join this
        // closure. A non-observing store is also legal through mutable access
        // or into owned local storage; its operation kind does not imply an
        // opaque write-only boundary. Such boundaries retain their separate
        // source admission and provider-installation evidence requirements.
        // Several provider owners may share one ordinary implementation.
        // Candidate associations stay distinct; operation roots are a set.
        let mut new_roots = Vec::new();
        for candidate in &candidates {
            if !closure.contains(&candidate.candidate) && !new_roots.contains(&candidate.candidate)
            {
                new_roots.push(candidate.candidate);
            }
        }
        let mut ordinary_scalar_roots = Vec::new();
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
        for machine_symbol in &source_machines {
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
        }
        for machine_symbol in &closure {
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
        let scalar_closure = checked_scalar_call_closure_with_structural_roots(
            checked,
            &scalar_roots,
            &structural_scalar_roots,
        )?;

        for source in source_machines.iter().chain(&scalar_closure) {
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
                    checked, *source, None,
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
            CheckedScalarCallee::find_for_unit_call(checked, *source)?;
            if !closure.contains(source) && !new_roots.contains(source) {
                new_roots.push(*source);
            }
        }
        // A selection's table rows are reachable only through dispatch, never
        // through a call edge: every implementation a closure body selects
        // joins the closure, so the dispatch catalog can name each row.
        let mut dispatched = false;
        for source in source_machines.iter().chain(&scalar_closure) {
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
        // Every scalar helper joined the operation closure above.
        if scalar_closure
            .iter()
            .any(|source| !closure.contains(source))
        {
            return unsupported("scalar helper is absent from the Unit machine closure");
        }
        // This finite roster names callable declarations, not active invocations.
        // Discovery deduplicates recursive edges; attached_unit allocates every
        // signature and machine identity before emitting any body. Ordinary
        // calls use separate invocation frames, so a cyclic roster needs no
        // alternate lowering route. Source call requirements and custody are
        // still checked before these plans reach the catalog.
        return Ok(CheckedCallCatalog {
            operations: closure,
            source_machines,
            providers: candidates,
            boundaries: retained_boundary_roots,
        });
    }
}
