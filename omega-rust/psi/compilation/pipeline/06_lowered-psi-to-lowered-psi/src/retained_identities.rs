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
