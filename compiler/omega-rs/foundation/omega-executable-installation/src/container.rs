use super::*;

pub const OMEGA_EXECUTABLE_CONTAINER_VERSION: u16 = 2;
const CANONICAL_ENTRY_RECORD_BYTES: u64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerLimits {
    pub max_total_bytes: u64,
    pub max_sections: usize,
    pub max_section_bytes: u64,
    pub max_relocations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerSection {
    pub kind: ContainerSectionKind,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerSectionKind {
    Code,
    Relocations(RelocationSetId),
    Contracts(MachineContractSetId),
    Footprint(MachineFootprintId),
    Placement(PlacementPlanId),
    Entries(EntrySetId),
    Proof(ProofPayloadId),
    Informational(InformationalSectionId),
    /// An unrecognized optional section is informational by definition. It
    /// cannot supply an identity used by admission.
    Unknown {
        identity: u64,
        required: bool,
    },
}

/// Closed semantic relocation vocabulary accepted from the checked artifact
/// schema. These are normalized relocation meanings, not object-format
/// record numbers; the target writer translates them to its native format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactRelocationKind {
    Absolute64,
    X86Relative32,
    Aarch64Page21,
    Aarch64PageOffset12,
    Aarch64Branch26,
}

impl ArtifactRelocationKind {
    const fn byte_width(self) -> u64 {
        match self {
            Self::Absolute64 => 8,
            Self::X86Relative32
            | Self::Aarch64Page21
            | Self::Aarch64PageOffset12
            | Self::Aarch64Branch26 => 4,
        }
    }
}

/// One checked-schema relocation record before semantic validation. The
/// target remains symbolic: decoding an artifact never exposes a numeric code
/// or data address to source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedArtifactRelocation {
    pub kind: ArtifactRelocationKind,
    pub destination_offset: u64,
    pub target: RelocationTarget,
    pub addend: i64,
}

/// Output of checked schema/layout decoding, before semantic container
/// validation. This type deliberately has no scripts, constructors, imports,
/// or recursive section form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedArtifactContainer {
    pub format_version: u16,
    pub total_length: u64,
    pub artifact: ArtifactId,
    pub content: ArtifactContentId,
    pub code_length: u64,
    pub contracts: MachineContractSetId,
    pub declared_footprint: MachineFootprintId,
    pub placement_plan: PlacementPlanId,
    /// Checked decode of the canonical placement section. Its identity and
    /// normalized constraints are both bound into artifact admission.
    pub placement_constraints: PlacementConstraints,
    /// Checked decode of the compiler-selected entry set. Entry identities are
    /// sealed materialization symbols; offsets are interpreted only by the
    /// installation/provider layer.
    pub entry_set: EntrySetId,
    pub entries: Vec<ArtifactEntry>,
    pub relocation_set: RelocationSetId,
    pub relocations: Vec<DecodedArtifactRelocation>,
    pub proof_payload: ProofPayloadId,
    pub sections: Vec<ContainerSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedArtifactContainer {
    artifact: Artifact,
    relocation_set: RelocationSetId,
    relocations: Vec<DecodedArtifactRelocation>,
    proof_payload: ProofPayloadId,
    informational_sections: Vec<InformationalSectionId>,
    unknown_informational_sections: Vec<u64>,
}

impl ValidatedArtifactContainer {
    pub const fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    pub const fn relocation_set(&self) -> RelocationSetId {
        self.relocation_set
    }

    /// Canonical destination-order relocation records. Their symbolic targets
    /// remain sealed identities; this projection grants no resolver.
    pub fn relocations(&self) -> &[DecodedArtifactRelocation] {
        &self.relocations
    }

    pub const fn proof_payload(&self) -> ProofPayloadId {
        self.proof_payload
    }

    pub fn informational_sections(&self) -> &[InformationalSectionId] {
        &self.informational_sections
    }

    pub fn unknown_informational_sections(&self) -> &[u64] {
        &self.unknown_informational_sections
    }
}

