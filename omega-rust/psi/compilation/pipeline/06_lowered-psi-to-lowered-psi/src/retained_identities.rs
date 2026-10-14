//! Optimizer module role: stage group. Identities a rewrite must not erase.
//!
//! Proof, custody and ranking sidecars name values and coordinates that no
//! direct use in the module mentions, so a pass that judges liveness or
//! equivalence from uses alone would delete or renumber them. `proof_values`
//! answers which values those sidecars name; `ranked_coverage` answers which
//! coordinates one machine's retained ranking evidence covers. The three
//! rewrites that can drop or merge a value — `copy_propagation`,
//! `global_value_numbering` and `dead_scalar_elimination` — consult both
//! before acting; nothing here rewrites anything itself.

pub(crate) mod proof_values;
pub(crate) mod ranked_coverage;

/// Selected boundary operations retain their meaning until Omega supplies
/// the realization. The sorted portable demand roster is the authority here,
/// not a checked-source occurrence list.
pub(crate) fn has_boundary_application(
    demands: &[terminal_psi::TerminalBoundaryApplicationDemand],
    operation: semantic_vocabulary::OperationId,
) -> bool {
    demands
        .binary_search_by_key(&operation, |demand| demand.terminal_operation())
        .is_ok()
}

pub(crate) fn boundary_application_results(
    module: &terminal_psi::TerminalModule,
) -> impl Iterator<Item = semantic_vocabulary::ValueId> + '_ {
    module
        .operations()
        .filter(|operation| {
            has_boundary_application(&module.boundary_application_demands, operation.id)
        })
        .filter_map(|operation| operation.result.scalar().map(|result| result.id))
}

/// Unsealed source custody names either an ordinary operation or a terminal
/// invocation edge. Their namespaces remain distinct, even at equal markers.
pub(crate) fn contains_source_call(
    machine: &terminal_psi::TerminalMachine,
    occurrence: &lowered_psi::LoweredSourceCallOccurrence,
) -> bool {
    match occurrence.terminal_site {
        semantic_vocabulary::BoundaryInvocationSite::Operation(operation) => machine
            .blocks
            .iter()
            .any(|block| block.operations.iter().any(|row| row.id == operation)),
        semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(edge) => {
            machine.blocks.iter().any(|block| {
                matches!(block.terminator,
                terminal_psi::Terminator::ExternalCompletion { edge: actual, .. } if actual == edge)
            })
        }
    }
}

/// Prune debug sites whose subject a rewrite removed — dropped operations,
/// their scalar results, and dropped block parameters — then restamp the
/// module identity on the surviving map.
pub(crate) fn prune_debug_map(
    before: &lowered_psi::LoweredPsi,
    after: &mut lowered_psi::LoweredPsi,
) -> Result<(), crate::PsiOptimizationStageError> {
    use std::collections::BTreeSet;
    let Some(debug) = after.debug_map.as_mut() else {
        return Ok(());
    };
    let operations = after
        .semantic_module
        .operations()
        .map(|operation| operation.id)
        .collect::<BTreeSet<_>>();
    let removed_values = before
        .semantic_module
        .machines
        .iter()
        .zip(&after.semantic_module.machines)
        .flat_map(|(old_machine, new_machine)| {
            let removed_results = old_machine
                .operations()
                .filter(|operation| !operations.contains(&operation.id))
                .filter_map(|operation| operation.result.scalar().map(|value| value.id));
            let removed_parameters = old_machine.blocks.iter().zip(&new_machine.blocks).flat_map(
                |(old_block, new_block)| {
                    let retained = new_block
                        .parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect::<BTreeSet<_>>();
                    old_block
                        .parameters
                        .iter()
                        .filter(move |parameter| !retained.contains(&parameter.id))
                        .map(|parameter| parameter.id)
                },
            );
            removed_results.chain(removed_parameters)
        })
        .collect::<BTreeSet<_>>();
    debug.sites.retain(|site| match site.subject {
        terminal_psi::DebugSubject::Operation(operation) => operations.contains(&operation),
        terminal_psi::DebugSubject::Value(value) => !removed_values.contains(&value),
        _ => true,
    });
    debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
        .map_err(crate::PsiOptimizationStageError::InvalidSemantic)?;
    Ok(())
}

/// The catalog joins a selected representative call to its public operation.
/// Its scalar result remains an exact correspondence endpoint even when no
/// ordinary executable use survives; preserve it until catalog-aware rewriting
/// can transport that judgment along with the computation.
pub(crate) fn quotient_call_results(
    module: &terminal_psi::TerminalModule,
) -> impl Iterator<Item = semantic_vocabulary::ValueId> + '_ {
    module
        .operations()
        .filter(|operation| {
            module
                .quotients
                .operations
                .iter()
                .any(|row| row.call == operation.id)
        })
        .filter_map(|operation| operation.result.scalar().map(|result| result.id))
}
