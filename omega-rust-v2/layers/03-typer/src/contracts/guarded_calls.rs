//! Follow a guarded callback through forwarding calls and retain the guard, its operands and the call that supplies the implementation.

use crate::contracts::callback_invocations;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{CallUse, GuardedCall, InvocationTarget, TypeError, TypedStage, ValueType};
use omega_resolver::DeclarationId;
use std::collections::{HashMap, HashSet};

pub(crate) fn record_guarded_calls(
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut routes = HashMap::<&crate::Site, Vec<&crate::CrashRoute>>::new();
    for route in &out.crash_routes {
        routes.entry(&route.contract).or_default().push(route);
    }
    let effects: HashMap<_, _> = out.machine_effects.iter().map(|e| (&e.machine, e)).collect();
    let files = units
        .iter()
        .map(|unit| ((unit.file.package, unit.file.path), unit))
        .collect();
    let mut callers = HashMap::<&DeclarationId, Vec<&crate::TypedCall>>::new();
    for call in &out.calls {
        if call.use_site == CallUse::Execution && call.caller.is_some() {
            callers.entry(&call.machine).or_default().push(call);
        }
    }
    for edge in &out.indirect_calls {
        let Some(caller) = edge.caller.as_ref() else { continue };
        if edge.guarantee {
            continue;
        }
        if edge.use_site != CallUse::Execution {
            if edge.contract.pause.suspend || edge.contract.pause.block || edge.contract.reach.as_ref().is_none_or(|r| !r.is_empty()) {
                return Err(TypeError {
                    package: edge.site.package.clone(),
                    file: edge.site.file.clone(),
                    span: edge.site.span,
                    message: "fact requires an admitted effect-free callable".into(),
                });
            }
            continue;
        }
        let mut queue = std::collections::VecDeque::from([(caller, edge.entry.clone())]);
        let mut visited = HashSet::new();
        while let Some((owner, entry)) = queue.pop_front() {
            if !matches!(entry, InvocationTarget::StaticParameter { .. } | InvocationTarget::RuntimeParameter { .. }) || !visited.insert((owner, entry.clone()))
            {
                continue;
            }
            for call in callers.get(owner).into_iter().flatten() {
                let Some(parent) = call.caller.as_ref() else { continue };
                let supplied = callback_invocations::through(call, &entry, &files, catalog);
                if let InvocationTarget::Declaration(slot) = &supplied {
                    let Some(signature) = catalog.signatures.get(&slot.declaration) else {
                        continue;
                    };
                    for route in routes.get(&signature.site).into_iter().flatten() {
                        out.guarded_calls.push(GuardedCall {
                            call: edge.site.clone(),
                            specialization: Some(call.site.clone()),
                            caller: parent.clone(),
                            guard_contract: route.contract.clone(),
                            guard: route.guard.clone(),
                            cause: route.cause,
                            operands: edge.operands.clone(),
                            generic_arguments: call.type_arguments.clone(),
                            entry: Some(edge.entry.clone()),
                        });
                    }
                } else {
                    queue.push_back((parent, supplied));
                }
            }
        }
        for route in routes.get(&edge.contract.contract).into_iter().flatten() {
            out.guarded_calls.push(GuardedCall {
                call: edge.site.clone(),
                specialization: None,
                caller: caller.clone(),
                guard_contract: route.contract.clone(),
                guard: route.guard.clone(),
                cause: route.cause,
                operands: edge.operands.clone(),
                generic_arguments: vec![],
                entry: Some(edge.entry.clone()),
            });
        }
    }
    for call in &out.calls {
        if call.use_site != CallUse::Execution {
            let e = effects[&call.machine];
            if e.pause.suspend || e.pause.block || !e.reach.is_empty() {
                return Err(TypeError {
                    package: call.site.package.clone(),
                    file: call.site.file.clone(),
                    span: call.site.span,
                    message: "fact or static argument requires an effect-free computation".into(),
                });
            }
            continue;
        }
        let Some(caller) = &call.caller else { continue };
        for route in routes
            .get(&catalog.signatures[&call.machine].site)
            .into_iter()
            .flatten()
        {
            out.guarded_calls.push(GuardedCall {
                call: call.site.clone(),
                specialization: None,
                caller: caller.clone(),
                guard_contract: route.contract.clone(),
                guard: route.guard.clone(),
                cause: route.cause,
                operands: call.operands.clone(),
                generic_arguments: call.type_arguments.clone(),
                entry: None,
            });
        }
    }
    for obligation in &out.obligations {
        let candidate = match &obligation.kind {
            crate::ObligationKind::SupplyHypothesis {
                evidence: ValueType::ProofCitation(machine),
                ..
            } => machine.as_ref(),
            crate::ObligationKind::RefineMathematicalCitation { machine, .. } => machine,
            _ => continue,
        };
        {
            let effect = effects[candidate];
            if effect.pause.suspend
                || effect.pause.block
                || !effect.reach.is_empty()
                || effect.crash_ceiling.as_ref().is_some_and(|c| !c.is_empty())
                || !effect.invokes.is_empty()
            {
                return Err(TypeError {
                    package: obligation.site.package.clone(),
                    file: obligation.site.file.clone(),
                    span: obligation.site.span,
                    message: "logical evidence cannot supply operational behavior".into(),
                });
            }
        }
    }
    Ok(())
}
