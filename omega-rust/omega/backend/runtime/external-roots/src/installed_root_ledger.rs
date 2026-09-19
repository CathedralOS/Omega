//! The ledger itself: the installed root records it keeps, the borrowed
//! handle that pins installed code while a root is reachable, and the
//! install, remove and teardown operations with the errors that hand every
//! consumed authority back.

use crate::AdmittedProgressProfileEstablishment;
use crate::ComponentVersionPin;
use crate::ExternalRootEntryClaim;
use crate::ExternalRootResultClaim;
use crate::InstalledProviderOccurrenceClosure;
use crate::InstalledRequiredRootSlotClosure;
use crate::LogicalFuelResourceColumn;
use crate::MachineStateResourceColumn;
use crate::OpaqueProviderExitAssurance;
use crate::ProviderExecution;
use crate::StackResourceColumn;
use crate::ValidatedExternalRoot;
use crate::identities::Fnv1a;
use crate::interrupts::interrupt_entries::ActiveInterruptEntry;
use crate::{
    AcknowledgementPolicyId, ExternalRootDiagnostic, ExternalRootId, InstalledProviderOccurrenceId,
    InterruptAcknowledgementId, InterruptInvocationId, NestingRelationId,
    ProgressProfileEstablishmentReceiptId, ProgressProfileGrantInvocationId, ProviderExecutionId,
    ProviderPlanId, RootAdmission, RootAdmissionId, RootEffectId, RootProviderId,
    RootRemovalReceiptId, RootSlotAuthority, RootSlotId, RootSlotOwnerId, TrustReceiptId,
};
use calling_conventions::BoundaryEntryPlan;
use executable_installation::{
    ArtifactId, InstallationRegistryAuthority, InstallationScopeId, InstalledCode,
    InstalledCodeContext, InstalledCodeId,
};
use layout_plans::EntryStubId;
use std::collections::{BTreeMap, BTreeSet};

/// Reportable root record. Compact execution summaries are explicitly named
/// report coordinates and remain beside the complete boundary, stack, fuel,
/// service-reach, provider-exit, and machine-state evidence, never a numeric
/// code address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRootRecord {
    pub root: ExternalRootId,
    /// Normalizer-owned identity of the complete root candidate plus its
    /// validated boundary contract. This is distinct from the friendly/root
    /// slot identity and remains stable across installation placement.
    pub normalized_root_report_identity: u64,
    pub entry: EntryStubId,
    pub installed_code: InstalledCodeId,
    pub artifact: ArtifactId,
    pub slot: RootSlotId,
    pub owner: RootSlotOwnerId,
    pub admission: RootAdmissionId,
    pub provider_execution: ProviderExecutionId,
    pub provider_execution_report_fingerprint: u64,
    pub provider_exit_assurance: OpaqueProviderExitAssurance,
    pub provider_exit_assurance_report_fingerprint: u64,
    pub provider_plan: ProviderPlanId,
    pub requirement_identity: String,
    pub entry_claims: Vec<ExternalRootEntryClaim>,
    pub acknowledgement_parameter_index: Option<usize>,
    pub interrupt_mask_guard_claim: Option<ExternalRootResultClaim>,
    /// Final service row after substituting every installation-bound provider
    /// requirement in this exact root closure.
    pub service_reach: Vec<String>,
    /// Non-authoritative report fingerprint of the selected provider closure
    /// that supplied the rows.
    pub selected_provider_closure_report_fingerprint: u64,
    /// Collision-resistant identity of the complete selected provider closure.
    pub selected_provider_closure_digest: effects::SelectedProviderClosureDigest,
    /// Exact bounded requirement resolutions retained for audit and replay.
    pub installation_reach_resolutions: Vec<effects::InstallationReachResolution>,
    pub boundary_contract_report_fingerprint: u64,
    pub boundary: BoundaryEntryPlan,
    pub provider: RootProviderId,
    pub effects: BTreeSet<RootEffectId>,
    pub trust_receipts: BTreeSet<TrustReceiptId>,
    pub nesting_relation: NestingRelationId,
    pub acknowledgement_policy: Option<AcknowledgementPolicyId>,
    pub stack: StackResourceColumn,
    pub logical_fuel: LogicalFuelResourceColumn,
    pub machine_state: MachineStateResourceColumn,
    pub component_pins: BTreeSet<ComponentVersionPin>,
}

