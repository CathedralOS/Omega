use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::typing::preparation_dependencies::PreparationDependencies;
use crate::{AccessPolicySelection, ObligationKind, TypeError, TypedStage, TypingObligation, ValueType};
use omega_resolved::{DeclarationId, Symbol, TreeNodeId};
use omega_syntax::NodeKind;
use omega_typed::site_at;

pub(crate) struct PolicyApplication {
    clause: TreeNodeId,
    head: usize,
    domain: DeclarationId,
    policy: DeclarationId,
    arguments: Vec<ValueType>,
}

/// Discover policy arguments before the typing pass collects its static demands.
/// The selection itself also needs the implementation's inferred effects.
pub(crate) fn policy_applications(
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'_>,
) -> Result<Vec<PolicyApplication>, TypeError> {
    let mut applications = Vec::new();
    for unit in units {
        let file = unit.file;
        for (node, clause) in file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, clause)| clause.kind == NodeKind::Access)
        {
            let domain = unit.declaration(
                clause
                    .parent
                    .ok_or_else(|| TypeError::unsupported(file, node))?,
            )?;
            let head = unit.single_child(node)?;
            let Some(Symbol::Declaration(policy)) = unit.symbol(head) else {
                return Err(TypeError::at(file, head, "access requires an exact policy implementation"));
            };
            let arguments = if policy.kind == NodeKind::Conformance {
                match unit.application_arguments(head, policy, &catalog.types) {
                    Ok(arguments) => arguments,
                    Err(error) if error.waiting_for_static_value() => continue,
                    Err(error) => return Err(error),
                }
            } else {
                // The requirement check diagnoses a declaration of the wrong kind.
                Vec::new()
            };
            applications.push(PolicyApplication {
                clause: file.node_id(node),
                head,
                domain: domain.clone(),
                policy: policy.clone(),
                arguments,
            });
        }
    }
    Ok(applications)
}

pub(crate) fn check_access_policies(
    applications: Vec<PolicyApplication>,
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'_>,
    dependencies: &PreparationDependencies<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    for PolicyApplication {
        clause,
        head,
        domain,
        policy,
        arguments,
    } in applications
    {
        if !dependencies.node_is_ready(clause) {
            continue;
        }
        let file = units[clause.file].file;
        let requirement = catalog
            .access_plan_requirement
            .ok_or_else(|| TypeError::at(file, head, "access policy requires the admitted AccessPolicy plan requirement"))?;
        let row = out
            .conformance_rows
            .iter()
            .find(|r| r.conformance.as_ref() == Some(&policy) && &r.requirement.declaration == requirement)
            .ok_or_else(|| TypeError::at(file, head, "selected policy does not supply the admitted plan requirement"))?;
        let machine = &row.implementation;
        let effects = out
            .machine_effects
            .iter()
            .find(|e| e.machine == *machine)
            .ok_or_else(|| TypeError::at(file, head, "missing access plan machine effects"))?;
        if effects.pause.suspend
            || effects.pause.block
            || !effects.reach.is_empty()
            || !effects.invokes.is_empty()
            || effects
                .crash_ceiling
                .as_ref()
                .is_some_and(|causes| !causes.is_empty())
        {
            return Err(TypeError::at(file, head, "access plan computation must be effect-free"));
        }
        let site = site_at(file, clause.node);
        out.obligations.push(TypingObligation {
            node: clause,
            site: site.clone(),
            kind: ObligationKind::EvaluateAccessPlan {
                domain: domain.clone(),
                machine: machine.clone(),
            },
        });
        out.access_policies.push(AccessPolicySelection {
            site,
            domain,
            policy,
            arguments,
            plan_machine: machine.clone(),
        });
    }
    Ok(())
}
