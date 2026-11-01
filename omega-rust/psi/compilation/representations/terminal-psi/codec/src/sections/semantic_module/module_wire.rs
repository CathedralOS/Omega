//! Canonical terminal-module envelope wire format.
//!
//! This module owns the ordered top-level declaration tables and exact module
//! vocabulary envelope. Individual declaration, machine, scalar, proof, and
//! structural payloads remain in their dedicated sibling wire modules.
//!
//! `encode_raw` and `decode_module_body` are the ordered section list: each
//! row kind is encoded and decoded by a named pair in this folder
//! (`declaration_wire`, `access_view_wire`, `borrow_wire`,
//! `float_meaning_wire`, `evidence_wire`, `proof_output_wire`,
//! `recursive_component_wire`, `closed_conformance_wire`,
//! `carry_and_suspension_wire`, `scalar_block_invariant_wire`,
//! `operation_crash_contract_wire`) or in a sibling wire module.

mod borrow_wire;
mod boundary_application_wire;
mod callback_application_wire;
mod calling_signature_wire;
mod carry_and_suspension_wire;
pub(crate) mod quotient_wire;
mod recursive_component_wire;
mod unoptimized_behavior_wire;

pub(crate) mod access_lifecycle_wire;
pub(crate) mod access_view_plan_wire;
pub(crate) mod access_view_wire;
mod closed_conformance_wire;
pub(crate) mod declaration_wire;
mod dynamic_dispatch_wire;
mod evidence_wire;
mod float_meaning_wire;
mod folded_constant_wire;
mod operation_crash_contract_wire;
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
    TerminalIdentityIndex, TerminalIdentityTable, TerminalIndexedTheorem,
    TerminalMathematicalCarrier, TerminalModule, TerminalRootServiceReach, VocabularyMarker,
};

use super::wire::{Reader, Writer, decode_bytes, strictly_increasing};
use super::wire_fields::{
    Field, Ids, List, MachineDeclaration, Optional, Referent, StructuralType, U32,
    delegated_fields, wire_record,
};
use super::{CodecError, FORMAT_MARKER, MAGIC};

delegated_fields! {
    ScalarQualifications: terminal_psi::ScalarQualificationCatalog
        => super::scalar_qualification_wire::encode, super::scalar_qualification_wire::decode;
    ScalarApplications: terminal_psi::ScalarApplications
        => scalar_application_wire::encode, scalar_application_wire::decode;
    AccessViewPlan: terminal_psi::TerminalAccessViewPlan
        => access_view_plan_wire::encode, access_view_plan_wire::decode;
    AccessLifecycle: terminal_psi::TerminalAccessLifecyclePlan
        => access_lifecycle_wire::encode, access_lifecycle_wire::decode;
    ExternalProvider: terminal_psi::ExternalProviderConformance
        => external_provider_wire::encode, external_provider_wire::decode;
    ProviderCandidate: terminal_psi::ProviderCandidateConformance
        => super::provider_candidate_wire::encode_provider_candidate,
           super::provider_candidate_wire::decode_provider_candidate;
    SuspensionPlan: terminal_psi::TerminalSuspensionCallPlan
        => carry_and_suspension_wire::encode_suspension_call_plan,
           carry_and_suspension_wire::decode_suspension_call_plan;
    Machine: terminal_psi::TerminalMachine
        => super::machine_wire::encode_machine, super::machine_wire::decode_machine;
    TargetFamily: terminal_psi::TargetMachineFamily
        => target_family_wire::encode_family, target_family_wire::decode_family;
    BoundaryDemand: terminal_psi::TerminalBoundaryApplicationDemand
        => boundary_application_wire::encode_demand, boundary_application_wire::decode_demand;
    BoundaryCandidate: terminal_psi::BoundaryApplicationCandidate
        => boundary_application_wire::encode_candidate, boundary_application_wire::decode_candidate;
}

// The ordered section list. Sections follow the vocabulary marker in this
// exact order; each names the row spelling its owner module states.
wire_record! {
    Module: TerminalModule {
        vocabulary_marker: Vocabulary,
        entry: Optional<target_family_wire::Callable>,
        identities: Identities,
        indexed_theorems: List<IndexedTheorem>,
        mathematical_carriers: List<MathematicalCarrier>,
        scalar_qualifications: ScalarQualifications,
        quotients: quotient_wire::Catalog,
        folded_constants: folded_constant_wire::Closure,
        scalar_applications: ScalarApplications,
        structural_types: List<StructuralType>,
        structural_referents: List<Referent>,
        structural_domains: List<declaration_wire::Domain>,
        services: List<declaration_wire::Service>,
        root_service_reach: RootServiceReach,
        access_view_plans: List<AccessViewPlan>,
        access_lifecycle_plans: List<AccessLifecycle>,
        access_view_inputs: List<access_view_wire::Input>,
        reborrow_restored_call_uses: List<borrow_wire::RestoredCallUse>,
        boundary_machines: List<super::structural_signature_wire::BoundaryMachine>,
        calling_signatures: List<calling_signature_wire::Declaration>,
        unoptimized_behavior: List<unoptimized_behavior_wire::Behavior>,
        external_providers: List<ExternalProvider>,
        provider_candidates: List<ProviderCandidate>,
        dynamic_dispatch: dynamic_dispatch_wire::Catalog,
        float_meaning_projections: List<float_meaning_wire::Projection>,
        float_meaning_equalities: List<float_meaning_wire::Equality>,
        proposition_declarations: List<super::proof_declaration_wire::Declaration>,
        proposition_applications: List<super::proof_declaration_wire::Application>,
        evidence_terms: List<evidence_wire::Term>,
        evidence_contract_lanes: List<evidence_wire::Lane>,
        proof_output_calls: List<proof_output_wire::Call>,
        proof_recursive_components: List<recursive_component_wire::Component>,
        closed_conformance_applications: List<closed_conformance_wire::Application>,
        suspension_call_plan_count: U32,
        suspension_call_sites: List<carry_and_suspension_wire::CallSite>,
        suspension_call_plans: List<SuspensionPlan>,
        scalar_block_invariants: List<scalar_block_invariant_wire::Invariant>,
        operation_crash_contracts: List<operation_crash_contract_wire::Contract>,
        machines: List<Machine>,
        target_machine_families: List<TargetFamily>,
        target_layout_applications: List<target_layout_wire::Application>,
        boundary_application_demands: List<BoundaryDemand>,
        callback_applications: List<callback_application_wire::Application>,
        boundary_application_candidates: List<BoundaryCandidate>,
        task_activation_demands: task_activation_wire::Demands,
    }
    RootServiceReach: TerminalRootServiceReach {
        concrete: Ids,
        installation_dependencies: List<declaration_wire::ReachDependency>,
    }
}

