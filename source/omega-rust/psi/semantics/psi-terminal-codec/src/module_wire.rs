//! Canonical terminal-module envelope wire format.
//!
//! This module owns the ordered top-level declaration tables and exact module
//! vocabulary envelope. Individual declaration, machine, scalar, proof, and
//! structural payloads remain in their dedicated sibling wire modules.

use psi_core::{ContentProjectionIdentity, IeeeFloatFormat};
use psi_terminal::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceParameterBinding, ClosedConformanceParameterKind, ClosedConformanceRow,
    EvidenceContractLane, EvidenceContractLaneKind, EvidenceTermDeclaration,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatProjectionInput, FloatProjectionInputId, InstallationReachDependency, ProofOnlyValueType,
    ProofOutput, ProofOutputCall, ProofOutputEvidenceArgument, ProofOutputRuntimeCall,
    ProofOutputRuntimeResult, ProofPropositionId, ProofValueDeclaration, ProofValueId,
    ServiceDeclaration, StaticRequirementDispatch, StructuralContentProjection,
    StructuralDomainDeclaration, TerminalModule, TerminalRootServiceReach, VocabularyMarker,
};

use super::content_wire::{decode_content_algebra, encode_content_algebra};
use super::machine_wire::{decode_machine, encode_machine};
use super::proof_declaration_wire::{
    decode_evidence_interface, decode_proposition_application, decode_proposition_declaration,
    encode_evidence_interface, encode_proposition_application, encode_proposition_declaration,
};
use super::provider_candidate_wire::{decode_provider_candidate, encode_provider_candidate};
use super::quotient_correspondence_wire::{
    decode_quotient_correspondence, encode_quotient_correspondence,
};
use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::structural_signature_wire::{
    decode_boundary_machine, decode_content_projection_expression, encode_boundary_machine,
    encode_content_projection_expression,
};
use super::structural_type_wire::{decode_structural_type, encode_structural_type};
use super::wire::{Reader, Writer};
use super::{CodecError, FORMAT_MARKER, MAGIC, decode_counted, decode_ids};

pub(super) fn encode_raw(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    writer.u16(module.vocabulary_marker.get());
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
        writer.u32(projection.result.id.0);
        writer.u8(match projection.result.value_type {
            ProofOnlyValueType::FloatMeaning => 1,
        });
        writer.u32(projection.source.id.0);
        writer.u8(match projection.source.format {
            IeeeFloatFormat::Binary32 => 1,
            IeeeFloatFormat::Binary64 => 2,
        });
        writer.u8(match projection.operation {
            FloatMeaningProjectionOperation::Meaning32 => 1,
            FloatMeaningProjectionOperation::Meaning64 => 2,
        });
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
            "closed conformance trait arguments",
            &application.trait_arguments,
        )?;
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
        }
        writer.u64(application.report_fingerprint);
        writer.bytes(&application.commitment.as_bytes());
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
        encode_machine(&mut writer, machine)?;
    }
    Ok(writer.finish())
}

pub(super) fn decode_module_body(reader: &mut Reader<'_>) -> Result<TerminalModule, CodecError> {
    let vocabulary_marker_raw = reader.u16()?;
    let vocabulary_marker = VocabularyMarker::new(vocabulary_marker_raw).ok_or(
        CodecError::UnsupportedVocabularyMarker(vocabulary_marker_raw),
    )?;
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
            source: FloatProjectionInput {
                id: FloatProjectionInputId(reader.u32()?),
                format: match reader.u8()? {
                    1 => IeeeFloatFormat::Binary32,
                    2 => IeeeFloatFormat::Binary64,
                    tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                },
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
            trait_arguments: reader.strings("closed conformance trait arguments")?,
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
                })
            })?,
            report_fingerprint: reader.u64()?,
            commitment: ClosedConformanceApplicationCommitment::from_digest(reader.array()?),
        })
    })?;
    let quotient_correspondences = decode_counted(reader, decode_quotient_correspondence)?;
    let machine_count = reader.count()?;
    let mut machines = Vec::new();
    for _ in 0..machine_count {
        machines.push(decode_machine(reader)?);
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
        boundary_machines,
        provider_candidates,
        float_meaning_projections,
        float_meaning_equalities,
        proposition_declarations,
        proposition_applications,
        evidence_terms,
        evidence_contract_lanes,
        proof_output_calls,
        closed_conformance_applications,
        quotient_correspondences,
        machines,
    })
}
