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
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
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
