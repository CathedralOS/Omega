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

use proof_admission::{Obligation, ObligationClass, ProofNode, ProofRule, check_certificate};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, Proposition, PropositionContext, ScalarTerm, ValueId,
};
use std::collections::BTreeMap;
use terminal_psi::{TerminalMachine, TerminalModule, Terminator};

#[cfg(test)]
mod tests;

use super::{ReconstructedOperationObligation, ReconstructedTerminalObligationOwner, path_facts};

/// One header-invariant member tagged by how the generator discharged it.
pub(super) struct HeaderAxiomFact {
    /// The member proposition appended to the header block's arrival set.
    pub proposition: Proposition,
    /// `true` when a fixed-shape `ConjunctionElimination` certificate —
    /// the declared predicate is assumption zero and the member is
    /// eliminated along its exact conjunct-index path — was re-decided by
    /// the certificate checker before this fact was emitted. `false` marks
    /// a licensed premise introduction retained under
    /// `scope:header-edge-arrival`.
    ///
    /// The generation-time check runs in production regardless; only the
    /// test module reads the classification back. The certificate
    /// machinery is exercised against every emitted member either way.
    #[allow(dead_code)]
    pub certified: bool,
}

pub(super) fn header_axioms(
    module: &TerminalModule,
    machine: MachineId,
    header: BlockId,
    proposition_context: &PropositionContext,
) -> Vec<HeaderAxiomFact> {
    let mut axioms = Vec::new();
    for invariant in module
        .scalar_block_invariants
        .iter()
        .filter(|invariant| invariant.machine == machine && invariant.header == header)
    {
        axioms.extend(certified_members(proposition_context, &invariant.predicate));
    }
    axioms
}

/// Publish every conjunction member of one declared predicate in declaration
/// order. The traversal is the former pending-stack walk: only conjunction
/// members recurse, so alternatives and implications stay whole. Each leaf
/// joins the roster only after its fixed-shape elimination certificate is
/// re-decided; a rejected certificate keeps the emission as a licensed
/// premise introduction under `scope:header-edge-arrival`.
fn certified_members(
    proposition_context: &PropositionContext,
    predicate: &Proposition,
) -> Vec<HeaderAxiomFact> {
    let mut facts = Vec::new();
    let mut pending = vec![(predicate, Vec::new())];
    while let Some((member, path)) = pending.pop() {
        if let Proposition::Conjunction(members) = member {
            pending.extend(members.iter().enumerate().rev().map(|(index, child)| {
                let mut child_path = path.clone();
                child_path.push(index);
                (child, child_path)
            }));
        } else {
            facts.push(HeaderAxiomFact {
                certified: member_certified(proposition_context, predicate, &path, member),
                proposition: member.clone(),
            });
        }
    }
    facts
}

/// Re-decide a fixed-shape elimination certificate for one emitted member
/// before it is classified. The declared predicate is assumption zero; the
/// member is eliminated along its recorded conjunct-index path so the
/// checker re-decides that the emission is a genuine conjunct of the
/// declaration rather than a fabricated proposition. A certificate the
/// checker rejects leaves the emission under `scope:header-edge-arrival`'s
/// licensed premise introductions rather than failing the module.
fn member_certified(
    proposition_context: &PropositionContext,
    predicate: &Proposition,
    path: &[usize],
    member: &Proposition,
) -> bool {
    let mut node = ProofNode {
        conclusion: predicate.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    for &conjunct in path {
        let Proposition::Conjunction(members) = &node.conclusion else {
            return false;
        };
        let Some(conclusion) = members.get(conjunct).cloned() else {
            return false;
        };
        node = ProofNode {
            conclusion,
            rule: ProofRule::ConjunctionElimination {
                conjunction: Box::new(node),
                conjunct,
            },
        };
    }
    node.conclusion == *member
        && check_certificate(
            proposition_context,
            member,
            std::slice::from_ref(predicate),
            &[],
            &node,
        )
        .is_ok()
}

pub(super) fn append_arrival_obligations(
    module: &TerminalModule,
    machine: &TerminalMachine,
    terminator: &Terminator,
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    proposition_context: &PropositionContext,
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
    let mut append_edge = |edge: EdgeId,
                           target: BlockId,
                           arguments: &[ValueId],
                           erased_arguments: &[ScalarTerm],
                           selected_axioms: &[Proposition]| {
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
            let mut substitutions = header
                .parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.id, value_term(*argument)))
                .collect::<BTreeMap<_, _>>();
            substitutions.extend(
                header
                    .erased_scalar_formals
                    .iter()
                    .zip(erased_arguments)
                    .map(|(formal, argument)| (formal.id, argument.clone())),
            );
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
            erased_arguments,
            ..
        } => append_edge(*edge, *target, arguments, erased_arguments, axioms),
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            for (successor, positive) in [(when_true, true), (when_false, false)] {
                let mut selected_axioms = axioms.to_vec();
                if let Some(fact) = path_facts::condition_fact(
                    *condition,
                    positive,
                    axioms,
                    value_term,
                    proposition_context,
                ) && !selected_axioms.contains(&fact.proposition)
                {
                    selected_axioms.push(fact.proposition);
                }
                append_edge(
                    successor.edge,
                    successor.target,
                    &successor.arguments,
                    &successor.erased_arguments,
                    &selected_axioms,
                );
            }
        }
        _ => {}
    }
}
