//! Install authorities, receipts, installed code, its registry authority
//! and installation diagnostics.

use crate::executable_installation::code_placement::ValidatedPlacementEvidence;
use crate::executable_installation::{
    ArtifactId, CodePlacementId, FinalValidationId, InstallationAudience, InstallationFactDigest,
    InstallationScopeId, InstalledCodeId, ValidatedPlacement,
};
use installation_evidence::InstalledArtifactOccurrenceDigest;
use layout_plans::{EntryStubId, PlacementConstraints, RelocationTarget};
use sha2::Digest;
use sha2::Sha256;
use target::Architecture;

/// One-shot authority to install one exact validated placement. Required
/// facts are open provider vocabulary — the receiver names in advance the
/// provider-canonical claims the contracted write-to-execute operation must
/// establish, such as its exact cache-order and instruction-fetch completion
/// facts — while instruction-fetch visibility and the W^X transition remain
/// mandatory lifecycle gates. An authority demanding no facts admits any
/// receipt that satisfies the lifecycle gates.
#[derive(Debug, PartialEq, Eq)]
pub struct InstallAuthority {
    pub(crate) validated: ValidatedPlacementEvidence,
    pub(crate) required_facts: std::collections::BTreeSet<InstallationFactDigest>,
}

impl InstallAuthority {
    pub fn from_admitted_provider(validated: &ValidatedPlacement) -> Self {
        Self {
            validated: ValidatedPlacementEvidence::from_validated(validated),
            required_facts: std::collections::BTreeSet::new(),
        }
    }

    /// Names the provider-canonical facts the installation receipt must
    /// establish, the same contract retirement authorities already impose.
    pub fn with_required_facts(
        mut self,
        required_facts: impl IntoIterator<Item = InstallationFactDigest>,
    ) -> Self {
        self.required_facts = required_facts.into_iter().collect();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WxEnforcement {
    HardwareEnforced,
    ConventionOnly,
    Unsupported,
}

/// Provider result of the one contracted write-to-execute operation. Beyond
/// the mandatory visibility and W^X claims it reports the provider-canonical
/// facts the operation established; the install gate requires the authority's
/// demanded set to appear here rather than recording assertions unchallenged.
#[derive(Debug, PartialEq, Eq)]
pub struct InstallationReceipt {
    pub(crate) installed: InstalledCodeId,
    pub(crate) validated: ValidatedPlacementEvidence,
    pub(crate) visibility_complete: bool,
    pub(crate) wx: WxEnforcement,
    pub(crate) established_facts: std::collections::BTreeSet<InstallationFactDigest>,
}

impl InstallationReceipt {
    pub fn from_provider(
        installed: InstalledCodeId,
        validated: &ValidatedPlacement,
        visibility_complete: bool,
        wx: WxEnforcement,
    ) -> Self {
        Self {
            installed,
            validated: ValidatedPlacementEvidence::from_validated(validated),
            visibility_complete,
            wx,
            established_facts: std::collections::BTreeSet::new(),
        }
    }

    /// Records the provider-canonical facts the operation established.
    pub fn with_established_facts(
        mut self,
        established_facts: impl IntoIterator<Item = InstallationFactDigest>,
    ) -> Self {
        self.established_facts = established_facts.into_iter().collect();
        self
    }
}

#[derive(Debug)]
pub struct InstallationError {
    pub(crate) validated: ValidatedPlacement,
    pub(crate) authority: InstallAuthority,
    pub(crate) receipt: InstallationReceipt,
    pub(crate) diagnostic: InstallationDiagnostic,
}

impl InstallationError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedPlacement, InstallAuthority, InstallationReceipt) {
        (self.validated, self.authority, self.receipt)
    }
}

/// Linear installed claim. It exposes identity/reporting only; callable entry
/// references are derived by the separate CFI/entry-reference gate.
#[derive(Debug)]
pub struct InstalledCode {
    pub(crate) identity: InstalledCodeId,
    pub(crate) validated: ValidatedPlacement,
    pub(crate) wx: WxEnforcement,
    pub(crate) installation_registry_claimed: bool,
}

/// One-shot authority to create the canonical installation-wide registry for
/// one exact installed-code occurrence.
///
/// This value is deliberately opaque and non-clonable. Compact installation
/// and artifact IDs are report keys only; equality retains the complete
/// installed-code evidence, including the exact placement scope and bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct InstallationRegistryAuthority {
    installed: InstalledCodeEvidence,
}

impl InstalledCode {
    pub const fn identity(&self) -> InstalledCodeId {
        self.identity
    }

    pub fn artifact(&self) -> ArtifactId {
        self.validated.frozen.artifact.artifact.0.identity
    }

    /// Exact installation scope carried by this admitted placement.
    pub const fn installation_scope(&self) -> InstallationScopeId {
        self.validated.frozen.placement.scope
    }

