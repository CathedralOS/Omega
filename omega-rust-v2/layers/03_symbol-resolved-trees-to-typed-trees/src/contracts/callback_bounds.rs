use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

pub(crate) fn resolve_callback_bounds<'a>(
    units: &[ResolvedSourceFile<'a>],
    catalog: &mut DeclarationCatalog<'a>,
) -> Result<(), TypeError> {
    let mut forwarding = Vec::new();
    for unit in units {
        for &(head, slot) in &unit.nominal_callbacks {
            let (requirement, applied) = super::conformance::applied_requirement(unit, catalog, head)?;
            let bound = catalog.requirement_type(&applied, requirement);
            catalog.types.set_carrier(slot.0, slot.1, bound);
        }
        if unit.inferred_callbacks.is_empty() {
            continue;
        }
        for (section, name) in unit.applications() {
            let path = name.node.node;
            let Symbol::Declaration(callee) = &name.target else { continue };
            if callee.kind != NodeKind::Machine || unit.file.nodes[path].kind != NodeKind::PathExpression {
                continue;
            }
            let actuals = &unit.file.nodes[section].children;
            if actuals.len() != catalog.types.arity(callee) {
                continue;
            }
            for (position, &arg) in actuals.iter().enumerate() {
                if let Some(Symbol::Binding { binder_node, .. }) = unit.name_in_role(arg, false)
                    && let Some(&slot) = unit.inferred_callbacks.get(binder_node)
                {
                    forwarding.push((unit, path, callee, actuals, position, slot));
                }
            }
        }
    }
    // An inferred bound can supply the origin for a further forwarding call.
    // Retire each edge after checking it, including edges into an already known slot.
    loop {
        let before = forwarding.len();
        let mut pending = Vec::new();
        for edge @ (unit, path, callee, actuals, position, slot) in forwarding {
            let Some(formal @ ValueType::Callable(_)) = catalog.types.bound(callee, position) else {
                pending.push(edge);
                continue;
            };
            let receiver = unit.receiver_arguments(path, catalog)?;
            let arguments = unit.bind_formals(
                path,
                callee,
                actuals,
                &catalog.types,
                |ty| ty.in_enclosing_scopes(&receiver),
                |i, arg, bound| {
                    if matches!(bound, Some(ValueType::Callable(_)))
                        && let Some(Symbol::Binding { span, .. }) = unit.name_in_role(arg, false)
                        && let Some(value) = unit.parameters.get(span)
                    {
                        return Ok(value.clone());
                    }
                    unit.argument(arg, &catalog.types, callee, i, bound)
                },
            )?;
            let derived = formal.at_machine_application(callee, &arguments, &receiver, &[]);
            if let Some(previous) = catalog.types.bound(slot.0, slot.1) {
                if previous != &derived {
                    return Err(TypeError::at(unit.file, actuals[position], "incompatible originating callback requirements"));
                }
            } else {
                catalog.types.set_carrier(slot.0, slot.1, derived);
            }
        }
        if pending.len() == before {
            break;
        }
        forwarding = pending;
    }
    for unit in units {
        for (&node, &(owner, index)) in &unit.inferred_callbacks {
            if catalog.types.bound(owner, index).is_none() {
                return Err(TypeError::at(unit.file, node, "static callback lacks an originating requirement"));
            }
        }
    }
    Ok(())
}
