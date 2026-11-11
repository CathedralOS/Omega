/*
The goal of a crash site's `CrashCoverage` obligation.

`wiki/spec/terminal-psi/calls_and_outcomes.md`, "Crash": "The verifier proves
every site guard from independent entry/CFG facts and checks same-cause
coverage: the published route is `Truth`, or the guard set contains a
canonical predicate from that route." `wiki/spec/language/effects.md`: "For
each derived site guard `D`, the same-cause published guards `C_i` must
satisfy `D implies OR_i(C_i)`."

So the goal is the site guard's clauses together with the disjunction of the
machine's published alternatives for that cause, proved from the knowledge
available before the site, which includes the guard's own path decisions.
Proving the disjunction from everything known at the site proves the
implication from the guard. A machine that publishes no route for the cause
owes `Falsehood`: the site must be shown unreachable, which this prover does
not do, so such a site is unproved.

At a call site (an `InvokeNoNormal` terminator, or a `Call` or `CallUnit`
operation whose callee may return and crash) the callee's substituted route
would be one more hypothesis ("Entry-only call-ceiling coverage proves the same-cause union of
published alternatives", fact_validity.md). It is not added: proving the
caller's coverage without it is a stronger demand, so leaving it out can only
refuse, never accept wrongly. A call site has no guard of its own.
*/

use semantic_vocabulary::{BlockId, CrashCause, MachineId, ObligationId, OperationId, Proposition};
use terminal_check_tables::obligations::CrashSite;
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard, OperationKind, TerminalMachine, Terminator};

use crate::refusal;

pub(crate) fn coverage_goal(
    lowered: &TerminalMachine,
    id: MachineId,
    obligation: ObligationId,
    site: CrashSite,
    cause: CrashCause,
) -> Checked<Proposition> {
    let concerns = Construct::Obligation(obligation);
    let mut parts = match site {
        CrashSite::Block(block) => block_site(lowered, id, concerns, block, cause)?,
        CrashSite::Operation(operation) => operation_site(lowered, id, concerns, operation, cause)?,
    };
    let coverage = coverage(lowered, cause);
    if coverage != Proposition::Truth {
        parts.push(coverage);
    }
    Ok(match parts.len() {
        0 => Proposition::Truth,
        1 => parts.remove(0),
        _ => Proposition::Conjunction(parts),
    })
}

fn has_cause(continuations: &[CrashRouteBucket], cause: CrashCause) -> bool {
    continuations.iter().any(|bucket| bucket.cause == cause)
}

/// A call operation that may crash with `cause`: no guard of its own.
fn operation_site(
    lowered: &TerminalMachine,
    id: MachineId,
    concerns: Construct,
    operation: OperationId,
    cause: CrashCause,
) -> Checked<Vec<Proposition>> {
    let found = lowered
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|candidate| candidate.id == operation);
    match found.map(|found| &found.kind) {
        Some(
            OperationKind::Call {
                crash_continuations,
                ..
            }
            | OperationKind::CallUnit {
                crash_continuations,
                ..
            },
        ) if has_cause(crash_continuations, cause) => Ok(Vec::new()),
        _ => Err(refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the operation is not a call that may crash with this cause",
        )),
    }
}

/// The guard clauses of a block site, empty for a no-normal call.
fn block_site(
    lowered: &TerminalMachine,
    id: MachineId,
    concerns: Construct,
    site: BlockId,
    cause: CrashCause,
) -> Checked<Vec<Proposition>> {
    let block = usize::try_from(site.get() - 1)
        .ok()
        .and_then(|position| lowered.blocks.get(position))
        .filter(|block| block.id == site)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                concerns,
                "the crash site is past the end",
            )
        })?;
    match &block.terminator {
        Terminator::Crash {
            cause: crashes,
            site_guard,
            ..
        } if *crashes == cause => Ok(site_guard
            .iter()
            .map(|term| term.proposition().clone())
            .collect()),
        Terminator::InvokeNoNormal {
            crash_continuations,
            ..
        } if has_cause(crash_continuations, cause) => Ok(Vec::new()),
        _ => Err(refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the site does not crash with this cause",
        )),
    }
}

/// The disjunction of this machine's published alternatives for `cause`:
/// `Truth` when one is unconditional, `Falsehood` when it publishes none.
fn coverage(lowered: &TerminalMachine, cause: CrashCause) -> Proposition {
    let published = lowered
        .contract
        .crash_routes
        .iter()
        .find(|bucket| bucket.cause == cause);
    match published {
        None => Proposition::Falsehood,
        Some(bucket) => {
            let mut alternatives = Vec::with_capacity(bucket.alternatives.len());
            let mut unconditional = false;
            for alternative in &bucket.alternatives {
                match alternative {
                    CrashRouteGuard::Truth => unconditional = true,
                    CrashRouteGuard::Predicate(term) => {
                        alternatives.push(term.proposition().clone());
                    }
                }
            }
            if unconditional {
                Proposition::Truth
            } else {
                match alternatives.len() {
                    0 => Proposition::Falsehood,
                    1 => alternatives.remove(0),
                    _ => Proposition::Disjunction(alternatives),
                }
            }
        }
    }
}
