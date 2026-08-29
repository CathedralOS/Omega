use std::collections::BTreeMap;

use psi_layout_plans::{
    ArtifactInstallationScopeId, ByteOrder, DataSymbolId, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport, MachineRegimeId, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, ScalarFieldSchema, ScalarFieldValue,
    decode_scalar_layout, materialize_scalar_layout_into,
};

use super::*;

pub const OMEGA_EXECUTABLE_CONTAINER_MAGIC: [u8; 8] = *b"OMEGAXE!";
pub const OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES: u64 = 64;
pub const OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES: u64 = 32;

const RELOCATION_COUNT_BYTES: u64 = 8;
const RELOCATION_RECORD_BYTES: u64 = 32;
const ENTRY_RECORD_BYTES: u64 = 16;
const PLACEMENT_RECORD_BYTES: u64 = 64;

const SECTION_CODE: u16 = 1;
const SECTION_RELOCATIONS: u16 = 2;
const SECTION_CONTRACTS: u16 = 3;
const SECTION_FOOTPRINT: u16 = 4;
const SECTION_PLACEMENT: u16 = 5;
const SECTION_ENTRIES: u16 = 6;
const SECTION_PROOF: u16 = 7;
const SECTION_INFORMATIONAL: u16 = 8;

pub fn non_authoritative_informational_section_fingerprint(
    kind: u16,
    payload: &[u8],
) -> NonAuthoritativeInformationalFingerprint64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in b"omega.executable-container.information.v1"
        .iter()
        .copied()
        .chain(kind.to_le_bytes())
        .chain((payload.len() as u64).to_le_bytes())
        .chain(payload.iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    NonAuthoritativeInformationalFingerprint64::from_compatibility_value(if hash == 0 {
        1
    } else {
        hash
    })
    .expect("fixed FNV normalization replaces zero")
}

#[derive(Debug, Clone, Copy)]
struct WireSection {
    kind: u16,
    required: bool,
    identity: u64,
    offset: u64,
    length: u64,
}

