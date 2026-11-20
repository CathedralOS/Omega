//! These tests explicitly trust their controlled fixture producer, not a field
//! claiming that the artifact was checked.

pub fn authorize(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> psi_authorization::AuthorizedTerminalArtifact {
    let producer = psi_authorization::HostProducerTrust::accept("controlled compiler fixture");
    psi_authorization::authorize_terminal_artifact(
        std::sync::Arc::new(artifact.clone()),
        Some(&producer),
    )
    .expect("the test host explicitly accepts its fixture producer")
}
