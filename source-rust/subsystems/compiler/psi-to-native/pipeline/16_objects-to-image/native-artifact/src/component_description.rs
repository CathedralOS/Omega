//! Publish the component identity, provider receipts and stack demand of the finished native image.
use component_description::encode_component_description_checked;
use component_description::inventory::{
    DerivedInventory, derive_component_inventory, port_mechanism_assumption,
    register_mechanism_assumption, requirement_contract_identity,
};
use component_description::records::*;
use semantic_vocabulary::MachineId;
use std::collections::BTreeMap;
use terminal_psi::{TerminalModule, TerminalPsiIdentity};
use wire::digest_encoder::hex;

pub(super) fn publish_component_description(
    facts: ComponentDescriptionFacts<'_>,
) -> Result<(TerminalPsiIdentity, Vec<u8>), DescribeError> {
    let module = facts.artifact.semantic_module();
    let description = describe_decoded_component_facts(facts, module)?;
    let subject =
        terminal_codec::terminal_psi_identity(module).map_err(DescribeError::ArtifactIdentity)?;
    let encoded = encode_component_description_checked(&description)?;
    Ok((subject, encoded))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StackDemandFacts {
    pub entry: MachineId,
    pub ceiling_bytes: u64,
    pub stack_alignment: u32,
}

fn component_references_plan(
    module: &TerminalModule,
    inventory: &DerivedInventory,
    plan: &effects::provider_plan::ProviderPlan,
) -> bool {
    plan.rows.iter().any(|row| {
        component_references_requirement(
            module,
            inventory,
            plan,
            plan.identities.text(row.requirement_identity),
        )
    })
}

fn component_references_requirement(
    module: &TerminalModule,
    inventory: &DerivedInventory,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: &str,
) -> bool {
    inventory
        .called_requirement_identities
        .iter()
        .any(|called| called == requirement)
        || module.provider_candidates.iter().any(|candidate| {
            module
                .identities
                .text(candidate.source_requirement_identity)
                == Some(requirement)
                && module.identities.text(candidate.provider_identity)
                    == Some(plan.provider_type.as_str())
        })
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ComponentDescriptionFacts<'a> {
    pub artifact: &'a terminal_codec::CanonicalTerminalArtifact,
    pub selected_provider_plans: &'a effects::SelectedProviderPlanFacts,
    pub stack_demand: Option<StackDemandFacts>,
    pub realization_identity: Option<[u8; 32]>,
}

fn describe_decoded_component_facts(
    facts: ComponentDescriptionFacts<'_>,
    module: &TerminalModule,
) -> Result<ComponentDescription, DescribeError> {
    let artifact_bytes = facts.artifact.to_bytes();
    let inventory = derive_component_inventory(module)?;

    let selected = facts.selected_provider_plans;
    let providers: Vec<RetainedProvider> = selected
        .plans()
        .iter()
        .filter(|plan| component_references_plan(module, &inventory, plan))
        .map(|plan| {
            let mut requirement_identities: Vec<String> = plan
                .rows
                .iter()
                .map(|row| plan.identities.text(row.requirement_identity).to_owned())
                .filter(|requirement| {
                    component_references_requirement(module, &inventory, plan, requirement)
                })
                .collect();
            requirement_identities.sort();
            requirement_identities.dedup();
            RetainedProvider {
                report_identity: plan.report_fingerprint(),
                plan_digest: *plan.identity_digest().as_bytes(),
                plan_input: plan.identity_preimage(),
                requirement_identities,
            }
        })
        .collect();

    let mut sealed: BTreeMap<&str, [u8; 32]> = BTreeMap::new();
    for provider in &providers {
        for requirement in &provider.requirement_identities {
            sealed
                .entry(requirement.as_str())
                .or_insert(provider.plan_digest);
        }
    }

    let mut imports = Vec::new();
    let mut outgoing = Vec::new();
    let mut assumptions = Vec::new();
    let mut obligations = Vec::new();

    for requirement_identity in &inventory.called_requirement_identities {
        let evidence = match sealed.get(requirement_identity.as_str()) {
            Some(plan_digest) => OutgoingEvidence::ProviderSealed(*plan_digest),
            None => OutgoingEvidence::ModuleDerived,
        };
        outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::BoundaryRequirement,
            identity: requirement_identity.clone(),
            evidence,
        });
        if !sealed.contains_key(requirement_identity.as_str()) {
            let slot = u32::try_from(imports.len()).unwrap_or(u32::MAX);
            imports.push(ImportSlot {
                slot,
                requirement_identity: requirement_identity.clone(),
                contract_identity: requirement_contract_identity(requirement_identity),
            });
        }
    }

    for (service, port, value) in &inventory.port_writes {
        let assumption = port_mechanism_assumption(*service, *port, *value);
        assumptions.push(assumption);
        outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::PortSpaceWrite,
            identity: format!("port-write:{}:{port}:{value}", service.get()),
            evidence: OutgoingEvidence::PhysicalMechanism(assumption),
        });
    }

    for (service, operation) in &inventory.register_accesses {
        let assumption = register_mechanism_assumption(*service, *operation);
        assumptions.push(assumption);
        outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::RegisterAccess,
            identity: format!(
                "register-access:{}:{}",
                service.get(),
                operation.catalog_tag()
            ),
            evidence: OutgoingEvidence::PhysicalMechanism(assumption),
        });
    }

    for service in &inventory.concrete_service_reach {
        outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::ServiceCeiling,
            identity: format!("service-ceiling:{}", service.get()),
            evidence: OutgoingEvidence::ModuleDerived,
        });
    }

    if let Some(stack) = facts.stack_demand {
        obligations.push(InstallationObligation {
            kind: ObligationKind::StackProvision,
            identity: format!(
                "stack-provision:{}:{}:{}",
                stack.entry.get(),
                stack.ceiling_bytes,
                stack.stack_alignment,
            ),
            detail: "provision the exact selected-entry stack closure under a fresh profile"
                .to_string(),
        });
    }
    for import in &imports {
        obligations.push(InstallationObligation {
            kind: ObligationKind::ImportBinding,
            identity: import.requirement_identity.clone(),
            detail: "bind one exact provider occurrence for this import".to_string(),
        });
    }
    for provider in &providers {
        obligations.push(InstallationObligation {
            kind: ObligationKind::ProviderOccurrence,
            identity: format!("provider-occurrence:{}", hex(&provider.plan_digest)),
            detail: "admit one retained provider occurrence under a fresh profile".to_string(),
        });
    }
    for row in &inventory.custody {
        if row.kind == CustodyKind::ProgramLocalRootIntroduction {
            obligations.push(InstallationObligation {
                kind: ObligationKind::ResourceAdmission,
                identity: row.identity.clone(),
                detail: "admit the per-occurrence resource capacity of this schema".to_string(),
            });
        }
    }

    assumptions.sort();
    assumptions.dedup();
    imports.sort();
    outgoing.sort();
    obligations.sort();
    obligations.dedup();

    Ok(ComponentDescription {
        schema: COMPONENT_DESCRIPTION_SCHEMA_V3,
        frontier: DescriptionFrontier::TerminalArtifactClosure,
        artifact_bytes,
        imports,
        exports: inventory
            .export_contracts
            .iter()
            .map(|contract| ExportSurface {
                identity: contract.identity.clone(),
            })
            .collect(),
        entries: inventory.entries,
        outgoing,
        service_bounds: inventory.service_bounds,
        custody: inventory.custody,
        providers,
        provider_closure_digest: *selected.identity_digest().as_bytes(),
        obligations,
        assumptions,
        realization_identity: facts.realization_identity,
    })
}