/// Emits the canonical Omega-native byte container for one compiler-produced
/// artifact candidate and exact proof payload.
///
/// The encoder publishes no optional informational sections and derives the
/// proof identity from the exact bytes. Before returning, it routes its own
/// output through the hostile-input decoder and semantic validator. An encoder
/// bug therefore fails closed instead of producing a container that a later
/// loader interprets differently.
pub fn encode_executable_container(
    artifact: &Artifact,
    proof: &[u8],
    limits: ContainerLimits,
) -> Result<Vec<u8>, InstallationDiagnostic> {
    if proof.is_empty() {
        return Err(InstallationDiagnostic(
            "artifact proof section cannot be empty".into(),
        ));
    }
    let section_count = 7_u64;
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

    let payload_lengths = [
        artifact.0.byte_length,
        relocation_length,
        8,
        8,
        PLACEMENT_RECORD_BYTES,
        entry_length,
        proof_length,
    ];
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
    for length in payload_lengths {
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
                u64::from(OMEGA_EXECUTABLE_CONTAINER_MARKER),
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
    let sections = [
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

/// Decodes and validates one canonical Omega-native executable container.
///
/// This is the only raw-byte entry to executable-artifact admission. The
/// fixed records are decoded through validated layout plans, all arithmetic is
/// checked before slicing, and the result still grants no executable
/// eligibility: callers receive only the ordinary validated candidate consumed
/// by the separate admission gate.
pub fn decode_executable_container(
    bytes: &[u8],
    limits: ContainerLimits,
) -> Result<ValidatedArtifactContainer, InstallationDiagnostic> {
    validate_decode_limits(bytes, limits)?;
    let header = decode_record(header_layout(), header_schema(), bytes, "container header")?;
    if header["magic"] != u64::from_le_bytes(OMEGA_EXECUTABLE_CONTAINER_MAGIC) {
        return Err(InstallationDiagnostic(
            "Omega executable container has invalid magic".into(),
        ));
    }
    require_zero("container header reserved0", header["reserved0"])?;
    require_zero("container header reserved1", header["reserved1"])?;
    require_zero("container header reserved2", header["reserved2"])?;
    if header["format_marker"] != u64::from(OMEGA_EXECUTABLE_CONTAINER_MARKER) {
        return Err(InstallationDiagnostic(format!(
            "unsupported Omega executable container marker 0x{:04x}",
            header["format_marker"]
        )));
    }
    if header["header_bytes"] != OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES {
        return Err(InstallationDiagnostic(format!(
            "Omega executable container header length {} is not canonical {}",
            header["header_bytes"], OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES
        )));
    }
    let total_length = header["total_length"];
    if total_length != bytes.len() as u64 {
        return Err(InstallationDiagnostic(format!(
            "Omega executable container declares {total_length} bytes but input has {}",
            bytes.len()
        )));
    }
    let section_count = usize::try_from(header["section_count"]).map_err(|_| {
        InstallationDiagnostic("artifact section count does not fit this compiler host".into())
    })?;
    if section_count == 0 || section_count > limits.max_sections {
        return Err(InstallationDiagnostic(format!(
            "artifact container has {section_count} sections, configured bound is {}",
            limits.max_sections
        )));
    }
    let directory_offset = header["directory_offset"];
    if directory_offset != OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES {
        return Err(InstallationDiagnostic(format!(
            "artifact section directory offset {directory_offset} is not canonical {}",
            OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES
        )));
    }
    let directory_bytes = (section_count as u64)
        .checked_mul(OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES)
        .ok_or_else(|| InstallationDiagnostic("artifact section directory overflows".into()))?;
    let directory_end = directory_offset
        .checked_add(directory_bytes)
        .ok_or_else(|| {
            InstallationDiagnostic("artifact section directory range overflows".into())
        })?;
    checked_slice(
        bytes,
        directory_offset,
        directory_bytes,
        "artifact section directory",
    )?;

    let architecture = match header["architecture"] {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        value => {
            return Err(InstallationDiagnostic(format!(
                "unknown executable-container architecture {value}"
            )));
        }
    };
    let artifact = ArtifactId::from_normalized_identity(header["artifact"])?;
    let content_fingerprint =
        NonAuthoritativeContainerFingerprint64::from_compatibility_value(header["content"])?;

    let mut wire_sections = Vec::with_capacity(section_count);
    for index in 0..section_count {
        let record_offset = directory_offset
            .checked_add(
                (index as u64)
                    .checked_mul(OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES)
                    .ok_or_else(|| {
                        InstallationDiagnostic("artifact section-record offset overflows".into())
                    })?,
            )
            .ok_or_else(|| {
                InstallationDiagnostic("artifact section-record address overflows".into())
            })?;
        let record = decode_record(
            section_layout(),
            section_schema(),
            checked_slice(
                bytes,
                record_offset,
                OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES,
                "artifact section record",
            )?,
            "artifact section record",
        )?;
        require_zero("artifact section record reserved", record["reserved"])?;
        if record["flags"] & !1 != 0 {
            return Err(InstallationDiagnostic(format!(
                "artifact section record {index} has unknown flags {:#x}",
                record["flags"]
            )));
        }
        let kind = u16::try_from(record["kind"]).expect("16-bit layout field");
        let required = record["flags"] == 1;
        validate_known_section_flags(kind, required)?;
        let section = WireSection {
            kind,
            required,
            identity: record["identity"],
            offset: record["offset"],
            length: record["length"],
        };
        if section.length == 0 || section.length > limits.max_section_bytes {
            return Err(InstallationDiagnostic(format!(
                "artifact section length {} is empty or exceeds configured bound {}",
                section.length, limits.max_section_bytes
            )));
        }
        if section.offset < directory_end {
            return Err(InstallationDiagnostic(format!(
                "artifact section {} begins inside the canonical header/directory prefix",
                section.offset
            )));
        }
        checked_slice(
            bytes,
            section.offset,
            section.length,
            "artifact section payload",
        )?;
        validate_wire_section_identity(section)?;
        if section.kind > SECTION_INFORMATIONAL && section.required {
            return Err(InstallationDiagnostic(format!(
                "unknown required artifact section {}",
                section.identity
            )));
        }
        if section.kind >= SECTION_INFORMATIONAL {
            let payload = checked_slice(
                bytes,
                section.offset,
                section.length,
                "informational section payload",
            )?;
            let normalized =
                non_authoritative_informational_section_fingerprint(section.kind, payload);
            if section.identity != normalized.compatibility_value() {
                return Err(InstallationDiagnostic(format!(
                    "informational artifact section kind {} identity does not match its exact opaque bytes",
                    section.kind
                )));
            }
        }
        wire_sections.push(section);
    }
    validate_payload_tiling(&wire_sections, directory_end, total_length)?;

    let code_section = only_wire_section(&wire_sections, SECTION_CODE, "code")?;
    let relocation_section = only_wire_section(&wire_sections, SECTION_RELOCATIONS, "relocations")?;
    let contracts_section = only_wire_section(&wire_sections, SECTION_CONTRACTS, "contracts")?;
    let footprint_section = only_wire_section(&wire_sections, SECTION_FOOTPRINT, "footprint")?;
    let placement_section = only_wire_section(&wire_sections, SECTION_PLACEMENT, "placement")?;
    let entries_section = only_wire_section(&wire_sections, SECTION_ENTRIES, "entries")?;
    let proof_section = only_wire_section(&wire_sections, SECTION_PROOF, "proof")?;

    let code = checked_slice(
        bytes,
        code_section.offset,
        code_section.length,
        "artifact code section",
    )?
    .to_vec();
    let contracts = decode_identity_payload(
        bytes,
        contracts_section,
        "contract",
        MachineContractSetId::from_normalized_identity,
    )?;
    let declared_footprint = decode_identity_payload(
        bytes,
        footprint_section,
        "footprint",
        MachineFootprintId::from_normalized_identity,
    )?;
    let (placement_plan, placement_constraints) = decode_placement(bytes, placement_section)?;
    let (entry_set, entries) = decode_entries(bytes, entries_section, architecture)?;
    let (relocation_set, relocations) =
        decode_relocations(bytes, relocation_section, limits.max_relocations)?;
    let proof = checked_slice(
        bytes,
        proof_section.offset,
        proof_section.length,
        "artifact proof section",
    )?
    .to_vec();
    let proof_payload = normalized_proof_payload_digest(&proof);

    let sections = wire_sections
        .into_iter()
        .map(|section| {
            let kind = match section.kind {
                SECTION_CODE => ContainerSectionKind::Code,
                SECTION_RELOCATIONS => ContainerSectionKind::Relocations(relocation_set),
                SECTION_CONTRACTS => ContainerSectionKind::Contracts(contracts),
                SECTION_FOOTPRINT => ContainerSectionKind::Footprint(declared_footprint),
                SECTION_PLACEMENT => ContainerSectionKind::Placement(placement_plan),
                SECTION_ENTRIES => ContainerSectionKind::Entries(entry_set),
                SECTION_PROOF => ContainerSectionKind::Proof(proof_payload),
                SECTION_INFORMATIONAL => ContainerSectionKind::Informational(
                    NonAuthoritativeInformationalFingerprint64::from_compatibility_value(
                        section.identity,
                    )
                    .expect("wire identity checked before section construction"),
                ),
                identity => ContainerSectionKind::Unknown {
                    identity: if section.identity == 0 {
                        u64::from(identity)
                    } else {
                        section.identity
                    },
                    required: section.required,
                },
            };
            ContainerSection {
                kind,
                offset: section.offset,
                length: section.length,
            }
        })
        .collect();

    validate_decoded_container(
        DecodedArtifactContainer {
            format_marker: OMEGA_EXECUTABLE_CONTAINER_MARKER,
            total_length,
            artifact,
            content_fingerprint,
            architecture,
            code_length: code_section.length,
            code,
            contracts,
            declared_footprint,
            placement_plan,
            placement_constraints,
            entry_set,
            entries,
            relocation_set,
            relocations,
            proof_payload,
            proof,
            sections,
        },
        limits,
    )
}

fn validate_decode_limits(
    bytes: &[u8],
    limits: ContainerLimits,
) -> Result<(), InstallationDiagnostic> {
    if limits.max_total_bytes == 0
        || limits.max_sections == 0
        || limits.max_section_bytes == 0
        || limits.max_relocations == 0
    {
        return Err(InstallationDiagnostic(
            "artifact-container limits must all be nonzero".into(),
        ));
    }
    if bytes.len() as u64 > limits.max_total_bytes {
        return Err(InstallationDiagnostic(format!(
            "artifact container has {} bytes, exceeding configured bound {}",
            bytes.len(),
            limits.max_total_bytes
        )));
    }
    if bytes.len() < OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize {
        return Err(InstallationDiagnostic(format!(
            "artifact container needs {} header bytes, input has {}",
            OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES,
            bytes.len()
        )));
    }
    Ok(())
}

fn validate_known_section_flags(kind: u16, required: bool) -> Result<(), InstallationDiagnostic> {
    match kind {
        SECTION_CODE | SECTION_RELOCATIONS | SECTION_CONTRACTS | SECTION_FOOTPRINT
        | SECTION_PLACEMENT | SECTION_ENTRIES | SECTION_PROOF
            if !required =>
        {
            Err(InstallationDiagnostic(format!(
                "semantic artifact section kind {kind} must be required"
            )))
        }
        SECTION_INFORMATIONAL if required => Err(InstallationDiagnostic(
            "informational artifact sections cannot be required".into(),
        )),
        _ => Ok(()),
    }
}

fn validate_wire_section_identity(section: WireSection) -> Result<(), InstallationDiagnostic> {
    match section.kind {
        SECTION_CODE | SECTION_PROOF if section.identity != 0 => {
            Err(InstallationDiagnostic(format!(
                "artifact section kind {} must use zero wire identity",
                section.kind
            )))
        }
        SECTION_RELOCATIONS
        | SECTION_CONTRACTS
        | SECTION_FOOTPRINT
        | SECTION_PLACEMENT
        | SECTION_ENTRIES
        | SECTION_INFORMATIONAL
            if section.identity == 0 =>
        {
            Err(InstallationDiagnostic(format!(
                "artifact section kind {} requires a nonzero normalized identity",
                section.kind
            )))
        }
        kind if kind > SECTION_INFORMATIONAL && section.identity == 0 => {
            Err(InstallationDiagnostic(format!(
                "unknown artifact section kind {kind} requires a nonzero trace identity"
            )))
        }
        _ => Ok(()),
    }
}

fn only_wire_section(
    sections: &[WireSection],
    kind: u16,
    name: &str,
) -> Result<WireSection, InstallationDiagnostic> {
    let matching = sections
        .iter()
        .copied()
        .filter(|section| section.kind == kind)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(InstallationDiagnostic(format!(
            "artifact container requires exactly one {name} section, found {}",
            matching.len()
        )));
    }
    Ok(matching[0])
}

fn validate_payload_tiling(
    sections: &[WireSection],
    payload_start: u64,
    total_length: u64,
) -> Result<(), InstallationDiagnostic> {
    let mut ranges = sections
        .iter()
        .map(|section| {
            section
                .offset
                .checked_add(section.length)
                .map(|end| (section.offset, end))
                .ok_or_else(|| InstallationDiagnostic("artifact section range overflows".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    ranges.sort_unstable();
    let mut cursor = payload_start;
    for (start, end) in ranges {
        if start != cursor {
            let relation = if start < cursor {
                "overlaps"
            } else {
                "leaves a gap after"
            };
            return Err(InstallationDiagnostic(format!(
                "artifact section at {start}..{end} {relation} canonical payload cursor {cursor}"
            )));
        }
        cursor = end;
    }
    if cursor != total_length {
        return Err(InstallationDiagnostic(format!(
            "artifact sections end at {cursor}, leaving unreferenced bytes before declared length {total_length}"
        )));
    }
    Ok(())
}

fn decode_identity_payload<T>(
    bytes: &[u8],
    section: WireSection,
    name: &str,
    constructor: fn(u64) -> Result<T, InstallationDiagnostic>,
) -> Result<T, InstallationDiagnostic>
where
    T: Copy + PartialEq,
{
    if section.length != 8 {
        return Err(InstallationDiagnostic(format!(
            "{name} section must contain exactly one 8-byte normalized identity"
        )));
    }
    let decoded = decode_record(
        identity_layout(),
        identity_schema(),
        checked_slice(bytes, section.offset, section.length, name)?,
        name,
    )?;
    if decoded["identity"] != section.identity {
        return Err(InstallationDiagnostic(format!(
            "{name} section payload identity does not match its directory identity"
        )));
    }
    constructor(decoded["identity"])
}

fn decode_placement(
    bytes: &[u8],
    section: WireSection,
) -> Result<(PlacementPlanId, PlacementConstraints), InstallationDiagnostic> {
    if section.length != PLACEMENT_RECORD_BYTES {
        return Err(InstallationDiagnostic(format!(
            "placement section must contain exactly {PLACEMENT_RECORD_BYTES} bytes"
        )));
    }
    let decoded = decode_record(
        placement_layout(),
        placement_schema(),
        checked_slice(bytes, section.offset, section.length, "placement section")?,
        "placement section",
    )?;
    for field in ["reserved0", "reserved1"] {
        require_zero("placement reserved field", decoded[field])?;
    }
    if decoded["plan"] != section.identity {
        return Err(InstallationDiagnostic(
            "placement section payload identity does not match its directory identity".into(),
        ));
    }
    let placement_plan = PlacementPlanId::from_normalized_identity(decoded["plan"])?;
    let phase = match decoded["phase"] {
        1 => PlacementPhase::Build,
        2 => PlacementPhase::Load,
        3 => PlacementPhase::PostHandoff,
        value => {
            return Err(InstallationDiagnostic(format!(
                "unknown placement phase {value}"
            )));
        }
    };
    let range = decode_optional_pair(
        decoded["range_present"],
        decoded["range_start"],
        decoded["range_end"],
        "placement range",
        PlacementAddressRange::new,
    )?;
    let regime = decode_optional_identity(
        decoded["regime_present"],
        decoded["regime"],
        "machine regime",
        MachineRegimeId::from_normalized_identity,
    )?;
    let scope = decode_optional_identity(
        decoded["scope_present"],
        decoded["scope"],
        "installation scope",
        ArtifactInstallationScopeId::from_normalized_identity,
    )?;
    let constraints = PlacementConstraints::new(range, decoded["alignment"], phase, regime, scope)
        .map_err(|error| InstallationDiagnostic(error.0))?;
    Ok((placement_plan, constraints))
}

fn decode_entries(
    bytes: &[u8],
    section: WireSection,
    architecture: Architecture,
) -> Result<(EntrySetId, Vec<ArtifactEntry>), InstallationDiagnostic> {
    if !section.length.is_multiple_of(ENTRY_RECORD_BYTES) {
        return Err(InstallationDiagnostic(format!(
            "entry section length {} is not a multiple of {ENTRY_RECORD_BYTES}",
            section.length
        )));
    }
    let count = usize::try_from(section.length / ENTRY_RECORD_BYTES).map_err(|_| {
        InstallationDiagnostic("artifact entry count does not fit this compiler host".into())
    })?;
    if count == 0 {
        return Err(InstallationDiagnostic(
            "artifact entry section cannot be empty".into(),
        ));
    }
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let offset = section
            .offset
            .checked_add(
                (index as u64)
                    .checked_mul(ENTRY_RECORD_BYTES)
                    .ok_or_else(|| {
                        InstallationDiagnostic("artifact entry-record offset overflows".into())
                    })?,
            )
            .ok_or_else(|| {
                InstallationDiagnostic("artifact entry-record range overflows".into())
            })?;
        let decoded = decode_record(
            entry_layout(),
            entry_schema(),
            checked_slice(bytes, offset, ENTRY_RECORD_BYTES, "artifact entry record")?,
            "artifact entry record",
        )?;
        let identity = EntryStubId::from_normalized_identity(decoded["identity"])
            .map_err(|error| InstallationDiagnostic(error.0))?;
        if matches!(architecture, Architecture::Aarch64) && decoded["offset"] % 4 != 0 {
            return Err(InstallationDiagnostic(format!(
                "AArch64 artifact entry {:?} offset {} is not instruction-aligned",
                identity, decoded["offset"]
            )));
        }
        entries.push(ArtifactEntry::from_canonical_decode(
            identity,
            decoded["offset"],
        ));
    }
    Ok((
        EntrySetId::from_normalized_identity(section.identity)?,
        entries,
    ))
}

fn decode_relocations(
    bytes: &[u8],
    section: WireSection,
    max_relocations: usize,
) -> Result<(RelocationSetId, Vec<DecodedArtifactRelocation>), InstallationDiagnostic> {
    if section.length < RELOCATION_COUNT_BYTES {
        return Err(InstallationDiagnostic(
            "relocation section is missing its count record".into(),
        ));
    }
    let count_record = decode_record(
        identity_layout(),
        identity_schema(),
        checked_slice(
            bytes,
            section.offset,
            RELOCATION_COUNT_BYTES,
            "relocation count",
        )?,
        "relocation count",
    )?;
    let count = usize::try_from(count_record["identity"]).map_err(|_| {
        InstallationDiagnostic("artifact relocation count does not fit this compiler host".into())
    })?;
    if count > max_relocations {
        return Err(InstallationDiagnostic(format!(
            "artifact contains {count} relocations, exceeding configured bound {max_relocations}"
        )));
    }
    let expected = RELOCATION_COUNT_BYTES
        .checked_add(
            (count as u64)
                .checked_mul(RELOCATION_RECORD_BYTES)
                .ok_or_else(|| {
                    InstallationDiagnostic("artifact relocation section length overflows".into())
                })?,
        )
        .ok_or_else(|| {
            InstallationDiagnostic("artifact relocation section range overflows".into())
        })?;
    if section.length != expected {
        return Err(InstallationDiagnostic(format!(
            "relocation section length {} does not match {count} canonical records",
            section.length
        )));
    }
    let mut relocations = Vec::with_capacity(count);
    for index in 0..count {
        let offset = section
            .offset
            .checked_add(RELOCATION_COUNT_BYTES)
            .and_then(|start| {
                (index as u64)
                    .checked_mul(RELOCATION_RECORD_BYTES)
                    .and_then(|delta| start.checked_add(delta))
            })
            .ok_or_else(|| {
                InstallationDiagnostic("artifact relocation-record offset overflows".into())
            })?;
        let decoded = decode_record(
            relocation_layout(),
            relocation_schema(),
            checked_slice(
                bytes,
                offset,
                RELOCATION_RECORD_BYTES,
                "artifact relocation record",
            )?,
            "artifact relocation record",
        )?;
        require_zero("artifact relocation reserved", decoded["reserved"])?;
        let kind = match decoded["kind"] {
            1 => ArtifactRelocationKind::Absolute64,
            2 => ArtifactRelocationKind::X86Relative32,
            3 => ArtifactRelocationKind::Aarch64Page21,
            4 => ArtifactRelocationKind::Aarch64PageOffset12,
            5 => ArtifactRelocationKind::Aarch64Branch26,
            value => {
                return Err(InstallationDiagnostic(format!(
                    "unknown artifact relocation kind {value}"
                )));
            }
        };
        let target = match decoded["target_kind"] {
            1 => RelocationTarget::Entry(
                EntryStubId::from_normalized_identity(decoded["target"])
                    .map_err(|error| InstallationDiagnostic(error.0))?,
            ),
            2 => RelocationTarget::Data(
                DataSymbolId::from_normalized_identity(decoded["target"])
                    .map_err(|error| InstallationDiagnostic(error.0))?,
            ),
            value => {
                return Err(InstallationDiagnostic(format!(
                    "unknown artifact relocation target kind {value}"
                )));
            }
        };
        relocations.push(DecodedArtifactRelocation {
            kind,
            destination_offset: decoded["destination"],
            target,
            addend: decoded["addend"] as i64,
        });
    }
    Ok((
        RelocationSetId::from_normalized_identity(section.identity)?,
        relocations,
    ))
}

fn decode_optional_identity<T, E>(
    present: u64,
    identity: u64,
    name: &str,
    constructor: fn(u64) -> Result<T, E>,
) -> Result<Option<T>, InstallationDiagnostic>
where
    E: std::fmt::Debug,
{
    match present {
        0 if identity == 0 => Ok(None),
        0 => Err(InstallationDiagnostic(format!(
            "{name} identity must be zero when absent"
        ))),
        1 if identity != 0 => constructor(identity)
            .map(Some)
            .map_err(|_| InstallationDiagnostic(format!("{name} identity is invalid"))),
        1 => Err(InstallationDiagnostic(format!(
            "{name} identity cannot be zero when present"
        ))),
        value => Err(InstallationDiagnostic(format!(
            "{name} presence flag {value} is not boolean"
        ))),
    }
}

fn decode_optional_pair<T, E>(
    present: u64,
    first: u64,
    second: u64,
    name: &str,
    constructor: fn(u64, u64) -> Result<T, E>,
) -> Result<Option<T>, InstallationDiagnostic>
where
    E: std::fmt::Debug,
{
    match present {
        0 if first == 0 && second == 0 => Ok(None),
        0 => Err(InstallationDiagnostic(format!(
            "{name} values must be zero when absent"
        ))),
        1 => constructor(first, second)
            .map(Some)
            .map_err(|_| InstallationDiagnostic(format!("{name} is invalid"))),
        value => Err(InstallationDiagnostic(format!(
            "{name} presence flag {value} is not boolean"
        ))),
    }
}

fn checked_slice<'a>(
    bytes: &'a [u8],
    offset: u64,
    length: u64,
    label: &str,
) -> Result<&'a [u8], InstallationDiagnostic> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| InstallationDiagnostic(format!("{label} range overflows")))?;
    let start = usize::try_from(offset)
        .map_err(|_| InstallationDiagnostic(format!("{label} offset is not host-sized")))?;
    let end = usize::try_from(end)
        .map_err(|_| InstallationDiagnostic(format!("{label} end is not host-sized")))?;
    bytes.get(start..end).ok_or_else(|| {
        InstallationDiagnostic(format!(
            "{label} {offset}..{} exceeds {}-byte input",
            offset.saturating_add(length),
            bytes.len()
        ))
    })
}

fn checked_slice_mut<'a>(
    bytes: &'a mut [u8],
    offset: u64,
    length: u64,
    label: &str,
) -> Result<&'a mut [u8], InstallationDiagnostic> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| InstallationDiagnostic(format!("{label} range overflows")))?;
    let start = usize::try_from(offset)
        .map_err(|_| InstallationDiagnostic(format!("{label} offset is not host-sized")))?;
    let end = usize::try_from(end)
        .map_err(|_| InstallationDiagnostic(format!("{label} end is not host-sized")))?;
    let input_len = bytes.len();
    bytes.get_mut(start..end).ok_or_else(|| {
        InstallationDiagnostic(format!(
            "{label} {offset}..{} exceeds {input_len}-byte output",
            offset.saturating_add(length)
        ))
    })
}

fn require_zero(label: &str, value: u64) -> Result<(), InstallationDiagnostic> {
    if value != 0 {
        return Err(InstallationDiagnostic(format!(
            "{label} must be zero, found {value:#x}"
        )));
    }
    Ok(())
}

fn decode_record(
    layout: LayoutPlanReport,
    schema: Vec<ScalarFieldSchema>,
    bytes: &[u8],
    label: &str,
) -> Result<BTreeMap<String, u64>, InstallationDiagnostic> {
    let values = decode_scalar_layout(&layout, &schema, ByteOrder::LittleEndian, bytes)
        .map_err(|error| InstallationDiagnostic(format!("{label}: {}", error.0)))?;
    Ok(values
        .into_iter()
        .map(|value| (value.field, value.value))
        .collect())
}

fn encode_record(
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

fn scalar_layout(size: u64, fields: &[(&str, u64, u16)]) -> LayoutPlanReport {
    LayoutPlanReport {
        schema_identity: 1,
        entries: fields
            .iter()
            .map(|(field, offset, _)| LayoutFieldEntryReport {
                field: (*field).into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: *offset },
            })
            .collect(),
        offsets: Some(fields.iter().map(|(_, offset, _)| *offset).collect()),
        size: Some(size),
        align: 1,
    }
}

fn scalar_schema(fields: &[(&str, u64, u16)]) -> Vec<ScalarFieldSchema> {
    fields
        .iter()
        .map(|(field, _, width)| {
            ScalarFieldSchema::new(*field, *width).expect("static scalar schema is valid")
        })
        .collect()
}

const HEADER_FIELDS: &[(&str, u64, u16)] = &[
    ("magic", 0, 64),
    ("format_marker", 8, 16),
    ("header_bytes", 10, 16),
    ("architecture", 12, 8),
    ("reserved0", 13, 8),
    ("section_count", 14, 16),
    ("directory_offset", 16, 64),
    ("total_length", 24, 64),
    ("artifact", 32, 64),
    ("content", 40, 64),
    ("reserved1", 48, 64),
    ("reserved2", 56, 64),
];

fn header_layout() -> LayoutPlanReport {
    scalar_layout(OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES, HEADER_FIELDS)
}

fn header_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(HEADER_FIELDS)
}

