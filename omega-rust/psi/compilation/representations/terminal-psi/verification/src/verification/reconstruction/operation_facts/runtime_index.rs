//! The bound of every runtime index an operation's projections carry.
//!
//! Each `RuntimeIndex { index, obligation }` segment is an obligation of the
//! operation that carries it. The proposition is the one
//! `terminal_semantics::runtime_index_bound` defines for the fixed array the
//! segment's prefix resolves to — or, on an element view, `index < length`
//! over the dominating `ElementViewLength` observation the emission minted,
//! the same relation an element-view write carries as its `length` operand —
//! reconstructed here rather than read from the module, over the facts that
//! hold before the operation runs: the selected element must exist before a
//! store writes it or a call borrows it.

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId};
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
        let proposition = match site.extent {
            crate::validation::structural::runtime_indexes::RuntimeIndexExtent::Literal(extent) => {
                terminal_semantics::runtime_index_bound(site.index, index_type, extent)
                    .ok_or(invalid)?
            }
            crate::validation::structural::runtime_indexes::RuntimeIndexExtent::ObservedLength(
                length,
            ) => {
                // Selector emission always converts to the u64 count
                // coordinate an `ElementViewLength` observation returns, so
                // both terms of the bound carry that one scalar type.
                let u64 = ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?,
                );
                if index_type != u64 {
                    return Err(invalid);
                }
                Proposition::LessThan(
                    ScalarTerm::value(site.index, u64),
                    ScalarTerm::value(length, u64),
                )
            }
        };
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