pub fn validate_decoded_container(
    decoded: DecodedArtifactContainer,
    limits: ContainerLimits,
) -> Result<ValidatedArtifactContainer, InstallationDiagnostic> {
    if limits.max_total_bytes == 0
        || limits.max_sections == 0
        || limits.max_section_bytes == 0
        || limits.max_relocations == 0
    {
        return Err(InstallationDiagnostic(
            "artifact-container limits must all be nonzero".into(),
        ));
    }
    if decoded.format_version != OMEGA_EXECUTABLE_CONTAINER_VERSION {
        return Err(InstallationDiagnostic(format!(
            "unsupported Omega executable container version {}",
            decoded.format_version
        )));
    }
    if decoded.total_length == 0 || decoded.total_length > limits.max_total_bytes {
        return Err(InstallationDiagnostic(format!(
            "artifact container length {} exceeds configured bound {}",
            decoded.total_length, limits.max_total_bytes
        )));
    }
    if decoded.sections.len() > limits.max_sections {
        return Err(InstallationDiagnostic(format!(
            "artifact container has {} sections, exceeding configured bound {}",
            decoded.sections.len(),
            limits.max_sections
        )));
    }

    let mut ranges = Vec::with_capacity(decoded.sections.len());
    let mut code = 0;
    let mut relocations = 0;
    let mut contracts = 0;
    let mut footprint = 0;
    let mut placement = 0;
    let mut entries = 0;
    let mut proof = 0;
    let mut informational = Vec::new();
    let mut unknown_informational = Vec::new();
    for section in &decoded.sections {
        if section.length == 0 || section.length > limits.max_section_bytes {
            return Err(InstallationDiagnostic(format!(
                "artifact section length {} is empty or exceeds configured bound {}",
                section.length, limits.max_section_bytes
            )));
        }
        let end = section
            .offset
            .checked_add(section.length)
            .ok_or_else(|| InstallationDiagnostic("artifact section range overflows".into()))?;
        if end > decoded.total_length {
            return Err(InstallationDiagnostic(format!(
                "artifact section {}..{} exceeds {}-byte container",
                section.offset, end, decoded.total_length
            )));
        }
        ranges.push((section.offset, end));

        match section.kind {
            ContainerSectionKind::Code => {
                code += 1;
                if section.length != decoded.code_length {
                    return Err(InstallationDiagnostic(
                        "code section length does not match canonical header".into(),
                    ));
                }
            }
            ContainerSectionKind::Relocations(identity) => {
                relocations += 1;
                require_identity("relocation", identity, decoded.relocation_set)?;
            }
            ContainerSectionKind::Contracts(identity) => {
                contracts += 1;
                require_identity("contract", identity, decoded.contracts)?;
            }
            ContainerSectionKind::Footprint(identity) => {
                footprint += 1;
                require_identity("footprint", identity, decoded.declared_footprint)?;
            }
            ContainerSectionKind::Placement(identity) => {
                placement += 1;
                require_identity("placement", identity, decoded.placement_plan)?;
            }
            ContainerSectionKind::Entries(identity) => {
                entries += 1;
                require_identity("entry-set", identity, decoded.entry_set)?;
                let entry_count = u64::try_from(decoded.entries.len()).map_err(|_| {
                    InstallationDiagnostic(
                        "artifact entry count cannot be represented by the container".into(),
                    )
                })?;
                let expected_length = entry_count
                    .checked_mul(CANONICAL_ENTRY_RECORD_BYTES)
                    .ok_or_else(|| {
                        InstallationDiagnostic("artifact entry-set length overflows".into())
                    })?;
                if section.length != expected_length {
                    return Err(InstallationDiagnostic(format!(
                        "entry-set section length {} does not match {} canonical entries",
                        section.length,
                        decoded.entries.len()
                    )));
                }
            }
            ContainerSectionKind::Proof(identity) => {
                proof += 1;
                require_identity("proof", identity, decoded.proof_payload)?;
            }
            ContainerSectionKind::Informational(identity) => informational.push(identity),
            ContainerSectionKind::Unknown { identity, required } => {
                if required {
                    return Err(InstallationDiagnostic(format!(
                        "unknown required artifact section {identity}"
                    )));
                }
                unknown_informational.push(identity);
            }
        }
    }

    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(InstallationDiagnostic(
                "artifact container sections overlap".into(),
            ));
        }
    }
    for (name, count) in [
        ("code", code),
        ("relocations", relocations),
        ("contracts", contracts),
        ("footprint", footprint),
        ("placement", placement),
        ("entry-set", entries),
        ("proof", proof),
    ] {
        if count != 1 {
            return Err(InstallationDiagnostic(format!(
                "artifact container requires exactly one {name} section, found {count}"
            )));
        }
    }

    let relocations = validate_decoded_relocations(
        decoded.relocations,
        decoded.code_length,
        limits.max_relocations,
    )?;
    let artifact = Artifact::from_canonical_decode(
        decoded.artifact,
        decoded.content,
        decoded.code_length,
        decoded.contracts,
        decoded.declared_footprint,
        decoded.placement_plan,
        decoded.placement_constraints,
        decoded.entry_set,
        decoded.entries,
    )?;
    Ok(ValidatedArtifactContainer {
        artifact,
        relocation_set: decoded.relocation_set,
        relocations,
        proof_payload: decoded.proof_payload,
        informational_sections: informational,
        unknown_informational_sections: unknown_informational,
    })
}

