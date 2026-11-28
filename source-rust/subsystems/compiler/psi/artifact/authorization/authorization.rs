//! Keep the receiving host's producer-trust decision beside the exact immutable
//! artifact it authorizes. Canonical decoding supplies neither decision.

#![forbid(unsafe_code)]

pub mod selection_questions;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactTrustError {
    ProducerTrustRequired,
    /// The received artifact failed a check that execution relies on and
    /// decoding skips, such as a structural membership the checker refuses.
    ExecutionPreparation(terminal_codec::CodecError),
    /// Preparing the received artifact would exceed the work budget it was
    /// loaded under. A resource refusal, not a failed check.
    LoadWorkExhausted(terminal_codec::CodecError),
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
            Self::LoadWorkExhausted(reason) => {
                write!(
                    formatter,
                    "received Psi exceeds the host's load budget: {reason}"
                )
            }
            Self::ExecutionPreparation(reason) => {
                write!(
                    formatter,
                    "received Psi cannot be prepared for execution: {reason}"
                )
            }
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
        // Received bytes become executable only here, beside the host's
        // decision. The caller's own copy stays received.
        artifact: artifact.record_host_authorization().map_err(|error| {
            if error.is_resource_refusal() {
                ArtifactTrustError::LoadWorkExhausted(error)
            } else {
                ArtifactTrustError::ExecutionPreparation(error)
            }
        })?,
        producer_identity: Arc::clone(&producer.identity),
    })
}

/// An exact received artifact the host admitted for contained interpretation
/// without trusting its producer.
///
/// It runs only on the interpreter, under host bindings that enforce what they
/// rely on dynamically (`terminal_interpreter::TerminalProgram::admit_contained`).
/// Its checked judgments are not reused: native lowering and fact-driven
/// optimization take [`AuthorizedTerminalArtifact`] and refuse this one, and
/// its inner artifact still requires host trust everywhere else.
#[derive(Debug, Clone)]
pub struct ContainedTerminalArtifact {
    artifact: Arc<CanonicalTerminalArtifact>,
}

impl ContainedTerminalArtifact {
    pub fn artifact(&self) -> &CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn identity(&self) -> TerminalArtifactIdentity {
        self.artifact.manifest().identity()
    }

    /// Checks a separate input without broadening this admission.
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

/// Admits an exact artifact for contained interpretation without producer
/// trust. The same execution-only checks run, under the load budget the
/// artifact was decoded with. The caller's own copy stays received.
pub fn admit_contained_interpretation(
    artifact: Arc<CanonicalTerminalArtifact>,
) -> Result<ContainedTerminalArtifact, ArtifactTrustError> {
    Ok(ContainedTerminalArtifact {
        artifact: artifact.record_contained_admission().map_err(|error| {
            if error.is_resource_refusal() {
                ArtifactTrustError::LoadWorkExhausted(error)
            } else {
                ArtifactTrustError::ExecutionPreparation(error)
            }
        })?,
    })
}

/// An artifact admitted to runtime admission: provider selection, target
/// binding, abstract-operation lowering and realization admission.
///
/// Either the host trusts its producer, or the host admitted it only for
/// contained interpretation. Every stage that runtime admission shares with
/// native compilation takes this. The optimizer, image emission, component
/// emission and foreign-adapter compilation take only the authorized form,
/// through [`Self::authorized`], so a contained artifact never reaches native
/// code or fact-driven optimization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmittedTerminalArtifact {
    Authorized(AuthorizedTerminalArtifact),
    Contained(ContainedTerminalArtifact),
}

impl AdmittedTerminalArtifact {
    pub fn artifact(&self) -> &CanonicalTerminalArtifact {
        self.shared_artifact()
    }

    pub fn shared_artifact(&self) -> &Arc<CanonicalTerminalArtifact> {
        match self {
            Self::Authorized(artifact) => artifact.shared_artifact(),
            Self::Contained(artifact) => &artifact.artifact,
        }
    }

    pub fn identity(&self) -> TerminalArtifactIdentity {
        self.artifact().manifest().identity()
    }