const SECTION_FIELDS: &[(&str, u64, u16)] = &[
    ("kind", 0, 16),
    ("flags", 2, 16),
    ("reserved", 4, 32),
    ("identity", 8, 64),
    ("offset", 16, 64),
    ("length", 24, 64),
];

fn section_layout() -> LayoutPlanReport {
    scalar_layout(
        OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES,
        SECTION_FIELDS,
    )
}

fn section_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(SECTION_FIELDS)
}

const IDENTITY_FIELDS: &[(&str, u64, u16)] = &[("identity", 0, 64)];

fn identity_layout() -> LayoutPlanReport {
    scalar_layout(8, IDENTITY_FIELDS)
}

fn identity_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(IDENTITY_FIELDS)
}

const PLACEMENT_FIELDS: &[(&str, u64, u16)] = &[
    ("plan", 0, 64),
    ("range_present", 8, 8),
    ("phase", 9, 8),
    ("regime_present", 10, 8),
    ("scope_present", 11, 8),
    ("reserved0", 12, 32),
    ("range_start", 16, 64),
    ("range_end", 24, 64),
    ("alignment", 32, 64),
    ("regime", 40, 64),
    ("scope", 48, 64),
    ("reserved1", 56, 64),
];

fn placement_layout() -> LayoutPlanReport {
    scalar_layout(PLACEMENT_RECORD_BYTES, PLACEMENT_FIELDS)
}