    /// Issue the sole registry authority for this installed-code occurrence.
    /// Dropping the authority does not reopen issuance: the burned claim is
    /// retained by `InstalledCode` until that occurrence is retired.
    pub fn claim_installation_registry(
        &mut self,
    ) -> Result<InstallationRegistryAuthority, InstallationDiagnostic> {
        if self.installation_registry_claimed {
            return Err(InstallationDiagnostic(
                "installation registry authority was already issued for this installed-code occurrence"
                    .into(),
            ));
        }
        self.installation_registry_claimed = true;
        Ok(InstallationRegistryAuthority {
            installed: InstalledCodeEvidence::from_installed(self),
        })
    }

    /// Test whether this exact installed realization came from one
    /// relocation-free artifact whose canonical and frozen bytes both equal
    /// the expected bytes. The bytes remain provider-side: callers receive
    /// only the sealed equality result needed to bind higher-level
    /// certificates.
    pub fn binds_exact_unrelocated_artifact_bytes(&self, expected: &[u8]) -> bool {
        let frozen = &self.validated.frozen;
        frozen.artifact.artifact.0.relocations.is_empty()
            && frozen.artifact.artifact.0.code == expected
            && frozen.materialized.bytes() == expected
    }

    /// Test both sides of a relocatable installation: the exact frozen
    /// compiler-authored bytes before relocation and the exact materialized
    /// bytes after the admitted relocation set was applied. No bytes or
    /// addresses cross this evidence boundary.
    pub fn binds_exact_materialized_artifact_bytes(
        &self,
        expected_unrelocated: &[u8],
        expected_materialized: &[u8],
    ) -> bool {
        let frozen = &self.validated.frozen;
        frozen.artifact.artifact.0.code == expected_unrelocated
            && frozen.materialized.bytes() == expected_materialized
    }

    /// Test the exact materialized byte interval beginning at one admitted
    /// entry. The installed image remains provider-side: callers can bind a
    /// generated stub to its frozen executable bytes without receiving either
    /// the image or an executable address.
    pub fn binds_exact_materialized_entry_bytes(
        &self,
        entry: EntryStubId,
        expected: &[u8],
    ) -> bool {
        if expected.is_empty() {
            return false;
        }
        let frozen = &self.validated.frozen;
        let Some(candidate) = frozen.artifact.artifact.entry(entry) else {
            return false;
        };
        let Ok(start) = usize::try_from(candidate.code_offset) else {
            return false;
        };
        let Some(end) = start.checked_add(expected.len()) else {
            return false;
        };
        frozen.materialized.bytes().get(start..end) == Some(expected)
    }

    /// Test the exact admitted code offset of one selected entry without
    /// exposing a resolved address.
    pub fn binds_entry_offset(&self, entry: EntryStubId, expected_offset: u64) -> bool {
        self.validated
            .frozen
            .artifact
            .artifact
            .entry(entry)
            .is_some_and(|candidate| candidate.code_offset == expected_offset)
    }

    /// Exact placement constraints retained by this installed occurrence.
    /// The projection is descriptive evidence — normalized range, alignment,
    /// phase, regime, and scope identities — and grants no placement or
    /// execute authority.
    pub const fn placement_constraints(&self) -> PlacementConstraints {
        self.validated.frozen.placement.constraints
    }

    /// Test the exact realized placement geometry retained by this installed
    /// occurrence. Callers supply the claimed base and length; the sealed
    /// equality result binds downstream contracts (for example a startup
    /// vector page) without exposing or deriving an executable address.
    pub fn binds_placement_geometry(&self, base: u64, length: u64) -> bool {
        let extent = &self.validated.frozen.placement.extent;
        extent.base() == base && extent.length() == length
    }

    pub fn architecture(&self) -> Architecture {
        self.validated.frozen.artifact.artifact.0.architecture
    }

    pub const fn placement(&self) -> CodePlacementId {
        self.validated.frozen.placement.placement
    }

    pub const fn validation(&self) -> FinalValidationId {
        self.validated.validation
    }

    pub const fn wx(&self) -> WxEnforcement {
        self.wx
    }

    pub fn receipt_context(&self) -> InstalledCodeContext {
        InstalledCodeContext(InstalledCodeEvidence::from_installed(self))
    }

    pub fn occurrence_digest(&self) -> InstalledArtifactOccurrenceDigest {
        installed_artifact_occurrence_digest(&InstalledCodeEvidence::from_installed(self))
    }

    /// Returns a sealed target only for an entry admitted with this installed
    /// artifact. The numeric address stays private to writer execution.
    pub fn selected_entry_target(
        &self,
        identity: EntryStubId,
    ) -> Result<RelocationTarget, InstallationDiagnostic> {
        self.validated
            .frozen
            .artifact
            .selected_entry_target(identity)
    }
}

impl InstallationRegistryAuthority {
    pub const fn installation_scope(&self) -> InstallationScopeId {
        self.installed.validated.scope
    }

