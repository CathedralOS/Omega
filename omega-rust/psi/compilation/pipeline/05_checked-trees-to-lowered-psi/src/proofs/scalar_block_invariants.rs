//! Propose scalar block predicates only after proving every arrival.
//!
//! Entry ranges and scoped authored guarantees are candidates, not facts. This optional
//! producer pass drops any candidate whose exact establishment or preservation
//! question cannot be proved. Ordinary finalization then produces certificates
//! for the retained roster and all operation questions; independent verification
//! checks both before granting execution authority. No ranking premise is used.
//! An outer join can depend on an earlier join, so discover prerequisite demands
//! before proving the group. Discovery shares one finite budget and stops on a
//! stable canonical roster, and the discovery/proof rounds — each a whole-module
//! reconstruction or revalidation — draw from one aggregate bound so the pass's
//! total compile-time work is named and accounted rather than emergent. Failed
//! expansion can restore the initial proposals once; restoration reruns proof
//! checking and cannot confer authority itself.
//!
//! Authored rows are mandatory, not proposals. The pass never judges, prunes
//! or removes an authored row it did not extend; a rejected extension falls
//! back to exactly the authored predicate. Finalization proves authored
//! arrivals or rejects the module. Judging them here made one machine's
//! authored contract, unprovable by this optional search alone, roll back
//! every other machine's proved inference through the mandatory-roster check.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{MachineId, ObligationId, Proposition, ValueId};
use terminal_psi::{ScalarBlockInvariantArrival, Terminator};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

use crate::lowering_error::{LoweringError, ProofSearchCapacity};
use crate::proofs::ProofSearchDeclines;
use crate::proofs::nonzero_divisor_certificate::{
    produce_checked_canonical_integer_proof, produce_relaxed_integer_proof,
};
use lowered_psi::LoweredPsi;

mod cyclic_guarantees;
mod entry_ranges;
mod field_bounds;
mod joins;
mod lockstep;
use super::retained_evidence;

/// One reconstructed question as the discovery producers read it. A roster
/// round re-reconstructs the module, and a site can come back under its old
/// obligation identity with a different question: a strengthened invariant
/// keeps its arrival obligations while its goal grows, and a newly retained
/// invariant adds axioms to the operation sites it dominates. Producer output
/// is a function of all of it, so the queried roster keys the whole question;
/// an unchanged site is still transported at most once across rounds.
type QueriedSite = (
    ObligationId,
    Proposition,
    Vec<Proposition>,
    Vec<Proposition>,
);

fn queried_site(site: &terminal_verifier::ReconstructedTerminalObligation) -> QueriedSite {
    (
        site.obligation.id,
        site.obligation.proposition.clone(),
        site.requirements.clone(),
        site.semantic_axioms.clone(),
    )
}

#[cfg(test)]
mod authored_row_tests;
#[cfg(test)]
mod tests;

/// Run optional inference under the production provisions, returning the
/// capacities that declined so a later unmet obligation can name the
/// resource that refused instead of reading as a disproof. Only tests call
/// this spelling: production reaches the same pass through finalization
/// with the explicit provision.
#[cfg(test)]
pub(crate) fn retain_provable(
    lowered: &mut LoweredPsi,
) -> Result<ProofSearchDeclines, LoweringError> {
    retain_provable_bounded(lowered, MAX_ROSTER_ROUNDS)
}

/// The same pass under an explicit roster-round provision: the ledger a
/// caller gets back records every named capacity the pass declined against.
pub(crate) fn retain_provable_bounded(
    lowered: &mut LoweredPsi,
    roster_rounds: usize,
) -> Result<ProofSearchDeclines, LoweringError> {
    let mut declines = ProofSearchDeclines::default();
    // Authored arrival contracts are mandatory, but they do not describe every
    // fact needed by the body. Inference may strengthen them; its prove-or-drop
    // transaction must never remove a clause or arrival supplied by the source.
    let mandatory = lowered.semantic_module.scalar_block_invariants.clone();
    let original = terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    retain_provable_roster_bounded(lowered, &original, roster_rounds, &mut declines)?;
    if !preserves_mandatory_roster(&mandatory, &lowered.semantic_module.scalar_block_invariants) {
        lowered.semantic_module.scalar_block_invariants = mandatory;
        return Ok(declines);
    }
    // A cyclic machine's guarantee may still be open once the ordinary
    // roster is retained; its strengthening is a separate all-or-nothing
    // transaction that leaves this roster unchanged when it cannot help.
    //
    // The roster pass writes only `scalar_block_invariants`. An unchanged
    // roster reuses the original reconstruction; a strengthened one needs
    // reconstruction against its new predicates.
    let unchanged = lowered.semantic_module.scalar_block_invariants == mandatory;
    cyclic_guarantees::strengthen(lowered, unchanged.then_some(&original), &mut declines)?;
    Ok(declines)
}

