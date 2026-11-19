use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::{MachineEffects, PauseEffects, ServiceApplication, TypeError, TypedStage};
use std::collections::{BTreeMap, HashMap, HashSet};

pub(crate) fn check_machine_effects(
    catalog: &DeclarationCatalog<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let formulas = derive_pause(catalog, out)?;
    for c in out
        .calls
        .iter()
        .filter(|call| call.use_site == crate::CallUse::Execution)
    {
        let specialized = specialize(c, &formulas, catalog).upper();
        let err = |m: &str| TypeError {
            package: c.site.package.clone(),
            file: c.site.file.clone(),
            span: c.site.span,
            message: m.into(),
        };
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
    for call in out
        .indirect_calls
        .iter()
        .filter(|call| call.use_site == crate::CallUse::Execution)
    {
        if call.guarantee {
            continue;
        }
        let Some(caller) = &call.caller else { continue };
        if let Some(row) = &call.contract.reach {
            if let Some(ceiling) = &catalog.signatures[caller].reach
                && row.iter().any(|service| !ceiling.contains(service))
            {
                return Err(TypeError {
                    package: call.site.package.clone(),
                    file: call.site.file.clone(),
                    span: call.site.span,
                    message: "callback exceeds the machine's service reach".into(),
                });
            }
            reach.get_mut(caller).unwrap().extend(row.iter().cloned());
        }
        for &cause in call.contract.crash_ceiling.iter().flatten() {
            out.obligations.push(crate::TypingObligation {
                site: call.site.clone(),
                kind: crate::ObligationKind::CrashCoverage {
                    machine: caller.clone(),
                    cause,
                },
            });
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for c in out
            .calls
            .iter()
            .filter(|call| call.use_site == crate::CallUse::Execution)
        {
            let Some(caller) = &c.caller else {
                continue;
            };
            let selected = reach[&c.machine]
                .iter()
                .map(|service| {
                    let arguments = service
                        .arguments
                        .iter()
                        .map(|t| {
                            let ty = c
                                .enclosing_arguments
                                .iter()
                                .fold(t.clone(), |t, (owner, args)| t.substitute(owner, args));
                            ty.substitute(&c.machine, &c.type_arguments)
                        })
                        .collect();
                    ServiceApplication {
                        service: service.service.clone(),
                        arguments,
                    }
                })
                .collect::<Vec<_>>();
            let restrictions = catalog.evidence_refinements(c.evidence.as_ref());
            for service in selected {
                if restrictions.iter().any(|(r, o)| {
                    r.reach.as_ref().is_some_and(|row| {
                        !row.iter().any(|s| {
                            s.service == service.service
                                && s.arguments
                                    .iter()
                                    .map(|a| a.substitute(&o.declaration, &o.trait_arguments))
                                    .eq(service.arguments.iter().cloned())
                        })
                    })
                }) {
                    continue;
                }

                if let Some(bound) = &catalog.signatures[caller].reach {
                    if !bound.contains(&service) {
                        return Err(TypeError {
                            package: c.site.package.clone(),
                            file: c.site.file.clone(),
                            span: c.site.span,
                            message: format!("call exceeds service reach for {}", service.service.path),
                        });
                    }
                } else {
                    let row = reach.get_mut(caller).unwrap();
                    // A generic recursive family may produce a distinct substituted
                    // service at each depth. It requires a finite published ceiling.
                    if row.len() > 4096 {
                        return Err(TypeError {
                            package: c.site.package.clone(),
                            file: c.site.file.clone(),
                            span: c.site.span,
                            message: "service reach inference exceeds implementation capacity".into(),
                        });
                    }
                    changed |= row.insert(service);
                }
            }
        }
    }
    for call in out
        .calls
        .iter()
        .filter(|call| call.use_site == crate::CallUse::Execution)
    {
        if let Some(caller) = &call.caller
            && let Some(causes) = &catalog.signatures[&call.machine].crash_ceiling
        {
            for &cause in causes {
                out.obligations.push(crate::TypingObligation {
                    site: call.site.clone(),
                    kind: crate::ObligationKind::CrashCoverage {
                        machine: caller.clone(),
                        cause,
                    },
                });
            }
        }
    }
    // Source order is stable across input permutations, including overloaded paths.
    let mut ids = catalog.signatures.keys().copied().collect::<Vec<_>>();
    ids.sort_by_cached_key(|id| format!("{id:?}"));
    for id in ids {
        let mut row = reach.remove(id).unwrap().into_iter().collect::<Vec<_>>();
        row.sort_by_cached_key(|r| format!("{r:?}"));
        out.machine_effects.push(MachineEffects {
            invokes: Vec::new(),
            machine: id.clone(),
            pause: formulas[id].upper(),
            fixed_pause: formulas[id].fixed,
            callback_pause: formulas[id].binders.clone(),
            reach: row,
            crash_ceiling: catalog.signatures[id].crash_ceiling.clone(),
            terminates: catalog.signatures[id].terminates,
        });
    }
    for obligation in &out.obligations {
        if let crate::ObligationKind::RefineCallable { contract, machine } = &obligation.kind {
            let effect = out
                .machine_effects
                .iter()
                .find(|e| &e.machine == machine)
                .unwrap();
            if effect.pause.suspend && !contract.pause.suspend
                || effect.pause.block && !contract.pause.block
                || contract
                    .reach
                    .as_ref()
                    .is_some_and(|row| effect.reach.iter().any(|s| !row.contains(s)))
            {
                return Err(TypeError {
                    package: obligation.site.package.clone(),
                    file: obligation.site.file.clone(),
                    span: obligation.site.span,
                    message: "selected machine exceeds the callable's inferred effect bounds".into(),
                });
            }
        }
    }
    Ok(())
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
            Some(crate::ValueType::MachineSelection(id)) => {
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
    out: &TypedStage,
    catalog: &DeclarationCatalog<'_>,
    formulas: &mut HashMap<&omega_resolver::DeclarationId, PauseEffectFormula>,
) {
    // A finite monotone union of two flags per machine and per binder.
    loop {
        let mut changed = false;
        for call in out
            .calls
            .iter()
            .filter(|call| call.use_site == crate::CallUse::Execution)
        {
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
) -> Result<HashMap<&'a omega_resolver::DeclarationId, PauseEffectFormula>, TypeError> {
    let mut formulas = catalog
        .signatures
        .keys()
        .map(|&id| (id, PauseEffectFormula::default()))
        .collect::<HashMap<_, _>>();
    for call in out
        .indirect_calls
        .iter()
        .filter(|call| call.use_site == crate::CallUse::Execution)
    {
        if call.guarantee {
            continue;
        }
        let Some(caller) = &call.caller else { continue };
        let pause = call.contract.pause;
        let ceiling = catalog.signatures[caller].pause;
        if pause.suspend && ceiling.suspend == Some(false) || pause.block && ceiling.block == Some(false) {
            return Err(TypeError {
                package: call.site.package.clone(),
                file: call.site.file.clone(),
                span: call.site.span,
                message: "callback exceeds the machine's pause ceiling".into(),
            });
        }
        let f = formulas.get_mut(caller).unwrap();
        if let Some(crate::ValueType::Parameter { owner, index }) = &call.binder
            && owner.as_ref() == caller
        {
            f.binders.entry(*index).or_default().union(pause);
        } else {
            f.fixed.union(pause);
        }
    }
    propagate(out, catalog, &mut formulas);
    for (&id, sig) in &catalog.signatures {
        let f = formulas.get_mut(id).unwrap();
        let has_suspend = f.binders.values().any(|b| b.suspend);
        let has_block = f.binders.values().any(|b| b.block);
        f.fixed.union(PauseEffects {
            suspend: !has_suspend && sig.pause.suspend == Some(true),
            block: !has_block && sig.pause.block == Some(true),
        });
    }
    propagate(out, catalog, &mut formulas);
    Ok(formulas)
}
