//! Independent, source-free verification of canonical component descriptions.
//!
//! `verify_component` is the verified-description consumer for independent
//! component admission, replacement, and topology checks. It re-decodes the
//! embedded canonical artifact, reconstructs the component subject itself,
//! replays the subject-bound proof section decode, and runs the Terminal
//! verifier on the decoded module under the caller's admission profile: the
//! code-to-inventory claim is the verifier's, so a module that merely decodes
//! and subject-matches is not a verified component. Every module-evident
//! description row is then re-derived from the verified module rather than
//! trusting producer claims. Execution admission remains purpose-specific and
//! stays with the verifier's own distinct carriers; this consumer is
//! deliberately neutral about which execution admission a caller will need.
//!
//! The result carrier is deliberately inert: it exposes checked evidence
//! (subject, rosters, obligations, and the verified module) but grants no
//! callable authority, no installed custody, and no resource admission.
//! Installation-dependent facts stay obligations on the description and must
//! be discharged per occurrence under a fresh resource/profile admission.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use terminal_psi::{TerminalModule, TerminalPsiIdentity};

use effects::provider_plan::{ProviderBinding, ProviderPlan};

pub use proof_admission::AdmissionProfile;

use crate::component_description::{
    ComponentDescription, ComponentEntry, CustodyConstraint, CustodyEvidence, CustodyKind,
    DescriptionDecodeRejection, DescriptionFrontier, EntryEvidence, ExportSurface, ImportSlot,
    InstallationObligation, InstallationServiceBound, MAX_COMPONENT_DESCRIPTION_BYTES,
    ObligationKind, OutgoingAuthority, OutgoingAuthorityClass, OutgoingEvidence, RetainedProvider,
    component_description_identity, decode_component_description, derive_component_inventory, hex,
    requirement_contract_identity, requirement_export_identity,
};

const VERIFIED_COMPONENT_CLOSURE_DOMAIN: &[u8] = b"omega-verified-component-closure-v1";
const VERIFICATION_PROFILE_DOMAIN: &[u8] = b"omega-component-verification-profile-v1";

/// The caller's independent admission profile for a component description.
///
/// Everything here is supplied by the consumer and never read from the
/// description itself: the component subject to admit, the description
/// schemas this consumer understands, the assumption digests it accepts, and
/// the module-proof admission profile the embedded module must verify under.
#[derive(Debug, Clone)]
pub struct ComponentVerificationRequest {
    /// The exact component subject this admission expects.
    pub expected_subject: TerminalPsiIdentity,
    /// Description schema versions this consumer admits.
    pub accepted_schemas: BTreeSet<u32>,
    /// Assumption digests this consumer accepts for declared rows and
    /// physical mechanisms. Any digest outside this set rejects.
    pub accepted_assumptions: BTreeSet<[u8; 32]>,
    /// The admission profile `terminal-verifier` discharges the embedded
    /// module's proof obligations under. An empty profile admits only modules
    /// whose obligations are kernel-dischargeable; each acceptance names one
    /// site, its evidence identity, and the profile decision that admitted it.
    pub admission_profile: AdmissionProfile,
}

impl ComponentVerificationRequest {
    /// Collision-resistant identity of this exact request, binding the
    /// verification result to the profile that admitted it.
    pub fn profile_identity(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(VERIFICATION_PROFILE_DOMAIN);
        digest.update(self.expected_subject.vocabulary_marker.get().to_le_bytes());
        digest.update(self.expected_subject.program_fingerprint.as_bytes());
        digest.update((self.accepted_schemas.len() as u64).to_le_bytes());
        for schema in &self.accepted_schemas {
            digest.update(schema.to_le_bytes());
        }
        digest.update((self.accepted_assumptions.len() as u64).to_le_bytes());
        for assumption in &self.accepted_assumptions {
            digest.update(assumption);
        }
        let acceptances: Vec<_> = self.admission_profile.acceptances().collect();
        digest.update((acceptances.len() as u64).to_le_bytes());
        for acceptance in acceptances {
            digest.update(acceptance.site.get().to_le_bytes());
            digest.update(acceptance.evidence_identity.get().to_le_bytes());
            digest.update(acceptance.profile_decision.get().to_le_bytes());
        }
        digest.finalize().into()
    }
}

