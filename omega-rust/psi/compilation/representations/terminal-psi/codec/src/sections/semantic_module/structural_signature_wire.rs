//! Canonical structural signatures and boundary declaration wire format.
//!
//! This module owns exact structural parameter rows, published service
//! ceilings, and boundary-machine declaration envelopes. Operation bodies and
//! provider refinement semantics remain outside this module.

use semantic_vocabulary::{
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression, ContentProjectionScalar,
    QualifiedScalarType, ServiceId,
};
use terminal_psi::{
    BoundaryCompletion, BoundaryContentGuarantee, BoundaryDeclarationRole,
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    BoundaryReferenceOutput, BoundaryRequirementKind, BoundaryStructuralResultDeclaration,
    CheckedBoundaryCallable, CheckedBoundaryExecution, ExternalCompletionKind,
    ProgramLocalRootIntroductionSchema, RetainedBorrowContentProjection, RetainedBorrowCustody,
    RetainedBorrowPlace, RetainedBorrowPlaceRoot, ScalarDomainRequirement, StructuralAccess,
    StructuralDomainRequirement, StructuralParameterDeclaration, StructuralPathQualification,
};

use super::CodecError;
use super::wire::{Reader, Writer};
use super::wire_fields::{
    Access, ContentAlgebra, ContentExpression, ContentProjection, CrashRoutes, Field, Flag, Id,
    Ids, Index, List, MachineDeclaration, Multiplicity, Nominal, OptionalId, PathQualification,
    ProjectionIdentity, Proposition, QualificationPath, QualificationSet, ReachArgument, Scalar,
    Text, U32, U64, wire_enum, wire_record,
};
use crate::sections::semantic_module::wire::decode_counted;

mod reference_outputs;

wire_record! {
    BoundaryMachine: BoundaryMachineDeclaration {
        id: Id,
        declaration: OptionalCallable,
        numeric_intrinsic: NumericIntrinsic,
        identity: Index,
        owner: Nominal,
        role: Role,
        kind: RequirementKind,
        completion: Completion,
        unconditional_termination: Flag,
        attachment: OptionalId,
        parameter_order: List<ParameterKind>,
        scalar_parameters: List<Scalar>,
        crash_routes: CrashRoutes,
        structural_parameters: List<StructuralParameter>,
        result: CallResult,
        requires: List<DomainRequirement>,
        normal_qualifications: List<DomainRequirement>,
        scalar_domain_requirements: List<ScalarDomain>,
        scalar_requires: List<Proposition>,
        scalar_ensures: List<Proposition>,
        reference_outputs: ReferenceOutputs,
        program_local_root_introductions: List<RootIntroduction>,
        content_guarantees: List<ContentGuarantee>,
        fixed_service_reach: Ids,
        owning_service: OptionalId,
        published_service_ceiling: Ids,
    }
    Callable: CheckedBoundaryCallable {
        source: MachineDeclaration,
        method_arguments: List<ReachArgument>,
        execution: Execution,
    }
    DomainRequirement: StructuralDomainRequirement {
        argument_index: U32,
        path: QualificationPath,
        domain: Id,
    }
    ScalarDomain: ScalarDomainRequirement { argument_index: U32, domain: Id }
    RootIntroduction: ProgramLocalRootIntroductionSchema {
        argument_index: U32,
        source_parameter_position: U32,
        qualification: Id,
        carrier: Id,
        projection: ProjectionIdentity,
        algebra: ContentAlgebra,
        capacity: ContentExpression,
        compatibility_report_identity: U64,
    }
    StructuralParameter: StructuralParameterDeclaration {
        place: Id,
        position: U32,
        structural_type: Id,
        multiplicity: Multiplicity,
        access: Access,
        qualifications: Ids,
        projected_qualifications: List<PathQualification>,
    }
    BorrowPlace: RetainedBorrowPlace {
        version: PlaceVersion,
        root: PlaceRoot,
        segments: List<PlaceSegment>,
    }
    BorrowProjection: RetainedBorrowContentProjection {
        semantic_domain: Id,
        carrier_identity: Index,
        projection: ContentProjection,
    }
    BorrowCustody: RetainedBorrowCustody {
        callable_identity: Index,
        source: BorrowPlace,
        result: BorrowPlace,
        access: Access,
        callable_lifetime_parameter_count: U32,
        callable_lifetime_parameter_ordinal: U32,
        result_nominal_identity: Index,
        result_multiplicity: Multiplicity,
        result_lifetime_argument_count: U32,
        result_lifetime_argument_ordinal: U32,
        result_lifetime_slot_is_erased: Flag,
        retained_semantic_domain: Id,
        source_projection: BorrowProjection,
        result_projection: BorrowProjection,
    }
    StructuralResult: BoundaryStructuralResultDeclaration {
        structural_type: Id,
        multiplicity: Multiplicity,
        qualifications: Ids,
        projected_qualifications: List<PathQualification>,
    }
    QualifiedScalar: QualifiedScalarType { scalar_type: Scalar, qualifications: QualificationSet }
}

