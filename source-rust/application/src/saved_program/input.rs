//! Read saved Psi and bind execution to the manifest pinned by the receiving host.
use diagnostics::Diagnostic;

const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn read_artifact(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_ARTIFACT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Psi artifact exceeds 64 MiB",
        ));
    }
    Ok(bytes)
}

pub(crate) fn open_trusted_program(
    path: &std::path::Path,
    trusted_artifact: Option<&str>,
) -> Result<psi_authorization::AuthorizedTerminalArtifact, Vec<Diagnostic>> {
    let expected = trusted_artifact.ok_or_else(|| vec![Diagnostic::error("saved Psi execution requires --trusted-artifact with the manifest identity supplied by the trusted producer")])?;
    let bytes = read_artifact(path).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read Psi artifact {}: {error}",
            path.display()
        ))]
    })?;
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot inspect Terminal artifact: {error}"
            ))]
        })?;
    if artifact.manifest().identity().to_string() != expected {
        return Err(vec![Diagnostic::error(
            "saved Psi manifest differs from the artifact authorized by this host",
        )]);
    }
    let producer = psi_authorization::HostProducerTrust::accept(
        "trusted producer of the host-pinned artifact",
    );
    psi_authorization::authorize_terminal_artifact(std::sync::Arc::new(artifact), Some(&producer))
        .map_err(|reason| vec![Diagnostic::error(reason.to_string())])
}
