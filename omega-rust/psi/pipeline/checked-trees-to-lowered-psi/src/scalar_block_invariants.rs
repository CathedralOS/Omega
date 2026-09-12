//! Propose scalar block predicates only after proving every arrival.
//!
//! Entry ranges and scoped authored guarantees are candidates, not facts. This optional
//! producer pass drops any candidate whose exact establishment or preservation
//! question cannot be proved. Ordinary finalization then produces certificates
//! for the retained roster and all operation questions; independent verification
//! checks both before granting execution authority. No ranking premise is used.
//! An outer join can depend on an earlier join, so discover prerequisite demands
//! before proving the group. Discovery shares one finite budget and stops on a
//! stable canonical roster. Failed expansion can restore the initial proposals
//! once; restoration reruns proof checking and cannot confer authority itself.

use std::collections::BTreeSet;

use semantic_vocabulary::{ObligationId, Proposition};
use terminal_psi::{ScalarBlockInvariantArrival, Terminator};
use terminal_verifier::ReconstructedTerminalObligationOwner;

use crate::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
use crate::{LoweredPsi, LoweringError};

mod entry_ranges;
mod joins;
mod retained_evidence;

#[cfg(test)]
mod tests;

pub(super) fn retain_provable(lowered: &mut LoweredPsi) -> Result<(), LoweringError> {
    let module = &mut lowered.semantic_module;
    if !module.scalar_block_invariants.is_empty() {
        return Ok(());
    }
    // The legacy countdown has its own interpretation-only admission route;
    // this inference uses ordinary cyclic control, ranked or unranked.
    if module.machines.iter().any(|machine| {
        machine
            .ranked_scc
            .as_ref()
            .is_some_and(|ranking| ranking.as_unsigned_countdown().is_some())
    }) {
        return Ok(());
    }
    let original = terminal_verifier::reconstruct_terminal_obligations(module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut remaining = 4096usize;
    let mut candidates = entry_ranges::candidates(module);
    candidates.extend(joins::candidates(module, &original, &mut remaining));
    if candidates.is_empty() {
        return Ok(());
    }
    let mut next = terminal_verifier::maximum_registered_obligation_id(module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut seeds = None;
    'discovery: loop {
        // A block has one assertion and one exhaustive arrival group. Aggregate
        // optional range and guarantee proposals with the ordinary conjunction;
        // failure of any conjunct discards this proposal, never an actual edge.
        candidates.sort_by_key(|candidate| (candidate.machine, candidate.header));
        let mut grouped: Vec<terminal_psi::ScalarBlockInvariant> = Vec::new();
        for mut candidate in candidates {
            candidate.predicate = canonical_predicate(candidate.predicate)?;
            if let Some(previous) = grouped.last_mut()
                && (previous.machine, previous.header) == (candidate.machine, candidate.header)
            {
                previous.predicate = crate::contract_predicates::connective(
                    previous.predicate.clone(),
                    candidate.predicate,
                    true,
                )?;
            } else {
                grouped.push(candidate);
            }
        }
        let mut changed = false;
        for mut candidate in grouped {
            if let Some(existing) = module.scalar_block_invariants.iter_mut().find(|existing| {
                (existing.machine, existing.header) == (candidate.machine, candidate.header)
            }) {
                let predicate = crate::contract_predicates::connective(
                    existing.predicate.clone(),
                    candidate.predicate,
                    true,
                )?;
                changed |= predicate != existing.predicate;
                existing.predicate = predicate;
                continue;
            }
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == candidate.machine)
                .ok_or(LoweringError::Unsupported(
                    "invariant candidate lost its machine",
                ))?;
            let mut edges = Vec::new();
            for block in &machine.blocks {
                match &block.terminator {
                    Terminator::Jump { edge, target, .. } if *target == candidate.header => {
                        edges.push(*edge);
                    }
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        edges.extend(
                            [when_true, when_false]
                                .into_iter()
                                .filter(|successor| successor.target == candidate.header)
                                .map(|successor| successor.edge),
                        );
                    }
                    _ => {}
                }
            }
            edges.sort();
            for edge in edges {
                let Some(fresh) = next.checked_add(1) else {
                    if !restore_seeds(&mut module.scalar_block_invariants, &mut seeds) {
                        module.scalar_block_invariants.clear();
                    }
                    break 'discovery;
                };
                next = fresh;
                candidate.arrivals.push(ScalarBlockInvariantArrival {
                    edge,
                    obligation: ObligationId::new(next).ok_or(LoweringError::Unsupported(
                        "scalar invariant obligation identity is zero",
                    ))?,
                });
            }
            module.scalar_block_invariants.push(candidate);
            changed = true;
        }
        module
            .scalar_block_invariants
            .sort_by_key(|candidate| (candidate.machine, candidate.header));
        if !changed {
            break;
        }
        if seeds.is_none() {
            seeds = Some(module.scalar_block_invariants.clone());
        }
        // An outer join's arrival may itself need a predicate at an earlier join.
        // Discover that demand before rejecting the outer proposal. These rows are
        // provisional: the unchanged all-arrival proof/drop transaction below is
        // the acceptance boundary, including every discovered prerequisite.
        let Ok(questions) = terminal_verifier::reconstruct_terminal_obligations(module) else {
            if !restore_seeds(&mut module.scalar_block_invariants, &mut seeds) {
                module.scalar_block_invariants.clear();
            }
            break;
        };
        candidates = joins::candidates(module, &questions, &mut remaining);
    }
    loop {
        // A candidate may not name the verifier's canonical feedback header.
        // The original module was validated above; optional inference must not
        // turn an unsupported candidate shape into a new program restriction.
        let Ok(validated) = terminal_verifier::validate_module(module) else {
            if restore_seeds(&mut module.scalar_block_invariants, &mut seeds) {
                continue;
            }
            module.scalar_block_invariants.clear();
            return Ok(());
        };
        let questions = terminal_verifier::reconstruct_terminal_obligations(module)
            .map_err(LoweringError::InvalidTerminalModule)?;
        let mut rejected = BTreeSet::new();
        for site in questions.obligations() {
            let ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                machine, header, ..
            } = site.owner
            else {
                continue;
            };
            let owner = module
                .machines
                .iter()
                .find(|owner| owner.id == machine)
                .ok_or(LoweringError::Unsupported(
                    "invariant proof owner disappeared",
                ))?;
            let context = validated
                .value_context(owner)
                .map_err(LoweringError::InvalidTerminalModule)?;
            let parameters = owner.parameters.iter().map(|value| value.id).collect();
            if produce_checked_canonical_integer_proof(
                &context,
                &site.obligation.proposition,
                &site.requirements,
                &site.semantic_axioms,
                &parameters,
            )
            .is_none()
            {
                rejected.insert((machine, header));
            }
        }
        if rejected.is_empty() {
            // Previously supplied source certificates can carry exact axiom
            // indexes. Optional inference must not make them stale. If a changed
            // question cannot replay its retained certificate, keep the original
            // module rather than silently discard its source-derived evidence.
            if !retained_evidence::replays(module, &original, &questions, &lowered.proof_bundle)? {
                if restore_seeds(&mut module.scalar_block_invariants, &mut seeds) {
                    continue;
                }
                module.scalar_block_invariants.clear();
            }
            return Ok(());
        }
        // A failed strengthening must not erase a useful original assertion.
        // Restore the whole seed transaction: another seed can depend on this
        // one and already have failed in the same round. The snapshot contains
        // proposals, not authority; every arrival is proved again below.
        if seeds.as_ref().is_some_and(|seeds| {
            seeds
                .iter()
                .any(|seed| rejected.contains(&(seed.machine, seed.header)))
        }) && restore_seeds(&mut module.scalar_block_invariants, &mut seeds)
        {
            continue;
        }
        module
            .scalar_block_invariants
            .retain(|candidate| !rejected.contains(&(candidate.machine, candidate.header)));
        if module.scalar_block_invariants.is_empty() {
            return Ok(());
        }
        // A surviving proof may have used a removed candidate. Reconstruct and
        // prove again under only the remaining hypotheses before retaining it.
    }
}

