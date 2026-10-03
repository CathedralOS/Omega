//! Entry origins for unversioned structural observations in private crash proofs.
//! Shared access alone does not freeze atomic residents. Inspect the exact
//! observed projection, so an ordinary sibling of an atomic field stays stable.

use semantic_vocabulary::Proposition;
use terminal_psi::{StructuralAccess, TerminalMachine, TerminalModule};
use terminal_semantics::{SharedMutation, shared_mutation};

pub(super) fn retains_entry_meaning(
    proposition: &Proposition,
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> bool {
    crate::validation::storage_observations(proposition)
        .iter()
        .all(|((root, path), _)| {
            machine.structural_parameters.iter().any(|parameter| {
                parameter.place == *root
                    && parameter.access == StructuralAccess::SharedBorrow
                    && shared_mutation(
                        module.structural_types.iter(),
                        parameter.structural_type,
                        path.as_deref().unwrap_or_default(),
                    ) == SharedMutation::None
            })
        })
}