/// Linear liveness pin for one installed external root. Borrowing the code is
/// intentional: retirement needs ownership of `InstalledCode`, which cannot
/// be recovered until every root handle has been removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledRootEvidence {
    pub(crate) root: ValidatedExternalRoot,
    pub(crate) provider_execution: ProviderExecution,
    pub(crate) installed_code: InstalledCodeContext,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
    pub(crate) admission: RootAdmissionId,
}

#[derive(Debug)]
pub struct InstalledExternalRoot<'code> {
    pub(crate) root: ExternalRootId,
    pub(crate) slot: RootSlotId,
    pub(crate) owner: RootSlotOwnerId,
    pub(crate) installed_code: &'code InstalledCode,
    pub(crate) evidence: InstalledRootEvidence,
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

    /// Replay one entry attribution against the complete installed-code
    /// occurrence and the exact entry selected by this admitted root. This is
    /// a descriptive equality gate only: it exposes no executable address or
    /// registration authority.
    pub fn binds_installed_entry(
        &self,
        installed_context: &InstalledCodeContext,
        entry: EntryStubId,
    ) -> bool {
        self.installed_code.receipt_context() == *installed_context
            && self.evidence.installed_code == *installed_context
            && self.evidence.root.candidate.entry == entry
    }

    pub fn installed_artifact_occurrence_digest(
        &self,
    ) -> installation_evidence::InstalledArtifactOccurrenceDigest {
        self.installed_code.occurrence_digest()
    }
}

#[derive(Debug)]
pub struct InstalledRootLedger {
    registry: InstallationRegistryAuthority,
    pub(crate) installed_context: InstalledCodeContext,
    pub(crate) installed_code: InstalledCodeId,
    pub(crate) artifact: ArtifactId,
    pub(crate) installation_scope: InstallationScopeId,
    pub(crate) required_root_slots: Option<InstalledRequiredRootSlotClosure>,
    pub(crate) provider_occurrence_closure: Option<InstalledProviderOccurrenceClosure>,
    pub(crate) admitted_progress_receipts:
        BTreeMap<ProgressProfileEstablishmentReceiptId, AdmittedProgressProfileEstablishment>,
    pub(crate) admitted_progress_invocations: BTreeMap<
        (
            InstalledProviderOccurrenceId,
            ProgressProfileGrantInvocationId,
        ),
        ProgressProfileEstablishmentReceiptId,
    >,
    pub(crate) accepted_component_progress: Vec<effects::ComponentProgressManifest>,
    pub(crate) program_local_root_cohort_claimed: bool,
    pub(crate) roots: BTreeMap<ExternalRootId, InstalledRootRecord>,
    pub(crate) root_evidence: BTreeMap<ExternalRootId, InstalledRootEvidence>,
    slots: BTreeSet<RootSlotId>,
    /// Live interrupt entries keyed by their exact (root, invocation)
    /// identity. Each entry retains the admitted arrival context, the live
    /// epoch stage that only `turn_interrupt_epoch_stage` may advance, the
    /// live nesting depth, and the invocation it preempted so a later nested
    /// arrival rejoins the declared relation instead of a bare root pair.
    pub(crate) active_interrupts:
        BTreeMap<(ExternalRootId, InterruptInvocationId), ActiveInterruptEntry>,
    pub(crate) entered_interrupts: BTreeSet<(ProviderExecutionId, InterruptInvocationId)>,
    pub(crate) minted_acknowledgements: BTreeSet<(ProviderExecutionId, InterruptAcknowledgementId)>,
    /// The fatal entry whose settle halted this ledger. A fatal exception
    /// records the fault and never resumes ordinary work, so after its
    /// settle every still-live invocation stays held as halted evidence: no
    /// further entry, epoch turn, or settle can reach the interrupted chain.
    pub(crate) halted_by: Option<(ExternalRootId, InterruptInvocationId)>,
}

