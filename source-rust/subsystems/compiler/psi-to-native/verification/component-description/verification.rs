//! Check the complete component profile and every retained row against the published artifact.

use std::collections::{BTreeMap, BTreeSet};

use terminal_psi::{TerminalModule, TerminalPsiIdentity};

use crate::inventory::{
    DerivedInventory, component_description_identity, derive_component_inventory, identity_text,
    requirement_contract_identity, requirement_export_identity,
};
use crate::records::{
    ComponentDescription, ComponentEntry, CustodyConstraint, CustodyEvidence, CustodyKind,
    DescriptionDecodeRejection, DescriptionFrontier, EntryEvidence, ExportSurface, ImportSlot,
    InstallationObligation, InstallationServiceBound, MAX_COMPONENT_DESCRIPTION_BYTES,
    ObligationKind, OutgoingAuthority, OutgoingAuthorityClass, OutgoingEvidence, RetainedProvider,
};
use crate::retained_plan::{ProviderPlanInput, ProviderPlanInputRow};
use crate::wire_format::decode_component_description_with_artifact;
use ::wire::digest_encoder::{self, hex};

const VERIFIED_COMPONENT_CLOSURE_DOMAIN: &[u8] = b"omega-verified-component-closure-v1";
const VERIFICATION_PROFILE_DOMAIN: &[u8] = b"omega-component-verification-profile-v1";

#[derive(Debug, Clone)]
pub struct ComponentVerificationRequest {
    pub expected_subject: TerminalPsiIdentity,
    pub accepted_schemas: BTreeSet<u32>,
    pub accepted_assumptions: AcceptedComponentAssumptions,
}

/*
Which environmental premises the verifier may admit (component_publication.md,
"Consumer acceptance and Build attenuation"). A standalone verifier names the
exact accepted digests. Product compilation instead verifies everything else
about the description and leaves the description's own roster pending: the
consuming project's lock acceptance owns that decision, and root Product
admission compares the canonical component-assumption rows against it before
any artifact is produced. Pending is never acceptance; it only moves the
comparison to the owner that holds the consumer's reviewed rows.
*/
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptedComponentAssumptions {
    Exact(BTreeSet<[u8; 32]>),
    PendingConsumerAcceptance,
}

impl ComponentVerificationRequest {
    fn profile_identity(&self, roster: &[[u8; 32]]) -> [u8; 32] {
        let mut digest = digest_encoder::DomainDigestEncoder::new(VERIFICATION_PROFILE_DOMAIN);
        digest.raw(&self.expected_subject.vocabulary_marker.get().to_le_bytes());
        digest.raw(self.expected_subject.program_fingerprint.as_bytes());
        digest.u64(self.accepted_schemas.len() as u64);
        for schema in &self.accepted_schemas {
            digest.raw(&schema.to_le_bytes());
        }
        let pending: BTreeSet<[u8; 32]>;
        let accepted = match &self.accepted_assumptions {
            AcceptedComponentAssumptions::Exact(accepted) => accepted,
            AcceptedComponentAssumptions::PendingConsumerAcceptance => {
                pending = roster.iter().copied().collect();
                &pending
            }
        };
        digest.u64(accepted.len() as u64);
        for assumption in accepted {
            digest.raw(assumption);
        }
        digest.finish()
    }
}

#[derive(Debug)]
pub enum ComponentVerificationRejection {
    Corrupt(DescriptionDecodeRejection),
    IncompatibleSchema {
        schema: u32,
    },
    WrongSubject {
        reconstructed: Box<TerminalPsiIdentity>,
        expected: Box<TerminalPsiIdentity>,
    },
    ArtifactInvalid(String),
    ProofSection(String),
    ModuleVerification(String),
    EarlyFrontier(DescriptionFrontier),
    UnacceptedAssumption([u8; 32]),
    UnboundAssumptionReference([u8; 32]),
    MissingComponentEntry(String),
    UnexpectedDerivedEntry(String),
    MissingExport(String),
    UnexpectedDerivedExport(String),
    MissingOutgoingAuthority(String),
    UnexpectedDerivedAuthority(String),
    InvalidAuthorityEvidence(String),
    UnsealedProvider(String),
    SmuggledProviderRequirement(String),
    UnboundRequirement(String),
    SmuggledImport(String),
    ImportContractMismatch(String),
    MissingCustodyConstraint(String),
    UnexpectedDerivedCustody(String),
    MissingServiceBound(String),
    UnexpectedServiceBound(String),
    MissingObligation(String),
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

#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedComponent {
    description: ComponentDescription,
    description_identity: [u8; 32],
    closure: [u8; 32],
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
}

