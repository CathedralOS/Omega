use crate::{CallableType, Catalog, RequirementSlot, TypeError, Unit, ValueType, types::Types};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

pub(super) fn slot(
    unit: &Unit<'_>,
    n: usize,
    types: &Types<'_>,
) -> Result<RequirementSlot, TypeError> {
    let Some(Symbol::Declaration(id)) = unit.symbol(n) else {
        return Err(TypeError::at(&unit.file, n, "requirement selection needs an exact machine"));
    };
    if id.kind != NodeKind::Machine {
        return Err(TypeError::at(&unit.file, n, "requirement selection needs a machine"));
    }
    let mut trait_arguments = Vec::new();
    for &section in &unit.file.nodes[n].children {
        let Some(Symbol::Declaration(owner)) = unit.application_segment(n, section) else {
            return Err(TypeError::unsupported(&unit.file, section));
        };
        if owner.kind != NodeKind::Trait {
            return Err(TypeError::at(&unit.file, section, "expected a trait application"));
        }
        for (index, &arg) in unit.file.nodes[section].children.iter().enumerate() {
            trait_arguments.push(unit.argument(arg, types, owner, index)?);
        }
    }
    Ok(RequirementSlot {
        declaration: id.clone(),
        trait_arguments,
    })
}
pub(super) fn finish<'a>(
    units: &[Unit<'a>],
    catalog: &mut Catalog<'a>,
) -> Result<(), TypeError> {
    let mut open = Vec::new();
    for unit in units {
        let f = &unit.file;
        for (n, node) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == NodeKind::ConstParam)
        {
            let list = node.parent.unwrap();
            if f.nodes[list].kind != NodeKind::GenericParams {
                continue;
            }
            let owner = unit.declaration(f.nodes[list].parent.unwrap())?;
            let name = node
                .words()
                .nth(1)
                .ok_or_else(|| TypeError::unsupported(f, n))?;
            let Some(ValueType::Parameter { index, .. }) = unit.parameters.get(&name.span) else {
                return Err(TypeError::unsupported(f, n));
            };
            let head = unit.single_child(n)?;
            if f.nodes[head].kind == NodeKind::PathType && f.nodes[head].path() == "machine" {
                open.push((unit, n, owner, *index));
                continue;
            }
            let Some(Symbol::Declaration(requirement)) = unit.symbol(head) else {
                continue;
            };
            if requirement.kind != NodeKind::Machine {
                continue;
            }
            let schema = catalog
                .requirements
                .iter()
                .find_map(|(&schema, rows)| rows.contains(&requirement).then_some(schema))
                .ok_or_else(|| TypeError::at(f, head, "static slot must name a trait requirement"))?;
            let slot = slot(unit, head, &catalog.types)?;
            if slot.trait_arguments.len() != catalog.types.arities[schema] {
                return Err(TypeError::at(f, head, "incomplete trait application"));
            }
            let sig = &catalog.signatures[requirement];
            let instantiate = |ty: &ValueType| ty.substitute(schema, &slot.trait_arguments);
            let bound = CallableType {
                parameters: sig.params.iter().map(|(_, p)| instantiate(p)).collect(),
                result: instantiate(&sig.result),
                contract: sig.site.clone(),
                closed_runtime: false,
                pause: sig.pause.declared(),
                has_guarantee: false,
                reach: sig.reach.clone(),
                terminates: sig.terminates,
                crash_ceiling: sig.crash_ceiling.clone(),
                requirement: Some(slot),
            };
            catalog
                .types
                .const_carriers
                .insert((owner, *index), ValueType::Callable(Box::new(bound)));
        }
    }
    loop {
        let mut changed = false;
        for &(unit, _, owner, index) in &open {
            let f = &unit.file;
            for (n, _) in f
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.kind == NodeKind::PathExpression)
            {
                let Some(Symbol::Declaration(callee)) = unit.symbol(n) else {
                    continue;
                };
                if callee.kind != NodeKind::Machine {
                    continue;
                }
                let Some(list) = f.child(n, NodeKind::ExpressionArgs) else {
                    continue;
                };
                let actuals = &f.nodes[list].children;
                if actuals.len() != catalog.types.arities[callee] {
                    continue;
                }
                for (position, &arg) in actuals.iter().enumerate() {
                    let Some(Symbol::Binding {
                        span,
                        kind: NodeKind::ConstParam,
                    }) = unit.symbol(arg)
                    else {
                        continue;
                    };
                    if !matches!(unit.parameters.get(span),Some(ValueType::Parameter{owner:o,index:i}) if o.as_ref()==owner && *i==index) {
                        continue;
                    }
                    let Some(formal @ ValueType::Callable(_)) = catalog.types.bound(callee, position) else {
                        continue;
                    };
                    let arguments = actuals
                        .iter()
                        .enumerate()
                        .map(|(i, &actual)| {
                            if let Some(Symbol::Binding { span, .. }) = unit.symbol(actual)
                                && let Some(value) = unit.parameters.get(span)
                            {
                                Ok(value.clone())
                            } else {
                                unit.argument(actual, &catalog.types, callee, i)
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let derived = formal.substitute(callee, &arguments);
                    if let Some(previous) = catalog.types.bound(owner, index) {
                        if previous != &derived {
                            return Err(TypeError::at(f, arg, "incompatible originating callback requirements"));
                        }
                    } else {
                        catalog.types.const_carriers.insert((owner, index), derived);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (unit, n, owner, index) in open {
        if catalog.types.bound(owner, index).is_none() {
            return Err(TypeError::at(&unit.file, n, "static callback lacks an originating requirement"));
        }
    }
    Ok(())
}