/// Every way a component description can fail independent verification.
///
/// Rejections are deliberately categorical: a corrupt byte stream, an
/// incompatible schema, the wrong component subject, an early frontier, an
/// unaccepted assumption, a missing or forged module-derived row, an unsealed
/// or smuggled provider, and a missing installation obligation are all
/// distinct, so a caller cannot confuse "not yet complete" with "complete
/// but dishonest".
#[derive(Debug)]
pub enum ComponentVerificationRejection {
    /// The description bytes did not decode under the canonical codec.
    Corrupt(DescriptionDecodeRejection),
    /// The description's schema is not admitted by the request.
    IncompatibleSchema { schema: u32 },
    /// The embedded artifact's reconstructed subject is not the admitted one.
    WrongSubject {
        reconstructed: Box<TerminalPsiIdentity>,
        expected: Box<TerminalPsiIdentity>,
    },
    /// The embedded artifact is not a valid canonical Terminal artifact.
    ArtifactInvalid(String),
    /// The embedded proof section could not be decoded for the exact subject.
    ProofSection(String),
    /// The embedded module failed Terminal validation or proof discharge
    /// under the request's admission profile.
    ModuleVerification(String),
    /// The description names a frontier earlier than a closed artifact.
    EarlyFrontier(DescriptionFrontier),
    /// The description carries an assumption the request does not accept.
    UnacceptedAssumption([u8; 32]),
    /// A declared row binds an assumption digest absent from the roster.
    UnboundAssumptionReference([u8; 32]),
    /// A module-derived entry is absent from the description.
    MissingComponentEntry(String),
    /// A row claims module-derived evidence the artifact does not contain.
    UnexpectedDerivedEntry(String),
    /// A module-derived export is absent from the description.
    MissingExport(String),
    /// The description exports a surface the artifact does not contain.
    UnexpectedDerivedExport(String),
    /// A module-derived outgoing authority row is absent.
    MissingOutgoingAuthority(String),
    /// The description claims outgoing authority the artifact lacks.
    UnexpectedDerivedAuthority(String),
    /// The evidence kind is not permitted for this row's class.
    InvalidAuthorityEvidence(String),
    /// A `ProviderSealed` row names a plan digest outside the provider roster
    /// or seals a requirement that provider does not claim.
    UnsealedProvider(String),
    /// A retained provider claims a requirement no outgoing row names.
    SmuggledProviderRequirement(String),
    /// A boundary requirement is neither provider-sealed nor import-bound.
    UnboundRequirement(String),
    /// An import slot names a requirement with no unsealed outgoing row.
    SmuggledImport(String),
    /// An import slot's contract identity is not the canonical requirement
    /// contract identity.
    ImportContractMismatch(String),
    /// A module-derived custody row is absent from the description.
    MissingCustodyConstraint(String),
    /// A row claims module-derived custody the artifact does not contain.
    UnexpectedDerivedCustody(String),
    /// A retained installation-bound reach row is absent from the
    /// description's service-bound roster.
    MissingServiceBound(String),
    /// The description publishes a service-bound row the artifact does not
    /// retain.
    UnexpectedServiceBound(String),
    /// A required installation obligation is absent from the description.
    MissingObligation(String),
    /// The provider roster and closure digest are not internally consistent.
    InconsistentProviderClosure(&'static str),
}

impl std::fmt::Display for ComponentVerificationRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Corrupt(rejection) => write!(formatter, "corrupt description: {rejection}"),
            Self::IncompatibleSchema { schema } => {
                write!(formatter, "description schema {schema} is not admitted")
            }
            Self::WrongSubject {
                reconstructed,
                expected,
            } => write!(
                formatter,
                "description subject {reconstructed:?} is not the admitted subject {expected:?}"
            ),
            Self::ArtifactInvalid(detail) => {
                write!(formatter, "embedded artifact is invalid: {detail}")
            }
            Self::ProofSection(detail) => {
                write!(formatter, "embedded proof section failed: {detail}")
            }
            Self::ModuleVerification(detail) => {
                write!(formatter, "embedded module failed verification: {detail}")
            }
            Self::EarlyFrontier(frontier) => write!(
                formatter,
                "description frontier {frontier:?} precedes a closed artifact"
            ),
            Self::UnacceptedAssumption(digest) => {
                write!(formatter, "assumption {} is not accepted", hex(digest))
            }
            Self::UnboundAssumptionReference(digest) => write!(
                formatter,
                "row binds assumption {} absent from the roster",
                hex(digest)
            ),
            Self::MissingComponentEntry(identity) => {
                write!(formatter, "module-derived entry `{identity}` is missing")
            }
            Self::UnexpectedDerivedEntry(identity) => write!(
                formatter,
                "entry `{identity}` claims module-derived evidence the artifact lacks"
            ),
            Self::MissingExport(identity) => {
                write!(formatter, "module-derived export `{identity}` is missing")
            }
            Self::UnexpectedDerivedExport(identity) => write!(
                formatter,
                "export `{identity}` claims a surface the artifact lacks"
            ),
            Self::MissingOutgoingAuthority(identity) => {
                write!(formatter, "outgoing authority `{identity}` is missing")
            }
            Self::UnexpectedDerivedAuthority(identity) => write!(
                formatter,
                "outgoing authority `{identity}` claims a fact the artifact lacks"
            ),
            Self::InvalidAuthorityEvidence(identity) => write!(
                formatter,
                "outgoing authority `{identity}` carries a forbidden evidence kind"
            ),
            Self::UnsealedProvider(identity) => write!(
                formatter,
                "requirement `{identity}` is sealed to a provider outside the roster"
            ),
            Self::SmuggledProviderRequirement(identity) => write!(
                formatter,
                "retained provider claims unrequired requirement `{identity}`"
            ),
            Self::UnboundRequirement(identity) => write!(
                formatter,
                "requirement `{identity}` is neither provider-sealed nor import-bound"
            ),
            Self::SmuggledImport(identity) => write!(
                formatter,
                "import slot `{identity}` names no unsealed requirement"
            ),
            Self::ImportContractMismatch(identity) => write!(
                formatter,
                "import `{identity}` contract digest is not canonical"
            ),
            Self::MissingCustodyConstraint(identity) => {
                write!(formatter, "custody constraint `{identity}` is missing")
            }
            Self::UnexpectedDerivedCustody(identity) => write!(
                formatter,
                "custody `{identity}` claims a fact the artifact lacks"
            ),
            Self::MissingServiceBound(identity) => write!(
                formatter,
                "installation-bound service bound `{identity}` is missing"
            ),
            Self::UnexpectedServiceBound(identity) => write!(
                formatter,
                "service bound `{identity}` names a bound the artifact does not retain"
            ),
            Self::MissingObligation(identity) => {
                write!(formatter, "installation obligation `{identity}` is missing")
            }
            Self::InconsistentProviderClosure(detail) => {
                write!(formatter, "provider closure is inconsistent: {detail}")
            }
        }
    }
}

