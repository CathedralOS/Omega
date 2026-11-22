use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::LogicalHypothesisError;
use crate::{LogicalHypothesis, TypeError, TypedStage, ValueType};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind::*;
use omega_syntax::TerminationClause;
use omega_typed::site_at;
use std::collections::HashMap;

fn check_parameter(
    unit: &ResolvedSourceFile<'_>,
    site: usize,
    parameter: &ValueType,
) -> std::result::Result<(), TypeError> {
    if matches!(parameter.carrier().reference_parts(), Some((crate::Access::Mut | crate::Access::Write, _))) {
        return Err(TypeError::for_reason(&site_at(unit.file, site), LogicalHypothesisError::MutableAuthority));
    }
    Ok(())
}

/// The outer result is the promise. In particular a mathematical arrow is
/// not an executable callback, even when its codomain is ordinary data, and
/// squash is not a promise to return its witness. Inspect normalized carriers
/// rather than guessing from layout, multiplicity, or the spelling of aliases.
fn check_result(
    unit: &ResolvedSourceFile<'_>,
    site: usize,
    result: &ValueType,
) -> std::result::Result<(), TypeError> {
    use ValueType as Ty;
    if matches!(
        result.carrier(),
        Ty::Bool | Ty::Integer { .. } | Ty::IndexedInteger { .. } | Ty::Float32 | Ty::Float64 | Ty::Address
            | Ty::Nominal(_) | Ty::Applied(..) | Ty::Tuple(_) | Ty::Array(..) | Ty::IndexedArray(..) | Ty::Slice(_)
            | Ty::Reference(..) | Ty::AtLifetime(..) | Ty::Callable(_)
    ) {
        return Err(TypeError::for_reason(&site_at(unit.file, site), LogicalHypothesisError::ObservedResult));
    }
    Ok(())
}

/// Every hypothesis in the selected declaration sees the substitution, including
/// nested requirements without a machine argument slot of their own.
pub(crate) fn check_application(
    catalog: &DeclarationCatalog<'_>,
    owner: &DeclarationId,
    unit: &ResolvedSourceFile<'_>,
    callee: usize,
    instantiate: impl Fn(&ValueType) -> ValueType,
) -> std::result::Result<(), TypeError> {
    if let Some(sites) = catalog.hypotheses_by_owner.get(owner) {
        for site in sites {
            let requirement = &catalog.hypotheses[site];
            for (_, parameter) in &requirement.parameters {
                check_parameter(unit, callee, &instantiate(parameter))?;
            }
            check_result(unit, callee, &instantiate(&requirement.result))?;
        }
    }
    Ok(())
}

/// A citation supplies evidence for a root hypothesis through the machine binder
/// that names it, and `validate_call_bindings` checks that argument. A root
/// hypothesis no binder names has no argument at the call, so the citation would
/// use the theorem without that evidence. Nested hypotheses are exempt: they
/// arrive inside their parent's evidence. The declaration itself still types.
pub(crate) fn check_root_evidence(
    catalog: &DeclarationCatalog<'_>,
    owner: &DeclarationId,
    unit: &ResolvedSourceFile<'_>,
    callee: usize,
    supplied: usize,
) -> std::result::Result<(), TypeError> {
    let Some(sites) = catalog.hypotheses_by_owner.get(owner) else {
        return Ok(());
    };
    for site in sites {
        let requirement = &catalog.hypotheses[site];
        let bound = (0..supplied).any(|index| matches!(catalog.types.bound(owner, index), Some(ValueType::LogicalRequirement(binder)) if binder == site));
        if requirement.parent.is_none() && !bound {
            return Err(TypeError::at(
                unit.file,
                callee,
                format!("theorem citation supplies no evidence for logical hypothesis `{}`", requirement.name),
            ));
        }
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
            .ancestors(n)
            .find_map(|a| unit.declaration_at(a))
            .filter(|owner| owner.kind == Machine)
            .ok_or_else(|| TypeError::at(f, n, "hypothesis needs an enclosing theorem"))?;
        let name = node.binder_name().expect("named hypothesis");
        let site = site_at(f, n);
        let mut requirement = LogicalHypothesis {
            site: site.clone(),
            parent: node
                .parent
                .filter(|&p| f.nodes[p].kind == Hypothesis)
                .map(|p| site_at(f, p)),
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
                        check_parameter(unit, f.nodes[annotation].parent.expect("parameter annotation"), &t)?;
                        requirement.parameters.push((parameter.span, t));
                    }
                }
                Result => {
                    let result = unit.annotation(unit.single_child(part)?, &catalog.types)?;
                    check_result(unit, part, &result)?;
                    requirement.result = result;
                }
                Requires => requirement.premises.push(site_at(f, unit.single_child(part)?)),
                Ensures => requirement
                    .conclusions
                    .push(site_at(f, unit.single_child(part)?)),
                Hypothesis => {}
                Terminates if f.nodes[part].termination_clause() == Some(TerminationClause::Bare) => {}
                Reaches | Crashes
                    if unit
                        .symbol(unit.single_child(part)?)
                        .is_some_and(|s| matches!(s,Symbol::Builtin(v) if v=="none")) => {}
                _ => return Err(TypeError::for_reason(&site_at(f, part), LogicalHypothesisError::ExecutableEffect)),
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
        catalog.hypotheses_by_owner.entry(owner).or_default().push(site.clone());
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
