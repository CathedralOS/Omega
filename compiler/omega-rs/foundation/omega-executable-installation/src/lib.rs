//! Normalized executable-artifact admission and installation ladder.
//!
//! No operation converts arbitrary bytes into executable memory. Immutable
//! artifacts are admitted once and reused; each installation instead consumes
//! one exact destination authority through frozen and validated states.

use std::sync::Arc;

use omega_extents::{AddressSpaceId, Extent, ExtentProvenanceId, ExtentRights};
use omega_layout_plans::{
    EntryStubId, MaterializationDiagnostic, PlacementConstraints, PlacementSite,
    PostHandoffWriterPlan, PostHandoffWriterSource, RelocationTarget,
};
use omega_target::Architecture;

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, InstallationDiagnostic> {
                if identity == 0 {
                    return Err(InstallationDiagnostic(format!(
                        "normalized {} identity cannot be zero",
                        $label
                    )));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_id!(ArtifactId, "artifact");
normalized_id!(ArtifactContentId, "artifact-content");
normalized_id!(MachineContractSetId, "machine-contract-set");
normalized_id!(MachineFootprintId, "machine-footprint");
normalized_id!(PlacementPlanId, "placement-plan");
normalized_id!(EntrySetId, "entry-set");
normalized_id!(AdmissionReceiptId, "admission-receipt");
normalized_id!(CodePlacementId, "code-placement");
normalized_id!(InstallationScopeId, "installation-scope");
normalized_id!(FinalBytesId, "final-bytes");
normalized_id!(FinalValidationId, "final-validation");
normalized_id!(InstalledCodeId, "installed-code");
normalized_id!(RetirementFactId, "retirement-fact");
normalized_id!(RelocationSetId, "relocation-set");
normalized_id!(ProofPayloadId, "proof-payload");
normalized_id!(InformationalSectionId, "informational-section");

mod container;
mod materializer;

pub use container::*;
pub use materializer::*;

#[derive(Debug, PartialEq, Eq)]
struct ArtifactRecord {
    identity: ArtifactId,
    content: ArtifactContentId,
    architecture: Architecture,
    byte_length: u64,
    code: Vec<u8>,
    contracts: MachineContractSetId,
    declared_footprint: MachineFootprintId,
    placement_plan: PlacementPlanId,
    placement_constraints: PlacementConstraints,
    entry_set: EntrySetId,
    entries: Vec<ArtifactEntry>,
    relocation_set: RelocationSetId,
    relocations: Vec<DecodedArtifactRelocation>,
}

/// Canonically decoded entry in one executable artifact. The offset remains
/// sealed inside the installation/provider layer; ordinary Omega code sees at
/// most the compiler-issued [`EntryStubId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactEntry {
    identity: EntryStubId,
    code_offset: u64,
}

impl ArtifactEntry {
    pub const fn from_canonical_decode(identity: EntryStubId, code_offset: u64) -> Self {
        Self {
            identity,
            code_offset,
        }
    }

    pub const fn identity(self) -> EntryStubId {
        self.identity
    }

    pub const fn code_offset(self) -> u64 {
        self.code_offset
    }
}

/// Immutable canonical decode result. Construction grants no executable
/// eligibility; it is merely the candidate consumed by admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact(Arc<ArtifactRecord>);

impl Artifact {
    pub fn from_canonical_decode(
        identity: ArtifactId,
        content: ArtifactContentId,
        architecture: Architecture,
        code: Vec<u8>,
        contracts: MachineContractSetId,
        declared_footprint: MachineFootprintId,
        placement_plan: PlacementPlanId,
        placement_constraints: PlacementConstraints,
        entry_set: EntrySetId,
        mut entries: Vec<ArtifactEntry>,
        relocation_set: RelocationSetId,
        relocations: Vec<DecodedArtifactRelocation>,
    ) -> Result<Self, InstallationDiagnostic> {
        if code.is_empty() {
            return Err(InstallationDiagnostic(
                "executable artifact cannot have empty content".into(),
            ));
        }
        let byte_length = u64::try_from(code.len()).map_err(|_| {
            InstallationDiagnostic(
                "executable artifact byte length cannot be represented by the container".into(),
            )
        })?;
        if entries.is_empty() {
            return Err(InstallationDiagnostic(
                "executable artifact must publish at least one selected entry".into(),
            ));
        }
        entries.sort_unstable_by_key(|entry| entry.identity);
        for entry in &entries {
            if entry.code_offset >= byte_length {
                return Err(InstallationDiagnostic(format!(
                    "artifact entry {:?} offset {} lies outside {} code bytes",
                    entry.identity, entry.code_offset, byte_length
                )));
            }
            if matches!(architecture, Architecture::Aarch64) && entry.code_offset % 4 != 0 {
                return Err(InstallationDiagnostic(format!(
                    "AArch64 artifact entry {:?} offset {} is not instruction-aligned",
                    entry.identity, entry.code_offset
                )));
            }
        }
        if entries
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(InstallationDiagnostic(
                "artifact entry identities must be unique".into(),
            ));
        }
        let relocations =
            validate_decoded_relocations(relocations, architecture, byte_length, usize::MAX)?;
        Ok(Self(Arc::new(ArtifactRecord {
            identity,
            content,
            architecture,
            byte_length,
            code,
            contracts,
            declared_footprint,
            placement_plan,
            placement_constraints,
            entry_set,
            entries,
            relocation_set,
            relocations,
        })))
    }

    pub fn identity(&self) -> ArtifactId {
        self.0.identity
    }

    pub fn content(&self) -> ArtifactContentId {
        self.0.content
    }

    pub fn architecture(&self) -> Architecture {
        self.0.architecture
    }

    pub fn byte_length(&self) -> u64 {
        self.0.byte_length
    }

    /// Exact immutable executable bytes bound by this artifact's normalized
    /// content identity. This provider-side projection grants neither
    /// placement nor execute authority.
    pub fn code(&self) -> &[u8] {
        &self.0.code
    }

    pub fn placement_constraints(&self) -> PlacementConstraints {
        self.0.placement_constraints
    }

    pub fn entry_set(&self) -> EntrySetId {
        self.0.entry_set
    }

    pub fn entries(&self) -> &[ArtifactEntry] {
        &self.0.entries
    }

    pub fn relocation_set(&self) -> RelocationSetId {
        self.0.relocation_set
    }

    /// Canonical destination-ordered relocation commitments retained through
    /// admission for the eventual provider materializer. Targets remain sealed
    /// identities; this projection grants no address resolver.
    pub fn relocations(&self) -> &[DecodedArtifactRelocation] {
        &self.0.relocations
    }

    fn entry(&self, identity: EntryStubId) -> Option<ArtifactEntry> {
        self.0
            .entries
            .binary_search_by_key(&identity, |entry| entry.identity)
            .ok()
            .map(|index| self.0.entries[index])
    }
}

/// Validator-authored evidence for the reusable executable qualification.
/// This is the normalized receipt carried by provider admission, not something
/// an Omega package can construct.
#[derive(Debug, PartialEq, Eq)]
pub struct ArtifactAdmissionEvidence {
    receipt: AdmissionReceiptId,
    artifact: Artifact,
    accepted: bool,
}