impl std::error::Error for ComponentVerificationRejection {}

/// A component description whose every checkable fact has been independently
/// replayed and accepted under the caller's request.
///
/// This carrier is evidence, not authority: it proves the embedded artifact
/// is canonical, subject-bound to its proof section, its module verified
/// under the request's admission profile, and exactly covered by the
/// description's entries, outgoing authority, custody, retained providers,
/// and installation obligations. It cannot be executed, installed, or used
/// to discharge any obligation it describes.
#[derive(Debug)]
pub struct VerifiedComponent {
    description: ComponentDescription,
    description_identity: [u8; 32],
    closure: [u8; 32],
    module: TerminalModule,
}

impl VerifiedComponent {
    /// The reconstructed component subject — always derived from the
    /// embedded artifact, never from a description field.
    pub fn subject(&self) -> TerminalPsiIdentity {
        terminal_codec::terminal_psi_identity(&self.module)
            .expect("a verified component always has a replayable subject")
    }

    /// Collision-resistant identity of the exact description bytes admitted.
    pub const fn description_identity(&self) -> &[u8; 32] {
        &self.description_identity
    }

    /// The closure identity binding this admission to its exact request
    /// profile. Replacement candidates must produce a closure comparable to
    /// the frozen envelope the initial composition recorded.
    pub const fn closure(&self) -> &[u8; 32] {
        &self.closure
    }

    /// The description frontier that verified. Always
    /// [`DescriptionFrontier::TerminalArtifactClosure`] for a `VerifiedComponent`.
    pub const fn frontier(&self) -> DescriptionFrontier {
        self.description.frontier
    }

    /// The decoded semantic module. Reading it grants no authority; proof
    /// consumers that need a purpose-specific verified carrier re-run the
    /// existing Terminal verifier under their own profile.
    pub const fn module(&self) -> &TerminalModule {
        &self.module
    }

    pub fn imports(&self) -> &[ImportSlot] {
        &self.description.imports
    }

    pub fn exports(&self) -> &[ExportSurface] {
        &self.description.exports
    }

    /// Every possible entry into the component.
    pub fn entries(&self) -> &[ComponentEntry] {
        &self.description.entries
    }

    /// Every authority path leaving the component closure.
    pub fn outgoing(&self) -> &[OutgoingAuthority] {
        &self.description.outgoing
    }

    /// Every retained installation-bound reach row with its declared
    /// conservative bound — the roster of bounds installation still owes,
    /// published separately from the concrete reach's `ServiceCeiling` rows.
    pub fn service_bounds(&self) -> &[InstallationServiceBound] {
        &self.description.service_bounds
    }

    /// Every custody constraint the component carries.
    pub fn custody(&self) -> &[CustodyConstraint] {
        &self.description.custody
    }

    /// The retained selected-provider roster.
    pub fn providers(&self) -> &[RetainedProvider] {
        &self.description.providers
    }

    /// Strong digest of the complete selected provider closure facts.
    pub const fn provider_closure_digest(&self) -> &[u8; 32] {
        &self.description.provider_closure_digest
    }

    /// Installation-dependent demands. Each remains an obligation: the
    /// description verified that they are demanded, not that they are met.
    pub fn obligations(&self) -> &[InstallationObligation] {
        &self.description.obligations
    }

    /// The accepted inseparable assumptions bound to declared rows.
    pub fn assumptions(&self) -> &[[u8; 32]] {
        &self.description.assumptions
    }

    /// Strong identity of the bound native realization, when present.
    pub const fn realization_identity(&self) -> Option<&[u8; 32]> {
        self.description.realization_identity.as_ref()
    }

