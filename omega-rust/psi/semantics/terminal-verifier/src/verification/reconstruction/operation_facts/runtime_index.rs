//! The bound of every runtime index an operation's projections carry.
//!
//! Each `RuntimeIndex { index, obligation }` segment is an obligation of the
//! operation that carries it. The proposition is the one
//! `terminal_semantics::runtime_index_bound` defines for the fixed array the
//! segment's prefix resolves to, reconstructed here rather than read from the
//! module, over the facts that hold before the operation runs: the selected
//! element must exist before a store writes it or a call borrows it.

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{Proposition, ScalarType, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{Operation, TerminalMachine, TerminalModule};

use super::super::{ReconstructedOperationObligation, ReconstructedTerminalObligationOwner};
use crate::ModuleError;

pub(super) fn append(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    axioms: &[Proposition],
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<(), ModuleError> {
    for site in crate::validation::structural::runtime_indexes::runtime_index_sites(
        module, machine, operation,
    )? {
        let invalid = ModuleError::InvalidRuntimeIndex {
            operation: operation.id,
            index: site.index,
        };
        let index_type = value_types
            .get(&site.index)
            .copied()
            .ok_or(invalid.clone())?;
        let proposition =
            terminal_semantics::runtime_index_bound(site.index, index_type, site.extent)
                .ok_or(invalid)?;
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::Operation {
                machine: machine.id,
                operation: operation.id,
            },
            obligation: Obligation {
                id: site.obligation,
                proposition,
                class: ObligationClass::Derivable,
            },
            semantic_axioms: axioms.to_vec(),
            canonical_certificate: true,
        });
    }
    Ok(())
}