wire_enum! {
    Execution: CheckedBoundaryExecution, "checked boundary execution" {
        0 => Provider,
        1 => AccessInitialize,
        2 => AccessMoveOut,
        3 => AccessView,
    }
    Role: BoundaryDeclarationRole, "BoundaryDeclarationRole" { 0 => Callable, 1 => ContractEvidence }
    RequirementKind: BoundaryRequirementKind, "BoundaryRequirementKind" {
        0 => Machine,
        1 => Trait,
        2 => Operator,
    }
    ParameterKind: BoundaryParameterKind, "BoundaryParameterKind" { 0 => Scalar, 1 => Structural }
    ContentGuarantee: BoundaryContentGuarantee, "BoundaryContentGuarantee" {
        1 => Conservation(guarantee: super::content_wire::Guarantee),
        2 => RetainedBorrow(custody: BorrowCustody),
    }
    PlaceVersion: ContentPlaceVersion, "ContentPlaceVersion" { 1 => Entry, 2 => Current }
    PlaceRoot: RetainedBorrowPlaceRoot, "RetainedBorrowPlaceRoot" {
        1 => Parameter { position: U32, identity: Index },
        2 => Result,
    }
    PlaceSegment: ContentPlaceSegment, "ContentPlaceSegment" {
        1 => Case(identity: Text),
        2 => Field(identity: Text),
        3 => FixedIndex(index: U64),
    }
    CallResult: BoundaryMachineResult, "BoundaryMachineResult" {
        0 => Unit,
        1 => Scalar(result: QualifiedScalar),
        2 => Structural(result: StructuralResult),
    }
}

