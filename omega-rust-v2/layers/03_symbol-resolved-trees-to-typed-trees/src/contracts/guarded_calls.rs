//! Follow a guarded callback through forwarding calls and retain the guard, its operands and the call that supplies the implementation.

use crate::contracts::{callback_invocations, effect_propagation::ExecutionCalls};
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::{CrashRouteRef, GuardedCall, GuardedInvocationRef, InvocationTarget, TypedStage};
use std::collections::{HashMap, HashSet, VecDeque};

pub(crate) fn record_guarded_calls(
    graph: &ExecutionCalls,
    library_graph: Option<&ExecutionCalls>,
    catalog: &DeclarationCatalog<'_>,
    out: &mut TypedStage,
) {
    let library = out.library;
    let mut routes = HashMap::<&crate::Site, Vec<CrashRouteRef>>::new();
    for (index, route) in out.crash_routes.iter().enumerate() {
        routes
            .entry(&route.contract)
            .or_default()
            .push(CrashRouteRef::Local(index));
    }
    if let Some(library) = library {
        for (index, route) in library.crash_routes.iter().enumerate() {
            routes
                .entry(&route.contract)
                .or_default()
                .push(CrashRouteRef::Library(index));
        }
    }
    if routes.is_empty() {
        return;
    }

    let imported = library.zip(library_graph);
    let invocations = graph
        .indirect
        .iter()
        .copied()
        .map(GuardedInvocationRef::Local)
        .chain(
            library_graph
                .into_iter()
                .flat_map(|graph| graph.indirect.iter().copied())
                .map(GuardedInvocationRef::Library),
        );
    for invocation in invocations {
        let edge = match invocation {
            GuardedInvocationRef::Local(index) => &out.indirect_calls[index],
            GuardedInvocationRef::Library(index) => &library.unwrap().indirect_calls[index],
        };
        let Some(caller) = edge.caller.as_ref() else { continue };
        let mut queue = VecDeque::from([(caller, edge.entry.clone())]);
        let mut visited = HashSet::new();
        while let Some((owner, entry)) = queue.pop_front() {
            if !matches!(entry, InvocationTarget::StaticParameter { .. } | InvocationTarget::RuntimeParameter { .. }) || !visited.insert((owner, entry.clone()))
            {
                continue;
            }
            let local_callers = graph
                .callers_of(owner)
                .iter()
                .map(|&index| (Some(index), &out.calls[index]));
            // Library-to-library specializations already live in the checked library.
            // Still follow its forwarding edges to reach specializations in this forest.
            let library_callers = imported.into_iter().flat_map(|(library, graph)| {
                graph
                    .callers_of(owner)
                    .iter()
                    .map(move |&index| (None, &library.calls[index]))
            });
            for (specialization, call) in local_callers.chain(library_callers) {
                let Some(parent) = call.caller.as_ref() else { continue };
                let supplied = callback_invocations::through(call, &entry);
                let declaration = match &supplied {
                    InvocationTarget::Declaration(slot) => Some(&slot.declaration),
                    InvocationTarget::AppliedMachine { declaration, .. } => Some(declaration),
                    _ => None,
                };
                if let Some(declaration) = declaration {
                    let (Some(specialization), Some(signature)) = (specialization, catalog.signatures.get(declaration)) else {
                        continue;
                    };
                    for &route in routes.get(&signature.site).into_iter().flatten() {
                        out.guarded_calls.push(GuardedCall::Indirect {
                            call: invocation,
                            route,
                            specialization: Some(specialization),
                        });
                    }
                } else {
                    queue.push_back((parent, supplied));
                }
            }
        }
        if matches!(invocation, GuardedInvocationRef::Local(_)) {
            for &route in routes.get(&edge.contract.contract).into_iter().flatten() {
                out.guarded_calls.push(GuardedCall::Indirect {
                    call: invocation,
                    route,
                    specialization: None,
                });
            }
        }
    }
    for &index in &graph.direct {
        let call = &out.calls[index];
        if call.caller.is_none() {
            continue;
        }
        for &route in routes
            .get(&catalog.signatures[&call.machine].site)
            .into_iter()
            .flatten()
        {
            out.guarded_calls
                .push(GuardedCall::Direct { call: index, route });
        }
    }
}