fn validate_decoded_relocations(
    mut relocations: Vec<DecodedArtifactRelocation>,
    code_length: u64,
    max_relocations: usize,
) -> Result<Vec<DecodedArtifactRelocation>, InstallationDiagnostic> {
    if relocations.len() > max_relocations {
        return Err(InstallationDiagnostic(format!(
            "artifact has {} relocations, exceeding configured bound {}",
            relocations.len(),
            max_relocations
        )));
    }

    relocations.sort_unstable_by_key(|relocation| {
        (
            relocation.destination_offset,
            relocation.kind,
            relocation.target,
            relocation.addend,
        )
    });
    let mut prior_end = 0u64;
    for (index, relocation) in relocations.iter().enumerate() {
        let width = relocation.kind.byte_width();
        let end = relocation
            .destination_offset
            .checked_add(width)
            .ok_or_else(|| InstallationDiagnostic("artifact relocation range overflows".into()))?;
        if end > code_length {
            return Err(InstallationDiagnostic(format!(
                "artifact relocation at {}..{} exceeds {}-byte code section",
                relocation.destination_offset, end, code_length
            )));
        }
        if index != 0 && relocation.destination_offset < prior_end {
            return Err(InstallationDiagnostic(format!(
                "artifact relocation at byte {} overlaps another relocation field",
                relocation.destination_offset
            )));
        }
        prior_end = end;
    }
    Ok(relocations)
}

