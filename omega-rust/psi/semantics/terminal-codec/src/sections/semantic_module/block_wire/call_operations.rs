//! Call operations on the wire: scalar, Unit, structural and dynamic calls
//! with their arguments, claim transfers, obligations and crash routes, plus
//! boundary calls and port writes.

use super::super::CodecError;
use super::super::contract_wire::{decode_crash_routes, encode_crash_routes};
use super::super::proof_declaration_wire::{decode_evidence_interface, encode_evidence_interface};
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use crate::sections::semantic_module::scalar_term_wire::{
    decode_scalar_terms, encode_scalar_terms,
};
use crate::sections::semantic_module::structural_place_wire::encode_obligation_ids;
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_arguments, encode_structural_arguments,
};
use crate::sections::semantic_module::wire::{decode_counted, decode_ids};
use semantic_vocabulary::ScalarTerm;
use semantic_vocabulary::{BoundaryMachineId, MachineId, ObligationId, ServiceId, ValueId};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, OperationKind, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificGuard, StructuralResultClaimTransfer,
};
use terminal_psi::{CrashRouteBucket, StructuralArgument};

pub(super) fn encode_call(
    writer: &mut Writer,
    callee: MachineId,
    arguments: Vec<ValueId>,
    erased_arguments: Vec<ScalarTerm>,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL);
    writer.id(callee);
    writer.len("call arguments", arguments.len())?;
    for argument in arguments {
        writer.id(argument);
    }
    encode_scalar_terms(writer, &erased_arguments)?;
    writer.len(
        "call requirement obligations",
        requirement_obligations.len(),
    )?;
    for obligation in requirement_obligations {
        writer.id(obligation);
    }
    encode_crash_routes(writer, &crash_continuations)?;
    Ok(())
}

pub(super) fn decode_call(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    let callee = reader.id("MachineId")?;
    let argument_count = reader.count()?;
    let mut arguments =
        Vec::with_capacity(usize::try_from(argument_count).expect("u32 count fits usize"));
    for _ in 0..argument_count {
        arguments.push(reader.id("ValueId")?);
    }
    let erased_arguments = decode_scalar_terms(reader)?;
    let requirement_count = reader.count()?;
    let mut requirement_obligations =
        Vec::with_capacity(usize::try_from(requirement_count).expect("u32 count fits usize"));
    for _ in 0..requirement_count {
        requirement_obligations.push(reader.id("ObligationId")?);
    }
    let crash_continuations = decode_crash_routes(reader)?;
    Ok(OperationKind::Call {
        callee,
        arguments,
        erased_arguments,
        requirement_obligations,
        crash_continuations,
    })
}

pub(super) fn encode_call_unit(
    writer: &mut Writer,
    callee: MachineId,
    arguments: Vec<ValueId>,
    erased_arguments: Vec<ScalarTerm>,
    structural_arguments: Vec<StructuralArgument>,
    claim_transfers: Vec<ClaimTransfer>,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL_UNIT);
    writer.id(callee);
    writer.len("unit-call scalar arguments", arguments.len())?;
    for argument in arguments {
        writer.id(argument);
    }
    encode_scalar_terms(writer, &erased_arguments)?;
    encode_structural_arguments(writer, &structural_arguments)?;
    writer.len("unit-call claim transfers", claim_transfers.len())?;
    for transfer in claim_transfers {
        writer.id(transfer.claim);
        writer.u32(transfer.argument_index);
    }
    encode_obligation_ids(writer, &requirement_obligations)?;
    encode_crash_routes(writer, &crash_continuations)?;
    Ok(())
}

pub(super) fn decode_call_unit(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::CallUnit {
        callee: reader.id("MachineId")?,
        arguments: decode_ids(reader, "ValueId")?,
        erased_arguments: decode_scalar_terms(reader)?,
        structural_arguments: decode_structural_arguments(reader)?,
        claim_transfers: decode_counted(reader, |reader| {
            Ok(ClaimTransfer {
                claim: reader.id("ClaimId")?,
                argument_index: reader.u32()?,
            })
        })?,
        requirement_obligations: decode_ids(reader, "ObligationId")?,
        crash_continuations: decode_crash_routes(reader)?,
    })
}