    /// Join one build-selected provider plan to this verified component's
    /// exported checked realizations.
    ///
    /// This is the consumer-side half of an `Independent` selection: the
    /// consumer selected `plan` (one provider type covering one boundary
    /// slot) and this component claims to be that provider's deployable
    /// closure. Every plan row must be a `CheckedAdapter` whose requirement,
    /// provider type, and machine identity name exactly one provider
    /// candidate in the *verified* module (never a description row), and
    /// the description must export that realization. External, syscall, or
    /// evaluated rows are not exported callable surfaces of a checked
    /// closure and reject. A component whose module still retains
    /// unresolved installation-bound reach rows also rejects: its declared
    /// service bounds are obligations installation still owes, not the
    /// resolved reach a callable component contract must carry.
    ///
    /// Success establishes only that the realization exists inside this
    /// exact subject. It grants no callable authority and discharges none of
    /// the component's installation obligations; the consumer's fence still
    /// owns subject/profile policy and the provider-occurrence binding.
    pub fn realizes_selected_plan(
        &self,
        plan: &ProviderPlan,
    ) -> Result<(), IndependentRealizationMismatch> {
        if plan.provider_type.is_empty() {
            return Err(IndependentRealizationMismatch::EmptyProviderType {
                plan: plan.name.clone(),
            });
        }
        // Schema validation owns coverage and row/origin consistency (a
        // checked adapter whose package drifts from the plan origin rejects
        // there); the join only reads a plan that is already a valid
        // selection.
        let coverage = plan.validate_against_schema();
        if plan.rows.is_empty() || !coverage.is_empty() {
            return Err(IndependentRealizationMismatch::InvalidPlan {
                plan: plan.name.clone(),
                detail: coverage.join("; "),
            });
        }
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } = &row.binding
            else {
                return Err(IndependentRealizationMismatch::UncheckedRow {
                    requirement_identity: row.requirement_identity.clone(),
                });
            };
            let for_requirement = self
                .module
                .provider_candidates
                .iter()
                .filter(|candidate| candidate.requirement_identity == row.requirement_identity)
                .collect::<Vec<_>>();
            if for_requirement.is_empty() {
                return Err(IndependentRealizationMismatch::MissingRealization {
                    requirement_identity: row.requirement_identity.clone(),
                });
            }
            let for_provider = for_requirement
                .iter()
                .filter(|candidate| candidate.provider_identity == plan.provider_type)
                .collect::<Vec<_>>();
            if for_provider.is_empty() {
                return Err(IndependentRealizationMismatch::ProviderTypeMismatch {
                    requirement_identity: row.requirement_identity.clone(),
                    selected_provider: plan.provider_type.clone(),
                });
            }
            let exact = for_provider
                .iter()
                .filter(|candidate| candidate.candidate_identity == *machine_identity)
                .collect::<Vec<_>>();
            match exact.as_slice() {
                [] => {
                    return Err(IndependentRealizationMismatch::MachineMismatch {
                        requirement_identity: row.requirement_identity.clone(),
                        selected_machine: machine_identity.clone(),
                    });
                }
                [_] => {}
                _ => {
                    return Err(IndependentRealizationMismatch::DuplicateRealization {
                        requirement_identity: row.requirement_identity.clone(),
                    });
                }
            }
            // Verification already proved the export roster equals the
            // module-derived roster; requiring the row here keeps the join
            // honest if a later description schema ever narrows exports.
            let export = requirement_export_identity(
                &row.requirement_identity,
                &plan.provider_type,
                machine_identity,
            );
            if !self
                .description
                .exports
                .iter()
                .any(|surface| surface.identity == export)
            {
                return Err(IndependentRealizationMismatch::MissingExport {
                    requirement_identity: row.requirement_identity.clone(),
                });
            }
        }
        // Only after every row matched is the component's own closure in
        // question: a realizer that still retains unresolved
        // installation-bound reach rows is not a closed callable component.
        // Its service bounds publish what installation still owes, so the
        // join must not treat the retained bound as the resolved reach the
        // selected row names.
        let unresolved: Vec<String> = self
            .module
            .root_service_reach
            .installation_dependencies
            .iter()
            .map(|dependency| dependency.requirement_identity.clone())
            .collect();
        if !unresolved.is_empty() {
            return Err(IndependentRealizationMismatch::UnresolvedInstallationRows {
                requirement_identities: unresolved,
            });
        }
        Ok(())
    }
}

/// Every way a selected provider plan can fail to join a verified
/// component's exported realizations. Categories are distinct so a consumer
/// fence can report a substituted provider, a substituted machine, or an
/// absent realization without conflating them with an unverified description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndependentRealizationMismatch {
    /// The plan names no provider type; a free external leaf cannot be an
    /// independently deployed component.
    EmptyProviderType { plan: String },
    /// The plan is not a valid fully covering selection: no rows, an
    /// uncovered schema method, or a row inconsistent with the plan origin.
    InvalidPlan { plan: String, detail: String },
    /// A row is not a checked adapter inside the component closure.
    UncheckedRow { requirement_identity: String },
    /// The verified module retains no candidate for this requirement.
    MissingRealization { requirement_identity: String },
    /// The requirement is realized, but not by the selected provider type.
    ProviderTypeMismatch {
        requirement_identity: String,
        selected_provider: String,
    },
    /// The selected provider realizes the requirement with another machine.
    MachineMismatch {
        requirement_identity: String,
        selected_machine: String,
    },
    /// More than one identical realization row exists.
    DuplicateRealization { requirement_identity: String },
    /// The description omits the export row the realization requires.
    MissingExport { requirement_identity: String },
    /// The realization matched, but the component still retains unresolved
    /// installation-bound reach rows: its declared service bounds are
    /// obligations installation still owes, not the resolved service reach
    /// the selected row names.
    UnresolvedInstallationRows { requirement_identities: Vec<String> },
}