impl InstalledRootLedger {
    /// Claim the sole external-root registry for one exact installed-code
    /// occurrence. The claim is burned in `InstalledCode`, so dropping this
    /// ledger cannot recreate an empty ledger for the same installation.
    pub fn claim(installed_code: &mut InstalledCode) -> Result<Self, ExternalRootDiagnostic> {
        let installed_context = installed_code.receipt_context();
        let registry = installed_code
            .claim_installation_registry()
            .map_err(|diagnostic| ExternalRootDiagnostic(diagnostic.0))?;
        let installation_scope = registry.installation_scope();
        Ok(Self {
            registry,
            installed_context,
            installed_code: installed_code.identity(),
            artifact: installed_code.artifact(),
            installation_scope,
            required_root_slots: None,
            provider_occurrence_closure: None,
            admitted_progress_receipts: BTreeMap::new(),
            admitted_progress_invocations: BTreeMap::new(),
            accepted_component_progress: Vec::new(),
            program_local_root_cohort_claimed: false,
            roots: BTreeMap::new(),
            root_evidence: BTreeMap::new(),
            slots: BTreeSet::new(),
            active_interrupts: BTreeMap::new(),
            entered_interrupts: BTreeSet::new(),
            minted_acknowledgements: BTreeSet::new(),
            halted_by: None,
        })
    }

