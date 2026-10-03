//! Standalone canonical bytes for the first bounded D39 profile instance.

use terminal_psi::{
    CrashCause, StructuralAccess, StructuralMultiplicity, TerminalModule,
    TerminalObservationSchema, TerminalOperationCrashDenotation, TerminalTraceBoundaryCrashSiteRow,
    TerminalTraceCrashSiteRow, TerminalTraceOperationCrashSiteRow, TerminalTraceOrdinaryEventKind,
    TerminalTraceOrdinaryEventRow, TerminalTraceResultSchema, TerminalTraceRootRow,
    TerminalTraceScalarSchema, TerminalTraceStructuralSchema, TerminalTraceV1Profile,
    TerminalTraceValueComparison, TrappingIntegerPrimitive, VocabularyMarker,
};
use terminal_verifier::{
    TerminalTraceV1ReconstructionError,
    reconstruct_terminal_trace_v1_rows as reconstruct_verified_rows,
};

use crate::sections::semantic_module::canonical_order::{
    crash_routes_are_canonical, validate_crash_route_predicates,
};
use crate::sections::semantic_module::contract_wire::{
    decode_crash_route_bucket, encode_crash_route_bucket,
};
use crate::sections::semantic_module::scalar_wire::{
    decode_ieee_float_format, decode_integer_type, decode_scalar_type, encode_ieee_float_format,
    encode_integer_type, encode_scalar_type,
};
use crate::sections::semantic_module::structural_signature_wire::{
    decode_projected_qualifications, encode_projected_qualifications,
};
use crate::sections::semantic_module::wire::{Reader, Writer};
use crate::{CodecError, terminal_psi_identity};