    /// Replay the opaque authority against one exact installed realization.
    /// This compares full evidence rather than normalized report identities.
    pub fn matches(&self, installed: &InstalledCode) -> bool {
        self.installed == InstalledCodeEvidence::from_installed(installed)
    }
}

/// One-shot authority to retire one exact installed realization. Required
/// completion facts are open provider vocabulary; quiescence and permission
/// transition remain mandatory lifecycle gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledCodeEvidence {
    pub(crate) installed: InstalledCodeId,
    validated: ValidatedPlacementEvidence,
    wx: WxEnforcement,
}

/// Opaque exact installed-realization context for downstream provider
/// admissions. It exposes no bytes, addresses, or constructors; consumers can
/// retain and compare it without reducing authority to compact report IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCodeContext(pub(crate) InstalledCodeEvidence);

impl InstalledCodeContext {
    pub fn occurrence_digest(&self) -> InstalledArtifactOccurrenceDigest {
        installed_artifact_occurrence_digest(&self.0)
    }
}

impl InstalledCodeEvidence {
    pub(crate) fn from_installed(installed: &InstalledCode) -> Self {
        Self {
            installed: installed.identity,
            validated: ValidatedPlacementEvidence::from_validated(&installed.validated),
            wx: installed.wx,
        }
    }
}

fn installed_artifact_occurrence_digest(
    evidence: &InstalledCodeEvidence,
) -> InstalledArtifactOccurrenceDigest {
    fn bytes(digest: &mut Sha256, value: &[u8]) {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value);
    }

    fn optional_u64(digest: &mut Sha256, value: Option<u64>) {
        match value {
            Some(value) => {
                digest.update([1]);
                digest.update(value.to_le_bytes());
            }
            None => digest.update([0]),
        }
    }

    let validated = &evidence.validated;
    let admitted = &validated.admission_evidence;
    let constraints = validated.constraints;
    let mut digest = Sha256::new();
    digest.update(b"omega.installed-artifact-occurrence.sha256.v1\0");
    digest.update(admitted.artifact.content().digest());
    digest.update(
        admitted
            .artifact
            .identity()
            .normalized_identity()
            .to_le_bytes(),
    );
    digest.update(admitted.admission.normalized_identity().to_le_bytes());
    match &admitted.container_proof {
        Some(proof) => {
            digest.update([1]);
            digest.update(proof.digest.digest());
            bytes(&mut digest, &proof.bytes);
        }
        None => digest.update([0]),
    }
    digest.update(evidence.installed.normalized_identity().to_le_bytes());
    digest.update(validated.placement.normalized_identity().to_le_bytes());
    digest.update(validated.scope.normalized_identity().to_le_bytes());
    digest.update([match validated.audience {
        InstallationAudience::DormantLocal => 1,
        InstallationAudience::FutureFetcher => 2,
    }]);
    match constraints.permitted_range() {
        Some(range) => {
            digest.update([1]);
            digest.update(range.start_inclusive().to_le_bytes());
            digest.update(range.end_exclusive().to_le_bytes());
        }
        None => digest.update([0]),
    }
    digest.update(constraints.alignment().to_le_bytes());
    digest.update([match constraints.phase() {
        layout_plans::PlacementPhase::Build => 1,
        layout_plans::PlacementPhase::Load => 2,
        layout_plans::PlacementPhase::PostHandoff => 3,
    }]);
    optional_u64(
        &mut digest,
        constraints
            .machine_regime()
            .map(|identity| identity.normalized_identity()),
    );
    optional_u64(
        &mut digest,
        constraints
            .installation_scope()
            .map(|identity| identity.normalized_identity()),
    );
    digest.update(validated.base.to_le_bytes());
    digest.update(validated.length.to_le_bytes());
    digest.update(validated.address_space.normalized_identity().to_le_bytes());
    digest.update((validated.rights.identities().count() as u64).to_le_bytes());
    for right in validated.rights.identities() {
        digest.update(right.normalized_identity().to_le_bytes());
    }
    digest.update(validated.provenance.normalized_identity().to_le_bytes());
    digest.update(validated.era.normalized_identity().to_le_bytes());
    digest.update(validated.lineage.normalized_identity().to_le_bytes());
    bytes(&mut digest, &validated.final_bytes);
    digest.update(
        validated
            .realized_footprint
            .normalized_identity()
            .to_le_bytes(),
    );
    digest.update(validated.validation.normalized_identity().to_le_bytes());
    digest.update([match evidence.wx {
        WxEnforcement::HardwareEnforced => 1,
        WxEnforcement::ConventionOnly => 2,
        WxEnforcement::Unsupported => 3,
    }]);
    InstalledArtifactOccurrenceDigest::from_sha256(digest.finalize().into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationDiagnostic(pub String);

impl std::fmt::Display for InstallationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for InstallationDiagnostic {}
