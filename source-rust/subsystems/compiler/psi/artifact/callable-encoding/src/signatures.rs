//! Parameters, results and static dispatch facts in a checked call signature.
use crate::{
    CodecError,
    fields::*,
    wire::{Reader, Writer},
};
use psi_declaration_encoding::access::mode::{decode_structural_access, encode_structural_access};
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryParameterKind, DeclaredResultLifetime, EntryClaim,
    OutcomeSpecificCallArgumentSubstitution, StaticRequirementDispatch,
    StructuralParameterDeclaration, StructuralPathSegment, TerminalMachineResult, ValueDeclaration,
};

wire_record! {
    Declaration: ValueDeclaration { id: Id, scalar_type: Scalar, qualifications: QualificationSet }
    Claim: EntryClaim { claim: Id, input: Id, path: Path }
    StructuralParameter: StructuralParameterDeclaration {
        place: Id,
        position: U32,
        structural_type: Id,
        multiplicity: Multiplicity,
        access: Access,
        qualifications: Ids,
        projected_qualifications: List<PathQualification>,
        additional_input_qualifications: List<PathQualification>,
    }
    ArgumentSubstitution: OutcomeSpecificCallArgumentSubstitution {
        argument_position: U32,
        parameter_position: U32,
        structural_path: Optional<Path>,
    }
    Dispatch: StaticRequirementDispatch {
        conformance_application_report_fingerprint: U64,
        conformance_application_commitment: Commitment,
        public_requirement_identity: Index,
        declaring_trait_identity: Index,
        requirement_identity: Index,
        realization_identity: Index,
        realization_callable_identity: Index,
        realization: Id,
    }
    ResultLifetime: DeclaredResultLifetime {
        inputs: List<DeclaredLifetimeDependency>,
        results: List<Path<DECLARED_LIFETIME_FAMILY>>,
    }
}
wire_enum! {
    MachineParameterKind: BoundaryParameterKind, "machine parameter kind" { 0 => Scalar, 1 => Structural }
    ParameterKind: BoundaryParameterKind, "BoundaryParameterKind" { 0 => Scalar, 1 => Structural }
    MachineResult: TerminalMachineResult, "TerminalMachineResult" {
        0 => Unit,
        1 => Scalar(declaration: Declaration),
        2 => Structural(result: FunctionResult),
    }
}
pub struct Commitment;
impl Field<terminal_psi::ClosedConformanceApplicationCommitment> for Commitment {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::ClosedConformanceApplicationCommitment,
    ) -> Result<(), CodecError> {
        writer.bytes(&value.as_bytes());
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<terminal_psi::ClosedConformanceApplicationCommitment, CodecError> {
        Ok(terminal_psi::ClosedConformanceApplicationCommitment::from_digest(reader.array()?))
    }
}

pub fn encode_dependency(
    writer: &mut Writer,
    dependency: &BoundaryLifetimeDependency,
    write_path: impl FnOnce(&mut Writer, &[StructuralPathSegment]) -> Result<(), CodecError>,
) -> Result<(), CodecError> {
    match dependency {
        BoundaryLifetimeDependency::Referent {
            argument_index,
            access,
        } => {
            writer.u8(0);
            writer.u32(*argument_index);
            encode_structural_access(writer, *access);
        }
        BoundaryLifetimeDependency::Stored {
            argument_index,
            path,
            access,
        } => {
            writer.u8(1);
            writer.u32(*argument_index);
            encode_structural_access(writer, *access);
            write_path(writer, path)?;
        }
    }
    Ok(())
}

pub fn decode_dependency(
    reader: &mut Reader<'_>,
    read_path: impl FnOnce(&mut Reader<'_>) -> Result<Vec<StructuralPathSegment>, CodecError>,
) -> Result<BoundaryLifetimeDependency, CodecError> {
    match reader.u8()? {
        0 => Ok(BoundaryLifetimeDependency::Referent {
            argument_index: reader.u32()?,
            access: decode_structural_access(reader)?,
        }),
        1 => Ok(BoundaryLifetimeDependency::Stored {
            argument_index: reader.u32()?,
            access: decode_structural_access(reader)?,
            path: read_path(reader)?,
        }),
        tag => Err(CodecError::InvalidTag("BoundaryLifetimeDependency", tag)),
    }
}
pub struct DeclaredLifetimeDependency;
impl Field<terminal_psi::BoundaryLifetimeDependency> for DeclaredLifetimeDependency {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::BoundaryLifetimeDependency,
    ) -> Result<(), CodecError> {
        encode_dependency(
            writer,
            value,
            psi_declaration_encoding::places::encode_declared_lifetime_path,
        )
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<terminal_psi::BoundaryLifetimeDependency, CodecError> {
        decode_dependency(
            reader,
            psi_declaration_encoding::places::decode_declared_lifetime_path,
        )
    }
}

delegated_fields! {
    FunctionResult: terminal_psi::StructuralResultDeclaration
        => crate::results::encode_function_result, crate::results::decode_function_result;
}