impl ArtifactAdmissionEvidence {
    pub fn from_validator(
        receipt: AdmissionReceiptId,
        artifact: &Artifact,
        accepted: bool,
    ) -> Self {
        Self {
            receipt,
            artifact: artifact.clone(),
            accepted,
        }
    }
}

/// Reusable immutable artifact carrying the sealed `AdmittedExecutable` fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedArtifact {
    artifact: Artifact,
    admission: AdmissionReceiptId,
}

impl AdmittedArtifact {
    pub const fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    pub const fn admission(&self) -> AdmissionReceiptId {
        self.admission
    }

    /// Selects a sealed materialization target only when the requested entry
    /// belongs to this admitted artifact's canonical, admission-bound entry
    /// set. Numeric address resolution remains deferred until materialization
    /// has bound the artifact to one exact placement.
    pub fn selected_entry_target(
        &self,
        identity: EntryStubId,
    ) -> Result<RelocationTarget, InstallationDiagnostic> {
        self.artifact
            .entry(identity)
            .map(|_| RelocationTarget::Entry(identity))
            .ok_or_else(|| {
                InstallationDiagnostic(format!(
                    "entry {identity:?} is not present in admitted artifact {:?}",
                    self.artifact.0.identity
                ))
            })
    }
}

pub fn admit_executable(
    artifact: &Artifact,
    evidence: ArtifactAdmissionEvidence,
) -> Result<AdmittedArtifact, InstallationDiagnostic> {
    if !evidence.accepted {
        return Err(InstallationDiagnostic(
            "artifact validator did not accept executable eligibility".into(),
        ));
    }
    if evidence.artifact != *artifact {
        return Err(InstallationDiagnostic(
            "artifact admission evidence does not match canonical candidate".into(),
        ));
    }
    Ok(AdmittedArtifact {
        artifact: artifact.clone(),
        admission: evidence.receipt,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationAudience {
    DormantLocal,
    FutureFetcher,
}

/// One-shot authority over an exact W+NX destination.
#[derive(Debug, PartialEq, Eq)]
pub struct CodePlacementAuthority {
    placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    required_rights: ExtentRights,
    constraints: PlacementConstraints,
    site: PlacementSite,
}

impl CodePlacementAuthority {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        placement: CodePlacementId,
        scope: InstallationScopeId,
        audience: InstallationAudience,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        required_rights: ExtentRights,
        constraints: PlacementConstraints,
        site: PlacementSite,
    ) -> Self {
        Self {
            placement,
            scope,
            audience,
            address_space,
            provenance,
            required_rights,
            constraints,
            site,
        }
    }

    pub fn claim(self, extent: Extent) -> Result<CodePlacement, Box<PlacementClaimError>> {
        let mismatch = if extent.address_space() != self.address_space {
            Some("extent address space does not match code-placement authority".into())
        } else if extent.provenance() != self.provenance {
            Some("extent provenance does not match code-placement authority".into())
        } else if !extent.rights().contains(&self.required_rights) {
            Some("extent lacks rights required by code-placement authority".into())
        } else if self.site.base_address != extent.base() {
            Some("placement site base does not match destination Extent".into())
        } else if self
            .site
            .installation_scope
            .is_none_or(|scope| scope.normalized_identity() != self.scope.normalized_identity())
        {
            Some("placement site does not carry the exact installation scope".into())
        } else {
            match usize::try_from(extent.length()) {
                Ok(length) => self
                    .constraints
                    .validate_site(length, self.site)
                    .err()
                    .map(|diagnostic| diagnostic.0),
                Err(_) => {
                    Some("placement length cannot be represented by the host validator".into())
                }
            }
        };
        if let Some(message) = mismatch {
            return Err(Box::new(PlacementClaimError {
                authority: self,
                extent,
                diagnostic: InstallationDiagnostic(message),
            }));
        }
        Ok(CodePlacement {
            placement: self.placement,
            scope: self.scope,
            audience: self.audience,
            constraints: self.constraints,
            extent,
        })
    }
}

#[derive(Debug)]
pub struct PlacementClaimError {
    authority: CodePlacementAuthority,
    extent: Extent,
    diagnostic: InstallationDiagnostic,
}

impl PlacementClaimError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (CodePlacementAuthority, Extent) {
        (self.authority, self.extent)
    }
}

/// Linear W+NX placement state. It carries no executable permission.
#[derive(Debug)]
pub struct CodePlacement {
    placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
    constraints: PlacementConstraints,
    extent: Extent,
}

impl CodePlacement {
    pub const fn identity(&self) -> CodePlacementId {
        self.placement
    }

    pub const fn base(&self) -> u64 {
        self.extent.base()
    }

    pub const fn length(&self) -> u64 {
        self.extent.length()
    }
}

/// Provider result after declared sections/relocations were written and all
/// write authority over the final bytes was frozen.
#[derive(Debug, PartialEq, Eq)]
pub struct MaterializationReceipt {
    materialized: MaterializedArtifactBytes,
    realized_footprint: MachineFootprintId,
    writes_frozen: bool,
}

impl MaterializationReceipt {
    /// Bind a provider's write/freeze receipt to the exact output of the
    /// canonical materializer instead of restating its artifact/placement
    /// identities independently.
    pub fn from_materialized(
        materialized: &MaterializedArtifactBytes,
        realized_footprint: MachineFootprintId,
        writes_frozen: bool,
    ) -> Self {
        Self {
            materialized: materialized.clone(),
            realized_footprint,
            writes_frozen,
        }
    }
}

pub fn materialize_and_freeze(
    artifact: &AdmittedArtifact,
    placement: CodePlacement,
    materialized: MaterializedArtifactBytes,
    receipt: MaterializationReceipt,
) -> Result<FrozenPlacement, Box<MaterializationError>> {
    let mismatch = if materialized.artifact() != artifact.artifact.0.identity
        || materialized.admission() != artifact.admission
    {
        Some("canonical materializer output names a different admitted artifact")
    } else if materialized.placement() != placement.placement {
        Some("canonical materializer output names a different code placement")
    } else if materialized.base_address() != placement.extent.base() {
        Some("canonical materializer output names a different placement base")
    } else if materialized.placement_plan() != artifact.artifact.0.placement_plan {
        Some("canonical materializer output did not use the admitted placement plan")
    } else if materialized.bytes().len() as u64 != artifact.artifact.0.byte_length {
        Some("canonical materializer output has the wrong executable byte length")
    } else if receipt.materialized != materialized {
        Some("materialization receipt does not bind the exact canonical output")
    } else if placement.extent.length() < artifact.artifact.0.byte_length {
        Some("code placement is smaller than the admitted artifact")
    } else if placement.constraints != artifact.artifact.0.placement_constraints {
        Some("code placement constraints do not match the admitted artifact")
    } else if !receipt.writes_frozen {
        Some("materialization did not freeze write authority over final bytes")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(MaterializationError {
            placement,
            materialized,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }
    Ok(FrozenPlacement {
        artifact: artifact.clone(),
        placement,
        materialized,
        realized_footprint: receipt.realized_footprint,
    })
}

#[derive(Debug)]
pub struct MaterializationError {
    placement: CodePlacement,
    materialized: MaterializedArtifactBytes,
    receipt: MaterializationReceipt,
    diagnostic: InstallationDiagnostic,
}

impl MaterializationError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        CodePlacement,
        MaterializedArtifactBytes,
        MaterializationReceipt,
    ) {
        (self.placement, self.materialized, self.receipt)
    }
}

/// Linear R+NX placement whose exact bytes can no longer change.
#[derive(Debug)]
pub struct FrozenPlacement {
    artifact: AdmittedArtifact,
    placement: CodePlacement,
    materialized: MaterializedArtifactBytes,
    realized_footprint: MachineFootprintId,
}

impl FrozenPlacement {
    /// Exact immutable byte snapshot whose write authority the provider froze.
    /// Final footprint/PCC validators inspect this provider-side view; it is
    /// not an Omega source-visible byte-to-code escape hatch.
    pub fn bytes(&self) -> &[u8] {
        self.materialized.bytes()
    }

    pub const fn final_bytes(&self) -> FinalBytesId {
        self.materialized.final_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalValidationCertificate {
    identity: FinalValidationId,
    artifact: Artifact,
    admission: AdmissionReceiptId,
    placement: CodePlacementId,
    final_bytes: Vec<u8>,
    realized_footprint: MachineFootprintId,
    accepted: bool,
}

impl FinalValidationCertificate {
    /// Construct validation evidence from the exact frozen carrier inspected
    /// by the validator. Compact normalized IDs remain useful report keys but
    /// are not accepted as collision-resistant content evidence.
    pub fn from_validator(
        identity: FinalValidationId,
        frozen: &FrozenPlacement,
        accepted: bool,
    ) -> Self {
        Self {
            identity,
            artifact: frozen.artifact.artifact.clone(),
            admission: frozen.artifact.admission,
            placement: frozen.placement.placement,
            final_bytes: frozen.materialized.bytes().to_vec(),
            realized_footprint: frozen.realized_footprint,
            accepted,
        }
    }
}

pub fn validate_final_placement(
    frozen: FrozenPlacement,
    certificate: &FinalValidationCertificate,
) -> Result<ValidatedPlacement, Box<FrozenPlacementError>> {
    let matches = certificate.accepted
        && certificate.artifact == frozen.artifact.artifact
        && certificate.admission == frozen.artifact.admission
        && certificate.placement == frozen.placement.placement
        && certificate.final_bytes == frozen.materialized.bytes()
        && certificate.realized_footprint == frozen.realized_footprint;
    if !matches {
        return Err(Box::new(FrozenPlacementError {
            frozen,
            diagnostic: InstallationDiagnostic(
                "final validation certificate does not match frozen placement".into(),
            ),
        }));
    }
    Ok(ValidatedPlacement {
        frozen,
        validation: certificate.identity,
    })
}

#[derive(Debug)]
pub struct FrozenPlacementError {
    frozen: FrozenPlacement,
    diagnostic: InstallationDiagnostic,
}

impl FrozenPlacementError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_frozen(self) -> FrozenPlacement {
        self.frozen
    }
}

/// Linear R+NX placement bound to checked final bytes and footprint.
#[derive(Debug)]
pub struct ValidatedPlacement {
    frozen: FrozenPlacement,
    validation: FinalValidationId,
}

#[derive(Debug, PartialEq, Eq)]
pub struct InstallAuthority {
    artifact: ArtifactId,
    admission: AdmissionReceiptId,
    placement: CodePlacementId,
    scope: InstallationScopeId,
    audience: InstallationAudience,
}

impl InstallAuthority {
    pub const fn from_admitted_provider(
        artifact: ArtifactId,
        admission: AdmissionReceiptId,
        placement: CodePlacementId,
        scope: InstallationScopeId,
        audience: InstallationAudience,
    ) -> Self {
        Self {
            artifact,
            admission,
            placement,
            scope,
            audience,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WxEnforcement {
    HardwareEnforced,
    ConventionOnly,
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
pub struct InstallationReceipt {
    installed: InstalledCodeId,
    artifact: ArtifactId,
    admission: AdmissionReceiptId,
    placement: CodePlacementId,
    scope: InstallationScopeId,
    validation: FinalValidationId,
    visibility_complete: bool,
    wx: WxEnforcement,
}

impl InstallationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_provider(
        installed: InstalledCodeId,
        artifact: ArtifactId,
        admission: AdmissionReceiptId,
        placement: CodePlacementId,
        scope: InstallationScopeId,
        validation: FinalValidationId,
        visibility_complete: bool,
        wx: WxEnforcement,
    ) -> Self {
        Self {
            installed,
            artifact,
            admission,
            placement,
            scope,
            validation,
            visibility_complete,
            wx,
        }
    }
}

pub fn install_validated(
    validated: ValidatedPlacement,
    authority: InstallAuthority,
    receipt: InstallationReceipt,
) -> Result<InstalledCode, Box<InstallationError>> {
    let frozen = &validated.frozen;
    let artifact = frozen.artifact.artifact.0.identity;
    let admission = frozen.artifact.admission;
    let placement = frozen.placement.placement;
    let scope = frozen.placement.scope;
    let audience = frozen.placement.audience;
    let mismatch = if authority.artifact != artifact
        || authority.admission != admission
        || authority.placement != placement
        || authority.scope != scope
        || authority.audience != audience
    {
        Some("install authority is not scoped to this validated placement")
    } else if receipt.artifact != artifact
        || receipt.admission != admission
        || receipt.placement != placement
        || receipt.scope != scope
        || receipt.validation != validated.validation
    {
        Some("installation receipt does not match validated placement")
    } else if !receipt.visibility_complete {
        Some("installation did not complete instruction-fetch visibility")
    } else if receipt.wx == WxEnforcement::Unsupported {
        Some("provider does not support executable installation")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(InstallationError {
            validated,
            authority,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }

    Ok(InstalledCode {
        identity: receipt.installed,
        artifact: validated.frozen.artifact,
        placement: validated.frozen.placement,
        validation: validated.validation,
        wx: receipt.wx,
    })
}

#[derive(Debug)]
pub struct InstallationError {
    validated: ValidatedPlacement,
    authority: InstallAuthority,
    receipt: InstallationReceipt,
    diagnostic: InstallationDiagnostic,
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
    identity: InstalledCodeId,
    artifact: AdmittedArtifact,
    placement: CodePlacement,
    validation: FinalValidationId,
    wx: WxEnforcement,
}

/// Opaque provider-private words for one checked post-handoff entry writer.
/// Word zero is the exact destination base and the remaining words are the
/// dense, first-occurrence-ordered source slots. The numeric words have no
/// public accessor and this carrier is deliberately non-clonable.
#[derive(PartialEq, Eq)]
pub struct ResolvedPostHandoffEntryWriterContext {
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination_site: PlacementSite,
    destination_len: usize,
    writer: PostHandoffWriterPlan,
    sources: Vec<PostHandoffWriterSource>,
    packed_words: Vec<u64>,
    fingerprint: u64,
}

impl std::fmt::Debug for ResolvedPostHandoffEntryWriterContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedPostHandoffEntryWriterContext")
            .field("installed_code", &self.installed_code)
            .field("artifact", &self.artifact)
            .field("destination_len", &self.destination_len)
            .field("source_slot_count", &self.sources.len())
            .field("fingerprint", &format_args!("{:016x}", self.fingerprint))
            .finish()
    }
}

impl ResolvedPostHandoffEntryWriterContext {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn source_slot_count(&self) -> usize {
        self.sources.len()
    }

    pub const fn packed_byte_len(&self) -> usize {
        self.packed_words.len() * std::mem::size_of::<u64>()
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

impl InstalledCode {
    pub const fn identity(&self) -> InstalledCodeId {
        self.identity
    }

    pub fn artifact(&self) -> ArtifactId {
        self.artifact.artifact.0.identity
    }

    pub const fn placement(&self) -> CodePlacementId {
        self.placement.placement
    }

    pub const fn validation(&self) -> FinalValidationId {
        self.validation
    }

    pub const fn wx(&self) -> WxEnforcement {
        self.wx
    }

    /// Returns a sealed target only for an entry admitted with this installed
    /// artifact. The numeric address stays private to writer execution.
    pub fn selected_entry_target(
        &self,
        identity: EntryStubId,
    ) -> Result<RelocationTarget, InstallationDiagnostic> {
        self.artifact.selected_entry_target(identity)
    }

    /// Check the exact post-handoff entry writer without resolving an entry
    /// address into a public value or changing destination bytes. This is the
    /// provider-side preparation gate used before compiler-generated writer
    /// lowering. Pre-resolved entry fragments must equal the address from this
    /// exact installed realization; unresolved fragments must be members of
    /// this artifact's admitted entry set.
    pub fn validate_post_handoff_entry_writer(
        &self,
        plan: &PostHandoffWriterPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        plan.validate(destination_len, destination_site)?;
        for step in &plan.steps {
            match step.source {
                PostHandoffWriterSource::Resolve(target) => {
                    if !self.contains_entry_target(target) {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff writer target {target:?} is not an admitted entry in the exact installed artifact"
                        )));
                    }
                }
                PostHandoffWriterSource::Resolved(value) => match step.write.target {
                    RelocationTarget::Entry(_)
                        if self.resolve_entry_target(step.write.target) == Some(value) => {}
                    RelocationTarget::Entry(_) => {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff writer pre-resolved entry {:?} does not match the exact installed realization",
                            step.write.target
                        )));
                    }
                    RelocationTarget::Data(_) => {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff entry writer target {:?} is not an admitted entry in the exact installed artifact",
                            step.write.target
                        )));
                    }
                },
            }
        }
        Ok(())
    }

    /// Resolve every distinct source exactly once into an opaque packed
    /// provider context. No numeric code or destination address is returned to
    /// the caller; only the sealed carrier may be passed to checked execution.
    pub fn populate_post_handoff_entry_writer_context(
        &self,
        plan: &PostHandoffWriterPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<ResolvedPostHandoffEntryWriterContext, MaterializationDiagnostic> {
        self.validate_post_handoff_entry_writer(plan, destination_len, destination_site)?;

        let mut sources = Vec::new();
        for step in &plan.steps {
            if !sources.contains(&step.source) {
                sources.push(step.source);
            }
        }
        let mut packed_words = Vec::with_capacity(sources.len() + 1);
        packed_words.push(destination_site.base_address);
        for source in &sources {
            packed_words.push(match source {
                PostHandoffWriterSource::Resolved(value) => *value,
                PostHandoffWriterSource::Resolve(target) => {
                    self.resolve_entry_target(*target).ok_or_else(|| {
                        MaterializationDiagnostic(format!(
                            "post-handoff writer could not populate symbolic target {target:?}"
                        ))
                    })?
                }
            });
        }
        let fingerprint = fingerprint_post_handoff_entry_writer_context(
            self.identity,
            self.artifact(),
            destination_site,
            destination_len,
            &sources,
            &packed_words,
        );
        Ok(ResolvedPostHandoffEntryWriterContext {
            installed_code: self.identity,
            artifact: self.artifact(),
            destination_site,
            destination_len,
            writer: plan.clone(),
            sources,
            packed_words,
            fingerprint,
        })
    }

    /// Execute with the exact once-resolved values sealed into `context`.
    /// Context/plan/site drift rejects before destination mutation.
    pub fn execute_populated_post_handoff_entry_writer(
        &self,
        context: &ResolvedPostHandoffEntryWriterContext,
        plan: &PostHandoffWriterPlan,
        destination: &mut [u8],
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        if context.installed_code != self.identity
            || context.artifact != self.artifact()
            || context.destination_site != destination_site
            || context.destination_len != destination.len()
            || context.writer != *plan
            || context.packed_words.first().copied() != Some(destination_site.base_address)
            || context.packed_words.len() != context.sources.len() + 1
        {
            return Err(MaterializationDiagnostic(
                "populated post-handoff writer context does not bind the exact installed code, plan, destination, and packed geometry"
                    .into(),
            ));
        }
        self.validate_post_handoff_entry_writer(plan, destination.len(), destination_site)?;
        plan.execute(destination, destination_site, |target| {
            context
                .sources
                .iter()
                .zip(&context.packed_words[1..])
                .find_map(|(source, value)| {
                    (*source == PostHandoffWriterSource::Resolve(target)).then_some(*value)
                })
        })
    }

    /// Executes an atomic post-handoff writer using this installed code as the
    /// resolver authority for entry targets. Data symbols and entries from any
    /// other artifact fail before the destination is published.
    pub fn execute_post_handoff_entry_writer(
        &self,
        plan: &PostHandoffWriterPlan,
        destination: &mut [u8],
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        self.validate_post_handoff_entry_writer(plan, destination.len(), destination_site)?;
        plan.execute(destination, destination_site, |target| {
            self.resolve_entry_target(target)
        })
    }

    fn resolve_entry_target(&self, target: RelocationTarget) -> Option<u64> {
        match target {
            RelocationTarget::Entry(identity) => self
                .artifact
                .artifact
                .entry(identity)
                .and_then(|entry| self.placement.extent.base().checked_add(entry.code_offset)),
            RelocationTarget::Data(_) => None,
        }
    }

    fn contains_entry_target(&self, target: RelocationTarget) -> bool {
        match target {
            RelocationTarget::Entry(identity) => self.artifact.artifact.entry(identity).is_some(),
            RelocationTarget::Data(_) => false,
        }
    }
}

