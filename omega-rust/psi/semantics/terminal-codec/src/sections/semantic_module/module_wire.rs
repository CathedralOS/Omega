//! Canonical terminal-module envelope wire format.
//!
//! This module owns the ordered top-level declaration tables and exact module
//! vocabulary envelope. Individual declaration, machine, scalar, proof, and
//! structural payloads remain in their dedicated sibling wire modules.
//!
//! `encode_raw` and `decode_module_body` are the ordered section list: each
//! row kind is encoded and decoded by a named pair in this folder
//! (`declaration_wire`, `placed_view_wire`, `borrow_wire`,
//! `float_meaning_wire`, `evidence_wire`, `proof_output_wire`,
//! `recursive_component_wire`, `closed_conformance_wire`,
//! `carry_and_suspension_wire`, `scalar_block_invariant_wire`,
//! `operation_crash_contract_wire`) or in a sibling wire module.

mod borrow_wire;
mod carry_and_suspension_wire;
mod recursive_component_wire;

mod closed_conformance_wire;
mod declaration_wire;
mod evidence_wire;
mod float_meaning_wire;
mod operation_crash_contract_wire;
mod placed_view_wire;
mod proof_output_wire;
mod scalar_block_invariant_wire;

use terminal_psi::{TerminalModule, TerminalRootServiceReach, VocabularyMarker};

use super::dynamic_dispatch_wire::{
    decode_direct_dynamic_dispatches, decode_dynamic_conformance_selections,
    decode_dynamic_descriptor_arguments, decode_dynamic_descriptor_parameters,
    decode_indirect_dynamic_dispatches, decode_parameter_dynamic_dispatches,
    decode_rebound_dynamic_descriptors, decode_stored_dynamic_descriptors,
    decode_stored_dynamic_dispatches, encode_direct_dynamic_dispatches,
    encode_dynamic_conformance_selections, encode_dynamic_descriptor_arguments,
    encode_dynamic_descriptor_parameters, encode_indirect_dynamic_dispatches,
    encode_parameter_dynamic_dispatches, encode_rebound_dynamic_descriptors,
    encode_stored_dynamic_descriptors, encode_stored_dynamic_dispatches,
};
use super::proof_declaration_wire::{
    decode_proposition_application, decode_proposition_declaration, encode_proposition_application,
    encode_proposition_declaration,
};
use super::provider_candidate_wire::{decode_provider_candidate, encode_provider_candidate};
use super::quotient_correspondence_wire::{
    decode_quotient_correspondence, encode_quotient_correspondence,
};
use super::structural_signature_wire::{decode_boundary_machine, encode_boundary_machine};
use super::structural_type_wire::{decode_structural_type, encode_structural_type};
use super::wire::{Reader, Writer};
use super::{CodecError, FORMAT_MARKER, MAGIC};
use crate::sections::semantic_module::module_wire::borrow_wire::{
    decode_reborrow_restored_call_use, decode_reborrow_root_handoff,
    encode_reborrow_restored_call_use, encode_reborrow_root_handoff,
};
use crate::sections::semantic_module::module_wire::carry_and_suspension_wire::{
    decode_suspension_call_plan, decode_suspension_call_site, encode_suspension_call_plan,
    encode_suspension_call_site,
};
use crate::sections::semantic_module::module_wire::recursive_component_wire::{
    decode_proof_recursive_component, encode_proof_recursive_component,
};
use crate::sections::semantic_module::wire::{decode_counted, decode_ids};

