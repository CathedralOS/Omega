use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::{MachineEffects, PauseEffects, ServiceApplication, TypeError, TypedStage};
use std::collections::{BTreeMap, HashMap, HashSet};

pub(crate) fn check_machine_effects(
    catalog: &DeclarationCatalog<'_>,
    graph: &super::effect_propagation::ExecutionCalls,
    boundary_slots: &[crate::BoundarySlot],
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let formulas = derive_pause(catalog, out, graph)?;
    for &index in &graph.direct {
        let c = &out.calls[index];
        let specialized = specialize(c, &formulas, catalog).upper();
        let err = |m: &str| TypeError::for_site(&c.site, m);
        if specialized.suspend && !c.acknowledgements.suspend {
            return Err(err("call requires a suspend acknowledgement"));
        }
        if specialized.block && !c.acknowledgements.block {
            return Err(err("call requires a block acknowledgement"));
        }
        if let Some(caller) = &c.caller {
            let ceiling = catalog.signatures[caller].pause;
            if specialized.suspend && ceiling.suspend == Some(false) {
                return Err(err("call exceeds the machine's suspension ceiling"));
            }
            if specialized.block && ceiling.block == Some(false) {
                return Err(err("call exceeds the machine's blocking ceiling"));
            }
        }
    }
    let mut reach = catalog
        .signatures
        .iter()
        .map(|(&id, s)| (id, s.reach.iter().flatten().cloned().collect::<HashSet<_>>()))
        .collect::<HashMap<_, _>>();
    for imported in &out.machine_effects {
        reach
            .get_mut(&imported.machine)
            .unwrap()
            .extend(imported.reach.iter().cloned());
    }
    for &index in &graph.indirect {
        let call = &out.indirect_calls[index];
        let Some(caller) = &call.caller else { continue };
        if let Some(row) = &call.contract.reach {
            if let Some(ceiling) = &catalog.signatures[caller].reach
                && row.iter().any(|service| !ceiling.contains(service))
            {
                return Err(TypeError::for_site(&call.site, "callback exceeds the machine's service reach"));
            }
            reach.get_mut(caller).unwrap().extend(row.iter().cloned());
        }
        for &cause in call.contract.crash_ceiling.iter().flatten() {
            out.obligations.push(crate::TypingObligation {
                node: call.node,
                site: call.site.clone(),
                kind: crate::ObligationKind::CrashCoverage {
                    machine: caller.clone(),
                    cause,
                },
            });
        }
    }
    graph.propagate(
        &out.calls,
        &mut reach,
        "service reach inference exceeds implementation capacity",
        |call, caller, required| {
            let service = ServiceApplication {
                service: required.service.clone(),
                arguments: required.arguments.iter().map(|t| call.instantiate(t)).collect(),
            };
            for (refinement, owner) in catalog.evidence_refinements(call.evidence.as_ref()) {
                if let Some(allowed) = &refinement.reach
                    && !allowed.iter().any(|s| {
                        s.service == service.service
                            && s.arguments
                                .iter()
                                .map(|a| a.substitute(&owner.declaration, &owner.trait_arguments))
                                .eq(service.arguments.iter().cloned())
                    })
                {
                    return Ok(None);
                }
            }
            if let Some(bound) = &catalog.signatures[caller].reach {
                if !bound.contains(&service) {
                    return Err(TypeError::for_site(
                        &call.site,
                        format!("call exceeds service reach for {}", service.service.path),
                    ));
                }
                return Ok(None);
            }
            Ok(Some(service))
        },
    )?;
    for &index in &graph.direct {
        let call = &out.calls[index];
        if let Some(caller) = &call.caller
            && let Some(causes) = &catalog.signatures[&call.machine].crash_ceiling
        {
            for &cause in causes {
                out.obligations.push(crate::TypingObligation {
                    node: call.node,
                    site: call.site.clone(),
                    kind: crate::ObligationKind::CrashCoverage {
                        machine: caller.clone(),
                        cause,
                    },
                });
            }
        }
    }
    for obligation in &out.obligations {
        if let crate::ObligationKind::RefineCallable { contract, machine, arguments } = &obligation.kind {
            let pause = formulas[machine].upper();
            if pause.suspend && !contract.pause.suspend
                || pause.block && !contract.pause.block
                || contract.reach.as_ref().is_some_and(|row| {
                    reach[machine].iter().any(|s| {
                        !row.contains(&ServiceApplication {
                            service: s.service.clone(),
                            arguments: s
                                .arguments
                                .iter()
                                .map(|a| a.substitute(machine, arguments))
                                .collect(),
                        })
                    })
                })
            {
                return Err(TypeError::for_site(
                    &obligation.site,
                    "selected machine exceeds the callable's inferred effect bounds",
                ));
            }
        }
    }
    let mut invokes = super::callback_invocations::compose_callback_invocations(catalog, graph, boundary_slots, out)?;
    for &id in &catalog.local_machines {
        let mut row = reach.remove(id).unwrap().into_iter().collect::<Vec<_>>();
        row.sort_by_cached_key(|r| format!("{r:?}"));
        let mut entries: Vec<_> = invokes.remove(id).unwrap().into_iter().collect();
        entries.sort_by_cached_key(|e| format!("{e:?}"));
        out.machine_effects.push(std::sync::Arc::new(MachineEffects {
            invokes: entries,
            machine: id.clone(),
            pause: formulas[id].upper(),
            fixed_pause: formulas[id].fixed,
            callback_pause: formulas[id].binders.clone(),
            reach: row,
            crash_ceiling: catalog.signatures[id].crash_ceiling.clone(),
            terminates: catalog.signatures[id].terminates,
        }));
    }

    check_fact_effects(out, catalog, &formulas)
}
#[derive(Clone, Default)]
struct PauseEffectFormula {
    fixed: PauseEffects,
    binders: BTreeMap<usize, PauseEffects>,
}
impl PauseEffects {
    fn union(
        &mut self,
        other: Self,
    ) -> bool {
        let old = *self;
        self.suspend |= other.suspend;
        self.block |= other.block;
        *self != old
    }
    fn intersect(
        self,
        other: Self,
    ) -> Self {
        Self {
            suspend: self.suspend && other.suspend,
            block: self.block && other.block,
        }
    }
}
impl PauseEffectFormula {
    fn upper(&self) -> PauseEffects {
        let mut result = self.fixed;
        for &effect in self.binders.values() {
            result.union(effect);
        }
        result
    }
    fn merge(
        &mut self,
        other: &PauseEffectFormula,
    ) -> bool {
        let mut changed = self.fixed.union(other.fixed);
        for (&i, &p) in &other.binders {
            if p.block || p.suspend {
                changed |= self.binders.entry(i).or_default().union(p);
            }
        }
        changed
    }
}
fn specialize(
    call: &crate::TypedCall,
    formulas: &HashMap<&omega_resolver::DeclarationId, PauseEffectFormula>,
    catalog: &DeclarationCatalog<'_>,
) -> PauseEffectFormula {
    let source = &formulas[&call.machine];
    let mut derived = PauseEffectFormula {
        fixed: source.fixed,
        ..Default::default()
    };
    for (&index, &bound) in &source.binders {
        match call.type_arguments.get(index) {
            Some(crate::ValueType::MachineSelection(id, _)) => {
                derived
                    .fixed
                    .union(formulas[id.as_ref()].upper().intersect(bound));
            }
            Some(crate::ValueType::Parameter { owner, index }) if call.caller.as_ref() == Some(owner.as_ref()) => {
                derived.binders.entry(*index).or_default().union(bound);
            }
            _ => {
                derived.fixed.union(bound);
            }
        }
    }
    for (r, _) in catalog.evidence_refinements(call.evidence.as_ref()) {
        let permits = PauseEffects {
            block: !r.prohibits_block,
            suspend: !r.prohibits_suspend,
        };
        derived.fixed = derived.fixed.intersect(permits);
        for b in derived.binders.values_mut() {
            *b = b.intersect(permits);
        }
    }
    derived
}
fn propagate(
    graph: &super::effect_propagation::ExecutionCalls,
    out: &TypedStage,
    catalog: &DeclarationCatalog<'_>,
    formulas: &mut HashMap<&omega_resolver::DeclarationId, PauseEffectFormula>,
) {
    // A finite monotone union of two flags per machine and per binder.
    loop {
        let mut changed = false;
        for &index in &graph.direct {
            let call = &out.calls[index];
            if let Some(caller) = &call.caller {
                let next = specialize(call, formulas, catalog);
                changed |= formulas.get_mut(caller).unwrap().merge(&next);
            }
        }
        if !changed {
            return;
        }
    }
}
fn derive_pause<'a>(
    catalog: &DeclarationCatalog<'a>,
    out: &TypedStage,
    graph: &super::effect_propagation::ExecutionCalls,
) -> Result<HashMap<&'a omega_resolver::DeclarationId, PauseEffectFormula>, TypeError> {
    let mut formulas = catalog
        .signatures
        .keys()
        .map(|&id| (id, PauseEffectFormula::default()))
        .collect::<HashMap<_, _>>();
    for imported in &out.machine_effects {
        *formulas.get_mut(&imported.machine).unwrap() = PauseEffectFormula {
            fixed: imported.fixed_pause,
            binders: imported.callback_pause.clone(),
        };
    }
    for &index in &graph.indirect {
        let call = &out.indirect_calls[index];
        let Some(caller) = &call.caller else { continue };
        let pause = call.contract.pause;
        let ceiling = catalog.signatures[caller].pause;
        if pause.suspend && ceiling.suspend == Some(false) || pause.block && ceiling.block == Some(false) {
            return Err(TypeError::for_site(&call.site, "callback exceeds the machine's pause ceiling"));
        }
        let f = formulas.get_mut(caller).unwrap();
        if let crate::InvocationTarget::StaticParameter { owner, index } = &call.entry
            && owner == caller
        {
            f.binders.entry(*index).or_default().union(pause);
        } else {
            f.fixed.union(pause);
        }
    }
    propagate(graph, out, catalog, &mut formulas);
    for &id in &catalog.local_machines {
        let sig = &catalog.signatures[id];
        let f = formulas.get_mut(id).unwrap();
        let has_suspend = f.binders.values().any(|b| b.suspend);
        let has_block = f.binders.values().any(|b| b.block);
        f.fixed.union(PauseEffects {
            suspend: !has_suspend && sig.pause.suspend == Some(true),
            block: !has_block && sig.pause.block == Some(true),
        });
    }
    propagate(graph, out, catalog, &mut formulas);
    Ok(formulas)
}

