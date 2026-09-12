//! Scalar merge and induction claims name exact blocks and every actual arrival.
//!
//! Predicates are scoped to the destination telescope and immutable invocation
//! formals, not every value known to the machine. In particular a branch-local
//! definition cannot become meaningful on another branch. Entry assertions are
//! forbidden because invocation is an implicit arrival without an edge proof.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{MachineId, Proposition, PropositionContext};
use terminal_psi::{TerminalMachine, TerminalModule, Terminator};

use super::{IdRegistry, ModuleError, insert_unique};

pub(super) fn validate(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    let mut previous = None;
    for invariant in &module.scalar_block_invariants {
        let identity = (invariant.machine, invariant.header);
        if previous.is_some_and(|previous| previous >= identity) {
            return Err(ModuleError::NonCanonicalScalarBlockInvariants);
        }
        previous = Some(identity);
        let invalid = || ModuleError::InvalidScalarBlockInvariant {
            machine: invariant.machine,
            header: invariant.header,
        };
        let invalid_arrivals = || ModuleError::InvalidScalarBlockInvariantArrivals {
            machine: invariant.machine,
            header: invariant.header,
        };
        let machine = machines.get(&invariant.machine).ok_or_else(invalid)?;
        let header = machine
            .blocks
            .iter()
            .find(|block| block.id == invariant.header)
            .ok_or_else(invalid)?;
        if header.id == machine.entry || !scalar_predicate(&invariant.predicate) {
            return Err(invalid());
        }
        let context = PropositionContext::from_value_types(
            machine
                .parameters
                .iter()
                .chain(&header.parameters)
                .map(|parameter| (parameter.id, parameter.scalar_type)),
        )
        .map_err(|_| invalid())?;
        context
            .validate(&invariant.predicate)
            .map_err(|_| invalid())?;
        let mut arrivals = BTreeSet::new();
        for block in &machine.blocks {
            match &block.terminator {
                Terminator::Jump { edge, target, .. } if *target == invariant.header => {
                    arrivals.insert(*edge);
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for successor in [when_true, when_false] {
                        if successor.target == invariant.header {
                            arrivals.insert(successor.edge);
                        }
                    }
                }
                Terminator::StructuralCase { cases, .. }
                    if cases
                        .iter()
                        .any(|successor| successor.target == invariant.header) =>
                {
                    return Err(invalid_arrivals());
                }
                _ => {}
            }
        }
        if !invariant
            .arrivals
            .iter()
            .map(|arrival| arrival.edge)
            .eq(arrivals)
        {
            return Err(invalid_arrivals());
        }
        for arrival in &invariant.arrivals {
            insert_unique(
                &mut registry.obligations,
                arrival.obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
    }
    Ok(())
}

// The context contains no places: even nested field terms fail formation.
// Explicitly reject non-scalar atoms which do not necessarily contain values.
fn scalar_predicate(predicate: &Proposition) -> bool {
    match predicate {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Equal(..)
        | Proposition::LessThan(..)
        | Proposition::LessOrEqual(..) => true,
        Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
            members.iter().all(scalar_predicate)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => scalar_predicate(premise) && scalar_predicate(conclusion),
        _ => false,
    }
}
