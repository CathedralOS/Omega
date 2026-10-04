//! Publish replayed state requirements at their actual CFG arrival boundary.
//!
//! Invocation has no edge, so the entry requirement joins the machine contract.
//! Re-entered entries have a prologue and a real header; all other state headers
//! likewise publish one invariant with an obligation for every emitted arrival,
//! including private argument-evaluation blocks. Proof reconstruction must
//! discharge these before using the header's hypotheses.

use super::{
    Block, CallEmissionContext, LoweringError, PreparedStateRequirements, Proposition, Terminator,
    ValueDeclaration, scalar_contracts, scalar_source_block, unsupported,
};
use semantic_vocabulary::{BlockId, MachineId};
use terminal_psi::{ScalarBlockInvariant, ScalarBlockInvariantArrival};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    states: &[PreparedStateRequirements],
    state_parameters: &[Vec<ValueDeclaration>],
    state_erased: &[Vec<ValueDeclaration>],
    parameters: &[ValueDeclaration],
    erased: &[ValueDeclaration],
    invocation_views: &scalar_contracts::ContractViewNamespace<'_>,
    machine: MachineId,
    identity_base: u64,
    entry: BlockId,
    blocks: &[Block],
    invocation_requires: &mut Vec<Proposition>,
    calls: &mut CallEmissionContext<'_>,
) -> Result<Vec<ScalarBlockInvariant>, LoweringError> {
    let mut invariants = Vec::new();
    let structural_types = invocation_views.structural_types;
    for (position, state) in states.iter().enumerate() {
        if state.clauses.is_empty() {
            continue;
        }
        let header = scalar_source_block(identity_base, position);
        let element_views = crate::expression_preparation::bindings::element_views(
            &state.parameters,
            structural_types,
        );
        let views = scalar_contracts::ContractViewNamespace {
            parameters: &state.parameters,
            structural_types,
            element_views: &element_views,
        };
        if position == 0
            && let Some(predicate) =
                scalar_contracts::clauses(&state.clauses, parameters, erased, invocation_views)?
        {
            // Preserve the one aggregate invocation slot used by scalar
            // calls, even when several source clauses constrain entry.
            if let Some(existing) = invocation_requires.pop() {
                invocation_requires.push(crate::proofs::contract_predicates::connective(
                    existing, predicate, true,
                )?);
            } else {
                invocation_requires.push(predicate);
            }
        }
        if header == entry {
            continue;
        }
        let values = state_parameters
            .get(position)
            .ok_or(LoweringError::Unsupported(
                "state invariant lost its scalar namespace",
            ))?;
        let erased = state_erased
            .get(position)
            .ok_or(LoweringError::Unsupported(
                "state invariant lost its erased namespace",
            ))?;
        let Some(predicate) = scalar_contracts::clauses(&state.clauses, values, erased, &views)?
        else {
            continue;
        };
        let mut arrivals = Vec::new();
        for block in blocks {
            let edges = match &block.terminator {
                Terminator::Jump { edge, target, .. } => {
                    [(*target == header).then_some(*edge), None]
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [
                    (when_true.target == header).then_some(when_true.edge),
                    (when_false.target == header).then_some(when_false.edge),
                ],
                Terminator::StructuralCase { cases, .. }
                    if cases.iter().any(|case| case.target == header) =>
                {
                    return unsupported("scalar invariant header needs case arrival proof custody");
                }
                _ => [None, None],
            };
            for edge in edges.into_iter().flatten() {
                arrivals.push(ScalarBlockInvariantArrival {
                    edge,
                    obligation: calls.allocate_requirement()?,
                });
            }
        }
        if arrivals.is_empty() {
            return unsupported("scalar state invariant has no emitted arrival");
        }
        arrivals.sort_by_key(|arrival| arrival.edge);
        invariants.push(ScalarBlockInvariant {
            machine,
            header,
            predicate,
            arrivals,
        });
    }
    invariants.sort_by_key(|invariant| (invariant.machine, invariant.header));
    Ok(invariants)
}
