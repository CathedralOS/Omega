//! Matching scalar predicates use the current destination and exact selected edge.
//!
//! Header predicates are hypotheses of the induction step, not trusted producer
//! facts. Reconstruction emits every establishment and preservation obligation
//! together with the ordinary operation-safety questions. The verifier returns
//! authority only if that whole transaction checks. Initial arrival cannot see
//! a header hypothesis; a partial operation cannot see its own result equation
//! when proving safety. Completed operations may then prove the next arrival.
//! Private crash-entry reconstruction does not import these current-value facts.
//!
//! Arrival goals are emitted before successor binding, including scheduling-cut
//! edges. Initial arrival therefore cannot assume the destination's predicate.
//! Preservation uses current-header hypotheses and completed operation facts;
//! each operation's own safety question was captured before its result facts.

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, Proposition, ScalarTerm, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{TerminalMachine, TerminalModule, Terminator};

use super::{ReconstructedOperationObligation, ReconstructedTerminalObligationOwner, path_facts};

pub(super) fn header_axioms(
    module: &TerminalModule,
    machine: MachineId,
    header: BlockId,
) -> Vec<Proposition> {
    let mut axioms = Vec::new();
    for invariant in module
        .scalar_block_invariants
        .iter()
        .filter(|invariant| invariant.machine == machine && invariant.header == header)
    {
        // A checked conjunction establishes every member unconditionally.
        // Publish those facts in the same form as ordinary operation facts and
        // the former range bounds; hiding them inside a connective defeats
        // atomic arithmetic reconstruction/search. Never split alternatives or
        // implications: their members are not independently established.
        let mut pending = vec![&invariant.predicate];
        while let Some(predicate) = pending.pop() {
            if let Proposition::Conjunction(members) = predicate {
                pending.extend(members.iter().rev());
            } else {
                axioms.push(predicate.clone());
            }
        }
    }
    axioms
}

pub(super) fn append_arrival_obligations(
    module: &TerminalModule,
    machine: &TerminalMachine,
    terminator: &Terminator,
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    axioms: &[Proposition],
    obligations: &mut Vec<ReconstructedOperationObligation>,
) {
    if !module
        .scalar_block_invariants
        .iter()
        .any(|invariant| invariant.machine == machine.id)
    {
        return;
    }
    let mut append_edge =
        |edge: EdgeId, target: BlockId, arguments: &[ValueId], selected_axioms: &[Proposition]| {
            for invariant in module
                .scalar_block_invariants
                .iter()
                .filter(|invariant| invariant.machine == machine.id && invariant.header == target)
            {
                let header = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == target)
                    .expect("validated invariant header exists");
                let arrival = invariant
                    .arrivals
                    .iter()
                    .find(|arrival| arrival.edge == edge)
                    .expect("validated invariant retains every actual arrival");
                // Simultaneous substitution is essential for crossed or repeated
                // arguments: replacement terms remain in the predecessor scope.
                let substitutions = header
                    .parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.id, value_term(*argument)))
                    .collect::<BTreeMap<_, _>>();
                let proposition = super::super::substitution::substitute_proposition_values(
                    &invariant.predicate,
                    &substitutions,
                );
                obligations.push(ReconstructedOperationObligation {
                    owner: ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                        machine: machine.id,
                        header: target,
                        edge,
                    },
                    obligation: Obligation {
                        id: arrival.obligation,
                        proposition,
                        class: ObligationClass::Derivable,
                    },
                    semantic_axioms: selected_axioms.to_vec(),
                    canonical_certificate: true,
                });
            }
        };
    match terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            ..
        } => append_edge(*edge, *target, arguments, axioms),
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            for (successor, positive) in [(when_true, true), (when_false, false)] {
                let mut selected_axioms = axioms.to_vec();
                if let Some(fact) =
                    path_facts::condition_fact(*condition, positive, axioms, value_term)
                    && !selected_axioms.contains(&fact)
                {
                    selected_axioms.push(fact);
                }
                append_edge(
                    successor.edge,
                    successor.target,
                    &successor.arguments,
                    &selected_axioms,
                );
            }
        }
        _ => {}
    }
}
