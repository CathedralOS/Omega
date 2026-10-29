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
mod boundary_application_wire;
mod callback_application_wire;
mod carry_and_suspension_wire;
pub(crate) mod quotient_wire;
mod recursive_component_wire;

mod closed_conformance_wire;
pub(crate) mod declaration_wire;
mod dynamic_dispatch_wire;
mod evidence_wire;
mod float_meaning_wire;
mod folded_constant_wire;
mod operation_crash_contract_wire;
pub(crate) mod placed_plan_wire;
pub(crate) mod placed_view_wire;
pub(crate) mod placement_lifecycle_wire;
mod proof_output_wire;
mod proof_scope_wire;
mod scalar_application_wire;
mod scalar_block_invariant_wire;
mod scalar_condition_wire;
mod scalar_predicate_wire;
mod target_family_wire;
mod target_layout_wire;
mod task_activation_wire;
use super::external_provider_wire;

use terminal_psi::{
    TerminalIdentityIndex, TerminalIdentityTable, TerminalModule, TerminalRootServiceReach,
    VocabularyMarker,
};

use super::proof_declaration_wire::{
    decode_proposition_application, decode_proposition_declaration, encode_proposition_application,
    encode_proposition_declaration,
};
use super::provider_candidate_wire::{decode_provider_candidate, encode_provider_candidate};
use super::structural_signature_wire::{decode_boundary_machine, encode_boundary_machine};
use super::structural_type_wire::{decode_structural_type, encode_structural_type};
use super::wire::{Reader, Writer, strictly_increasing};
use super::{CodecError, FORMAT_MARKER, MAGIC};
use crate::sections::semantic_module::module_wire::borrow_wire::{
    decode_reborrow_restored_call_use, encode_reborrow_restored_call_use,
};
use crate::sections::semantic_module::module_wire::carry_and_suspension_wire::{
    decode_suspension_call_plan, decode_suspension_call_site, encode_suspension_call_plan,
    encode_suspension_call_site,
};
use crate::sections::semantic_module::module_wire::recursive_component_wire::{
    decode_proof_recursive_component, encode_proof_recursive_component,
};
use crate::sections::semantic_module::module_wire::task_activation_wire::{
    decode_demands, encode_demands,
};
use crate::sections::semantic_module::wire::{decode_bytes, decode_counted, decode_ids};