fn fingerprint_post_handoff_entry_writer_context(
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination_site: PlacementSite,
    destination_len: usize,
    sources: &[PostHandoffWriterSource],
    packed_words: &[u64],
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(installed_code.normalized_identity());
    mix(artifact.normalized_identity());
    mix(destination_site.base_address);
    mix(destination_len as u64);
    mix(sources.len() as u64);
    for source in sources {
        match source {
            PostHandoffWriterSource::Resolved(_) => mix(1),
            PostHandoffWriterSource::Resolve(RelocationTarget::Entry(entry)) => {
                mix(2);
                mix(entry.normalized_identity());
            }
            PostHandoffWriterSource::Resolve(RelocationTarget::Data(data)) => {
                mix(3);
                mix(data.normalized_identity());
            }
        }
    }
    mix(packed_words.len() as u64);
    for word in packed_words {
        mix(*word);
    }
    if hash == 0 {
        0xcbf2_9ce4_8422_2325
    } else {
        hash
    }
}

/// One-shot authority to retire one exact installed realization. Required
/// completion facts are open provider vocabulary; quiescence and permission
/// transition remain mandatory lifecycle gates.
#[derive(Debug, PartialEq, Eq)]
pub struct RetirementAuthority {
    installed: InstalledCodeId,
    artifact: ArtifactId,
    placement: CodePlacementId,
    scope: InstallationScopeId,
    required_facts: std::collections::BTreeSet<RetirementFactId>,
}

