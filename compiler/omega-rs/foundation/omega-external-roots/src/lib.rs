//! Normalized ledger for entry points invoked from outside Omega's call graph.
//!
//! Installing code does not make any of its entries analysis roots. A slot
//! owner separately installs one admitted entry under a validated boundary
//! plan. That operation records the root's effects, trust receipts, stack and
//! nesting policy, WCSU demand, and component/version pins. The returned
//! handle borrows the installed code, preventing retirement while the root is
//! reachable.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{BoundaryEntryPlan, ValidatedBoundaryEntryPlan};
use omega_executable_installation::{ArtifactId, InstalledCode, InstalledCodeId};
use omega_layout_plans::EntryStubId;

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, ExternalRootDiagnostic> {
                if identity == 0 {
                    return Err(ExternalRootDiagnostic(format!(
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

normalized_id!(ExternalRootId, "external-root");
normalized_id!(RootSlotId, "external-root slot");
normalized_id!(RootSlotOwnerId, "external-root slot owner");
normalized_id!(RootProviderId, "external-root provider");
normalized_id!(RootEffectId, "external-root effect");
normalized_id!(TrustReceiptId, "external-root trust receipt");
normalized_id!(NestingRelationId, "external-root nesting relation");
normalized_id!(
    AcknowledgementPolicyId,
    "external-root acknowledgement policy"
);
normalized_id!(ComponentContractId, "component contract");
normalized_id!(ComponentArtifactId, "component artifact");
normalized_id!(ComponentProviderId, "component provider");
normalized_id!(ComponentVersionPinId, "component version pin");
normalized_id!(RootAdmissionId, "external-root admission");
normalized_id!(RootRemovalReceiptId, "external-root removal receipt");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentVersionPin {
    pub contract: ComponentContractId,
    pub artifact: ComponentArtifactId,
    pub provider: ComponentProviderId,
    pub version: ComponentVersionPinId,
}

/// Provider-independent facts required for one externally invoked entry.
///
/// Effects and receipts are normalized open sets. The concrete interrupt,
/// firmware, syscall, or callback package owns their vocabulary; the ledger
/// only requires that admission bind the exact sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootCandidate {
    pub identity: ExternalRootId,
    pub entry: EntryStubId,
    pub provider: RootProviderId,
    pub effects: BTreeSet<RootEffectId>,
    pub trust_receipts: BTreeSet<TrustReceiptId>,
    /// Identity of the artifact-wide relation that names which other roots may
    /// preempt this one. Stack class and maximum depth remain the one copy in
    /// `BoundaryEntryPlan::state`; they are not re-authored here.
    pub nesting_relation: NestingRelationId,
    pub acknowledgement_policy: Option<AcknowledgementPolicyId>,
    pub wcsu_bytes: u64,
    pub wcsu_alignment: u64,
    pub component_pins: BTreeSet<ComponentVersionPin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExternalRoot {
    candidate: ExternalRootCandidate,
    boundary: BoundaryEntryPlan,
    boundary_contract_fingerprint: u64,
    normalized_identity: u64,
}

impl ValidatedExternalRoot {
    pub const fn candidate(&self) -> &ExternalRootCandidate {
        &self.candidate
    }

    pub const fn boundary(&self) -> &BoundaryEntryPlan {
        &self.boundary
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
    }
}

pub fn validate_external_root(
    candidate: ExternalRootCandidate,
    boundary: &ValidatedBoundaryEntryPlan,
) -> Result<ValidatedExternalRoot, ExternalRootDiagnostic> {
    if candidate.wcsu_bytes == 0 {
        return Err(ExternalRootDiagnostic(
            "external-root WCSU demand must be nonzero".into(),
        ));
    }
    if candidate.wcsu_alignment == 0 || !candidate.wcsu_alignment.is_power_of_two() {
        return Err(ExternalRootDiagnostic(format!(
            "external-root WCSU alignment {} is not a nonzero power of two",
            candidate.wcsu_alignment
        )));
    }
    let mut component_contracts = BTreeSet::new();
    for pin in &candidate.component_pins {
        if !component_contracts.insert(pin.contract) {
            return Err(ExternalRootDiagnostic(
                "external root cannot pin more than one realization of one component contract"
                    .into(),
            ));
        }
    }

    let boundary_contract_fingerprint = boundary.contract_fingerprint();
    let normalized_identity = fingerprint_root(&candidate, boundary_contract_fingerprint);
    Ok(ValidatedExternalRoot {
        candidate,
        boundary: boundary.plan().clone(),
        boundary_contract_fingerprint,
        normalized_identity,
    })
}

/// Linear authority over one external-entry destination slot.
#[derive(Debug, PartialEq, Eq)]
pub struct RootSlotAuthority {
    slot: RootSlotId,
    owner: RootSlotOwnerId,
}

impl RootSlotAuthority {
    pub const fn from_admitted_owner(slot: RootSlotId, owner: RootSlotOwnerId) -> Self {
        Self { slot, owner }
    }

    pub const fn slot(&self) -> RootSlotId {
        self.slot
    }

    pub const fn owner(&self) -> RootSlotOwnerId {
        self.owner
    }
}

/// Admission commitment for one exact root, installed-code realization, and
/// owner-controlled destination slot. Construction represents provider
/// admission; ordinary callers cannot weaken its private bindings.
#[derive(Debug, PartialEq, Eq)]
pub struct RootAdmission {
    identity: RootAdmissionId,
    root_identity: u64,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
    trust_receipts: BTreeSet<TrustReceiptId>,
}

impl RootAdmission {
    pub fn from_admitted_provider(
        identity: RootAdmissionId,
        root: &ValidatedExternalRoot,
        installed_code: &InstalledCode,
        slot: &RootSlotAuthority,
        trust_receipts: impl IntoIterator<Item = TrustReceiptId>,
    ) -> Self {
        Self {
            identity,
            root_identity: root.normalized_identity,
            installed_code: installed_code.identity(),
            artifact: installed_code.artifact(),
            slot: slot.slot,
            owner: slot.owner,
            trust_receipts: trust_receipts.into_iter().collect(),
        }
    }

    pub const fn identity(&self) -> RootAdmissionId {
        self.identity
    }
}

/// Reportable root record. It contains normalized identities and the complete
/// boundary plan, never a numeric code address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRootRecord {
    pub root: ExternalRootId,
    pub entry: EntryStubId,
    pub installed_code: InstalledCodeId,
    pub artifact: ArtifactId,
    pub slot: RootSlotId,
    pub owner: RootSlotOwnerId,
    pub admission: RootAdmissionId,
    pub boundary_contract_fingerprint: u64,
    pub boundary: BoundaryEntryPlan,
    pub provider: RootProviderId,
    pub effects: BTreeSet<RootEffectId>,
    pub trust_receipts: BTreeSet<TrustReceiptId>,
    pub nesting_relation: NestingRelationId,
    pub acknowledgement_policy: Option<AcknowledgementPolicyId>,
    pub wcsu_bytes: u64,
    pub wcsu_alignment: u64,
    pub component_pins: BTreeSet<ComponentVersionPin>,
}

/// Linear liveness pin for one installed external root. Borrowing the code is
/// intentional: retirement needs ownership of `InstalledCode`, which cannot
/// be recovered until every root handle has been removed.
#[derive(Debug)]
pub struct InstalledExternalRoot<'code> {
    root: ExternalRootId,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
    installed_code: &'code InstalledCode,
}

impl InstalledExternalRoot<'_> {
    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn slot(&self) -> RootSlotId {
        self.slot
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code.identity()
    }
}

#[derive(Debug, Default)]
pub struct InstalledRootLedger {
    roots: BTreeMap<ExternalRootId, InstalledRootRecord>,
    slots: BTreeSet<RootSlotId>,
}

impl InstalledRootLedger {
    pub fn records(&self) -> impl Iterator<Item = &InstalledRootRecord> {
        self.roots.values()
    }

    pub fn record(&self, root: ExternalRootId) -> Option<&InstalledRootRecord> {
        self.roots.get(&root)
    }

    pub fn install<'code>(
        &mut self,
        installed_code: &'code InstalledCode,
        root: ValidatedExternalRoot,
        slot: RootSlotAuthority,
        admission: RootAdmission,
    ) -> Result<InstalledExternalRoot<'code>, Box<RootInstallError>> {
        let reject = |diagnostic: ExternalRootDiagnostic,
                      root: ValidatedExternalRoot,
                      slot: RootSlotAuthority,
                      admission: RootAdmission| {
            Err(Box::new(RootInstallError {
                root,
                slot,
                admission,
                diagnostic,
            }))
        };

        if self.roots.contains_key(&root.candidate.identity) {
            return reject(
                ExternalRootDiagnostic("external-root identity is already installed".into()),
                root,
                slot,
                admission,
            );
        }
        if self.slots.contains(&slot.slot) {
            return reject(
                ExternalRootDiagnostic("external-root slot is already occupied".into()),
                root,
                slot,
                admission,
            );
        }
        if installed_code
            .selected_entry_target(root.candidate.entry)
            .is_err()
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root entry is not in the admitted installed artifact".into(),
                ),
                root,
                slot,
                admission,
            );
        }
        if admission.root_identity != root.normalized_identity
            || admission.installed_code != installed_code.identity()
            || admission.artifact != installed_code.artifact()
            || admission.slot != slot.slot
            || admission.owner != slot.owner
            || admission.trust_receipts != root.candidate.trust_receipts
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root admission does not bind the exact root, code, slot, owner, and trust receipts"
                        .into(),
                ),
                root,
                slot,
                admission,
            );
        }

        let record = InstalledRootRecord {
            root: root.candidate.identity,
            entry: root.candidate.entry,
            installed_code: installed_code.identity(),
            artifact: installed_code.artifact(),
            slot: slot.slot,
            owner: slot.owner,
            admission: admission.identity,
            boundary_contract_fingerprint: root.boundary_contract_fingerprint,
            boundary: root.boundary,
            provider: root.candidate.provider,
            effects: root.candidate.effects,
            trust_receipts: root.candidate.trust_receipts,
            nesting_relation: root.candidate.nesting_relation,
            acknowledgement_policy: root.candidate.acknowledgement_policy,
            wcsu_bytes: root.candidate.wcsu_bytes,
            wcsu_alignment: root.candidate.wcsu_alignment,
            component_pins: root.candidate.component_pins,
        };
        let handle = InstalledExternalRoot {
            root: record.root,
            slot: record.slot,
            owner: record.owner,
            installed_code,
        };
        self.slots.insert(record.slot);
        self.roots.insert(record.root, record);
        Ok(handle)
    }

    pub fn remove<'code>(
        &mut self,
        root: InstalledExternalRoot<'code>,
        receipt: RootRemovalReceipt,
    ) -> Result<RootSlotAuthority, Box<RootRemovalError<'code>>> {
        let matches = receipt.root == root.root
            && receipt.slot == root.slot
            && receipt.installed_code == root.installed_code.identity()
            && receipt.entry_unreachable
            && receipt.executions_quiesced;
        if !matches || !self.roots.contains_key(&root.root) {
            return Err(Box::new(RootRemovalError {
                root,
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "external-root removal receipt does not prove exact-slot unreachability and quiescence"
                        .into(),
                ),
            }));
        }
        self.roots.remove(&root.root);
        self.slots.remove(&root.slot);
        Ok(RootSlotAuthority {
            slot: root.slot,
            owner: root.owner,
        })
    }
}