/// Invocation rows must be complete before checking a machine cited as logical evidence.
fn check_fact_effects(
    out: &TypedStage,
    catalog: &DeclarationCatalog<'_>,
    formulas: &HashMap<&omega_resolver::DeclarationId, PauseEffectFormula>,
) -> Result<(), TypeError> {
    use crate::{CallUse, ValueType};
    let effects: HashMap<_, _> = out.machine_effects.iter().map(|e| (&e.machine, e)).collect();
    for call in &out.indirect_calls {
        if call.use_site != CallUse::Execution
            && (call.contract.pause.suspend || call.contract.pause.block || call.contract.reach.as_ref().is_none_or(|r| !r.is_empty()))
        {
            return Err(TypeError::for_site(&call.site, "fact requires an admitted effect-free callable"));
        }
    }
    for call in out
        .calls
        .iter()
        .filter(|call| call.use_site != CallUse::Execution)
    {
        let pause = specialize(call, formulas, catalog).upper();
        if pause.suspend || pause.block || !effects[&call.machine].reach.is_empty() {
            return Err(TypeError::for_site(&call.site, "fact or static argument requires an effect-free computation"));
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
        let effect = effects[candidate];
        if effect.pause.suspend
            || effect.pause.block
            || !effect.reach.is_empty()
            || effect.crash_ceiling.as_ref().is_some_and(|c| !c.is_empty())
            || !effect.invokes.is_empty()
        {
            return Err(TypeError::for_site(&obligation.site, "logical evidence cannot supply operational behavior"));
        }
    }
    Ok(())
}
