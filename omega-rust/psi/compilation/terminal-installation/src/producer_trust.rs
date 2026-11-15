//! Keep the receiving host's producer-trust decision beside the exact immutable
//! artifact it authorizes. Canonical decoding supplies neither decision.

use std::sync::Arc;

use terminal_codec::{CanonicalTerminalArtifact, TerminalArtifactIdentity};

/// A receiving host's explicit acceptance of one producing process.
///
/// This is a host capability, not an artifact field or a producer assertion.
/// Reuse a clone for the same accepted producer; separate acceptances remain
/// distinct even when their informational descriptions are identical.
#[derive(Debug, Clone)]
pub struct HostProducerTrust {
    identity: Arc<str>,
}

impl HostProducerTrust {
    /// Records the host's decision to trust this producer's checked judgments.
    ///
    /// The host must establish the actual producing process independently of
    /// untrusted artifact metadata. Calling this with an artifact's claimed
    /// producer name does not perform that admission.
    pub fn accept(producer_description: &str) -> Self {
        Self {
            identity: Arc::from(producer_description),
        }
    }
}

impl PartialEq for HostProducerTrust {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

impl Eq for HostProducerTrust {}

/// An exact artifact authorized by the receiving host for checked execution.
///
/// This carrier cannot be decoded from bytes. It shares the immutable canonical
/// product, so later stages need neither another byte realization nor another
/// producer-trust decision. It does not expose the producer-wide capability:
/// possession of this artifact must not authorize a different artifact.
#[derive(Debug, Clone)]
pub struct AuthorizedTerminalArtifact {
    artifact: Arc<CanonicalTerminalArtifact>,
    producer_identity: Arc<str>,
}

impl AuthorizedTerminalArtifact {
    pub fn artifact(&self) -> &CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn shared_artifact(&self) -> &Arc<CanonicalTerminalArtifact> {
        &self.artifact
    }

    pub fn identity(&self) -> TerminalArtifactIdentity {
        self.artifact.manifest().identity()
    }

    pub fn producer_description(&self) -> &str {
        &self.producer_identity
    }

    pub fn authorized_by(&self, producer: &HostProducerTrust) -> bool {
        Arc::ptr_eq(&self.producer_identity, &producer.identity)
    }

    /// Checks a separate input without broadening this artifact's authority.
    pub fn require_artifact(
        &self,
        artifact: &CanonicalTerminalArtifact,
    ) -> Result<(), ArtifactTrustError> {
        let authorized = self.identity();
        let requested = artifact.manifest().identity();
        if authorized != requested {
            return Err(ArtifactTrustError::ArtifactMismatch {
                authorized,
                requested,
            });
        }
        Ok(())
    }
}

impl PartialEq for AuthorizedTerminalArtifact {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.producer_identity, &other.producer_identity)
            && self.identity() == other.identity()
    }
}

impl Eq for AuthorizedTerminalArtifact {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactTrustError {
    ProducerTrustRequired,
    ArtifactMismatch {
        authorized: TerminalArtifactIdentity,
        requested: TerminalArtifactIdentity,
    },
}

impl std::fmt::Display for ArtifactTrustError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProducerTrustRequired => formatter
                .write_str("execution requires the host's acceptance of the actual producer"),
            Self::ArtifactMismatch {
                authorized,
                requested,
            } => write!(
                formatter,
                "producer trust authorizes artifact {authorized}, not {requested}"
            ),
        }
    }
}

impl std::error::Error for ArtifactTrustError {}

/// Binds an existing host trust decision to the exact canonical product.
///
/// Absence of producer trust still permits inspection of `artifact`; it cannot
/// establish checked judgments for execution, optimization or native lowering.
pub fn authorize_terminal_artifact(
    artifact: Arc<CanonicalTerminalArtifact>,
    producer: Option<&HostProducerTrust>,
) -> Result<AuthorizedTerminalArtifact, ArtifactTrustError> {
    let producer = producer.ok_or(ArtifactTrustError::ProducerTrustRequired)?;
    Ok(AuthorizedTerminalArtifact {
        artifact,
        producer_identity: Arc::clone(&producer.identity),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_codec::{TerminalDebugMap, build_identity_optimization_execution_record};
    use terminal_psi::TerminalModule;

    fn artifact(debug: bool) -> Arc<CanonicalTerminalArtifact> {
        let module = TerminalModule::declarations();
        let optimization = build_identity_optimization_execution_record(&module).unwrap();
        let plain =
            CanonicalTerminalArtifact::from_parts(module.clone(), optimization.clone(), None)
                .unwrap();
        let debug = debug.then(|| TerminalDebugMap {
            semantic: plain.view().identity(),
            files: Vec::new(),
            sites: Vec::new(),
        });
        Arc::new(
            CanonicalTerminalArtifact::from_parts(module, optimization, debug.as_ref()).unwrap(),
        )
    }

    #[test]
    fn decoded_artifact_identity_does_not_supply_host_trust() {
        let original = artifact(false);
        let decoded =
            Arc::new(CanonicalTerminalArtifact::from_bytes(&original.to_bytes()).unwrap());
        assert_eq!(
            decoded.manifest().identity(),
            original.manifest().identity()
        );
        assert!(matches!(
            authorize_terminal_artifact(decoded, None),
            Err(ArtifactTrustError::ProducerTrustRequired)
        ));
    }

    #[test]
    fn authorization_shares_the_exact_product_and_keeps_producer_identity() {
        let original = artifact(false);
        let producer = HostProducerTrust::accept("host's admitted compiler");
        let authorized =
            authorize_terminal_artifact(Arc::clone(&original), Some(&producer)).unwrap();
        assert!(Arc::ptr_eq(authorized.shared_artifact(), &original));
        assert!(authorized.authorized_by(&producer.clone()));
        assert!(!authorized.authorized_by(&HostProducerTrust::accept("host's admitted compiler")));
        assert_eq!(
            authorized.producer_description(),
            "host's admitted compiler"
        );
        authorized.require_artifact(&original).unwrap();
    }

    #[test]
    fn equal_semantics_do_not_authorize_a_changed_artifact() {
        let original = artifact(false);
        let changed = artifact(true);
        assert_eq!(original.view().identity(), changed.view().identity());
        assert_ne!(
            original.manifest().identity(),
            changed.manifest().identity()
        );
        let producer = HostProducerTrust::accept("accepted producer");
        let authorized = authorize_terminal_artifact(original, Some(&producer)).unwrap();
        assert!(matches!(
            authorized.require_artifact(&changed),
            Err(ArtifactTrustError::ArtifactMismatch { .. })
        ));
    }
}