impl VerifiedComponent {
    pub fn subject(&self) -> TerminalPsiIdentity {
        self.artifact.view().identity()
    }

    pub const fn closure(&self) -> &[u8; 32] {
        &self.closure
    }

    pub const fn frontier(&self) -> DescriptionFrontier {
        self.description.frontier
    }

    pub fn module(&self) -> &TerminalModule {
        self.artifact.semantic_module()
    }

    pub fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn shared_artifact(&self) -> &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }

    pub fn imports(&self) -> &[ImportSlot] {
        &self.description.imports
    }

    pub fn exports(&self) -> &[ExportSurface] {
        &self.description.exports
    }

    pub fn entries(&self) -> &[ComponentEntry] {
        &self.description.entries
    }

    pub fn outgoing(&self) -> &[OutgoingAuthority] {
        &self.description.outgoing
    }

    pub fn custody(&self) -> &[CustodyConstraint] {
        &self.description.custody
    }

    pub fn providers(&self) -> &[RetainedProvider] {
        &self.description.providers
    }

    pub const fn provider_closure_digest(&self) -> &[u8; 32] {
        &self.description.provider_closure_digest
    }

    pub fn obligations(&self) -> &[InstallationObligation] {
        &self.description.obligations
    }

    pub fn assumptions(&self) -> &[[u8; 32]] {
        &self.description.assumptions
    }

    /// A native realization adds its image locator and measured stack provision to the reviewed closure.
    pub fn verify_native_realization(
        &self,
        bytes: &[u8],
        locator: [u8; 32],
        stack: &InstallationObligation,
    ) -> Result<(), String> {
        let request = ComponentVerificationRequest {
            expected_subject: self.subject(),
            accepted_schemas: BTreeSet::from([self.description.schema]),
            accepted_assumptions: AcceptedComponentAssumptions::Exact(
                self.assumptions().iter().copied().collect(),
            ),
        };
        let mut physical = verify_component(bytes, &request)
            .map_err(|reason| reason.to_string())?
            .description;
        if stack.kind != ObligationKind::StackProvision
            || physical.realization_identity.take() != Some(locator)
        {
            return Err("native component description differs from its emitted image locator or stack provision".into());
        }
        let position = physical
            .obligations
            .iter()
            .position(|row| row == stack)
            .ok_or("native component description omits its measured stack provision")?;
        physical.obligations.remove(position);
        if physical != self.description {
            return Err(
                "native component description changes the reviewed portable closure".into(),
            );
        }
        Ok(())
    }

    /// Complete coordinates of the rows that rely on each environmental premise.
    /// The closure commits to the exact subject, description and verification profile.
    pub fn assumption_rows(&self) -> Result<BTreeMap<[u8; 32], Vec<String>>, &'static str> {
        let mut rows: BTreeMap<_, BTreeSet<_>> = self
            .assumptions()
            .iter()
            .map(|&digest| (digest, BTreeSet::new()))
            .collect();
        if rows.is_empty() {
            return Ok(BTreeMap::new());
        }
        let closure = hex(self.closure());
        let mut bind = |digest, family, kind, identity: &str| {
            let set = rows
                .get_mut(&digest)
                .ok_or("component evidence uses an undeclared assumption")?;
            set.insert(format!(
                "{family} {kind} {identity} in verified closure {closure}"
            ));
            Ok::<_, &'static str>(())
        };
        for entry in self.entries() {
            if let EntryEvidence::AssumptionBound(digest) = entry.evidence {
                bind(digest, "entry", entry.kind.tag(), &entry.identity)?;
            }
        }
        for outgoing in self.outgoing() {
            match outgoing.evidence {
                OutgoingEvidence::PhysicalMechanism(digest) => bind(
                    digest,
                    "physical mechanism",
                    outgoing.class.tag(),
                    &outgoing.identity,
                )?,
                OutgoingEvidence::AssumptionBound(digest) => bind(
                    digest,
                    "outgoing authority",
                    outgoing.class.tag(),
                    &outgoing.identity,
                )?,
                OutgoingEvidence::ModuleDerived | OutgoingEvidence::ProviderSealed(_) => {}
            }
        }
        for custody in self.custody() {
            if let CustodyEvidence::AssumptionBound(digest) = custody.evidence {
                bind(digest, "custody", custody.kind.tag(), &custody.identity)?;
            }
        }
        rows.into_iter()
            .map(|(digest, coordinates)| {
                if coordinates.is_empty() {
                    return Err("a component assumption has no bound review row");
                }
                Ok((digest, coordinates.into_iter().collect()))
            })
            .collect()
    }

    pub const fn realization_identity(&self) -> Option<&[u8; 32]> {
        self.description.realization_identity.as_ref()
    }

    /// Match a candidate from this module to its verified, fully qualified export.
    pub fn exports_candidate_at(&self, position: usize) -> bool {
        let module = self.module();
        let Some(candidate) = module.interfaces.provider_candidates.get(position) else {
            return false;
        };
        let export = requirement_export_identity(
            identity_text(module, candidate.requirement_identity),
            identity_text(module, candidate.provider_identity),
            identity_text(module, candidate.candidate_identity),
        );
        self.description
            .exports
            .iter()
            .any(|surface| surface.identity == export)
    }
}