fn require_identity<T: Copy + PartialEq>(
    name: &str,
    actual: T,
    expected: T,
) -> Result<(), InstallationDiagnostic> {
    if actual != expected {
        return Err(InstallationDiagnostic(format!(
            "{name} section identity does not match canonical header"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn limits() -> ContainerLimits {
        ContainerLimits {
            max_total_bytes: 4096,
            max_sections: 16,
            max_section_bytes: 1024,
            max_relocations: 16,
        }
    }

    fn decoded() -> DecodedArtifactContainer {
        let contracts = id(3, MachineContractSetId::from_normalized_identity);
        let footprint = id(4, MachineFootprintId::from_normalized_identity);
        let placement = id(5, PlacementPlanId::from_normalized_identity);
        let relocations = id(6, RelocationSetId::from_normalized_identity);
        let proof = id(7, ProofPayloadId::from_normalized_identity);
        let entry_set = id(8, EntrySetId::from_normalized_identity);
        let entry = EntryStubId::from_normalized_identity(9).expect("entry identity");
        DecodedArtifactContainer {
            format_version: OMEGA_EXECUTABLE_CONTAINER_VERSION,
            total_length: 400,
            artifact: id(1, ArtifactId::from_normalized_identity),
            content: id(2, ArtifactContentId::from_normalized_identity),
            code_length: 64,
            contracts,
            declared_footprint: footprint,
            placement_plan: placement,
            placement_constraints: PlacementConstraints::unconstrained(
                omega_layout_plans::PlacementPhase::Load,
            ),
            entry_set,
            entries: vec![ArtifactEntry::from_canonical_decode(entry, 16)],
            relocation_set: relocations,
            relocations: vec![DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::X86Relative32,
                destination_offset: 32,
                target: RelocationTarget::Entry(entry),
                addend: -4,
            }],
            proof_payload: proof,
            sections: vec![
                ContainerSection {
                    kind: ContainerSectionKind::Code,
                    offset: 0,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Relocations(relocations),
                    offset: 64,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Contracts(contracts),
                    offset: 128,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Footprint(footprint),
                    offset: 192,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Placement(placement),
                    offset: 256,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Entries(entry_set),
                    offset: 320,
                    length: 16,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Proof(proof),
                    offset: 336,
                    length: 64,
                },
            ],
        }
    }

    #[test]
    fn canonical_bounded_container_produces_only_an_artifact_candidate() {
        let entry = EntryStubId::from_normalized_identity(9).expect("entry identity");
        let container = validate_decoded_container(decoded(), limits()).expect("container");
        assert_eq!(container.artifact().identity().normalized_identity(), 1);
        assert_eq!(container.artifact().byte_length(), 64);
        assert_eq!(container.artifact().entries().len(), 1);
        assert_eq!(container.artifact().entries()[0].identity(), entry);
        assert_eq!(container.artifact().entries()[0].code_offset(), 16);
        assert_eq!(
            container.relocations(),
            &[DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::X86Relative32,
                destination_offset: 32,
                target: RelocationTarget::Entry(entry),
                addend: -4,
            }]
        );
        assert_eq!(
            container.artifact().placement_constraints(),
            PlacementConstraints::unconstrained(omega_layout_plans::PlacementPhase::Load)
        );
    }

    #[test]
    fn unknown_required_rejects_while_unknown_optional_is_informational() {
        let mut optional = decoded();
        optional.total_length = 464;
        optional.sections.push(ContainerSection {
            kind: ContainerSectionKind::Unknown {
                identity: 99,
                required: false,
            },
            offset: 400,
            length: 64,
        });
        let container =
            validate_decoded_container(optional, limits()).expect("optional information");
        assert_eq!(container.unknown_informational_sections(), &[99]);

        let mut required = decoded();
        required.total_length = 464;
        required.sections.push(ContainerSection {
            kind: ContainerSectionKind::Unknown {
                identity: 99,
                required: true,
            },
            offset: 400,
            length: 64,
        });
        let error = validate_decoded_container(required, limits()).expect_err("required unknown");
        assert!(error.0.contains("unknown required"));
    }

    #[test]
    fn duplicate_missing_overlapping_and_out_of_bounds_sections_reject() {
        let mut duplicate = decoded();
        duplicate.total_length = 464;
        duplicate.sections.push(ContainerSection {
            kind: ContainerSectionKind::Code,
            offset: 400,
            length: 64,
        });
        let error = validate_decoded_container(duplicate, limits()).expect_err("duplicate code");
        assert!(error.0.contains("exactly one code"));

        let mut missing = decoded();
        missing.sections.pop();
        let error = validate_decoded_container(missing, limits()).expect_err("missing proof");
        assert!(error.0.contains("exactly one proof"));

        let mut overlapping = decoded();
        overlapping.sections[1].offset = 32;
        let error = validate_decoded_container(overlapping, limits()).expect_err("overlap");
        assert!(error.0.contains("overlap"));

        let mut outside = decoded();
        outside.sections[6].offset = 350;
        let error = validate_decoded_container(outside, limits()).expect_err("outside");
        assert!(error.0.contains("exceeds"));
    }

    #[test]
    fn missing_duplicate_and_out_of_bounds_entries_reject() {
        let mut missing = decoded();
        missing.entries.clear();
        let error = validate_decoded_container(missing, limits()).expect_err("missing entry");
        assert!(error.0.contains("does not match 0 canonical entries"));

        let mut duplicate = decoded();
        duplicate.entries.push(duplicate.entries[0]);
        duplicate.sections[5].length = 32;
        duplicate.sections[6].offset = 352;
        duplicate.total_length = 416;
        let error = validate_decoded_container(duplicate, limits()).expect_err("duplicate entry");
        assert!(error.0.contains("must be unique"));

        let mut outside = decoded();
        let identity = EntryStubId::from_normalized_identity(10).expect("entry identity");
        outside.entries = vec![ArtifactEntry::from_canonical_decode(identity, 64)];
        let error = validate_decoded_container(outside, limits()).expect_err("outside entry");
        assert!(error.0.contains("lies outside"));

        let mut mismatched_length = decoded();
        mismatched_length.sections[5].length = 32;
        let error = validate_decoded_container(mismatched_length, limits())
            .expect_err("entry section length must bind its decoded records");
        assert!(error.0.contains("does not match 1 canonical entries"));
    }

    #[test]
    fn relocation_records_are_bounded_canonical_and_non_overlapping() {
        let entry = EntryStubId::from_normalized_identity(9).expect("entry identity");
        let mut canonical = decoded();
        canonical.relocations = vec![
            DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Absolute64,
                destination_offset: 48,
                target: RelocationTarget::Entry(entry),
                addend: 0,
            },
            DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::X86Relative32,
                destination_offset: 8,
                target: RelocationTarget::Entry(entry),
                addend: -4,
            },
        ];
        let validated =
            validate_decoded_container(canonical, limits()).expect("canonical relocations");
        assert_eq!(validated.relocations()[0].destination_offset, 8);
        assert_eq!(validated.relocations()[1].destination_offset, 48);

        let mut overlapping = decoded();
        overlapping.relocations = vec![
            DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Absolute64,
                destination_offset: 8,
                target: RelocationTarget::Entry(entry),
                addend: 0,
            },
            DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Aarch64Branch26,
                destination_offset: 12,
                target: RelocationTarget::Entry(entry),
                addend: 0,
            },
        ];
        let error = validate_decoded_container(overlapping, limits()).expect_err("overlap rejects");
        assert!(error.0.contains("overlaps"));

        let mut outside = decoded();
        outside.relocations[0].destination_offset = 62;
        let error = validate_decoded_container(outside, limits()).expect_err("outside rejects");
        assert!(error.0.contains("exceeds 64-byte code"));

        let mut too_many = decoded();
        let mut strict_limits = limits();
        strict_limits.max_relocations = 1;
        too_many.relocations.push(DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::Aarch64Page21,
            destination_offset: 40,
            target: RelocationTarget::Entry(entry),
            addend: 0,
        });
        let error = validate_decoded_container(too_many, strict_limits).expect_err("bound rejects");
        assert!(error.0.contains("exceeding configured bound"));
    }
}