pub(crate) fn encode_raw(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    writer.u16(module.vocabulary_marker.get());
    target_family_wire::encode_callable(&mut writer, module.entry);
    writer.len("identities", module.identities.entries().len())?;
    for identity in module.identities.entries() {
        writer.string("identity", identity)?;
    }
    writer.len("indexed theorem roots", module.indexed_theorems.len())?;
    for theorem in &module.indexed_theorems {
        super::machine_declaration_wire::encode(&mut writer, &theorem.source_contract)?;
        writer.u32(theorem.ensure_ordinal);
        writer.u32(theorem.evidence_source_position);
        writer.u32(theorem.type_identity.0);
        writer.len("indexed theorem certificate", theorem.certificate.len())?;
        writer.bytes(&theorem.certificate);
        writer.len(
            "indexed theorem assumptions",
            theorem.assumption_closure.len(),
        )?;
        for position in &theorem.assumption_closure {
            writer.u32(*position);
        }
    }
    writer.len("mathematical carriers", module.mathematical_carriers.len())?;
    for carrier in &module.mathematical_carriers {
        writer.u32(carrier.type_identity.0);
        writer.len("mathematical carrier key", carrier.carrier_key.len())?;
        writer.bytes(&carrier.carrier_key);
    }
    super::scalar_qualification_wire::encode(&mut writer, &module.scalar_qualifications)?;
    quotient_wire::encode(&mut writer, &module.quotients)?;
    folded_constant_wire::encode(&mut writer, &module.folded_constants)?;
    scalar_application_wire::encode(&mut writer, &module.scalar_applications)?;
    writer.counted(
        "structural types",
        &module.structural_types,
        encode_structural_type,
    )?;
    writer.len("structural referents", module.structural_referents.len())?;
    for referent in &module.structural_referents {
        writer.id(referent.id);
        writer.u32(referent.identity.0);
    }
    writer.counted(
        "structural domains",
        &module.structural_domains,
        declaration_wire::encode_structural_domain,
    )?;
    writer.counted(
        "services",
        &module.services,
        declaration_wire::encode_service,
    )?;
    writer.ids(
        "concrete root service reach",
        &module.root_service_reach.concrete,
    )?;
    writer.counted(
        "installation reach dependencies",
        &module.root_service_reach.installation_dependencies,
        declaration_wire::encode_installation_reach_dependency,
    )?;
    writer.counted(
        "placed-view plans",
        &module.placed_view_plans,
        placed_plan_wire::encode,
    )?;
    writer.counted(
        "placement lifecycle plans",
        &module.placement_lifecycle_plans,
        placement_lifecycle_wire::encode,
    )?;
    writer.counted(
        "placed-view inputs",
        &module.placed_view_inputs,
        placed_view_wire::encode_placed_view_input,
    )?;
    writer.counted(
        "reborrow restored call uses",
        &module.reborrow_restored_call_uses,
        encode_reborrow_restored_call_use,
    )?;
    writer.counted(
        "boundary machines",
        &module.boundary_machines,
        encode_boundary_machine,
    )?;
    writer.counted(
        "external provider conformances",
        &module.external_providers,
        external_provider_wire::encode,
    )?;
    writer.counted(
        "provider candidates",
        &module.provider_candidates,
        encode_provider_candidate,
    )?;
    dynamic_dispatch_wire::encode_dynamic_dispatch(&mut writer, &module.dynamic_dispatch)?;
    writer.counted(
        "float-meaning projections",
        &module.float_meaning_projections,
        float_meaning_wire::encode_float_meaning_projection,
    )?;
    writer.counted(
        "float-meaning equalities",
        &module.float_meaning_equalities,
        float_meaning_wire::encode_float_meaning_equality,
    )?;
    writer.counted(
        "proposition declarations",
        &module.proposition_declarations,
        encode_proposition_declaration,
    )?;
    writer.counted(
        "proposition applications",
        &module.proposition_applications,
        encode_proposition_application,
    )?;
    writer.counted(
        "evidence terms",
        &module.evidence_terms,
        evidence_wire::encode_evidence_term,
    )?;
    writer.counted(
        "evidence contract lanes",
        &module.evidence_contract_lanes,
        evidence_wire::encode_evidence_contract_lane,
    )?;
    writer.counted(
        "proof-output invocations",
        &module.proof_output_calls,
        proof_output_wire::encode_proof_output_call,
    )?;
    writer.counted(
        "proof recursive components",
        &module.proof_recursive_components,
        encode_proof_recursive_component,
    )?;
    writer.counted(
        "closed conformance applications",
        &module.closed_conformance_applications,
        closed_conformance_wire::encode_closed_conformance_application,
    )?;
    writer.u32(module.suspension_call_plan_count);
    writer.len("suspension call sites", module.suspension_call_sites.len())?;
    for site in &module.suspension_call_sites {
        encode_suspension_call_site(&mut writer, site);
    }
    writer.counted(
        "suspension call plans",
        &module.suspension_call_plans,
        encode_suspension_call_plan,
    )?;
    writer.counted(
        "scalar block invariants",
        &module.scalar_block_invariants,
        scalar_block_invariant_wire::encode_scalar_block_invariant,
    )?;
    writer.counted(
        "operation crash contracts",
        &module.operation_crash_contracts,
        operation_crash_contract_wire::encode_operation_crash_contract,
    )?;
    writer.counted(
        "machines",
        &module.machines,
        super::machine_wire::encode_machine,
    )?;
    writer.counted(
        "target-machine families",
        &module.target_machine_families,
        target_family_wire::encode_family,
    )?;
    writer.counted(
        "target layout applications",
        &module.target_layout_applications,
        target_layout_wire::encode,
    )?;
    writer.counted(
        "boundary application demands",
        &module.boundary_application_demands,
        boundary_application_wire::encode_demand,
    )?;
    writer.counted(
        "callback applications",
        &module.callback_applications,
        callback_application_wire::encode,
    )?;
    writer.counted(
        "boundary application candidates",
        &module.boundary_application_candidates,
        boundary_application_wire::encode_candidate,
    )?;
    encode_demands(&mut writer, &module.task_activation_demands)?;
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
    let entry = target_family_wire::decode_callable(reader)?;
    let identity_entries = decode_counted(reader, |reader| reader.string("identity"))?;
    if identity_entries.first().map(String::as_str) != Some("") {
        return Err(CodecError::MalformedStructuralFoundation(
            "identity table must begin with the empty identity",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for identity in &identity_entries[1..] {
        if identity.is_empty() || !seen.insert(identity.as_str()) {
            return Err(CodecError::MalformedStructuralFoundation(
                "identity table entries must be non-empty and unique",
            ));
        }
    }
    let identities = TerminalIdentityTable::from(identity_entries);
    let indexed_theorems = decode_counted(reader, |reader| {
        let source_contract = super::machine_declaration_wire::decode(reader)?;
        let ensure_ordinal = reader.u32()?;
        let evidence_source_position = reader.u32()?;
        let type_identity = TerminalIdentityIndex(reader.u32()?);
        let certificate = decode_bytes(reader)?;
        super::mathematical_certificate_wire::decode_mathematical_certificate(&certificate)?;
        let assumption_closure = decode_counted(reader, |reader| reader.u32())?;
        if !strictly_increasing(assumption_closure.iter().copied()) || type_identity.is_absent() {
            return Err(CodecError::MalformedMathematicalCertificate(
                "invalid indexed theorem root",
            ));
        }
        Ok(terminal_psi::TerminalIndexedTheorem {
            source_contract,
            ensure_ordinal,
            evidence_source_position,
            type_identity,
            certificate,
            assumption_closure,
        })
    })?;
    let mathematical_carriers = decode_counted(reader, |reader| {
        let type_identity = TerminalIdentityIndex(reader.u32()?);
        let carrier_key = decode_bytes(reader)?;
        let decoded =
            super::mathematical_certificate_wire::decode_mathematical_certificate(&carrier_key)?;
        if !decoded
            .arena
            .structurally_equal(decoded.certificate.term, decoded.certificate.expected)
            || !decoded.certificate.context.is_empty()
            || decoded.certificate.level_arity != 0
            || type_identity.is_absent()
        {
            return Err(CodecError::MalformedMathematicalCertificate(
                "invalid mathematical carrier key",
            ));
        }
        Ok(terminal_psi::TerminalMathematicalCarrier {
            type_identity,
            carrier_key,
        })
    })?;
    let scalar_qualifications = super::scalar_qualification_wire::decode(reader)?;
    let quotients = quotient_wire::decode(reader)?;
    let folded_constants = folded_constant_wire::decode(reader)?;
    let scalar_applications = scalar_application_wire::decode(reader)?;
    let structural_types = decode_counted(reader, decode_structural_type)?;
    let structural_referents = decode_counted(reader, |reader| {
        Ok(terminal_psi::StructuralReferentIdentity {
            id: reader.id("StructuralTypeId")?,
            identity: TerminalIdentityIndex(reader.u32()?),
        })
    })?;
    let structural_domains = decode_counted(reader, declaration_wire::decode_structural_domain)?;
    let services = decode_counted(reader, declaration_wire::decode_service)?;
    let concrete_root_service_reach = decode_ids(reader, "ServiceId")?;
    let installation_reach_dependencies = decode_counted(
        reader,
        declaration_wire::decode_installation_reach_dependency,
    )?;
    let placed_view_plans = decode_counted(reader, placed_plan_wire::decode)?;
    let placement_lifecycle_plans = decode_counted(reader, placement_lifecycle_wire::decode)?;
    let placed_view_inputs = decode_counted(reader, placed_view_wire::decode_placed_view_input)?;
    let reborrow_restored_call_uses = decode_counted(reader, decode_reborrow_restored_call_use)?;
    let boundary_machines = decode_counted(reader, decode_boundary_machine)?;
    let external_providers = decode_counted(reader, external_provider_wire::decode)?;
    let provider_candidates = decode_counted(reader, decode_provider_candidate)?;
    let dynamic_dispatch = dynamic_dispatch_wire::decode_dynamic_dispatch(reader)?;
    let float_meaning_projections =
        decode_counted(reader, float_meaning_wire::decode_float_meaning_projection)?;
    let float_meaning_equalities =
        decode_counted(reader, float_meaning_wire::decode_float_meaning_equality)?;
    let proposition_declarations = decode_counted(reader, decode_proposition_declaration)?;
    let proposition_applications = decode_counted(reader, decode_proposition_application)?;
    let evidence_terms = decode_counted(reader, evidence_wire::decode_evidence_term)?;
    let evidence_contract_lanes =
        decode_counted(reader, evidence_wire::decode_evidence_contract_lane)?;
    let proof_output_calls = decode_counted(reader, proof_output_wire::decode_proof_output_call)?;
    let proof_recursive_components = decode_counted(reader, decode_proof_recursive_component)?;
    let closed_conformance_applications = decode_counted(
        reader,
        closed_conformance_wire::decode_closed_conformance_application,
    )?;
    let (suspension_call_plan_count, suspension_call_sites, suspension_call_plans) = (
        reader.u32()?,
        decode_counted(reader, decode_suspension_call_site)?,
        decode_counted(reader, decode_suspension_call_plan)?,
    );
    let scalar_block_invariants = decode_counted(
        reader,
        scalar_block_invariant_wire::decode_scalar_block_invariant,
    )?;
    let operation_crash_contracts = decode_counted(
        reader,
        operation_crash_contract_wire::decode_operation_crash_contract,
    )?;
    let machines = decode_counted(reader, super::machine_wire::decode_machine)?;
    let target_machine_families = decode_counted(reader, target_family_wire::decode_family)?;
    let target_layout_applications = decode_counted(reader, target_layout_wire::decode)?;
    let boundary_application_demands =
        decode_counted(reader, boundary_application_wire::decode_demand)?;
    let callback_applications = decode_counted(reader, callback_application_wire::decode)?;
    let boundary_application_candidates =
        decode_counted(reader, boundary_application_wire::decode_candidate)?;
    let task_activation_demands = decode_demands(reader)?;
    Ok(TerminalModule {
        scalar_applications,
        folded_constants,
        indexed_theorems,
        mathematical_carriers,
        quotients,
        scalar_qualifications,
        scalar_block_invariants,
        operation_crash_contracts,
        vocabulary_marker,
        identities,
        entry,
        structural_types,
        structural_referents,
        structural_domains,
        services,
        root_service_reach: TerminalRootServiceReach {
            concrete: concrete_root_service_reach,
            installation_dependencies: installation_reach_dependencies,
        },
        placed_view_inputs,
        placed_view_plans,
        placement_lifecycle_plans,
        reborrow_restored_call_uses,
        boundary_machines,
        external_providers,
        provider_candidates,
        dynamic_dispatch,
        float_meaning_projections,
        float_meaning_equalities,
        proposition_declarations,
        proposition_applications,
        evidence_terms,
        evidence_contract_lanes,
        proof_output_calls,
        proof_recursive_components,
        closed_conformance_applications,
        suspension_call_plan_count,
        suspension_call_sites,
        suspension_call_plans,
        machines,
        target_machine_families,
        target_layout_applications,
        boundary_application_demands,
        boundary_application_candidates,
        callback_applications,
        task_activation_demands,
    })
}
