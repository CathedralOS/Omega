use super::*;
use std::collections::HashMap;

pub(super) fn check(
    units: &[Unit<'_>],
    catalog: &Catalog<'_>,
    stage: &mut TypedStage,
) -> Result<(), TypeError> {
    for unit in units {
        let f = &unit.file;
        for (n, node) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::DroppedBy)
        {
            let owner = unit.declaration(node.parent.ok_or_else(|| TypeError::unsupported(f, n))?)?;
            let head = unit.single_child(n)?;
            let Some(Symbol::Declaration(machine)) = unit.symbol(head) else {
                return Err(TypeError::at(f, head, "cleanup must select one exact machine"));
            };
            let sig = catalog
                .signatures
                .get(machine)
                .ok_or_else(|| TypeError::at(f, head, "cleanup must select one exact machine"))?;
            let args = f
                .child(head, NodeKind::TypeArgs)
                .into_iter()
                .flat_map(|section| f.nodes[section].children.iter().copied())
                .enumerate()
                .map(|(i, a)| unit.argument(a, &catalog.types, machine, i))
                .collect::<Result<Vec<_>, _>>()?;
            let carrier = if owner.kind == NodeKind::Domain {
                catalog.types.domains.carrier(owner).clone()
            } else {
                catalog.types.open_data(owner)
            };
            if carrier
                .nominal_parts()
                .is_some_and(|(id, _)| id.package != owner.package)
            {
                return Err(TypeError::at(f, n, "cleanup selection requires the carrier's owning package"));
            }
            if args.len() != catalog.types.arities[machine]
                || sig.result != ValueType::NoResult
                || sig.params.len() != 1
                || sig.params[0].1.substitute(machine, &args) != carrier
            {
                return Err(TypeError::at(f, n, "cleanup must consume exactly one owned carrier and return no value"));
            }
            if sig
                .crash_ceiling
                .as_ref()
                .is_some_and(|causes| !causes.is_empty())
            {
                return Err(TypeError::at(f, n, "cleanup cannot publish a crash route"));
            }
            let effects = stage
                .machine_effects
                .iter()
                .find(|e| e.machine == *machine)
                .ok_or_else(|| TypeError::at(f, n, "missing cleanup machine effects"))?;
            if effects.pause.block || effects.pause.suspend {
                return Err(TypeError::at(f, n, "cleanup cannot block or suspend"));
            }
            stage.cleanups.push(CleanupSelection {
                site: Site::at(f, n),
                owner: owner.clone(),
                machine: machine.clone(),
                arguments: args,
            });
            stage.obligations.push(TypingObligation {
                site: Site::at(f, n),
                kind: ObligationKind::CleanupContract(machine.clone()),
            });
        }
    }
    if stage.cleanups.is_empty() {
        return Ok(());
    }
    let plans: HashMap<_, _> = stage.cleanups.iter().map(|plan| (&plan.owner, plan)).collect();
    for unit in units {
        for (n, node) in unit.file.nodes.iter().enumerate() {
            if node.kind == NodeKind::QualifiedType {
                let ty = unit.annotation(n, &catalog.types)?;
                compatible(&ty, &plans, catalog).map_err(|m| TypeError::at(&unit.file, n, m))?;
            }
        }
    }
    for expression in &stage.expressions {
        compatible(&expression.ty, &plans, catalog).map_err(|m| TypeError {
            package: expression.site.package.clone(),
            file: expression.site.file.clone(),
            span: expression.site.span,
            message: m.into(),
        })?;
    }
    Ok(())
}

fn compatible(
    ty: &ValueType,
    plans: &HashMap<&DeclarationId, &CleanupSelection>,
    catalog: &Catalog<'_>,
) -> Result<(), &'static str> {
    let mut selected = None;
    if let Some((id, args)) = ty.carrier().nominal_parts()
        && let Some(plan) = plans.get(id)
    {
        selected = Some((
            &plan.machine,
            plan.arguments
                .iter()
                .map(|t| catalog.types.field_type(t, id, args))
                .collect::<Vec<_>>(),
        ));
    }
    for d in ty.domains() {
        if let Some(plan) = plans.get(&d.declaration) {
            let args = if d.arguments.is_empty() && catalog.types.arities[&d.declaration] == 1 {
                std::slice::from_ref(ty.carrier())
            } else {
                d.arguments.as_slice()
            };
            if args.len() != catalog.types.arities[&d.declaration] {
                return Err("incomplete cleanup domain application");
            }
            let candidate = (
                &plan.machine,
                plan.arguments
                    .iter()
                    .map(|t| t.substitute(&d.declaration, args))
                    .collect::<Vec<_>>(),
            );
            if selected.as_ref().is_some_and(|prior| prior != &candidate) {
                return Err("conflicting cleanup selections on one owned subject");
            }
            selected = Some(candidate);
        }
    }
    Ok(())
}
pub(super) fn access_policies(
    units: &[Unit<'_>],
    catalog: &Catalog<'_>,
    env: &TypingEnvironment<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    for unit in units {
        let f = &unit.file;
        for (n, node) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::Access)
        {
            let domain = unit.declaration(node.parent.ok_or_else(|| TypeError::unsupported(f, n))?)?;
            let head = unit.single_child(n)?;
            let Some(Symbol::Declaration(policy)) = unit.symbol(head) else {
                return Err(TypeError::at(f, head, "access requires an exact policy implementation"));
            };
            let requirement = env
                .access_plan_requirement
                .ok_or_else(|| TypeError::at(f, head, "access policy requires the admitted AccessPolicy plan requirement"))?;
            let row = out
                .conformance_rows
                .iter()
                .find(|r| r.conformance == *policy && &r.requirement == requirement)
                .ok_or_else(|| TypeError::at(f, head, "selected policy does not supply the admitted plan requirement"))?;
            let machine = &row.implementation;
            let args = f
                .child(head, NodeKind::TypeArgs)
                .into_iter()
                .flat_map(|section| &f.nodes[section].children)
                .enumerate()
                .map(|(index, &a)| unit.argument(a, &catalog.types, policy, index))
                .collect::<Result<Vec<_>, _>>()?;
            if args.len() != catalog.types.arities[policy] {
                return Err(TypeError::at(f, head, "incomplete access policy application"));
            }
            let effects = out
                .machine_effects
                .iter()
                .find(|e| e.machine == *machine)
                .ok_or_else(|| TypeError::at(f, head, "missing access plan machine effects"))?;
            if effects.pause.suspend
                || effects.pause.block
                || !effects.reach.is_empty()
                || !effects.invokes.is_empty()
                || effects
                    .crash_ceiling
                    .as_ref()
                    .is_some_and(|causes| !causes.is_empty())
            {
                return Err(TypeError::at(f, head, "access plan computation must be effect-free"));
            }
            out.access_policies.push(AccessPolicySelection {
                site: Site::at(f, n),
                domain: domain.clone(),
                policy: policy.clone(),
                arguments: args,
                plan_machine: machine.clone(),
            });
            out.obligations.push(TypingObligation {
                site: Site::at(f, n),
                kind: ObligationKind::EvaluateAccessPlan {
                    domain: domain.clone(),
                    machine: machine.clone(),
                },
            });
        }
    }
    Ok(())
}