impl std::fmt::Display for IndependentRealizationMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyProviderType { plan } => {
                write!(formatter, "selected plan `{plan}` names no provider type")
            }
            Self::InvalidPlan { plan, detail } => {
                write!(
                    formatter,
                    "selected plan `{plan}` is not a valid selection: {detail}"
                )
            }
            Self::UncheckedRow {
                requirement_identity,
            } => write!(
                formatter,
                "requirement `{requirement_identity}` is not realized by a checked adapter"
            ),
            Self::MissingRealization {
                requirement_identity,
            } => write!(
                formatter,
                "verified component realizes no candidate for `{requirement_identity}`"
            ),
            Self::ProviderTypeMismatch {
                requirement_identity,
                selected_provider,
            } => write!(
                formatter,
                "`{requirement_identity}` is not realized by selected provider `{selected_provider}`"
            ),
            Self::MachineMismatch {
                requirement_identity,
                selected_machine,
            } => write!(
                formatter,
                "`{requirement_identity}` is not realized by selected machine `{selected_machine}`"
            ),
            Self::DuplicateRealization {
                requirement_identity,
            } => write!(
                formatter,
                "`{requirement_identity}` has more than one identical realization"
            ),
            Self::MissingExport {
                requirement_identity,
            } => write!(
                formatter,
                "description exports no realization row for `{requirement_identity}`"
            ),
            Self::UnresolvedInstallationRows {
                requirement_identities,
            } => write!(
                formatter,
                "verified component retains {} unresolved installation-bound requirement row(s) ({}); its service bounds are obligations installation still owes, not resolved reach",
                requirement_identities.len(),
                requirement_identities.join(", ")
            ),
        }
    }
}

impl std::error::Error for IndependentRealizationMismatch {}

/// Independently verify a canonical component description for admission.
///
/// The description is trusted for nothing: the codec enforces canonical
/// bounds and ordering, the embedded artifact is re-decoded with its
/// subject-bound proof section, the component subject is reconstructed
/// rather than read, the module is verified under the request's admission
/// profile, and every module-evident roster is re-derived from the verified
/// module and compared row for row. Installation obligations are checked for
/// presence and left undischarged.
pub fn verify_component(
    description_bytes: &[u8],
    request: &ComponentVerificationRequest,
) -> Result<VerifiedComponent, ComponentVerificationRejection> {
    if description_bytes.len() > MAX_COMPONENT_DESCRIPTION_BYTES {
        return Err(ComponentVerificationRejection::Corrupt(
            DescriptionDecodeRejection::RosterBoundExceeded("description byte length"),
        ));
    }
    let description = decode_component_description(description_bytes)
        .map_err(ComponentVerificationRejection::Corrupt)?;
    if !request.accepted_schemas.contains(&description.schema) {
        return Err(ComponentVerificationRejection::IncompatibleSchema {
            schema: description.schema,
        });
    }
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&description.artifact_bytes)
            .map_err(|error| ComponentVerificationRejection::ArtifactInvalid(error.to_string()))?;
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .map_err(|error| ComponentVerificationRejection::ArtifactInvalid(error.to_string()))?;
    let reconstructed = terminal_codec::terminal_psi_identity(&module)
        .map_err(|error| ComponentVerificationRejection::ArtifactInvalid(error.to_string()))?;
    if reconstructed != request.expected_subject {
        return Err(ComponentVerificationRejection::WrongSubject {
            reconstructed: Box::new(reconstructed),
            expected: Box::new(request.expected_subject),
        });
    }
    // The component artifact's proof section must be sealed to this exact
    // module, and the module must verify against it under the request's
    // admission profile: a decoded, subject-matched module whose obligations
    // go undischargeable is not a verified component.
    let proof_bundle = terminal_codec::decode_proof_section_for(&module, artifact.proof_bytes())
        .map_err(|error| ComponentVerificationRejection::ProofSection(error.to_string()))?;
    let verified_module =
        terminal_verifier::verify_module(&module, &proof_bundle, &request.admission_profile)
            .map_err(|error| {
                ComponentVerificationRejection::ModuleVerification(error.to_string())
            })?;
    if description.frontier != DescriptionFrontier::TerminalArtifactClosure {
        return Err(ComponentVerificationRejection::EarlyFrontier(
            description.frontier,
        ));
    }
    for assumption in &description.assumptions {
        if !request.accepted_assumptions.contains(assumption) {
            return Err(ComponentVerificationRejection::UnacceptedAssumption(
                *assumption,
            ));
        }
    }

    let inventory = derive_component_inventory(verified_module.module()).map_err(|error| {
        ComponentVerificationRejection::ArtifactInvalid(format!(
            "verified module did not admit a component inventory: {error}"
        ))
    })?;

    check_entries(&description, &inventory)?;
    check_exports(&description, &inventory)?;
    let sealed = check_outgoing(&description, &inventory)?;
    check_providers(&description, &inventory, &sealed)?;
    check_service_bounds(&description, &inventory)?;
    check_custody(&description, &inventory)?;
    check_imports(&description)?;
    check_obligations(&description, &inventory)?;

    let description_identity = component_description_identity(description_bytes);
    let mut digest = Sha256::new();
    digest.update(VERIFIED_COMPONENT_CLOSURE_DOMAIN);
    digest.update(description_identity);
    digest.update(request.profile_identity());
    let closure = digest.finalize().into();

    Ok(VerifiedComponent {
        description,
        description_identity,
        closure,
        module,
    })
}

