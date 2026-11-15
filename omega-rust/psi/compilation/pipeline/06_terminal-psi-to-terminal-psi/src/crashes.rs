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
operation whose callee may return and crash) the guard is the callee's
published route for the cause, substituted with the call's actuals: the call
crashes with that cause only when one of its alternatives holds ("Entry-only
call-ceiling coverage proves the same-cause union of published
alternatives", fact_validity.md). Unlike a `Crash` terminator's guard it is
not a path decision, so it is not among the facts before the call, and the
goal is the implication from that route to the caller's coverage. When the
caller's own requirement contradicts the route (`requires d != 0` against a
callee route `d == 0`), the implication holds however little the caller
publishes, `crashes none` included: the call never crashes with that cause.
An unconditional callee route is `Truth`, and the goal is the coverage
itself.

A Trapping operation (`TrappingInteger`) is its own `Trap` site
(`wiki/spec/language/numeric_values.md`, "Overflow policies"): it crashes
exactly where the Exact primitive it names is undefined. Its guard is that
negated condition, which Terminal semantics states. The goal is the
implication from the guard to the coverage, stated as "the primitive is
defined, or the coverage holds": a machine that publishes no `Trap` route
owes the definedness itself ("Path facts that disprove the predicate remove
the route", "Policy operators"), and an unconditional route owes nothing.
*/

use semantic_vocabulary::{
    BlockId, CrashCause, IdTable, MachineId, ObligationId, OperationId, Proposition, ScalarType,
    ValueId,
};
use terminal_check_tables::obligations::CrashSite;
use terminal_check_tables::{Checked, Construct, MachineTables, RefusalReason};
use terminal_psi::{
    CrashRouteBucket, CrashRouteGuard, TargetMachineCallable, TerminalMachine, TerminalModule,
    Terminator,
};

use crate::refusal;

#[allow(clippy::too_many_arguments)]
pub(crate) fn coverage_goal(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
    id: MachineId,
    obligation: ObligationId,
    site: CrashSite,
    cause: CrashCause,
) -> Checked<Proposition> {
    let concerns = Construct::Obligation(obligation);
    let guard = match site {
        CrashSite::Block(block) => block_site(lowered, id, concerns, block, cause)?,
        CrashSite::Operation(operation) => operation_site(
            module, lowered, tables, types, id, concerns, operation, cause,
        )?,
    };
    let coverage = coverage(&lowered.contract.crash_routes, cause);
    Ok(match guard {
        Guard::Unless(completes) => match coverage {
            Proposition::Truth => Proposition::Truth,
            Proposition::Falsehood => completes,
            coverage => Proposition::Disjunction(vec![completes, coverage]),
        },
        Guard::Path(mut parts) => {
            if coverage != Proposition::Truth {
                parts.push(coverage);
            }
            match parts.len() {
                0 => Proposition::Truth,
                1 => parts.remove(0),
                _ => Proposition::Conjunction(parts),
            }
        }
        Guard::Route(route) => match (route, coverage) {
            (_, Proposition::Truth) => Proposition::Truth,
            (Proposition::Truth, coverage) => coverage,
            (route, coverage) => Proposition::Implication {
                premise: Box::new(route),
                conclusion: Box::new(coverage),
            },
        },
    })
}

/// What a crash site's guard is.
enum Guard {
    /// A `Crash` terminator's clauses: path decisions known before it.
    Path(Vec<Proposition>),
    /// A call's: the callee's route for the cause, with the call's actuals.
    Route(Proposition),
    /// A Trapping operation's: it crashes unless this holds.
    Unless(Proposition),
}

fn has_cause(continuations: &[CrashRouteBucket], cause: CrashCause) -> bool {
    continuations.iter().any(|bucket| bucket.cause == cause)
}

/// A call's guard for `cause`: the disjunction of its continuations'
/// alternatives, when it may crash with that cause at all.
fn route(continuations: &[CrashRouteBucket], cause: CrashCause) -> Option<Proposition> {
    has_cause(continuations, cause).then(|| coverage(continuations, cause))
}

/// An operation that may crash with `cause`: a call, whose guard is the
/// callee's route, or a Trapping operation, which crashes unless its Exact
/// primitive is defined.
#[allow(clippy::too_many_arguments)]
fn operation_site(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
    id: MachineId,
    concerns: Construct,
    operation: OperationId,
    cause: CrashCause,
) -> Checked<Guard> {
    let found = lowered
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|candidate| candidate.id == operation);
    let refuse = || {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the operation is not a call that may crash with this cause",
        )
    };
    if let Some(found) = found
        && let Some(semantics) = terminal_semantics::trapping_integer_semantics(found, types)
            .map_err(crate::semantics_refusal(id, found.id))?
    {
        if cause != CrashCause::Trap {
            return Err(refuse());
        }
        return semantics
            .completes()
            .kernel_proposition()
            .map(Guard::Unless)
            .map_err(crate::semantics_refusal(id, found.id));
    }
    let inputs = found
        .and_then(|operation| operation.declared_invocation())
        .ok_or_else(refuse)?;
    let guard = match inputs.callee {
        TargetMachineCallable::Machine(_) => {
            route(inputs.crash_continuations, cause).ok_or_else(refuse)?
        }
        TargetMachineCallable::Boundary(boundary) => {
            // Interface calls keep their ceiling on the declaration. Bind it
            // through the same scalar substitution as call requirements; an
            // unavailable actual must not turn a possible crash into none.
            let declaration = module.boundary_machine(boundary).ok_or_else(refuse)?;
            let guard = route(&declaration.crash_routes, cause).ok_or_else(refuse)?;
            crate::substitute(
                &guard,
                &mut crate::ScalarActuals {
                    actuals: inputs.arguments,
                    result: None,
                    tables,
                },
            )
            .map_err(|unbound| unbound.refusal(id, concerns))?
        }
    };
    Ok(Guard::Route(guard))
}

/// The guard of a block site: a `Crash` terminator's clauses, or a no-normal
/// call's callee route.
fn block_site(
    lowered: &TerminalMachine,
    id: MachineId,
    concerns: Construct,
    site: BlockId,
    cause: CrashCause,
) -> Checked<Guard> {
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
        } if *crashes == cause => Ok(Guard::Path(
            site_guard
                .iter()
                .map(|term| term.proposition().clone())
                .collect(),
        )),
        Terminator::InvokeNoNormal {
            crash_continuations,
            ..
        } if has_cause(crash_continuations, cause) => {
            Ok(Guard::Route(coverage(crash_continuations, cause)))
        }
        _ => Err(refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the site does not crash with this cause",
        )),
    }
}

/// The disjunction of the alternatives `routes` holds for `cause` (a
/// machine's published routes, or a call's crash continuations): `Truth`
/// when one is unconditional, `Falsehood` when there are none.
fn coverage(routes: &[CrashRouteBucket], cause: CrashCause) -> Proposition {
    let published = routes.iter().find(|bucket| bucket.cause == cause);
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