pub fn verify_component(
    description_bytes: &[u8],
    request: &ComponentVerificationRequest,
) -> Result<VerifiedComponent, ComponentVerificationRejection> {
    if description_bytes.len() > MAX_COMPONENT_DESCRIPTION_BYTES {
        return Err(ComponentVerificationRejection::Corrupt(
            DescriptionDecodeRejection::RosterBoundExceeded("description byte length"),
        ));
    }
    let (description, artifact) = decode_component_description_with_artifact(description_bytes)
        .map_err(ComponentVerificationRejection::Corrupt)?;
    if !request.accepted_schemas.contains(&description.schema) {
        return Err(ComponentVerificationRejection::IncompatibleSchema {
            schema: description.schema,
        });
    }
    let module = artifact.semantic_module();
    let reconstructed = terminal_codec::terminal_psi_identity(module)
        .map_err(|error| ComponentVerificationRejection::ArtifactInvalid(error.to_string()))?;
    if reconstructed != request.expected_subject {
        return Err(ComponentVerificationRejection::WrongSubject {
            reconstructed: Box::new(reconstructed),
            expected: Box::new(request.expected_subject),
        });
    }
    if description.frontier != DescriptionFrontier::TerminalArtifactClosure {
        return Err(ComponentVerificationRejection::EarlyFrontier(
            description.frontier,
        ));
    }
    for assumption in &description.assumptions {
        if let AcceptedComponentAssumptions::Exact(accepted) = &request.accepted_assumptions
            && !accepted.contains(assumption)
        {
            return Err(ComponentVerificationRejection::UnacceptedAssumption(
                *assumption,
            ));
        }
    }

    let inventory = derive_component_inventory(module).map_err(|error| {
        ComponentVerificationRejection::ArtifactInvalid(format!(
            "verified module did not admit a component inventory: {error}"
        ))
    })?;

    check_entries(&description, &inventory)?;
    check_exports(&description, &inventory)?;
    let sealed = check_outgoing(&description, &inventory)?;
    check_providers(&description, module, &inventory, &sealed)?;
    check_service_bounds(&description, &inventory)?;
    check_custody(&description, &inventory)?;
    check_imports(&description)?;
    check_obligations(&description, &inventory)?;

    let description_identity = component_description_identity(description_bytes);
    let mut digest = digest_encoder::DomainDigestEncoder::new(VERIFIED_COMPONENT_CLOSURE_DOMAIN);
    digest.raw(&description_identity);
    digest.raw(&request.profile_identity(&description.assumptions));
    let closure = digest.finish();

    Ok(VerifiedComponent {
        description,
        description_identity,
        closure,
        artifact: std::sync::Arc::new(artifact),
    })
}

macro_rules! check_derived_roster {
    ($description:expr, $inventory:expr, $rows:ident, $evidence:ident, $unexpected:ident, $missing:ident) => {{
        let derived: BTreeSet<(u8, &str)> = $inventory
            .$rows
            .iter()
            .map(|row| (row.kind.tag(), row.identity.as_str()))
            .collect();
        let mut seen = BTreeSet::new();
        for row in &$description.$rows {
            match row.evidence {
                $evidence::ModuleDerived => {
                    let key = (row.kind.tag(), row.identity.as_str());
                    if !derived.contains(&key) {
                        return Err(ComponentVerificationRejection::$unexpected(
                            row.identity.clone(),
                        ));
                    }
                }
                $evidence::AssumptionBound(digest) => {
                    require_bound_assumption(&$description.assumptions, digest)?;
                }
            }
            seen.insert((row.kind.tag(), row.identity.as_str()));
        }
        for (tag, identity) in derived {
            if !seen.contains(&(tag, identity)) {
                return Err(ComponentVerificationRejection::$missing(
                    identity.to_string(),
                ));
            }
        }
        Ok(())
    }};
}