pub(crate) fn encode_raw(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    writer.u16(module.vocabulary_marker.get());
    writer.id(module.entry);
    super::scalar_qualification_wire::encode(&mut writer, &module.scalar_qualifications)?;
    writer.len("structural types", module.structural_types.len())?;
    for declaration in &module.structural_types {
        encode_structural_type(&mut writer, declaration)?;
    }
    writer.len("structural domains", module.structural_domains.len())?;
    for declaration in &module.structural_domains {
        declaration_wire::encode_structural_domain(&mut writer, declaration)?;
    }
    writer.len("services", module.services.len())?;
    for declaration in &module.services {
        declaration_wire::encode_service(&mut writer, declaration)?;
    }
    writer.len(
        "concrete root service reach",
        module.root_service_reach.concrete.len(),
    )?;
    for service in &module.root_service_reach.concrete {
        writer.id(*service);
    }
    writer.len(
        "installation reach dependencies",
        module.root_service_reach.installation_dependencies.len(),
    )?;
    for dependency in &module.root_service_reach.installation_dependencies {
        declaration_wire::encode_installation_reach_dependency(&mut writer, dependency)?;
    }
    writer.len("placed-view inputs", module.placed_view_inputs.len())?;
    for input in &module.placed_view_inputs {
        placed_view_wire::encode_placed_view_input(&mut writer, input)?;
    }
    writer.len(
        "reborrow root handoffs",
        module.reborrow_root_handoffs.len(),
    )?;
    for handoff in &module.reborrow_root_handoffs {
        encode_reborrow_root_handoff(&mut writer, handoff)?;
    }
    writer.len(
        "reborrow restored call uses",
        module.reborrow_restored_call_uses.len(),
    )?;
    for use_row in &module.reborrow_restored_call_uses {
        encode_reborrow_restored_call_use(&mut writer, use_row)?;
    }
    writer.len("boundary machines", module.boundary_machines.len())?;
    for declaration in &module.boundary_machines {
        encode_boundary_machine(&mut writer, declaration)?;
    }
    writer.len("provider candidates", module.provider_candidates.len())?;
    for candidate in &module.provider_candidates {
        encode_provider_candidate(&mut writer, candidate)?;
    }
    writer.len(
        "float-meaning projections",
        module.float_meaning_projections.len(),
    )?;
    for projection in &module.float_meaning_projections {
        float_meaning_wire::encode_float_meaning_projection(&mut writer, projection)?;
    }
    writer.len(
        "float-meaning equalities",
        module.float_meaning_equalities.len(),
    )?;
    for proposition in &module.float_meaning_equalities {
        float_meaning_wire::encode_float_meaning_equality(&mut writer, proposition)?;
    }
    writer.len(
        "proposition declarations",
        module.proposition_declarations.len(),
    )?;
    for declaration in &module.proposition_declarations {
        encode_proposition_declaration(&mut writer, declaration)?;
    }
    writer.len(
        "proposition applications",
        module.proposition_applications.len(),
    )?;
    for application in &module.proposition_applications {
        encode_proposition_application(&mut writer, application)?;
    }
    writer.len("evidence terms", module.evidence_terms.len())?;
    for term in &module.evidence_terms {
        evidence_wire::encode_evidence_term(&mut writer, term)?;
    }
    writer.len(
        "evidence contract lanes",
        module.evidence_contract_lanes.len(),
    )?;
    for lane in &module.evidence_contract_lanes {
        evidence_wire::encode_evidence_contract_lane(&mut writer, lane)?;
    }
    writer.len("proof-output invocations", module.proof_output_calls.len())?;
    for invocation in &module.proof_output_calls {
        proof_output_wire::encode_proof_output_call(&mut writer, invocation)?;
    }
    writer.len(
        "proof recursive components",
        module.proof_recursive_components.len(),
    )?;
    for component in &module.proof_recursive_components {
        encode_proof_recursive_component(&mut writer, component)?;
    }
    writer.len(
        "closed conformance applications",
        module.closed_conformance_applications.len(),
    )?;
    for application in &module.closed_conformance_applications {
        closed_conformance_wire::encode_closed_conformance_application(&mut writer, application)?;
    }
    encode_dynamic_descriptor_parameters(&mut writer, &module.dynamic_dispatch.parameters)?;
    encode_dynamic_descriptor_arguments(&mut writer, &module.dynamic_dispatch.arguments)?;
    encode_dynamic_conformance_selections(&mut writer, &module.dynamic_dispatch.selections)?;
    encode_rebound_dynamic_descriptors(&mut writer, &module.dynamic_dispatch.rebound_descriptors)?;
    encode_stored_dynamic_descriptors(&mut writer, &module.dynamic_dispatch.stored_descriptors)?;
    encode_direct_dynamic_dispatches(&mut writer, &module.dynamic_dispatch.direct_dispatches)?;
    encode_indirect_dynamic_dispatches(&mut writer, &module.dynamic_dispatch.indirect_dispatches)?;
    encode_stored_dynamic_dispatches(&mut writer, &module.dynamic_dispatch.stored_dispatches)?;
    encode_parameter_dynamic_dispatches(
        &mut writer,
        &module.dynamic_dispatch.parameter_dispatches,
    )?;
    writer.u32(module.suspension_call_plan_count);
    writer.len("suspension call sites", module.suspension_call_sites.len())?;
    for site in &module.suspension_call_sites {
        encode_suspension_call_site(&mut writer, site);
    }
    writer.len("suspension call plans", module.suspension_call_plans.len())?;
    for plan in &module.suspension_call_plans {
        encode_suspension_call_plan(&mut writer, plan)?;
    }
    writer.len(
        "quotient correspondences",
        module.quotient_correspondences.len(),
    )?;
    for correspondence in &module.quotient_correspondences {
        encode_quotient_correspondence(&mut writer, correspondence)?;
    }
    writer.len(
        "scalar block invariants",
        module.scalar_block_invariants.len(),
    )?;
    for invariant in &module.scalar_block_invariants {
        scalar_block_invariant_wire::encode_scalar_block_invariant(&mut writer, invariant)?;
    }
    writer.len(
        "operation crash contracts",
        module.operation_crash_contracts.len(),
    )?;
    for contract in &module.operation_crash_contracts {
        operation_crash_contract_wire::encode_operation_crash_contract(&mut writer, contract)?;
    }
    writer.len("machines", module.machines.len())?;
    for machine in &module.machines {
        super::machine_wire::encode_machine(&mut writer, machine)?;
    }
    Ok(writer.finish())
}