#[derive(Debug)]
pub struct RootRemovalReceipt {
    identity: RootRemovalReceiptId,
    root: ExternalRootId,
    slot: RootSlotId,
    installed_code: InstalledCodeId,
    entry_unreachable: bool,
    executions_quiesced: bool,
}

impl RootRemovalReceipt {
    pub const fn from_provider(
        identity: RootRemovalReceiptId,
        root: ExternalRootId,
        slot: RootSlotId,
        installed_code: InstalledCodeId,
        entry_unreachable: bool,
        executions_quiesced: bool,
    ) -> Self {
        Self {
            identity,
            root,
            slot,
            installed_code,
            entry_unreachable,
            executions_quiesced,
        }
    }

    pub const fn identity(&self) -> RootRemovalReceiptId {
        self.identity
    }
}

#[derive(Debug)]
pub struct RootInstallError {
    root: ValidatedExternalRoot,
    slot: RootSlotAuthority,
    admission: RootAdmission,
    diagnostic: ExternalRootDiagnostic,
}

impl RootInstallError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedExternalRoot, RootSlotAuthority, RootAdmission) {
        (self.root, self.slot, self.admission)
    }
}

#[derive(Debug)]
pub struct RootRemovalError<'code> {
    root: InstalledExternalRoot<'code>,
    receipt: RootRemovalReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> RootRemovalError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InstalledExternalRoot<'code>, RootRemovalReceipt) {
        (self.root, self.receipt)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootDiagnostic(pub String);

