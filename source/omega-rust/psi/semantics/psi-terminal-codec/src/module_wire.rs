//! Canonical terminal-module envelope wire format.
//!
//! This module owns the ordered top-level declaration tables and exact module
//! vocabulary envelope. Individual declaration, machine, scalar, proof, and
//! structural payloads remain in their dedicated sibling wire modules.

use psi_core::{ContentProjectionIdentity, IeeeFloatFormat};
use psi_terminal::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRow, DirectBlockFloatParameter,
    DirectMachineFloatParameter, DirectMachineFloatResult, DirectOperationFloatResult,
    EvidenceContractLane, EvidenceContractLaneKind, EvidenceTermDeclaration,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionInput, FloatProjectionInputId, InstallationReachDependency,
    ProofOnlyValueType, ProofOutput, ProofOutputCall, ProofOutputEvidenceArgument,
    ProofOutputRuntimeCall, ProofOutputRuntimeResult, ProofPropositionId, ProofValueDeclaration,
    ProofValueId, ServiceDeclaration, StaticRequirementDispatch, StructuralAccess,
    StructuralContentProjection, StructuralDomainDeclaration, TerminalBorrowBoundarySource,
    TerminalBorrowOwnerSegment, TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalModule,
    TerminalPlacedViewInput, TerminalProofRankingRelation, TerminalProofRecursiveCallSite,
    TerminalProofRecursiveComponent, TerminalProofRecursiveEdge, TerminalProofRecursiveField,
    TerminalProofRecursiveMember, TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType,
    TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse, TerminalReborrowRootHandoff,
    TerminalReborrowRootHandoffStep, TerminalReborrowSharedCohortMember, TerminalRootServiceReach,
    VocabularyMarker,
};

use super::content_wire::{decode_content_algebra, encode_content_algebra};
use super::dynamic_dispatch_wire::{
    decode_direct_dynamic_dispatches, decode_dynamic_conformance_selections,
    decode_dynamic_descriptor_arguments, decode_dynamic_descriptor_parameters,
    decode_indirect_dynamic_dispatches, decode_parameter_dynamic_dispatches,
    decode_rebound_dynamic_descriptors, encode_direct_dynamic_dispatches,
    encode_dynamic_conformance_selections, encode_dynamic_descriptor_arguments,
    encode_dynamic_descriptor_parameters, encode_indirect_dynamic_dispatches,
    encode_parameter_dynamic_dispatches, encode_rebound_dynamic_descriptors,
};
use super::proof_declaration_wire::{
    decode_evidence_interface, decode_proposition_application, decode_proposition_declaration,
    encode_evidence_interface, encode_proposition_application, encode_proposition_declaration,
};
use super::provider_candidate_wire::{decode_provider_candidate, encode_provider_candidate};
use super::quotient_correspondence_wire::{
    decode_quotient_correspondence, encode_quotient_correspondence,
};
use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::structural_result_wire::ResultPathWireFormat;
use super::structural_signature_wire::{
    decode_boundary_machine, decode_content_projection_expression, encode_boundary_machine,
    encode_content_projection_expression,
};
use super::structural_type_wire::{decode_structural_type, encode_structural_type};
use super::wire::{Reader, Writer};
use super::{
    CodecError, FORMAT_MARKER, LEGACY_RESULT_PATH_FORMAT_MARKER,
    LEGACY_RESULT_PATH_VOCABULARY_MARKER, MAGIC, decode_counted, decode_ids,
};

fn encode_borrow_boundary(
    writer: &mut Writer,
    source: &TerminalBorrowBoundarySource,
) -> Result<(), CodecError> {
    match source {
        TerminalBorrowBoundarySource::Statement { statement_index } => {
            writer.u8(1);
            writer.u64(*statement_index);
        }
        TerminalBorrowBoundarySource::Call {
            statement_index,
            call_ordinal,
            target_identity,
        } => {
            writer.u8(2);
            writer.u64(*statement_index);
            writer.u64(*call_ordinal);
            writer.string("reborrow call target identity", target_identity)?;
        }
    }
    Ok(())
}

fn decode_borrow_boundary(
    reader: &mut Reader<'_>,
) -> Result<TerminalBorrowBoundarySource, CodecError> {
    match reader.u8()? {
        1 => Ok(TerminalBorrowBoundarySource::Statement {
            statement_index: reader.u64()?,
        }),
        2 => Ok(TerminalBorrowBoundarySource::Call {
            statement_index: reader.u64()?,
            call_ordinal: reader.u64()?,
            target_identity: reader.string("reborrow call target identity")?,
        }),
        tag => Err(CodecError::InvalidTag("TerminalBorrowBoundarySource", tag)),
    }
}

fn encode_proof_recursive_component(
    writer: &mut Writer,
    component: &TerminalProofRecursiveComponent,
) -> Result<(), CodecError> {
    writer.u8(match component.ranking_relation {
        TerminalProofRankingRelation::StructuralSubterm => 1,
    });
    writer.string(
        "proof recursive rank type identity",
        &component.rank_type_identity,
    )?;
    writer.len("proof recursive types", component.types.len())?;
    for proof_type in &component.types {
        writer.string("proof recursive type identity", &proof_type.identity)?;
        writer.len("proof recursive type fields", proof_type.fields.len())?;
        for field in &proof_type.fields {
            writer.string("proof recursive field identity", &field.identity)?;
            writer.string("proof recursive field type identity", &field.type_identity)?;
        }
    }
    writer.len("proof recursive members", component.members.len())?;
    for member in &component.members {
        writer.id(member.contract);
        writer.string("proof recursive machine identity", &member.machine_identity)?;
        writer.string(
            "proof recursive rank parameter identity",
            &member.rank_parameter_identity,
        )?;
    }
    writer.len("proof recursive edges", component.edges.len())?;
    for edge in &component.edges {
        writer.id(edge.caller);
        writer.id(edge.callee);
        match &edge.site {
            TerminalProofRecursiveCallSite::Statement {
                state_identity,
                statement_index,
            } => {
                writer.u8(1);
                writer.string("proof recursive state identity", state_identity)?;
                writer.u64(*statement_index);
            }
            TerminalProofRecursiveCallSite::Expression {
                state_identity,
                statement_index,
                expression_ordinal,
            } => {
                writer.u8(2);
                writer.string("proof recursive state identity", state_identity)?;
                writer.u64(*statement_index);
                writer.u64(*expression_ordinal);
            }
            TerminalProofRecursiveCallSite::Transition {
                state_identity,
                statement_index,
                lane,
            } => {
                writer.u8(3);
                writer.string("proof recursive state identity", state_identity)?;
                writer.u64(*statement_index);
                writer.u8(match lane {
                    TerminalProofRecursiveTransitionLane::Target => 1,
                    TerminalProofRecursiveTransitionLane::Continuation => 2,
                });
            }
        }
        writer.strings(
            "proof recursive strict member path",
            &edge.strict_member_path,
        )?;
    }
    Ok(())
}

