use super::super::{id, refresh_identity, scalar_boundary_call_unit, unit};
use super::provider_specialization::provider_attachment_specialization_unit;
use crate::{
    expected_definitions, expected_edges, expected_ownership, expected_provenance, expected_uses,
    reconstruct_fact_index, refresh_root_service_reach,
};
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{OperationId, ServiceId};

pub(crate) fn service_declarations() -> Vec<terminal_psi::ServiceDeclaration> {
    let root = id(701, ServiceId::new);
    let middle = id(702, ServiceId::new);
    let leaf = id(703, ServiceId::new);
    vec![
        terminal_psi::ServiceDeclaration {
            id: root,
            identity: "validation::service-root".into(),
            parents: Vec::new(),
        },
        terminal_psi::ServiceDeclaration {
            id: middle,
            identity: "validation::service-middle".into(),
            parents: vec![root],
        },
        terminal_psi::ServiceDeclaration {
            id: leaf,
            identity: "validation::service-leaf".into(),
            parents: vec![root, middle],
        },
    ]
}

pub(crate) fn install_service_catalog(unit: &mut PsiOptimizationUnit) {
    let services = service_declarations();
    let ceiling = services
        .iter()
        .map(|service| service.id)
        .collect::<Vec<_>>();
    unit.services = services.into();
    for function in &mut unit.functions {
        function.published_service_ceiling = ceiling.clone();
    }
    for boundary in &mut unit.boundary_machines {
        boundary.published_service_ceiling = ceiling.clone();
    }
    refresh_root_service_reach(unit).expect("service fixture has a closed root reach");
    refresh_identity(unit);
}

pub(crate) fn service_effect_unit() -> PsiOptimizationUnit {
    let mut candidate = unit();
    install_service_catalog(&mut candidate);
    let block = candidate.functions[0].blocks[0].id;
    let mut node = candidate.functions[0].blocks[0].nodes[0].clone();
    node.operation = AbstractOperation::PortWrite {
        psi_operation: id(704, OperationId::new),
        service: id(703, ServiceId::new),
        port: 0x3f8,
        value: 0x41,
    };
    node.provenance = expected_provenance(&node.operation);
    node.fuel = node
        .provenance
        .iter()
        .copied()
        .map(|site| optimization_unit::FuelSettlement { site, units: 1 })
        .collect();
    node.definitions = expected_definitions(&node.operation, block, 1);
    node.uses = expected_uses(&node.operation, block, 1);
    node.successors = expected_edges(&node.operation);
    node.ownership = expected_ownership(&node.operation);
    candidate.functions[0].blocks[0].nodes.insert(1, node);
    for index in 0..candidate.functions[0].blocks[0].nodes.len() {
        let operation = candidate.functions[0].blocks[0].nodes[index]
            .operation
            .clone();
        let node = &mut candidate.functions[0].blocks[0].nodes[index];
        node.effect.input = index as u64;
        node.effect.output = index as u64 + 1;
        node.provenance = expected_provenance(&operation);
        node.fuel = node
            .provenance
            .iter()
            .copied()
            .map(|site| optimization_unit::FuelSettlement { site, units: 1 })
            .collect();
        node.definitions = expected_definitions(&operation, block, index as u32);
        node.uses = expected_uses(&operation, block, index as u32);
        node.successors = expected_edges(&operation);
        node.ownership = expected_ownership(&operation);
    }
    candidate.functions[0].facts = reconstruct_fact_index(&candidate.functions[0]);
    refresh_root_service_reach(&mut candidate).expect("PortWrite fixture has exact root reach");
    refresh_identity(&mut candidate);
    candidate
}

pub(crate) fn provider_service_unit() -> PsiOptimizationUnit {
    let mut candidate = provider_attachment_specialization_unit();
    install_service_catalog(&mut candidate);
    let boundary = candidate.boundary_machines[0].id;
    let requirement_identity = candidate.boundary_machines[0].identity.clone();
    let callee = candidate.functions[0].machine;
    let ceiling = service_declarations()
        .iter()
        .map(|service| service.id)
        .collect::<Vec<_>>();
    candidate
        .provider_candidates
        .push(terminal_psi::ProviderCandidateConformance {
            boundary,
            requirement_identity,
            provider_identity: "validation::service-provider".into(),
            candidate_identity: "validation::service-provider-candidate".into(),
            candidate: callee,
            signature: terminal_psi::ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: ceiling,
            },
        });
    refresh_identity(&mut candidate);
    candidate
}

pub(crate) fn installation_root_service_unit() -> PsiOptimizationUnit {
    let mut candidate = scalar_boundary_call_unit();
    install_service_catalog(&mut candidate);
    let boundary = &candidate.boundary_machines[0];
    candidate.root_service_reach.installation_dependencies =
        vec![terminal_psi::InstallationReachDependency {
            requirement_identity: boundary.identity.clone(),
            upper_bound: boundary.published_service_ceiling.clone(),
        }];
    refresh_root_service_reach(&mut candidate)
        .expect("installation-bound fixture has exact root reach");
    refresh_identity(&mut candidate);
    candidate
}

pub(crate) fn multiple_installation_root_service_unit() -> PsiOptimizationUnit {
    let mut candidate = provider_attachment_specialization_unit();
    install_service_catalog(&mut candidate);
    candidate.root_service_reach.installation_dependencies = candidate.boundary_machines[..2]
        .iter()
        .map(|boundary| terminal_psi::InstallationReachDependency {
            requirement_identity: boundary.identity.clone(),
            upper_bound: boundary.published_service_ceiling.clone(),
        })
        .collect();
    candidate
        .root_service_reach
        .installation_dependencies
        .sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
    refresh_root_service_reach(&mut candidate)
        .expect("multi-dependency fixture has exact root reach");
    refresh_identity(&mut candidate);
    candidate
}