pub(super) fn encode_call_structural_scalar(
    writer: &mut Writer,
    callee: MachineId,
    arguments: Vec<ValueId>,
    erased_arguments: Vec<ScalarTerm>,
    structural_arguments: Vec<StructuralArgument>,
    claim_transfers: Vec<ClaimTransfer>,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL_STRUCTURAL_SCALAR);
    writer.id(callee);
    writer.len("structural-scalar-call scalar arguments", arguments.len())?;
    for argument in arguments {
        writer.id(argument);
    }
    encode_scalar_terms(writer, &erased_arguments)?;
    encode_structural_arguments(writer, &structural_arguments)?;
    writer.len(
        "structural-scalar-call claim transfers",
        claim_transfers.len(),
    )?;
    for transfer in claim_transfers {
        writer.id(transfer.claim);
        writer.u32(transfer.argument_index);
    }
    encode_obligation_ids(writer, &requirement_obligations)?;
    encode_crash_routes(writer, &crash_continuations)?;
    Ok(())
}

pub(super) fn decode_call_structural_scalar(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::CallStructuralScalar {
        callee: reader.id("MachineId")?,
        arguments: decode_ids(reader, "ValueId")?,
        erased_arguments: decode_scalar_terms(reader)?,
        structural_arguments: decode_structural_arguments(reader)?,
        claim_transfers: decode_counted(reader, |reader| {
            Ok(ClaimTransfer {
                claim: reader.id("ClaimId")?,
                argument_index: reader.u32()?,
            })
        })?,
        requirement_obligations: decode_ids(reader, "ObligationId")?,
        crash_continuations: decode_crash_routes(reader)?,
    })
}

pub(super) fn encode_call_dynamic_scalar(
    writer: &mut Writer,
    descriptor_ordinal: u32,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL_DYNAMIC_SCALAR);
    writer.u32(descriptor_ordinal);
    encode_obligation_ids(writer, &requirement_obligations)?;
    encode_crash_routes(writer, &crash_continuations)?;
    Ok(())
}

pub(super) fn decode_call_dynamic_scalar(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::CallDynamicScalar {
        descriptor_ordinal: reader.u32()?,
        requirement_obligations: decode_ids(reader, "ObligationId")?,
        crash_continuations: decode_crash_routes(reader)?,
    })
}

pub(super) fn encode_call_dynamic_parameter_scalar(
    writer: &mut Writer,
    parameter_ordinal: u32,
    requirement_slot: u32,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL_DYNAMIC_PARAMETER_SCALAR);
    writer.u32(parameter_ordinal);
    writer.u32(requirement_slot);
    encode_obligation_ids(writer, &requirement_obligations)?;
    encode_crash_routes(writer, &crash_continuations)?;
    Ok(())
}

pub(super) fn decode_call_dynamic_parameter_scalar(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::CallDynamicParameterScalar {
        parameter_ordinal: reader.u32()?,
        requirement_slot: reader.u32()?,
        requirement_obligations: decode_ids(reader, "ObligationId")?,
        crash_continuations: decode_crash_routes(reader)?,
    })
}

pub(super) fn encode_call_dynamic_unit(
    writer: &mut Writer,
    descriptor_ordinal: u32,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL_DYNAMIC_UNIT);
    writer.u32(descriptor_ordinal);
    encode_obligation_ids(writer, &requirement_obligations)?;
    encode_crash_routes(writer, &crash_continuations)?;
    Ok(())
}

pub(super) fn decode_call_dynamic_unit(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::CallDynamicUnit {
        descriptor_ordinal: reader.u32()?,
        requirement_obligations: decode_ids(reader, "ObligationId")?,
        crash_continuations: decode_crash_routes(reader)?,
    })
}

pub(super) fn encode_call_dynamic_parameter_unit(
    writer: &mut Writer,
    parameter_ordinal: u32,
    requirement_slot: u32,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL_DYNAMIC_PARAMETER_UNIT);
    writer.u32(parameter_ordinal);
    writer.u32(requirement_slot);
    encode_obligation_ids(writer, &requirement_obligations)?;
    encode_crash_routes(writer, &crash_continuations)?;
    Ok(())
}

pub(super) fn decode_call_dynamic_parameter_unit(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::CallDynamicParameterUnit {
        parameter_ordinal: reader.u32()?,
        requirement_slot: reader.u32()?,
        requirement_obligations: decode_ids(reader, "ObligationId")?,
        crash_continuations: decode_crash_routes(reader)?,
    })
}

