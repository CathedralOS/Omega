use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{LogicalHypothesis, Site, TypeError, TypedStage, ValueType};
use omega_parser::NodeKind::*;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::{HashMap, HashSet};

pub(crate) fn index_mathematical_interfaces<'a>(
    units: &[ResolvedSourceFile<'a>],
    types: &mut TypeCatalog<'a>,
) -> std::result::Result<(), TypeError> {
    let mut entries = Vec::new();
    let mut index = HashMap::new();
    for unit in units {
        for (n, _) in unit
            .file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == MathLet)
        {
            index.insert(unit.declaration(n)?, entries.len());
            entries.push((unit, n));
        }
    }
    let mut waiting = vec![0usize; entries.len()];
    let mut users = vec![Vec::new(); entries.len()];
    let mut ready = std::collections::VecDeque::new();
    for (i, (unit, n)) in entries.iter().enumerate() {
        let mut parents = HashSet::new();
        let equals = unit.file.nodes[*n]
            .leaves
            .iter()
            .find(|t| t.text == "=")
            .map(|t| t.span.get_start_offset());
        for child in *n + 1..unit.file.nodes[*n].end {
            if equals.is_some_and(|offset| unit.file.span(child).get_start_offset() > offset) {
                break;
            }
            if matches!(unit.file.nodes[child].kind, FamilyApplication | Call)
                && let Some(&head) = unit.file.nodes[child].children.first()
                && let Some(Symbol::Declaration(id)) = unit.symbol(head)
                && let Some(&dep) = index.get(id)
            {
                parents.insert(dep);
            }
        }
        waiting[i] = parents.len();
        for dep in parents {
            users[dep].push(i);
        }
        if waiting[i] == 0 {
            ready.push_back(i);
        }
    }
    while let Some(i) = ready.pop_front() {
        let (unit, n) = entries[i];
        unit.index_mathematical(n, types)?;
        for &user in &users[i] {
            waiting[user] -= 1;
            if waiting[user] == 0 {
                ready.push_back(user);
            }
        }
    }
    if let Some(i) = waiting.iter().position(|n| *n > 0) {
        let (unit, n) = entries[i];
        return Err(TypeError::at(unit.file, n, "cyclic mathematical family signatures"));
    }
    Ok(())
}

