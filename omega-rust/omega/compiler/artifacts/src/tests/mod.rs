//! Artifact construction and projection regression tests.

use executable_installation::{
    Artifact, ArtifactAuthorityCommitments, ArtifactEntry, ArtifactId, ContainerLimits,
    DecodedArtifactContainer, EntrySetId, InstallationDiagnostic, MachineContractSetId,
    MachineFootprintId, PlacementPlanId, RelocationSetId, decode_executable_container,
    normalized_decoded_content_digest,
};
use layout_plans::{EntryStubId, PlacementConstraints, PlacementPhase};
use target::Architecture;

use super::ArtifactWriter;

fn install_id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn executable_container_fixture() -> Artifact {
    let artifact_id = install_id(900, ArtifactId::from_normalized_identity);
    let contracts = install_id(901, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(902, MachineFootprintId::from_normalized_identity);
    let placement_plan = install_id(903, PlacementPlanId::from_normalized_identity);
    let entry_set = install_id(904, EntrySetId::from_normalized_identity);
    let relocation_set = install_id(905, RelocationSetId::from_normalized_identity);
    let code = vec![0xc3];
    let entries = vec![ArtifactEntry::from_canonical_decode(
        EntryStubId::from_normalized_identity(906).unwrap(),
        0,
    )];
    let placement_constraints =
        PlacementConstraints::new(None, 1, PlacementPhase::Load, None, None)
            .expect("placement constraints");
    let authority_commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
        contracts,
        b"test imported contract set",
        footprint,
        b"test declared footprint",
        None,
        None,
    );
    let mut decoded = DecodedArtifactContainer {
        format_marker: executable_installation::OMEGA_EXECUTABLE_CONTAINER_MARKER,
        total_length: 1,
        artifact: artifact_id,
        content_fingerprint:
            executable_installation::NonAuthoritativeContainerFingerprint64::from_compatibility_value(1)
                .unwrap(),
        architecture: Architecture::X86_64,
        code_length: code.len() as u64,
        code: code.clone(),
        contracts,
        declared_footprint: footprint,
        placement_plan,
        placement_constraints,
        entry_set,
        entries: entries.clone(),
        relocation_set,
        relocations: Vec::new(),
        proof_payload: executable_installation::normalized_proof_payload_digest(b""),
        proof: Vec::new(),
        authority_commitments: Some(authority_commitments),
        sections: Vec::new(),
    };
    decoded.content_fingerprint =
        executable_installation::non_authoritative_decoded_container_fingerprint(&decoded)
            .expect("normalized content fingerprint");
    let content = normalized_decoded_content_digest(&decoded).expect("normalized content digest");
    let artifact = Artifact::from_canonical_decode(
        artifact_id,
        Architecture::X86_64,
        code,
        contracts,
        footprint,
        placement_plan,
        placement_constraints,
        entry_set,
        entries,
        relocation_set,
        Vec::new(),
        authority_commitments,
    )
    .expect("canonical artifact");
    assert_eq!(artifact.content(), content);
    artifact
}

#[test]
fn writes_canonical_executable_container_atomically() {
    let root = std::env::temp_dir().join(format!(
        "omega-artifact-container-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let limits = ContainerLimits {
        max_total_bytes: 64 * 1024,
        max_sections: 16,
        max_section_bytes: 32 * 1024,
        max_relocations: 64,
    };
    let artifact = executable_container_fixture();

    let path = writer
        .write_executable_container("program.omega-artifact", &artifact, b"proof", limits)
        .expect("canonical artifact output");
    let bytes = std::fs::read(&path).expect("written artifact bytes");
    let decoded =
        decode_executable_container(&bytes, limits).expect("written bytes remain canonical");

    assert_eq!(decoded.artifact(), &artifact);
    assert_eq!(decoded.proof(), b"proof");
    assert!(!root.join(".program.omega-artifact.tmp").exists());
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}
