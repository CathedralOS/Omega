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
published route for the cause, restated over the call's actuals with the
binding its `requires` clauses use (`at_call_actuals`), so a guard over a
formal's field or element reads that storage of the actual when the call is
made ("borrowed observations still use their appropriate storage
revisions", fact_validity.md). The call
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

The coverage is the machine's own published route, a clause over its
invocation entry, so it is restated over this machine's entry storage
(`Storage::at_entry`) in the form its `requires` premises take
(`entry_goal`): an element no operation reads keeps its form over the formal.
The call's route is a premise and keeps plain `substitute`, which refuses
what it cannot state: dropping one of its alternatives would assume the call
crashes less often than its callee publishes.

A Trapping operation (`TrappingInteger`) is its own `Trap` site
(`wiki/spec/language/numeric_values.md`, "Overflow policies"): it crashes
exactly where the Exact primitive it names is undefined. Its guard is that
negated condition, which Terminal semantics states. The goal is the
implication from the guard to the coverage, stated as "the primitive is
defined, or the coverage holds": a machine that publishes no `Trap` route
owes the definedness itself ("Path facts that disprove the predicate remove
the route", "Policy operators"), and an unconditional route owes nothing.
*/

use arena::IdTable;
use semantic_vocabulary::{
    BlockId, CrashCause, InvocationSite, MachineId, ObligationId, OperationId, Proposition,
    ScalarType, ValueId,
};
use terminal_check_tables::obligations::CrashSite;
use terminal_check_tables::{Checked, Construct, MachineTables, RefusalReason};
use terminal_psi::{
    CrashRouteBucket, CrashRouteGuard, TerminalMachine, TerminalModule, Terminator,
};

use crate::places::Storage;
use crate::refusal;
use terminal_semantics::descriptor_results::DescriptorAnalysis;

/// The storage a crash goal reads, and the descriptor results it reads with.
type Observed<'s, 'a> = (&'s Storage<'a>, &'s mut DescriptorAnalysis<'a>);

#[allow(clippy::too_many_arguments)]
pub(crate) fn coverage_goal<'a>(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    (storage, references): Observed<'_, 'a>,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
    id: MachineId,
    obligation: ObligationId,
    site: CrashSite,
    cause: CrashCause,
) -> Checked<Proposition> {
    let concerns = Construct::Obligation(obligation);
    let guard = match site {
        CrashSite::Block(block) => block_site(
            module,
            lowered,
            (storage, &mut *references),
            tables,
            id,
            concerns,
            block,
            cause,
        )?,
        CrashSite::Operation(operation) => operation_site(
            module,
            lowered,
            (storage, &mut *references),
            tables,
            types,
            id,
            concerns,
            operation,
            cause,
        )?,
    };
    let coverage = crate::entry_goal(
        &coverage(&lowered.contract.crash_routes, cause),
        &mut storage.at_entry(references, tables),
    )
    .map_err(|unbound| unbound.refusal(id, concerns))?;
    Ok(compose_goal(guard, coverage))
}

fn compose_goal(guard: Guard, coverage: Proposition) -> Proposition {
    match guard {
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
    }
}

/// What a crash site's guard is.
#[derive(Clone)]
enum Guard {
    /// A `Crash` terminator's clauses: path decisions known before it.
    Path(Vec<Proposition>),
    /// A call's: the callee's route for the cause, with the call's actuals.
    Route(Proposition),
    /// A Trapping operation's: it crashes unless this holds.
    Unless(Proposition),
}

/// A callee's guard for `cause`: the disjunction of its published
/// alternatives, when it may crash with that cause at all.
fn route(routes: &[CrashRouteBucket], cause: CrashCause) -> Option<Proposition> {
    routes
        .iter()
        .any(|bucket| bucket.cause == cause)
        .then(|| coverage(routes, cause))
}

/// A trapping primitive supplies its definedness guard; other operations
/// use their exact invocation row and public crash contract.
#[allow(clippy::too_many_arguments)]
fn operation_site<'a>(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    observed: Observed<'_, 'a>,
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
        .find(|found| found.id == operation)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                concerns,
                "crash coverage names no operation in this machine",
            )
        })?;
    if let Some(semantics) = terminal_semantics::trapping_integer_semantics(found, types)
        .map_err(crate::semantics_refusal(id, found.id))?
    {
        if cause != CrashCause::Trap {
            return Err(refusal(
                RefusalReason::ForeignIdentity,
                id,
                concerns,
                "a trapping primitive has only the Trap crash cause",
            ));
        }
        return semantics
            .completes()
            .kernel_proposition()
            .map(Guard::Unless)
            .map_err(crate::semantics_refusal(id, found.id));
    }
    call_site(
        module,
        lowered,
        observed,
        tables,
        id,
        concerns,
        InvocationSite::Operation(operation),
        cause,
    )
}