impl std::fmt::Display for ExternalRootDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExternalRootDiagnostic {}

fn fingerprint_root(candidate: &ExternalRootCandidate, boundary: u64) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(candidate.identity.normalized_identity());
    hash.u64(candidate.entry.normalized_identity());
    hash.u64(candidate.provider.normalized_identity());
    hash.u64(boundary);
    hash.u64(candidate.nesting_relation.normalized_identity());
    hash.u64(
        candidate
            .acknowledgement_policy
            .map(AcknowledgementPolicyId::normalized_identity)
            .unwrap_or_default(),
    );
    hash.u64(candidate.wcsu_bytes);
    hash.u64(candidate.wcsu_alignment);
    for effect in &candidate.effects {
        hash.u64(effect.normalized_identity());
    }
    hash.u64(0xff01);
    for receipt in &candidate.trust_receipts {
        hash.u64(receipt.normalized_identity());
    }
    hash.u64(0xff02);
    for pin in &candidate.component_pins {
        hash.u64(pin.contract.normalized_identity());
        hash.u64(pin.artifact.normalized_identity());
        hash.u64(pin.provider.normalized_identity());
        hash.u64(pin.version.normalized_identity());
    }
    hash.finish()
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
    };
    use omega_executable_installation::{
        AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
        CodePlacementAuthority, CodePlacementId, EntrySetId, FinalBytesId,
        FinalValidationCertificate, FinalValidationId, InstallAuthority, InstallationAudience,
        InstallationReceipt, InstallationScopeId, MachineContractSetId, MachineFootprintId,
        MaterializationReceipt, PlacementPlanId, WxEnforcement, admit_executable,
        install_validated, materialize_and_freeze, validate_final_placement,
    };
    use omega_extents::{
        AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
        ExtentRights, ExtentRootGrant, MappingEraId,
    };
    use omega_layout_plans::{
        ArtifactInstallationScopeId, PlacementAddressRange, PlacementConstraints, PlacementPhase,
        PlacementSite,
    };

    fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        constructor(identity).expect("normalized external-root identity")
    }

    fn install_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn entry_id(identity: u64) -> EntryStubId {
        EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
    }

    fn constraints() -> PlacementConstraints {
        PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
            4096,
            PlacementPhase::PostHandoff,
            None,
            Some(
                ArtifactInstallationScopeId::from_normalized_identity(61)
                    .expect("installation scope"),
            ),
        )
        .expect("placement constraints")
    }

    fn installed_code(artifact_identity: u64, entry: EntryStubId) -> InstalledCode {
        let artifact = Artifact::from_canonical_decode(
            install_id(artifact_identity, ArtifactId::from_normalized_identity),
            install_id(
                artifact_identity + 10,
                ArtifactContentId::from_normalized_identity,
            ),
            64,
            install_id(30, MachineContractSetId::from_normalized_identity),
            install_id(31, MachineFootprintId::from_normalized_identity),
            install_id(32, PlacementPlanId::from_normalized_identity),
            constraints(),
            install_id(33, EntrySetId::from_normalized_identity),
            vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        )
        .expect("artifact");
        let admitted = admit_executable(
            &artifact,
            ArtifactAdmissionEvidence::from_validator(
                install_id(40, AdmissionReceiptId::from_normalized_identity),
                artifact.identity(),
                install_id(
                    artifact_identity + 10,
                    ArtifactContentId::from_normalized_identity,
                ),
                install_id(30, MachineContractSetId::from_normalized_identity),
                install_id(31, MachineFootprintId::from_normalized_identity),
                install_id(32, PlacementPlanId::from_normalized_identity),
                constraints(),
                install_id(33, EntrySetId::from_normalized_identity),
                true,
            ),
        )
        .expect("admitted artifact");

        let rights = ExtentRights::from_normalized_identities([extent_id(
            51,
            ExtentRightId::from_normalized_identity,
        )]);
        let extent = ExtentRootGrant::from_admitted_provider(
            extent_id(100, ExtentLineageId::from_normalized_identity),
            extent_id(50, AddressSpaceId::from_normalized_identity),
            rights.clone(),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            extent_id(53, MappingEraId::from_normalized_identity),
        )
        .mint(0x1000, 4096)
        .expect("placement extent");
        let placement = CodePlacementAuthority::from_admitted_provider(
            install_id(100, CodePlacementId::from_normalized_identity),
            install_id(61, InstallationScopeId::from_normalized_identity),
            InstallationAudience::FutureFetcher,
            extent_id(50, AddressSpaceId::from_normalized_identity),
            extent_id(52, ExtentProvenanceId::from_normalized_identity),
            rights,
            constraints(),
            PlacementSite {
                base_address: 0x1000,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: Some(
                    ArtifactInstallationScopeId::from_normalized_identity(61)
                        .expect("installation scope"),
                ),
            },
        )
        .claim(extent)
        .expect("placement");
        let frozen = materialize_and_freeze(
            &admitted,
            placement,
            MaterializationReceipt::from_provider(
                artifact.identity(),
                admitted.admission(),
                install_id(100, CodePlacementId::from_normalized_identity),
                install_id(32, PlacementPlanId::from_normalized_identity),
                install_id(170, FinalBytesId::from_normalized_identity),
                install_id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("frozen placement");
        let validated = validate_final_placement(
            frozen,
            &FinalValidationCertificate::from_validator(
                install_id(180, FinalValidationId::from_normalized_identity),
                artifact.identity(),
                admitted.admission(),
                install_id(100, CodePlacementId::from_normalized_identity),
                install_id(170, FinalBytesId::from_normalized_identity),
                install_id(71, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("validated placement");
        install_validated(
            validated,
            InstallAuthority::from_admitted_provider(
                artifact.identity(),
                admitted.admission(),
                install_id(100, CodePlacementId::from_normalized_identity),
                install_id(61, InstallationScopeId::from_normalized_identity),
                InstallationAudience::FutureFetcher,
            ),
            InstallationReceipt::from_provider(
                install_id(300, InstalledCodeId::from_normalized_identity),
                artifact.identity(),
                admitted.admission(),
                install_id(100, CodePlacementId::from_normalized_identity),
                install_id(61, InstallationScopeId::from_normalized_identity),
                install_id(180, FinalValidationId::from_normalized_identity),
                true,
                WxEnforcement::HardwareEnforced,
            ),
        )
        .expect("installed code")
    }

    fn boundary() -> ValidatedBoundaryEntryPlan {
        evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .expect("validated boundary")
    }

    fn candidate(entry: EntryStubId) -> ExternalRootCandidate {
        ExternalRootCandidate {
            identity: root_id(1, ExternalRootId::from_normalized_identity),
            entry,
            provider: root_id(2, RootProviderId::from_normalized_identity),
            effects: [root_id(3, RootEffectId::from_normalized_identity)]
                .into_iter()
                .collect(),
            trust_receipts: [root_id(4, TrustReceiptId::from_normalized_identity)]
                .into_iter()
                .collect(),
            nesting_relation: root_id(6, NestingRelationId::from_normalized_identity),
            acknowledgement_policy: Some(root_id(
                7,
                AcknowledgementPolicyId::from_normalized_identity,
            )),
            wcsu_bytes: 4096,
            wcsu_alignment: 16,
            component_pins: [ComponentVersionPin {
                contract: root_id(8, ComponentContractId::from_normalized_identity),
                artifact: root_id(9, ComponentArtifactId::from_normalized_identity),
                provider: root_id(10, ComponentProviderId::from_normalized_identity),
                version: root_id(11, ComponentVersionPinId::from_normalized_identity),
            }]
            .into_iter()
            .collect(),
        }
    }

    fn slot() -> RootSlotAuthority {
        RootSlotAuthority::from_admitted_owner(
            root_id(20, RootSlotId::from_normalized_identity),
            root_id(21, RootSlotOwnerId::from_normalized_identity),
        )
    }

    #[test]
    fn installation_records_the_complete_external_root_and_pins_code_liveness() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
        let authority = slot();
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        );
        let mut ledger = InstalledRootLedger::default();
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed external root");

        let record = ledger.record(installed.root()).expect("root record");
        assert_eq!(record.entry, entry);
        assert_eq!(record.installed_code, code.identity());
        assert_eq!(record.effects.len(), 1);
        assert_eq!(record.trust_receipts.len(), 1);
        assert_eq!(record.wcsu_bytes, 4096);
        assert_eq!(record.component_pins.len(), 1);
        assert_eq!(
            record.boundary_contract_fingerprint,
            boundary().contract_fingerprint()
        );

        let root_identity = installed.root();
        let root_slot = installed.slot();
        let receipt = RootRemovalReceipt::from_provider(
            root_id(23, RootRemovalReceiptId::from_normalized_identity),
            root_identity,
            root_slot,
            installed.installed_code(),
            true,
            true,
        );
        let returned = ledger.remove(installed, receipt).expect("root removal");
        assert_eq!(returned.slot(), root_slot);
        assert!(ledger.record(root_identity).is_none());
    }

    #[test]
    fn install_rejects_foreign_entries_and_returns_every_consumed_authority() {
        let admitted_entry = entry_id(1001);
        let code = installed_code(1, admitted_entry);
        let foreign_entry = entry_id(1002);
        let validated =
            validate_external_root(candidate(foreign_entry), &boundary()).expect("root plan");
        let authority = slot();
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        );
        let mut ledger = InstalledRootLedger::default();
        let error = ledger
            .install(&code, validated, authority, admission)
            .expect_err("foreign entry must reject");

        assert!(error.diagnostic().0.contains("not in the admitted"));
        let (root, slot, admission) = error.into_parts();
        assert_eq!(root.candidate().entry, foreign_entry);
        assert_eq!(
            slot.slot(),
            root_id(20, RootSlotId::from_normalized_identity)
        );
        assert_eq!(
            admission.identity(),
            root_id(22, RootAdmissionId::from_normalized_identity)
        );
        assert_eq!(ledger.records().count(), 0);
    }

    #[test]
    fn removal_requires_both_unreachability_and_execution_quiescence() {
        let entry = entry_id(1001);
        let code = installed_code(1, entry);
        let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
        let authority = slot();
        let admission = RootAdmission::from_admitted_provider(
            root_id(22, RootAdmissionId::from_normalized_identity),
            &validated,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        );
        let mut ledger = InstalledRootLedger::default();
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed external root");
        let receipt = RootRemovalReceipt::from_provider(
            root_id(23, RootRemovalReceiptId::from_normalized_identity),
            installed.root(),
            installed.slot(),
            installed.installed_code(),
            true,
            false,
        );
        let error = ledger
            .remove(installed, receipt)
            .expect_err("live executions prevent slot reuse");
        assert!(error.diagnostic().0.contains("quiescence"));
        assert_eq!(ledger.records().count(), 1);
        let (installed, _) = error.into_parts();
        assert_eq!(installed.installed_code(), code.identity());
    }

    #[test]
    fn wcsu_demand_is_validated_before_a_root_can_enter_the_ledger() {
        let mut invalid = candidate(entry_id(1001));
        invalid.wcsu_alignment = 3;
        let error = validate_external_root(invalid, &boundary()).expect_err("bad WCSU alignment");
        assert!(error.0.contains("power of two"));

        let mut empty = candidate(entry_id(1001));
        empty.wcsu_bytes = 0;
        let error = validate_external_root(empty, &boundary()).expect_err("zero WCSU");
        assert!(error.0.contains("nonzero"));

        let mut conflicting = candidate(entry_id(1001));
        conflicting.component_pins.insert(ComponentVersionPin {
            contract: root_id(8, ComponentContractId::from_normalized_identity),
            artifact: root_id(90, ComponentArtifactId::from_normalized_identity),
            provider: root_id(91, ComponentProviderId::from_normalized_identity),
            version: root_id(92, ComponentVersionPinId::from_normalized_identity),
        });
        let error = validate_external_root(conflicting, &boundary())
            .expect_err("one contract cannot pin two component realizations");
        assert!(error.0.contains("more than one realization"));
    }
}
