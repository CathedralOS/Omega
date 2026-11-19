use super::{Catalog, MachineEffects, PauseEffects, TypeError, TypedStage, Unit};
use super::{ServiceApplication, types::Types};
use omega_parser::NodeKind;
use omega_resolver::Symbol;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Copy)]
pub(super) struct Envelope {
    suspend: Option<bool>,
    block: Option<bool>,
}
impl Envelope {
    pub fn declared(self) -> PauseEffects {
        PauseEffects {
            suspend: self.suspend == Some(true),
            block: self.block == Some(true),
        }
    }
    pub(super) fn read(
        unit: &Unit<'_>,
        n: usize,
    ) -> Result<Self, TypeError> {
        let f = &unit.file;
        let published = f.nodes[n].words().any(|w| w.text == "public")
            || f.nodes[n]
                .parent
                .is_some_and(|p| f.nodes[p].kind == NodeKind::Trait && f.nodes[p].words().any(|w| w.text == "boundary"));
        let flag = |kind| -> Result<Option<bool>, TypeError> {
            let Some(c) = f.child(n, kind) else {
                return Ok(published.then_some(false));
            };
            if !f.nodes[c].children.is_empty() {
                return Err(TypeError::unsupported(f, c));
            }
            match f.nodes[c].words().nth(1).map(|w| w.text) {
                Some("false") => Ok(Some(false)),
                None | Some("true") => Ok(Some(true)),
                _ => Err(TypeError::unsupported(f, c)),
            }
        };
        Ok(Self {
            suspend: flag(NodeKind::Suspends)?,
            block: flag(NodeKind::Blocks)?,
        })
    }
}
pub(super) fn crashes(
    unit: &Unit<'_>,
    n: usize,
) -> Result<Option<Vec<crate::CrashCause>>, TypeError> {
    let f = &unit.file;
    let mut causes = Vec::new();
    let mut specified = false;
    let mut empty = false;
    for &c in &f.nodes[n].children {
        if f.nodes[c].kind != NodeKind::Crashes {
            continue;
        }
        specified = true;
        let (&head, guards) = f.nodes[c]
            .children
            .split_first()
            .ok_or_else(|| TypeError::unsupported(f, c))?;
        if !guards.is_empty() && matches!(unit.symbol(head),Some(Symbol::Builtin(s)) if s=="none") {
            return Err(TypeError::at(f, c, "crashes none cannot carry a route guard"));
        }
        match unit.symbol(head) {
            Some(Symbol::Builtin(s)) if s == "none" => empty = true,
            Some(Symbol::Builtin(s)) if s == "Trap" => causes.push(crate::CrashCause::Trap),
            Some(Symbol::Builtin(s)) if s == "Abort" => causes.push(crate::CrashCause::Abort),
            _ => return Err(TypeError::at(f, head, "crash clause requires Trap, Abort or none")),
        }
    }
    if empty && !causes.is_empty() {
        return Err(TypeError::at(f, n, "crashes none conflicts with a declared cause"));
    }
    causes.sort_by_key(|cause| matches!(cause, crate::CrashCause::Abort));
    causes.dedup();
    Ok((specified
        || f.nodes[n].words().any(|w| w.text == "public")
        || f.nodes[n]
            .parent
            .is_some_and(|p| f.nodes[p].kind == NodeKind::Trait && f.nodes[p].words().any(|w| w.text == "boundary")))
    .then_some(causes))
}
pub(super) fn empty_clause(
    unit: &Unit<'_>,
    n: usize,
) -> bool {
    matches!(unit.file.nodes[n].children.as_slice(),[i] if matches!(unit.symbol(*i),Some(Symbol::Builtin(s)) if s=="none"))
}
pub(super) fn reach(
    unit: &Unit<'_>,
    types: &Types<'_>,
    n: usize,
) -> Result<Option<Vec<ServiceApplication>>, TypeError> {
    let f = &unit.file;
    let Some(clause) = f.child(n, NodeKind::Reaches) else {
        return Ok(None);
    };
    if empty_clause(unit, clause) {
        return Ok(Some(Vec::new()));
    }
    let mut row = Vec::new();
    for &entry in &f.nodes[clause].children {
        let Some(Symbol::Declaration(id)) = unit.symbol(entry) else {
            return Err(TypeError::unsupported(f, entry));
        };
        if id.kind != NodeKind::Trait {
            return Err(TypeError::at(f, entry, "reach requires a service trait"));
        }
        let args = f
            .child(entry, NodeKind::TypeArgs)
            .into_iter()
            .flat_map(|section| &f.nodes[section].children)
            .enumerate()
            .map(|(index, &arg)| unit.argument(arg, types, id, index))
            .collect::<Result<Vec<_>, _>>()?;
        if args.len() != types.arities[id] {
            return Err(TypeError::at(f, entry, "incomplete service application"));
        }
        let service = ServiceApplication {
            service: id.clone(),
            arguments: args,
        };
        if !row.contains(&service) {
            row.push(service);
        }
    }
    Ok(Some(row))
}