    pub const fn installation_scope(&self) -> InstallationScopeId {
        self.installation_scope
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    /// The fatal entry whose settle halted this ledger, when one has
    /// settled. A halted ledger admits no new entries, epoch turns, or
    /// settles: the interrupted chain never resumes ordinary work.
    pub const fn halted_by(&self) -> Option<(ExternalRootId, InterruptInvocationId)> {
        self.halted_by
    }

    /// Compare the complete private installation-registry evidence with one
    /// exact installed-code occurrence. Compact identities are insufficient.
    pub fn binds_installed_code(&self, installed_code: &InstalledCode) -> bool {
        self.registry.matches(installed_code)
            && self.installed_context == installed_code.receipt_context()
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn required_root_slots(&self) -> Option<&InstalledRequiredRootSlotClosure> {
        self.required_root_slots.as_ref()
    }

    pub fn records(&self) -> impl Iterator<Item = &InstalledRootRecord> {
        self.roots.values()
    }

    /// Whether all runtime-live external-root custody has left this registry.
    /// Sealed installation history deliberately does not participate: it
    /// remains bound to this exact already-claimed ledger across teardown.
    pub fn live_external_roots_are_empty(&self) -> bool {
        self.roots.is_empty()
            && self.root_evidence.is_empty()
            && self.slots.is_empty()
            && self.active_interrupts.is_empty()
    }

    pub fn record(&self, root: ExternalRootId) -> Option<&InstalledRootRecord> {
        self.roots.get(&root)
    }

    /// Deterministic identity of the currently installed root set.
    ///
    /// Candidate policy is already covered by each root's normalized identity;
    /// this layer binds it to the exact installed realization and owner-scoped
    /// destination. Presentation order cannot affect the result because the
    /// ledger is keyed by the normalized `ExternalRootId`.
    pub fn report_fingerprint(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.u64(self.roots.len() as u64);
        for record in self.roots.values() {
            hash.u64(record.normalized_root_report_identity);
            hash.u64(record.installed_code.normalized_identity());
            hash.u64(record.artifact.normalized_identity());
            hash.u64(record.slot.normalized_identity());
            hash.u64(record.owner.normalized_identity());
            hash.u64(record.admission.normalized_identity());
            hash.u64(record.provider_execution.normalized_identity());
            hash.u64(record.provider_execution_report_fingerprint);
            hash.u64(record.provider_exit_assurance_report_fingerprint);
            hash.u64(record.provider_plan.normalized_identity());
            hash.u64(record.selected_provider_closure_report_fingerprint);
            hash.bytes(record.selected_provider_closure_digest.as_bytes());
            hash.u64(record.boundary_contract_report_fingerprint);
        }
        hash.finish()
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

        if !self.registry.matches(installed_code)
            || installed_code.installation_scope() != self.installation_scope
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root ledger does not belong to the exact installed-code occurrence and installation scope"
                        .into(),
                ),
                root,
                slot,
                admission,
            );
        }

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
        if let Some(existing) = self.roots.values().next()
            && (existing.nesting_relation != root.candidate.nesting_relation
                || existing.stack.realization != root.candidate.stack.realization)
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root stack realization does not match the ledger's artifact-wide nesting composition"
                        .into(),
                ),
                root,
                slot,
                admission,
            );
        }
        if let Err(diagnostic) = admission
            .provider_execution_evidence
            .validate_installed_entry_binding(installed_code)
        {
            return reject(diagnostic, root, slot, admission);
        }
        if admission.root_evidence != root
            || admission.provider_execution_evidence.root_evidence != root
            || admission.provider_execution_evidence.identity != admission.provider_execution
            || admission
                .provider_execution_evidence
                .normalized_report_identity
                != admission.provider_execution_report_fingerprint
            || admission.root_report_identity != root.normalized_report_identity
            || admission.installed_code != installed_code.identity()
            || admission.installed_code_context != installed_code.receipt_context()
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

        // The retained execution evidence is replayed against the validated
        // root it claims, not trusted because an admission carries it: every
        // root-bound coordinate, the honestly recomputed compact report
        // identity, and the opaque exit assurance's own validation all run
        // again here.
        if !admission.provider_execution_evidence.matches_root(&root)
            || !admission
                .provider_execution_evidence
                .replays_root_report_identity(&root)
            || !admission
                .provider_execution_evidence
                .replays_exit_assurance(&root)
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root admission's retained provider execution does not replay the exact validated root, its report identity, or its exit assurance"
                        .into(),
                ),
                root,
                slot,
                admission,
            );
        }

        // The record publishes the admission's reportable copies of the
        // execution's plan and exit assurance; both must equal the retained
        // evidence's values rather than merely claim them.
        if admission.provider_execution_evidence.provider_plan() != admission.provider_plan
            || admission.provider_execution_evidence.exit_assurance()
                != admission.provider_exit_assurance
            || admission
                .provider_execution_evidence
                .exit_assurance_report_fingerprint()
                != admission.provider_exit_assurance_report_fingerprint
        {
            return reject(
                ExternalRootDiagnostic(
                    "external-root admission does not carry the admitted provider execution's exact provider plan and exit assurance"
                        .into(),
                ),
                root,
                slot,
                admission,
            );
        }

        let installed_root_evidence = InstalledRootEvidence {
            root: root.clone(),
            provider_execution: admission.provider_execution_evidence.clone(),
            installed_code: installed_code.receipt_context(),
            slot: slot.slot,
            owner: slot.owner,
            admission: admission.identity,
        };
        let record = InstalledRootRecord {
            root: root.candidate.identity,
            normalized_root_report_identity: root.normalized_report_identity,
            entry: root.candidate.entry,
            installed_code: installed_code.identity(),
            artifact: installed_code.artifact(),
            slot: slot.slot,
            owner: slot.owner,
            admission: admission.identity,
            provider_execution: admission.provider_execution,
            provider_execution_report_fingerprint: admission.provider_execution_report_fingerprint,
            provider_exit_assurance: admission.provider_exit_assurance,
            provider_exit_assurance_report_fingerprint: admission
                .provider_exit_assurance_report_fingerprint,
            provider_plan: admission.provider_plan,
            requirement_identity: root.candidate.requirement_identity,
            entry_claims: root.candidate.entry_claims,
            acknowledgement_parameter_index: root.candidate.acknowledgement_parameter_index,
            interrupt_mask_guard_claim: root.candidate.interrupt_mask_guard_claim,
            service_reach: root.candidate.service_reach.effective().to_vec(),
            selected_provider_closure_report_fingerprint: root
                .candidate
                .service_reach
                .selected_provider_closure_report_fingerprint(),
            selected_provider_closure_digest: root
                .candidate
                .service_reach
                .selected_provider_closure_digest(),
            installation_reach_resolutions: root.candidate.service_reach.resolutions().to_vec(),
            boundary_contract_report_fingerprint: root.boundary_contract_report_fingerprint,
            boundary: root.boundary.plan().clone(),
            provider: root.candidate.provider,
            effects: root.candidate.effects,
            trust_receipts: root.candidate.trust_receipts,
            nesting_relation: root.candidate.nesting_relation,
            acknowledgement_policy: root.candidate.acknowledgement_policy,
            stack: root.candidate.stack,
            logical_fuel: root.candidate.logical_fuel,
            machine_state: root.candidate.machine_state,
            component_pins: root.candidate.component_pins,
        };
        let handle = InstalledExternalRoot {
            root: record.root,
            slot: record.slot,
            owner: record.owner,
            installed_code,
            evidence: installed_root_evidence.clone(),
        };
        self.slots.insert(record.slot);
        self.root_evidence
            .insert(record.root, installed_root_evidence);
        self.roots.insert(record.root, record);
        Ok(handle)
    }

    pub fn remove<'code>(
        &mut self,
        root: InstalledExternalRoot<'code>,
        receipt: RootRemovalReceipt,
    ) -> Result<RootSlotAuthority, Box<RootRemovalError<'code>>> {
        if self
            .required_root_slots
            .as_ref()
            .is_some_and(|closure| closure.slot(root.slot).is_some())
        {
            return Err(Box::new(RootRemovalError {
                root,
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "a sealed required root-slot closure keeps that installed root frozen".into(),
                ),
            }));
        }
        let matches = receipt.root == root.root
            && receipt.slot == root.slot
            && receipt.installed_code == root.installed_code.identity()
            && receipt.installed_root == root.evidence
            && self
                .root_evidence
                .get(&root.root)
                .is_some_and(|evidence| evidence == &root.evidence)
            && receipt.entry_unreachable
            && receipt.executions_quiesced
            && !self
                .active_interrupts
                .keys()
                .any(|(active_root, _)| *active_root == root.root);
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
        self.root_evidence.remove(&root.root);
        self.slots.remove(&root.slot);
        Ok(RootSlotAuthority {
            slot: root.slot,
            owner: root.owner,
        })
    }

    /// Transactionally tear down the complete live installed-root set.
    ///
    /// Every owned row is preflighted before any ledger state changes. This
    /// whole-registry operation intentionally permits members of a sealed
    /// required-slot closure: the closure remains as installation history,
    /// while all runtime root, evidence, slot, and interrupt state leaves.
    pub fn teardown_installed_roots<'code>(
        mut self,
        rows: Vec<InstalledRootRemoval<'code>>,
    ) -> Result<(InstalledRootLedger, Vec<RootSlotAuthority>), Box<InstalledRootTeardownError<'code>>>
    {
        if let Err(diagnostic) = self.preflight_installed_root_teardown(&rows) {
            return Err(Box::new(InstalledRootTeardownError {
                ledger: self,
                rows,
                diagnostic,
            }));
        }

        let authorities = rows
            .iter()
            .map(|row| RootSlotAuthority {
                slot: row.root.slot,
                owner: row.root.owner,
            })
            .collect();
        self.roots.clear();
        self.root_evidence.clear();
        self.slots.clear();
        self.active_interrupts.clear();
        debug_assert!(self.live_external_roots_are_empty());
        drop(rows);
        Ok((self, authorities))
    }

    pub(crate) fn preflight_installed_root_teardown(
        &self,
        rows: &[InstalledRootRemoval<'_>],
    ) -> Result<(), ExternalRootDiagnostic> {
        let recorded_slots = self
            .roots
            .values()
            .map(|record| record.slot)
            .collect::<BTreeSet<_>>();
        if self.root_evidence.len() != self.roots.len()
            || recorded_slots.len() != self.roots.len()
            || self
                .roots
                .keys()
                .any(|identity| !self.root_evidence.contains_key(identity))
            || self.slots != recorded_slots
        {
            return Err(ExternalRootDiagnostic(
                "installed-root teardown requires exact live root, evidence, and slot state".into(),
            ));
        }
        let mut seen = BTreeSet::new();
        for row in rows {
            let root = &row.root;
            let receipt = &row.receipt;
            if !self.registry.matches(root.installed_code)
                || self.installed_context != root.installed_code.receipt_context()
            {
                return Err(ExternalRootDiagnostic(
                    "installed-root teardown row belongs to a different installed-code occurrence"
                        .into(),
                ));
            }
            if !seen.insert(root.root) {
                return Err(ExternalRootDiagnostic(
                    "installed-root teardown contains a duplicate root row".into(),
                ));
            }
            let Some(record) = self.roots.get(&root.root) else {
                return Err(ExternalRootDiagnostic(
                    "installed-root teardown contains a root outside the live ledger".into(),
                ));
            };
            let exact_row = root.root == record.root
                && root.slot == record.slot
                && root.owner == record.owner
                && root.installed_code.identity() == record.installed_code
                && self
                    .root_evidence
                    .get(&root.root)
                    .is_some_and(|evidence| evidence == &root.evidence)
                && self.slots.contains(&root.slot)
                && receipt.root == root.root
                && receipt.slot == root.slot
                && receipt.installed_code == root.installed_code.identity()
                && receipt.installed_root == root.evidence;
            if !exact_row {
                return Err(ExternalRootDiagnostic(
                    "installed-root teardown row does not bind the exact root, evidence, slot, owner, code, and receipt"
                        .into(),
                ));
            }
            if !receipt.entry_unreachable {
                return Err(ExternalRootDiagnostic(
                    "installed-root teardown requires every entry to be unreachable".into(),
                ));
            }
            if !receipt.executions_quiesced {
                return Err(ExternalRootDiagnostic(
                    "installed-root teardown requires every execution to be quiesced".into(),
                ));
            }
            if self
                .active_interrupts
                .keys()
                .any(|(active_root, _)| *active_root == root.root)
            {
                return Err(ExternalRootDiagnostic(
                    "installed-root teardown rejects roots with an active interrupt invocation"
                        .into(),
                ));
            }
        }
        if seen.len() != self.roots.len()
            || self.roots.keys().any(|identity| !seen.contains(identity))
        {
            return Err(ExternalRootDiagnostic(
                "installed-root teardown rows do not completely cover the live ledger".into(),
            ));
        }
        if !self.active_interrupts.is_empty() {
            return Err(ExternalRootDiagnostic(
                "installed-root teardown rejects a ledger with active interrupt invocations".into(),
            ));
        }
        Ok(())
    }
}

