use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{LogicalHypothesis, Site, TypeError, TypedStage, ValueType};
use omega_parser::NodeKind::*;
use omega_parser::TerminationClause;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashMap;

pub(crate) fn index_logical_hypotheses<'a>(
    unit: &mut ResolvedSourceFile<'a>,
    catalog: &mut DeclarationCatalog<'a>,
    output: &mut TypedStage,
) -> std::result::Result<(), TypeError> {
    let f = unit.file;
    for (n, node) in f.nodes.iter().enumerate().filter(|(_, n)| n.kind == Hypothesis) {
        let owner = f
            .ancestors(n)
            .find_map(|a| unit.declaration_at(a))
            .filter(|owner| owner.kind == Machine)
            .ok_or_else(|| TypeError::at(f, n, "hypothesis needs an enclosing theorem"))?;
        let name = node.binder_name().expect("named hypothesis");
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
                    for (parameter, annotation) in f.parameters(n) {
                        let t = unit.annotation(annotation, &catalog.types)?;
                        if matches!(t.carrier(), ValueType::Reference(crate::Access::Mut | crate::Access::Write, _)) {
                            return Err(TypeError::at(
                                f,
                                f.nodes[annotation].parent.expect("parameter annotation"),
                                "logical hypothesis cannot supply mutable authority",
                            ));
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
                Terminates if f.nodes[part].termination_clause() == Some(TerminationClause::Bare) => {}
                Reaches | Crashes
                    if unit
                        .symbol(unit.single_child(part)?)
                        .is_some_and(|s| matches!(s,Symbol::Builtin(v) if v=="none")) => {}
                _ => return Err(TypeError::at(f, part, "logical hypothesis cannot promise executable effects")),
            }
        }
        unit.hypotheses.insert(name.span, site.clone());
        if let Some(parent) = node.parent {
            for (formal, p, kind) in f.generic_parameters(parent) {
                if kind == MachineParam && formal.text == name.text {
                    let Some(ValueType::Parameter { index, .. }) = unit.parameters.get(&formal.span) else {
                        return Err(TypeError::unsupported(f, p));
                    };
                    if catalog
                        .types
                        .set_carrier(owner, *index, ValueType::LogicalRequirement(site.clone()))
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
            .binder_name()
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
) {
    let mut callers = HashMap::<&DeclarationId, Vec<&DeclarationId>>::new();
    let mut queue = std::collections::VecDeque::new();
    for unit in units {
        for (n, id) in unit.declaration_nodes().filter(|(_, id)| id.kind == MathLet) {
            let node = &unit.file.nodes[n];
            if node.has_boundary_modifier() {
                queue.push_back((id, id.clone()));
            }
            for i in n + 1..node.end {
                if let Some(Symbol::Declaration(dep)) = unit.symbol(i)
                    && dep.kind == MathLet
                {
                    if let Some(imported) = catalog.assumptions.get(dep) {
                        queue.extend(imported.iter().cloned().map(|assumption| (id, assumption)));
                    }
                    callers.entry(dep).or_default().push(id);
                }
            }
        }
    }
    while let Some((id, assumption)) = queue.pop_front() {
        let assumptions = catalog.assumptions.entry(id.clone()).or_default();
        if !assumptions.contains(&assumption) {
            assumptions.push(assumption.clone());
            if let Some(parents) = callers.get(id) {
                queue.extend(parents.iter().map(|&p| (p, assumption.clone())));
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
}