pub(super) fn markers(
    unit: &Unit<'_>,
    mut n: usize,
) -> Result<PauseEffects, TypeError> {
    use NodeKind::*;
    let f = &unit.file;
    let mut markers = PauseEffects::default();
    while let Some(parent) = f.nodes[n].parent.filter(|&p| f.nodes[p].kind == CallMarker) {
        match f.nodes[parent].words().next().map(|w| w.text) {
            Some("block") if !markers.block && !markers.suspend => markers.block = true,
            Some("suspend") if !markers.suspend => markers.suspend = true,
            _ => return Err(TypeError::at(f, parent, "repeated or reversed call acknowledgement")),
        }
        n = parent;
    }
    if markers.suspend {
        let legal = f.nodes[n].parent.is_some_and(|p| match f.nodes[p].kind {
            Tail | ExpressionStatement | Discard => true,
            Let => f.nodes[p].children.last() == Some(&n) && f.nodes[f.nodes[p].children[0]].kind == BindingPattern,
            Transition => f.nodes[p].children.first() == Some(&n),
            Arm => f.nodes[p].children.last() == Some(&n) && f.nodes[p].parent.is_some_and(|d| f.nodes[d].kind == Transition),
            _ => false,
        });
        if !legal {
            return Err(TypeError::at(f, n, "suspend acknowledgement requires a complete suspension position"));
        }
    }
    Ok(markers)
}
pub(super) fn validate(
    catalog: &Catalog<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let formulas = derive_pause(catalog, out)?;
    for c in &out.calls {
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
    for call in &out.indirect_calls {
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
        for c in &out.calls {
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
    for call in &out.calls {
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
struct Formula {
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
impl Formula {
    fn upper(&self) -> PauseEffects {
        let mut result = self.fixed;
        for &effect in self.binders.values() {
            result.union(effect);
        }
        result
    }
    fn merge(
        &mut self,
        other: &Formula,
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
    formulas: &HashMap<&omega_resolver::DeclarationId, Formula>,
    catalog: &Catalog<'_>,
) -> Formula {
    let source = &formulas[&call.machine];
    let mut derived = Formula {
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
    catalog: &Catalog<'_>,
    formulas: &mut HashMap<&omega_resolver::DeclarationId, Formula>,
) {
    // A finite monotone union of two flags per machine and per binder.
    loop {
        let mut changed = false;
        for call in &out.calls {
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
    catalog: &Catalog<'a>,
    out: &TypedStage,
) -> Result<HashMap<&'a omega_resolver::DeclarationId, Formula>, TypeError> {
    let mut formulas = catalog
        .signatures
        .keys()
        .map(|&id| (id, Formula::default()))
        .collect::<HashMap<_, _>>();
    for call in &out.indirect_calls {
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
