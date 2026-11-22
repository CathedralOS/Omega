//! Published crash routes and the typed evidence that each invocation keeps them.
//!
//! A route is a cause and an optional guard over the invocation's entry formals.
//! L5 retains the exact authored routes; it does not decide whether a guard holds
//! or whether a caller covers a callee's route. A call keeps its callee and its
//! ordered arguments, so coverage at that site is each callee route by index under
//! those arguments. Anything that would lose that correspondence is refused.

use super::contracts::{ClauseOwner, PredicateRole, lower_predicate};
use crate::lowering::applied_tree::LoweringTree;
use crate::{CrashRoute, LoweringError, site_at};
use omega_prepared::PreparedTrees;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{CallUse, CrashCause, CrashRouteRef, GuardedCall, NodeSemantics, ObligationKind, Site, TypedSignature, TypedStage};

/// Lower every `crashes` clause of `declaration` in authored order. Each route
/// must be the exact typed route of this contract, a guard must carry its total
/// entry obligation, and the causes must be exactly the published ceiling.
pub(crate) fn lower_routes(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    owner: ClauseOwner<'_>,
    declaration: usize,
    contract: &Site,
    ceiling: Option<&[CrashCause]>,
) -> Result<Vec<CrashRoute>, LoweringError> {
    // A contract's routes are published by the forest that declares it.
    let typed = tree.forest();
    let file = tree.resolved().file;
    let mut published: Vec<_> = typed
        .crash_routes
        .iter()
        .filter(|route| &route.contract == contract)
        .collect();
    let mut routes = Vec::new();
    for &clause in &file.nodes[declaration].children {
        if file.nodes[clause].kind != NodeKind::Crashes {
            continue;
        }
        let site = site_at(tree, clause);
        let bad = |message: &str| LoweringError::new(&site, message);
        let (&head, guards) = file.nodes[clause]
            .children
            .split_first()
            .ok_or_else(|| bad("crash clause has no cause"))?;
        if tree.semantics_at(clause).next().is_some() || (head..file.nodes[head].end).any(|n| tree.semantics_at(n).next().is_some()) {
            return Err(bad("crash clause has an unsupported typed record"));
        }
        let Some(position) = published.iter().position(|route| route.clause == site) else {
            // Only an unguarded `crashes none` publishes no route.
            if guards.is_empty() && matches!(tree.resolved().symbol(head), Some(Symbol::Builtin(name)) if name == "none") {
                continue;
            }
            return Err(bad("crash clause has no typed route"));
        };
        let route = published.remove(position);
        let guard = match (route.guard.as_ref(), guards) {
            (None, []) => None,
            (Some(guard_site), &[guard]) if &site_at(tree, guard) == guard_site => {
                let total = tree.semantics_at(guard).any(|row| {
                    matches!(row, NodeSemantics::Obligation(o)
                        if matches!(o.kind, ObligationKind::TotalCrashGuard) && &o.site == guard_site && o.node == file.node_id(guard))
                });
                if !total {
                    return Err(bad("crash guard lacks its total entry obligation"));
                }
                Some(lower_predicate(prepared, tree, owner, guard, guard_site.clone(), PredicateRole::CrashGuard)?)
            }
            _ => return Err(bad("crash clause disagrees with its typed guard")),
        };
        routes.push(CrashRoute {
            cause: route.cause,
            guard,
            site,
        });
    }
    if !published.is_empty() {
        return Err(LoweringError::new(&published[0].clause, "typed crash route has no authored clause"));
    }
    let agrees = match ceiling {
        None => routes.is_empty(),
        Some(causes) => {
            routes.iter().all(|route| causes.contains(&route.cause)) && causes.iter().all(|cause| routes.iter().any(|route| route.cause == *cause))
        }
    };
    if !agrees {
        return Err(LoweringError::new(contract, "crash ceiling and published crash routes disagree"));
    }
    Ok(routes)
}

/// Join L3's three coordinates before accepting a binder contribution. Each
/// supplying execution of this *exact application* must associate this local
/// invocation with every route of its selected contract, in published order.
/// Keep the row identities so other (e.g. forwarded or library) rows at a
/// supplier cannot be accidentally credited to the same instance.
pub(crate) fn closed_callback_routes(
    typed: &TypedStage,
    app: &omega_typed::MachineApplication,
    binding: &crate::lowering::callbacks::Callback<'_>,
    call: &omega_typed::IndirectCall,
) -> Result<(std::ops::Range<usize>, Vec<usize>), LoweringError> {
    let bad = || LoweringError::new(&call.site, "selected callback lacks exact local invocation route coverage");
    if call.use_site != CallUse::Execution || call.contract.crash_ceiling.is_some() {
        return Err(bad());
    }
    let invocation = typed.indirect_calls.iter().position(|edge| std::ptr::eq(edge, call)).ok_or_else(bad)?;
    let expected = published(typed, &binding.selection.site);
    let mut retained = Vec::new();
    for (supplier, _) in typed.calls.iter().enumerate().filter(|(_, c)| {
        c.use_site == CallUse::Execution && c.caller.is_some() && c.machine == *app.machine
            && c.type_arguments == app.arguments && c.enclosing_arguments == app.enclosing_arguments && c.evidence.is_none()
    }) {
        let rows = typed.guarded_calls.iter().enumerate().filter_map(|(i, row)| match *row {
            GuardedCall::Indirect { call: omega_typed::GuardedInvocationRef::Local(edge), specialization: Some(s), route }
                if edge == invocation && s == supplier => Some((i, route)),
            _ => None,
        });
        let (indices, routes): (Vec<_>, Vec<_>) = rows.unzip();
        if routes != expected { return Err(bad()); }
        retained.extend(indices);
    }
    if retained.is_empty() { return Err(bad()); }
    Ok((0..expected.len(), retained))
}