/// The atomic members of a predicate, with nested conjunctions flattened.
fn members(predicate: &Proposition) -> BTreeSet<&Proposition> {
    let mut pending = vec![predicate];
    let mut members = BTreeSet::new();
    while let Some(member) = pending.pop() {
        if let Proposition::Conjunction(children) = member {
            pending.extend(children);
        } else {
            members.insert(member);
        }
    }
    members
}

fn preserves_mandatory_roster(
    mandatory: &[terminal_psi::ScalarBlockInvariant],
    retained: &[terminal_psi::ScalarBlockInvariant],
) -> bool {
    mandatory.iter().all(|original| {
        retained.iter().any(|candidate| {
            original.machine == candidate.machine
                && original.header == candidate.header
                && original.arrivals == candidate.arrivals
                && members(&original.predicate).is_subset(&members(&candidate.predicate))
        })
    })
}

/// Aggregate bound on the roster pass's repeated whole-module work: each
/// discovery round below reconstructs the module's obligation set and
/// regenerates join demands, and each proof round revalidates and
/// reconstructs the module before checking every retained site. Both loops
/// draw from this one budget so the pass's aggregate compile-time proof work
/// stays a named, accounted constant rather than an emergent convergent
/// schedule. Declining a round drops only provisional proposals — every
/// retained row still passes the all-arrival proof transaction — so
/// exhaustion is never itself a reason to reject the module.
pub(crate) const MAX_ROSTER_ROUNDS: usize =
    ProofSearchCapacity::ScalarInvariantRosterRounds.limit() as usize;

/// The shared candidate-transport pool the field-bound and join producers
/// draw from, one provisioned work budget across every discovery iteration.
const MAX_CANDIDATE_WORK: usize =
    ProofSearchCapacity::ScalarInvariantCandidateWork.limit() as usize;

/// The authored row at `header`, when the source supplied one.
fn authored_row(
    mandatory: &[terminal_psi::ScalarBlockInvariant],
    machine: MachineId,
    header: semantic_vocabulary::BlockId,
) -> Option<&terminal_psi::ScalarBlockInvariant> {
    mandatory
        .iter()
        .find(|row| row.machine == machine && row.header == header)
}