pub(super) fn encode_call_structural(
    writer: &mut Writer,
    callee: MachineId,
    structural_arguments: Vec<StructuralArgument>,
    claim_transfers: Vec<ClaimTransfer>,
    returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
    selected_evidence: Vec<OutcomeSpecificCallEvidence>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL_STRUCTURAL);
    writer.id(callee);
    encode_structural_arguments(writer, &structural_arguments)?;
    writer.len("structural-call claim transfers", claim_transfers.len())?;
    for transfer in claim_transfers {
        writer.id(transfer.claim);
        writer.u32(transfer.argument_index);
    }
    writer.len(
        "structural-call returned claim transfers",
        returned_claim_transfers.len(),
    )?;
    for transfer in returned_claim_transfers {
        writer.id(transfer.callee_claim);
        writer.id(transfer.caller_claim);
    }
    encode_obligation_ids(writer, &requirement_obligations)?;
    encode_crash_routes(writer, &crash_continuations)?;
    writer.len("guarded call selected evidence", selected_evidence.len())?;
    for binding in selected_evidence {
        writer.id(binding.guard.result_type);
        writer.id(binding.guard.result_case);
        writer.u32(binding.position);
        writer.id(binding.callee_obligation);
        writer.id(binding.callee_term);
        writer.string("guarded call output field", &binding.output_field)?;
        writer.id(binding.callee_proposition);
        writer.id(binding.instantiated_proposition);
        writer.id(binding.output);
        match binding.result_substitution {
            None => writer.u8(0),
            Some(substitution) => {
                writer.u8(1);
                writer.u32(substitution.argument_position);
                writer.id(substitution.callee_result);
                writer.id(substitution.caller_result);
            }
        }
        writer.id(binding.validity.result);
        writer.len(
            "guarded call proposition dependencies",
            binding.validity.proposition_dependencies.len(),
        )?;
        for dependency in &binding.validity.proposition_dependencies {
            writer.id(*dependency);
        }
        encode_evidence_interface(writer, &binding.validity.evidence_interface)?;
        writer.len(
            "guarded call interface dependencies",
            binding.validity.interface_dependencies.len(),
        )?;
        for dependency in &binding.validity.interface_dependencies {
            writer.id(*dependency);
        }
        writer.u32(binding.expected_use_count);
        writer.len("guarded selected evidence uses", binding.uses.len())?;
        for use_ in &binding.uses {
            writer.id(use_.target);
            writer.u32(use_.input_position);
            writer.id(use_.target_requirement);
            writer.id(use_.target_term);
            writer.id(use_.source);
            writer.id(use_.instantiated_proposition);
            writer.id(use_.target_parameter);
            writer.id(use_.caller_result);
        }
    }
    Ok(())
}

pub(super) fn decode_call_structural(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::CallStructural {
        callee: reader.id("MachineId")?,
        structural_arguments: decode_structural_arguments(reader)?,
        claim_transfers: decode_counted(reader, |reader| {
            Ok(ClaimTransfer {
                claim: reader.id("ClaimId")?,
                argument_index: reader.u32()?,
            })
        })?,
        returned_claim_transfers: decode_counted(reader, |reader| {
            Ok(StructuralResultClaimTransfer {
                callee_claim: reader.id("ClaimId")?,
                caller_claim: reader.id("ClaimId")?,
            })
        })?,
        requirement_obligations: decode_ids(reader, "ObligationId")?,
        crash_continuations: decode_crash_routes(reader)?,
        selected_evidence: decode_counted(reader, |reader| {
            Ok(OutcomeSpecificCallEvidence {
                guard: OutcomeSpecificGuard {
                    result_type: reader.id("StructuralTypeId")?,
                    result_case: reader.id("StructuralCaseId")?,
                },
                position: reader.u32()?,
                callee_obligation: reader.id("ObligationId")?,
                callee_term: reader.id("EvidenceTermId")?,
                output_field: reader.string("guarded call output field")?,
                callee_proposition: reader.id("PropositionId")?,
                instantiated_proposition: reader.id("PropositionId")?,
                output: reader.id("EvidenceTermId")?,
                result_substitution: match reader.u8()? {
                    0 => None,
                    1 => Some(OutcomeSpecificCallResultSubstitution {
                        argument_position: reader.u32()?,
                        callee_result: reader.id("PlaceId")?,
                        caller_result: reader.id("PlaceId")?,
                    }),
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "OutcomeSpecificCallResultSubstitution",
                            tag,
                        ));
                    }
                },
                validity: OutcomeSpecificCallEvidenceValidity {
                    result: reader.id("PlaceId")?,
                    proposition_dependencies: decode_ids(reader, "PlaceId")?,
                    evidence_interface: decode_evidence_interface(reader)?,
                    interface_dependencies: decode_ids(reader, "PlaceId")?,
                },
                expected_use_count: reader.u32()?,
                uses: decode_counted(reader, |reader| {
                    Ok(terminal_psi::OutcomeSpecificEvidenceUse {
                        target: reader.id("MachineId")?,
                        input_position: reader.u32()?,
                        target_requirement: reader.id("PropositionId")?,
                        target_term: reader.id("EvidenceTermId")?,
                        source: reader.id("EvidenceTermId")?,
                        instantiated_proposition: reader.id("PropositionId")?,
                        target_parameter: reader.id("PlaceId")?,
                        caller_result: reader.id("PlaceId")?,
                    })
                })?,
            })
        })?,
    })
}