/// The caller owes coverage of each cause its callee publishes, once, at this
/// exact call. The Psi call keeps the callee and its ordered arguments, and the
/// callee contract keeps each route's guard over its formals, so the coverage
/// question survives with its operands. L5 does not answer it.
pub(crate) fn check_call_coverage(
    site: &Site,
    declared: Option<&[CrashCause]>,
    coverage: &[CrashCause],
) -> Result<(), LoweringError> {
    if coverage != declared.unwrap_or_default() {
        return Err(LoweringError::new(site, "call crash coverage disagrees with its callee's published routes"));
    }
    Ok(())
}

/// Check the typed invocation rows owned by `caller`. Each direct call must
/// name every published route of its callee exactly once, in route order. A
/// closed callback row is retained only by its matching instance dependency.
pub(crate) fn check_invocations(
    forest: &TypedStage,
    caller: &TypedSignature,
    retained: &std::collections::HashSet<usize>,
) -> Result<(), LoweringError> {
    check_routed_invocations(forest, caller, false, retained)
}

/// A template call keeps its exact callee declaration or binder and its
/// ordered operands. A route its binder's bound publishes stays on that
/// binder's lowered interface; a routed selection supplied here does not.
pub(crate) fn check_template_invocations(
    prepared: &PreparedTrees<'_>,
    template: &TypedSignature,
) -> Result<(), LoweringError> {
    check_routed_invocations(prepared.typed(), template, true, &Default::default())
}

fn check_routed_invocations(
    typed: &TypedStage,
    caller: &TypedSignature,
    bound_routes_retained: bool,
    retained: &std::collections::HashSet<usize>,
) -> Result<(), LoweringError> {
    check_callbacks(typed, &caller.machine, bound_routes_retained, retained)?;
    for (index, call) in typed.calls.iter().enumerate() {
        if call.caller.as_ref() != Some(&caller.machine) {
            continue;
        }
        let rows: Vec<_> = typed
            .guarded_calls
            .iter()
            .filter_map(|row| match *row {
                GuardedCall::Direct { call, route } if call == index => Some(route),
                _ => None,
            })
            .collect();
        if call.use_site != CallUse::Execution {
            if rows.is_empty() {
                continue;
            }
            return Err(LoweringError::new(&call.site, "a crash route names a call that does not execute"));
        }
        let expected = match callee_site(typed, &call.machine) {
            Some(site) => published(typed, site),
            None if rows.is_empty() => continue,
            None => return Err(LoweringError::new(&call.site, "a guarded call has no published callee contract")),
        };
        if rows != expected {
            return Err(LoweringError::new(&call.site, "call does not retain each published crash route of its callee"));
        }
    }
    Ok(())
}

fn check_callbacks(
    typed: &TypedStage,
    caller: &DeclarationId,
    bound_routes_retained: bool,
    retained: &std::collections::HashSet<usize>,
) -> Result<(), LoweringError> {
    for (index, row) in typed.guarded_calls.iter().enumerate() {
        if retained.contains(&index) { continue; }
        let GuardedCall::Indirect { call, specialization, .. } = *row else {
            continue;
        };
        let invocation = typed.guarded_invocation(call);
        let owner = match specialization {
            // The callable bound itself publishes the route.
            None if bound_routes_retained => continue,
            None => invocation.and_then(|edge| edge.caller.as_ref()),
            // The supplying call selected a routed callee.
            Some(index) => typed.calls.get(index).and_then(|supplier| supplier.caller.as_ref()),
        };
        if owner == Some(caller) {
            let site = specialization
                .and_then(|index| typed.calls.get(index))
                .map(|supplier| &supplier.site)
                .or(invocation.map(|edge| &edge.site));
            if let Some(site) = site {
                return Err(LoweringError::new(site, "a callback crash route requires indirect invocation coverage"));
            }
        }
    }
    Ok(())
}

fn callee_site<'a>(
    typed: &'a TypedStage,
    machine: &DeclarationId,
) -> Option<&'a Site> {
    let local = typed.signatures.iter().find(|sig| &sig.machine == machine);
    let library = || {
        typed
            .library
            .and_then(|library| library.signatures.iter().find(|sig| &sig.machine == machine))
    };
    local.or_else(library).map(|sig| &sig.site)
}

/// The routes typing attaches to every invocation of this contract, local first.
fn published(
    typed: &TypedStage,
    contract: &Site,
) -> Vec<CrashRouteRef> {
    let local = typed
        .crash_routes
        .iter()
        .enumerate()
        .filter(|(_, route)| &route.contract == contract)
        .map(|(index, _)| CrashRouteRef::Local(index));
    let library = typed.library.into_iter().flat_map(move |library| {
        library
            .crash_routes
            .iter()
            .enumerate()
            .filter(move |(_, route)| &route.contract == contract)
            .map(|(index, _)| CrashRouteRef::Library(index))
    });
    local.chain(library).collect()
}