/// Use the same actual invocation, public contract and actual binding as call
/// requirements. The call's retained crash continuations are checked against
/// that contract when the call is admitted (`minted`); the route is judged
/// from the contract itself, where each storage observation can be read.
#[allow(clippy::too_many_arguments)]
fn call_site<'a>(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    observed: Observed<'_, 'a>,
    tables: &MachineTables,
    id: MachineId,
    concerns: Construct,
    site: InvocationSite,
    cause: CrashCause,
) -> Checked<Guard> {
    let refuse = || {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the invocation is not a call that may crash with this cause",
        )
    };
    let mut rows = tables
        .invocations
        .iter()
        .filter(|(_, row)| row.site == site);
    let (Some((invocation, row)), None) = (rows.next(), rows.next()) else {
        return Err(refuse());
    };
    let call = crate::call_contracts::invocation(module, lowered, tables, invocation, row)?;
    let guard = route(call.contract.crash_routes, cause).ok_or_else(refuse)?;
    crate::at_call_actuals(
        observed,
        tables,
        (invocation, row, &call),
        id,
        concerns,
        |bindings| crate::substitute(&guard, bindings),
    )
    .map(Guard::Route)
}

/// The guard of a block site: a `Crash` terminator's clauses, or a no-normal
/// call's callee route.
#[allow(clippy::too_many_arguments)]
fn block_site<'a>(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    observed: Observed<'_, 'a>,
    tables: &MachineTables,
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
        Terminator::InvokeNoNormal { edge, .. } => call_site(
            module,
            lowered,
            observed,
            tables,
            id,
            concerns,
            InvocationSite::InvokeNoNormal(*edge),
            cause,
        ),
        Terminator::ExternalCompletion { edge, .. } => call_site(
            module,
            lowered,
            observed,
            tables,
            id,
            concerns,
            InvocationSite::ExternalCompletion(*edge),
            cause,
        ),
        _ => Err(refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the site does not crash with this cause",
        )),
    }
}

/// The disjunction of the alternatives a machine's published `routes` hold
/// for `cause`: `Truth` when one is unconditional, `Falsehood` when there are
/// none.
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

/// Infer tighter proved crash guards for private machines that omitted `crashes`.
///
/// Stage 05 publishes each reached crash cause of an omitted private route as
/// `Truth`. For a private helper whose body calls a guarded callee (or a
/// constant initializer whose crashing call is unreachable under its actuals or
/// branch guards), inferring each cause's guard from its crash sites and
/// proving every site covered lets callers discharge the call without a blank
/// `Truth` route while keeping unproved routes refused.
pub(crate) fn infer(
    scope: &mut crate::ReplayScope<'_>,
    unreached: &mut crate::Unreached,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    let order = callees_first_inferred(scope.source())?;
    for id in order {
        narrow_machine(scope, id, unreached, inhabitation, activity)?;
    }
    Ok(())
}

/// Eligible private machines in postorder (callees before callers) so a
/// helper's narrowed route reaches its callers during the same pass.
fn callees_first_inferred(
    elaboration: &terminal_check_tables::program::Elaboration,
) -> Checked<Vec<MachineId>> {
    use terminal_check_tables::edges::Callee;
    use terminal_psi::TargetMachineCallable;

    let tables = &elaboration.tables.machines;
    let mut eligible = tables.map(|_, _| false);
    let mut roots = Vec::new();
    for machine in &elaboration.module.machines {
        if machine.contract.declared_crash_routes.is_none()
            && !machine.contract.crash_routes.is_empty()
            && !machine.blocks.is_empty()
            && machine
                .contract
                .crash_routes
                .iter()
                .any(|bucket| bucket.alternatives.contains(&CrashRouteGuard::Truth))
            && let Some(slot) = eligible.get_mut(machine.id)
        {
            *slot = true;
            roots.push(machine.id);
        }
    }
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let direct_callees = |id: MachineId| -> Checked<Vec<MachineId>> {
        let machine_tables = terminal_check_tables::row(tables, id, id, Construct::Machine)?;
        let mut called = Vec::new();
        for (_, row) in machine_tables.invocations.iter() {
            if let Callee::Declared(TargetMachineCallable::Machine(callee)) = row.callee
                && callee != id
                && !called.contains(&callee)
            {
                called.push(callee);
            }
        }
        Ok(called)
    };
    let mut seen = tables.map(|_, _| false);
    let mut order = Vec::with_capacity(roots.len());
    for root in roots {
        if seen.get(root) != Some(&false) {
            continue;
        }
        *seen.get_mut(root).expect("root is in tables") = true;
        let mut open = vec![(root, direct_callees(root)?)];
        while let Some((id, pending)) = open.last_mut() {
            let id = *id;
            match pending.pop() {
                Some(callee) => {
                    if let Some(reached) = seen.get_mut(callee)
                        && !std::mem::replace(reached, true)
                    {
                        open.push((callee, direct_callees(callee)?));
                    }
                }
                None => {
                    open.pop();
                    if eligible.get(id) == Some(&true) {
                        order.push(id);
                    }
                }
            }
        }
    }
    Ok(order)
}

