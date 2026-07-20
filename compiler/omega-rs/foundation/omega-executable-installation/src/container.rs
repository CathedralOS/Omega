use super::*;

pub const OMEGA_EXECUTABLE_CONTAINER_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerLimits {
    pub max_total_bytes: u64,
    pub max_sections: usize,
    pub max_section_bytes: u64,
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
    Proof(ProofPayloadId),
    Informational(InformationalSectionId),
    /// An unrecognized optional section is informational by definition. It
    /// cannot supply an identity used by admission.
    Unknown {
        identity: u64,
        required: bool,
    },
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
    pub relocation_set: RelocationSetId,
    pub proof_payload: ProofPayloadId,
    pub sections: Vec<ContainerSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedArtifactContainer {
    artifact: Artifact,
    relocation_set: RelocationSetId,
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
    if limits.max_total_bytes == 0 || limits.max_sections == 0 || limits.max_section_bytes == 0 {
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
        ("proof", proof),
    ] {
        if count != 1 {
            return Err(InstallationDiagnostic(format!(
                "artifact container requires exactly one {name} section, found {count}"
            )));
        }
    }

    let artifact = Artifact::from_canonical_decode(
        decoded.artifact,
        decoded.content,
        decoded.code_length,
        decoded.contracts,
        decoded.declared_footprint,
        decoded.placement_plan,
    )?;
    Ok(ValidatedArtifactContainer {
        artifact,
        relocation_set: decoded.relocation_set,
        proof_payload: decoded.proof_payload,
        informational_sections: informational,
        unknown_informational_sections: unknown_informational,
    })
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
        }
    }

    fn decoded() -> DecodedArtifactContainer {
        let contracts = id(3, MachineContractSetId::from_normalized_identity);
        let footprint = id(4, MachineFootprintId::from_normalized_identity);
        let placement = id(5, PlacementPlanId::from_normalized_identity);
        let relocations = id(6, RelocationSetId::from_normalized_identity);
        let proof = id(7, ProofPayloadId::from_normalized_identity);
        DecodedArtifactContainer {
            format_version: OMEGA_EXECUTABLE_CONTAINER_VERSION,
            total_length: 384,
            artifact: id(1, ArtifactId::from_normalized_identity),
            content: id(2, ArtifactContentId::from_normalized_identity),
            code_length: 64,
            contracts,
            declared_footprint: footprint,
            placement_plan: placement,
            relocation_set: relocations,
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
                    kind: ContainerSectionKind::Proof(proof),
                    offset: 320,
                    length: 64,
                },
            ],
        }
    }

    #[test]
    fn canonical_bounded_container_produces_only_an_artifact_candidate() {
        let container = validate_decoded_container(decoded(), limits()).expect("container");
        assert_eq!(container.artifact().identity().normalized_identity(), 1);
        assert_eq!(container.artifact().byte_length(), 64);
    }

    #[test]
    fn unknown_required_rejects_while_unknown_optional_is_informational() {
        let mut optional = decoded();
        optional.total_length = 448;
        optional.sections.push(ContainerSection {
            kind: ContainerSectionKind::Unknown {
                identity: 99,
                required: false,
            },
            offset: 384,
            length: 64,
        });
        let container =
            validate_decoded_container(optional, limits()).expect("optional information");
        assert_eq!(container.unknown_informational_sections(), &[99]);

        let mut required = decoded();
        required.total_length = 448;
        required.sections.push(ContainerSection {
            kind: ContainerSectionKind::Unknown {
                identity: 99,
                required: true,
            },
            offset: 384,
            length: 64,
        });
        let error = validate_decoded_container(required, limits()).expect_err("required unknown");
        assert!(error.0.contains("unknown required"));
    }

    #[test]
    fn duplicate_missing_overlapping_and_out_of_bounds_sections_reject() {
        let mut duplicate = decoded();
        duplicate.total_length = 448;
        duplicate.sections.push(ContainerSection {
            kind: ContainerSectionKind::Code,
            offset: 384,
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
        outside.sections[5].offset = 360;
        let error = validate_decoded_container(outside, limits()).expect_err("outside");
        assert!(error.0.contains("exceeds"));
    }
}