/// Only the current vocabulary decodes.
struct Vocabulary;
impl Field<VocabularyMarker> for Vocabulary {
    fn write(writer: &mut Writer, value: &VocabularyMarker) -> Result<(), CodecError> {
        writer.u16(value.get());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<VocabularyMarker, CodecError> {
        let raw = reader.u16()?;
        if raw != VocabularyMarker::CURRENT.get() {
            return Err(CodecError::UnsupportedVocabularyMarker(raw));
        }
        Ok(VocabularyMarker::CURRENT)
    }
}

/// The identity table begins with the empty identity; every later entry is
/// non-empty and unique.
struct Identities;
impl Field<TerminalIdentityTable> for Identities {
    fn write(writer: &mut Writer, value: &TerminalIdentityTable) -> Result<(), CodecError> {
        writer.strings("identity", value.entries())
    }
    fn read(reader: &mut Reader<'_>) -> Result<TerminalIdentityTable, CodecError> {
        let entries = super::wire::decode_counted(reader, |reader| reader.string("identity"))?;
        if entries.first().map(String::as_str) != Some("") {
            return Err(CodecError::MalformedStructuralFoundation(
                "identity table must begin with the empty identity",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for identity in &entries[1..] {
            if identity.is_empty() || !seen.insert(identity.as_str()) {
                return Err(CodecError::MalformedStructuralFoundation(
                    "identity table entries must be non-empty and unique",
                ));
            }
        }
        Ok(TerminalIdentityTable::from(entries))
    }
}

/// An indexed theorem root decodes only with a well-formed certificate and a
/// strictly increasing assumption closure.
struct IndexedTheorem;
impl Field<TerminalIndexedTheorem> for IndexedTheorem {
    fn write(writer: &mut Writer, value: &TerminalIndexedTheorem) -> Result<(), CodecError> {
        MachineDeclaration::write(writer, &value.source_contract)?;
        writer.u32(value.ensure_ordinal);
        writer.u32(value.evidence_source_position);
        writer.u32(value.type_identity.0);
        writer.len("indexed theorem certificate", value.certificate.len())?;
        writer.bytes(&value.certificate);
        List::<U32>::write(writer, &value.assumption_closure)
    }
    fn read(reader: &mut Reader<'_>) -> Result<TerminalIndexedTheorem, CodecError> {
        let source_contract = MachineDeclaration::read(reader)?;
        let ensure_ordinal = reader.u32()?;
        let evidence_source_position = reader.u32()?;
        let type_identity = TerminalIdentityIndex(reader.u32()?);
        let certificate = decode_bytes(reader)?;
        super::mathematical_certificate_wire::decode_mathematical_certificate(&certificate)?;
        let assumption_closure = List::<U32>::read(reader)?;
        if !strictly_increasing(assumption_closure.iter().copied()) || type_identity.is_absent() {
            return Err(CodecError::MalformedMathematicalCertificate(
                "invalid indexed theorem root",
            ));
        }
        Ok(TerminalIndexedTheorem {
            source_contract,
            ensure_ordinal,
            evidence_source_position,
            type_identity,
            certificate,
            assumption_closure,
        })
    }
}

/// A carrier key is a closed reflexive certificate over no universe parameters.
struct MathematicalCarrier;
impl Field<TerminalMathematicalCarrier> for MathematicalCarrier {
    fn write(writer: &mut Writer, value: &TerminalMathematicalCarrier) -> Result<(), CodecError> {
        writer.u32(value.type_identity.0);
        writer.len("mathematical carrier key", value.carrier_key.len())?;
        writer.bytes(&value.carrier_key);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<TerminalMathematicalCarrier, CodecError> {
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
        Ok(TerminalMathematicalCarrier {
            type_identity,
            carrier_key,
        })
    }
}

pub(crate) fn encode_raw(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    Module::write(&mut writer, module)?;
    Ok(writer.finish())
}

pub(crate) fn decode_module_body(reader: &mut Reader<'_>) -> Result<TerminalModule, CodecError> {
    Module::read(reader)
}