fn restore_seeds(
    roster: &mut Vec<terminal_psi::ScalarBlockInvariant>,
    seeds: &mut Option<Vec<terminal_psi::ScalarBlockInvariant>>,
) -> bool {
    let Some(seeds) = seeds.take() else {
        return false;
    };
    if *roster == seeds {
        return false;
    }
    *roster = seeds;
    true
}

fn canonical_predicate(predicate: Proposition) -> Result<Proposition, LoweringError> {
    Ok(match predicate {
        Proposition::Equal(left, right) => {
            crate::contract_predicates::canonical_equality(left, right)?
        }
        Proposition::Conjunction(members) => canonical_members(members, true)?,
        Proposition::Disjunction(members) => canonical_members(members, false)?,
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            let premise = canonical_predicate(*premise)?;
            let conclusion = canonical_predicate(*conclusion)?;
            // Transport through a selected arrival often closes an inner
            // guard. Keep the resulting demand, not a tower of vacuous guards.
            // This simplifies proposals only; all arrival proofs remain exact.
            match premise {
                Proposition::Truth => conclusion,
                Proposition::Falsehood => Proposition::Truth,
                _ if premise == conclusion || conclusion == Proposition::Truth => {
                    Proposition::Truth
                }
                _ => Proposition::Implication {
                    premise: Box::new(premise),
                    conclusion: Box::new(conclusion),
                },
            }
        }
        other => other,
    })
}

fn canonical_members(
    members: Vec<Proposition>,
    conjunction: bool,
) -> Result<Proposition, LoweringError> {
    let mut members = members.into_iter();
    let Some(first) = members.next() else {
        return Ok(if conjunction {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    };
    let mut result = canonical_predicate(first)?;
    for member in members {
        result = crate::contract_predicates::connective(
            result,
            canonical_predicate(member)?,
            conjunction,
        )?;
    }
    Ok(result)
}