pub(super) fn encode_call_structural_with_scalar_arguments(
    writer: &mut Writer,
    callee: MachineId,
    arguments: Vec<ValueId>,
    erased_arguments: Vec<ScalarTerm>,
    structural_arguments: Vec<StructuralArgument>,
    claim_transfers: Vec<ClaimTransfer>,
    returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
    requirement_obligations: Vec<ObligationId>,
    crash_continuations: Vec<CrashRouteBucket>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS);
    writer.id(callee);
    writer.len("mixed structural-call scalar arguments", arguments.len())?;
    for argument in arguments {
        writer.id(argument);
    }
    encode_scalar_terms(writer, &erased_arguments)?;
    encode_structural_arguments(writer, &structural_arguments)?;
    writer.len(
        "mixed structural-call claim transfers",
        claim_transfers.len(),
    )?;
    for transfer in claim_transfers {
        writer.id(transfer.claim);
        writer.u32(transfer.argument_index);
    }
    writer.len(
        "mixed structural-call returned claim transfers",
        returned_claim_transfers.len(),
    )?;
    for transfer in returned_claim_transfers {
        writer.id(transfer.callee_claim);
        writer.id(transfer.caller_claim);
    }
    encode_obligation_ids(writer, &requirement_obligations)?;
    encode_crash_routes(writer, &crash_continuations)?;
    Ok(())
}

pub(super) fn decode_call_structural_with_scalar_arguments(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::CallStructuralWithScalarArguments {
        callee: reader.id("MachineId")?,
        arguments: decode_ids(reader, "ValueId")?,
        erased_arguments: decode_scalar_terms(reader)?,
        structural_arguments: decode_structural_arguments(reader)?,
        claim_transfers: decode_counted(reader, |reader| {
            Ok(ClaimTransfer {
                claim: reader.id("ClaimId")?,
                argument_index: reader.u32()?,
            })
        })?,
        returned_claim_transfers: decode_counted(reader, |reader| {
            Ok(StructuralResultClaimTransfer {
                callee_claim: reader.id("ClaimId")?,
                caller_claim: reader.id("ClaimId")?,
            })
        })?,
        requirement_obligations: decode_ids(reader, "ObligationId")?,
        crash_continuations: decode_crash_routes(reader)?,
    })
}

pub(super) fn encode_boundary_call(
    writer: &mut Writer,
    boundary: BoundaryMachineId,
    arguments: Vec<ValueId>,
    structural_arguments: Vec<StructuralArgument>,
    completion_receipts: Vec<CompletionReceipt>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BOUNDARY_CALL);
    writer.id(boundary);
    writer.len("boundary scalar arguments", arguments.len())?;
    for argument in arguments {
        writer.id(argument);
    }
    encode_structural_arguments(writer, &structural_arguments)?;
    writer.len("boundary claim settlements", completion_receipts.len())?;
    for settlement in completion_receipts {
        writer.id(settlement.claim);
        writer.u32(settlement.argument_index);
    }
    Ok(())
}

pub(super) fn decode_boundary_call(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::BoundaryCall {
        boundary: reader.id("BoundaryMachineId")?,
        arguments: decode_ids(reader, "ValueId")?,
        structural_arguments: decode_structural_arguments(reader)?,
        completion_receipts: decode_counted(reader, |reader| {
            Ok(CompletionReceipt {
                claim: reader.id("ClaimId")?,
                argument_index: reader.u32()?,
            })
        })?,
    })
}

pub(super) fn encode_port_write(
    writer: &mut Writer,
    service: ServiceId,
    port: u16,
    value: u8,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::PORT_WRITE);
    writer.id(service);
    writer.u16(port);
    writer.u8(value);
    Ok(())
}

pub(super) fn decode_port_write(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::PortWrite {
        service: reader.id("ServiceId")?,
        port: reader.u16()?,
        value: reader.u8()?,
    })
}
