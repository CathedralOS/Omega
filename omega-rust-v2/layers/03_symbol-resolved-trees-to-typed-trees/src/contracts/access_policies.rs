use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::typing::preparation_dependencies::PreparationDependencies;
use crate::{AccessPolicySelection, ObligationKind, Site, TypeError, TypedStage, TypingObligation};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

pub(crate) fn check_access_policies(
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'_>,
    dependencies: &PreparationDependencies<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    for unit in units {
        let f = unit.file;
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
            if !dependencies.node_is_ready(f.node_id(n)) {
                continue;
            }
            let requirement = catalog
                .access_plan_requirement
                .ok_or_else(|| TypeError::at(f, head, "access policy requires the admitted AccessPolicy plan requirement"))?;
            let row = out
                .conformance_rows
                .iter()
                .find(|r| r.conformance.as_ref() == Some(policy) && &r.requirement.declaration == requirement)
                .ok_or_else(|| TypeError::at(f, head, "selected policy does not supply the admitted plan requirement"))?;
            let machine = &row.implementation;
            let args = unit.application_arguments(head, policy, &catalog.types)?;
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
                node: f.node_id(n),
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