/// Entry rows must exactly cover the derived set: every derived entry is
/// required, every `ModuleDerived` row must be derived, and every
/// `AssumptionBound` row must name a roster assumption. Declared rows bound
/// to accepted assumptions are how startup, callback, timer, cleanup, and
/// retained-provider entries enter the roster.
fn check_entries(
    description: &ComponentDescription,
    inventory: &crate::component_description::DerivedInventory,
) -> Result<(), ComponentVerificationRejection> {
    let derived: BTreeSet<(u8, &str)> = inventory
        .entries
        .iter()
        .map(|entry| (entry.kind.tag(), entry.identity.as_str()))
        .collect();
    let mut seen = BTreeSet::new();
    for entry in &description.entries {
        match entry.evidence {
            EntryEvidence::ModuleDerived => {
                let key = (entry.kind.tag(), entry.identity.as_str());
                if !derived.contains(&key) {
                    return Err(ComponentVerificationRejection::UnexpectedDerivedEntry(
                        entry.identity.clone(),
                    ));
                }
            }
            EntryEvidence::AssumptionBound(digest) => {
                require_bound_assumption(&description.assumptions, digest)?;
            }
        }
        seen.insert((entry.kind.tag(), entry.identity.as_str()));
    }
    for (tag, identity) in derived {
        if !seen.contains(&(tag, identity)) {
            return Err(ComponentVerificationRejection::MissingComponentEntry(
                identity.to_string(),
            ));
        }
    }
    Ok(())
}

/// The exported surface is exactly the module-derived export set.
fn check_exports(
    description: &ComponentDescription,
    inventory: &crate::component_description::DerivedInventory,
) -> Result<(), ComponentVerificationRejection> {
    let derived: BTreeSet<&str> = inventory
        .exports
        .iter()
        .map(|export| export.identity.as_str())
        .collect();
    for export in &description.exports {
        if !derived.contains(export.identity.as_str()) {
            return Err(ComponentVerificationRejection::UnexpectedDerivedExport(
                export.identity.clone(),
            ));
        }
    }
    let described: BTreeSet<&str> = description
        .exports
        .iter()
        .map(|export| export.identity.as_str())
        .collect();
    for identity in derived {
        if !described.contains(identity) {
            return Err(ComponentVerificationRejection::MissingExport(
                identity.to_string(),
            ));
        }
    }
    Ok(())
}