/// The checked callable, present behind a flag.
struct OptionalCallable;
impl Field<Option<CheckedBoundaryCallable>> for OptionalCallable {
    fn write(
        writer: &mut Writer,
        value: &Option<CheckedBoundaryCallable>,
    ) -> Result<(), CodecError> {
        writer.boolean(value.is_some());
        if let Some(callable) = value {
            Callable::write(writer, callable)?;
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<CheckedBoundaryCallable>, CodecError> {
        Ok(if reader.boolean()? {
            Some(Callable::read(reader)?)
        } else {
            None
        })
    }
}

/// A numeric intrinsic meaning in its four-byte canonical encoding.
struct NumericIntrinsic;
impl Field<Option<language_semantics::numeric_intrinsics::NumericIntrinsic>> for NumericIntrinsic {
    fn write(
        writer: &mut Writer,
        value: &Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
    ) -> Result<(), CodecError> {
        writer.boolean(value.is_some());
        if let Some(meaning) = value {
            let Some(bytes) = meaning.canonical_encoding() else {
                return crate::codec_error::malformed("unsupported numeric intrinsic meaning");
            };
            writer.bytes(&bytes);
        }
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<Option<language_semantics::numeric_intrinsics::NumericIntrinsic>, CodecError> {
        if !reader.boolean()? {
            return Ok(None);
        }
        let bytes = [reader.u8()?, reader.u8()?, reader.u8()?, reader.u8()?];
        language_semantics::numeric_intrinsics::NumericIntrinsic::from_canonical_encoding(bytes)
            .map(Some)
            .ok_or_else(|| {
                CodecError::MalformedStructuralFoundation(
                    "invalid numeric intrinsic meaning".into(),
                )
            })
    }
}

/// Normal return, or the one external completion the wire admits.
struct Completion;
impl Field<BoundaryCompletion> for Completion {
    fn write(writer: &mut Writer, value: &BoundaryCompletion) -> Result<(), CodecError> {
        writer.u8(match value {
            BoundaryCompletion::NormalReturn => 0,
            BoundaryCompletion::External(ExternalCompletionKind::ProcessExit) => 1,
        });
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<BoundaryCompletion, CodecError> {
        match reader.u8()? {
            0 => Ok(BoundaryCompletion::NormalReturn),
            1 => Ok(BoundaryCompletion::External(
                ExternalCompletionKind::ProcessExit,
            )),
            tag => Err(CodecError::InvalidTag("BoundaryCompletion", tag)),
        }
    }
}

struct ReferenceOutputs;
impl Field<Vec<BoundaryReferenceOutput>> for ReferenceOutputs {
    fn write(writer: &mut Writer, value: &Vec<BoundaryReferenceOutput>) -> Result<(), CodecError> {
        reference_outputs::encode(writer, value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<BoundaryReferenceOutput>, CodecError> {
        reference_outputs::decode(reader)
    }
}

pub(crate) fn encode_structural_parameters(
    writer: &mut Writer,
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    writer.counted(
        "structural parameters",
        parameters,
        StructuralParameter::write,
    )
}

pub(crate) fn decode_structural_parameters(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralParameterDeclaration>, CodecError> {
    List::<StructuralParameter>::read(reader)
}

pub(crate) fn encode_service_ceiling(
    writer: &mut Writer,
    services: &[ServiceId],
) -> Result<(), CodecError> {
    writer.ids("published service ceiling", services)
}

pub(crate) fn encode_projected_qualifications(
    writer: &mut Writer,
    qualifications: &[StructuralPathQualification],
) -> Result<(), CodecError> {
    writer.counted(
        "projected structural parameter qualifications",
        qualifications,
        PathQualification::write,
    )
}

pub(crate) fn decode_projected_qualifications(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathQualification>, CodecError> {
    List::<PathQualification>::read(reader)
}

/// Shared place-free result signature for boundary declarations and dynamic slots.
pub(super) fn encode_call_result(
    writer: &mut Writer,
    result: &BoundaryMachineResult,
) -> Result<(), CodecError> {
    CallResult::write(writer, result)
}

pub(super) fn decode_call_result(
    reader: &mut Reader<'_>,
) -> Result<BoundaryMachineResult, CodecError> {
    CallResult::read(reader)
}

pub(crate) fn encode_content_projection_expression(
    writer: &mut Writer,
    capacity: &ContentProjectionExpression,
) -> Result<(), CodecError> {
    match capacity {
        ContentProjectionExpression::IntervalSet(members) => {
            writer.u8(1);
            writer.len("program-local interval members", members.len())?;
            for (start, end) in members {
                encode_capacity_scalar(writer, start)?;
                encode_capacity_scalar(writer, end)?;
            }
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            writer.u8(2);
            encode_capacity_scalar(writer, magnitude)?;
        }
    }
    Ok(())
}

fn encode_capacity_scalar(
    writer: &mut Writer,
    scalar: &ContentProjectionScalar,
) -> Result<(), CodecError> {
    match scalar {
        ContentProjectionScalar::SubjectField(path)
        | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
            writer.u8(
                if matches!(scalar, ContentProjectionScalar::SubjectField(_)) {
                    1
                } else {
                    2
                },
            );
            writer.strings("program-local capacity field path", path)?;
        }
        ContentProjectionScalar::Natural(value) => {
            writer.u8(3);
            writer.string("program-local natural", value)?;
        }
        ContentProjectionScalar::Successor(inner) => {
            writer.u8(4);
            encode_capacity_scalar(writer, inner)?;
        }
        ContentProjectionScalar::Add(left, right)
        | ContentProjectionScalar::Subtract(left, right)
        | ContentProjectionScalar::Multiply(left, right) => {
            writer.u8(match scalar {
                ContentProjectionScalar::Add(_, _) => 5,
                ContentProjectionScalar::Subtract(_, _) => 6,
                ContentProjectionScalar::Multiply(_, _) => 7,
                _ => unreachable!(),
            });
            encode_capacity_scalar(writer, left)?;
            encode_capacity_scalar(writer, right)?;
        }
    }
    Ok(())
}

pub(crate) fn decode_content_projection_expression(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ContentProjectionExpression, CodecError> {
    if depth > 256 {
        return Err(CodecError::InvalidTag(
            "ContentProjectionExpressionDepth",
            0,
        ));
    }
    match reader.u8()? {
        1 => Ok(ContentProjectionExpression::IntervalSet(decode_counted(
            reader,
            |reader| {
                Ok((
                    decode_capacity_scalar(reader, depth + 1)?,
                    decode_capacity_scalar(reader, depth + 1)?,
                ))
            },
        )?)),
        2 => Ok(ContentProjectionExpression::CountedQuantity(
            decode_capacity_scalar(reader, depth + 1)?,
        )),
        tag => Err(CodecError::InvalidTag("ContentProjectionExpression", tag)),
    }
}

fn decode_capacity_scalar(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ContentProjectionScalar, CodecError> {
    if depth > 256 {
        return Err(CodecError::InvalidTag("ContentProjectionScalarDepth", 0));
    }
    match reader.u8()? {
        1 => Ok(ContentProjectionScalar::SubjectField(
            reader.strings("program-local capacity field path")?,
        )),
        2 => Ok(ContentProjectionScalar::RuntimeScalarEmbedding(
            reader.strings("program-local capacity field path")?,
        )),
        3 => Ok(ContentProjectionScalar::Natural(
            reader.string("program-local natural")?,
        )),
        4 => Ok(ContentProjectionScalar::Successor(Box::new(
            decode_capacity_scalar(reader, depth + 1)?,
        ))),
        5 => Ok(ContentProjectionScalar::Add(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        6 => Ok(ContentProjectionScalar::Subtract(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        7 => Ok(ContentProjectionScalar::Multiply(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        tag => Err(CodecError::InvalidTag("ContentProjectionScalar", tag)),
    }
}

pub(crate) fn encode_structural_access(writer: &mut Writer, access: StructuralAccess) {
    writer.u8(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

pub(crate) fn decode_structural_access(
    reader: &mut Reader<'_>,
) -> Result<StructuralAccess, CodecError> {
    decode_structural_access_tag(reader.u8()?)
}

fn decode_structural_access_tag(tag: u8) -> Result<StructuralAccess, CodecError> {
    match tag {
        1 => Ok(StructuralAccess::Owned),
        2 => Ok(StructuralAccess::SharedBorrow),
        3 => Ok(StructuralAccess::MutableBorrow),
        4 => Ok(StructuralAccess::WriteOnlyBorrow),
        tag => Err(CodecError::InvalidTag("StructuralAccess", tag)),
    }
}

/// Decode the optional access byte of a standalone view type: zero is the
/// absent access a declared view slot authors; nonzero is exactly the table
/// `decode_structural_access` states.
pub(crate) fn decode_optional_structural_access(
    reader: &mut Reader<'_>,
) -> Result<Option<StructuralAccess>, CodecError> {
    match reader.u8()? {
        0 => Ok(None),
        tag => Ok(Some(decode_structural_access_tag(tag)?)),
    }
}