impl RetirementAuthority {
    pub fn from_admitted_provider(
        installed: InstalledCodeId,
        artifact: ArtifactId,
        placement: CodePlacementId,
        scope: InstallationScopeId,
        required_facts: impl IntoIterator<Item = RetirementFactId>,
    ) -> Self {
        Self {
            installed,
            artifact,
            placement,
            scope,
            required_facts: required_facts.into_iter().collect(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RetirementReceipt {
    installed: InstalledCodeId,
    artifact: ArtifactId,
    placement: CodePlacementId,
    scope: InstallationScopeId,
    executors_quiesced: bool,
    execute_disabled: bool,
    write_authority_restored: bool,
    established_facts: std::collections::BTreeSet<RetirementFactId>,
}

impl RetirementReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn from_provider(
        installed: InstalledCodeId,
        artifact: ArtifactId,
        placement: CodePlacementId,
        scope: InstallationScopeId,
        executors_quiesced: bool,
        execute_disabled: bool,
        write_authority_restored: bool,
        established_facts: impl IntoIterator<Item = RetirementFactId>,
    ) -> Self {
        Self {
            installed,
            artifact,
            placement,
            scope,
            executors_quiesced,
            execute_disabled,
            write_authority_restored,
            established_facts: established_facts.into_iter().collect(),
        }
    }
}

/// Result of synchronous retirement. The previous artifact remains reusable;
/// the exact destination returns to the W+NX `CodePlacement` state.
#[derive(Debug)]
pub struct RetiredInstallation {
    previous_artifact: AdmittedArtifact,
    placement: CodePlacement,
}

impl RetiredInstallation {
    pub const fn previous_artifact(&self) -> &AdmittedArtifact {
        &self.previous_artifact
    }

    pub fn into_placement(self) -> CodePlacement {
        self.placement
    }
}

pub fn retire_installed(
    installed: InstalledCode,
    authority: RetirementAuthority,
    receipt: RetirementReceipt,
) -> Result<RetiredInstallation, Box<RetirementError>> {
    let artifact = installed.artifact.artifact.0.identity;
    let placement = installed.placement.placement;
    let scope = installed.placement.scope;
    let mismatch = if authority.installed != installed.identity
        || authority.artifact != artifact
        || authority.placement != placement
        || authority.scope != scope
    {
        Some("retirement authority is not scoped to this installed code")
    } else if receipt.installed != installed.identity
        || receipt.artifact != artifact
        || receipt.placement != placement
        || receipt.scope != scope
    {
        Some("retirement receipt does not match installed code")
    } else if !receipt.executors_quiesced {
        Some("retirement receipt does not establish executor quiescence")
    } else if !receipt.execute_disabled {
        Some("retirement receipt does not establish execute removal")
    } else if !receipt.write_authority_restored {
        Some("retirement receipt does not restore placement write authority")
    } else if !authority
        .required_facts
        .is_subset(&receipt.established_facts)
    {
        Some("retirement receipt lacks required completion facts")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(RetirementError {
            installed,
            authority,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }

    Ok(RetiredInstallation {
        previous_artifact: installed.artifact,
        placement: installed.placement,
    })
}

#[derive(Debug)]
pub struct RetirementError {
    installed: InstalledCode,
    authority: RetirementAuthority,
    receipt: RetirementReceipt,
    diagnostic: InstallationDiagnostic,
}

impl RetirementError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InstalledCode, RetirementAuthority, RetirementReceipt) {
        (self.installed, self.authority, self.receipt)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationDiagnostic(pub String);

impl std::fmt::Display for InstallationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for InstallationDiagnostic {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_extents::{
        ExtentDiagnostic, ExtentLineageId, ExtentRightId, ExtentRootGrant, MappingEraId,
    };
    use omega_layout_plans::{
        ArtifactInstallationScopeId, ByteOrder, MaterializationWrite, PlacementAddressRange,
        PlacementPhase, PostHandoffWriterSource, PostHandoffWriterStep,
    };

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn entry_id(identity: u64) -> EntryStubId {
        EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
    }

    fn rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(
            identities
                .iter()
                .copied()
                .map(|identity| extent_id(identity, ExtentRightId::from_normalized_identity)),
        )
    }

    fn artifact_placement_constraints() -> PlacementConstraints {
        let scope = ArtifactInstallationScopeId::from_normalized_identity(61)
            .expect("artifact installation scope");
        PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
            4096,
            PlacementPhase::PostHandoff,
            None,
            Some(scope),
        )
        .expect("placement constraints")
    }

    fn artifact(identity: u64) -> Artifact {
        artifact_with(
            identity,
            artifact_placement_constraints(),
            id(33, EntrySetId::from_normalized_identity),
            entry_id(identity + 1000),
        )
    }

    fn artifact_with(
        identity: u64,
        constraints: PlacementConstraints,
        entry_set: EntrySetId,
        entry: EntryStubId,
    ) -> Artifact {
        Artifact::from_canonical_decode(
            id(identity, ArtifactId::from_normalized_identity),
            id(identity + 10, ArtifactContentId::from_normalized_identity),
            Architecture::X86_64,
            vec![0; 64],
            id(30, MachineContractSetId::from_normalized_identity),
            id(31, MachineFootprintId::from_normalized_identity),
            id(32, PlacementPlanId::from_normalized_identity),
            constraints,
            entry_set,
            vec![ArtifactEntry::from_canonical_decode(entry, 16)],
            id(34, RelocationSetId::from_normalized_identity),
            Vec::new(),
        )
        .expect("artifact")
    }

    fn admit(candidate: &Artifact) -> AdmittedArtifact {
        admit_executable(
            candidate,
            ArtifactAdmissionEvidence::from_validator(
                id(40, AdmissionReceiptId::from_normalized_identity),
                candidate,
                true,
            ),
        )
        .expect("admitted artifact")
    }

    fn placement_extent(lineage: u64, base: u64, length: u64) -> Extent {
        ExtentRootGrant::from_admitted_provider(
            extent_id(lineage, ExtentLineageId::from_normalized_identity),
            extent_id(50, AddressSpaceId::from_normalized_identity),
            rights(&[51]),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            extent_id(53, MappingEraId::from_normalized_identity),
        )
        .mint(base, length)
        .expect("placement extent")
    }

    fn placement_authority(placement: u64, base: u64) -> CodePlacementAuthority {
        placement_authority_with_constraints(placement, base, artifact_placement_constraints())
    }

    fn placement_authority_with_constraints(
        placement: u64,
        base: u64,
        constraints: PlacementConstraints,
    ) -> CodePlacementAuthority {
        let scope = ArtifactInstallationScopeId::from_normalized_identity(61)
            .expect("artifact installation scope");
        CodePlacementAuthority::from_admitted_provider(
            id(placement, CodePlacementId::from_normalized_identity),
            id(61, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
            extent_id(50, AddressSpaceId::from_normalized_identity),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            rights(&[51]),
            constraints,
            PlacementSite {
                base_address: base,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: Some(scope),
            },
        )
    }

    fn relocatable_artifact(
        identity: u64,
        architecture: Architecture,
        code: Vec<u8>,
        relocations: Vec<DecodedArtifactRelocation>,
    ) -> Artifact {
        Artifact::from_canonical_decode(
            id(identity, ArtifactId::from_normalized_identity),
            id(identity + 10, ArtifactContentId::from_normalized_identity),
            architecture,
            code,
            id(30, MachineContractSetId::from_normalized_identity),
            id(31, MachineFootprintId::from_normalized_identity),
            id(32, PlacementPlanId::from_normalized_identity),
            artifact_placement_constraints(),
            id(33, EntrySetId::from_normalized_identity),
            vec![ArtifactEntry::from_canonical_decode(
                entry_id(identity + 1000),
                0,
            )],
            id(34, RelocationSetId::from_normalized_identity),
            relocations,
        )
        .expect("relocatable artifact")
    }

    fn frozen(admitted: &AdmittedArtifact, placement_identity: u64, base: u64) -> FrozenPlacement {
        let placement = placement_authority(placement_identity, base)
            .claim(placement_extent(placement_identity, base, 4096))
            .expect("placement");
        let materialized = materialize_admitted_artifact(admitted, &placement, |_| None)
            .expect("artifact without relocations materializes");
        materialize_and_freeze(
            admitted,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("frozen placement")
    }

    fn certificate(frozen: &FrozenPlacement, identity: u64) -> FinalValidationCertificate {
        FinalValidationCertificate::from_validator(
            id(identity, FinalValidationId::from_normalized_identity),
            frozen,
            true,
        )
    }

    fn installed_code(admitted: &AdmittedArtifact, placement: u64, base: u64) -> InstalledCode {
        let frozen = frozen(admitted, placement, base);
        let certificate = certificate(&frozen, 80 + placement);
        let validated =
            validate_final_placement(frozen, &certificate).expect("validated placement");
        install_validated(
            validated,
            InstallAuthority::from_admitted_provider(
                admitted.artifact().identity(),
                admitted.admission(),
                id(placement, CodePlacementId::from_normalized_identity),
                id(61, InstallationScopeId::from_normalized_identity),
                InstallationAudience::FutureFetcher,
            ),
            InstallationReceipt::from_provider(
                id(200 + placement, InstalledCodeId::from_normalized_identity),
                admitted.artifact().identity(),
                admitted.admission(),
                id(placement, CodePlacementId::from_normalized_identity),
                id(61, InstallationScopeId::from_normalized_identity),
                id(80 + placement, FinalValidationId::from_normalized_identity),
                true,
                WxEnforcement::HardwareEnforced,
            ),
        )
        .expect("installed code")
    }

    #[test]
    fn canonical_materializer_patches_x86_relative_targets_and_binds_the_receipt() {
        let target = RelocationTarget::Entry(entry_id(1001));
        let mut code = vec![0x90; 64];
        code[0] = 0xe8;
        code[1..5].fill(0);
        let candidate = relocatable_artifact(
            1,
            Architecture::X86_64,
            code,
            vec![DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::X86Relative32,
                destination_offset: 1,
                target,
                addend: -4,
            }],
        );
        let admitted = admit(&candidate);
        let placement = placement_authority(100, 0x1000)
            .claim(placement_extent(100, 0x1000, 4096))
            .expect("placement");

        let materialized = materialize_admitted_artifact(&admitted, &placement, |candidate| {
            (candidate == target).then_some(0x1024)
        })
        .expect("materialized bytes");
        assert_eq!(
            i32::from_le_bytes(materialized.bytes()[1..5].try_into().unwrap()),
            0x1b
        );

        let frozen = materialize_and_freeze(
            &admitted,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                id(31, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("receipt is bound to canonical materializer output");
        assert_eq!(frozen.bytes(), materialized.bytes());
        assert_eq!(frozen.final_bytes(), materialized.final_bytes());
    }

    #[test]
    fn aarch64_materialization_validates_the_relocated_instruction_shape() {
        let target = RelocationTarget::Entry(entry_id(1002));
        let mut branch = vec![0; 64];
        branch[..4].copy_from_slice(&0x9400_0000u32.to_le_bytes());
        let candidate = relocatable_artifact(
            2,
            Architecture::Aarch64,
            branch,
            vec![DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Aarch64Branch26,
                destination_offset: 0,
                target,
                addend: 0,
            }],
        );
        let admitted = admit(&candidate);
        let placement = placement_authority(101, 0x1000)
            .claim(placement_extent(101, 0x1000, 4096))
            .expect("placement");
        let materialized = materialize_admitted_artifact(&admitted, &placement, |_| Some(0x1010))
            .expect("AArch64 branch materialization");
        assert_eq!(
            u32::from_le_bytes(materialized.bytes()[..4].try_into().unwrap()),
            0x9400_0004
        );

        let invalid = relocatable_artifact(
            3,
            Architecture::Aarch64,
            vec![0; 64],
            vec![DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Aarch64Branch26,
                destination_offset: 0,
                target,
                addend: 0,
            }],
        );
        let invalid = admit(&invalid);
        let error = materialize_admitted_artifact(&invalid, &placement, |_| Some(0x1010))
            .expect_err("relocation cannot rewrite an arbitrary instruction");
        assert!(error.0.contains("B/BL"));
    }

    #[test]
    fn aarch64_materialization_patches_page_pairs_and_absolute_data() {
        let target = RelocationTarget::Entry(entry_id(1004));
        let mut code = vec![0; 64];
        code[..4].copy_from_slice(&0x9000_0000u32.to_le_bytes());
        code[4..8].copy_from_slice(&0x9100_0000u32.to_le_bytes());
        let candidate = relocatable_artifact(
            4,
            Architecture::Aarch64,
            code,
            vec![
                DecodedArtifactRelocation {
                    kind: ArtifactRelocationKind::Aarch64Page21,
                    destination_offset: 0,
                    target,
                    addend: 0,
                },
                DecodedArtifactRelocation {
                    kind: ArtifactRelocationKind::Aarch64PageOffset12,
                    destination_offset: 4,
                    target,
                    addend: 0,
                },
                DecodedArtifactRelocation {
                    kind: ArtifactRelocationKind::Absolute64,
                    destination_offset: 8,
                    target,
                    addend: -6,
                },
            ],
        );
        let admitted = admit(&candidate);
        let placement = placement_authority(102, 0x1000)
            .claim(placement_extent(102, 0x1000, 4096))
            .expect("placement");
        let materialized = materialize_admitted_artifact(&admitted, &placement, |_| Some(0x3456))
            .expect("AArch64 page-pair materialization");

        assert_eq!(
            u32::from_le_bytes(materialized.bytes()[..4].try_into().unwrap()),
            0xd000_0000
        );
        assert_eq!(
            u32::from_le_bytes(materialized.bytes()[4..8].try_into().unwrap()),
            0x9111_5800
        );
        assert_eq!(
            u64::from_le_bytes(materialized.bytes()[8..16].try_into().unwrap()),
            0x3450
        );
    }

    #[test]
    fn admitted_artifact_is_reusable_but_each_placement_is_linear() {
        let candidate = artifact(1);
        let admitted = admit(&candidate);
        let second_reference = admitted.clone();

        let frozen = frozen(&admitted, 100, 0x1000);
        let certificate = certificate(&frozen, 180);
        let validated =
            validate_final_placement(frozen, &certificate).expect("validated placement");
        let authority = InstallAuthority::from_admitted_provider(
            admitted.artifact().identity(),
            admitted.admission(),
            id(100, CodePlacementId::from_normalized_identity),
            id(61, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
        );
        let installed = install_validated(
            validated,
            authority,
            InstallationReceipt::from_provider(
                id(200, InstalledCodeId::from_normalized_identity),
                admitted.artifact().identity(),
                admitted.admission(),
                id(100, CodePlacementId::from_normalized_identity),
                id(61, InstallationScopeId::from_normalized_identity),
                id(180, FinalValidationId::from_normalized_identity),
                true,
                WxEnforcement::HardwareEnforced,
            ),
        )
        .expect("installed code");
        assert_eq!(installed.artifact(), second_reference.artifact().identity());
        assert_eq!(installed.wx(), WxEnforcement::HardwareEnforced);
    }

    #[test]
    fn materialization_cannot_substitute_another_artifact() {
        let first = admit(&artifact(1));
        let second = admit(&artifact(2));
        let placement = placement_authority(101, 0x2000)
            .claim(placement_extent(101, 0x2000, 4096))
            .expect("placement");
        let first_materialized = materialize_admitted_artifact(&first, &placement, |_| None)
            .expect("first artifact materialization");
        let second_materialized = materialize_admitted_artifact(&second, &placement, |_| None)
            .expect("second artifact materialization");
        let error = materialize_and_freeze(
            &first,
            placement,
            first_materialized,
            MaterializationReceipt::from_materialized(
                &second_materialized,
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect_err("artifact substitution rejects");
        assert!(error.diagnostic().0.contains("exact canonical output"));
        let (_placement, _materialized, _receipt) = (*error).into_parts();
    }

    #[test]
    fn canonical_materializer_output_cannot_substitute_another_artifact() {
        let first = admit(&artifact(1));
        let second = admit(&artifact(2));
        let placement = placement_authority(111, 0x9000)
            .claim(placement_extent(111, 0x9000, 4096))
            .expect("placement");
        let materialized = materialize_admitted_artifact(&second, &placement, |_| None)
            .expect("second artifact materialization");
        let receipt = MaterializationReceipt::from_materialized(
            &materialized,
            id(71, MachineFootprintId::from_normalized_identity),
            true,
        );
        let error = materialize_and_freeze(&first, placement, materialized, receipt)
            .expect_err("canonical output substitution rejects");
        assert!(
            error
                .diagnostic()
                .0
                .contains("materializer output names a different admitted artifact")
        );
        let (_placement, _materialized, _receipt) = (*error).into_parts();
    }

    #[test]
    fn admission_evidence_cannot_substitute_placement_constraints() {
        let candidate = artifact(1);
        let weaker = PlacementConstraints::unconstrained(PlacementPhase::PostHandoff);
        let substituted = artifact_with(
            1,
            weaker,
            candidate.0.entry_set,
            candidate.0.entries[0].identity,
        );
        let error = admit_executable(
            &candidate,
            ArtifactAdmissionEvidence::from_validator(
                id(40, AdmissionReceiptId::from_normalized_identity),
                &substituted,
                true,
            ),
        )
        .expect_err("admission evidence must pin the decoded placement constraints");
        assert!(error.0.contains("does not match canonical candidate"));
    }

    #[test]
    fn admission_evidence_cannot_substitute_the_selected_entry_set() {
        let candidate = artifact(1);
        let substituted = artifact_with(
            1,
            candidate.0.placement_constraints,
            id(34, EntrySetId::from_normalized_identity),
            candidate.0.entries[0].identity,
        );
        let error = admit_executable(
            &candidate,
            ArtifactAdmissionEvidence::from_validator(
                id(40, AdmissionReceiptId::from_normalized_identity),
                &substituted,
                true,
            ),
        )
        .expect_err("admission evidence must pin the decoded entry set");
        assert!(error.0.contains("does not match canonical candidate"));
    }

    #[test]
    fn admitted_artifact_selects_only_its_canonical_entry_targets() {
        let candidate = artifact(1);
        let selected = entry_id(1001);
        let admitted = admit(&candidate);
        assert_eq!(
            admitted
                .selected_entry_target(selected)
                .expect("selected entry target"),
            RelocationTarget::Entry(selected)
        );
        let foreign = entry_id(1002);
        assert!(admitted.selected_entry_target(foreign).is_err());
    }

    #[test]
    fn installed_code_resolves_only_its_entries_for_atomic_post_handoff_writers() {
        let admitted = admit(&artifact(1));
        let installed = installed_code(&admitted, 110, 0x8000);
        let selected = entry_id(1001);
        let target = installed
            .selected_entry_target(selected)
            .expect("installed selected entry");
        let writer = |target| PostHandoffWriterPlan {
            byte_len: 8,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "address".into(),
                    target,
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                },
                source: PostHandoffWriterSource::Resolve(target),
            }],
        };
        let destination_site = PlacementSite {
            base_address: 0x9000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        };

        let mut destination = [0u8; 8];
        installed
            .execute_post_handoff_entry_writer(&writer(target), &mut destination, destination_site)
            .expect("installed entry writer");
        assert_eq!(u64::from_le_bytes(destination), 0x8010);

        let mut checked_writer = writer(target);
        let mut low_half = checked_writer.steps[0].clone();
        low_half.write.width = 32;
        let mut high_half = low_half.clone();
        high_half.write.destination_lsb = 32;
        high_half.write.source_lsb = 32;
        checked_writer.steps = vec![low_half, high_half];
        let context = installed
            .populate_post_handoff_entry_writer_context(
                &checked_writer,
                destination.len(),
                destination_site,
            )
            .expect("installed resolver populates opaque writer context");
        assert_eq!(context.installed_code(), installed.identity());
        assert_eq!(context.artifact(), installed.artifact());
        assert_eq!(context.source_slot_count(), 1);
        assert_eq!(context.packed_byte_len(), 16);
        assert_ne!(context.fingerprint(), 0);
        let context_debug = format!("{context:?}");
        assert!(!context_debug.contains("packed_words"));
        assert!(!context_debug.contains("destination_site"));
        let mut populated_destination = [0u8; 8];
        installed
            .execute_populated_post_handoff_entry_writer(
                &context,
                &checked_writer,
                &mut populated_destination,
                destination_site,
            )
            .expect("populated context executes without public address resolution");
        assert_eq!(u64::from_le_bytes(populated_destination), 0x8010);
        let mut unchanged_context_destination = [0xa5; 8];
        let error = installed
            .execute_populated_post_handoff_entry_writer(
                &context,
                &checked_writer,
                &mut unchanged_context_destination,
                PlacementSite {
                    base_address: destination_site.base_address + 8,
                    ..destination_site
                },
            )
            .expect_err("destination-site drift must reject the populated context");
        assert!(error.0.contains("exact installed code, plan, destination"));
        assert_eq!(unchanged_context_destination, [0xa5; 8]);

        let foreign = RelocationTarget::Entry(entry_id(1002));
        let mut unchanged = [0xa5u8; 8];
        let error = installed
            .execute_post_handoff_entry_writer(&writer(foreign), &mut unchanged, destination_site)
            .expect_err("foreign artifact entry must not resolve");
        assert!(error.0.contains("exact installed artifact"));
        assert_eq!(unchanged, [0xa5; 8]);

        let mut stale = writer(target);
        stale.steps[0].source = PostHandoffWriterSource::Resolved(0xdead_beef);
        let error = installed
            .execute_post_handoff_entry_writer(&stale, &mut unchanged, destination_site)
            .expect_err("pre-resolved address from another realization must reject");
        assert!(error.0.contains("exact installed realization"));
        assert_eq!(unchanged, [0xa5; 8]);
    }

    #[test]
    fn final_certificate_is_bound_to_one_placement_and_final_bytes() {
        let admitted = admit(&artifact(1));
        let target = frozen(&admitted, 102, 0x3000);
        let foreign = frozen(&admitted, 103, 0x3000);
        let certificate = certificate(&foreign, 183);
        let error = validate_final_placement(target, &certificate)
            .expect_err("certificate transplant rejects");
        assert!(error.diagnostic().0.contains("does not match"));
    }

    #[test]
    fn final_certificate_retains_the_exact_frozen_byte_snapshot() {
        let admitted = admit(&artifact(1));
        let frozen = frozen(&admitted, 113, 0xb000);
        let mut certificate = certificate(&frozen, 193);
        certificate.final_bytes[0] ^= 1;

        let error = validate_final_placement(frozen, &certificate)
            .expect_err("a certificate for substituted exact bytes must reject");
        assert!(error.diagnostic().0.contains("does not match"));
    }

    #[test]
    fn unsupported_execute_transition_preserves_all_linear_inputs() {
        let admitted = admit(&artifact(1));
        let frozen = frozen(&admitted, 104, 0x4000);
        let certificate = certificate(&frozen, 184);
        let validated =
            validate_final_placement(frozen, &certificate).expect("validated placement");
        let authority = InstallAuthority::from_admitted_provider(
            admitted.artifact().identity(),
            admitted.admission(),
            id(104, CodePlacementId::from_normalized_identity),
            id(61, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
        );
        let error = install_validated(
            validated,
            authority,
            InstallationReceipt::from_provider(
                id(204, InstalledCodeId::from_normalized_identity),
                admitted.artifact().identity(),
                admitted.admission(),
                id(104, CodePlacementId::from_normalized_identity),
                id(61, InstallationScopeId::from_normalized_identity),
                id(184, FinalValidationId::from_normalized_identity),
                true,
                WxEnforcement::Unsupported,
            ),
        )
        .expect_err("unsupported provider rejects");
        assert!(error.diagnostic().0.contains("does not support"));
        let (_validated, _authority, _receipt) = (*error).into_parts();
    }

    #[test]
    fn materialization_uses_admitted_artifact_size_not_a_caller_hint() {
        let admitted = admit(&artifact(1));
        let placement = placement_authority(105, 0x5000)
            .claim(placement_extent(105, 0x5000, 32))
            .expect("qualified but undersized destination");
        let error = materialize_admitted_artifact(&admitted, &placement, |_| None)
            .expect_err("artifact cannot fit");
        assert!(error.0.contains("smaller"));
    }

    #[test]
    fn materialization_rejects_placement_constraint_substitution() {
        let admitted = admit(&artifact(1));
        let substituted =
            PlacementConstraints::new(None, 1, PlacementPhase::PostHandoff, None, None)
                .expect("weaker substituted constraints");
        let placement = placement_authority_with_constraints(109, 0x8000, substituted)
            .claim(placement_extent(109, 0x8000, 4096))
            .expect("substituted constraints independently accept the site");
        let error = materialize_admitted_artifact(&admitted, &placement, |_| None)
            .expect_err("provider cannot substitute weaker placement constraints");
        assert!(error.0.contains("constraints do not match"));
    }

    #[test]
    fn failed_placement_claim_returns_extent_and_one_shot_authority() {
        let extent = ExtentRootGrant::from_admitted_provider(
            extent_id(106, ExtentLineageId::from_normalized_identity),
            extent_id(50, AddressSpaceId::from_normalized_identity),
            ExtentRights::none(),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            extent_id(53, MappingEraId::from_normalized_identity),
        )
        .mint(0x6000, 4096)
        .expect("placement extent without required rights");
        let error = placement_authority(106, 0x6000)
            .claim(extent)
            .expect_err("missing placement right");
        assert!(error.diagnostic().0.contains("lacks rights"));
        let (_authority, extent) = (*error).into_parts();
        assert_eq!(extent.base(), 0x6000);
    }

    #[test]
    fn retirement_requires_quiescence_then_returns_writable_placement() {
        let admitted = admit(&artifact(1));
        let installed = installed_code(&admitted, 107, 0x7000);
        let retirement_fact = id(300, RetirementFactId::from_normalized_identity);
        let authority = RetirementAuthority::from_admitted_provider(
            installed.identity(),
            installed.artifact(),
            installed.placement(),
            id(61, InstallationScopeId::from_normalized_identity),
            [retirement_fact],
        );
        let error = retire_installed(
            installed,
            authority,
            RetirementReceipt::from_provider(
                id(307, InstalledCodeId::from_normalized_identity),
                admitted.artifact().identity(),
                id(107, CodePlacementId::from_normalized_identity),
                id(61, InstallationScopeId::from_normalized_identity),
                false,
                true,
                true,
                [retirement_fact],
            ),
        )
        .expect_err("visibility is not quiescence");
        assert!(error.diagnostic().0.contains("quiescence"));
        let (installed, authority, _) = (*error).into_parts();

        let retired = retire_installed(
            installed,
            authority,
            RetirementReceipt::from_provider(
                id(307, InstalledCodeId::from_normalized_identity),
                admitted.artifact().identity(),
                id(107, CodePlacementId::from_normalized_identity),
                id(61, InstallationScopeId::from_normalized_identity),
                true,
                true,
                true,
                [retirement_fact],
            ),
        )
        .expect("retired code");
        assert_eq!(
            retired.previous_artifact().artifact().identity(),
            admitted.artifact().identity()
        );

        let replacement = admit(&artifact(2));
        let placement = retired.into_placement();
        let materialized = materialize_admitted_artifact(&replacement, &placement, |_| None)
            .expect("replacement materialization");
        materialize_and_freeze(
            &replacement,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("placement reusable only after quiescent retirement");
    }

    #[test]
    fn placement_claim_validates_actual_extent_site_against_plan_constraints() {
        let error = placement_authority(108, 0x7101)
            .claim(placement_extent(108, 0x7101, 4096))
            .expect_err("misaligned site rejects before materialization");
        assert!(error.diagnostic().0.contains("not aligned"));
        let (_authority, extent) = (*error).into_parts();
        assert_eq!(extent.base(), 0x7101);
    }
}
