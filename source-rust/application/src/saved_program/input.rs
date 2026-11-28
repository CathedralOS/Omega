//! Read saved Psi and bind execution to the manifest pinned by the receiving host.
use diagnostics::Diagnostic;

/// The receiving host may bound the encoded bytes before any Psi decoding.
pub(crate) fn read_artifact(
    path: &std::path::Path,
    max_bytes: Option<u64>,
) -> std::io::Result<Vec<u8>> {
    use std::io::{Error, ErrorKind, Read};
    let mut file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    if let Some(limit) = max_bytes {
        let exceeded = || {
            Error::new(
                ErrorKind::FileTooLarge,
                format!("saved Psi exceeds the host input limit of {limit} bytes"),
            )
        };
        if file.metadata()?.len() > limit {
            return Err(exceeded());
        }
        // The file can grow after metadata was read. A stream may report no length.
        file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(exceeded());
        }
    } else {
        file.read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

/// Open saved Psi for `omega run`. With `--trusted-artifact` the host vouches
/// for the producer of that exact manifest; without it the program is admitted
/// only for contained interpretation.
pub(crate) fn open_run_program(
    path: &std::path::Path,
    trusted_artifact: Option<&str>,
    max_input_bytes: Option<u64>,
) -> Result<psi_authorization::AdmittedTerminalArtifact, Vec<Diagnostic>> {
    if trusted_artifact.is_some() {
        return open_trusted_program(path, trusted_artifact, max_input_bytes).map(Into::into);
    }
    let artifact = decode(path, max_input_bytes)?;
    psi_authorization::admit_contained_interpretation(std::sync::Arc::new(artifact))
        .map(Into::into)
        .map_err(|reason| vec![Diagnostic::error(reason.to_string())])
}

fn decode(
    path: &std::path::Path,
    max_input_bytes: Option<u64>,
) -> Result<terminal_codec::CanonicalTerminalArtifact, Vec<Diagnostic>> {
    let bytes = read_artifact(path, max_input_bytes).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read Psi artifact {}: {error}",
            path.display()
        ))]
    })?;
    let limits = terminal_codec::PsiLoadLimits {
        max_input_bytes,
        ..Default::default()
    };
    terminal_codec::CanonicalTerminalArtifact::from_bytes_within(&bytes, limits).map_err(|error| {
        let reason = if error.is_resource_refusal() {
            "saved Psi exceeds the host's load limits"
        } else {
            "cannot inspect Psi artifact"
        };
        vec![Diagnostic::error(format!("{reason}: {error}"))]
    })
}

pub(crate) fn open_trusted_program(
    path: &std::path::Path,
    trusted_artifact: Option<&str>,
    max_input_bytes: Option<u64>,
) -> Result<psi_authorization::AuthorizedTerminalArtifact, Vec<Diagnostic>> {
    let expected = trusted_artifact.ok_or_else(|| vec![Diagnostic::error("saved Psi execution requires --trusted-artifact with the manifest identity supplied by the trusted producer")])?;
    let artifact = decode(path, max_input_bytes)?;
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
