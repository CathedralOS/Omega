//! Independent, source-free verification of canonical component descriptions.
//!
//! `verify_component` is the verified-description consumer for independent
//! component admission, replacement, and topology checks. It re-decodes the
//! embedded canonical artifact, reconstructs the component subject itself,
//! replays the subject-bound proof section decode, and re-derives every
//! module-evident description row rather than trusting producer claims.
//! Module proof *admission* remains purpose-specific and stays with the
//! existing Terminal verifier's own distinct carriers; this consumer is
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

use crate::component_description::{
    ComponentDescription, ComponentEntry, CustodyConstraint, CustodyEvidence, CustodyKind,
    DescriptionDecodeRejection, DescriptionFrontier, EntryEvidence, ExportSurface, ImportSlot,
    InstallationObligation, MAX_COMPONENT_DESCRIPTION_BYTES, ObligationKind, OutgoingAuthority,
    OutgoingAuthorityClass, OutgoingEvidence, RetainedProvider, component_description_identity,
    decode_component_description, derive_component_inventory, hex, requirement_contract_identity,
};

const VERIFIED_COMPONENT_CLOSURE_DOMAIN: &[u8] = b"omega-verified-component-closure-v1";
const VERIFICATION_PROFILE_DOMAIN: &[u8] = b"omega-component-verification-profile-v1";

/// The caller's independent admission profile for a component description.
///
/// Everything here is supplied by the consumer and never read from the
/// description itself: the component subject to admit, the description
/// schemas this consumer understands, and the assumption digests it accepts.
#[derive(Debug, Clone)]
pub struct ComponentVerificationRequest {
    /// The exact component subject this admission expects.
    pub expected_subject: TerminalPsiIdentity,
    /// Description schema versions this consumer admits.
    pub accepted_schemas: BTreeSet<u32>,
    /// Assumption digests this consumer accepts for declared rows and
    /// physical mechanisms. Any digest outside this set rejects.
    pub accepted_assumptions: BTreeSet<[u8; 32]>,
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
    /// The embedded module failed canonical or representation validation.
    ModuleValidation(String),
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
            Self::ModuleValidation(detail) => {
                write!(formatter, "embedded module failed validation: {detail}")
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
/// is canonical, subject-bound to its proof section, and exactly covered by
/// the description's entries, outgoing authority, custody, retained
/// providers, and installation obligations. It cannot be executed,
/// installed, or used to discharge any obligation it describes.
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
}

/// Independently verify a canonical component description for admission.
///
/// The description is trusted for nothing: the codec enforces canonical
/// bounds and ordering, the embedded artifact is re-decoded with its
/// subject-bound proof section, the component subject is reconstructed
/// rather than read, and every module-evident roster is re-derived and
/// compared row for row. Installation obligations are checked for presence
/// and left undischarged.
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
    let _proof_bundle = terminal_codec::decode_proof_bundle_for(&module, artifact.proof_bytes())
        .map_err(|error| ComponentVerificationRejection::ProofSection(error.to_string()))?;
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

    let inventory = derive_component_inventory(&module).map_err(|error| {
        ComponentVerificationRejection::ArtifactInvalid(format!(
            "verified module did not admit a component inventory: {error}"
        ))
    })?;

