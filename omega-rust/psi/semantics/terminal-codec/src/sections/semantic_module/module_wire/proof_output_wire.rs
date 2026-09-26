//! Proof-output invocations on the wire: the target machine, static
//! requirement dispatch, runtime result and call, evidence arguments and
//! outputs of one invocation.

use super::super::CodecError;
use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::decode_counted;
use terminal_psi::{
    ClosedConformanceApplicationCommitment, ProofOutput, ProofOutputCall,
    ProofOutputEvidenceArgument, ProofOutputRuntimeCall, ProofOutputRuntimeResult,
    StaticRequirementDispatch,
};

pub(super) fn encode_proof_output_call(
    writer: &mut Writer,
    invocation: &ProofOutputCall,
) -> Result<(), CodecError> {
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
            encode_scalar_type(writer, runtime_value);
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
    Ok(())
}

pub(super) fn decode_proof_output_call(
    reader: &mut Reader<'_>,
) -> Result<ProofOutputCall, CodecError> {
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
                forwarded_input_position: reader.boolean()?.then(|| reader.u32()).transpose()?,
                output: reader
                    .boolean()?
                    .then(|| reader.id("EvidenceTermId"))
                    .transpose()?,
            })
        })?,
    })
}
