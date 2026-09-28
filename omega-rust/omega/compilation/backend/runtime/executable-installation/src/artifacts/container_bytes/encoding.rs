//! Encoding one executable container version and its records.

use crate::artifacts::Artifact;
use crate::artifacts::container::{
    ArtifactRelocationKind, ContainerLimits, OMEGA_EXECUTABLE_CONTAINER_V1_MARKER,
    OMEGA_EXECUTABLE_CONTAINER_V2_MARKER, normalized_proof_payload_digest,
};
use crate::artifacts::container_bytes::decoding::checked_slice_mut;
use crate::artifacts::container_bytes::decoding::decode_executable_container;
use crate::artifacts::container_bytes::record_layouts::{
    entry_layout, header_layout, identity_layout, placement_layout, relocation_layout,
    section_layout,
};
use crate::artifacts::container_bytes::{
    AUTHORITY_COMMITMENT_BYTES, ENTRY_RECORD_BYTES, OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES,
    OMEGA_EXECUTABLE_CONTAINER_MAGIC, OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES,
    PLACEMENT_RECORD_BYTES, RELOCATION_COUNT_BYTES, RELOCATION_RECORD_BYTES,
    SECTION_AUTHORITY_COMMITMENTS, SECTION_CODE, SECTION_CONTRACTS, SECTION_ENTRIES,
    SECTION_FOOTPRINT, SECTION_PLACEMENT, SECTION_PROOF, SECTION_RELOCATIONS,
};
use crate::installation::InstallationDiagnostic;
use layout_plans::RelocationTarget;
use layout_plans::{
    ByteOrder, LayoutPlanReport, PlacementPhase, ScalarFieldValue, materialize_scalar_layout_into,
};
use target::Architecture;

