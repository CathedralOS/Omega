//! Standalone canonical bytes for the first bounded D39 profile instance.

use psi_terminal::{
    CrashCause, StructuralAccess, StructuralMultiplicity, TerminalModule,
    TerminalObservationSchema, TerminalTraceCrashSiteRow, TerminalTraceResultSchema,
    TerminalTraceRootRow, TerminalTraceScalarSchema, TerminalTraceStructuralSchema,
    TerminalTraceV1Profile, TerminalTraceValueComparison, VocabularyMarker,
};
use psi_terminal_verifier::{
    TerminalTraceV1ReconstructionError,
    reconstruct_terminal_trace_v1_rows as reconstruct_verified_rows,
};

use crate::scalar_wire::{decode_scalar_type, encode_scalar_type};
use crate::wire::{Reader, Writer};
use crate::{CodecError, terminal_psi_identity};

const DOMAIN: &[u8] = b"omega.terminal.observation-profile.v1";
const ROOT_ROW_TAG: u8 = 1;
const CRASH_ROW_TAG: u8 = 2;

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
    NonCanonicalStructuralQualifications,
    NonCanonicalCrashSiteOrder,
    UnsupportedOrdinaryExternalEventRows(u32),
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
    let module_commitment = psi_terminal::SemanticFingerprint::from_bytes(reader.array()?);
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

    let ordinary_count = reader.count()?;
    if ordinary_count != 0 {
        return Err(
            TerminalTraceV1ProfileCodecError::UnsupportedOrdinaryExternalEventRows(ordinary_count),
        );
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
        module_identity: psi_terminal::TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: module_commitment,
        },
        root,
        crash_sites,
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

    // D39 fixes these groups and their order. This first internal-only rung
    // encodes their explicit empty counts rather than omitting them.
    writer.u32(0);
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
    match &root.result {
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
    let result = match reader.u8()? {
        1 => TerminalTraceResultSchema::Unit,
        2 => TerminalTraceResultSchema::Scalar(decode_scalar_schema(reader)?),
        3 => TerminalTraceResultSchema::Structural(decode_structural_schema(reader)?),
        tag => return Err(TerminalTraceV1ProfileCodecError::InvalidResultTag(tag)),
    };
    Ok(TerminalTraceRootRow {
        entry,
        scalar_inputs,
        structural_inputs,
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
    Ok(TerminalTraceStructuralSchema {
        structural_type,
        multiplicity,
        access,
        qualifications,
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
    Ok(())
}

fn crash_site_key(
    row: &TerminalTraceCrashSiteRow,
) -> (psi_core::MachineId, psi_core::BlockId, psi_core::EdgeId) {
    (row.machine, row.block, row.edge)
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, ScalarType,
        StructuralDomainId, StructuralTypeId,
    };
    use psi_terminal::{
        Block, CrashRouteBucket, CrashRouteGuard, MachineContract, StructuralAccess,
        StructuralMultiplicity, TerminalMachine, TerminalMachineResult, TerminalModule,
        TerminalPsiIdentity, TerminalTraceScalarSchema, TerminalTraceStructuralSchema,
        TerminalTraceValueComparison, Terminator,
    };

    use super::*;

    fn id<T>(raw: u64, make: impl FnOnce(u64) -> Option<T>) -> T {
        make(raw).expect("nonzero test identity")
    }

    fn module(terminator: Terminator, crash_routes: Vec<CrashRouteBucket>) -> TerminalModule {
        let machine = id(11, MachineId::new);
        let block = id(12, BlockId::new);
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
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
                    id: block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator,
                }],
                contract: MachineContract {
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

    fn direct_profile() -> TerminalTraceV1Profile {
        TerminalTraceV1Profile {
            schema: TerminalObservationSchema::TerminalTraceV1,
            module_identity: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([9; 32]),
            },
            root: TerminalTraceRootRow {
                entry: id(11, MachineId::new),
                scalar_inputs: Vec::new(),
                structural_inputs: Vec::new(),
                result: TerminalTraceResultSchema::Unit,
            },
            crash_sites: Vec::new(),
        }
    }

    #[test]
    fn canonical_profile_reconstruction_and_standalone_codec_round_trip() {
        let module = unit_module();
        let profile = reconstruct_canonical_terminal_trace_v1_profile(&module)
            .expect("canonical internal module profile");
        assert_eq!(
            profile.module_identity,
            terminal_psi_identity(&module).unwrap()
        );
        assert_eq!(profile.root.entry, module.entry);
        assert!(profile.crash_sites.is_empty());

        let bytes = encode_terminal_trace_v1_profile(&profile).expect("encode profile");
        assert!(bytes.starts_with(DOMAIN));
        assert_eq!(&bytes[DOMAIN.len()..DOMAIN.len() + 2], &1_u16.to_le_bytes(),);
        assert_eq!(
            decode_terminal_trace_v1_profile(&bytes).expect("decode profile"),
            profile,
        );
        assert_eq!(
            accept_terminal_trace_v1_profile(&module, &bytes)
                .expect("module-bound acceptance replays the same profile"),
            profile,
        );
    }

    #[test]
    fn standalone_codec_retains_ordered_scalar_structural_and_result_schemas() {
        let exact = TerminalTraceValueComparison::ExactSemanticValue;
        let mut profile = direct_profile();
        profile.root.scalar_inputs = vec![
            TerminalTraceScalarSchema {
                scalar_type: ScalarType::Boolean,
                comparison: exact,
            },
            TerminalTraceScalarSchema {
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 16).expect("i16"),
                ),
                comparison: exact,
            },
        ];
        profile.root.structural_inputs = vec![TerminalTraceStructuralSchema {
            structural_type: id(30, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::WriteOnlyBorrow,
            qualifications: vec![
                id(31, StructuralDomainId::new),
                id(32, StructuralDomainId::new),
            ],
            comparison: exact,
        }];
        profile.root.result =
            TerminalTraceResultSchema::Structural(TerminalTraceStructuralSchema {
                structural_type: id(33, StructuralTypeId::new),
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: vec![id(34, StructuralDomainId::new)],
                comparison: exact,
            });

        let bytes = encode_terminal_trace_v1_profile(&profile).expect("rich root profile bytes");
        assert_eq!(
            decode_terminal_trace_v1_profile(&bytes).expect("rich root profile decode"),
            profile,
        );

        let mut noncanonical = profile;
        noncanonical.root.structural_inputs[0]
            .qualifications
            .reverse();
        assert!(matches!(
            encode_terminal_trace_v1_profile(&noncanonical),
            Err(TerminalTraceV1ProfileCodecError::NonCanonicalStructuralQualifications),
        ));
    }

    #[test]
    fn canonical_codec_rejects_header_row_group_and_padding_mutations() {
        let bytes = encode_terminal_trace_v1_profile(&direct_profile()).expect("profile bytes");

        let mut invalid_domain = bytes.clone();
        invalid_domain[0] ^= 1;
        assert!(matches!(
            decode_terminal_trace_v1_profile(&invalid_domain),
            Err(TerminalTraceV1ProfileCodecError::InvalidDomain),
        ));

        let mut invalid_version = bytes.clone();
        invalid_version[DOMAIN.len()..DOMAIN.len() + 2].copy_from_slice(&2_u16.to_le_bytes());
        assert!(matches!(
            decode_terminal_trace_v1_profile(&invalid_version),
            Err(TerminalTraceV1ProfileCodecError::UnsupportedSchemaVersion(
                2
            )),
        ));

        let mut invalid_vocabulary = bytes.clone();
        invalid_vocabulary[DOMAIN.len() + 2..DOMAIN.len() + 4]
            .copy_from_slice(&0_u16.to_le_bytes());
        assert!(matches!(
            decode_terminal_trace_v1_profile(&invalid_vocabulary),
            Err(TerminalTraceV1ProfileCodecError::UnsupportedVocabularyMarker(0)),
        ));

        let root_count_offset = DOMAIN.len() + 2 + 2 + 32;
        let mut missing_root = bytes.clone();
        missing_root[root_count_offset..root_count_offset + 4]
            .copy_from_slice(&0_u32.to_le_bytes());
        assert!(matches!(
            decode_terminal_trace_v1_profile(&missing_root),
            Err(TerminalTraceV1ProfileCodecError::InvalidRootCount(0)),
        ));

        let mut invalid_root_tag = bytes.clone();
        invalid_root_tag[root_count_offset + 4] = 9;
        assert!(matches!(
            decode_terminal_trace_v1_profile(&invalid_root_tag),
            Err(TerminalTraceV1ProfileCodecError::InvalidRowTag {
                group: "root",
                tag: 9,
            }),
        ));

        let mut ordinary_rows = bytes.clone();
        let ordinary_count_offset = ordinary_rows.len() - 8;
        ordinary_rows[ordinary_count_offset..ordinary_count_offset + 4]
            .copy_from_slice(&1_u32.to_le_bytes());
        assert!(matches!(
            decode_terminal_trace_v1_profile(&ordinary_rows),
            Err(TerminalTraceV1ProfileCodecError::UnsupportedOrdinaryExternalEventRows(1)),
        ));

        let mut terminal_rows = bytes.clone();
        let terminal_count_offset = terminal_rows.len() - 4;
        terminal_rows[terminal_count_offset..].copy_from_slice(&1_u32.to_le_bytes());
        assert!(matches!(
            decode_terminal_trace_v1_profile(&terminal_rows),
            Err(TerminalTraceV1ProfileCodecError::UnsupportedExternalTerminationRows(1)),
        ));

        let mut padded = bytes;
        padded.push(0);
        assert!(matches!(
            decode_terminal_trace_v1_profile(&padded),
            Err(TerminalTraceV1ProfileCodecError::TrailingBytes(1)),
        ));
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
    fn module_bound_acceptance_rejects_stale_substituted_missing_and_extra_rows() {
        let cause = CrashCause::Abort;
        let edge = id(16, EdgeId::new);
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
        let profile = reconstruct_canonical_terminal_trace_v1_profile(&module).unwrap();
        let bytes = encode_terminal_trace_v1_profile(&profile).unwrap();

        let mut substituted_module = module.clone();
        let substituted_machine = id(21, MachineId::new);
        substituted_module.entry = substituted_machine;
        substituted_module.machines[0].id = substituted_machine;
        assert!(matches!(
            accept_terminal_trace_v1_profile(&substituted_module, &bytes),
            Err(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch),
        ));

        let mut missing = profile.clone();
        missing.crash_sites.clear();
        let missing_bytes = encode_terminal_trace_v1_profile(&missing).unwrap();
        assert!(matches!(
            accept_terminal_trace_v1_profile(&module, &missing_bytes),
            Err(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch),
        ));

        let mut extra = profile;
        extra.crash_sites.push(TerminalTraceCrashSiteRow {
            machine: id(22, MachineId::new),
            block: id(23, BlockId::new),
            edge: id(24, EdgeId::new),
            cause,
        });
        let extra_bytes = encode_terminal_trace_v1_profile(&extra).unwrap();
        assert!(matches!(
            accept_terminal_trace_v1_profile(&module, &extra_bytes),
            Err(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch),
        ));
    }
}
