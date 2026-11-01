//! Float-meaning projections and equalities on the wire: each projection's
//! result, source, operation and contract identity.

use super::super::CodecError;
use super::super::structural_field_wire::{
    decode_ieee_float_field, decode_ieee_float_format, encode_ieee_float_field,
    encode_ieee_float_format,
};
use super::super::wire::{Reader, Writer};
use super::super::wire_fields::{
    Digest, Field, Id, List, U8, U16, U32, U64, u32_newtypes, wire_enum, wire_record,
};
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatStructuralField};
use terminal_psi::{
    DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatParameter,
    DirectMachineFloatResult, DirectOperationFloatResult, DirectStructuralFloatLeaf,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionContractIdentity, FloatProjectionInput,
    FloatProjectionInputId, FloatSemanticApplication, FloatSemanticApplicationOperand,
    FloatSemanticContractIdentity, ProofOnlyValueType, ProofPropositionId, ProofValueDeclaration,
    ProofValueId,
};

u32_newtypes! {
    Value: ProofValueId;
    Proposition: ProofPropositionId;
    InputId: FloatProjectionInputId;
}

struct Format;
impl Field<IeeeFloatFormat> for Format {
    fn write(writer: &mut Writer, value: &IeeeFloatFormat) -> Result<(), CodecError> {
        encode_ieee_float_format(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<IeeeFloatFormat, CodecError> {
        decode_ieee_float_format(reader)
    }
}

struct FloatField;
impl Field<IeeeFloatStructuralField> for FloatField {
    fn write(writer: &mut Writer, value: &IeeeFloatStructuralField) -> Result<(), CodecError> {
        encode_ieee_float_field(writer, value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<IeeeFloatStructuralField, CodecError> {
        decode_ieee_float_field(reader)
    }
}

wire_record! {
    Projection: FloatMeaningProjection {
        result: Declaration,
        source: Source,
        operation: Operation,
        contract: ProjectionContract,
    }
    Declaration: ProofValueDeclaration { id: Value, value_type: ValueType }
    Input: FloatProjectionInput { id: InputId, format: Format }
    MachineParameter: DirectMachineFloatParameter { owner: Id, parameter: Id, format: Format }
    MachineResult: DirectMachineFloatResult { owner: Id, result: Id, format: Format }
    OperationResult: DirectOperationFloatResult {
        owner: Id,
        producer: Id,
        result: Id,
        format: Format,
    }
    BlockParameter: DirectBlockFloatParameter {
        owner: Id,
        block: Id,
        parameter: Id,
        format: Format,
    }
    CallResult: DirectCallFloatResult { owner: Id, producer: Id, result: Id, format: Format }
    StructuralLeaf: DirectStructuralFloatLeaf { owner: Id, field: FloatField, format: Format }
    Application: FloatSemanticApplication {
        contract: ApplicationContract,
        format: Format,
        operands: List<ApplicationOperand>,
    }
    ApplicationContract: FloatSemanticContractIdentity {
        row: U8,
        catalog_version: U16,
        commitment: Digest,
    }
    ProjectionContract: FloatProjectionContractIdentity {
        format: U16,
        operation: U8,
        declaration: U8,
        catalog_version: U16,
        commitment: Digest,
    }
    Equality: FloatMeaningEqualityProposition { id: Proposition, left: Value, right: Value }
}

wire_enum! {
    ValueType: ProofOnlyValueType, "ProofOnlyValueType" {
        1 => FloatMeaning,
    }
    Source: FloatMeaningSource, "FloatMeaningSource" {
        1 => TransitionalInput(input: Input),
        2 => ExactBinary32Literal(bits: U32),
        3 => ExactBinary64Literal(bits: U64),
        4 => DirectMachineParameter(parameter: MachineParameter),
        5 => DirectMachineResult(result: MachineResult),
        6 => DirectOperationResult(result: OperationResult),
        7 => DirectBlockParameter(parameter: BlockParameter),
        8 => DirectCallResult(result: CallResult),
        9 => DirectStructuralLeaf(leaf: StructuralLeaf),
        10 => SemanticApplication(application: Application),
    }
    ApplicationOperand: FloatSemanticApplicationOperand, "FloatSemanticApplicationOperand" {
        1 => Format(format: Format),
        2 => Meaning(value: Value),
    }
    Operation: FloatMeaningProjectionOperation, "FloatMeaningProjectionOperation" {
        1 => Meaning32,
        2 => Meaning64,
    }
}
