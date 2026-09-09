//! Propose entry ranges as loop invariants only after proving every arrival.
//!
//! Entry requirements are candidates, not current-iteration facts. This optional
//! producer pass drops any candidate whose exact establishment or preservation
//! question cannot be proved. Ordinary finalization then produces certificates
//! for the retained roster and all operation questions; independent verification
//! checks both before granting execution authority. No ranking premise is used.

use std::collections::BTreeSet;

use semantic_vocabulary::ObligationId;
use terminal_psi::{ScalarRangeInvariantArrival, Terminator};
use terminal_verifier::ReconstructedTerminalObligationOwner;

use crate::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
use crate::{LoweredPsi, LoweringError};

mod entry_ranges;
mod retained_evidence;

pub(super) fn retain_provable(lowered: &mut LoweredPsi) -> Result<(), LoweringError> {
    let module = &mut lowered.semantic_module;
    if !module.scalar_range_invariants.is_empty() {
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
    let candidates = entry_ranges::candidates(module);
    if candidates.is_empty() {
        return Ok(());
    }
    let original = terminal_verifier::reconstruct_terminal_obligations(module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut next = terminal_verifier::maximum_registered_obligation_id(module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    for mut candidate in candidates {
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
                module.scalar_range_invariants.clear();
                return Ok(());
            };
            next = fresh;
            candidate.arrivals.push(ScalarRangeInvariantArrival {
                edge,
                obligation: ObligationId::new(next).ok_or(LoweringError::Unsupported(
                    "scalar invariant obligation identity is zero",
                ))?,
            });
        }
        module.scalar_range_invariants.push(candidate);
    }
    loop {
        // A candidate may not name the verifier's canonical feedback header.
        // The original module was validated above; optional inference must not
        // turn an unsupported candidate shape into a new program restriction.
        let Ok(validated) = terminal_verifier::validate_module(module) else {
            module.scalar_range_invariants.clear();
            return Ok(());
        };
        let questions = terminal_verifier::reconstruct_terminal_obligations(module)
            .map_err(LoweringError::InvalidTerminalModule)?;
        let mut rejected = BTreeSet::new();
        for site in questions.obligations() {
            let ReconstructedTerminalObligationOwner::ScalarRangeInvariant {
                machine,
                header,
                parameter,
                ..
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
                rejected.insert((machine, header, parameter));
            }
        }
        if rejected.is_empty() {
            // Previously supplied source certificates can carry exact axiom
            // indexes. Optional inference must not make them stale. If a changed
            // question cannot replay its retained certificate, keep the original
            // module rather than silently discard its source-derived evidence.
            if !retained_evidence::replays(module, &original, &questions, &lowered.proof_bundle)? {
                module.scalar_range_invariants.clear();
            }
            return Ok(());
        }
        module.scalar_range_invariants.retain(|candidate| {
            !rejected.contains(&(candidate.machine, candidate.header, candidate.parameter))
        });
        if module.scalar_range_invariants.is_empty() {
            return Ok(());
        }
        // A surviving proof may have used a removed candidate. Reconstruct and
        // prove again under only the remaining hypotheses before retaining it.
    }
}
