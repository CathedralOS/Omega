use super::*;

fn declared(id: &DeclarationId) -> InvocationTarget {
    InvocationTarget::Declaration(RequirementSlot {
        declaration: id.clone(),
        trait_arguments: Vec::new(),
    })
}
fn through(
    call: &TypedCall,
    entry: &InvocationTarget,
    files: &HashMap<(&str, &str), &Unit<'_>>,
    catalog: &Catalog<'_>,
) -> InvocationTarget {
    match entry {
        InvocationTarget::RuntimeParameter { owner, index } if owner == &call.machine => {
            let site = &call.operands[*index];
            let unit = files[&(site.package.as_str(), site.file.as_str())];
            match unit.names.get(&site.span).copied() {
                Some(Symbol::Binding { span, .. })
                    if let Some(caller) = &call.caller
                        && let Some(i) = catalog.signatures[caller]
                            .params
                            .iter()
                            .position(|(s, _)| s == span) =>
                {
                    InvocationTarget::RuntimeParameter {
                        owner: caller.clone(),
                        index: i,
                    }
                }
                Some(Symbol::Declaration(id)) if id.kind == NodeKind::Machine => declared(id),
                _ => InvocationTarget::StoredCallable(site.clone()),
            }
        }
        InvocationTarget::StaticParameter { owner, index } if owner == &call.machine => match &call.type_arguments[*index] {
            ValueType::MachineSelection(id) => declared(id),
            ValueType::Parameter { owner, index } => InvocationTarget::StaticParameter {
                owner: *owner.clone(),
                index: *index,
            },
            _ => entry.clone(),
        },
        InvocationTarget::Declaration(slot) => InvocationTarget::Declaration(RequirementSlot {
            declaration: slot.declaration.clone(),
            trait_arguments: slot
                .trait_arguments
                .iter()
                .map(|t| t.substitute(&call.machine, &call.type_arguments))
                .collect(),
        }),
        _ => entry.clone(),
    }
}
pub(super) fn compose(
    units: &[Unit<'_>],
    catalog: &Catalog<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let files = units
        .iter()
        .map(|u| ((u.file.package, u.file.path), u))
        .collect();
    let mut rows: HashMap<_, HashSet<_>> = catalog
        .signatures
        .keys()
        .map(|&id| (id, HashSet::new()))
        .collect();
    let mut ceilings: HashMap<&DeclarationId, Vec<&InvocationTarget>> = HashMap::new();
    for (owner, _, entry) in &out.invocation_ceilings {
        ceilings.entry(owner).or_default().push(entry);
        rows.get_mut(owner).unwrap().insert(entry.clone());
    }
    let admits = |owner: &DeclarationId, entry: &InvocationTarget| {
        let Some(allowed) = ceilings.get(owner) else {
            return true;
        };
        if allowed.contains(&entry) {
            return true;
        }
        let InvocationTarget::StaticParameter { owner: bound, index } = entry else {
            return false;
        };
        let Some(ValueType::Callable(c)) = catalog.types.bound(bound, *index) else {
            return false;
        };
        let Some(req) = &c.requirement else {
            return false;
        };
        allowed.iter().any(|a|matches!(a,InvocationTarget::Declaration(slot) if slot==req || catalog.requirements.get(&slot.declaration).is_some_and(|r|r.contains(&&req.declaration)) && slot.trait_arguments==req.trait_arguments))
    };
    let error = |site: &Site| TypeError {
        package: site.package.clone(),
        file: site.file.clone(),
        span: site.span,
        message: "call exceeds the declared synchronous invocation ceiling".into(),
    };
    for call in &out.indirect_calls {
        if let Some(owner) = &call.caller
            && !call.guarantee
        {
            if !admits(owner, &call.entry) {
                return Err(error(&call.site));
            }
            rows.get_mut(owner).unwrap().insert(call.entry.clone());
        }
    }
    loop {
        let mut changed = false;
        for call in &out.calls {
            let Some(caller) = &call.caller else {
                continue;
            };
            let mut incoming: Vec<_> = rows[&call.machine]
                .iter()
                .map(|entry| through(call, entry, &files, catalog))
                .collect();
            incoming.sort_by_cached_key(|entry| format!("{entry:?}"));
            for e in incoming {
                if !admits(caller, &e) {
                    return Err(error(&call.site));
                }
                let row = rows.get_mut(caller).unwrap();
                if row.len() >= 4096 && !row.contains(&e) {
                    return Err(TypeError {
                        package: call.site.package.clone(),
                        file: call.site.file.clone(),
                        span: call.site.span,
                        message: "invocation inference exceeds implementation capacity".into(),
                    });
                }
                changed |= row.insert(e);
            }
        }
        if !changed {
            break;
        }
    }
    for effect in &mut out.machine_effects {
        effect.invokes = rows.remove(&effect.machine).unwrap().into_iter().collect();
        effect.invokes.sort_by_cached_key(|e| format!("{e:?}"));
    }
    Ok(())
}