/// One owned root handle paired with the provider's exact removal evidence.
#[derive(Debug)]
pub struct InstalledRootRemoval<'code> {
    pub(crate) root: InstalledExternalRoot<'code>,
    pub(crate) receipt: RootRemovalReceipt,
}

impl<'code> InstalledRootRemoval<'code> {
    pub fn new(root: InstalledExternalRoot<'code>, receipt: RootRemovalReceipt) -> Self {
        Self { root, receipt }
    }

    pub const fn root(&self) -> ExternalRootId {
        self.root.root
    }

    pub const fn receipt(&self) -> &RootRemovalReceipt {
        &self.receipt
    }

    pub fn into_parts(self) -> (InstalledExternalRoot<'code>, RootRemovalReceipt) {
        (self.root, self.receipt)
    }
}

#[derive(Debug)]
pub struct RootRemovalReceipt {
    identity: RootRemovalReceiptId,
    installed_root: InstalledRootEvidence,
    root: ExternalRootId,
    slot: RootSlotId,
    installed_code: InstalledCodeId,
    pub(crate) entry_unreachable: bool,
    pub(crate) executions_quiesced: bool,
}

impl RootRemovalReceipt {
    pub fn from_provider(
        identity: RootRemovalReceiptId,
        root: &InstalledExternalRoot<'_>,
        entry_unreachable: bool,
        executions_quiesced: bool,
    ) -> Self {
        Self {
            identity,
            installed_root: root.evidence.clone(),
            root: root.root,
            slot: root.slot,
            installed_code: root.installed_code.identity(),
            entry_unreachable,
            executions_quiesced,
        }
    }

    pub const fn identity(&self) -> RootRemovalReceiptId {
        self.identity
    }
}

#[derive(Debug)]
pub struct InstalledRootTeardownError<'code> {
    ledger: InstalledRootLedger,
    rows: Vec<InstalledRootRemoval<'code>>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> InstalledRootTeardownError<'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InstalledRootLedger, Vec<InstalledRootRemoval<'code>>) {
        (self.ledger, self.rows)
    }
}

impl std::fmt::Display for InstalledRootTeardownError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.0.fmt(formatter)
    }
}

impl std::error::Error for InstalledRootTeardownError<'_> {}

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
