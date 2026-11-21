use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::typing::preparation_dependencies::PreparationDependencies;
use crate::{CleanupEffectError, CleanupSelection, CleanupSelectionError, ObligationKind, TypeError, TypedStage, TypingObligation, ValueType};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::site_at;
use std::collections::HashMap;

pub(crate) fn check_cleanup_selections(
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'_>,
    dependencies: &PreparationDependencies<'_>,
    stage: &mut TypedStage,
) -> Result<(), TypeError> {
    for unit in units {
        let f = unit.file;
        for (n, node) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::DroppedBy)
        {
            let owner = unit.declaration(node.parent.ok_or_else(|| TypeError::unsupported(f, n))?)?;
            let head = unit.single_child(n)?;
            let Some(Symbol::Declaration(machine)) = unit.symbol(head) else {
                return Err(TypeError::for_reason(&site_at(f, head), CleanupSelectionError::NotExactMachine));
            };
            if !dependencies.node_is_ready(f.node_id(n)) || !dependencies.declaration_is_ready(machine) {
                continue;
            }
            let sig = catalog
                .signatures
                .get(machine)
                .ok_or_else(|| TypeError::for_reason(&site_at(f, head), CleanupSelectionError::NotExactMachine))?;
            let args = unit.application_arguments(head, machine, &catalog.types)?;
            let carrier = if owner.kind == NodeKind::Domain {
                catalog.types.domains.carrier(owner).clone()
            } else {
                catalog.types.open_data(owner)
            };
            if carrier
                .nominal_parts()
                .is_some_and(|(id, _)| id.package != owner.package)
            {
                return Err(TypeError::for_reason(&site_at(f, n), CleanupSelectionError::ForeignCarrier));
            }
            if sig.result != ValueType::NoResult || sig.parameters.len() != 1 || sig.parameters[0].1.substitute(machine, &args) != carrier {
                return Err(TypeError::for_reason(&site_at(f, n), CleanupSelectionError::InvalidSignature));
            }
            if sig
                .crash_ceiling
                .as_ref()
                .is_some_and(|causes| !causes.is_empty())
            {
                return Err(TypeError::for_reason(&site_at(f, n), CleanupEffectError::PublishesCrashRoute));
            }
            // Defensive handoff check: ready local machines have already published effects,
            // and checked-library signatures are imported with their effects.
            let effects = stage
                .machine_effects
                .iter()
                .find(|e| e.machine == *machine)
                // The effect pass publishes a row for each ready machine before this check.
                .ok_or_else(|| TypeError::at(f, n, "missing cleanup machine effects"))?;
            if effects.pause.block || effects.pause.suspend {
                return Err(TypeError::for_reason(&site_at(f, n), CleanupEffectError::MayPause));
            }
            stage.cleanups.push(CleanupSelection {
                site: site_at(f, n),
                owner: owner.clone(),
                machine: machine.clone(),
                arguments: args,
            });
            stage.obligations.push(TypingObligation {
                node: f.node_id(n),
                site: site_at(f, n),
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
            if node.kind == NodeKind::QualifiedType && dependencies.node_is_ready(unit.file.node_id(n)) {
                let ty = unit.annotation(n, &catalog.types)?;
                compatible(&ty, &plans, catalog).map_err(|cause| TypeError::for_reason(&site_at(unit.file, n), cause))?;
            }
        }
    }
    for expression in &stage.expressions {
        if dependencies.node_is_ready(expression.node) {
            compatible(&expression.ty, &plans, catalog).map_err(|cause| TypeError::for_reason(&expression.site, cause))?;
        }
    }
    Ok(())
}

fn compatible(
    ty: &ValueType,
    plans: &HashMap<&DeclarationId, &CleanupSelection>,
    catalog: &DeclarationCatalog<'_>,
) -> Result<(), CleanupSelectionError> {
    let mut selected = None;
    if let Some((id, args)) = ty.carrier().nominal_parts()
        && let Some(plan) = plans.get(id)
    {
        selected = Some((
            &plan.machine,
            plan.arguments
                .iter()
                .map(|t| catalog.types.apply_type_arguments(t, id, args))
                .collect::<Vec<_>>(),
        ));
    }
    for d in ty.domains() {
        if let Some(plan) = plans.get(&d.declaration) {
            let args = catalog.types.domains.bind_arguments(d, ty.carrier());
            if args.len() != catalog.types.arity(&d.declaration) {
                return Err(CleanupSelectionError::IncompleteDomainApplication);
            }
            let candidate = (
                &plan.machine,
                plan.arguments
                    .iter()
                    .map(|t| t.substitute(&d.declaration, &args))
                    .collect::<Vec<_>>(),
            );
            if selected.as_ref().is_some_and(|prior| prior != &candidate) {
                return Err(CleanupSelectionError::ConflictingSelections);
            }
            selected = Some(candidate);
        }
    }
    Ok(())
}