    /// Checks a separate input without broadening this admission.
    pub fn require_artifact(
        &self,
        artifact: &CanonicalTerminalArtifact,
    ) -> Result<(), ArtifactTrustError> {
        match self {
            Self::Authorized(authorized) => authorized.require_artifact(artifact),
            Self::Contained(contained) => contained.require_artifact(artifact),
        }
    }

    /// The host-authorized form, required by every stage past runtime
    /// admission.
    pub fn authorized(&self) -> Result<&AuthorizedTerminalArtifact, ArtifactTrustError> {
        match self {
            Self::Authorized(authorized) => Ok(authorized),
            Self::Contained(_) => Err(ArtifactTrustError::ProducerTrustRequired),
        }
    }

    pub fn into_authorized(self) -> Result<AuthorizedTerminalArtifact, ArtifactTrustError> {
        match self {
            Self::Authorized(authorized) => Ok(authorized),
            Self::Contained(_) => Err(ArtifactTrustError::ProducerTrustRequired),
        }
    }

    /// The contained form, for a host that runs this artifact only on the
    /// contained interpreter route.
    pub fn contained(&self) -> Option<&ContainedTerminalArtifact> {
        match self {
            Self::Authorized(_) => None,
            Self::Contained(contained) => Some(contained),
        }
    }

    pub const fn is_contained(&self) -> bool {
        matches!(self, Self::Contained(_))
    }
}

impl From<AuthorizedTerminalArtifact> for AdmittedTerminalArtifact {
    fn from(artifact: AuthorizedTerminalArtifact) -> Self {
        Self::Authorized(artifact)
    }
}

impl From<ContainedTerminalArtifact> for AdmittedTerminalArtifact {
    fn from(artifact: ContainedTerminalArtifact) -> Self {
        Self::Contained(artifact)
    }
}

impl PartialEq for ContainedTerminalArtifact {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.artifact, &other.artifact) || self.identity() == other.identity()
    }
}