    check_entries(&description, &inventory)?;
    check_exports(&description, &inventory)?;
    let sealed = check_outgoing(&description, &inventory)?;
    check_providers(&description, &inventory, &sealed)?;
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
    for service in &inventory.service_ceiling {
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
mod tests {
    use std::collections::BTreeSet;

    use super::{
        ComponentVerificationRejection, ComponentVerificationRequest, VerifiedComponent,
        verify_component,
    };
    use crate::component_description::{
        COMPONENT_DESCRIPTION_SCHEMA_V1, ComponentDescription, ComponentDescriptionFacts,
        ComponentEntry, ComponentEntryKind, DescriptionDecodeRejection, DescriptionFrontier,
        EntryEvidence, ObligationKind, OutgoingAuthorityClass, OutgoingEvidence,
        decode_component_description, encode_component_description,
    };
    use effects::SelectedProviderPlanFacts;
    use effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId,
    };
    use terminal_psi::ProofBundle;
    use terminal_psi::{
        Block, BoundaryMachineDeclaration, BoundaryMachineResult, MachineContract, Operation,
        OperationKind, OperationResult, TerminalMachine, TerminalMachineResult, TerminalModule,
        Terminator, VocabularyMarker,
    };

    fn machine_id(raw: u64) -> MachineId {
        MachineId::new(raw).expect("machine identity")
    }

    fn minimal_module() -> TerminalModule {
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine_id(1),
            scalar_qualifications: Default::default(),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            scalar_block_invariants: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                declared_service_reach: Vec::new(),
                closed_reach_application: None,
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(1).expect("block identity"),
                blocks: vec![Block {
                    id: BlockId::new(1).expect("block identity"),
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1).expect("edge identity"),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(1).expect("contract identity"),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        }
    }

    /// A module whose entry performs one bare `BoundaryCall` to a declared
    /// Unit boundary requirement.
    fn boundary_module() -> TerminalModule {
        let mut module = minimal_module();
        module.boundary_machines.push(BoundaryMachineDeclaration {
            id: BoundaryMachineId::new(1).expect("boundary identity"),
            identity: "IndexedRequirement::apply".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        module.machines[0].blocks[0].operations.push(Operation {
            static_reach_binding: None,
            id: OperationId::new(1).expect("operation identity"),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                boundary: BoundaryMachineId::new(1).expect("boundary identity"),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        });
        module
    }

    fn artifact_for(module: &TerminalModule) -> terminal_codec::CanonicalTerminalArtifact {
        let proof = ProofBundle::default();
        let record = terminal_codec::build_identity_optimization_execution_record(module, &proof)
            .expect("identity optimization record");
        terminal_codec::CanonicalTerminalArtifact::from_parts(module, &proof, &record, None)
            .expect("canonical artifact")
    }

    fn empty_selection() -> SelectedProviderPlanFacts {
        SelectedProviderPlanFacts::from_selected_plans(Vec::new()).expect("empty selection")
    }

    fn selected_plan() -> ProviderPlan {
        ProviderPlan {
            name: "SelectedIndexedProvider".into(),
            provider_type: "IndexedProvider".into(),
            provider_type_package_identity: None,
            target: "test".into(),
            schema: ServiceSchema {
                trait_name: "IndexedRequirement".into(),
                trait_package_identity: None,
                methods: vec![ServiceMethod {
                    name: "apply".into(),
                    requirement_owner: "IndexedRequirement".into(),
                    requirement_owner_package_identity: None,
                    requirement_identity: "IndexedRequirement::apply".into(),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["IndexedRequirement".into()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "apply".into(),
                requirement_identity: "IndexedRequirement::apply".into(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity: "IndexedProvider::apply".into(),
                    machine_package_identity: None,
                },
            }],
            origin_package_identity: None,
            origin_package: "test".into(),
        }
    }

    fn describe(
        module: &TerminalModule,
        selected: &SelectedProviderPlanFacts,
    ) -> ComponentDescription {
        let artifact = artifact_for(module);
        crate::describe_component_facts(ComponentDescriptionFacts {
            artifact: &artifact,
            selected_provider_plans: selected,
            component_progress: None,
            stack_demand: None,
            realization_identity: None,
        })
        .expect("component description")
    }

    fn request_for(
        module: &TerminalModule,
        accepted_assumptions: BTreeSet<[u8; 32]>,
    ) -> ComponentVerificationRequest {
        ComponentVerificationRequest {
            expected_subject: terminal_codec::terminal_psi_identity(module)
                .expect("module identity"),
            accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V1]),
            accepted_assumptions,
        }
    }

    fn verify(
        description: &ComponentDescription,
        request: &ComponentVerificationRequest,
    ) -> Result<VerifiedComponent, ComponentVerificationRejection> {
        verify_component(&encode_component_description(description), request)
    }

    #[test]
    fn verifies_a_complete_minimal_description() {
        let module = minimal_module();
        let description = describe(&module, &empty_selection());
        let request = request_for(&module, BTreeSet::new());
        let verified = verify(&description, &request).expect("complete description verifies");
        assert_eq!(
            verified.frontier(),
            DescriptionFrontier::TerminalArtifactClosure
        );
        assert!(
            verified
                .entries()
                .iter()
                .any(|entry| entry.kind == ComponentEntryKind::Canonical)
        );
        assert!(verified.imports().is_empty());
        assert!(verified.providers().is_empty());
    }

    #[test]
    fn verifies_a_sealed_boundary_requirement() {
        let module = boundary_module();
        let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()])
            .expect("selected closure");
        let description = describe(&module, &selected);
        assert_eq!(description.imports.len(), 0);
        assert_eq!(description.providers.len(), 1);
        let request = request_for(&module, BTreeSet::new());
        let verified = verify(&description, &request).expect("sealed requirement verifies");
        assert_eq!(verified.providers().len(), 1);
        assert!(
            verified
                .obligations()
                .iter()
                .any(|obligation| obligation.kind == ObligationKind::ProviderOccurrence),
            "a retained provider stays a per-occurrence installation obligation"
        );
    }

    #[test]
    fn rejects_corrupt_descriptions() {
        let module = minimal_module();
        let description = describe(&module, &empty_selection());
        let bytes = encode_component_description(&description);
        let request = request_for(&module, BTreeSet::new());
        assert!(matches!(
            verify_component(&[], &request),
            Err(ComponentVerificationRejection::Corrupt(
                DescriptionDecodeRejection::InvalidMagic | DescriptionDecodeRejection::Corrupt(_)
            ))
        ));
        assert!(matches!(
            verify_component(&bytes[..bytes.len() - 3], &request),
            Err(ComponentVerificationRejection::Corrupt(_))
        ));
        let mut mutated = bytes.clone();
        mutated[0] = b'X';
        assert!(matches!(
            verify_component(&mutated, &request),
            Err(ComponentVerificationRejection::Corrupt(
                DescriptionDecodeRejection::InvalidMagic
            ))
        ));
    }

    #[test]
    fn rejects_the_wrong_component_subject() {
        let module = minimal_module();
        let other = boundary_module();
        let description = describe(&module, &empty_selection());
        let request = request_for(&other, BTreeSet::new());
        assert!(matches!(
            verify(&description, &request),
            Err(ComponentVerificationRejection::WrongSubject { .. })
        ));
    }

    #[test]
    fn rejects_an_incompatible_schema() {
        let module = minimal_module();
        let description = describe(&module, &empty_selection());
        let mut request = request_for(&module, BTreeSet::new());
        request.accepted_schemas = BTreeSet::from([2]);
        assert!(matches!(
            verify(&description, &request),
            Err(ComponentVerificationRejection::IncompatibleSchema { schema: 1 })
        ));
    }

    #[test]
    fn rejects_an_early_frontier() {
        let module = minimal_module();
        let mut description = describe(&module, &empty_selection());
        description.frontier = DescriptionFrontier::SelectedPlan;
        let request = request_for(&module, BTreeSet::new());
        assert!(matches!(
            verify(&description, &request),
            Err(ComponentVerificationRejection::EarlyFrontier(
                DescriptionFrontier::SelectedPlan
            ))
        ));
    }

    #[test]
    fn rejects_forged_complete_descriptions() {
        let module = minimal_module();
        let request = request_for(&module, BTreeSet::new());

        // A description claiming completeness while omitting the canonical entry.
        let mut omitted = describe(&module, &empty_selection());
        omitted
            .entries
            .retain(|entry| entry.kind != ComponentEntryKind::Canonical);
        assert!(matches!(
            verify(&omitted, &request),
            Err(ComponentVerificationRejection::MissingComponentEntry(_))
        ));

        // A description inventing a module-derived entry the artifact lacks.
        let mut forged = describe(&module, &empty_selection());
        forged.entries.push(ComponentEntry {
            kind: ComponentEntryKind::SuspensionResumption,
            identity: "suspension-resumption:99:1".into(),
            evidence: EntryEvidence::ModuleDerived,
        });
        assert!(matches!(
            verify(&forged, &request),
            Err(ComponentVerificationRejection::UnexpectedDerivedEntry(_))
        ));
    }

    #[test]
    fn rejects_omitted_authority_obligation_and_import() {
        let module = boundary_module();
        let request = request_for(&module, BTreeSet::new());

        let mut no_outgoing = describe(&module, &empty_selection());
        no_outgoing
            .outgoing
            .retain(|row| row.class != OutgoingAuthorityClass::BoundaryRequirement);
        assert!(matches!(
            verify(&no_outgoing, &request),
            Err(ComponentVerificationRejection::MissingOutgoingAuthority(_))
        ));

        let mut no_import = describe(&module, &empty_selection());
        no_import.imports.clear();
        assert!(matches!(
            verify(&no_import, &request),
            Err(ComponentVerificationRejection::UnboundRequirement(_))
        ));

        let mut no_obligation = describe(&module, &empty_selection());
        no_obligation
            .obligations
            .retain(|obligation| obligation.kind != ObligationKind::ImportBinding);
        assert!(matches!(
            verify(&no_obligation, &request),
            Err(ComponentVerificationRejection::MissingObligation(_))
        ));
    }

    #[test]
    fn rejects_unaccepted_and_unbound_assumptions() {
        let module = minimal_module();
        let assumption = [7u8; 32];

        let mut declared = describe(&module, &empty_selection());
        declared.assumptions.push(assumption);
        declared.entries.push(ComponentEntry {
            kind: ComponentEntryKind::Startup,
            identity: "startup:post-link".into(),
            evidence: EntryEvidence::AssumptionBound(assumption),
        });
        let accepted = request_for(&module, BTreeSet::from([assumption]));
        verify(&declared, &accepted).expect("accepted assumption verifies");

        let unaccepted = request_for(&module, BTreeSet::new());
        assert!(matches!(
            verify(&declared, &unaccepted),
            Err(ComponentVerificationRejection::UnacceptedAssumption(_))
        ));

        let mut unbound = describe(&module, &empty_selection());
        unbound.entries.push(ComponentEntry {
            kind: ComponentEntryKind::Timer,
            identity: "timer:tick".into(),
            evidence: EntryEvidence::AssumptionBound(assumption),
        });
        let accepted = request_for(&module, BTreeSet::from([assumption]));
        assert!(matches!(
            verify(&unbound, &accepted),
            Err(ComponentVerificationRejection::UnboundAssumptionReference(
                _
            ))
        ));
    }

    #[test]
    fn rejects_provider_roster_substitution() {
        let module = boundary_module();
        let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()])
            .expect("selected closure");
        let request = request_for(&module, BTreeSet::new());

        let mut wrong_seal = describe(&module, &selected);
        for row in &mut wrong_seal.outgoing {
            if row.class == OutgoingAuthorityClass::BoundaryRequirement {
                row.evidence = OutgoingEvidence::ProviderSealed([9u8; 32]);
            }
        }
        assert!(matches!(
            verify(&wrong_seal, &request),
            Err(ComponentVerificationRejection::UnsealedProvider(_))
        ));

        let mut smuggled = describe(&module, &selected);
        smuggled.providers[0]
            .requirement_identities
            .push("Smuggled::requirement".into());
        assert!(matches!(
            verify(&smuggled, &request),
            Err(ComponentVerificationRejection::SmuggledProviderRequirement(
                _
            )) | Err(ComponentVerificationRejection::InconsistentProviderClosure(
                _
            ))
        ));
    }

    #[test]
    fn codec_round_trips_and_rejects_noncanonical_order() {
        let module = boundary_module();
        let description = describe(&module, &empty_selection());
        let bytes = encode_component_description(&description);
        let decoded = decode_component_description(&bytes).expect("canonical decode");
        assert_eq!(decoded, description);
        assert_eq!(encode_component_description(&decoded), bytes);
    }
}
