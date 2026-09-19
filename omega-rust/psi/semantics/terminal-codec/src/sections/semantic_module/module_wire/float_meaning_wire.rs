//! Float-meaning projections and equalities on the wire: each projection's
//! result, source, operation and contract identity.

use super::super::CodecError;
use super::super::structural_field_wire::{decode_ieee_float_field, encode_ieee_float_field};
use super::super::wire::{Reader, Writer};
use semantic_vocabulary::IeeeFloatFormat;
use terminal_psi::{
    DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatParameter,
    DirectMachineFloatResult, DirectOperationFloatResult, DirectStructuralFloatLeaf,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionInput, FloatProjectionInputId, FloatSemanticApplication,
    FloatSemanticApplicationOperand, FloatSemanticContractIdentity, ProofOnlyValueType,
    ProofPropositionId, ProofValueDeclaration, ProofValueId,
};

fn encode_ieee_format(writer: &mut Writer, format: IeeeFloatFormat) {
    writer.u8(match format {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}

fn decode_ieee_format(reader: &mut Reader<'_>) -> Result<IeeeFloatFormat, CodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatFormat::Binary32),
        2 => Ok(IeeeFloatFormat::Binary64),
        tag => Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
    }
}

pub(super) fn encode_float_meaning_projection(
    writer: &mut Writer,
    projection: &FloatMeaningProjection,
) -> Result<(), CodecError> {
    writer.u32(projection.result.id.0);
    writer.u8(match projection.result.value_type {
        ProofOnlyValueType::FloatMeaning => 1,
    });
    match &projection.source {
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
        FloatMeaningSource::DirectCallResult(result) => {
            writer.u8(8);
            writer.id(result.owner);
            writer.id(result.producer);
            writer.id(result.result);
            writer.u8(match result.format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            });
        }
        FloatMeaningSource::DirectStructuralLeaf(leaf) => {
            writer.u8(9);
            writer.id(leaf.owner);
            encode_ieee_float_field(writer, &leaf.field)?;
            writer.u8(match leaf.format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            });
        }
        FloatMeaningSource::ExactBinary32Literal(bits) => {
            writer.u8(2);
            writer.u32(*bits);
        }
        FloatMeaningSource::ExactBinary64Literal(bits) => {
            writer.u8(3);
            writer.u64(*bits);
        }
        FloatMeaningSource::SemanticApplication(application) => {
            writer.u8(10);
            writer.u8(application.contract.row);
            writer.u16(application.contract.catalog_version);
            writer.bytes(&application.contract.commitment);
            encode_ieee_format(writer, application.format);
            writer.len(
                "float semantic application operands",
                application.operands.len(),
            )?;
            for operand in &application.operands {
                match operand {
                    FloatSemanticApplicationOperand::Format(format) => {
                        writer.u8(1);
                        encode_ieee_format(writer, *format);
                    }
                    FloatSemanticApplicationOperand::Meaning(value) => {
                        writer.u8(2);
                        writer.u32(value.0);
                    }
                }
            }
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
    Ok(())
}

pub(super) fn encode_float_meaning_equality(
    writer: &mut Writer,
    proposition: &FloatMeaningEqualityProposition,
) -> Result<(), CodecError> {
    writer.u32(proposition.id.0);
    writer.u32(proposition.left.0);
    writer.u32(proposition.right.0);
    Ok(())
}

pub(super) fn decode_float_meaning_projection(
    reader: &mut Reader<'_>,
) -> Result<FloatMeaningProjection, CodecError> {
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
            6 => FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
                owner: reader.id("float-meaning direct operation-result owner")?,
                producer: reader.id("float-meaning direct operation-result producer")?,
                result: reader.id("float-meaning direct operation-result value")?,
                format: match reader.u8()? {
                    1 => IeeeFloatFormat::Binary32,
                    2 => IeeeFloatFormat::Binary64,
                    tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                },
            }),
            7 => FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
                owner: reader.id("float-meaning direct block-parameter owner")?,
                block: reader.id("float-meaning direct block-parameter block")?,
                parameter: reader.id("float-meaning direct block-parameter value")?,
                format: match reader.u8()? {
                    1 => IeeeFloatFormat::Binary32,
                    2 => IeeeFloatFormat::Binary64,
                    tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                },
            }),
            8 => FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
                owner: reader.id("float-meaning direct call-result owner")?,
                producer: reader.id("float-meaning direct call-result producer")?,
                result: reader.id("float-meaning direct call-result value")?,
                format: match reader.u8()? {
                    1 => IeeeFloatFormat::Binary32,
                    2 => IeeeFloatFormat::Binary64,
                    tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                },
            }),
            9 => FloatMeaningSource::DirectStructuralLeaf(DirectStructuralFloatLeaf {
                owner: reader.id("float-meaning direct structural-leaf owner")?,
                field: decode_ieee_float_field(reader)?,
                format: decode_ieee_format(reader)?,
            }),
            10 => {
                let contract = FloatSemanticContractIdentity {
                    row: reader.u8()?,
                    catalog_version: reader.u16()?,
                    commitment: reader.array()?,
                };
                let format = decode_ieee_format(reader)?;
                let operand_count = reader.count()?;
                let mut operands =
                    Vec::with_capacity(usize::try_from(operand_count).unwrap_or(usize::MAX));
                for _ in 0..operand_count {
                    operands.push(match reader.u8()? {
                        1 => FloatSemanticApplicationOperand::Format(decode_ieee_format(reader)?),
                        2 => FloatSemanticApplicationOperand::Meaning(ProofValueId(reader.u32()?)),
                        tag => {
                            return Err(CodecError::InvalidTag(
                                "FloatSemanticApplicationOperand",
                                tag,
                            ));
                        }
                    });
                }
                FloatMeaningSource::SemanticApplication(FloatSemanticApplication {
                    contract,
                    format,
                    operands,
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
        contract: terminal_psi::FloatProjectionContractIdentity {
            format: reader.u16()?,
            operation: reader.u8()?,
            declaration: reader.u8()?,
            catalog_version: reader.u16()?,
            commitment: reader.array()?,
        },
    })
}

pub(super) fn decode_float_meaning_equality(
    reader: &mut Reader<'_>,
) -> Result<FloatMeaningEqualityProposition, CodecError> {
    Ok(FloatMeaningEqualityProposition {
        id: ProofPropositionId(reader.u32()?),
        left: ProofValueId(reader.u32()?),
        right: ProofValueId(reader.u32()?),
    })
}