impl Eq for ContainedTerminalArtifact {}

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
        assert!(decoded.requires_host_trust());
        assert!(matches!(
            authorize_terminal_artifact(Arc::clone(&decoded), None),
            Err(ArtifactTrustError::ProducerTrustRequired)
        ));

        // The host's decision makes its own copy executable. The received
        // original stays received, so holding it authorizes nothing.
        let producer = HostProducerTrust::accept("host-pinned producer");
        let authorized =
            authorize_terminal_artifact(Arc::clone(&decoded), Some(&producer)).unwrap();
        assert!(!authorized.artifact().requires_host_trust());
        assert_eq!(authorized.artifact(), decoded.as_ref());
        assert!(decoded.requires_host_trust());
    }

    /// Decoding checks canonical form and structure, so a host can inspect an
    /// artifact whose dynamic table disagrees with its interface. Execution
    /// relies on the lowered-form checker's judgment of that table, so the
    /// host's authorization replays it once and refuses the artifact.
    #[test]
    fn authorization_replays_execution_judgments_that_inspection_skips() {
        use terminal_psi::{
            TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicTable,
            TerminalIdentityTableBuilder,
        };
        let mut module = TerminalModule::declarations();
        let mut identities = TerminalIdentityTableBuilder::default();
        let interface = identities.intern("dispatched_interface");
        let declaring = identities.intern("dispatched_trait");
        let requirement = identities.intern("dispatched_requirement");
        let implementation = identities.intern("dispatched_implementation");
        module.identities = identities.build();
        module
            .execution
            .dynamic_dispatch
            .interfaces
            .push(TerminalDynamicInterface {
                id: 0,
                identity: interface,
                slots: vec![TerminalDynamicSlot {
                    slot: 0,
                    declaring_trait_identity: declaring,
                    requirement_identity: requirement,
                    family_tuple: Vec::new(),
                    scalar_parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    entry_claim_paths: Vec::new(),
                    result: terminal_psi::BoundaryMachineResult::Unit,
                    result_claim_paths: Vec::new(),
                    result_reference_regions: Vec::new(),
                }],
            });
        // One slot, but the table names no row for it.
        module
            .execution
            .dynamic_dispatch
            .tables
            .push(TerminalDynamicTable {
                id: 0,
                interface: 0,
                implementation_identity: implementation,
                rows: Vec::new(),
            });
        let optimization = build_identity_optimization_execution_record(&module).unwrap();
        let produced = CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap();
        let received =
            Arc::new(CanonicalTerminalArtifact::from_bytes(&produced.to_bytes()).unwrap());
        assert_eq!(
            received
                .semantic_module()
                .execution
                .dynamic_dispatch
                .tables
                .len(),
            1
        );
        assert!(matches!(
            admit_contained_interpretation(Arc::clone(&received)),
            Err(ArtifactTrustError::ExecutionPreparation(_))
        ));
        let producer = HostProducerTrust::accept("inspection test host");
        assert!(matches!(
            authorize_terminal_artifact(received, Some(&producer)),
            Err(ArtifactTrustError::ExecutionPreparation(_))
        ));
    }

    /// Execution preparation draws on the budget the artifact was loaded
    /// under. A budget that pays for decoding but not for the replayed
    /// judgments refuses authorization as exhausted work, not as a failed
    /// check.
    #[test]
    fn preparation_shares_the_load_budget_with_decoding() {
        let bytes = include_bytes!(
            "../../../../../../tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi"
        );
        let load = |work_units| {
            Arc::new(
                CanonicalTerminalArtifact::from_bytes_within(
                    bytes,
                    terminal_codec::PsiLoadLimits {
                        max_input_bytes: None,
                        work_units,
                    },
                )
                .unwrap(),
            )
        };
        let producer = HostProducerTrust::accept("budget test host");
        let decoded = load(terminal_codec::DEFAULT_LOAD_WORK_UNITS);
        let decode_used = decoded.closed_module().load_account().unwrap().used();
        authorize_terminal_artifact(decoded, Some(&producer)).unwrap();
        let tight = load(decode_used);
        assert!(matches!(
            admit_contained_interpretation(Arc::clone(&tight)),
            Err(ArtifactTrustError::LoadWorkExhausted(error)) if error.is_resource_refusal()
        ));
        assert!(matches!(
            authorize_terminal_artifact(tight, Some(&producer)),
            Err(ArtifactTrustError::LoadWorkExhausted(error)) if error.is_resource_refusal()
        ));
    }

    /// Only this crate may turn received bytes into executable code. Walk the
    /// Rust workspace and refuse any other caller of the codec's hook.
    #[test]
    fn only_authorization_records_host_acceptance() {
        fn visit(directory: &std::path::Path, callers: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|name| name == "target") {
                        continue;
                    }
                    visit(&path, callers);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    if text.contains(".record_host_authorization(")
                        || text.contains(".record_contained_admission(")
                    {
                        callers.push(path);
                    }
                }
            }
        }
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace = manifest.join("../../../../..");
        let mut callers = Vec::new();
        visit(&workspace, &mut callers);
        let callers = callers
            .iter()
            .map(|path| path.canonicalize().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            callers,
            [manifest.join("authorization.rs").canonicalize().unwrap()],
            "received Psi must become executable only through authorize_terminal_artifact"
        );
    }

    /// Contained admission makes a copy the interpreter may run without
    /// producer trust. It passes the same execution-only checks, and every
    /// trusted route still refuses it.
    #[test]
    fn contained_admission_grants_interpretation_without_producer_trust() {
        let original = artifact(false);
        let decoded =
            Arc::new(CanonicalTerminalArtifact::from_bytes(&original.to_bytes()).unwrap());
        let contained = admit_contained_interpretation(Arc::clone(&decoded)).unwrap();
        assert!(contained.artifact().admitted_for_contained_interpretation());
        assert!(contained.artifact().requires_host_trust());
        assert!(!decoded.admitted_for_contained_interpretation());
        assert!(decoded.requires_host_trust());
        assert_eq!(contained.identity(), decoded.manifest().identity());
        contained.require_artifact(&decoded).unwrap();
        assert!(matches!(
            contained.require_artifact(&artifact(true)),
            Err(ArtifactTrustError::ArtifactMismatch { .. })
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