pub(crate) fn index_logical_hypotheses<'a>(
    unit: &mut ResolvedSourceFile<'a>,
    catalog: &mut DeclarationCatalog<'a>,
    output: &mut TypedStage,
) -> std::result::Result<(), TypeError> {
    let f = unit.file;
    for (n, node) in f.nodes.iter().enumerate().filter(|(_, n)| n.kind == Hypothesis) {
        let owner = f
            .enclosing_contexts(n)
            .find_map(|a| unit.declaration_at(a))
            .ok_or_else(|| TypeError::at(f, n, "hypothesis needs an enclosing theorem"))?;
        if owner.kind != Machine {
            return Err(TypeError::at(f, n, "hypothesis needs an enclosing theorem"));
        }
        let name = node
            .words()
            .nth(2)
            .ok_or_else(|| TypeError::unsupported(f, n))?;
        let site = Site::at(f, n);
        let mut requirement = LogicalHypothesis {
            site: site.clone(),
            parent: node
                .parent
                .filter(|&p| f.nodes[p].kind == Hypothesis)
                .map(|p| Site::at(f, p)),
            owner: owner.clone(),
            name: name.text.into(),
            parameters: vec![],
            result: ValueType::NoResult,
            premises: vec![],
            conclusions: vec![],
        };
        for &part in &node.children {
            match f.nodes[part].kind {
                Params => {
                    for &p in &f.nodes[part].children {
                        let parameter = f.nodes[p]
                            .words()
                            .next()
                            .ok_or_else(|| TypeError::unsupported(f, p))?;
                        let t = unit.annotation(unit.single_child(p)?, &catalog.types)?;
                        if matches!(t.carrier(), ValueType::Reference(crate::Access::Mut | crate::Access::Write, _)) {
                            return Err(TypeError::at(f, p, "logical hypothesis cannot supply mutable authority"));
                        }
                        requirement.parameters.push((parameter.span, t));
                    }
                }
                Result => requirement.result = unit.annotation(unit.single_child(part)?, &catalog.types)?,
                Requires => requirement.premises.push(Site::at(f, unit.single_child(part)?)),
                Ensures => requirement
                    .conclusions
                    .push(Site::at(f, unit.single_child(part)?)),
                Hypothesis => {}
                Terminates if f.nodes[part].children.is_empty() => {}
                Reaches | Crashes
                    if unit
                        .symbol(unit.single_child(part)?)
                        .is_some_and(|s| matches!(s,Symbol::Builtin(v) if v=="none")) => {}
                _ => return Err(TypeError::at(f, part, "logical hypothesis cannot promise executable effects")),
            }
        }
        unit.hypotheses.insert(name.span, site.clone());
        if let Some(telescope) = node.parent.and_then(|p| f.child(p, GenericParams)) {
            for &p in &f.nodes[telescope].children {
                if f.nodes[p].kind == MachineParam
                    && let Some(formal) = f.nodes[p].words().nth(1)
                    && formal.text == name.text
                {
                    let Some(ValueType::Parameter { index, .. }) = unit.parameters.get(&formal.span) else {
                        return Err(TypeError::unsupported(f, p));
                    };
                    if catalog
                        .types
                        .const_carriers
                        .insert((owner, *index), ValueType::LogicalRequirement(site.clone()))
                        .is_some()
                    {
                        return Err(TypeError::at(f, n, "duplicate logical requirement for binder"));
                    }
                    unit.hypotheses.insert(formal.span, site.clone());
                }
            }
        }
        catalog.hypotheses.insert(site, requirement.clone());
        output.logical_hypotheses.push(requirement);
    }
    for (n, node) in f
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == MachineParam)
    {
        if node
            .words()
            .nth(1)
            .is_none_or(|name| !unit.hypotheses.contains_key(&name.span))
        {
            return Err(TypeError::at(
                f,
                n,
                "machine binder requires a logical hypothesis; executable code uses const F: machine",
            ));
        }
    }
    Ok(())
}
pub(crate) fn record_assumption_dependencies(
    units: &[ResolvedSourceFile<'_>],
    catalog: &mut DeclarationCatalog<'_>,
    output: &mut TypedStage,
) -> std::result::Result<(), TypeError> {
    let mut callers = HashMap::<DeclarationId, Vec<DeclarationId>>::new();
    let mut queue = std::collections::VecDeque::new();
    for unit in units {
        for (n, node) in unit
            .file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == MathLet)
        {
            let id = unit.declaration(n)?;
            if node.words().any(|w| w.text == "boundary") {
                queue.push_back((id.clone(), id.clone()));
            }
            for i in n + 1..node.end {
                if let Some(Symbol::Declaration(dep)) = unit.symbol(i)
                    && dep.kind == MathLet
                {
                    callers.entry(dep.clone()).or_default().push(id.clone());
                }
            }
        }
    }
    let mut closure = HashSet::new();
    while let Some((id, assumption)) = queue.pop_front() {
        if closure.insert((id.clone(), assumption.clone())) {
            catalog
                .assumptions
                .entry(id.clone())
                .or_default()
                .push(assumption.clone());
            if let Some(parents) = callers.get(&id) {
                queue.extend(parents.iter().map(|p| (p.clone(), assumption.clone())));
            }
        }
    }
    for proofs in catalog.assumptions.values_mut() {
        proofs.sort_by_cached_key(|id| format!("{id:?}"));
    }
    for (id, ty) in &catalog.types.mathematical {
        output
            .mathematical_interfaces
            .push(((*id).clone(), ty.clone(), catalog.assumptions.get(*id).cloned().unwrap_or_default()));
    }
    output
        .mathematical_interfaces
        .sort_by_cached_key(|row| format!("{:?}", row.0));
    Ok(())
}