fn check_entries(
    description: &ComponentDescription,
    inventory: &DerivedInventory,
) -> Result<(), ComponentVerificationRejection> {
    check_derived_roster!(
        description,
        inventory,
        entries,
        EntryEvidence,
        UnexpectedDerivedEntry,
        MissingComponentEntry
    )
}

fn check_exports(
    description: &ComponentDescription,
    inventory: &DerivedInventory,
) -> Result<(), ComponentVerificationRejection> {
    let derived: BTreeSet<&str> = inventory
        .export_contracts
        .iter()
        .map(|contract| contract.identity.as_str())
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

fn check_outgoing(
    description: &ComponentDescription,
    inventory: &DerivedInventory,
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

    // Physical premises bind one exact operation derived from the module.
    let mut derived_keys = BTreeMap::new();
    for requirement in &inventory.called_requirement_identities {
        derived_keys.insert(
            (
                OutgoingAuthorityClass::BoundaryRequirement,
                requirement.clone(),
            ),
            None,
        );
    }
    for &(service, port, value) in &inventory.port_writes {
        let identity = format!("port-write:{}:{port}:{value}", service.get());
        derived_keys.insert(
            (OutgoingAuthorityClass::PortSpaceWrite, identity),
            Some(crate::inventory::port_mechanism_assumption(
                service, port, value,
            )),
        );
    }
    for &(service, operation) in &inventory.register_accesses {
        let identity = format!(
            "register-access:{}:{}",
            service.get(),
            operation.catalog_tag()
        );
        derived_keys.insert(
            (OutgoingAuthorityClass::RegisterAccess, identity),
            Some(crate::inventory::register_mechanism_assumption(
                service, operation,
            )),
        );
    }
    for service in &inventory.concrete_service_reach {
        let identity = format!("service-ceiling:{}", service.get());
        derived_keys.insert((OutgoingAuthorityClass::ServiceCeiling, identity), None);
    }

    let mut sealed: BTreeMap<String, [u8; 32]> = BTreeMap::new();
    for row in &description.outgoing {
        let key = (row.class, row.identity.clone());
        let expected = derived_keys.get(&key);
        let derived = expected.is_some();
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
            (OutgoingAuthorityClass::ServiceCeiling, OutgoingEvidence::ModuleDerived, true) => {}
            (
                OutgoingAuthorityClass::PortSpaceWrite | OutgoingAuthorityClass::RegisterAccess,
                OutgoingEvidence::PhysicalMechanism(assumption),
                true,
            ) if expected == Some(&Some(assumption)) => {
                require_bound_assumption(&description.assumptions, assumption)?;
            }
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

    let described_keys: BTreeSet<(OutgoingAuthorityClass, &str)> = description
        .outgoing
        .iter()
        .map(|row| (row.class, row.identity.as_str()))
        .collect();
    for (class, identity) in derived_keys.into_keys() {
        if !described_keys.contains(&(class, identity.as_str())) {
            return Err(ComponentVerificationRejection::MissingOutgoingAuthority(
                identity,
            ));
        }
    }

    // Each called requirement reads the first boundary-requirement row that
    // names it, so keep only the first evidence per identity.
    let mut boundary_evidence: BTreeMap<&str, OutgoingEvidence> = BTreeMap::new();
    for row in &description.outgoing {
        if row.class == OutgoingAuthorityClass::BoundaryRequirement {
            boundary_evidence
                .entry(row.identity.as_str())
                .or_insert(row.evidence);
        }
    }
    let imported: BTreeSet<&str> = description
        .imports
        .iter()
        .map(|slot| slot.requirement_identity.as_str())
        .collect();
    for requirement in &inventory.called_requirement_identities {
        match boundary_evidence.get(requirement.as_str()) {
            Some(OutgoingEvidence::ProviderSealed(_)) => {}
            Some(OutgoingEvidence::ModuleDerived) => {
                if !imported.contains(requirement.as_str()) {
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

fn check_providers(
    description: &ComponentDescription,
    module: &terminal_psi::TerminalModule,
    inventory: &DerivedInventory,
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
        let plan = ProviderPlanInput::read(&provider.plan_input, &provider.plan_digest)
            .map_err(ComponentVerificationRejection::InconsistentProviderClosure)?;
        let planned: BTreeSet<&str> = plan.rows.iter().map(|row| row.requirement).collect();
        if provider
            .requirement_identities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || provider
                .requirement_identities
                .iter()
                .any(|requirement| !planned.contains(requirement.as_str()))
        {
            return Err(ComponentVerificationRejection::InconsistentProviderClosure(
                "retained requirement claims are not a canonical subset of the complete committed plan",
            ));
        }
        // The check above proved the claims strictly sorted, so a binary
        // search finds each plan row's claim.
        for row in plan.rows.iter().filter(|row| {
            provider
                .requirement_identities
                .binary_search_by(|claimed| claimed.as_str().cmp(row.requirement))
                .is_ok()
        }) {
            let requirement = row.requirement;
            if called.contains(requirement) {
                match sealed.get(requirement) {
                    Some(digest) if *digest == provider.plan_digest => {}
                    _ => {
                        return Err(ComponentVerificationRejection::InconsistentProviderClosure(
                            "a called provider requirement is not sealed to that provider's plan",
                        ));
                    }
                }
            } else if !is_checked_export(module, &plan, row) {
                return Err(ComponentVerificationRejection::SmuggledProviderRequirement(
                    requirement.to_owned(),
                ));
            }
            if !claimed.insert(requirement) {
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

fn is_checked_export(
    module: &terminal_psi::TerminalModule,
    plan: &ProviderPlanInput<'_>,
    row: &ProviderPlanInputRow<'_>,
) -> bool {
    let Some((machine, package)) = row.checked_adapter else {
        return false;
    };
    if plan.provider.is_empty()
        || plan.provider_package != package
        || plan.origin_package != package
    {
        return false;
    }
    let mut candidates = module.interfaces.provider_candidates.iter().filter(|candidate| {
        module
            .identities
            .text(candidate.source_requirement_identity)
            == Some(row.requirement)
            && module.identities.text(candidate.provider_identity) == Some(plan.provider)
            && module.identities.text(candidate.source_candidate_identity) == Some(machine)
            && candidate
                .source_candidate_package_identity
                .map(|identity| identity.digest())
                == package
            && module
                .identities
                .text(candidate.provider_declaration.identity())
                .is_some_and(|identity| !identity.is_empty())
            && module
                .machine(candidate.candidate)
                .and_then(|body| body.declaration.as_ref())
                .is_some_and(|declaration| {
                    module.identities.text(declaration.callable)
                        == module.identities.text(candidate.candidate_identity)
                        && module.identities.text(
                            declaration
                                .specialization
                                .as_ref()
                                .map_or(declaration.callable, |specialization| {
                                    specialization.template_callable
                                }),
                        ) == Some(machine)
                })
    });
    candidates.next().is_some() && candidates.next().is_none()
}

fn check_custody(
    description: &ComponentDescription,
    inventory: &DerivedInventory,
) -> Result<(), ComponentVerificationRejection> {
    check_derived_roster!(
        description,
        inventory,
        custody,
        CustodyEvidence,
        UnexpectedDerivedCustody,
        MissingCustodyConstraint
    )
}

fn check_service_bounds(
    description: &ComponentDescription,
    inventory: &DerivedInventory,
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

fn check_obligations(
    description: &ComponentDescription,
    inventory: &DerivedInventory,
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
mod outgoing_premises {
    use super::*;

    #[test]
    fn a_port_mechanism_binds_its_exact_checked_operation() {
        let (mut description, artifact) =
            decode_component_description_with_artifact(&crate::tests::provider_free_component())
                .unwrap();
        let mut inventory = derive_component_inventory(artifact.semantic_module()).unwrap();
        let service = semantic_vocabulary::ServiceId::new(1).unwrap();
        inventory.port_writes.insert((service, 7, 1));
        let exact = crate::inventory::port_mechanism_assumption(service, 7, 1);
        let other = crate::inventory::port_mechanism_assumption(service, 7, 2);
        description.assumptions.extend([exact, other]);
        let identity = format!("port-write:{}:7:1", service.get());
        description.outgoing.push(OutgoingAuthority {
            class: OutgoingAuthorityClass::PortSpaceWrite,
            identity: identity.clone(),
            evidence: OutgoingEvidence::PhysicalMechanism(exact),
        });
        assert!(check_outgoing(&description, &inventory).is_ok());
        // Knowing which operation occurs does not establish what the device does.
        // Both a different premise and an omitted environmental premise must be refused.
        for evidence in [
            OutgoingEvidence::PhysicalMechanism(other),
            OutgoingEvidence::ModuleDerived,
        ] {
            description.outgoing.last_mut().unwrap().evidence = evidence;
            assert!(matches!(check_outgoing(&description, &inventory),
                Err(ComponentVerificationRejection::InvalidAuthorityEvidence(rejected)) if rejected == identity));
        }
    }
}