fn decode_proof_recursive_component(
    reader: &mut Reader<'_>,
) -> Result<TerminalProofRecursiveComponent, CodecError> {
    let ranking_relation = match reader.u8()? {
        1 => TerminalProofRankingRelation::StructuralSubterm,
        tag => return Err(CodecError::InvalidTag("TerminalProofRankingRelation", tag)),
    };
    let rank_type_identity = reader.string("proof recursive rank type identity")?;
    let types = decode_counted(reader, |reader| {
        Ok(TerminalProofRecursiveType {
            identity: reader.string("proof recursive type identity")?,
            fields: decode_counted(reader, |reader| {
                Ok(TerminalProofRecursiveField {
                    identity: reader.string("proof recursive field identity")?,
                    type_identity: reader.string("proof recursive field type identity")?,
                })
            })?,
        })
    })?;
    let members = decode_counted(reader, |reader| {
        Ok(TerminalProofRecursiveMember {
            contract: reader.id("ContractId")?,
            machine_identity: reader.string("proof recursive machine identity")?,
            rank_parameter_identity: reader.string("proof recursive rank parameter identity")?,
        })
    })?;
    let edges = decode_counted(reader, |reader| {
        let caller = reader.id("ContractId")?;
        let callee = reader.id("ContractId")?;
        let site = match reader.u8()? {
            1 => TerminalProofRecursiveCallSite::Statement {
                state_identity: reader.string("proof recursive state identity")?,
                statement_index: reader.u64()?,
            },
            2 => TerminalProofRecursiveCallSite::Expression {
                state_identity: reader.string("proof recursive state identity")?,
                statement_index: reader.u64()?,
                expression_ordinal: reader.u64()?,
            },
            3 => TerminalProofRecursiveCallSite::Transition {
                state_identity: reader.string("proof recursive state identity")?,
                statement_index: reader.u64()?,
                lane: match reader.u8()? {
                    1 => TerminalProofRecursiveTransitionLane::Target,
                    2 => TerminalProofRecursiveTransitionLane::Continuation,
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "TerminalProofRecursiveTransitionLane",
                            tag,
                        ));
                    }
                },
            },
            tag => {
                return Err(CodecError::InvalidTag(
                    "TerminalProofRecursiveCallSite",
                    tag,
                ));
            }
        };
        Ok(TerminalProofRecursiveEdge {
            caller,
            callee,
            site,
            strict_member_path: reader.strings("proof recursive strict member path")?,
        })
    })?;
    Ok(TerminalProofRecursiveComponent {
        ranking_relation,
        rank_type_identity,
        types,
        members,
        edges,
    })
}

fn encode_owner_path(
    writer: &mut Writer,
    path: &[TerminalBorrowOwnerSegment],
) -> Result<(), CodecError> {
    writer.len("reborrow owner path", path.len())?;
    for segment in path {
        match segment {
            TerminalBorrowOwnerSegment::Field(identity) => {
                writer.u8(1);
                writer.string("reborrow owner field", identity)?;
            }
            TerminalBorrowOwnerSegment::Case(identity) => {
                writer.u8(2);
                writer.string("reborrow owner case", identity)?;
            }
            TerminalBorrowOwnerSegment::FixedIndex(index) => {
                writer.u8(3);
                writer.u64(*index);
            }
            TerminalBorrowOwnerSegment::DynamicIndex => writer.u8(4),
        }
    }
    Ok(())
}

fn decode_owner_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalBorrowOwnerSegment>, CodecError> {
    decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(TerminalBorrowOwnerSegment::Field(
            reader.string("reborrow owner field")?,
        )),
        2 => Ok(TerminalBorrowOwnerSegment::Case(
            reader.string("reborrow owner case")?,
        )),
        3 => Ok(TerminalBorrowOwnerSegment::FixedIndex(reader.u64()?)),
        4 => Ok(TerminalBorrowOwnerSegment::DynamicIndex),
        tag => Err(CodecError::InvalidTag("TerminalBorrowOwnerSegment", tag)),
    })
}

fn encode_place_segments(
    writer: &mut Writer,
    segments: &[TerminalBorrowPlaceSegment],
) -> Result<(), CodecError> {
    writer.len("reborrow place segments", segments.len())?;
    for segment in segments {
        match segment {
            TerminalBorrowPlaceSegment::Field(identity) => {
                writer.u8(1);
                writer.string("reborrow place field", identity)?;
            }
            TerminalBorrowPlaceSegment::Case(identity) => {
                writer.u8(2);
                writer.string("reborrow place case", identity)?;
            }
            TerminalBorrowPlaceSegment::FixedIndex(index) => {
                writer.u8(3);
                writer.u64(*index);
            }
            TerminalBorrowPlaceSegment::FixedRange { start, end } => {
                writer.u8(4);
                writer.u64(*start);
                writer.u64(*end);
            }
        }
    }
    Ok(())
}

fn decode_place_segments(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalBorrowPlaceSegment>, CodecError> {
    decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(TerminalBorrowPlaceSegment::Field(
            reader.string("reborrow place field")?,
        )),
        2 => Ok(TerminalBorrowPlaceSegment::Case(
            reader.string("reborrow place case")?,
        )),
        3 => Ok(TerminalBorrowPlaceSegment::FixedIndex(reader.u64()?)),
        4 => Ok(TerminalBorrowPlaceSegment::FixedRange {
            start: reader.u64()?,
            end: reader.u64()?,
        }),
        tag => Err(CodecError::InvalidTag("TerminalBorrowPlaceSegment", tag)),
    })
}

