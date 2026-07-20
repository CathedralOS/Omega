//! Normalized executable-artifact admission and installation ladder.
//!
//! No operation converts arbitrary bytes into executable memory. Immutable
//! artifacts are admitted once and reused; each installation instead consumes
//! one exact destination authority through frozen and validated states.

use std::sync::Arc;

use omega_extents::{AddressSpaceId, Extent, ExtentProvenanceId, ExtentRights};

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

pub use container::*;

#[derive(Debug, PartialEq, Eq)]
struct ArtifactRecord {
    identity: ArtifactId,
    content: ArtifactContentId,
    byte_length: u64,
    contracts: MachineContractSetId,
    declared_footprint: MachineFootprintId,
    placement_plan: PlacementPlanId,
}

/// Immutable canonical decode result. Construction grants no executable
/// eligibility; it is merely the candidate consumed by admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact(Arc<ArtifactRecord>);

impl Artifact {
    pub fn from_canonical_decode(
        identity: ArtifactId,
        content: ArtifactContentId,
        byte_length: u64,
        contracts: MachineContractSetId,
        declared_footprint: MachineFootprintId,
        placement_plan: PlacementPlanId,
    ) -> Result<Self, InstallationDiagnostic> {
        if byte_length == 0 {
            return Err(InstallationDiagnostic(
                "executable artifact cannot have empty content".into(),
            ));
        }
        Ok(Self(Arc::new(ArtifactRecord {
            identity,
            content,
            byte_length,
            contracts,
            declared_footprint,
            placement_plan,
        })))
    }

    pub fn identity(&self) -> ArtifactId {
        self.0.identity
    }

    pub fn content(&self) -> ArtifactContentId {
        self.0.content
    }

    pub fn byte_length(&self) -> u64 {
        self.0.byte_length
    }
}

/// Validator-authored evidence for the reusable executable qualification.
/// This is the normalized receipt carried by provider admission, not something
/// an Omega package can construct.
#[derive(Debug, PartialEq, Eq)]
pub struct ArtifactAdmissionEvidence {
    receipt: AdmissionReceiptId,
    artifact: ArtifactId,
    content: ArtifactContentId,
    contracts: MachineContractSetId,
    footprint: MachineFootprintId,
    placement_plan: PlacementPlanId,
    accepted: bool,
}