fn retain_provable_roster_bounded(
    lowered: &mut LoweredPsi,
    original: &ReconstructedTerminalObligationSet,
    mut rounds: usize,
    declines: &mut ProofSearchDeclines,
) -> Result<(), LoweringError> {
    // The roster on entry is the source's: every row is an authored,
    // mandatory arrival contract.
    let source_roster = lowered.semantic_module.scalar_block_invariants.clone();
    let mandatory = source_roster.as_slice();
    let module = &mut lowered.semantic_module;
    let mut remaining = MAX_CANDIDATE_WORK;
    // A reconstructed question yields the same producer output every round;
    // each round's fresh questions repeat every unchanged prior site plus the
    // newly minted or changed ones (see `QueriedSite`). Share the queried
    // roster with the producers so an unchanged site is transported at most
    // once across the whole discovery loop: rounds then cost only the
    // genuinely new questions, and a rejected candidate cannot re-mint itself
    // and keep discovery from settling.
    let mut join_queried = BTreeSet::new();
    let mut arrival_queried = BTreeSet::new();
    let mut candidates = entry_ranges::candidates(module);
    candidates.extend(field_bounds::candidates(
        module,
        original,
        &mut remaining,
        declines,
    ));
    if remaining == 0 {
        declines.record(ProofSearchCapacity::ScalarInvariantCandidateWork);
    }
    candidates.extend(joins::candidates(
        module,
        original,
        &mut remaining,
        declines,
        &mut join_queried,
    ));
    if remaining == 0 {
        declines.record(ProofSearchCapacity::ScalarInvariantCandidateWork);
    }
    // A guarded bound the cycle's own update can break is rewritten into the
    // lockstep family the induction step needs; the prove-or-drop boundary
    // below still decides every clause.
    lockstep::strengthen(module, &mut candidates);
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
                previous.predicate = crate::proofs::contract_predicates::connective(
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
                let predicate = crate::proofs::contract_predicates::connective(
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
            for edge in header_arrival_edges(machine, candidate.header) {
                let Some(fresh) = next.checked_add(1) else {
                    declines.record(ProofSearchCapacity::TerminalObligationIdentities);
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
        //
        // Another whole-module reconstruction follows; when the round budget
        // is spent, keep the merged roster for the proof transaction to judge
        // and stop discovering instead of looping without bound.
        let Some(next_round) = rounds.checked_sub(1) else {
            declines.record(ProofSearchCapacity::ScalarInvariantRosterRounds);
            break;
        };
        rounds = next_round;
        let started = std::time::Instant::now();
        let questions = terminal_verifier::reconstruct_terminal_obligations(module);
        crate::proofs::proof_timing::count_reconstruct(started.elapsed());
        let Ok(questions) = questions else {
            if !restore_seeds(&mut module.scalar_block_invariants, &mut seeds) {
                module.scalar_block_invariants.clear();
            }
            break;
        };
        // Nested feedback cuts need retained storage bounds as well as scalar
        // join demands. Revisit only the arrivals of demanded headers: rerunning
        // operation proposals would repeatedly wrap their expanding guard sets.
        candidates = field_bounds::arrival_bounds(
            module,
            &questions,
            &mut remaining,
            declines,
            &mut arrival_queried,
        );
        candidates.extend(joins::candidates(
            module,
            &questions,
            &mut remaining,
            declines,
            &mut join_queried,
        ));

        if remaining == 0 {
            declines.record(ProofSearchCapacity::ScalarInvariantCandidateWork);
        }
    }
    // A proof verdict is a pure function of the producer inputs below: the
    // machine-local value context, the goal, the reconstructed requirements and
    // axioms, and the parameter roster. A roster round that leaves a site's
    // inputs unchanged cannot change its verdict, so rounds re-proving an
    // unchanged site reuse the recorded verdict instead of re-searching.
    let mut verdicts = BTreeMap::<
        (
            MachineId,
            Proposition,
            Vec<Proposition>,
            Vec<Proposition>,
            BTreeSet<ValueId>,
        ),
        bool,
    >::new();
    // Machine-local proposition contexts shared across sites and rounds; a
    // context reads the machine's own places, which roster pruning never
    // mutates.
    let mut contexts = BTreeMap::new();
    if module.scalar_block_invariants.is_empty() {
        // Nothing survived discovery: the proof loop checks only invariant
        // arrivals, so its whole-module revalidation would decide nothing.
        // Source-supplied certificates are unaffected because `original` was
        // reconstructed under this same empty roster.
        return Ok(());
    }
    loop {
        // Each round revalidates and reconstructs the whole module before
        // checking the retained sites. A declined round clears the
        // provisional roster: the pass is optional inference, so running out
        // of rounds is never itself a reason to reject the module.
        let Some(next_round) = rounds.checked_sub(1) else {
            declines.record(ProofSearchCapacity::ScalarInvariantRosterRounds);
            module.scalar_block_invariants.clear();
            return Ok(());
        };
        rounds = next_round;
        // A candidate may not name the verifier's canonical feedback header.
        // The original module was validated above; optional inference must not
        // turn an unsupported candidate shape into a new program restriction.
        let started = std::time::Instant::now();
        let validated = terminal_verifier::validate_module(module);
        crate::proofs::proof_timing::count_validate(started.elapsed());
        let Ok(validated) = validated else {
            if restore_seeds(&mut module.scalar_block_invariants, &mut seeds) {
                continue;
            }
            module.scalar_block_invariants.clear();
            return Ok(());
        };
        let started = std::time::Instant::now();
        let questions = terminal_verifier::reconstruct_terminal_obligations(module)
            .map_err(LoweringError::InvalidTerminalModule)?;
        crate::proofs::proof_timing::count_reconstruct(started.elapsed());
        let mut rejected = BTreeSet::new();
        // Members of a rejected header's conjunction that fail on their own at
        // some arrival, by position, with the conjunction's arity at that arrival.
        let mut refuted = BTreeMap::<_, (usize, BTreeSet<usize>)>::new();
        for site in questions.obligations() {
            let ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                machine, header, ..
            } = site.owner
            else {
                continue;
            };
            // An authored row this pass did not extend is not a proposal. Its
            // arrivals are mandatory obligations: finalization proves them or
            // rejects the module. Judging them here could only remove a
            // source clause the optional search cannot reproduce, and the
            // mandatory-roster check below would then discard every other
            // machine's proved inference with it.
            if let Some(authored) = authored_row(mandatory, machine, header)
                && module
                    .scalar_block_invariants
                    .iter()
                    .any(|row| row == authored)
            {
                continue;
            }
            let owner = module
                .machines
                .iter()
                .find(|owner| owner.id == machine)
                .ok_or(LoweringError::Unsupported(
                    "invariant proof owner disappeared",
                ))?;
            // A value context is machine-local; sites on one machine share
            // it instead of rebuilding the machine's proposition context.
            let context = match contexts.entry(owner.id) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let vc_started = std::time::Instant::now();
                    let context = validated
                        .value_context(owner)
                        .map_err(LoweringError::InvalidTerminalModule)?;
                    crate::proofs::proof_timing::count_value_context(vc_started.elapsed());
                    entry.insert(context)
                }
            };
            let context = &*context;
            let parameters = owner
                .parameters
                .iter()
                .map(|value| value.id)
                .collect::<BTreeSet<ValueId>>();
            let mut proves = |goal: &Proposition| {
                let key = (
                    machine,
                    goal.clone(),
                    site.requirements.clone(),
                    site.semantic_axioms.clone(),
                    parameters.clone(),
                );
                if let Some(verdict) = verdicts.get(&key) {
                    crate::proofs::proof_timing::count_verdict_hit();
                    return *verdict;
                }
                crate::proofs::proof_timing::count_verdict_miss();
                let counted = crate::proofs::proof_timing::Counted::enter(
                    crate::proofs::proof_timing::count_producer,
                );
                let verdict = produce_checked_canonical_integer_proof(
                    context,
                    goal,
                    &site.requirements,
                    &site.semantic_axioms,
                    &parameters,
                )
                // Arrival obligations can outgrow the canonical custody envelope
                // the same way operation obligations do: an endpoint that meets
                // cited facts only through equality/definition chains. The relaxed
                // search is the named last resort here too — canonical producers
                // first, the bounded derived closure only at unproven leaves, and
                // the kernel re-checks the certificate before the candidate is
                // retained. Guarded `Implication` premises keep their scope.
                .or_else(|| {
                    produce_relaxed_integer_proof(
                        context,
                        goal,
                        &site.requirements,
                        &site.semantic_axioms,
                        &parameters,
                    )
                })
                .is_some();
                drop(counted);
                verdicts.insert(key, verdict);
                verdict
            };
            if !proves(&site.obligation.proposition) {
                rejected.insert((machine, header));
                if let Proposition::Conjunction(members) = &site.obligation.proposition {
                    let (arity, failing) = refuted
                        .entry((machine, header))
                        .or_insert_with(|| (members.len(), BTreeSet::new()));
                    if *arity == members.len() {
                        failing.extend(
                            members
                                .iter()
                                .enumerate()
                                .filter(|(_, member)| !proves(member))
                                .map(|(index, _)| index),
                        );
                    }
                }
            }
        }
        if rejected.is_empty() {
            // Previously supplied source certificates can carry exact axiom
            // indexes. Optional inference must not make them stale. If a changed
            // question cannot replay its retained certificate, keep the original
            // module rather than silently discard its source-derived evidence.
            // `validated` covers this module state: a rejected-empty round
            // reaches here without the member pruning below having run.
            if !retained_evidence::replays(validated, original, &questions, &lowered.proof_bundle)?
            {
                if restore_seeds(&mut module.scalar_block_invariants, &mut seeds) {
                    continue;
                }
                module.scalar_block_invariants.clear();
            }
            return Ok(());
        }
        let narrowed = prune_refuted_members(
            &mut module.scalar_block_invariants,
            &refuted,
            mandatory,
            &mut rejected,
        );
        let restored = restore_authored_rows(
            &mut module.scalar_block_invariants,
            mandatory,
            &mut rejected,
        );
        if (narrowed || restored) && rejected.is_empty() {
            // The surviving members are new proposals: prove every arrival again.
            continue;
        }
        // Keep independently surviving headers: one rejected outer-loop guess
        // must not erase discovered inner-loop bounds. Reconstructing below
        // rechecks survivors without the removed hypotheses. Retry the original
        // seed group only when the expanded group has no surviving header;
        // unextended authored rows are never judged and do not count.
        if module
            .scalar_block_invariants
            .iter()
            .filter(|candidate| !mandatory.contains(candidate))
            .all(|candidate| rejected.contains(&(candidate.machine, candidate.header)))
            && restore_seeds(&mut module.scalar_block_invariants, &mut seeds)
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

/// A header's proposals are conjoined, so one non-inductive guess would
/// discard every useful member beside it. Drop exactly the members that fail
/// alone at some arrival and keep the header; a later round proves the
/// surviving conjunction again, since a survivor may have leaned on a dropped
/// member. A header whose every member fails, or whose failure is only joint,
/// stays rejected. Returns whether any header was narrowed.
///
/// Keep every independently surviving member, including carrier bounds. An
/// authored contract may contain those bounds: dropping them as redundant
/// makes mandatory-roster preservation roll back unrelated useful inference.
/// This is proof pruning, not contract simplification. For the same reason an
/// authored member is never pruned, even when it fails alone; a header whose
/// only failing members are authored stays rejected for
/// `restore_authored_rows`.
fn prune_refuted_members(
    roster: &mut [terminal_psi::ScalarBlockInvariant],
    refuted: &BTreeMap<
        (semantic_vocabulary::MachineId, semantic_vocabulary::BlockId),
        (usize, BTreeSet<usize>),
    >,
    mandatory: &[terminal_psi::ScalarBlockInvariant],
    rejected: &mut BTreeSet<(semantic_vocabulary::MachineId, semantic_vocabulary::BlockId)>,
) -> bool {
    let mut pruned = false;
    for candidate in roster {
        let key = (candidate.machine, candidate.header);
        let Some((arity, failing)) = refuted.get(&key) else {
            continue;
        };
        let Proposition::Conjunction(members) = &candidate.predicate else {
            continue;
        };
        if members.len() != *arity || failing.is_empty() {
            continue;
        }
        let authored = authored_row(mandatory, candidate.machine, candidate.header)
            .map(|row| self::members(&row.predicate))
            .unwrap_or_default();
        let mut surviving = members
            .iter()
            .enumerate()
            .filter(|(index, member)| !failing.contains(index) || authored.contains(member))
            .map(|(_, member)| member.clone())
            .collect::<Vec<_>>();
        if surviving.is_empty() || surviving.len() == members.len() {
            continue;
        }
        candidate.predicate = if surviving.len() == 1 {
            surviving.pop().expect("one surviving member")
        } else {
            Proposition::Conjunction(surviving)
        };
        rejected.remove(&key);
        pruned = true;
    }
    pruned
}

/// A rejected header that extends an authored row falls back to exactly the
/// authored predicate. Its inferred members are dropped together; its
/// authored members are not this pass's to remove. The next round no longer
/// judges the row, so the fallback cannot loop.
fn restore_authored_rows(
    roster: &mut [terminal_psi::ScalarBlockInvariant],
    mandatory: &[terminal_psi::ScalarBlockInvariant],
    rejected: &mut BTreeSet<(semantic_vocabulary::MachineId, semantic_vocabulary::BlockId)>,
) -> bool {
    let mut restored = false;
    for candidate in roster {
        let key = (candidate.machine, candidate.header);
        if !rejected.contains(&key) {
            continue;
        }
        let Some(authored) = authored_row(mandatory, candidate.machine, candidate.header) else {
            continue;
        };
        *candidate = authored.clone();
        rejected.remove(&key);
        restored = true;
    }
    restored
}

/// Every actual edge into `header`, in canonical order: one arrival
/// obligation each, whichever pass proposed the row.
pub(super) fn header_arrival_edges(
    machine: &terminal_psi::TerminalMachine,
    header: semantic_vocabulary::BlockId,
) -> Vec<semantic_vocabulary::EdgeId> {
    let mut edges = Vec::new();
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump { edge, target, .. } if *target == header => edges.push(*edge),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => edges.extend(
                [when_true, when_false]
                    .into_iter()
                    .filter(|successor| successor.target == header)
                    .map(|successor| successor.edge),
            ),
            _ => {}
        }
    }
    edges.sort();
    edges
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
            crate::proofs::contract_predicates::canonical_equality(left, right)?
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
        result = crate::proofs::contract_predicates::connective(
            result,
            canonical_predicate(member)?,
            conjunction,
        )?;
    }
    Ok(result)
}