/// Outgoing authority rows must cover the derived set with evidence legal
/// for their class. Returns the requirement→plan-digest seal map used by the
/// provider-roster check.
fn check_outgoing(
    description: &ComponentDescription,
    inventory: &crate::component_description::DerivedInventory,
) -> Result<BTreeMap<String, [u8; 32]>, ComponentVerificationRejection> {
    let provider_requirements: BTreeMap<[u8; 32], BTreeSet<&str>> = description
        .providers
        .iter()
        .map(|provider| {
            (
                provider.plan_digest,
                provider
                    .requirement_identities
                    .iter()
                    .map(String::as_str)
                    .collect(),
            )
        })
        .collect();

    let mut derived_keys = BTreeSet::new();
    for requirement in &inventory.called_requirement_identities {
        derived_keys.insert((
            OutgoingAuthorityClass::BoundaryRequirement,
            requirement.clone(),
        ));
    }
    for (service, port, value) in &inventory.port_writes {
        let identity = format!("port-write:{}:{port}:{value}", service.get());
        derived_keys.insert((OutgoingAuthorityClass::PortSpaceWrite, identity));
    }
    for service in &inventory.concrete_service_reach {
        derived_keys.insert((
            OutgoingAuthorityClass::ServiceCeiling,
            format!("service-ceiling:{}", service.get()),
        ));
    }

    let mut sealed: BTreeMap<String, [u8; 32]> = BTreeMap::new();
    for row in &description.outgoing {
        let key = (row.class, row.identity.clone());
        let derived = derived_keys.contains(&key);
        match (row.class, row.evidence, derived) {
            (
                OutgoingAuthorityClass::BoundaryRequirement,
                OutgoingEvidence::ModuleDerived,
                true,
            ) => {}
            (
                OutgoingAuthorityClass::BoundaryRequirement,
                OutgoingEvidence::ProviderSealed(plan_digest),
                true,
            ) => {
                let requirements = provider_requirements.get(&plan_digest).ok_or_else(|| {
                    ComponentVerificationRejection::UnsealedProvider(row.identity.clone())
                })?;
                if !requirements.contains(row.identity.as_str()) {
                    return Err(ComponentVerificationRejection::UnsealedProvider(
                        row.identity.clone(),
                    ));
                }
                sealed.insert(row.identity.clone(), plan_digest);
            }
            (OutgoingAuthorityClass::PortSpaceWrite, OutgoingEvidence::ModuleDerived, true) => {}
            (
                OutgoingAuthorityClass::PortSpaceWrite,
                OutgoingEvidence::PhysicalMechanism(assumption),
                true,
            ) => {
                require_bound_assumption(&description.assumptions, assumption)?;
            }
            (OutgoingAuthorityClass::ServiceCeiling, OutgoingEvidence::ModuleDerived, true) => {}
            (_, OutgoingEvidence::AssumptionBound(assumption), false) => {
                require_bound_assumption(&description.assumptions, assumption)?;
            }
            (_, _, false) => {
                return Err(ComponentVerificationRejection::UnexpectedDerivedAuthority(
                    row.identity.clone(),
                ));
            }
            (_, _, true) => {
                return Err(ComponentVerificationRejection::InvalidAuthorityEvidence(
                    row.identity.clone(),
                ));
            }
        }
    }

    let described_keys: BTreeSet<(OutgoingAuthorityClass, String)> = description
        .outgoing
        .iter()
        .map(|row| (row.class, row.identity.clone()))
        .collect();
    for (class, identity) in derived_keys {
        if !described_keys.contains(&(class, identity.clone())) {
            return Err(ComponentVerificationRejection::MissingOutgoingAuthority(
                identity,
            ));
        }
    }

    for requirement in &inventory.called_requirement_identities {
        let row = description.outgoing.iter().find(|row| {
            row.class == OutgoingAuthorityClass::BoundaryRequirement && row.identity == *requirement
        });
        match row.map(|row| row.evidence) {
            Some(OutgoingEvidence::ProviderSealed(_)) => {}
            Some(OutgoingEvidence::ModuleDerived) => {
                if !description
                    .imports
                    .iter()
                    .any(|slot| slot.requirement_identity == *requirement)
                {
                    return Err(ComponentVerificationRejection::UnboundRequirement(
                        requirement.clone(),
                    ));
                }
            }
            _ => {
                return Err(ComponentVerificationRejection::UnboundRequirement(
                    requirement.clone(),
                ));
            }
        }
    }
    Ok(sealed)
}

/// The retained provider roster must be internally consistent: every sealed
/// requirement resolves to a listed plan digest that claims it, no two
/// providers claim the same requirement, no provider claims a requirement
/// the component does not name, and a nonempty roster carries a nonzero
/// closure digest.
fn check_providers(
    description: &ComponentDescription,
    inventory: &crate::component_description::DerivedInventory,
    sealed: &BTreeMap<String, [u8; 32]>,
) -> Result<(), ComponentVerificationRejection> {
    let called: BTreeSet<&str> = inventory
        .called_requirement_identities
        .iter()
        .map(String::as_str)
        .collect();
    let mut claimed: BTreeSet<&str> = BTreeSet::new();
    let mut plan_digests: BTreeSet<[u8; 32]> = BTreeSet::new();
    for provider in &description.providers {
        if !plan_digests.insert(provider.plan_digest) {
            return Err(ComponentVerificationRejection::InconsistentProviderClosure(
                "two retained providers share one plan digest",
            ));
        }
        if provider.plan_digest == [0; 32] {
            return Err(ComponentVerificationRejection::InconsistentProviderClosure(
                "a retained provider carries a zero plan digest",
            ));
        }
        if provider.requirement_identities.is_empty() {
            return Err(ComponentVerificationRejection::InconsistentProviderClosure(
                "a retained provider seals no requirement",
            ));
        }
        for requirement in &provider.requirement_identities {
            if !called.contains(requirement.as_str()) {
                return Err(ComponentVerificationRejection::SmuggledProviderRequirement(
                    requirement.clone(),
                ));
            }
            match sealed.get(requirement) {
                Some(digest) if *digest == provider.plan_digest => {}
                _ => {
                    return Err(ComponentVerificationRejection::InconsistentProviderClosure(
                        "a provider requirement is not sealed to that provider's plan",
                    ));
                }
            }
            if !claimed.insert(requirement.as_str()) {
                return Err(ComponentVerificationRejection::InconsistentProviderClosure(
                    "two providers claim the same requirement",
                ));
            }
        }
    }
    if !description.providers.is_empty() && description.provider_closure_digest == [0; 32] {
        return Err(ComponentVerificationRejection::InconsistentProviderClosure(
            "a nonempty provider roster carries a zero closure digest",
        ));
    }
    Ok(())
}