impl ArtifactAdmissionEvidence {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_validator(
        receipt: AdmissionReceiptId,
        artifact: ArtifactId,
        content: ArtifactContentId,
        contracts: MachineContractSetId,
        footprint: MachineFootprintId,
        placement_plan: PlacementPlanId,
        accepted: bool,
    ) -> Self {
        Self {
            receipt,
            artifact,
            content,
            contracts,
            footprint,
            placement_plan,
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
    if evidence.artifact != artifact.0.identity
        || evidence.content != artifact.0.content
        || evidence.contracts != artifact.0.contracts
        || evidence.footprint != artifact.0.declared_footprint
        || evidence.placement_plan != artifact.0.placement_plan
    {
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
}

impl CodePlacementAuthority {
    pub fn from_admitted_provider(
        placement: CodePlacementId,
        scope: InstallationScopeId,
        audience: InstallationAudience,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        required_rights: ExtentRights,
    ) -> Self {
        Self {
            placement,
            scope,
            audience,
            address_space,
            provenance,
            required_rights,
        }
    }

    pub fn claim(self, extent: Extent) -> Result<CodePlacement, Box<PlacementClaimError>> {
        let mismatch = if extent.address_space() != self.address_space {
            Some("extent address space does not match code-placement authority")
        } else if extent.provenance() != self.provenance {
            Some("extent provenance does not match code-placement authority")
        } else if !extent.rights().contains(&self.required_rights) {
            Some("extent lacks rights required by code-placement authority")
        } else {
            None
        };
        if let Some(message) = mismatch {
            return Err(Box::new(PlacementClaimError {
                authority: self,
                extent,
                diagnostic: InstallationDiagnostic(message.into()),
            }));
        }
        Ok(CodePlacement {
            placement: self.placement,
            scope: self.scope,
            audience: self.audience,
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
    artifact: ArtifactId,
    admission: AdmissionReceiptId,
    placement: CodePlacementId,
    placement_plan: PlacementPlanId,
    final_bytes: FinalBytesId,
    realized_footprint: MachineFootprintId,
    writes_frozen: bool,
}

impl MaterializationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_provider(
        artifact: ArtifactId,
        admission: AdmissionReceiptId,
        placement: CodePlacementId,
        placement_plan: PlacementPlanId,
        final_bytes: FinalBytesId,
        realized_footprint: MachineFootprintId,
        writes_frozen: bool,
    ) -> Self {
        Self {
            artifact,
            admission,
            placement,
            placement_plan,
            final_bytes,
            realized_footprint,
            writes_frozen,
        }
    }
}

pub fn materialize_and_freeze(
    artifact: &AdmittedArtifact,
    placement: CodePlacement,
    receipt: MaterializationReceipt,
) -> Result<FrozenPlacement, Box<MaterializationError>> {
    let mismatch = if receipt.artifact != artifact.artifact.0.identity
        || receipt.admission != artifact.admission
    {
        Some("materialization receipt names a different admitted artifact")
    } else if receipt.placement != placement.placement {
        Some("materialization receipt names a different code placement")
    } else if placement.extent.length() < artifact.artifact.0.byte_length {
        Some("code placement is smaller than the admitted artifact")
    } else if receipt.placement_plan != artifact.artifact.0.placement_plan {
        Some("materialization did not use the admitted placement plan")
    } else if !receipt.writes_frozen {
        Some("materialization did not freeze write authority over final bytes")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(MaterializationError {
            placement,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }
    Ok(FrozenPlacement {
        artifact: artifact.clone(),
        placement,
        final_bytes: receipt.final_bytes,
        realized_footprint: receipt.realized_footprint,
    })
}

#[derive(Debug)]
pub struct MaterializationError {
    placement: CodePlacement,
    receipt: MaterializationReceipt,
    diagnostic: InstallationDiagnostic,
}

impl MaterializationError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (CodePlacement, MaterializationReceipt) {
        (self.placement, self.receipt)
    }
}

/// Linear R+NX placement whose exact bytes can no longer change.
#[derive(Debug)]
pub struct FrozenPlacement {
    artifact: AdmittedArtifact,
    placement: CodePlacement,
    final_bytes: FinalBytesId,
    realized_footprint: MachineFootprintId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalValidationCertificate {
    identity: FinalValidationId,
    artifact: ArtifactId,
    admission: AdmissionReceiptId,
    placement: CodePlacementId,
    final_bytes: FinalBytesId,
    realized_footprint: MachineFootprintId,
    accepted: bool,
}

impl FinalValidationCertificate {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_validator(
        identity: FinalValidationId,
        artifact: ArtifactId,
        admission: AdmissionReceiptId,
        placement: CodePlacementId,
        final_bytes: FinalBytesId,
        realized_footprint: MachineFootprintId,
        accepted: bool,
    ) -> Self {
        Self {
            identity,
            artifact,
            admission,
            placement,
            final_bytes,
            realized_footprint,
            accepted,
        }
    }
}

pub fn validate_final_placement(
    frozen: FrozenPlacement,
    certificate: &FinalValidationCertificate,
) -> Result<ValidatedPlacement, Box<FrozenPlacementError>> {
    let matches = certificate.accepted
        && certificate.artifact == frozen.artifact.artifact.0.identity
        && certificate.admission == frozen.artifact.admission
        && certificate.placement == frozen.placement.placement
        && certificate.final_bytes == frozen.final_bytes
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

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(
            identities
                .iter()
                .copied()
                .map(|identity| extent_id(identity, ExtentRightId::from_normalized_identity)),
        )
    }

    fn artifact(identity: u64) -> Artifact {
        Artifact::from_canonical_decode(
            id(identity, ArtifactId::from_normalized_identity),
            id(identity + 10, ArtifactContentId::from_normalized_identity),
            64,
            id(30, MachineContractSetId::from_normalized_identity),
            id(31, MachineFootprintId::from_normalized_identity),
            id(32, PlacementPlanId::from_normalized_identity),
        )
        .expect("artifact")
    }

    fn admit(candidate: &Artifact) -> AdmittedArtifact {
        admit_executable(
            candidate,
            ArtifactAdmissionEvidence::from_validator(
                id(40, AdmissionReceiptId::from_normalized_identity),
                candidate.0.identity,
                candidate.0.content,
                candidate.0.contracts,
                candidate.0.declared_footprint,
                candidate.0.placement_plan,
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

    fn placement_authority(placement: u64) -> CodePlacementAuthority {
        CodePlacementAuthority::from_admitted_provider(
            id(placement, CodePlacementId::from_normalized_identity),
            id(61, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
            extent_id(50, AddressSpaceId::from_normalized_identity),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            rights(&[51]),
        )
    }

    fn frozen(admitted: &AdmittedArtifact, placement_identity: u64, base: u64) -> FrozenPlacement {
        let placement = placement_authority(placement_identity)
            .claim(placement_extent(placement_identity, base, 4096))
            .expect("placement");
        materialize_and_freeze(
            admitted,
            placement,
            MaterializationReceipt::from_provider(
                admitted.artifact().identity(),
                admitted.admission(),
                id(
                    placement_identity,
                    CodePlacementId::from_normalized_identity,
                ),
                id(32, PlacementPlanId::from_normalized_identity),
                id(
                    70 + placement_identity,
                    FinalBytesId::from_normalized_identity,
                ),
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("frozen placement")
    }

    fn certificate(admitted: &AdmittedArtifact, placement: u64) -> FinalValidationCertificate {
        FinalValidationCertificate::from_validator(
            id(80 + placement, FinalValidationId::from_normalized_identity),
            admitted.artifact().identity(),
            admitted.admission(),
            id(placement, CodePlacementId::from_normalized_identity),
            id(70 + placement, FinalBytesId::from_normalized_identity),
            id(71, MachineFootprintId::from_normalized_identity),
            true,
        )
    }

    fn installed_code(admitted: &AdmittedArtifact, placement: u64, base: u64) -> InstalledCode {
        let validated = validate_final_placement(
            frozen(admitted, placement, base),
            &certificate(admitted, placement),
        )
        .expect("validated placement");
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
    fn admitted_artifact_is_reusable_but_each_placement_is_linear() {
        let candidate = artifact(1);
        let admitted = admit(&candidate);
        let second_reference = admitted.clone();

        let validated =
            validate_final_placement(frozen(&admitted, 100, 0x1000), &certificate(&admitted, 100))
                .expect("validated placement");
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
        let placement = placement_authority(101)
            .claim(placement_extent(101, 0x2000, 4096))
            .expect("placement");
        let error = materialize_and_freeze(
            &first,
            placement,
            MaterializationReceipt::from_provider(
                second.artifact().identity(),
                second.admission(),
                id(101, CodePlacementId::from_normalized_identity),
                id(32, PlacementPlanId::from_normalized_identity),
                id(171, FinalBytesId::from_normalized_identity),
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect_err("artifact substitution rejects");
        assert!(error.diagnostic().0.contains("different admitted artifact"));
        let (_placement, _receipt) = (*error).into_parts();
    }

    #[test]
    fn final_certificate_is_bound_to_one_placement_and_final_bytes() {
        let admitted = admit(&artifact(1));
        let frozen = frozen(&admitted, 102, 0x3000);
        let error = validate_final_placement(frozen, &certificate(&admitted, 103))
            .expect_err("certificate transplant rejects");
        assert!(error.diagnostic().0.contains("does not match"));
    }

    #[test]
    fn unsupported_execute_transition_preserves_all_linear_inputs() {
        let admitted = admit(&artifact(1));
        let validated =
            validate_final_placement(frozen(&admitted, 104, 0x4000), &certificate(&admitted, 104))
                .expect("validated placement");
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
        let placement = placement_authority(105)
            .claim(placement_extent(105, 0x5000, 32))
            .expect("qualified but undersized destination");
        let error = materialize_and_freeze(
            &admitted,
            placement,
            MaterializationReceipt::from_provider(
                admitted.artifact().identity(),
                admitted.admission(),
                id(105, CodePlacementId::from_normalized_identity),
                id(32, PlacementPlanId::from_normalized_identity),
                id(175, FinalBytesId::from_normalized_identity),
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect_err("artifact cannot fit");
        assert!(error.diagnostic().0.contains("smaller"));
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
        let error = placement_authority(106)
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
        materialize_and_freeze(
            &replacement,
            retired.into_placement(),
            MaterializationReceipt::from_provider(
                replacement.artifact().identity(),
                replacement.admission(),
                id(107, CodePlacementId::from_normalized_identity),
                id(32, PlacementPlanId::from_normalized_identity),
                id(177, FinalBytesId::from_normalized_identity),
                id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("placement reusable only after quiescent retirement");
    }
}