// Revision 2 appended the operation-crash-site group. The domain names the
// revision so a revision-1 consumer rejects the bytes at the domain tag
// rather than misreading the later groups.
const DOMAIN: &[u8] = b"omega.terminal.observation-profile.v2";
const ROOT_ROW_TAG: u8 = 1;
const CRASH_ROW_TAG: u8 = 2;
const ORDINARY_EVENT_ROW_TAG: u8 = 3;
const BOUNDARY_CRASH_ROW_TAG: u8 = 4;
const OPERATION_CRASH_ROW_TAG: u8 = 5;
const BOUNDARY_CALL_EVENT_TAG: u8 = 1;
const PORT_WRITE_EVENT_TAG: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTraceV1ProfileCodecError {
    Wire(CodecError),
    InvalidDomain,
    UnsupportedSchemaVersion(u16),
    UnsupportedVocabularyMarker(u16),
    ZeroModuleCommitment,
    InvalidRootCount(u32),
    InvalidRowTag { group: &'static str, tag: u8 },
    InvalidComparison(u8),
    InvalidStructuralMultiplicity(u8),
    InvalidStructuralAccess(u8),
    InvalidResultTag(u8),
    InvalidCrashCause(u8),
    InvalidTrappingPrimitive(u8),
    InvalidOperationCrashDenotation,
    InvalidOrdinaryEventKind(u8),
    NonCanonicalStructuralQualifications,
    NonCanonicalCrashSiteOrder,
    NonCanonicalBoundaryCrashSiteOrder,
    NonCanonicalOperationCrashSiteOrder,
    NonCanonicalOrdinaryEventOrder,
    UnsupportedExternalTerminationRows(u32),
    TrailingBytes(usize),
    NonCanonicalEncoding,
}

impl std::fmt::Display for TerminalTraceV1ProfileCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalTraceV1ProfileCodecError {}

impl From<CodecError> for TerminalTraceV1ProfileCodecError {
    fn from(error: CodecError) -> Self {
        Self::Wire(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTraceV1ProfileBuildError {
    InvalidCanonicalModule(CodecError),
    Reconstruction(TerminalTraceV1ReconstructionError),
}

impl std::fmt::Display for TerminalTraceV1ProfileBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalTraceV1ProfileBuildError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTraceV1ProfileAcceptanceError {
    InvalidProfileBytes(TerminalTraceV1ProfileCodecError),
    InvalidCanonicalModule(TerminalTraceV1ProfileBuildError),
    ProfileMismatch,
}

impl std::fmt::Display for TerminalTraceV1ProfileAcceptanceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalTraceV1ProfileAcceptanceError {}

/// The canonical consumer entrypoint. The codec computes the exact module
/// commitment, while the verifier independently derives every profile row.
pub fn reconstruct_canonical_terminal_trace_v1_profile(
    module: &TerminalModule,
) -> Result<TerminalTraceV1Profile, TerminalTraceV1ProfileBuildError> {
    let module_identity = terminal_psi_identity(module)
        .map_err(TerminalTraceV1ProfileBuildError::InvalidCanonicalModule)?;
    let rows = reconstruct_verified_rows(module)
        .map_err(TerminalTraceV1ProfileBuildError::Reconstruction)?;
    Ok(TerminalTraceV1Profile {
        schema: TerminalObservationSchema::TerminalTraceV1,
        module_identity,
        root: rows.root,
        crash_sites: rows.crash_sites,
        boundary_crash_sites: rows.boundary_crash_sites,
        operation_crash_sites: rows.operation_crash_sites,
        ordinary_events: rows.ordinary_events,
    })
}

/// Canonical module-bound acceptance. Standalone decoding establishes only a
/// well-formed profile value; this gate rejects every stale, substituted,
/// missing, extra, or redirected row by exact comparison with verifier-owned
/// reconstruction for the canonically validated module.
pub fn accept_terminal_trace_v1_profile(
    module: &TerminalModule,
    profile_bytes: &[u8],
) -> Result<TerminalTraceV1Profile, TerminalTraceV1ProfileAcceptanceError> {
    let supplied = decode_terminal_trace_v1_profile(profile_bytes)
        .map_err(TerminalTraceV1ProfileAcceptanceError::InvalidProfileBytes)?;
    let expected = reconstruct_canonical_terminal_trace_v1_profile(module)
        .map_err(TerminalTraceV1ProfileAcceptanceError::InvalidCanonicalModule)?;
    if supplied != expected {
        return Err(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch);
    }
    Ok(supplied)
}

pub fn encode_terminal_trace_v1_profile(
    profile: &TerminalTraceV1Profile,
) -> Result<Vec<u8>, TerminalTraceV1ProfileCodecError> {
    validate_profile(profile)?;
    encode_raw(profile)
}

pub fn decode_terminal_trace_v1_profile(
    bytes: &[u8],
) -> Result<TerminalTraceV1Profile, TerminalTraceV1ProfileCodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(DOMAIN.len())? != DOMAIN {
        return Err(TerminalTraceV1ProfileCodecError::InvalidDomain);
    }
    let version = reader.u16()?;
    let schema = TerminalObservationSchema::from_version(version).ok_or(
        TerminalTraceV1ProfileCodecError::UnsupportedSchemaVersion(version),
    )?;
    let vocabulary_raw = reader.u16()?;
    let vocabulary_marker = VocabularyMarker::new(vocabulary_raw)
        .ok_or(TerminalTraceV1ProfileCodecError::UnsupportedVocabularyMarker(vocabulary_raw))?;
    let module_commitment = terminal_psi::SemanticFingerprint::from_bytes(reader.array()?);
    if module_commitment.as_bytes() == &[0; 32] {
        return Err(TerminalTraceV1ProfileCodecError::ZeroModuleCommitment);
    }

    let root_count = reader.count()?;
    if root_count != 1 {
        return Err(TerminalTraceV1ProfileCodecError::InvalidRootCount(
            root_count,
        ));
    }
    require_row_tag(&mut reader, "root", ROOT_ROW_TAG)?;
    let root = decode_root(&mut reader)?;

    let crash_count = reader.count()?;
    let mut crash_sites = Vec::with_capacity(crash_count as usize);
    for _ in 0..crash_count {
        require_row_tag(&mut reader, "crash", CRASH_ROW_TAG)?;
        crash_sites.push(TerminalTraceCrashSiteRow {
            machine: reader.id("trace crash machine")?,
            block: reader.id("trace crash block")?,
            edge: reader.id("trace crash edge")?,
            cause: decode_crash_cause(&mut reader)?,
        });
    }

    let boundary_crash_count = reader.count()?;
    let mut boundary_crash_sites = Vec::with_capacity(boundary_crash_count as usize);
    for _ in 0..boundary_crash_count {
        require_row_tag(&mut reader, "boundary crash", BOUNDARY_CRASH_ROW_TAG)?;
        boundary_crash_sites.push(TerminalTraceBoundaryCrashSiteRow {
            machine: reader.id("trace boundary crash machine")?,
            block: reader.id("trace boundary crash block")?,
            operation: reader.id("trace boundary crash operation")?,
            boundary: reader.id("trace boundary crash boundary")?,
            boundary_identity: reader.string("trace boundary crash identity")?,
            route: decode_crash_route_bucket(&mut reader)?,
        });
    }

    let operation_crash_count = reader.count()?;
    let mut operation_crash_sites = Vec::with_capacity(operation_crash_count as usize);
    for _ in 0..operation_crash_count {
        require_row_tag(&mut reader, "operation crash", OPERATION_CRASH_ROW_TAG)?;
        operation_crash_sites.push(TerminalTraceOperationCrashSiteRow {
            machine: reader.id("trace operation crash machine")?,
            block: reader.id("trace operation crash block")?,
            operation: reader.id("trace operation crash operation")?,
            cause: decode_crash_cause(&mut reader)?,
            denotation: decode_operation_crash_denotation(&mut reader)?,
        });
    }

    let ordinary_count = reader.count()?;
    let mut ordinary_events = Vec::with_capacity(ordinary_count as usize);
    for _ in 0..ordinary_count {
        require_row_tag(&mut reader, "ordinary event", ORDINARY_EVENT_ROW_TAG)?;
        ordinary_events.push(decode_ordinary_event(&mut reader)?);
    }
    let external_termination_count = reader.count()?;
    if external_termination_count != 0 {
        return Err(
            TerminalTraceV1ProfileCodecError::UnsupportedExternalTerminationRows(
                external_termination_count,
            ),
        );
    }
    if reader.remaining() != 0 {
        return Err(TerminalTraceV1ProfileCodecError::TrailingBytes(
            reader.remaining(),
        ));
    }

    let profile = TerminalTraceV1Profile {
        schema,
        module_identity: terminal_psi::TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: module_commitment,
        },
        root,
        crash_sites,
        boundary_crash_sites,
        operation_crash_sites,
        ordinary_events,
    };
    validate_profile(&profile)?;
    if encode_raw(&profile)? != bytes {
        return Err(TerminalTraceV1ProfileCodecError::NonCanonicalEncoding);
    }
    Ok(profile)
}

fn encode_raw(
    profile: &TerminalTraceV1Profile,
) -> Result<Vec<u8>, TerminalTraceV1ProfileCodecError> {
    let mut writer = Writer::default();
    writer.bytes(DOMAIN);
    writer.u16(profile.schema.version());
    writer.u16(profile.module_identity.vocabulary_marker.get());
    writer.bytes(profile.module_identity.program_fingerprint.as_bytes());

    writer.u32(1);
    writer.u8(ROOT_ROW_TAG);
    encode_root(&mut writer, &profile.root)?;

    writer.len("trace crash sites", profile.crash_sites.len())?;
    for crash in &profile.crash_sites {
        writer.u8(CRASH_ROW_TAG);
        writer.id(crash.machine);
        writer.id(crash.block);
        writer.id(crash.edge);
        writer.u8(match crash.cause {
            CrashCause::Trap => 1,
            CrashCause::Abort => 2,
        });
    }

    // A boundary crash is observed at its call operation, never on a fabricated
    // terminator edge, so these rows form their own group with operation-level
    // identity and the declaration's exact route bucket.
    writer.len(
        "trace boundary crash sites",
        profile.boundary_crash_sites.len(),
    )?;
    for crash in &profile.boundary_crash_sites {
        writer.u8(BOUNDARY_CRASH_ROW_TAG);
        writer.id(crash.machine);
        writer.id(crash.block);
        writer.id(crash.operation);
        writer.id(crash.boundary);
        writer.string("trace boundary crash identity", &crash.boundary_identity)?;
        encode_crash_route_bucket(&mut writer, &crash.route)?;
    }

    // A Trapping primitive's crash is observed at the operation itself: the
    // row carries its exact cause and primitive denotation, never an edge or
    // a boundary identity.
    writer.len(
        "trace operation crash sites",
        profile.operation_crash_sites.len(),
    )?;
    for crash in &profile.operation_crash_sites {
        writer.u8(OPERATION_CRASH_ROW_TAG);
        writer.id(crash.machine);
        writer.id(crash.block);
        writer.id(crash.operation);
        writer.u8(match crash.cause {
            CrashCause::Trap => 1,
            CrashCause::Abort => 2,
        });
        encode_operation_crash_denotation(&mut writer, crash.denotation);
    }

    writer.len("trace ordinary events", profile.ordinary_events.len())?;
    for event in &profile.ordinary_events {
        writer.u8(ORDINARY_EVENT_ROW_TAG);
        encode_ordinary_event(&mut writer, event)?;
    }

    // D39 fixes this later group and its order. This bounded rung encodes its
    // explicit empty count rather than omitting it.
    writer.u32(0);
    Ok(writer.finish())
}

fn encode_root(
    writer: &mut Writer,
    root: &TerminalTraceRootRow,
) -> Result<(), TerminalTraceV1ProfileCodecError> {
    writer.id(root.entry);
    writer.len("trace scalar inputs", root.scalar_inputs.len())?;
    for schema in &root.scalar_inputs {
        encode_scalar_schema(writer, *schema);
    }
    writer.len("trace structural inputs", root.structural_inputs.len())?;
    for schema in &root.structural_inputs {
        encode_structural_schema(writer, schema)?;
    }
    encode_result_schema(writer, &root.result)
}

fn encode_result_schema(
    writer: &mut Writer,
    result: &TerminalTraceResultSchema,
) -> Result<(), TerminalTraceV1ProfileCodecError> {
    match result {
        TerminalTraceResultSchema::Unit => writer.u8(1),
        TerminalTraceResultSchema::Scalar(schema) => {
            writer.u8(2);
            encode_scalar_schema(writer, *schema);
        }
        TerminalTraceResultSchema::Structural(schema) => {
            writer.u8(3);
            encode_structural_schema(writer, schema)?;
        }
    }
    Ok(())
}

fn decode_root(
    reader: &mut Reader<'_>,
) -> Result<TerminalTraceRootRow, TerminalTraceV1ProfileCodecError> {
    let entry = reader.id("trace root entry")?;
    let scalar_count = reader.count()?;
    let scalar_inputs = (0..scalar_count)
        .map(|_| decode_scalar_schema(reader))
        .collect::<Result<_, _>>()?;
    let structural_count = reader.count()?;
    let structural_inputs = (0..structural_count)
        .map(|_| decode_structural_schema(reader))
        .collect::<Result<_, _>>()?;
    let result = decode_result_schema(reader)?;
    Ok(TerminalTraceRootRow {
        entry,
        scalar_inputs,
        structural_inputs,
        result,
    })
}

fn decode_result_schema(
    reader: &mut Reader<'_>,
) -> Result<TerminalTraceResultSchema, TerminalTraceV1ProfileCodecError> {
    Ok(match reader.u8()? {
        1 => TerminalTraceResultSchema::Unit,
        2 => TerminalTraceResultSchema::Scalar(decode_scalar_schema(reader)?),
        3 => TerminalTraceResultSchema::Structural(decode_structural_schema(reader)?),
        tag => return Err(TerminalTraceV1ProfileCodecError::InvalidResultTag(tag)),
    })
}

fn encode_ordinary_event(
    writer: &mut Writer,
    event: &TerminalTraceOrdinaryEventRow,
) -> Result<(), TerminalTraceV1ProfileCodecError> {
    writer.id(event.machine);
    writer.id(event.block);
    writer.id(event.operation);
    match &event.kind {
        TerminalTraceOrdinaryEventKind::BoundaryCall {
            boundary,
            boundary_identity,
        } => {
            writer.u8(BOUNDARY_CALL_EVENT_TAG);
            writer.id(*boundary);
            writer.string("trace boundary identity", boundary_identity)?;
        }
        TerminalTraceOrdinaryEventKind::PortWrite {
            service,
            service_identity,
        } => {
            writer.u8(PORT_WRITE_EVENT_TAG);
            writer.id(*service);
            writer.string("trace service identity", service_identity)?;
        }
    }
    writer.len("trace event scalar arguments", event.scalar_arguments.len())?;
    for schema in &event.scalar_arguments {
        encode_scalar_schema(writer, *schema);
    }
    writer.len(
        "trace event structural arguments",
        event.structural_arguments.len(),
    )?;
    for schema in &event.structural_arguments {
        encode_structural_schema(writer, schema)?;
    }
    encode_result_schema(writer, &event.result)
}

fn decode_ordinary_event(
    reader: &mut Reader<'_>,
) -> Result<TerminalTraceOrdinaryEventRow, TerminalTraceV1ProfileCodecError> {
    let machine = reader.id("trace event machine")?;
    let block = reader.id("trace event block")?;
    let operation = reader.id("trace event operation")?;
    let kind = match reader.u8()? {
        BOUNDARY_CALL_EVENT_TAG => TerminalTraceOrdinaryEventKind::BoundaryCall {
            boundary: reader.id("trace event boundary")?,
            boundary_identity: reader.string("trace boundary identity")?,
        },
        PORT_WRITE_EVENT_TAG => TerminalTraceOrdinaryEventKind::PortWrite {
            service: reader.id("trace event service")?,
            service_identity: reader.string("trace service identity")?,
        },
        tag => {
            return Err(TerminalTraceV1ProfileCodecError::InvalidOrdinaryEventKind(
                tag,
            ));
        }
    };
    let scalar_count = reader.count()?;
    let scalar_arguments = (0..scalar_count)
        .map(|_| decode_scalar_schema(reader))
        .collect::<Result<_, _>>()?;
    let structural_count = reader.count()?;
    let structural_arguments = (0..structural_count)
        .map(|_| decode_structural_schema(reader))
        .collect::<Result<_, _>>()?;
    let result = decode_result_schema(reader)?;
    Ok(TerminalTraceOrdinaryEventRow {
        machine,
        block,
        operation,
        kind,
        scalar_arguments,
        structural_arguments,
        result,
    })
}

fn encode_scalar_schema(writer: &mut Writer, schema: TerminalTraceScalarSchema) {
    encode_comparison(writer, schema.comparison);
    encode_scalar_type(writer, schema.scalar_type);
}

fn decode_scalar_schema(
    reader: &mut Reader<'_>,
) -> Result<TerminalTraceScalarSchema, TerminalTraceV1ProfileCodecError> {
    Ok(TerminalTraceScalarSchema {
        comparison: decode_comparison(reader)?,
        scalar_type: decode_scalar_type(reader)?,
    })
}

fn encode_structural_schema(
    writer: &mut Writer,
    schema: &TerminalTraceStructuralSchema,
) -> Result<(), TerminalTraceV1ProfileCodecError> {
    encode_comparison(writer, schema.comparison);
    writer.id(schema.structural_type);
    writer.u8(match schema.multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
    writer.u8(match schema.access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
    writer.len(
        "trace structural qualifications",
        schema.qualifications.len(),
    )?;
    for qualification in &schema.qualifications {
        writer.id(*qualification);
    }
    encode_projected_qualifications(writer, &schema.projected_qualifications)?;
    Ok(())
}

fn decode_structural_schema(
    reader: &mut Reader<'_>,
) -> Result<TerminalTraceStructuralSchema, TerminalTraceV1ProfileCodecError> {
    let comparison = decode_comparison(reader)?;
    let structural_type = reader.id("trace structural type")?;
    let multiplicity = match reader.u8()? {
        1 => StructuralMultiplicity::Unrestricted,
        2 => StructuralMultiplicity::Affine,
        3 => StructuralMultiplicity::Linear,
        tag => {
            return Err(TerminalTraceV1ProfileCodecError::InvalidStructuralMultiplicity(tag));
        }
    };
    let access = match reader.u8()? {
        1 => StructuralAccess::Owned,
        2 => StructuralAccess::SharedBorrow,
        3 => StructuralAccess::MutableBorrow,
        4 => StructuralAccess::WriteOnlyBorrow,
        tag => {
            return Err(TerminalTraceV1ProfileCodecError::InvalidStructuralAccess(
                tag,
            ));
        }
    };
    let qualification_count = reader.count()?;
    let qualifications = (0..qualification_count)
        .map(|_| {
            reader
                .id("trace structural qualification")
                .map_err(Into::into)
        })
        .collect::<Result<Vec<_>, TerminalTraceV1ProfileCodecError>>()?;
    let projected_qualifications = decode_projected_qualifications(reader)?;
    Ok(TerminalTraceStructuralSchema {
        structural_type,
        multiplicity,
        access,
        qualifications,
        projected_qualifications,
        comparison,
    })
}

fn encode_comparison(writer: &mut Writer, comparison: TerminalTraceValueComparison) {
    writer.u8(match comparison {
        TerminalTraceValueComparison::ExactSemanticValue => 1,
    });
}

fn decode_comparison(
    reader: &mut Reader<'_>,
) -> Result<TerminalTraceValueComparison, TerminalTraceV1ProfileCodecError> {
    match reader.u8()? {
        1 => Ok(TerminalTraceValueComparison::ExactSemanticValue),
        tag => Err(TerminalTraceV1ProfileCodecError::InvalidComparison(tag)),
    }
}

fn decode_crash_cause(
    reader: &mut Reader<'_>,
) -> Result<CrashCause, TerminalTraceV1ProfileCodecError> {
    match reader.u8()? {
        1 => Ok(CrashCause::Trap),
        2 => Ok(CrashCause::Abort),
        tag => Err(TerminalTraceV1ProfileCodecError::InvalidCrashCause(tag)),
    }
}

const fn encode_trapping_primitive(primitive: TrappingIntegerPrimitive) -> u8 {
    match primitive {
        TrappingIntegerPrimitive::Add => 1,
        TrappingIntegerPrimitive::Subtract => 2,
        TrappingIntegerPrimitive::Multiply => 3,
        TrappingIntegerPrimitive::Divide => 4,
        TrappingIntegerPrimitive::Remainder => 5,
        TrappingIntegerPrimitive::ShiftLeft => 6,
        TrappingIntegerPrimitive::ShiftRight => 7,
        TrappingIntegerPrimitive::Convert => 8,
    }
}

fn decode_trapping_primitive(
    tag: u8,
) -> Result<TrappingIntegerPrimitive, TerminalTraceV1ProfileCodecError> {
    Ok(match tag {
        1 => TrappingIntegerPrimitive::Add,
        2 => TrappingIntegerPrimitive::Subtract,
        3 => TrappingIntegerPrimitive::Multiply,
        4 => TrappingIntegerPrimitive::Divide,
        5 => TrappingIntegerPrimitive::Remainder,
        6 => TrappingIntegerPrimitive::ShiftLeft,
        7 => TrappingIntegerPrimitive::ShiftRight,
        8 => TrappingIntegerPrimitive::Convert,
        tag => {
            return Err(TerminalTraceV1ProfileCodecError::InvalidTrappingPrimitive(
                tag,
            ));
        }
    })
}

// Tags 1..8 retain the original integer denotation bytes. The new closed tag
// 9 carries only an IEEE format; an older decoder rejects it before consuming
// an integer carrier. No extra envelope changes existing integer rows.
fn encode_operation_crash_denotation(
    writer: &mut Writer,
    denotation: TerminalOperationCrashDenotation,
) {
    match denotation {
        TerminalOperationCrashDenotation::Integer {
            primitive,
            result_type,
            operand_type,
        } => {
            writer.u8(encode_trapping_primitive(primitive));
            encode_integer_type(writer, result_type);
            encode_integer_type(writer, operand_type);
        }
        TerminalOperationCrashDenotation::FloatNonFinite { format } => {
            writer.u8(9);
            encode_ieee_float_format(writer, format);
        }
    }
}

fn decode_operation_crash_denotation(
    reader: &mut Reader<'_>,
) -> Result<TerminalOperationCrashDenotation, TerminalTraceV1ProfileCodecError> {
    let tag = reader.u8()?;
    Ok(if tag == 9 {
        TerminalOperationCrashDenotation::FloatNonFinite {
            format: decode_ieee_float_format(reader)?,
        }
    } else {
        TerminalOperationCrashDenotation::Integer {
            primitive: decode_trapping_primitive(tag)?,
            result_type: decode_integer_type(reader)?,
            operand_type: decode_integer_type(reader)?,
        }
    })
}

fn require_row_tag(
    reader: &mut Reader<'_>,
    group: &'static str,
    expected: u8,
) -> Result<(), TerminalTraceV1ProfileCodecError> {
    let tag = reader.u8()?;
    if tag != expected {
        return Err(TerminalTraceV1ProfileCodecError::InvalidRowTag { group, tag });
    }
    Ok(())
}

fn validate_profile(
    profile: &TerminalTraceV1Profile,
) -> Result<(), TerminalTraceV1ProfileCodecError> {
    if profile.schema != TerminalObservationSchema::TerminalTraceV1 {
        return Err(TerminalTraceV1ProfileCodecError::UnsupportedSchemaVersion(
            profile.schema.version(),
        ));
    }
    if profile.module_identity.vocabulary_marker != VocabularyMarker::CURRENT {
        return Err(
            TerminalTraceV1ProfileCodecError::UnsupportedVocabularyMarker(
                profile.module_identity.vocabulary_marker.get(),
            ),
        );
    }
    if profile.module_identity.program_fingerprint.as_bytes() == &[0; 32] {
        return Err(TerminalTraceV1ProfileCodecError::ZeroModuleCommitment);
    }
    for schema in &profile.root.structural_inputs {
        validate_structural_schema(schema)?;
    }
    if let TerminalTraceResultSchema::Structural(schema) = &profile.root.result {
        validate_structural_schema(schema)?;
    }
    if profile
        .crash_sites
        .windows(2)
        .any(|rows| crash_site_key(&rows[0]) >= crash_site_key(&rows[1]))
    {
        return Err(TerminalTraceV1ProfileCodecError::NonCanonicalCrashSiteOrder);
    }
    for row in &profile.boundary_crash_sites {
        if !crash_routes_are_canonical(std::slice::from_ref(&row.route)) {
            return Err(TerminalTraceV1ProfileCodecError::Wire(
                CodecError::NonCanonicalOrder("boundary crash route alternatives"),
            ));
        }
        validate_crash_route_predicates(std::slice::from_ref(&row.route))?;
    }
    if profile
        .boundary_crash_sites
        .windows(2)
        .any(|rows| boundary_crash_site_key(&rows[0]) >= boundary_crash_site_key(&rows[1]))
    {
        return Err(TerminalTraceV1ProfileCodecError::NonCanonicalBoundaryCrashSiteOrder);
    }
    for row in &profile.operation_crash_sites {
        // A Trapping primitive always commits `Trap`, over fixed carriers;
        // binary arithmetic keeps one carrier for both operands.
        let invalid_integer = match row.denotation {
            TerminalOperationCrashDenotation::FloatNonFinite { .. } => false,
            TerminalOperationCrashDenotation::Integer {
                primitive,
                result_type,
                operand_type,
            } => {
                let same_carrier = matches!(
                    primitive,
                    TrappingIntegerPrimitive::Add
                        | TrappingIntegerPrimitive::Subtract
                        | TrappingIntegerPrimitive::Multiply
                        | TrappingIntegerPrimitive::Divide
                        | TrappingIntegerPrimitive::Remainder
                );
                result_type.is_address()
                    || operand_type.is_address()
                    || (same_carrier && operand_type != result_type)
            }
        };
        if row.cause != CrashCause::Trap || invalid_integer {
            return Err(TerminalTraceV1ProfileCodecError::InvalidOperationCrashDenotation);
        }
    }
    if profile
        .operation_crash_sites
        .windows(2)
        .any(|rows| operation_crash_site_key(&rows[0]) >= operation_crash_site_key(&rows[1]))
    {
        return Err(TerminalTraceV1ProfileCodecError::NonCanonicalOperationCrashSiteOrder);
    }
    for event in &profile.ordinary_events {
        for schema in &event.structural_arguments {
            validate_structural_schema(schema)?;
        }
        if let TerminalTraceResultSchema::Structural(schema) = &event.result {
            validate_structural_schema(schema)?;
        }
    }
    if profile
        .ordinary_events
        .windows(2)
        .any(|rows| ordinary_event_site_key(&rows[0]) >= ordinary_event_site_key(&rows[1]))
    {
        return Err(TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder);
    }
    Ok(())
}

fn crash_site_key(
    row: &TerminalTraceCrashSiteRow,
) -> (
    semantic_vocabulary::MachineId,
    semantic_vocabulary::BlockId,
    semantic_vocabulary::EdgeId,
) {
    (row.machine, row.block, row.edge)
}

fn boundary_crash_site_key(
    row: &TerminalTraceBoundaryCrashSiteRow,
) -> (
    semantic_vocabulary::MachineId,
    semantic_vocabulary::BlockId,
    semantic_vocabulary::OperationId,
    CrashCause,
) {
    (row.machine, row.block, row.operation, row.route.cause)
}

fn operation_crash_site_key(
    row: &TerminalTraceOperationCrashSiteRow,
) -> (
    semantic_vocabulary::MachineId,
    semantic_vocabulary::BlockId,
    semantic_vocabulary::OperationId,
) {
    (row.machine, row.block, row.operation)
}

fn ordinary_event_site_key(
    row: &TerminalTraceOrdinaryEventRow,
) -> (
    semantic_vocabulary::MachineId,
    semantic_vocabulary::BlockId,
    semantic_vocabulary::OperationId,
) {
    (row.machine, row.block, row.operation)
}

fn validate_structural_schema(
    schema: &TerminalTraceStructuralSchema,
) -> Result<(), TerminalTraceV1ProfileCodecError> {
    if schema
        .qualifications
        .windows(2)
        .any(|values| values[0] >= values[1])
    {
        return Err(TerminalTraceV1ProfileCodecError::NonCanonicalStructuralQualifications);
    }
    if schema
        .projected_qualifications
        .windows(2)
        .any(|values| values[0] >= values[1])
    {
        return Err(TerminalTraceV1ProfileCodecError::NonCanonicalStructuralQualifications);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BOUNDARY_CRASH_ROW_TAG, CRASH_ROW_TAG, CrashCause, DOMAIN, OPERATION_CRASH_ROW_TAG,
        TerminalObservationSchema, TerminalOperationCrashDenotation, TerminalTraceCrashSiteRow,
        TerminalTraceResultSchema, TerminalTraceRootRow, TerminalTraceV1Profile,
        TerminalTraceV1ProfileAcceptanceError, TerminalTraceV1ProfileCodecError, VocabularyMarker,
        accept_terminal_trace_v1_profile, decode_terminal_trace_v1_profile, encode_raw,
        encode_terminal_trace_v1_profile, reconstruct_canonical_terminal_trace_v1_profile,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId,
        OperationId, PlaceId, ScalarType, ServiceId, StructuralDomainId, StructuralPlaceKind,
        StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Block, BoundaryMachineDeclaration, CrashRouteBucket, CrashRouteGuard, MachineContract,
        Operation, OperationKind, OperationResult, ServiceDeclaration, StructuralAccess,
        StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
        StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalMachine, TerminalMachineResult, TerminalModule, TerminalPsiIdentity,
        TerminalTraceStructuralSchema, TerminalTraceValueComparison, Terminator, ValueDeclaration,
    };

    fn id<T>(raw: u64, make: impl FnOnce(u64) -> Option<T>) -> T {
        make(raw).expect("nonzero test identity")
    }

    fn module(terminator: Terminator, crash_routes: Vec<CrashRouteBucket>) -> TerminalModule {
        let machine = id(11, MachineId::new);
        let block = id(12, BlockId::new);
        TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator,
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    id: id(13, ContractId::new),
                    crash_routes,
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        }
    }

    fn unit_module() -> TerminalModule {
        module(
            Terminator::ReturnUnit {
                edge: id(14, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
            Vec::new(),
        )
    }

    fn ordinary_event_module() -> TerminalModule {
        let mut module = unit_module();
        let service = id(1, ServiceId::new);
        let boundary = id(1, BoundaryMachineId::new);
        let flag = id(1, ValueId::new);
        let byte = id(2, ValueId::new);
        let first_type = id(1, StructuralTypeId::new);
        let second_type = id(2, StructuralTypeId::new);
        let first_place = id(1, PlaceId::new);
        let second_place = id(2, PlaceId::new);
        let u8_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 test type"));
        module.structural_types = vec![
            StructuralTypeDeclaration {
                id: first_type,
                identity: "Console::Message".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: second_type,
                identity: "Console::Context".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ];
        module.services = vec![ServiceDeclaration {
            id: service,
            identity: "PortIo::write-byte".into(),
            parents: Vec::new(),
        }];
        module.root_service_reach.concrete = vec![service];
        module.boundary_machines = vec![BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            fixed_service_reach: Vec::new(),
            crash_routes: Vec::new(),
            id: boundary,
            identity: "Console::publish".into(),
            attachment: None,
            parameter_order: vec![
                terminal_psi::BoundaryParameterKind::Scalar,
                terminal_psi::BoundaryParameterKind::Scalar,
                terminal_psi::BoundaryParameterKind::Structural,
                terminal_psi::BoundaryParameterKind::Structural,
            ],
            scalar_parameters: vec![ScalarType::Boolean, u8_type],
            structural_parameters: vec![
                StructuralParameterDeclaration {
                    place: id(3, PlaceId::new),
                    position: 0,
                    is_self: false,
                    structural_type: first_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                StructuralParameterDeclaration {
                    place: id(4, PlaceId::new),
                    position: 1,
                    is_self: false,
                    structural_type: second_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
            ],
            result: terminal_psi::BoundaryMachineResult::Scalar(u8_type),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }];
        module.machines[0].parameters = vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: flag,
                scalar_type: ScalarType::Boolean,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: byte,
                scalar_type: u8_type,
            },
        ];
        module.machines[0].structural_parameters = vec![
            StructuralParameterDeclaration {
                place: first_place,
                position: 0,
                is_self: false,
                structural_type: first_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: second_place,
                position: 1,
                is_self: false,
                structural_type: second_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
        ];
        module.machines[0].structural_places = vec![
            StructuralPlaceDeclaration {
                id: first_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: second_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            },
        ];
        module.machines[0].published_service_ceiling = vec![service];
        module.machines[0].blocks[0].operations = vec![
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(2, OperationId::new),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id(3, ValueId::new),
                    scalar_type: u8_type,
                }),
                kind: OperationKind::BoundaryCall {
                    requirement_obligations: Vec::new(),
                    boundary,
                    arguments: vec![flag, byte],
                    structural_arguments: vec![
                        StructuralArgument {
                            place: first_place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        },
                        StructuralArgument {
                            place: second_place,
                            path: Vec::new(),
                            access: StructuralAccess::SharedBorrow,
                        },
                    ],
                    completion_receipts: Vec::new(),
                },
            },
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(1, OperationId::new),
                result: OperationResult::Unit,
                kind: OperationKind::PortWrite {
                    service,
                    port: 0x3f8,
                    value: b'X',
                },
            },
        ];
        module
    }

    fn direct_profile() -> TerminalTraceV1Profile {
        TerminalTraceV1Profile {
            schema: TerminalObservationSchema::TerminalTraceV1,
            module_identity: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([9; 32]),
            },
            root: TerminalTraceRootRow {
                entry: id(11, MachineId::new),
                scalar_inputs: Vec::new(),
                structural_inputs: Vec::new(),
                result: TerminalTraceResultSchema::Unit,
            },
            crash_sites: Vec::new(),
            boundary_crash_sites: Vec::new(),
            operation_crash_sites: Vec::new(),
            ordinary_events: Vec::new(),
        }
    }

    #[test]
    fn operation_crash_rows_round_trip_and_reject_a_changed_denotation() {
        use terminal_psi::{TerminalTraceOperationCrashSiteRow, TrappingIntegerPrimitive};
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let row = |operation, primitive, operand_type| TerminalTraceOperationCrashSiteRow {
            machine: id(11, MachineId::new),
            block: id(12, BlockId::new),
            operation: id(operation, OperationId::new),
            cause: CrashCause::Trap,
            denotation: TerminalOperationCrashDenotation::Integer {
                primitive,
                result_type: u8_type,
                operand_type,
            },
        };
        let mut profile = direct_profile();
        profile.operation_crash_sites = vec![
            row(3, TrappingIntegerPrimitive::Add, u8_type),
            row(4, TrappingIntegerPrimitive::ShiftLeft, u32_type),
        ];
        let bytes = encode_terminal_trace_v1_profile(&profile).expect("operation crash rows");
        assert_eq!(
            decode_terminal_trace_v1_profile(&bytes),
            Ok(profile.clone())
        );

        // The primitive fixes the cause, and binary arithmetic keeps one
        // carrier for both operands.
        let mut aborting = profile.clone();
        aborting.operation_crash_sites[0].cause = CrashCause::Abort;
        assert_eq!(
            encode_terminal_trace_v1_profile(&aborting),
            Err(TerminalTraceV1ProfileCodecError::InvalidOperationCrashDenotation),
        );
        let mut mixed = profile.clone();
        mixed.operation_crash_sites[0].denotation = TerminalOperationCrashDenotation::Integer {
            primitive: TrappingIntegerPrimitive::Add,
            result_type: u8_type,
            operand_type: u32_type,
        };
        assert_eq!(
            encode_terminal_trace_v1_profile(&mixed),
            Err(TerminalTraceV1ProfileCodecError::InvalidOperationCrashDenotation),
        );
        let mut reordered = profile.clone();
        reordered.operation_crash_sites.swap(0, 1);
        assert_eq!(
            encode_terminal_trace_v1_profile(&reordered),
            Err(TerminalTraceV1ProfileCodecError::NonCanonicalOperationCrashSiteOrder),
        );

        // Bytes: the row tag, then machine/block/operation, cause, primitive.
        let empty = encode_terminal_trace_v1_profile(&direct_profile()).unwrap();
        let group = empty.len() - 4 - 4 - 4;
        assert_eq!(&bytes[group..group + 4], &2_u32.to_le_bytes());
        assert_eq!(bytes[group + 4], OPERATION_CRASH_ROW_TAG);
        let primitive_offset = group + 4 + 1 + 8 + 8 + 8 + 1;
        let mut unknown_primitive = bytes.clone();
        unknown_primitive[primitive_offset] = 10;
        assert_eq!(
            decode_terminal_trace_v1_profile(&unknown_primitive),
            Err(TerminalTraceV1ProfileCodecError::InvalidTrappingPrimitive(
                10
            )),
        );
        let mut wrong_tag = bytes;
        wrong_tag[group + 4] = BOUNDARY_CRASH_ROW_TAG;
        assert!(matches!(
            decode_terminal_trace_v1_profile(&wrong_tag),
            Err(TerminalTraceV1ProfileCodecError::InvalidRowTag {
                group: "operation crash",
                ..
            }),
        ));
    }

    #[test]
    fn float_result_policy_crash_denotations_are_canonical() {
        use super::{Writer, encode_operation_crash_denotation};
        use semantic_vocabulary::IeeeFloatFormat;
        use terminal_psi::TerminalTraceOperationCrashSiteRow;
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let mut integer_writer = Writer::default();
        encode_operation_crash_denotation(
            &mut integer_writer,
            TerminalOperationCrashDenotation::Integer {
                primitive: terminal_psi::TrappingIntegerPrimitive::Add,
                result_type: u8_type,
                operand_type: u8_type,
            },
        );
        assert_eq!(
            integer_writer.finish(),
            [1, 2, 8, 0, 2, 8, 0],
            "existing integer row bytes are unchanged"
        );
        for (format, tag) in [
            (IeeeFloatFormat::Binary32, 1),
            (IeeeFloatFormat::Binary64, 2),
        ] {
            let denotation = TerminalOperationCrashDenotation::FloatNonFinite { format };
            let mut writer = Writer::default();
            encode_operation_crash_denotation(&mut writer, denotation);
            assert_eq!(writer.finish(), [9, tag]);
            let mut profile = direct_profile();
            profile
                .operation_crash_sites
                .push(TerminalTraceOperationCrashSiteRow {
                    machine: id(11, MachineId::new),
                    block: id(12, BlockId::new),
                    operation: id(3, OperationId::new),
                    cause: CrashCause::Trap,
                    denotation,
                });
            let encoded = encode_terminal_trace_v1_profile(&profile).unwrap();
            assert_eq!(decode_terminal_trace_v1_profile(&encoded).unwrap(), profile);
            profile.operation_crash_sites[0].cause = CrashCause::Abort;
            assert_eq!(
                encode_terminal_trace_v1_profile(&profile),
                Err(TerminalTraceV1ProfileCodecError::InvalidOperationCrashDenotation)
            );
        }
        let invalid_format = [9, 3];
        assert!(
            super::decode_operation_crash_denotation(&mut super::Reader::new(&invalid_format))
                .is_err()
        );
    }

    #[test]
    fn crash_rows_bind_exact_sites_causes_tags_and_canonical_order() {
        let cause = CrashCause::Trap;
        let edge = id(15, EdgeId::new);
        let module = module(
            Terminator::Crash {
                edge,
                cause,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
            vec![CrashRouteBucket {
                cause,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
        );
        let profile = reconstruct_canonical_terminal_trace_v1_profile(&module)
            .expect("canonical crash profile");
        assert_eq!(
            profile.crash_sites,
            [TerminalTraceCrashSiteRow {
                machine: id(11, MachineId::new),
                block: id(12, BlockId::new),
                edge,
                cause,
            }],
        );
        let bytes = encode_terminal_trace_v1_profile(&profile).expect("crash profile bytes");
        assert_eq!(
            accept_terminal_trace_v1_profile(&module, &bytes)
                .expect("module-bound crash profile acceptance"),
            profile,
        );
        let crash_tag_offset = DOMAIN.len() + 2 + 2 + 32 + 4 + 1 + 8 + 4 + 4 + 1 + 4;
        assert_eq!(bytes[crash_tag_offset], CRASH_ROW_TAG);

        let mut invalid_crash_tag = bytes.clone();
        invalid_crash_tag[crash_tag_offset] = 7;
        assert!(matches!(
            decode_terminal_trace_v1_profile(&invalid_crash_tag),
            Err(TerminalTraceV1ProfileCodecError::InvalidRowTag {
                group: "crash",
                tag: 7,
            }),
        ));

        let mut extra = profile.clone();
        extra.crash_sites.push(TerminalTraceCrashSiteRow {
            machine: id(12, MachineId::new),
            block: id(12, BlockId::new),
            edge,
            cause,
        });
        let extra_bytes =
            encode_terminal_trace_v1_profile(&extra).expect("canonical extra crash row bytes");
        assert!(matches!(
            accept_terminal_trace_v1_profile(&module, &extra_bytes),
            Err(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch),
        ));

        let mut duplicate = profile;
        let mut same_site_different_cause = duplicate.crash_sites[0];
        same_site_different_cause.cause = CrashCause::Abort;
        duplicate.crash_sites.push(same_site_different_cause);
        assert!(matches!(
            encode_terminal_trace_v1_profile(&duplicate),
            Err(TerminalTraceV1ProfileCodecError::NonCanonicalCrashSiteOrder),
        ));

        let mut reordered_bytes = extra_bytes;
        let first = reordered_bytes[crash_tag_offset..crash_tag_offset + 26].to_vec();
        let second = reordered_bytes[crash_tag_offset + 26..crash_tag_offset + 52].to_vec();
        reordered_bytes[crash_tag_offset..crash_tag_offset + 26].copy_from_slice(&second);
        reordered_bytes[crash_tag_offset + 26..crash_tag_offset + 52].copy_from_slice(&first);
        assert!(matches!(
            accept_terminal_trace_v1_profile(&module, &reordered_bytes),
            Err(TerminalTraceV1ProfileAcceptanceError::InvalidProfileBytes(
                TerminalTraceV1ProfileCodecError::NonCanonicalCrashSiteOrder,
            )),
        ));
    }

    #[test]
    fn ordinary_event_codec_rejects_duplicate_reordered_and_noncanonical_schemas() {
        let module = ordinary_event_module();
        let profile = reconstruct_canonical_terminal_trace_v1_profile(&module).unwrap();

        let mut duplicate = profile.clone();
        duplicate
            .ordinary_events
            .insert(1, duplicate.ordinary_events[0].clone());
        assert!(matches!(
            encode_terminal_trace_v1_profile(&duplicate),
            Err(TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder),
        ));
        let duplicate_bytes = encode_raw(&duplicate).expect("raw duplicate canary bytes");
        assert!(matches!(
            decode_terminal_trace_v1_profile(&duplicate_bytes),
            Err(TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder),
        ));

        let mut reordered = profile.clone();
        reordered.ordinary_events.swap(0, 1);
        assert!(matches!(
            encode_terminal_trace_v1_profile(&reordered),
            Err(TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder),
        ));
        let reordered_bytes = encode_raw(&reordered).expect("raw reordered canary bytes");
        assert!(matches!(
            decode_terminal_trace_v1_profile(&reordered_bytes),
            Err(TerminalTraceV1ProfileCodecError::NonCanonicalOrdinaryEventOrder),
        ));

        let mut rich = profile;
        rich.ordinary_events[0].structural_arguments = vec![TerminalTraceStructuralSchema {
            structural_type: id(30, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::SharedBorrow,
            qualifications: vec![
                id(32, StructuralDomainId::new),
                id(31, StructuralDomainId::new),
            ],
            projected_qualifications: Vec::new(),
            comparison: TerminalTraceValueComparison::ExactSemanticValue,
        }];
        assert!(matches!(
            encode_terminal_trace_v1_profile(&rich),
            Err(TerminalTraceV1ProfileCodecError::NonCanonicalStructuralQualifications),
        ));
    }
}