pub(crate) fn encode_executable_container_version(
    artifact: &Artifact,
    proof: &[u8],
    limits: ContainerLimits,
    strong_authority: bool,
) -> Result<Vec<u8>, InstallationDiagnostic> {
    if proof.is_empty() {
        return Err(InstallationDiagnostic(
            "artifact proof section cannot be empty".into(),
        ));
    }
    match (strong_authority, artifact.0.authority_commitments) {
        (true, None) => {
            return Err(InstallationDiagnostic(
                "container-v2 encoding requires strong authority commitments".into(),
            ));
        }
        (false, Some(_)) => {
            return Err(InstallationDiagnostic(
                "container-v1 compatibility encoding cannot discard strong authority commitments"
                    .into(),
            ));
        }
        _ => {}
    }
    let section_count = if strong_authority { 8_u64 } else { 7_u64 };
    if limits.max_sections < section_count as usize {
        return Err(InstallationDiagnostic(format!(
            "canonical executable container needs {section_count} sections, configured bound is {}",
            limits.max_sections
        )));
    }
    if artifact.0.relocations.len() > limits.max_relocations {
        return Err(InstallationDiagnostic(format!(
            "artifact contains {} relocations, exceeding configured bound {}",
            artifact.0.relocations.len(),
            limits.max_relocations
        )));
    }

    let directory_end = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES
        .checked_add(
            section_count
                .checked_mul(OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES)
                .ok_or_else(|| {
                    InstallationDiagnostic("artifact section directory overflows".into())
                })?,
        )
        .ok_or_else(|| {
            InstallationDiagnostic("artifact section directory range overflows".into())
        })?;
    let relocation_length = RELOCATION_COUNT_BYTES
        .checked_add(
            (artifact.0.relocations.len() as u64)
                .checked_mul(RELOCATION_RECORD_BYTES)
                .ok_or_else(|| {
                    InstallationDiagnostic("artifact relocation payload length overflows".into())
                })?,
        )
        .ok_or_else(|| InstallationDiagnostic("artifact relocation section overflows".into()))?;
    let entry_length = (artifact.0.entries.len() as u64)
        .checked_mul(ENTRY_RECORD_BYTES)
        .ok_or_else(|| InstallationDiagnostic("artifact entry payload length overflows".into()))?;
    let proof_length = u64::try_from(proof.len())
        .map_err(|_| InstallationDiagnostic("artifact proof length is not representable".into()))?;

    let mut payload_lengths = vec![
        artifact.0.byte_length,
        relocation_length,
        8,
        8,
        PLACEMENT_RECORD_BYTES,
        entry_length,
        proof_length,
    ];
    if strong_authority {
        payload_lengths.push(AUTHORITY_COMMITMENT_BYTES);
    }
    if let Some(length) = payload_lengths
        .iter()
        .copied()
        .find(|length| *length == 0 || *length > limits.max_section_bytes)
    {
        return Err(InstallationDiagnostic(format!(
            "canonical artifact section length {length} is empty or exceeds configured bound {}",
            limits.max_section_bytes
        )));
    }

    let mut offsets = Vec::with_capacity(payload_lengths.len());
    let mut cursor = directory_end;
    for length in payload_lengths.iter().copied() {
        offsets.push(cursor);
        cursor = cursor
            .checked_add(length)
            .ok_or_else(|| InstallationDiagnostic("artifact container length overflows".into()))?;
    }
    let total_length = cursor;
    if total_length > limits.max_total_bytes {
        return Err(InstallationDiagnostic(format!(
            "canonical artifact container needs {total_length} bytes, configured bound is {}",
            limits.max_total_bytes
        )));
    }
    let total_host_length = usize::try_from(total_length).map_err(|_| {
        InstallationDiagnostic("artifact container length does not fit this compiler host".into())
    })?;
    let mut bytes = vec![0_u8; total_host_length];

    encode_record(
        &mut bytes[..OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize],
        header_layout(),
        &[
            (
                "magic",
                64,
                u64::from_le_bytes(OMEGA_EXECUTABLE_CONTAINER_MAGIC),
            ),
            (
                "format_marker",
                16,
                u64::from(if strong_authority {
                    OMEGA_EXECUTABLE_CONTAINER_V2_MARKER
                } else {
                    OMEGA_EXECUTABLE_CONTAINER_V1_MARKER
                }),
            ),
            ("header_bytes", 16, OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES),
            (
                "architecture",
                8,
                match artifact.0.architecture {
                    Architecture::Aarch64 => 1,
                    Architecture::X86_64 => 2,
                },
            ),
            ("reserved0", 8, 0),
            ("section_count", 16, section_count),
            (
                "directory_offset",
                64,
                OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES,
            ),
            ("total_length", 64, total_length),
            ("artifact", 64, artifact.0.identity.normalized_identity()),
            (
                "content",
                64,
                artifact.0.container_fingerprint.compatibility_value(),
            ),
            ("reserved1", 64, 0),
            ("reserved2", 64, 0),
        ],
        "container header",
    )?;

    let proof_digest = normalized_proof_payload_digest(proof);
    let mut sections = vec![
        (SECTION_CODE, 1, 0, offsets[0], payload_lengths[0]),
        (
            SECTION_RELOCATIONS,
            1,
            artifact.0.relocation_set.normalized_identity(),
            offsets[1],
            payload_lengths[1],
        ),
        (
            SECTION_CONTRACTS,
            1,
            artifact.0.contracts.normalized_identity(),
            offsets[2],
            payload_lengths[2],
        ),
        (
            SECTION_FOOTPRINT,
            1,
            artifact.0.declared_footprint.normalized_identity(),
            offsets[3],
            payload_lengths[3],
        ),
        (
            SECTION_PLACEMENT,
            1,
            artifact.0.placement_plan.normalized_identity(),
            offsets[4],
            payload_lengths[4],
        ),
        (
            SECTION_ENTRIES,
            1,
            artifact.0.entry_set.normalized_identity(),
            offsets[5],
            payload_lengths[5],
        ),
        (SECTION_PROOF, 1, 0, offsets[6], payload_lengths[6]),
    ];
    if strong_authority {
        sections.push((
            SECTION_AUTHORITY_COMMITMENTS,
            1,
            0,
            offsets[7],
            payload_lengths[7],
        ));
    }
    for (index, (kind, flags, identity, offset, length)) in sections.iter().enumerate() {
        let record_offset = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES
            + index as u64 * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES;
        encode_record_at(
            &mut bytes,
            record_offset,
            OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES,
            section_layout(),
            &[
                ("kind", 16, u64::from(*kind)),
                ("flags", 16, *flags),
                ("reserved", 32, 0),
                ("identity", 64, *identity),
                ("offset", 64, *offset),
                ("length", 64, *length),
            ],
            "artifact section record",
        )?;
    }

    checked_slice_mut(
        &mut bytes,
        offsets[0],
        payload_lengths[0],
        "artifact code section",
    )?
    .copy_from_slice(&artifact.0.code);
    encode_record_at(
        &mut bytes,
        offsets[1],
        RELOCATION_COUNT_BYTES,
        identity_layout(),
        &[("identity", 64, artifact.0.relocations.len() as u64)],
        "relocation count",
    )?;
    for (index, relocation) in artifact.0.relocations.iter().enumerate() {
        let offset = offsets[1]
            .checked_add(RELOCATION_COUNT_BYTES)
            .and_then(|start| {
                (index as u64)
                    .checked_mul(RELOCATION_RECORD_BYTES)
                    .and_then(|delta| start.checked_add(delta))
            })
            .ok_or_else(|| {
                InstallationDiagnostic("artifact relocation-record offset overflows".into())
            })?;
        let (target_kind, target) = match relocation.target {
            RelocationTarget::Entry(identity) => (1, identity.normalized_identity()),
            RelocationTarget::Data(identity) => (2, identity.normalized_identity()),
        };
        encode_record_at(
            &mut bytes,
            offset,
            RELOCATION_RECORD_BYTES,
            relocation_layout(),
            &[
                (
                    "kind",
                    16,
                    match relocation.kind {
                        ArtifactRelocationKind::Absolute64 => 1,
                        ArtifactRelocationKind::X86Relative32 => 2,
                        ArtifactRelocationKind::Aarch64Page21 => 3,
                        ArtifactRelocationKind::Aarch64PageOffset12 => 4,
                        ArtifactRelocationKind::Aarch64Branch26 => 5,
                    },
                ),
                ("target_kind", 16, target_kind),
                ("reserved", 32, 0),
                ("destination", 64, relocation.destination_offset),
                ("target", 64, target),
                ("addend", 64, relocation.addend as u64),
            ],
            "artifact relocation record",
        )?;
    }
    encode_record_at(
        &mut bytes,
        offsets[2],
        8,
        identity_layout(),
        &[("identity", 64, artifact.0.contracts.normalized_identity())],
        "contract section",
    )?;
    encode_record_at(
        &mut bytes,
        offsets[3],
        8,
        identity_layout(),
        &[(
            "identity",
            64,
            artifact.0.declared_footprint.normalized_identity(),
        )],
        "footprint section",
    )?;
    let constraints = artifact.0.placement_constraints;
    let (range_present, range_start, range_end) = constraints
        .permitted_range()
        .map(|range| (1, range.start_inclusive(), range.end_exclusive()))
        .unwrap_or((0, 0, 0));
    let (regime_present, regime) = constraints
        .machine_regime()
        .map(|regime| (1, regime.normalized_identity()))
        .unwrap_or((0, 0));
    let (scope_present, scope) = constraints
        .installation_scope()
        .map(|scope| (1, scope.normalized_identity()))
        .unwrap_or((0, 0));
    encode_record_at(
        &mut bytes,
        offsets[4],
        PLACEMENT_RECORD_BYTES,
        placement_layout(),
        &[
            ("plan", 64, artifact.0.placement_plan.normalized_identity()),
            ("range_present", 8, range_present),
            (
                "phase",
                8,
                match constraints.phase() {
                    PlacementPhase::Build => 1,
                    PlacementPhase::Load => 2,
                    PlacementPhase::PostHandoff => 3,
                },
            ),
            ("regime_present", 8, regime_present),
            ("scope_present", 8, scope_present),
            ("reserved0", 32, 0),
            ("range_start", 64, range_start),
            ("range_end", 64, range_end),
            ("alignment", 64, constraints.alignment()),
            ("regime", 64, regime),
            ("scope", 64, scope),
            ("reserved1", 64, 0),
        ],
        "placement section",
    )?;
    for (index, entry) in artifact.0.entries.iter().enumerate() {
        let offset = offsets[5]
            .checked_add(index as u64 * ENTRY_RECORD_BYTES)
            .ok_or_else(|| {
                InstallationDiagnostic("artifact entry-record offset overflows".into())
            })?;
        encode_record_at(
            &mut bytes,
            offset,
            ENTRY_RECORD_BYTES,
            entry_layout(),
            &[
                ("identity", 64, entry.identity().normalized_identity()),
                ("offset", 64, entry.code_offset()),
            ],
            "artifact entry record",
        )?;
    }
    checked_slice_mut(
        &mut bytes,
        offsets[6],
        payload_lengths[6],
        "artifact proof section",
    )?
    .copy_from_slice(proof);
    if let Some(commitments) = artifact.0.authority_commitments {
        let authority = checked_slice_mut(
            &mut bytes,
            offsets[7],
            payload_lengths[7],
            "artifact authority-commitment section",
        )?;
        authority[..32].copy_from_slice(commitments.imported_contracts().as_bytes());
        authority[32..64].copy_from_slice(commitments.declared_footprint().as_bytes());
        authority[64..96].copy_from_slice(commitments.machine_regime().as_bytes());
        authority[96..128].copy_from_slice(commitments.installation_scope().as_bytes());
    }

    let checked = decode_executable_container(&bytes, limits)?;
    if checked.artifact() != artifact
        || checked.proof_payload() != proof_digest
        || checked.proof() != proof
    {
        return Err(InstallationDiagnostic(
            "canonical executable-container encoder self-check disagrees with its input".into(),
        ));
    }
    Ok(bytes)
}

pub(crate) fn encode_record(
    destination: &mut [u8],
    layout: LayoutPlanReport,
    values: &[(&str, u16, u64)],
    label: &str,
) -> Result<(), InstallationDiagnostic> {
    let values = values
        .iter()
        .map(|(field, width, value)| {
            ScalarFieldValue::new(*field, *width, *value)
                .map_err(|error| InstallationDiagnostic(format!("{label}: {}", error.0)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    materialize_scalar_layout_into(&layout, &values, ByteOrder::LittleEndian, destination)
        .map_err(|error| InstallationDiagnostic(format!("{label}: {}", error.0)))
}

fn encode_record_at(
    bytes: &mut [u8],
    offset: u64,
    length: u64,
    layout: LayoutPlanReport,
    values: &[(&str, u16, u64)],
    label: &str,
) -> Result<(), InstallationDiagnostic> {
    let destination = checked_slice_mut(bytes, offset, length, label)?;
    encode_record(destination, layout, values, label)
}
