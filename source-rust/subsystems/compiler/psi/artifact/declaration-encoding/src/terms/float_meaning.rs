use crate::CodecError;
use crate::fields::{
    Digest, Field, Id, List, U8, U16, U32, U64, u32_newtypes, wire_enum, wire_record,
};
use crate::structural_fields::{
    decode_ieee_float_field, decode_ieee_float_format, encode_ieee_float_field,
    encode_ieee_float_format,
};
use crate::wire::{Reader, Writer};
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

use numerics::float_semantics::FloatSemanticOperationIdentity;
use numerics::integer_carrier::{FixedInteger, IntegerSign};

struct IntegerCarrier;
impl Field<FixedInteger> for IntegerCarrier {
    fn write(writer: &mut Writer, value: &FixedInteger) -> Result<(), CodecError> {
        writer.u8(u8::from(value.is_signed()));
        writer.u64(value.width());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<FixedInteger, CodecError> {
        let sign = match reader.u8()? {
            0 => IntegerSign::Unsigned,
            1 => IntegerSign::Signed,
            tag => return Err(CodecError::InvalidTag("float semantic integer sign", tag)),
        };
        FixedInteger::new(sign, reader.u64()?).ok_or(CodecError::MalformedStructuralFoundation(
            "float semantic integer width must be positive",
        ))
    }
}

wire_enum! {
    SemanticOperation: FloatSemanticOperationIdentity, "FloatSemanticOperationIdentity" {
        0 => Fixed(row: U8),
        1 => FromInteger(carrier: IntegerCarrier),
        2 => ToIntegerExact(carrier: IntegerCarrier),
        3 => ToIntegerSaturating(carrier: IntegerCarrier),
    }
}

#[cfg(test)]
mod float_conversion_identity_tests {
    use super::*;
    use numerics::float_semantics::{FloatSemanticOperation, FloatSemanticValueKind};

    #[test]
    fn width_parameterized_float_contracts_round_trip_without_source() {
        for carrier in [
            FixedInteger::signed(1),
            FixedInteger::unsigned(13),
            FixedInteger::signed(257),
            FixedInteger::unsigned(u64::MAX),
        ] {
            let integer = FloatSemanticValueKind::Integer(carrier);
            for (name, parameters, result) in [
                (
                    "from_integer".to_owned(),
                    vec![FloatSemanticValueKind::Format, integer],
                    FloatSemanticValueKind::Meaning,
                ),
                (
                    format!("to_{carrier}"),
                    vec![FloatSemanticValueKind::Meaning],
                    integer,
                ),
                (
                    format!("to_{carrier}_saturating"),
                    vec![FloatSemanticValueKind::Meaning],
                    integer,
                ),
            ] {
                let operation = FloatSemanticOperation::from_source_identity(
                    "FloatSemantics",
                    &name,
                    &parameters,
                    result,
                )
                .unwrap();
                let contract = operation.contract_identity();
                let mut writer = Writer::default();
                ApplicationContract::write(&mut writer, &contract).unwrap();
                let bytes = writer.finish();
                let decoded = ApplicationContract::read(&mut Reader::new(&bytes)).unwrap();
                assert_eq!(decoded, contract);
                assert_eq!(
                    FloatSemanticOperation::for_contract_identity(&decoded),
                    Some(operation)
                );
            }
        }
    }

    #[test]
    fn width_parameterized_float_contracts_reject_invalid_carriers() {
        for bytes in [
            vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![2, 2, 13, 0, 0, 0, 0, 0, 0, 0],
            vec![3, 0, 13],
            vec![255],
        ] {
            assert!(SemanticOperation::read(&mut Reader::new(&bytes)).is_err());
        }
    }
}

pub struct Format;
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
        operation: SemanticOperation,
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