fn placement_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(PLACEMENT_FIELDS)
}

const ENTRY_FIELDS: &[(&str, u64, u16)] = &[("identity", 0, 64), ("offset", 8, 64)];

fn entry_layout() -> LayoutPlanReport {
    scalar_layout(ENTRY_RECORD_BYTES, ENTRY_FIELDS)
}

fn entry_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(ENTRY_FIELDS)
}

const RELOCATION_FIELDS: &[(&str, u64, u16)] = &[
    ("kind", 0, 16),
    ("target_kind", 2, 16),
    ("reserved", 4, 32),
    ("destination", 8, 64),
    ("target", 16, 64),
    ("addend", 24, 64),
];

fn relocation_layout() -> LayoutPlanReport {
    scalar_layout(RELOCATION_RECORD_BYTES, RELOCATION_FIELDS)
}

fn relocation_schema() -> Vec<ScalarFieldSchema> {
    scalar_schema(RELOCATION_FIELDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> ContainerLimits {
        ContainerLimits {
            max_total_bytes: 4096,
            max_sections: 16,
            max_section_bytes: 1024,
            max_relocations: 16,
        }
    }

    fn write_record(destination: &mut [u8], layout: LayoutPlanReport, values: &[(&str, u16, u64)]) {
        encode_record(destination, layout, values, "test record")
            .expect("test record materializes");
    }

    fn canonical_bytes() -> Vec<u8> {
        let section_count = 7_u64;
        let directory_end = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES
            + section_count * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES;
        let code_offset = directory_end;
        let relocation_offset = code_offset + 64;
        let contracts_offset = relocation_offset + 40;
        let footprint_offset = contracts_offset + 8;
        let placement_offset = footprint_offset + 8;
        let entries_offset = placement_offset + PLACEMENT_RECORD_BYTES;
        let proof_offset = entries_offset + ENTRY_RECORD_BYTES;
        let total_length = proof_offset + 64;

        let contracts = MachineContractSetId::from_normalized_identity(3).unwrap();
        let footprint = MachineFootprintId::from_normalized_identity(4).unwrap();
        let placement = PlacementPlanId::from_normalized_identity(5).unwrap();
        let relocation_set = RelocationSetId::from_normalized_identity(6).unwrap();
        let entry_set = EntrySetId::from_normalized_identity(8).unwrap();
        let entry = EntryStubId::from_normalized_identity(9).unwrap();
        let proof = vec![0xa5; 64];
        let mut decoded =
            DecodedArtifactContainer {
                format_marker: OMEGA_EXECUTABLE_CONTAINER_MARKER,
                total_length,
                artifact: ArtifactId::from_normalized_identity(1).unwrap(),
                content_fingerprint:
                    NonAuthoritativeContainerFingerprint64::from_compatibility_value(2).unwrap(),
                architecture: Architecture::X86_64,
                code_length: 64,
                code: vec![0x90; 64],
                contracts,
                declared_footprint: footprint,
                placement_plan: placement,
                placement_constraints: PlacementConstraints::unconstrained(PlacementPhase::Load),
                entry_set,
                entries: vec![ArtifactEntry::from_canonical_decode(entry, 16)],
                relocation_set,
                relocations: vec![DecodedArtifactRelocation {
                    kind: ArtifactRelocationKind::X86Relative32,
                    destination_offset: 32,
                    target: RelocationTarget::Entry(entry),
                    addend: -4,
                }],
                proof_payload: normalized_proof_payload_digest(&proof),
                proof,
                sections: Vec::new(),
            };
        decoded.content_fingerprint =
            non_authoritative_decoded_container_fingerprint(&decoded).unwrap();

        let mut bytes = vec![0_u8; total_length as usize];
        write_record(
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
                    u64::from(OMEGA_EXECUTABLE_CONTAINER_MARKER),
                ),
                ("header_bytes", 16, OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES),
                ("architecture", 8, 2),
                ("reserved0", 8, 0),
                ("section_count", 16, section_count),
                (
                    "directory_offset",
                    64,
                    OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES,
                ),
                ("total_length", 64, total_length),
                ("artifact", 64, 1),
                (
                    "content",
                    64,
                    decoded.content_fingerprint.compatibility_value(),
                ),
                ("reserved1", 64, 0),
                ("reserved2", 64, 0),
            ],
        );
        let sections = [
            (SECTION_CODE, 1, 0, code_offset, 64),
            (
                SECTION_RELOCATIONS,
                1,
                relocation_set.normalized_identity(),
                relocation_offset,
                40,
            ),
            (
                SECTION_CONTRACTS,
                1,
                contracts.normalized_identity(),
                contracts_offset,
                8,
            ),
            (
                SECTION_FOOTPRINT,
                1,
                footprint.normalized_identity(),
                footprint_offset,
                8,
            ),
            (
                SECTION_PLACEMENT,
                1,
                placement.normalized_identity(),
                placement_offset,
                PLACEMENT_RECORD_BYTES,
            ),
            (
                SECTION_ENTRIES,
                1,
                entry_set.normalized_identity(),
                entries_offset,
                ENTRY_RECORD_BYTES,
            ),
            (SECTION_PROOF, 1, 0, proof_offset, 64),
        ];
        for (index, (kind, flags, identity, offset, length)) in sections.iter().enumerate() {
            let start = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize
                + index * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize;
            let end = start + OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize;
            write_record(
                &mut bytes[start..end],
                section_layout(),
                &[
                    ("kind", 16, u64::from(*kind)),
                    ("flags", 16, *flags),
                    ("reserved", 32, 0),
                    ("identity", 64, *identity),
                    ("offset", 64, *offset),
                    ("length", 64, *length),
                ],
            );
        }
        bytes[code_offset as usize..relocation_offset as usize].fill(0x90);
        write_record(
            &mut bytes[relocation_offset as usize..relocation_offset as usize + 8],
            identity_layout(),
            &[("identity", 64, 1)],
        );
        write_record(
            &mut bytes[relocation_offset as usize + 8..contracts_offset as usize],
            relocation_layout(),
            &[
                ("kind", 16, 2),
                ("target_kind", 16, 1),
                ("reserved", 32, 0),
                ("destination", 64, 32),
                ("target", 64, 9),
                ("addend", 64, (-4_i64) as u64),
            ],
        );
        write_record(
            &mut bytes[contracts_offset as usize..footprint_offset as usize],
            identity_layout(),
            &[("identity", 64, contracts.normalized_identity())],
        );
        write_record(
            &mut bytes[footprint_offset as usize..placement_offset as usize],
            identity_layout(),
            &[("identity", 64, footprint.normalized_identity())],
        );
        write_record(
            &mut bytes[placement_offset as usize..entries_offset as usize],
            placement_layout(),
            &[
                ("plan", 64, placement.normalized_identity()),
                ("range_present", 8, 0),
                ("phase", 8, 2),
                ("regime_present", 8, 0),
                ("scope_present", 8, 0),
                ("reserved0", 32, 0),
                ("range_start", 64, 0),
                ("range_end", 64, 0),
                ("alignment", 64, 1),
                ("regime", 64, 0),
                ("scope", 64, 0),
                ("reserved1", 64, 0),
            ],
        );
        write_record(
            &mut bytes[entries_offset as usize..proof_offset as usize],
            entry_layout(),
            &[("identity", 64, 9), ("offset", 64, 16)],
        );
        bytes[proof_offset as usize..].fill(0xa5);
        bytes
    }

    fn add_optional_section(mut bytes: Vec<u8>, kind: u16, payload: &[u8]) -> Vec<u8> {
        let old_section_count = 7_usize;
        let inserted_directory_offset = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize
            + old_section_count * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize;
        bytes.splice(
            inserted_directory_offset..inserted_directory_offset,
            std::iter::repeat_n(0, OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize),
        );
        for index in 0..old_section_count {
            let record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize
                + index * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize;
            let offset = u64::from_le_bytes(bytes[record + 16..record + 24].try_into().unwrap());
            bytes[record + 16..record + 24].copy_from_slice(
                &(offset + OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES).to_le_bytes(),
            );
        }
        let payload_offset = bytes.len() as u64;
        bytes.extend_from_slice(payload);
        bytes[14..16].copy_from_slice(&8_u16.to_le_bytes());
        let total_length = bytes.len() as u64;
        bytes[24..32].copy_from_slice(&total_length.to_le_bytes());
        let identity = non_authoritative_informational_section_fingerprint(kind, payload)
            .compatibility_value();
        write_record(
            &mut bytes[inserted_directory_offset
                ..inserted_directory_offset
                    + OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize],
            section_layout(),
            &[
                ("kind", 16, u64::from(kind)),
                ("flags", 16, 0),
                ("reserved", 32, 0),
                ("identity", 64, identity),
                ("offset", 64, payload_offset),
                ("length", 64, payload.len() as u64),
            ],
        );
        bytes
    }

    #[test]
    fn canonical_bytes_decode_through_layouts_into_validated_candidate() {
        let bytes = canonical_bytes();
        let decoded = decode_executable_container(&bytes, limits()).expect("canonical decode");
        assert_eq!(decoded.artifact().code(), vec![0x90; 64]);
        assert_eq!(decoded.artifact().entries()[0].code_offset(), 16);
        assert_eq!(decoded.relocations()[0].addend, -4);
        assert_eq!(decoded.proof(), vec![0xa5; 64]);
    }

    #[test]
    fn stale_container_marker_bytes_reject() {
        let mut stale = canonical_bytes();
        stale[8..10].copy_from_slice(b"NO");
        let error = decode_executable_container(&stale, limits()).expect_err("stale marker");
        assert!(
            error
                .0
                .contains("unsupported Omega executable container marker")
        );
    }

    #[test]
    fn canonical_encoder_round_trips_the_exact_validated_artifact_and_proof() {
        let canonical = canonical_bytes();
        let source = decode_executable_container(&canonical, limits()).expect("source");
        let encoded = encode_executable_container(source.artifact(), source.proof(), limits())
            .expect("encode");
        assert_eq!(
            encoded, canonical,
            "container-v1 wire bytes must remain stable"
        );
        assert_eq!(
            u64::from_le_bytes(encoded[40..48].try_into().unwrap()),
            source
                .artifact()
                .non_authoritative_container_fingerprint()
                .compatibility_value()
        );
        let decoded = decode_executable_container(&encoded, limits()).expect("round trip");
        assert_eq!(decoded.artifact(), source.artifact());
        assert_eq!(decoded.proof(), source.proof());
        assert_eq!(decoded.proof_payload(), source.proof_payload());
    }

    #[test]
    fn optional_section_trace_identity_is_derived_from_exact_opaque_bytes() {
        let known = add_optional_section(canonical_bytes(), SECTION_INFORMATIONAL, b"debug");
        let decoded = decode_executable_container(&known, limits()).expect("known information");
        assert_eq!(
            decoded.informational_sections()[0].compatibility_value(),
            non_authoritative_informational_section_fingerprint(SECTION_INFORMATIONAL, b"debug")
                .compatibility_value()
        );

        let unknown = add_optional_section(canonical_bytes(), 99, b"future");
        let decoded = decode_executable_container(&unknown, limits()).expect("unknown information");
        assert_eq!(
            decoded.unknown_informational_sections(),
            &[
                non_authoritative_informational_section_fingerprint(99, b"future")
                    .compatibility_value()
            ]
        );

        let mut substituted = known;
        *substituted.last_mut().expect("payload byte") ^= 1;
        let error =
            decode_executable_container(&substituted, limits()).expect_err("identity replay");
        assert!(error.0.contains("does not match its exact opaque bytes"));
    }

    #[test]
    fn truncation_directory_overlap_and_bad_reserved_bits_reject() {
        let mut truncated = canonical_bytes();
        truncated.pop();
        let error = decode_executable_container(&truncated, limits()).expect_err("truncation");
        assert!(error.0.contains("declares"));

        let mut prefix_overlap = canonical_bytes();
        let first_section = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize;
        prefix_overlap[first_section + 16..first_section + 24]
            .copy_from_slice(&64_u64.to_le_bytes());
        let error =
            decode_executable_container(&prefix_overlap, limits()).expect_err("prefix overlap");
        assert!(error.0.contains("header/directory"));

        let mut reserved = canonical_bytes();
        reserved[13] = 1;
        let error = decode_executable_container(&reserved, limits()).expect_err("reserved");
        assert!(error.0.contains("reserved0"));
    }

    #[test]
    fn payload_gaps_and_unreferenced_trailing_bytes_reject() {
        let mut gap = canonical_bytes();
        let code_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize;
        let code_offset =
            u64::from_le_bytes(gap[code_record + 16..code_record + 24].try_into().unwrap());
        gap[code_record + 16..code_record + 24].copy_from_slice(&(code_offset + 1).to_le_bytes());
        gap[code_record + 24..code_record + 32].copy_from_slice(&63_u64.to_le_bytes());
        let error = decode_executable_container(&gap, limits()).expect_err("payload gap");
        assert!(error.0.contains("leaves a gap"));

        let mut trailing = canonical_bytes();
        let proof_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize + 6 * 32;
        trailing[proof_record + 24..proof_record + 32].copy_from_slice(&63_u64.to_le_bytes());
        let error =
            decode_executable_container(&trailing, limits()).expect_err("unreferenced trailing");
        assert!(error.0.contains("unreferenced bytes"));
    }

    #[test]
    fn malformed_counts_unknown_required_and_identity_drift_reject() {
        let mut count = canonical_bytes();
        let relocation_section_offset = 64 + 32;
        let relocation_payload = u64::from_le_bytes(
            count[relocation_section_offset + 16..relocation_section_offset + 24]
                .try_into()
                .unwrap(),
        ) as usize;
        count[relocation_payload..relocation_payload + 8].copy_from_slice(&2_u64.to_le_bytes());
        let error = decode_executable_container(&count, limits()).expect_err("count mismatch");
        assert!(error.0.contains("does not match 2"));

        let mut unknown = canonical_bytes();
        let code_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize;
        unknown[code_record..code_record + 2].copy_from_slice(&99_u16.to_le_bytes());
        unknown[code_record + 8..code_record + 16].copy_from_slice(&99_u64.to_le_bytes());
        let error = decode_executable_container(&unknown, limits()).expect_err("unknown required");
        assert!(error.0.contains("unknown required"));

        let mut identity = canonical_bytes();
        let contracts_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize + 2 * 32;
        identity[contracts_record + 8..contracts_record + 16]
            .copy_from_slice(&99_u64.to_le_bytes());
        let error = decode_executable_container(&identity, limits()).expect_err("identity drift");
        assert!(error.0.contains("payload identity"));
    }

    #[test]
    fn semantic_byte_drift_reaches_normalized_content_check() {
        let mut bytes = canonical_bytes();
        let directory_end = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES
            + 7 * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES;
        bytes[directory_end as usize] ^= 1;
        let error = decode_executable_container(&bytes, limits()).expect_err("content drift");
        assert!(error.0.contains("content fingerprint"));
    }
}