/// Custody rows must cover the derived set under the same evidence rules as
/// entries: derived rows required, `ModuleDerived` rows replayed, and
/// `AssumptionBound` rows bound to the roster.
fn check_custody(
    description: &ComponentDescription,
    inventory: &crate::component_description::DerivedInventory,
) -> Result<(), ComponentVerificationRejection> {
    let derived: BTreeSet<(u8, &str)> = inventory
        .custody
        .iter()
        .map(|row| (row.kind.tag(), row.identity.as_str()))
        .collect();
    let mut seen = BTreeSet::new();
    for row in &description.custody {
        match row.evidence {
            CustodyEvidence::ModuleDerived => {
                let key = (row.kind.tag(), row.identity.as_str());
                if !derived.contains(&key) {
                    return Err(ComponentVerificationRejection::UnexpectedDerivedCustody(
                        row.identity.clone(),
                    ));
                }
            }
            CustodyEvidence::AssumptionBound(digest) => {
                require_bound_assumption(&description.assumptions, digest)?;
            }
        }
        seen.insert((row.kind.tag(), row.identity.as_str()));
    }
    for (tag, identity) in derived {
        if !seen.contains(&(tag, identity)) {
            return Err(ComponentVerificationRejection::MissingCustodyConstraint(
                identity.to_string(),
            ));
        }
    }
    Ok(())
}

/// The service-bound roster must replay the module's retained
/// installation-bound reach rows exactly: each requirement identity carries
/// the same declared bound it does in the module. Bounds publish here and
/// never inside the concrete reach's `ServiceCeiling` outgoing rows, so this
/// roster is the only place a consumer reads what installation still owes.
fn check_service_bounds(
    description: &ComponentDescription,
    inventory: &crate::component_description::DerivedInventory,
) -> Result<(), ComponentVerificationRejection> {
    let derived: BTreeSet<&InstallationServiceBound> = inventory.service_bounds.iter().collect();
    let declared: BTreeSet<&InstallationServiceBound> = description.service_bounds.iter().collect();
    if let Some(row) = derived.difference(&declared).next() {
        return Err(ComponentVerificationRejection::MissingServiceBound(
            row.requirement_identity.clone(),
        ));
    }
    if let Some(row) = declared.difference(&derived).next() {
        return Err(ComponentVerificationRejection::UnexpectedServiceBound(
            row.requirement_identity.clone(),
        ));
    }
    Ok(())
}

/// Every import slot must name an unsealed boundary requirement with the
/// canonical contract identity.
fn check_imports(description: &ComponentDescription) -> Result<(), ComponentVerificationRejection> {
    let unsealed: BTreeSet<&str> = description
        .outgoing
        .iter()
        .filter(|row| {
            row.class == OutgoingAuthorityClass::BoundaryRequirement
                && matches!(row.evidence, OutgoingEvidence::ModuleDerived)
        })
        .map(|row| row.identity.as_str())
        .collect();
    let mut slotted: BTreeSet<&str> = BTreeSet::new();
    for slot in &description.imports {
        if !unsealed.contains(slot.requirement_identity.as_str()) {
            return Err(ComponentVerificationRejection::SmuggledImport(
                slot.requirement_identity.clone(),
            ));
        }
        if slot.contract_identity != requirement_contract_identity(&slot.requirement_identity) {
            return Err(ComponentVerificationRejection::ImportContractMismatch(
                slot.requirement_identity.clone(),
            ));
        }
        slotted.insert(slot.requirement_identity.as_str());
    }
    for requirement in unsealed {
        if !slotted.contains(requirement) {
            return Err(ComponentVerificationRejection::UnboundRequirement(
                requirement.to_string(),
            ));
        }
    }
    Ok(())
}

/// Obligations that must be present because the artifact demands them:
/// an import binding per import slot, a provider occurrence per retained
/// provider, and a resource admission per program-local root-introduction
/// custody row. Stack and progress obligations are declared facts the
/// description carries but the artifact alone cannot derive; they remain
/// obligations either way.
fn check_obligations(
    description: &ComponentDescription,
    inventory: &crate::component_description::DerivedInventory,
) -> Result<(), ComponentVerificationRejection> {
    let present: BTreeSet<(u8, &str)> = description
        .obligations
        .iter()
        .map(|obligation| (obligation.kind.tag(), obligation.identity.as_str()))
        .collect();
    let require = |kind: ObligationKind, identity: String| {
        if !present.contains(&(kind.tag(), identity.as_str())) {
            return Err(ComponentVerificationRejection::MissingObligation(identity));
        }
        Ok(())
    };
    for slot in &description.imports {
        require(
            ObligationKind::ImportBinding,
            slot.requirement_identity.clone(),
        )?;
    }
    for provider in &description.providers {
        require(
            ObligationKind::ProviderOccurrence,
            format!("provider-occurrence:{}", hex(&provider.plan_digest)),
        )?;
    }
    for row in &inventory.custody {
        if row.kind == CustodyKind::ProgramLocalRootIntroduction {
            require(ObligationKind::ResourceAdmission, row.identity.clone())?;
        }
    }
    Ok(())
}

fn require_bound_assumption(
    roster: &[[u8; 32]],
    digest: [u8; 32],
) -> Result<(), ComponentVerificationRejection> {
    if !roster.contains(&digest) {
        return Err(ComponentVerificationRejection::UnboundAssumptionReference(
            digest,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