pub(crate) fn decode_module_body(reader: &mut Reader<'_>) -> Result<TerminalModule, CodecError> {
    let vocabulary_marker_raw = reader.u16()?;
    if vocabulary_marker_raw != VocabularyMarker::CURRENT.get() {
        return Err(CodecError::UnsupportedVocabularyMarker(
            vocabulary_marker_raw,
        ));
    }
    let vocabulary_marker = VocabularyMarker::CURRENT;
    let entry = reader.id("MachineId")?;
    let scalar_qualifications = super::scalar_qualification_wire::decode(reader)?;
    let structural_types = decode_counted(reader, decode_structural_type)?;
    let structural_domains = decode_counted(reader, declaration_wire::decode_structural_domain)?;
    let services = decode_counted(reader, declaration_wire::decode_service)?;
    let concrete_root_service_reach = decode_ids(reader, "ServiceId")?;
    let installation_reach_dependencies = decode_counted(
        reader,
        declaration_wire::decode_installation_reach_dependency,
    )?;
    let placed_view_inputs = decode_counted(reader, placed_view_wire::decode_placed_view_input)?;
    let reborrow_root_handoffs = decode_counted(reader, decode_reborrow_root_handoff)?;
    let reborrow_restored_call_uses = decode_counted(reader, decode_reborrow_restored_call_use)?;
    let boundary_machines = decode_counted(reader, decode_boundary_machine)?;
    let provider_candidates = decode_counted(reader, decode_provider_candidate)?;
    let float_meaning_projections =
        decode_counted(reader, float_meaning_wire::decode_float_meaning_projection)?;
    let float_meaning_equalities =
        decode_counted(reader, float_meaning_wire::decode_float_meaning_equality)?;
    let count = reader.count()?;
    let mut proposition_declarations = Vec::with_capacity(count as usize);
    for _ in 0..count {
        proposition_declarations.push(decode_proposition_declaration(reader)?);
    }
    let count = reader.count()?;
    let mut proposition_applications = Vec::with_capacity(count as usize);
    for _ in 0..count {
        proposition_applications.push(decode_proposition_application(reader)?);
    }
    let evidence_terms = decode_counted(reader, evidence_wire::decode_evidence_term)?;
    let evidence_contract_lanes =
        decode_counted(reader, evidence_wire::decode_evidence_contract_lane)?;
    let proof_output_calls = decode_counted(reader, proof_output_wire::decode_proof_output_call)?;
    let proof_recursive_components = decode_counted(reader, decode_proof_recursive_component)?;
    let closed_conformance_applications = decode_counted(
        reader,
        closed_conformance_wire::decode_closed_conformance_application,
    )?;
    let (
        dynamic_descriptor_parameters,
        dynamic_descriptor_arguments,
        dynamic_conformance_selections,
        rebound_dynamic_descriptors,
        stored_dynamic_descriptors,
        direct_dynamic_dispatches,
        indirect_dynamic_dispatches,
        stored_dynamic_dispatches,
        parameter_dynamic_dispatches,
    ) = (
        decode_dynamic_descriptor_parameters(reader)?,
        decode_dynamic_descriptor_arguments(reader)?,
        decode_dynamic_conformance_selections(reader)?,
        decode_rebound_dynamic_descriptors(reader)?,
        decode_stored_dynamic_descriptors(reader)?,
        decode_direct_dynamic_dispatches(reader)?,
        decode_indirect_dynamic_dispatches(reader)?,
        decode_stored_dynamic_dispatches(reader)?,
        decode_parameter_dynamic_dispatches(reader)?,
    );
    let (suspension_call_plan_count, suspension_call_sites, suspension_call_plans) = (
        reader.u32()?,
        decode_counted(reader, decode_suspension_call_site)?,
        decode_counted(reader, decode_suspension_call_plan)?,
    );
    let quotient_correspondences = decode_counted(reader, decode_quotient_correspondence)?;
    let scalar_block_invariants = decode_counted(
        reader,
        scalar_block_invariant_wire::decode_scalar_block_invariant,
    )?;
    let operation_crash_contracts = decode_counted(
        reader,
        operation_crash_contract_wire::decode_operation_crash_contract,
    )?;
    let machine_count = reader.count()?;
    let mut machines = Vec::new();
    for _ in 0..machine_count {
        machines.push(super::machine_wire::decode_machine(reader)?);
    }
    Ok(TerminalModule {
        scalar_qualifications,
        scalar_block_invariants,
        operation_crash_contracts,
        vocabulary_marker,
        entry,
        structural_types,
        structural_domains,
        services,
        root_service_reach: TerminalRootServiceReach {
            concrete: concrete_root_service_reach,
            installation_dependencies: installation_reach_dependencies,
        },
        placed_view_inputs,
        reborrow_root_handoffs,
        reborrow_restored_call_uses,
        boundary_machines,
        provider_candidates,
        float_meaning_projections,
        float_meaning_equalities,
        proposition_declarations,
        proposition_applications,
        evidence_terms,
        evidence_contract_lanes,
        proof_output_calls,
        proof_recursive_components,
        closed_conformance_applications,
        dynamic_dispatch: terminal_psi::TerminalDynamicDispatchCatalog {
            parameters: dynamic_descriptor_parameters,
            arguments: dynamic_descriptor_arguments,
            selections: dynamic_conformance_selections,
            rebound_descriptors: rebound_dynamic_descriptors,
            stored_descriptors: stored_dynamic_descriptors,
            direct_dispatches: direct_dynamic_dispatches,
            indirect_dispatches: indirect_dynamic_dispatches,
            stored_dispatches: stored_dynamic_dispatches,
            parameter_dispatches: parameter_dynamic_dispatches,
        },
        suspension_call_plan_count,
        suspension_call_sites,
        suspension_call_plans,
        quotient_correspondences,
        machines,
    })
}