fn encode_place(writer: &mut Writer, place: &TerminalBorrowPlace) -> Result<(), CodecError> {
    writer.string("reborrow place root", &place.root_identity)?;
    encode_place_segments(writer, &place.segments)
}

fn decode_place(reader: &mut Reader<'_>) -> Result<TerminalBorrowPlace, CodecError> {
    Ok(TerminalBorrowPlace {
        root_identity: reader.string("reborrow place root")?,
        segments: decode_place_segments(reader)?,
    })
}

fn encode_borrow_access(writer: &mut Writer, access: StructuralAccess) {
    writer.u8(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

fn decode_borrow_access(reader: &mut Reader<'_>) -> Result<StructuralAccess, CodecError> {
    match reader.u8()? {
        1 => Ok(StructuralAccess::Owned),
        2 => Ok(StructuralAccess::SharedBorrow),
        3 => Ok(StructuralAccess::MutableBorrow),
        4 => Ok(StructuralAccess::WriteOnlyBorrow),
        tag => Err(CodecError::InvalidTag("StructuralAccess", tag)),
    }
}

fn encode_reborrow_root_handoff(
    writer: &mut Writer,
    handoff: &TerminalReborrowRootHandoff,
) -> Result<(), CodecError> {
    writer.id(handoff.machine);
    writer.string(
        "reborrow machine identity",
        &handoff.source_machine_identity,
    )?;
    writer.string("reborrow state identity", &handoff.source_state_identity)?;
    writer.string("reborrow direct owner", &handoff.direct_root_owner_identity)?;
    encode_owner_path(writer, &handoff.direct_root_owner_path)?;
    encode_place(writer, &handoff.direct_root_place)?;
    encode_borrow_access(writer, handoff.direct_root_access);
    encode_borrow_boundary(writer, &handoff.direct_root_activation)?;
    encode_borrow_boundary(writer, &handoff.direct_root_weakening)?;
    writer.string(
        "reborrow direct-root lifetime",
        &handoff.direct_root_lifetime_identity,
    )?;
    writer.len("reborrow root-handoff lineage", handoff.lineage.len())?;
    for step in &handoff.lineage {
        writer.string("reborrow child owner", &step.child_owner_identity)?;
        encode_owner_path(writer, &step.child_owner_path)?;
        encode_place(writer, &step.child_place)?;
        encode_place_segments(writer, &step.projection_remainder)?;
        encode_borrow_access(writer, step.child_access);
        encode_borrow_boundary(writer, &step.child_activation)?;
        encode_borrow_boundary(writer, &step.formation_boundary)?;
        encode_borrow_boundary(writer, &step.child_weakening)?;
    }
    Ok(())
}

fn decode_reborrow_root_handoff(
    reader: &mut Reader<'_>,
) -> Result<TerminalReborrowRootHandoff, CodecError> {
    Ok(TerminalReborrowRootHandoff {
        machine: reader.id("MachineId")?,
        source_machine_identity: reader.string("reborrow machine identity")?,
        source_state_identity: reader.string("reborrow state identity")?,
        direct_root_owner_identity: reader.string("reborrow direct owner")?,
        direct_root_owner_path: decode_owner_path(reader)?,
        direct_root_place: decode_place(reader)?,
        direct_root_access: decode_borrow_access(reader)?,
        direct_root_activation: decode_borrow_boundary(reader)?,
        direct_root_weakening: decode_borrow_boundary(reader)?,
        direct_root_lifetime_identity: reader.string("reborrow direct-root lifetime")?,
        lineage: decode_counted(reader, |reader| {
            Ok(TerminalReborrowRootHandoffStep {
                child_owner_identity: reader.string("reborrow child owner")?,
                child_owner_path: decode_owner_path(reader)?,
                child_place: decode_place(reader)?,
                projection_remainder: decode_place_segments(reader)?,
                child_access: decode_borrow_access(reader)?,
                child_activation: decode_borrow_boundary(reader)?,
                formation_boundary: decode_borrow_boundary(reader)?,
                child_weakening: decode_borrow_boundary(reader)?,
            })
        })?,
    })
}

fn encode_reborrow_restored_call_use(
    writer: &mut Writer,
    use_row: &TerminalReborrowRestoredCallUse,
) -> Result<(), CodecError> {
    writer.id(use_row.machine);
    writer.id(use_row.operation);
    writer.u8(match use_row.restoration_class {
        TerminalReborrowRestorationClass::ExclusiveReactivation => 1,
        TerminalReborrowRestorationClass::SharedFreezeRestoration => 2,
    });
    encode_borrow_boundary(writer, &use_row.call_boundary)?;
    writer.id(use_row.call_target_machine);
    writer.string(
        "restored-use machine identity",
        &use_row.source_machine_identity,
    )?;
    writer.string(
        "restored-use state identity",
        &use_row.source_state_identity,
    )?;
    writer.string(
        "restored-use direct owner",
        &use_row.direct_root_owner_identity,
    )?;
    encode_owner_path(writer, &use_row.direct_root_owner_path)?;
    encode_place(writer, &use_row.direct_root_place)?;
    encode_borrow_boundary(writer, &use_row.direct_root_activation)?;
    encode_borrow_boundary(writer, &use_row.direct_root_weakening)?;
    writer.string(
        "restored-use direct-root lifetime",
        &use_row.direct_root_lifetime_identity,
    )?;
    writer.string("restored-use child owner", &use_row.child_owner_identity)?;
    encode_owner_path(writer, &use_row.child_owner_path)?;
    encode_place(writer, &use_row.child_place)?;
    encode_place_segments(writer, &use_row.projection_remainder)?;
    encode_borrow_access(writer, use_row.child_access);
    encode_borrow_boundary(writer, &use_row.child_activation)?;
    encode_borrow_boundary(writer, &use_row.formation_boundary)?;
    encode_borrow_boundary(writer, &use_row.child_weakening)?;
    writer.len("restored-use shared cohort", use_row.shared_cohort.len())?;
    for member in &use_row.shared_cohort {
        writer.string(
            "restored-use cohort child owner",
            &member.child_owner_identity,
        )?;
        encode_owner_path(writer, &member.child_owner_path)?;
        encode_place(writer, &member.child_place)?;
        encode_borrow_access(writer, member.child_access);
        encode_borrow_boundary(writer, &member.child_activation)?;
        encode_borrow_boundary(writer, &member.child_weakening)?;
    }
    Ok(())
}

fn decode_reborrow_restored_call_use(
    reader: &mut Reader<'_>,
) -> Result<TerminalReborrowRestoredCallUse, CodecError> {
    Ok(TerminalReborrowRestoredCallUse {
        machine: reader.id("MachineId")?,
        operation: reader.id("OperationId")?,
        restoration_class: match reader.u8()? {
            1 => TerminalReborrowRestorationClass::ExclusiveReactivation,
            2 => TerminalReborrowRestorationClass::SharedFreezeRestoration,
            tag => {
                return Err(CodecError::InvalidTag(
                    "TerminalReborrowRestorationClass",
                    tag,
                ));
            }
        },
        call_boundary: decode_borrow_boundary(reader)?,
        call_target_machine: reader.id("MachineId")?,
        source_machine_identity: reader.string("restored-use machine identity")?,
        source_state_identity: reader.string("restored-use state identity")?,
        direct_root_owner_identity: reader.string("restored-use direct owner")?,
        direct_root_owner_path: decode_owner_path(reader)?,
        direct_root_place: decode_place(reader)?,
        direct_root_activation: decode_borrow_boundary(reader)?,
        direct_root_weakening: decode_borrow_boundary(reader)?,
        direct_root_lifetime_identity: reader.string("restored-use direct-root lifetime")?,
        child_owner_identity: reader.string("restored-use child owner")?,
        child_owner_path: decode_owner_path(reader)?,
        child_place: decode_place(reader)?,
        projection_remainder: decode_place_segments(reader)?,
        child_access: decode_borrow_access(reader)?,
        child_activation: decode_borrow_boundary(reader)?,
        formation_boundary: decode_borrow_boundary(reader)?,
        child_weakening: decode_borrow_boundary(reader)?,
        shared_cohort: decode_counted(reader, |reader| {
            Ok(TerminalReborrowSharedCohortMember {
                child_owner_identity: reader.string("restored-use cohort child owner")?,
                child_owner_path: decode_owner_path(reader)?,
                child_place: decode_place(reader)?,
                child_access: decode_borrow_access(reader)?,
                child_activation: decode_borrow_boundary(reader)?,
                child_weakening: decode_borrow_boundary(reader)?,
            })
        })?,
    })
}

pub(super) fn encode_raw(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    encode_raw_for_result_paths(module, ResultPathWireFormat::Current)
}

pub(super) fn encode_legacy_result_path_raw(
    module: &TerminalModule,
) -> Result<Vec<u8>, CodecError> {
    encode_raw_for_result_paths(module, ResultPathWireFormat::LegacyWithoutResultPaths)
}

fn encode_raw_for_result_paths(
    module: &TerminalModule,
    result_path_format: ResultPathWireFormat,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(match result_path_format {
        ResultPathWireFormat::LegacyWithoutResultPaths => LEGACY_RESULT_PATH_FORMAT_MARKER,
        ResultPathWireFormat::Current => FORMAT_MARKER,
    });
    writer.u16(match result_path_format {
        ResultPathWireFormat::LegacyWithoutResultPaths => LEGACY_RESULT_PATH_VOCABULARY_MARKER,
        ResultPathWireFormat::Current => module.vocabulary_marker.get(),
    });
    writer.id(module.entry);
    writer.len("structural types", module.structural_types.len())?;
    for declaration in &module.structural_types {
        encode_structural_type(&mut writer, declaration)?;
    }
    writer.len("structural domains", module.structural_domains.len())?;
    for declaration in &module.structural_domains {
        writer.id(declaration.id);
        writer.id(declaration.semantic_domain);
        writer.string("structural domain identity", &declaration.identity)?;
        writer.id(declaration.carrier);
        writer.boolean(declaration.content_projection.is_some());
        if let Some(projection) = &declaration.content_projection {
            writer.id(projection.identity.domain);
            writer.u64(projection.identity.projection_report_fingerprint);
            encode_content_algebra(&mut writer, &projection.algebra)?;
            encode_content_projection_expression(&mut writer, &projection.expression)?;
        }
    }
    writer.len("services", module.services.len())?;
    for declaration in &module.services {
        writer.id(declaration.id);
        writer.string("service identity", &declaration.identity)?;
        writer.len("service parents", declaration.parents.len())?;
        for parent in &declaration.parents {
            writer.id(*parent);
        }
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
        writer.string(
            "installation reach requirement identity",
            &dependency.requirement_identity,
        )?;
        writer.len(
            "installation reach upper bound",
            dependency.upper_bound.len(),
        )?;
        for service in &dependency.upper_bound {
            writer.id(*service);
        }
    }
    writer.len("placed-view inputs", module.placed_view_inputs.len())?;
    for input in &module.placed_view_inputs {
        writer.id(input.machine);
        writer.u32(input.position);
        writer.string(
            "placed-view source machine identity",
            &input.source_machine_identity,
        )?;
        writer.string(
            "placed-view source state identity",
            &input.source_state_identity,
        )?;
        writer.string(
            "placed-view source parameter identity",
            &input.source_parameter_identity,
        )?;
        writer.u8(match input.access {
            StructuralAccess::Owned => 1,
            StructuralAccess::SharedBorrow => 2,
            StructuralAccess::MutableBorrow => 3,
            StructuralAccess::WriteOnlyBorrow => 4,
        });
        writer.boolean(input.binding_is_const);
        writer.boolean(input.binding_is_mutable);
        writer.string("placed-view identity", &input.view_identity)?;
        writer.string("placed-view policy identity", &input.policy_identity)?;
        writer.string(
            "placed-view policy-plan machine identity",
            &input.policy_plan_machine_identity,
        )?;
        writer.string("placed-view schema identity", &input.schema_identity)?;
        writer.u64(input.placement_report_fingerprint);
        writer.bytes(&input.placement_commitment);
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
        if result_path_format == ResultPathWireFormat::LegacyWithoutResultPaths {
            let current_only_tag = match projection.source {
                FloatMeaningSource::DirectOperationResult(_) => Some(6),
                FloatMeaningSource::DirectBlockParameter(_) => Some(7),
                _ => None,
            };
            if let Some(tag) = current_only_tag {
                return Err(CodecError::InvalidTag("legacy FloatMeaningSource", tag));
            }
        }
        writer.u32(projection.result.id.0);
        writer.u8(match projection.result.value_type {
            ProofOnlyValueType::FloatMeaning => 1,
        });
        match projection.source {
            FloatMeaningSource::TransitionalInput(input) => {
                writer.u8(1);
                writer.u32(input.id.0);
                writer.u8(match input.format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
            FloatMeaningSource::DirectMachineParameter(parameter) => {
                writer.u8(4);
                writer.id(parameter.owner);
                writer.id(parameter.parameter);
                writer.u8(match parameter.format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
            FloatMeaningSource::DirectMachineResult(result) => {
                writer.u8(5);
                writer.id(result.owner);
                writer.id(result.result);
                writer.u8(match result.format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
            FloatMeaningSource::DirectBlockParameter(parameter) => {
                writer.u8(7);
                writer.id(parameter.owner);
                writer.id(parameter.block);
                writer.id(parameter.parameter);
                writer.u8(match parameter.format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
            FloatMeaningSource::DirectOperationResult(result) => {
                writer.u8(6);
                writer.id(result.owner);
                writer.id(result.producer);
                writer.id(result.result);
                writer.u8(match result.format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
            FloatMeaningSource::ExactBinary32Literal(bits) => {
                writer.u8(2);
                writer.u32(bits);
            }
            FloatMeaningSource::ExactBinary64Literal(bits) => {
                writer.u8(3);
                writer.u64(bits);
            }
        }
        writer.u8(match projection.operation {
            FloatMeaningProjectionOperation::Meaning32 => 1,
            FloatMeaningProjectionOperation::Meaning64 => 2,
        });
        writer.u16(projection.contract.format);
        writer.u8(projection.contract.operation);
        writer.u8(projection.contract.declaration);
        writer.u16(projection.contract.catalog_version);
        writer.bytes(&projection.contract.commitment);
    }
    writer.len(
        "float-meaning equalities",
        module.float_meaning_equalities.len(),
    )?;
    for proposition in &module.float_meaning_equalities {
        writer.u32(proposition.id.0);
        writer.u32(proposition.left.0);
        writer.u32(proposition.right.0);
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
        writer.id(term.id);
        writer.id(term.proposition);
        encode_evidence_interface(&mut writer, &term.interface)?;
    }
    writer.len(
        "evidence contract lanes",
        module.evidence_contract_lanes.len(),
    )?;
    for lane in &module.evidence_contract_lanes {
        writer.id(lane.machine);
        writer.u8(match lane.kind {
            EvidenceContractLaneKind::Requires => 1,
            EvidenceContractLaneKind::Ensures => 2,
        });
        writer.u32(lane.position);
        writer.id(lane.term);
        writer.boolean(lane.output_field.is_some());
        if let Some(field) = &lane.output_field {
            writer.string("evidence output field", field)?;
        }
    }
    writer.len("proof-output invocations", module.proof_output_calls.len())?;
    for invocation in &module.proof_output_calls {
        writer.id(invocation.caller);
        writer.u32(invocation.ordinal);
        writer.string(
            "proof-output target machine identity",
            &invocation.target_machine_identity,
        )?;
        writer.boolean(invocation.static_requirement_dispatch.is_some());
        if let Some(dispatch) = &invocation.static_requirement_dispatch {
            writer.u64(dispatch.conformance_application_report_fingerprint);
            writer.bytes(&dispatch.conformance_application_commitment.as_bytes());
            writer.string(
                "static public requirement identity",
                &dispatch.public_requirement_identity,
            )?;
            writer.string(
                "static requirement declaring trait identity",
                &dispatch.declaring_trait_identity,
            )?;
            writer.string(
                "static requirement identity",
                &dispatch.requirement_identity,
            )?;
            writer.string(
                "static requirement realization identity",
                &dispatch.realization_identity,
            )?;
            writer.string(
                "static requirement realization callable identity",
                &dispatch.realization_callable_identity,
            )?;
            writer.id(dispatch.realization);
        }
        writer.boolean(invocation.runtime_result.is_some());
        if let Some(runtime_result) = invocation.runtime_result {
            writer.boolean(matches!(
                runtime_result,
                ProofOutputRuntimeResult::Scalar(_)
            ));
            if let ProofOutputRuntimeResult::Scalar(runtime_value) = runtime_result {
                encode_scalar_type(&mut writer, runtime_value);
            }
        }
        writer.boolean(invocation.runtime_call.is_some());
        if let Some(runtime_call) = invocation.runtime_call {
            writer.id(runtime_call.operation);
            writer.id(runtime_call.callee);
        }
        writer.len(
            "proof-output evidence arguments",
            invocation.evidence_arguments.len(),
        )?;
        for argument in &invocation.evidence_arguments {
            writer.u32(argument.input_position);
            writer.id(argument.callee_proposition);
            writer.id(argument.source);
            writer.id(argument.instantiated_proposition);
        }
        writer.len("proof outputs", invocation.outputs.len())?;
        for output in &invocation.outputs {
            writer.u32(output.output_position);
            writer.string("proof-output field", &output.output_field)?;
            writer.id(output.callee_proposition);
            writer.boolean(output.callee_output.is_some());
            if let Some(callee_output) = output.callee_output {
                writer.id(callee_output);
            }
            writer.id(output.instantiated_proposition);
            writer.boolean(output.forwarded_input_position.is_some());
            if let Some(position) = output.forwarded_input_position {
                writer.u32(position);
            }
            writer.boolean(output.output.is_some());
            if let Some(output) = output.output {
                writer.id(output);
            }
        }
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
        writer.id(application.owner);
        writer.string(
            "closed conformance declaration identity",
            &application.declaration_identity,
        )?;
        writer.len("closed conformance telescope", application.telescope.len())?;
        for binding in &application.telescope {
            writer.string("closed conformance parameter", &binding.parameter)?;
            writer.u8(match binding.kind {
                ClosedConformanceParameterKind::Lifetime => 1,
                ClosedConformanceParameterKind::Type => 2,
                ClosedConformanceParameterKind::Const => 3,
                ClosedConformanceParameterKind::Machine => 4,
            });
            writer.string("closed conformance argument", &binding.argument)?;
        }
        writer.boolean(application.subject_identity.is_some());
        if let Some(subject) = &application.subject_identity {
            writer.string("closed conformance subject identity", subject)?;
        }
        writer.string(
            "closed conformance trait identity",
            &application.trait_identity,
        )?;
        writer.strings(
            "closed conformance trait lifetime arguments",
            &application.trait_lifetime_arguments,
        )?;
        writer.strings(
            "closed conformance trait arguments",
            &application.trait_arguments,
        )?;
        writer.len(
            "closed conformance realization callables",
            application.realization_callables.len(),
        )?;
        for callable in &application.realization_callables {
            writer.string(
                "closed conformance realization callable identity",
                &callable.source_callable_identity,
            )?;
            writer.id(callable.machine);
            writer.u8(match callable.result {
                ClosedConformanceCallableResult::Unit => 1,
                ClosedConformanceCallableResult::I32 => 2,
                ClosedConformanceCallableResult::Bool => 3,
            });
        }
        writer.len("closed conformance rows", application.rows.len())?;
        for row in &application.rows {
            writer.string(
                "closed conformance row declaring trait identity",
                &row.declaring_trait_identity,
            )?;
            writer.string(
                "closed conformance row public requirement identity",
                &row.public_requirement_identity,
            )?;
            writer.string(
                "closed conformance row requirement identity",
                &row.requirement_identity,
            )?;
            writer.string(
                "closed conformance row realization identity",
                &row.realization_identity,
            )?;
            writer.boolean(row.realization_callable_identity.is_some());
            if let Some(identity) = &row.realization_callable_identity {
                writer.string(
                    "closed conformance row realization callable identity",
                    identity,
                )?;
            }
        }
        writer.u64(application.report_fingerprint);
        writer.bytes(&application.commitment.as_bytes());
    }
    if result_path_format == ResultPathWireFormat::Current {
        encode_dynamic_descriptor_parameters(&mut writer, &module.dynamic_dispatch.parameters)?;
        encode_dynamic_descriptor_arguments(&mut writer, &module.dynamic_dispatch.arguments)?;
        encode_dynamic_conformance_selections(&mut writer, &module.dynamic_dispatch.selections)?;
        encode_rebound_dynamic_descriptors(
            &mut writer,
            &module.dynamic_dispatch.rebound_descriptors,
        )?;
        encode_direct_dynamic_dispatches(&mut writer, &module.dynamic_dispatch.direct_dispatches)?;
        encode_indirect_dynamic_dispatches(
            &mut writer,
            &module.dynamic_dispatch.indirect_dispatches,
        )?;
        encode_parameter_dynamic_dispatches(
            &mut writer,
            &module.dynamic_dispatch.parameter_dispatches,
        )?;
    }
    writer.len(
        "quotient correspondences",
        module.quotient_correspondences.len(),
    )?;
    for correspondence in &module.quotient_correspondences {
        encode_quotient_correspondence(&mut writer, correspondence)?;
    }
    writer.len("machines", module.machines.len())?;
    for machine in &module.machines {
        super::machine_wire::encode_machine_for_result_paths(
            &mut writer,
            machine,
            result_path_format,
        )?;
    }
    Ok(writer.finish())
}

pub(super) fn decode_module_body(
    reader: &mut Reader<'_>,
    format_marker: u16,
) -> Result<TerminalModule, CodecError> {
    let vocabulary_marker_raw = reader.u16()?;
    let result_path_format = match (format_marker, vocabulary_marker_raw) {
        (FORMAT_MARKER, raw) if raw == VocabularyMarker::CURRENT.get() => {
            ResultPathWireFormat::Current
        }
        (LEGACY_RESULT_PATH_FORMAT_MARKER, LEGACY_RESULT_PATH_VOCABULARY_MARKER) => {
            ResultPathWireFormat::LegacyWithoutResultPaths
        }
        _ => {
            return Err(CodecError::UnsupportedVocabularyMarker(
                vocabulary_marker_raw,
            ));
        }
    };
    let vocabulary_marker = VocabularyMarker::CURRENT;
    let entry = reader.id("MachineId")?;
    let structural_types = decode_counted(reader, decode_structural_type)?;
    let structural_domains = decode_counted(reader, |reader| {
        Ok(StructuralDomainDeclaration {
            id: reader.id("StructuralDomainId")?,
            semantic_domain: reader.id("DomainSemanticId")?,
            identity: reader.string("structural domain identity")?,
            carrier: reader.id("StructuralTypeId")?,
            content_projection: if reader.boolean()? {
                Some(StructuralContentProjection {
                    identity: ContentProjectionIdentity {
                        domain: reader.id("ContentDomainId")?,
                        projection_report_fingerprint: reader.u64()?,
                    },
                    algebra: decode_content_algebra(reader)?,
                    expression: decode_content_projection_expression(reader, 0)?,
                })
            } else {
                None
            },
        })
    })?;
    let services = decode_counted(reader, |reader| {
        Ok(ServiceDeclaration {
            id: reader.id("ServiceId")?,
            identity: reader.string("service identity")?,
            parents: decode_ids(reader, "ServiceId")?,
        })
    })?;
    let concrete_root_service_reach = decode_ids(reader, "ServiceId")?;
    let installation_reach_dependencies = decode_counted(reader, |reader| {
        Ok(InstallationReachDependency {
            requirement_identity: reader.string("installation reach requirement identity")?,
            upper_bound: decode_ids(reader, "ServiceId")?,
        })
    })?;
    let placed_view_inputs = decode_counted(reader, |reader| {
        Ok(TerminalPlacedViewInput {
            machine: reader.id("MachineId")?,
            position: reader.u32()?,
            source_machine_identity: reader.string("placed-view source machine identity")?,
            source_state_identity: reader.string("placed-view source state identity")?,
            source_parameter_identity: reader.string("placed-view source parameter identity")?,
            access: match reader.u8()? {
                1 => StructuralAccess::Owned,
                2 => StructuralAccess::SharedBorrow,
                3 => StructuralAccess::MutableBorrow,
                4 => StructuralAccess::WriteOnlyBorrow,
                tag => return Err(CodecError::InvalidTag("StructuralAccess", tag)),
            },
            binding_is_const: reader.boolean()?,
            binding_is_mutable: reader.boolean()?,
            view_identity: reader.string("placed-view identity")?,
            policy_identity: reader.string("placed-view policy identity")?,
            policy_plan_machine_identity: reader
                .string("placed-view policy-plan machine identity")?,
            schema_identity: reader.string("placed-view schema identity")?,
            placement_report_fingerprint: reader.u64()?,
            placement_commitment: reader.array()?,
        })
    })?;
    let reborrow_root_handoffs = decode_counted(reader, decode_reborrow_root_handoff)?;
    let reborrow_restored_call_uses = decode_counted(reader, decode_reborrow_restored_call_use)?;
    let boundary_machines = decode_counted(reader, decode_boundary_machine)?;
    let provider_candidates = decode_counted(reader, decode_provider_candidate)?;
    let float_meaning_projections = decode_counted(reader, |reader| {
        Ok(FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(reader.u32()?),
                value_type: match reader.u8()? {
                    1 => ProofOnlyValueType::FloatMeaning,
                    tag => return Err(CodecError::InvalidTag("ProofOnlyValueType", tag)),
                },
            },
            source: match reader.u8()? {
                1 => FloatMeaningSource::TransitionalInput(FloatProjectionInput {
                    id: FloatProjectionInputId(reader.u32()?),
                    format: match reader.u8()? {
                        1 => IeeeFloatFormat::Binary32,
                        2 => IeeeFloatFormat::Binary64,
                        tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                    },
                }),
                2 => FloatMeaningSource::ExactBinary32Literal(reader.u32()?),
                3 => FloatMeaningSource::ExactBinary64Literal(reader.u64()?),
                4 => FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
                    owner: reader.id("float-meaning direct parameter owner")?,
                    parameter: reader.id("float-meaning direct parameter value")?,
                    format: match reader.u8()? {
                        1 => IeeeFloatFormat::Binary32,
                        2 => IeeeFloatFormat::Binary64,
                        tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                    },
                }),
                5 => FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
                    owner: reader.id("float-meaning direct result owner")?,
                    result: reader.id("float-meaning direct result value")?,
                    format: match reader.u8()? {
                        1 => IeeeFloatFormat::Binary32,
                        2 => IeeeFloatFormat::Binary64,
                        tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                    },
                }),
                6 if result_path_format == ResultPathWireFormat::Current => {
                    FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
                        owner: reader.id("float-meaning direct operation-result owner")?,
                        producer: reader.id("float-meaning direct operation-result producer")?,
                        result: reader.id("float-meaning direct operation-result value")?,
                        format: match reader.u8()? {
                            1 => IeeeFloatFormat::Binary32,
                            2 => IeeeFloatFormat::Binary64,
                            tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                        },
                    })
                }
                7 if result_path_format == ResultPathWireFormat::Current => {
                    FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
                        owner: reader.id("float-meaning direct block-parameter owner")?,
                        block: reader.id("float-meaning direct block-parameter block")?,
                        parameter: reader.id("float-meaning direct block-parameter value")?,
                        format: match reader.u8()? {
                            1 => IeeeFloatFormat::Binary32,
                            2 => IeeeFloatFormat::Binary64,
                            tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                        },
                    })
                }
                tag => return Err(CodecError::InvalidTag("FloatMeaningSource", tag)),
            },
            operation: match reader.u8()? {
                1 => FloatMeaningProjectionOperation::Meaning32,
                2 => FloatMeaningProjectionOperation::Meaning64,
                tag => {
                    return Err(CodecError::InvalidTag(
                        "FloatMeaningProjectionOperation",
                        tag,
                    ));
                }
            },
            contract: psi_terminal::FloatProjectionContractIdentity {
                format: reader.u16()?,
                operation: reader.u8()?,
                declaration: reader.u8()?,
                catalog_version: reader.u16()?,
                commitment: reader.array()?,
            },
        })
    })?;
    let float_meaning_equalities = decode_counted(reader, |reader| {
        Ok(FloatMeaningEqualityProposition {
            id: ProofPropositionId(reader.u32()?),
            left: ProofValueId(reader.u32()?),
            right: ProofValueId(reader.u32()?),
        })
    })?;
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
    let evidence_terms = decode_counted(reader, |reader| {
        Ok(EvidenceTermDeclaration {
            id: reader.id("EvidenceTermId")?,
            proposition: reader.id("PropositionId")?,
            interface: decode_evidence_interface(reader)?,
        })
    })?;
    let evidence_contract_lanes = decode_counted(reader, |reader| {
        let machine = reader.id("MachineId")?;
        let kind = match reader.u8()? {
            1 => EvidenceContractLaneKind::Requires,
            2 => EvidenceContractLaneKind::Ensures,
            tag => return Err(CodecError::InvalidTag("EvidenceContractLaneKind", tag)),
        };
        Ok(EvidenceContractLane {
            machine,
            kind,
            position: reader.u32()?,
            term: reader.id("EvidenceTermId")?,
            output_field: reader
                .boolean()?
                .then(|| reader.string("evidence output field"))
                .transpose()?,
        })
    })?;
    let proof_output_calls = decode_counted(reader, |reader| {
        Ok(ProofOutputCall {
            caller: reader.id("MachineId")?,
            ordinal: reader.u32()?,
            target_machine_identity: reader.string("proof-output target machine identity")?,
            static_requirement_dispatch: reader
                .boolean()?
                .then(|| {
                    Ok(StaticRequirementDispatch {
                        conformance_application_report_fingerprint: reader.u64()?,
                        conformance_application_commitment:
                            ClosedConformanceApplicationCommitment::from_digest(reader.array()?),
                        public_requirement_identity: reader
                            .string("static public requirement identity")?,
                        declaring_trait_identity: reader
                            .string("static requirement declaring trait identity")?,
                        requirement_identity: reader.string("static requirement identity")?,
                        realization_identity: reader
                            .string("static requirement realization identity")?,
                        realization_callable_identity: reader
                            .string("static requirement realization callable identity")?,
                        realization: reader.id("MachineId")?,
                    })
                })
                .transpose()?,
            runtime_result: reader
                .boolean()?
                .then(|| {
                    Ok(if reader.boolean()? {
                        ProofOutputRuntimeResult::Scalar(decode_scalar_type(reader)?)
                    } else {
                        ProofOutputRuntimeResult::Unit
                    })
                })
                .transpose()?,
            runtime_call: reader
                .boolean()?
                .then(|| {
                    Ok(ProofOutputRuntimeCall {
                        operation: reader.id("OperationId")?,
                        callee: reader.id("MachineId")?,
                    })
                })
                .transpose()?,
            evidence_arguments: decode_counted(reader, |reader| {
                Ok(ProofOutputEvidenceArgument {
                    input_position: reader.u32()?,
                    callee_proposition: reader.id("PropositionId")?,
                    source: reader.id("EvidenceTermId")?,
                    instantiated_proposition: reader.id("PropositionId")?,
                })
            })?,
            outputs: decode_counted(reader, |reader| {
                Ok(ProofOutput {
                    output_position: reader.u32()?,
                    output_field: reader.string("proof-output field")?,
                    callee_proposition: reader.id("PropositionId")?,
                    callee_output: reader
                        .boolean()?
                        .then(|| reader.id("EvidenceTermId"))
                        .transpose()?,
                    instantiated_proposition: reader.id("PropositionId")?,
                    forwarded_input_position: reader
                        .boolean()?
                        .then(|| reader.u32())
                        .transpose()?,
                    output: reader
                        .boolean()?
                        .then(|| reader.id("EvidenceTermId"))
                        .transpose()?,
                })
            })?,
        })
    })?;
    let proof_recursive_components = decode_counted(reader, decode_proof_recursive_component)?;
    let closed_conformance_applications = decode_counted(reader, |reader| {
        Ok(ClosedConformanceApplication {
            owner: reader.id("MachineId")?,
            declaration_identity: reader.string("closed conformance declaration identity")?,
            telescope: decode_counted(reader, |reader| {
                Ok(ClosedConformanceParameterBinding {
                    parameter: reader.string("closed conformance parameter")?,
                    kind: match reader.u8()? {
                        1 => ClosedConformanceParameterKind::Lifetime,
                        2 => ClosedConformanceParameterKind::Type,
                        3 => ClosedConformanceParameterKind::Const,
                        4 => ClosedConformanceParameterKind::Machine,
                        tag => {
                            return Err(CodecError::InvalidTag(
                                "ClosedConformanceParameterKind",
                                tag,
                            ));
                        }
                    },
                    argument: reader.string("closed conformance argument")?,
                })
            })?,
            subject_identity: reader
                .boolean()?
                .then(|| reader.string("closed conformance subject identity"))
                .transpose()?,
            trait_identity: reader.string("closed conformance trait identity")?,
            trait_lifetime_arguments: reader
                .strings("closed conformance trait lifetime arguments")?,
            trait_arguments: reader.strings("closed conformance trait arguments")?,
            realization_callables: decode_counted(reader, |reader| {
                Ok(psi_terminal::ClosedConformanceRealizationCallable {
                    source_callable_identity: reader
                        .string("closed conformance realization callable identity")?,
                    machine: reader.id("MachineId")?,
                    result: match reader.u8()? {
                        1 => ClosedConformanceCallableResult::Unit,
                        2 => ClosedConformanceCallableResult::I32,
                        3 => ClosedConformanceCallableResult::Bool,
                        tag => {
                            return Err(CodecError::InvalidTag(
                                "ClosedConformanceCallableResult",
                                tag,
                            ));
                        }
                    },
                })
            })?,
            rows: decode_counted(reader, |reader| {
                Ok(ClosedConformanceRow {
                    declaring_trait_identity: reader
                        .string("closed conformance row declaring trait identity")?,
                    public_requirement_identity: reader
                        .string("closed conformance row public requirement identity")?,
                    requirement_identity: reader
                        .string("closed conformance row requirement identity")?,
                    realization_identity: reader
                        .string("closed conformance row realization identity")?,
                    realization_callable_identity: reader
                        .boolean()?
                        .then(|| {
                            reader.string("closed conformance row realization callable identity")
                        })
                        .transpose()?,
                })
            })?,
            report_fingerprint: reader.u64()?,
            commitment: ClosedConformanceApplicationCommitment::from_digest(reader.array()?),
        })
    })?;
    let (
        dynamic_descriptor_parameters,
        dynamic_descriptor_arguments,
        dynamic_conformance_selections,
        rebound_dynamic_descriptors,
        direct_dynamic_dispatches,
        indirect_dynamic_dispatches,
        parameter_dynamic_dispatches,
    ) = if result_path_format == ResultPathWireFormat::Current {
        (
            decode_dynamic_descriptor_parameters(reader)?,
            decode_dynamic_descriptor_arguments(reader)?,
            decode_dynamic_conformance_selections(reader)?,
            decode_rebound_dynamic_descriptors(reader)?,
            decode_direct_dynamic_dispatches(reader)?,
            decode_indirect_dynamic_dispatches(reader)?,
            decode_parameter_dynamic_dispatches(reader)?,
        )
    } else {
        (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    let quotient_correspondences = decode_counted(reader, decode_quotient_correspondence)?;
    let machine_count = reader.count()?;
    let mut machines = Vec::new();
    for _ in 0..machine_count {
        machines.push(super::machine_wire::decode_machine_for_result_paths(
            reader,
            result_path_format,
        )?);
    }
    Ok(TerminalModule {
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
        dynamic_dispatch: psi_terminal::TerminalDynamicDispatchCatalog {
            parameters: dynamic_descriptor_parameters,
            arguments: dynamic_descriptor_arguments,
            selections: dynamic_conformance_selections,
            rebound_descriptors: rebound_dynamic_descriptors,
            direct_dispatches: direct_dynamic_dispatches,
            indirect_dispatches: indirect_dynamic_dispatches,
            parameter_dispatches: parameter_dynamic_dispatches,
        },
        quotient_correspondences,
        machines,
    })
}