fn has_direct_callers(
    elaboration: &terminal_check_tables::program::Elaboration,
    target: MachineId,
) -> bool {
    use terminal_check_tables::edges::Callee;
    use terminal_psi::TargetMachineCallable;
    elaboration
        .tables
        .machines
        .iter()
        .any(|(_, machine_tables)| {
            machine_tables.invocations.iter().any(|(_, row)| {
                matches!(
                    row.callee,
                    Callee::Declared(TargetMachineCallable::Machine(callee)) if callee == target
                )
            })
        })
}

fn site_call_continuations(
    lowered: &TerminalMachine,
    site: CrashSite,
) -> Option<&[CrashRouteBucket]> {
    use terminal_psi::OperationKind;
    match site {
        CrashSite::Operation(op_id) => lowered
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|op| op.id == op_id)
            .and_then(|op| match &op.kind {
                OperationKind::Call {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallUnit {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallStructural {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallMachine {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallDynamic {
                    crash_continuations,
                    ..
                } => Some(crash_continuations.as_slice()),
                _ => None,
            }),
        CrashSite::Block(block_id) => lowered
            .blocks
            .iter()
            .find(|block| block.id == block_id)
            .and_then(|block| match &block.terminator {
                Terminator::InvokeNoNormal {
                    crash_continuations,
                    ..
                } => Some(crash_continuations.as_slice()),
                _ => None,
            }),
    }
}

fn narrow_machine(
    scope: &mut crate::ReplayScope<'_>,
    id: MachineId,
    unreached: &mut crate::Unreached,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    use terminal_check_tables::obligations::ObligationKind;
    use terminal_psi::CrashPredicateTerm;

    let elaboration = scope.source();
    let module = &elaboration.module;
    let lowered = crate::terminal_machine(&module.machines, id, id)?;
    if lowered.contract.declared_crash_routes.is_some()
        || lowered.contract.crash_routes.is_empty()
        || lowered.blocks.is_empty()
    {
        return Ok(());
    }
    let Ok(entry_context) = lowered.entry_proposition_context() else {
        return Ok(());
    };
    let called = has_direct_callers(elaboration, id);

    let original =
        terminal_check_tables::row(&elaboration.tables.machines, id, id, Construct::Machine)?;
    let mut scratch = original.clone();
    let prepared = crate::prepare_with_inhabitation(
        scope.formed(),
        &elaboration.domains,
        id,
        &mut scratch,
        inhabitation,
    )?;
    let mut references = prepared.references(&module.types.structural_types);
    let storage = Storage::new(
        id,
        lowered,
        module,
        &elaboration.domains,
        &prepared.graph,
        &scratch,
        &prepared.types,
        &mut references,
    )?;
    let declared_joins = crate::joins::prepare(
        lowered,
        id,
        &mut scratch,
        &prepared.graph,
        &prepared.context,
        &module.proofs.scalar_block_invariants,
        &storage,
        &mut references,
        activity,
    )?;
    let beyond = unreached.values(lowered, &scratch);
    let mut decided = (0..scratch.obligations.len())
        .map(|_| None)
        .collect::<Vec<_>>();
    crate::joins::check(
        &crate::joins::Machine {
            module,
            lowered,
            storage: &storage,
            id,
            types: &prepared.types,
            graph: &prepared.graph,
            context: &prepared.context,
            ranking_bindings: &prepared.ranking_bindings,
            declared: &declared_joins,
            unreached: &beyond,
            law_instances: &[],
        },
        &mut scratch,
        &mut decided,
        activity,
        &mut references,
    )?;

    let mut sites = Vec::new();
    for (obligation, row) in scratch.obligations.iter() {
        if let ObligationKind::CrashCoverage { site, cause } = row.kind {
            let concerns = Construct::Obligation(obligation);
            let guard = match site {
                CrashSite::Block(block) => block_site(
                    module,
                    lowered,
                    (&storage, &mut references),
                    &scratch,
                    id,
                    concerns,
                    block,
                    cause,
                )?,
                CrashSite::Operation(operation) => operation_site(
                    module,
                    lowered,
                    (&storage, &mut references),
                    &scratch,
                    &prepared.types,
                    id,
                    concerns,
                    operation,
                    cause,
                )?,
            };
            sites.push((obligation, row.created_at, site, cause, guard));
        }
    }

    let mut alternatives: Vec<(CrashCause, CrashRouteGuard)> = Vec::new();
    for bucket in &lowered.contract.crash_routes {
        if !bucket.alternatives.contains(&CrashRouteGuard::Truth) {
            for guard in &bucket.alternatives {
                alternatives.push((bucket.cause, guard.clone()));
            }
            continue;
        }
        let mut cause_guards: Vec<CrashRouteGuard> = Vec::new();
        let mut keep_truth = false;
        for &(obligation, created_at, site, cause, ref guard) in &sites {
            if cause != bucket.cause {
                continue;
            }
            let impossible_goal = compose_goal(guard.clone(), Proposition::Falsehood);
            if crate::decide(
                &prepared.context,
                &prepared.graph,
                &scratch,
                id,
                obligation,
                created_at,
                impossible_goal,
                &beyond,
                activity,
            )
            .is_ok()
            {
                continue;
            }
            if let Some(continuations) = site_call_continuations(lowered, site)
                && let Some(call_bucket) = continuations.iter().find(|found| found.cause == cause)
                && !call_bucket.alternatives.is_empty()
                && call_bucket.alternatives.iter().all(|alt| match alt {
                    CrashRouteGuard::Predicate(predicate) => {
                        entry_context.validate(predicate.proposition()).is_ok()
                    }
                    CrashRouteGuard::Truth => false,
                })
            {
                cause_guards.extend(call_bucket.alternatives.iter().cloned());
                continue;
            }
            if let Guard::Path(parts) = guard
                && !parts.is_empty()
                && parts
                    .iter()
                    .all(|part| entry_context.validate(part).is_ok())
            {
                let combined = if parts.len() == 1 {
                    parts[0].clone()
                } else {
                    Proposition::Conjunction(parts.clone())
                };
                if entry_context.validate(&combined).is_ok() {
                    cause_guards.push(CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                        combined,
                    )));
                    continue;
                }
            }
            keep_truth = true;
            break;
        }
        if keep_truth {
            alternatives.push((bucket.cause, CrashRouteGuard::Truth));
        } else if cause_guards.is_empty() {
            if called {
                alternatives.push((
                    bucket.cause,
                    CrashRouteGuard::Predicate(CrashPredicateTerm::new(Proposition::Falsehood)),
                ));
            }
        } else {
            for guard in cause_guards {
                alternatives.push((bucket.cause, guard));
            }
        }
    }

    let candidate_routes = terminal_codec::contract_forms::routes(alternatives);
    if candidate_routes == lowered.contract.crash_routes {
        return Ok(());
    }
    let mut proposed = lowered.contract.clone();
    proposed.crash_routes = candidate_routes.clone();
    if crate::formation::contract(module, id, &proposed, activity).is_err() {
        return Ok(());
    }
    for &(obligation, created_at, _, cause, ref guard) in &sites {
        let Ok(cov) = crate::entry_goal(
            &coverage(&candidate_routes, cause),
            &mut storage.at_entry(&mut references, &scratch),
        ) else {
            return Ok(());
        };
        let goal = compose_goal(guard.clone(), cov);
        if crate::decide(
            &prepared.context,
            &prepared.graph,
            &scratch,
            id,
            obligation,
            created_at,
            goal,
            &beyond,
            activity,
        )
        .is_err()
        {
            return Ok(());
        }
    }
    scope.publish_crash_routes(id, candidate_routes, activity)
}

pub(crate) fn publish(
    elaboration: &mut terminal_check_tables::program::Elaboration,
    id: MachineId,
    routes: Vec<CrashRouteBucket>,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    use terminal_psi::OperationKind;

    let lowered = crate::terminal_machine(&elaboration.module.machines, id, id)?;
    let mut proposed = lowered.contract.clone();
    proposed.crash_routes = routes.clone();
    crate::formation::contract(&elaboration.module, id, &proposed, activity)?;

    // Recompute every caller's substituted crash continuations before mutating
    // the module so any un-substitutable call leaves the module untouched.
    enum CallerSite {
        Operation {
            machine: usize,
            block: usize,
            operation: usize,
            routes: Vec<CrashRouteBucket>,
        },
        Terminator {
            machine: usize,
            block: usize,
            routes: Vec<CrashRouteBucket>,
        },
    }
    let mut updates = Vec::new();
    for (machine_idx, caller) in elaboration.module.machines.iter().enumerate() {
        let Some(caller_tables) = elaboration.tables.machines.get(caller.id) else {
            continue;
        };
        for (block_idx, block) in caller.blocks.iter().enumerate() {
            for (op_idx, op) in block.operations.iter().enumerate() {
                let (callee, arguments, structural_arguments): (
                    MachineId,
                    &[ValueId],
                    &[terminal_psi::StructuralArgument],
                ) = match &op.kind {
                    OperationKind::Call {
                        callee, arguments, ..
                    } => (*callee, arguments.as_slice(), &[]),
                    OperationKind::CallUnit {
                        callee,
                        arguments,
                        structural_arguments,
                        ..
                    }
                    | OperationKind::CallStructuralScalar {
                        callee,
                        arguments,
                        structural_arguments,
                        ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        callee,
                        arguments,
                        structural_arguments,
                        ..
                    } => (
                        *callee,
                        arguments.as_slice(),
                        structural_arguments.as_slice(),
                    ),
                    OperationKind::CallStructural {
                        callee,
                        structural_arguments,
                        ..
                    } => (*callee, &[], structural_arguments.as_slice()),
                    _ => continue,
                };
                if callee != id {
                    continue;
                }
                let places = terminal_codec::contract_forms::call_storage(
                    &elaboration.module.types.structural_types,
                    |place| caller.structural_place_type(place),
                    |value| caller_tables.values.get(value).map(|row| row.scalar_type),
                    structural_arguments,
                );
                let Ok(substituted) =
                    terminal_codec::contract_forms::substituted_routes(&routes, arguments, &places)
                else {
                    return Ok(());
                };
                updates.push(CallerSite::Operation {
                    machine: machine_idx,
                    block: block_idx,
                    operation: op_idx,
                    routes: substituted,
                });
            }
            if let Terminator::InvokeNoNormal {
                callee,
                arguments,
                structural_arguments,
                ..
            } = &block.terminator
                && *callee == id
            {
                let places = terminal_codec::contract_forms::call_storage(
                    &elaboration.module.types.structural_types,
                    |place| caller.structural_place_type(place),
                    |value| caller_tables.values.get(value).map(|row| row.scalar_type),
                    structural_arguments,
                );
                let Ok(substituted) =
                    terminal_codec::contract_forms::substituted_routes(&routes, arguments, &places)
                else {
                    return Ok(());
                };
                updates.push(CallerSite::Terminator {
                    machine: machine_idx,
                    block: block_idx,
                    routes: substituted,
                });
            }
        }
    }

    let position = usize::try_from(id.get() - 1).map_err(|_| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Machine,
            "inferred crash machine identity is out of range",
        )
    })?;
    let target_machine = elaboration
        .module
        .machines
        .get_mut(position)
        .filter(|machine| machine.id == id)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                "inferred crash machine is absent from the module",
            )
        })?;
    target_machine.contract.crash_routes = routes;

    for update in updates {
        match update {
            CallerSite::Operation {
                machine,
                block,
                operation,
                routes,
            } => {
                match &mut elaboration.module.machines[machine].blocks[block].operations[operation]
                    .kind
                {
                    OperationKind::Call {
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallUnit {
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallStructuralScalar {
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallStructural {
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        crash_continuations,
                        ..
                    } => *crash_continuations = routes,
                    _ => {}
                }
            }
            CallerSite::Terminator {
                machine,
                block,
                routes,
            } => {
                if let Terminator::InvokeNoNormal {
                    crash_continuations,
                    ..
                } = &mut elaboration.module.machines[machine].blocks[block].terminator
                {
                    *crash_continuations = routes;
                }
            }
        }
    }
    Ok(())
}
