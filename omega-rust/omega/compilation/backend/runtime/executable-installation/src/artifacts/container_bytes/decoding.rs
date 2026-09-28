//! Decoding and validating an executable container.

use crate::artifacts::ArtifactEntry;
use crate::artifacts::container::{
    ArtifactRelocationKind, ContainerLimits, ContainerSection, ContainerSectionKind,
    DecodedArtifactContainer, DecodedArtifactRelocation, OMEGA_EXECUTABLE_CONTAINER_V1_MARKER,
    OMEGA_EXECUTABLE_CONTAINER_V2_MARKER, ValidatedArtifactContainer,
    normalized_proof_payload_digest, validate_decoded_container,
};
use crate::artifacts::container_bytes::record_layouts::{
    entry_layout, entry_schema, header_layout, header_schema, identity_layout, identity_schema,
    placement_layout, placement_schema, relocation_layout, relocation_schema, section_layout,
    section_schema,
};
use crate::artifacts::container_bytes::{
    AUTHORITY_COMMITMENT_BYTES, ENTRY_RECORD_BYTES, OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES,
    OMEGA_EXECUTABLE_CONTAINER_MAGIC, OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES,
    PLACEMENT_RECORD_BYTES, RELOCATION_COUNT_BYTES, RELOCATION_RECORD_BYTES,
    SECTION_AUTHORITY_COMMITMENTS, SECTION_CODE, SECTION_CONTRACTS, SECTION_ENTRIES,
    SECTION_FOOTPRINT, SECTION_INFORMATIONAL, SECTION_PLACEMENT, SECTION_PROOF,
    SECTION_RELOCATIONS, WireSection, non_authoritative_informational_section_fingerprint,
};
use crate::authority_digests::ArtifactAuthorityCommitments;
use crate::authority_digests::{
    ArtifactId, EntrySetId, MachineContractSetId, MachineFootprintId,
    NonAuthoritativeContainerFingerprint64, NonAuthoritativeInformationalFingerprint64,
    PlacementPlanId, RelocationSetId,
};
use crate::installation::InstallationDiagnostic;
use layout_plans::{
    ArtifactInstallationScopeId, ByteOrder, DataSymbolId, LayoutPlanReport, MachineRegimeId,
    PlacementAddressRange, PlacementConstraints, PlacementPhase, ScalarFieldSchema,
    decode_scalar_layout,
};
use layout_plans::{EntryStubId, RelocationTarget};
use std::collections::BTreeMap;
use target::Architecture;

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
    let format_marker =
        u16::try_from(header["format_marker"]).expect("16-bit executable-container marker field");
    if !matches!(
        format_marker,
        OMEGA_EXECUTABLE_CONTAINER_V1_MARKER | OMEGA_EXECUTABLE_CONTAINER_V2_MARKER
    ) {
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
        validate_known_section_flags(kind, required, format_marker)?;
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
        if !is_known_section_kind(section.kind) && section.required {
            return Err(InstallationDiagnostic(format!(
                "unknown required artifact section {}",
                section.identity
            )));
        }
        if section.kind == SECTION_INFORMATIONAL
            || (section.kind > SECTION_INFORMATIONAL
                && section.kind != SECTION_AUTHORITY_COMMITMENTS)
        {
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
    let authority_section = match format_marker {
        OMEGA_EXECUTABLE_CONTAINER_V1_MARKER => {
            if wire_sections
                .iter()
                .any(|section| section.kind == SECTION_AUTHORITY_COMMITMENTS)
            {
                return Err(InstallationDiagnostic(
                    "container-v1 cannot carry a v2 authority-commitment section".into(),
                ));
            }
            None
        }
        OMEGA_EXECUTABLE_CONTAINER_V2_MARKER => Some(only_wire_section(
            &wire_sections,
            SECTION_AUTHORITY_COMMITMENTS,
            "authority-commitment",
        )?),
        _ => unreachable!("format marker checked above"),
    };

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
    let authority_commitments = authority_section
        .map(|section| {
            decode_authority_commitments(
                bytes,
                section,
                contracts,
                declared_footprint,
                placement_constraints,
            )
        })
        .transpose()?;

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
                SECTION_AUTHORITY_COMMITMENTS => ContainerSectionKind::AuthorityCommitments(
                    authority_commitments
                        .expect("v2 authority section decoded before section construction"),
                ),
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
            format_marker,
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
            authority_commitments,
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

fn validate_known_section_flags(
    kind: u16,
    required: bool,
    format_marker: u16,
) -> Result<(), InstallationDiagnostic> {
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
        SECTION_AUTHORITY_COMMITMENTS
            if format_marker == OMEGA_EXECUTABLE_CONTAINER_V2_MARKER && !required =>
        {
            Err(InstallationDiagnostic(
                "container-v2 authority commitments must be required".into(),
            ))
        }
        _ => Ok(()),
    }
}

fn is_known_section_kind(kind: u16) -> bool {
    matches!(
        kind,
        SECTION_CODE
            | SECTION_RELOCATIONS
            | SECTION_CONTRACTS
            | SECTION_FOOTPRINT
            | SECTION_PLACEMENT
            | SECTION_ENTRIES
            | SECTION_PROOF
            | SECTION_INFORMATIONAL
            | SECTION_AUTHORITY_COMMITMENTS
    )
}

fn validate_wire_section_identity(section: WireSection) -> Result<(), InstallationDiagnostic> {
    match section.kind {
        SECTION_CODE | SECTION_PROOF | SECTION_AUTHORITY_COMMITMENTS if section.identity != 0 => {
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
        kind if kind > SECTION_INFORMATIONAL
            && kind != SECTION_AUTHORITY_COMMITMENTS
            && section.identity == 0 =>
        {
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

fn decode_authority_commitments(
    bytes: &[u8],
    section: WireSection,
    contracts: MachineContractSetId,
    footprint: MachineFootprintId,
    placement: PlacementConstraints,
) -> Result<ArtifactAuthorityCommitments, InstallationDiagnostic> {
    if section.length != AUTHORITY_COMMITMENT_BYTES {
        return Err(InstallationDiagnostic(format!(
            "authority-commitment section must contain exactly {AUTHORITY_COMMITMENT_BYTES} bytes"
        )));
    }
    let payload = checked_slice(
        bytes,
        section.offset,
        section.length,
        "authority-commitment section",
    )?;
    ArtifactAuthorityCommitments::from_decoded_digests(
        contracts,
        footprint,
        placement,
        payload[..32].try_into().expect("32-byte contract digest"),
        payload[32..64]
            .try_into()
            .expect("32-byte footprint digest"),
        payload[64..96].try_into().expect("32-byte regime digest"),
        payload[96..128].try_into().expect("32-byte scope digest"),
    )
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

pub(crate) fn checked_slice_mut<'a>(
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
