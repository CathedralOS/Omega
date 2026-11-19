//! These tests explicitly trust their controlled fixture producer, not a field
//! claiming that the artifact was checked.

pub fn authorize(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> terminal_installation::AuthorizedTerminalArtifact {
    let producer = terminal_installation::HostProducerTrust::accept("controlled compiler fixture");
    terminal_installation::authorize_terminal_artifact(
        std::sync::Arc::new(artifact.clone()),
        Some(&producer),
    )
    .expect("the test host explicitly accepts its fixture producer")
}
