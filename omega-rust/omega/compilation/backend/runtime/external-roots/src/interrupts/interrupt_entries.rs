//! One interrupt entry from arrival to settled exit: the receipt and
//! invocation evidence, the linear obligations it mints, acknowledgement
//! through the installed completion route, and the ledger's begin and
//! finish operations.

use crate::installed_root_ledger::InstalledRootEvidence;
use crate::{
    AcknowledgementPolicyId, AdmittedEntryQualification, AdmittedEntrySubject,
    ExternalRootDiagnostic, ExternalRootId, InstalledExternalRoot, InstalledRootLedger,
    InterruptAcknowledgementId, InterruptAcknowledgementReceiptId, InterruptEntryReceiptId,
    InterruptInvocationId, InterruptMaskControl, InterruptMaskControlId, InterruptMaskStateId,
    ProviderExecutionId, ProviderPlanId, RootSlotId,
};
use calling_conventions::{
    ArrivalContextId, EntryControl, EntryStackStage, Preemption, StackDomainRef,
};
use executable_installation::InstalledCodeId;
use std::collections::BTreeSet;

/// The provider's report of which live invocation one arriving entry
/// preempts. A nested arrival is only admissible when the interrupted
/// invocation is the innermost live entry, the artifact-wide nesting relation
/// declares the edge, and the reported epoch stage rejoins the stage the
/// ledger retains for that invocation — a stage the ledger never admitted is
/// a stale disposition, not a description of the interrupted entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptPreemptionReport {
    interrupted_root: ExternalRootId,
    interrupted_invocation: InterruptInvocationId,
    interrupted_stage: EntryStackStage,
}

impl InterruptPreemptionReport {
    pub const fn new(
        interrupted_root: ExternalRootId,
        interrupted_invocation: InterruptInvocationId,
        interrupted_stage: EntryStackStage,
    ) -> Self {
        Self {
            interrupted_root,
            interrupted_invocation,
            interrupted_stage,
        }
    }

    pub const fn interrupted_root(&self) -> ExternalRootId {
        self.interrupted_root
    }

    pub const fn interrupted_invocation(&self) -> InterruptInvocationId {
        self.interrupted_invocation
    }

    /// The live epoch stage the provider reports for the interrupted
    /// invocation. The report must equal the stage the ledger retains from
    /// the invocation's own admission and later epoch turns; every epoch of
    /// the interrupted context at that stage must then permit the nested
    /// depth, so a masked or unbounded sibling epoch is an unresolved
    /// disposition.
    pub const fn interrupted_stage(&self) -> EntryStackStage {
        self.interrupted_stage
    }
}

/// The provider's report that one live interrupt invocation turned from its
/// retained epoch stage to the next stage its admitted arrival context
/// realizes. A side-epoch move between sibling epochs of one stage is not
/// observable at this granularity; the ledger tracks the stage sequence, so
/// the report names only the destination stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptEpochTurnReport {
    root: ExternalRootId,
    invocation: InterruptInvocationId,
    turned_stage: EntryStackStage,
}

impl InterruptEpochTurnReport {
    pub const fn new(
        root: ExternalRootId,
        invocation: InterruptInvocationId,
        turned_stage: EntryStackStage,
    ) -> Self {
        Self {
            root,
            invocation,
            turned_stage,
        }
    }

    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn invocation(&self) -> InterruptInvocationId {
        self.invocation
    }

    /// The stage the provider reports the invocation turned into. It must be
    /// the next stage the admitted arrival context realizes after the
    /// retained stage: a stage the context never realizes, a backward turn,
    /// and a skipped stage are all unresolved dispositions.
    pub const fn turned_stage(&self) -> EntryStackStage {
        self.turned_stage
    }
}

/// Ledger-visible state for one admitted, not yet settled interrupt entry.
/// The retained arrival context, live epoch stage, and parent edge let a
/// later nested arrival rejoin the exact epoch evidence the composition
/// assumed instead of trusting a fresh provider description of the parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActiveInterruptEntry {
    pub(crate) arrival_context: ArrivalContextId,
    /// The invocation's live epoch stage inside its admitted arrival context.
    /// A fresh activation realizes at the context's first epoch stage — Enter
    /// when the context realizes enter epochs, otherwise its single Body —
    /// and moves only through [`InstalledRootLedger::turn_interrupt_epoch_stage`].
    /// A preempted entry's stage is frozen while its nested child is live.
    pub(crate) stage: EntryStackStage,
    /// Simultaneously live occurrences on this root lineage, including this
    /// one. A top-level arrival is depth 1; a nested arrival is parent + 1.
    pub(crate) depth: u16,
    /// The live invocation this entry preempted, when nested.
    pub(crate) interrupted: Option<(ExternalRootId, InterruptInvocationId)>,
    /// Whether this entry carries a fatal-exception obligation: it arrived
    /// unconditionally — a processor fault cannot wait on a declared
    /// stack-nesting edge — and its settle halts the ledger rather than
    /// resuming the interrupted chain.
    pub(crate) fatal: bool,
}

/// Provider evidence for one concrete invocation of an installed interrupt
/// root. The exact installed realization and acknowledgement policy are bound
/// before the source-visible opaque obligations are minted.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptEntryReceipt {
    identity: InterruptEntryReceiptId,
    installed_root: InstalledRootEvidence,
    root: ExternalRootId,
    slot: RootSlotId,
    installed_code: InstalledCodeId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    /// The arrival context this invocation fired under. It must be one of the
    /// exact contexts the installed root's bound epoch realization admitted;
    /// an unadmitted or foreign-root context is an unresolved or cross-context
    /// disposition and rejects.
    arrival_context: ArrivalContextId,
    /// Which live invocation this entry preempts. Absent requires no live
    /// interrupt on this ledger; present requires the named invocation to be
    /// the innermost live entry under a declared nesting edge whose preempted
    /// epoch stage permits the nested depth.
    preemption: Option<InterruptPreemptionReport>,
    mask_control: InterruptMaskControlId,
    initial_mask_state: InterruptMaskStateId,
    acknowledgement_policy: Option<AcknowledgementPolicyId>,
    acknowledgement: Option<InterruptAcknowledgementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterruptInvocationEvidence {
    installed_root: InstalledRootEvidence,
    entry_receipt: InterruptEntryReceiptId,
    invocation: InterruptInvocationId,
    arrival_context: ArrivalContextId,
    preemption: Option<InterruptPreemptionReport>,
    mask_control: InterruptMaskControlId,
    initial_mask_state: InterruptMaskStateId,
    acknowledgement_policy: Option<AcknowledgementPolicyId>,
    acknowledgement: Option<InterruptAcknowledgementId>,
}

impl InterruptInvocationEvidence {
    fn from_entry_receipt(receipt: &InterruptEntryReceipt) -> Self {
        Self {
            installed_root: receipt.installed_root.clone(),
            entry_receipt: receipt.identity,
            invocation: receipt.invocation,
            arrival_context: receipt.arrival_context,
            preemption: receipt.preemption,
            mask_control: receipt.mask_control,
            initial_mask_state: receipt.initial_mask_state,
            acknowledgement_policy: receipt.acknowledgement_policy,
            acknowledgement: receipt.acknowledgement,
        }
    }
}

impl InterruptEntryReceipt {
    pub fn from_provider(
        identity: InterruptEntryReceiptId,
        root: &InstalledExternalRoot<'_>,
        invocation: InterruptInvocationId,
        arrival_context: ArrivalContextId,
        preemption: Option<InterruptPreemptionReport>,
        mask_control: InterruptMaskControlId,
        initial_mask_state: InterruptMaskStateId,
        acknowledgement_policy: Option<AcknowledgementPolicyId>,
        acknowledgement: Option<InterruptAcknowledgementId>,
    ) -> Self {
        Self {
            identity,
            installed_root: root.evidence.clone(),
            root: root.root,
            slot: root.slot,
            installed_code: root.installed_code.identity(),
            provider_execution: root.evidence.provider_execution.identity,
            invocation,
            arrival_context,
            preemption,
            mask_control,
            initial_mask_state,
            acknowledgement_policy,
            acknowledgement,
        }
    }

    pub const fn identity(&self) -> InterruptEntryReceiptId {
        self.identity
    }
}

/// The provider-owned half of an active interrupt. It must be reunited with
/// the exact restored mask control and completed acknowledgement before the
/// ledger accepts the deriver-owned exit.
#[derive(Debug, PartialEq, Eq)]
pub struct PendingInterruptExit {
    entry_receipt: InterruptEntryReceiptId,
    invocation_evidence: InterruptInvocationEvidence,
    root: ExternalRootId,
    installed_code: InstalledCodeId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    arrival_context: ArrivalContextId,
    mask_control: InterruptMaskControlId,
    initial_mask_state: InterruptMaskStateId,
    acknowledgement_policy: Option<AcknowledgementPolicyId>,
    acknowledgement: Option<InterruptAcknowledgementId>,
}

/// Provider-minted source obligations for one admitted interrupt invocation.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptEntryObligations {
    pending_exit: PendingInterruptExit,
    mask_control: InterruptMaskControl,
    acknowledgement: Option<InterruptAcknowledgement>,
}

impl InterruptEntryObligations {
    pub fn into_parts(
        self,
    ) -> (
        PendingInterruptExit,
        InterruptMaskControl,
        Option<InterruptAcknowledgement>,
    ) {
        (self.pending_exit, self.mask_control, self.acknowledgement)
    }
}

/// Opaque linear acknowledgement minted only by an admitted entry receipt.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptAcknowledgement {
    invocation_evidence: InterruptInvocationEvidence,
    identity: InterruptAcknowledgementId,
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    policy: AcknowledgementPolicyId,
    qualifications: Vec<AdmittedEntryQualification>,
}

/// Sealed installation evidence for one provider-completed interrupt
/// acknowledgement requirement.
///
/// The selected reach row is descriptive, not authority. Construction joins
/// it back to the exact installed root, provider execution, acknowledgement
/// policy, invocation, and linear acknowledgement occurrence before a provider
/// receipt can settle the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInterruptCompletionRoute {
    root: ExternalRootId,
    normalized_root_report_identity: u64,
    entry_provider_plan: ProviderPlanId,
    pub(crate) provider_execution: ProviderExecutionId,
    provider_execution_report_fingerprint: u64,
    pub(crate) completion_requirement_identity: String,
    pub(crate) resolution: effects::InstallationReachResolution,
    entry_receipt: InterruptEntryReceiptId,
    pub(crate) invocation: InterruptInvocationId,
    pub(crate) policy: AcknowledgementPolicyId,
    pub(crate) acknowledgement: InterruptAcknowledgementId,
}

impl InstalledInterruptCompletionRoute {
    fn from_provider_requirement(
        acknowledgement: &InterruptAcknowledgement,
        completion_requirement_identity: &str,
    ) -> Result<Self, ExternalRootDiagnostic> {
        if completion_requirement_identity.is_empty() {
            return Err(ExternalRootDiagnostic(
                "interrupt acknowledgement completion requirement identity cannot be empty".into(),
            ));
        }
        let installed = &acknowledgement.invocation_evidence.installed_root;
        let root = &installed.root;
        if !installed.provider_execution.matches_root(root)
            || installed.provider_execution.identity != acknowledgement.provider_execution
        {
            return Err(ExternalRootDiagnostic(
                "interrupt acknowledgement completion does not retain the exact installed provider execution"
                    .into(),
            ));
        }
        let mut matches = root
            .candidate
            .service_reach
            .resolutions()
            .iter()
            .filter(|resolution| {
                resolution.requirement_identity == completion_requirement_identity
            });
        let Some(resolution) = matches.next() else {
            return Err(ExternalRootDiagnostic(format!(
                "interrupt acknowledgement completion requirement `{completion_requirement_identity}` is absent from the exact installed reach"
            )));
        };
        if matches.next().is_some()
            || !root
                .candidate
                .service_reach
                .installation_requirements()
                .iter()
                .any(|requirement| requirement == completion_requirement_identity)
        {
            return Err(ExternalRootDiagnostic(format!(
                "interrupt acknowledgement completion requirement `{completion_requirement_identity}` does not have one exact installed reach resolution"
            )));
        }
        Ok(Self {
            root: acknowledgement.root,
            normalized_root_report_identity: root.normalized_report_identity,
            entry_provider_plan: installed.provider_execution.provider_plan,
            provider_execution: acknowledgement.provider_execution,
            provider_execution_report_fingerprint: installed
                .provider_execution
                .normalized_report_identity,
            completion_requirement_identity: completion_requirement_identity.into(),
            resolution: resolution.clone(),
            entry_receipt: acknowledgement.invocation_evidence.entry_receipt,
            invocation: acknowledgement.invocation,
            policy: acknowledgement.policy,
            acknowledgement: acknowledgement.identity,
        })
    }

    fn matches_acknowledgement(&self, acknowledgement: &InterruptAcknowledgement) -> bool {
        let installed = &acknowledgement.invocation_evidence.installed_root;
        let root = &installed.root;
        let replayed_resolution =
            root.candidate
                .service_reach
                .resolutions()
                .iter()
                .find(|resolution| {
                    resolution.requirement_identity == self.completion_requirement_identity
                });
        installed.provider_execution.matches_root(root)
            && self.root == acknowledgement.root
            && self.normalized_root_report_identity == root.normalized_report_identity
            && self.entry_provider_plan == installed.provider_execution.provider_plan
            && self.provider_execution == acknowledgement.provider_execution
            && self.provider_execution == installed.provider_execution.identity
            && self.provider_execution_report_fingerprint
                == installed.provider_execution.normalized_report_identity
            && self.completion_requirement_identity == self.resolution.requirement_identity
            && replayed_resolution == Some(&self.resolution)
            && root
                .candidate
                .service_reach
                .installation_requirements()
                .iter()
                .any(|requirement| requirement == &self.completion_requirement_identity)
            && self.entry_receipt == acknowledgement.invocation_evidence.entry_receipt
            && self.invocation == acknowledgement.invocation
            && self.policy == acknowledgement.policy
            && self.acknowledgement == acknowledgement.identity
            && acknowledgement.invocation_evidence.invocation == acknowledgement.invocation
            && acknowledgement.invocation_evidence.acknowledgement_policy
                == Some(acknowledgement.policy)
            && acknowledgement.invocation_evidence.acknowledgement == Some(acknowledgement.identity)
    }

    pub const fn entry_provider_plan(&self) -> ProviderPlanId {
        self.entry_provider_plan
    }

    pub const fn provider_execution(&self) -> ProviderExecutionId {
        self.provider_execution
    }

    pub fn completion_requirement_identity(&self) -> &str {
        &self.completion_requirement_identity
    }

    pub const fn resolution(&self) -> &effects::InstallationReachResolution {
        &self.resolution
    }

    pub const fn invocation(&self) -> InterruptInvocationId {
        self.invocation
    }

    pub const fn policy(&self) -> AcknowledgementPolicyId {
        self.policy
    }

    pub const fn acknowledgement(&self) -> InterruptAcknowledgementId {
        self.acknowledgement
    }
}

impl InterruptAcknowledgement {
    pub const fn identity(&self) -> InterruptAcknowledgementId {
        self.identity
    }

    /// Exact admitted source qualifications established for this concrete
    /// acknowledgement subject by the installed-root invocation receipt.
    pub fn qualifications(&self) -> &[AdmittedEntryQualification] {
        &self.qualifications
    }

    /// Resolve one exact static accepted-claim contract from this concrete
    /// linear occurrence. This never accepts a provider-plan receipt alone and
    /// never returns evidence detached from the acknowledgement carrier.
    pub fn qualification_for_contract(
        &self,
        provider_plan: ProviderPlanId,
        requirement_identity: &str,
        parameter_index: usize,
        domain: &str,
        effective_carry: language_semantics::CarryPolicy,
    ) -> Result<&AdmittedEntryQualification, ExternalRootDiagnostic> {
        let matches = self
            .qualifications
            .iter()
            .filter(|qualification| {
                qualification.matches_contract(
                    provider_plan,
                    requirement_identity,
                    parameter_index,
                    domain,
                    effective_carry,
                )
            })
            .collect::<Vec<_>>();
        let [qualification] = matches.as_slice() else {
            return Err(ExternalRootDiagnostic(format!(
                "interrupt acknowledgement maps to {} qualifications for the exact accepted entry contract",
                matches.len()
            )));
        };
        Ok(*qualification)
    }

    pub fn complete(
        self,
        receipt: InterruptAcknowledgementReceipt,
    ) -> Result<CompletedInterruptAcknowledgement, Box<InterruptAcknowledgementError>> {
        let matches = receipt.root == self.root
            && receipt.invocation_evidence == self.invocation_evidence
            && receipt.provider_execution == self.provider_execution
            && receipt.invocation == self.invocation
            && receipt.policy == self.policy
            && receipt.acknowledgement == self.identity
            && receipt.route.matches_acknowledgement(&self);
        if !matches {
            return Err(Box::new(InterruptAcknowledgementError {
                acknowledgement: self,
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt acknowledgement receipt does not complete the exact invocation and policy"
                        .into(),
                ),
            }));
        }
        Ok(CompletedInterruptAcknowledgement {
            invocation_evidence: self.invocation_evidence,
            root: self.root,
            provider_execution: self.provider_execution,
            invocation: self.invocation,
            policy: self.policy,
            acknowledgement: self.identity,
            receipt: receipt.identity,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InterruptAcknowledgementReceipt {
    identity: InterruptAcknowledgementReceiptId,
    invocation_evidence: InterruptInvocationEvidence,
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    policy: AcknowledgementPolicyId,
    acknowledgement: InterruptAcknowledgementId,
    pub(crate) route: InstalledInterruptCompletionRoute,
}

impl InterruptAcknowledgementReceipt {
    pub fn from_provider(
        identity: InterruptAcknowledgementReceiptId,
        acknowledgement: &InterruptAcknowledgement,
        completion_requirement_identity: &str,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let route = InstalledInterruptCompletionRoute::from_provider_requirement(
            acknowledgement,
            completion_requirement_identity,
        )?;
        Ok(Self {
            identity,
            invocation_evidence: acknowledgement.invocation_evidence.clone(),
            root: acknowledgement.root,
            provider_execution: acknowledgement.provider_execution,
            invocation: acknowledgement.invocation,
            policy: acknowledgement.policy,
            acknowledgement: acknowledgement.identity,
            route,
        })
    }

    pub const fn route(&self) -> &InstalledInterruptCompletionRoute {
        &self.route
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedInterruptAcknowledgement {
    invocation_evidence: InterruptInvocationEvidence,
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    invocation: InterruptInvocationId,
    policy: AcknowledgementPolicyId,
    acknowledgement: InterruptAcknowledgementId,
    receipt: InterruptAcknowledgementReceiptId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletedInterruptEntry {
    pub entry_receipt: InterruptEntryReceiptId,
    pub root: ExternalRootId,
    pub invocation: InterruptInvocationId,
    /// The admitted arrival context the completed invocation fired under.
    pub arrival_context: ArrivalContextId,
    /// The epoch stage the invocation settled at: the terminal stage its
    /// admitted arrival context realizes. `finish_interrupt_entry` admits
    /// settle only once the retained live stage rejoins that terminal stage,
    /// so the completed record proves no epoch was left unrealized.
    pub settled_stage: EntryStackStage,
    pub acknowledgement_receipt: Option<InterruptAcknowledgementReceiptId>,
    /// A settled fatal exception recorded the fault and ended ordinary work:
    /// its completion halted the ledger, so nothing the entry interrupted
    /// can resume.
    pub fatal: bool,
}

/// Whether the interrupted entry's retained arrival context permits one more
/// live occurrence above `parent_depth` at the reported epoch stage. Every
/// epoch of that context at the reported stage must carry a finite nestable
/// allowance beyond the parent's live depth: the ledger cannot observe which
/// epoch is live inside the stage, so a masked or unbounded sibling epoch is
/// an unresolved disposition. A stage the context does not realize rejects.
fn interrupted_epoch_permits_nesting(
    parent_input: &crate::BoundEpochStackCompositionInput,
    arrival_context: ArrivalContextId,
    interrupted_stage: EntryStackStage,
    parent_depth: u16,
) -> bool {
    let Some(context) = parent_input
        .realization_evidence()
        .realization()
        .realization()
        .contexts
        .iter()
        .find(|context| context.context == arrival_context)
    else {
        return false;
    };
    let stage_epochs = context
        .epochs
        .iter()
        .filter(|epoch| epoch.stage == interrupted_stage)
        .collect::<Vec<_>>();
    !stage_epochs.is_empty()
        && stage_epochs.iter().all(|epoch| {
            matches!(epoch.nesting, Preemption::Nestable { maximum_depth }
                if parent_depth < maximum_depth)
        })
}

impl InstalledRootLedger {
    /// Mint the opaque source obligations for one provider-reported interrupt
    /// invocation. Ordinary code has no constructor for these carriers; the
    /// receipt must match the exact installed root, selected execution, and
    /// acknowledgement policy. An invocation or acknowledgement identity can
    /// be admitted only once by a selected provider execution.
    ///
    /// The receipt's runtime context independently rejoins the retained bound
    /// epoch evidence: the reported arrival context must be one this exact
    /// installed root admitted with a resolved body-domain disposition, and a
    /// nested arrival must name the innermost live invocation under a declared
    /// nesting edge whose preempted epoch stage permits the depth.
    pub fn begin_interrupt_entry(
        &mut self,
        root: &InstalledExternalRoot<'_>,
        receipt: InterruptEntryReceipt,
    ) -> Result<InterruptEntryObligations, InterruptEntryStartError> {
        self.begin_interrupt_entry_as(root, receipt, false)
    }

    /// Begin one interrupt entry. `fatal` marks a fatal-exception arrival,
    /// which the published-table dispatch supplies from the member's
    /// declared obligation: the fault arrived on its dedicated critical
    /// stack whether or not the artifact declared a nesting edge, and its
    /// settle halts the ledger instead of resuming the interrupted chain.
    pub(crate) fn begin_interrupt_entry_as(
        &mut self,
        root: &InstalledExternalRoot<'_>,
        receipt: InterruptEntryReceipt,
        fatal: bool,
    ) -> Result<InterruptEntryObligations, InterruptEntryStartError> {
        if self.halted_by.is_some() {
            return Err(InterruptEntryStartError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt entry cannot arrive after a fatal exception settle halted the ledger"
                        .into(),
                ),
            });
        }
        let Some(record) = self.roots.get(&root.root) else {
            return Err(InterruptEntryStartError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt entry requires a currently installed external root".into(),
                ),
            });
        };
        let acknowledgement_shape_matches = match (
            record.acknowledgement_policy,
            receipt.acknowledgement_policy,
            receipt.acknowledgement,
        ) {
            (None, None, None) => true,
            (Some(expected), Some(actual), Some(_)) => expected == actual,
            _ => false,
        };
        let exact_root = root.slot == record.slot
            && root.installed_code.identity() == record.installed_code
            && self.root_evidence.get(&root.root).is_some_and(|evidence| {
                evidence == &root.evidence && evidence == &receipt.installed_root
            })
            && receipt.root == record.root
            && receipt.slot == record.slot
            && receipt.installed_code == record.installed_code
            && receipt.provider_execution == record.provider_execution
            && acknowledgement_shape_matches
            && (receipt.acknowledgement.is_none()
                || record.acknowledgement_parameter_index.is_some())
            && record.boundary.call.entry_control == EntryControl::InterruptReturn;
        if !exact_root {
            return Err(InterruptEntryStartError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt entry receipt does not bind the exact installed interrupt root, provider execution, and acknowledgement policy"
                        .into(),
                ),
            });
        }
        let entry_key = (record.provider_execution, receipt.invocation);
        let acknowledgement_key = receipt
            .acknowledgement
            .map(|identity| (record.provider_execution, identity));
        if self.entered_interrupts.contains(&entry_key)
            || acknowledgement_key.is_some_and(|key| self.minted_acknowledgements.contains(&key))
        {
            return Err(InterruptEntryStartError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt entry receipt replays an invocation or acknowledgement identity"
                        .into(),
                ),
            });
        }
        // The reported arrival context must be one this exact installed root's
        // bound epoch realization admitted, with its body-domain disposition
        // resolved in the retained closure. A context absent from the roster —
        // or realized only for a different root or installed occurrence — is
        // an unresolved or cross-context disposition.
        let admitted_context = record
            .stack
            .realization
            .input(record.root)
            .and_then(|input| {
                let evidence = input.realization_evidence();
                let context = evidence
                    .realization()
                    .realization()
                    .contexts
                    .iter()
                    .find(|context| context.context == receipt.arrival_context)?;
                (evidence.matches_installed_code_entry(root.installed_code, record.entry)
                    && evidence.body_domains.contexts().iter().any(|closed| {
                        closed.context == receipt.arrival_context
                            && closed.domain != StackDomainRef::ProviderSelected
                    }))
                .then_some(context)
            });
        let Some(admitted_context) = admitted_context else {
            return Err(InterruptEntryStartError {
                receipt,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt entry reports an arrival context outside the exact installed root's admitted epoch realization"
                        .into(),
                ),
            });
        };
        // A fresh activation realizes at the admitted context's first epoch
        // stage: Enter when the context realizes enter epochs, otherwise its
        // single Body. Later stages arrive only through the ledger's epoch
        // turn, so a preemption report cannot describe a stage the ledger
        // never admitted.
        let initial_stage = admitted_context
            .epochs
            .first()
            .expect("a validated arrival context realizes at least one epoch")
            .stage;
        // A nested arrival rejoins the artifact-wide nesting relation and the
        // finite depth bound the composition assumed: the reported parent must
        // be the innermost live invocation, the relation must declare the
        // edge, the reported stage must rejoin the stage the ledger retains
        // for that invocation, and every epoch of the parent's context at the
        // retained stage must permit one more live occurrence.
        let innermost = self
            .active_interrupts
            .iter()
            .max_by_key(|(_, entry)| entry.depth);
        let depth = match receipt.preemption {
            None => {
                if !self.active_interrupts.is_empty() {
                    return Err(InterruptEntryStartError {
                        receipt,
                        diagnostic: ExternalRootDiagnostic(
                            "interrupt entry arrived while interrupt invocations are live without naming the preempted invocation"
                                .into(),
                        ),
                    });
                }
                1
            }
            Some(report) => {
                let parent_key = (report.interrupted_root, report.interrupted_invocation);
                let Some(parent) = innermost
                    .filter(|(key, _)| **key == parent_key)
                    .map(|(_, entry)| *entry)
                else {
                    return Err(InterruptEntryStartError {
                        receipt,
                        diagnostic: ExternalRootDiagnostic(
                            "interrupt entry names a preempted invocation that is not the innermost live interrupt"
                                .into(),
                        ),
                    });
                };
                // A fatal entry preempts unconditionally: the processor
                // already faulted, and it arrives on its dedicated critical
                // stack, so no declared stack-nesting edge or parent depth
                // bound governs the arrival. The report still names the
                // innermost live invocation and rejoins its retained stage.
                let edge_declared = record
                    .stack
                    .realization
                    .relation()
                    .edges
                    .iter()
                    .any(|edge| {
                        edge.interrupted == report.interrupted_root && edge.preemptor == record.root
                    });
                if !fatal && !edge_declared {
                    return Err(InterruptEntryStartError {
                        receipt,
                        diagnostic: ExternalRootDiagnostic(
                            "nested interrupt entry preempts a live invocation without a declared stack-nesting edge"
                                .into(),
                        ),
                    });
                }
                // The reported preempted stage must rejoin the stage the
                // ledger itself retains for that invocation: the ledger
                // admitted every stage turn, so a report naming any other
                // stage is a stale or cross-context disposition regardless of
                // what the interrupted context would permit at it.
                if report.interrupted_stage != parent.stage {
                    return Err(InterruptEntryStartError {
                        receipt,
                        diagnostic: ExternalRootDiagnostic(format!(
                            "nested interrupt entry reports preempted epoch stage {:?} that does not rejoin the retained live stage {:?}",
                            report.interrupted_stage, parent.stage,
                        )),
                    });
                }
                let depth_permitted = self
                    .roots
                    .get(&report.interrupted_root)
                    .and_then(|parent_record| {
                        parent_record
                            .stack
                            .realization
                            .input(report.interrupted_root)
                    })
                    .is_some_and(|parent_input| {
                        interrupted_epoch_permits_nesting(
                            parent_input,
                            parent.arrival_context,
                            parent.stage,
                            parent.depth,
                        )
                    });
                if !fatal && !depth_permitted {
                    return Err(InterruptEntryStartError {
                        receipt,
                        diagnostic: ExternalRootDiagnostic(
                            "nested interrupt entry exceeds the finite nesting bound of the preempted invocation's retained epoch stage"
                                .into(),
                        ),
                    });
                }
                parent.depth + 1
            }
        };
        self.entered_interrupts.insert(entry_key);
        if let Some(key) = acknowledgement_key {
            self.minted_acknowledgements.insert(key);
        }
        self.active_interrupts.insert(
            (record.root, receipt.invocation),
            ActiveInterruptEntry {
                arrival_context: receipt.arrival_context,
                stage: initial_stage,
                depth,
                interrupted: receipt
                    .preemption
                    .map(|report| (report.interrupted_root, report.interrupted_invocation)),
                fatal,
            },
        );

        let invocation_evidence = InterruptInvocationEvidence::from_entry_receipt(&receipt);
        let acknowledgement = receipt.acknowledgement.map(|identity| {
            let parameter_index = record
                .acknowledgement_parameter_index
                .expect("exact interrupt root validated the acknowledgement parameter");
            let qualifications = record
                .entry_claims
                .iter()
                .filter(|claim| claim.parameter_index == parameter_index)
                .map(|claim| AdmittedEntryQualification {
                    provider_plan: record.provider_plan,
                    requirement_identity: record.requirement_identity.clone(),
                    parameter_index,
                    abi_placement: record.boundary.call.parameters[parameter_index].clone(),
                    domain: claim.domain.clone(),
                    effective_carry: claim.effective_carry,
                    entry_receipt: receipt.identity,
                    invocation: receipt.invocation,
                    subject: AdmittedEntrySubject::InterruptAcknowledgement(identity),
                })
                .collect::<Vec<_>>();
            InterruptAcknowledgement {
                invocation_evidence: invocation_evidence.clone(),
                identity,
                root: record.root,
                provider_execution: record.provider_execution,
                invocation: receipt.invocation,
                policy: record
                    .acknowledgement_policy
                    .expect("validated acknowledgement shape has a policy"),
                qualifications,
            }
        });
        Ok(InterruptEntryObligations {
            pending_exit: PendingInterruptExit {
                entry_receipt: receipt.identity,
                invocation_evidence: invocation_evidence.clone(),
                root: record.root,
                installed_code: record.installed_code,
                provider_execution: record.provider_execution,
                invocation: receipt.invocation,
                arrival_context: receipt.arrival_context,
                mask_control: receipt.mask_control,
                initial_mask_state: receipt.initial_mask_state,
                acknowledgement_policy: record.acknowledgement_policy,
                acknowledgement: receipt.acknowledgement,
            },
            mask_control: InterruptMaskControl {
                invocation_evidence,
                identity: receipt.mask_control,
                root: record.root,
                invocation: receipt.invocation,
                initial_state: receipt.initial_mask_state,
                current_state: receipt.initial_mask_state,
                live_guards: Vec::new(),
                used_guards: BTreeSet::new(),
                mask_guard_claim: record.interrupt_mask_guard_claim.clone(),
            },
            acknowledgement,
        })
    }

    /// Turn one live interrupt invocation's retained epoch stage to the next
    /// stage its admitted arrival context realizes.
    ///
    /// The ledger is the sole writer of the retained stage: a fresh
    /// activation realizes at its context's first epoch stage, and this edge
    /// is the only way a later stage becomes live. The named invocation must
    /// be the innermost live entry — a preempted invocation's epoch is frozen
    /// while its nested child runs — and the reported stage must be exactly
    /// the next stage in the context's canonical enter/body/exit order. A
    /// skipped, backward, or unrealized stage is an unresolved disposition;
    /// every rejection returns the report so the provider keeps its evidence
    /// custody.
    pub fn turn_interrupt_epoch_stage(
        &mut self,
        root: &InstalledExternalRoot<'_>,
        report: InterruptEpochTurnReport,
    ) -> Result<EntryStackStage, InterruptEpochTurnError> {
        let reject = |report: InterruptEpochTurnReport, diagnostic: ExternalRootDiagnostic| {
            Err(InterruptEpochTurnError { report, diagnostic })
        };
        if self.halted_by.is_some() {
            return reject(
                report,
                ExternalRootDiagnostic(
                    "interrupt epoch turn cannot advance after a fatal exception settle halted the ledger"
                        .into(),
                ),
            );
        }
        let Some(record) = self.roots.get(&root.root) else {
            return reject(
                report,
                ExternalRootDiagnostic(
                    "interrupt epoch turn requires a currently installed external root".into(),
                ),
            );
        };
        let exact_root = report.root == root.root
            && root.slot == record.slot
            && root.installed_code.identity() == record.installed_code
            && self
                .root_evidence
                .get(&root.root)
                .is_some_and(|evidence| evidence == &root.evidence);
        if !exact_root {
            return reject(
                report,
                ExternalRootDiagnostic(
                    "interrupt epoch turn does not bind the exact installed interrupt root".into(),
                ),
            );
        }
        let key = (record.root, report.invocation);
        let Some(entry) = self.active_interrupts.get(&key).copied() else {
            return reject(
                report,
                ExternalRootDiagnostic(
                    "interrupt epoch turn names an invocation that is not live".into(),
                ),
            );
        };
        // A preempted invocation's epoch is frozen while its nested child is
        // live: only the innermost entry can turn.
        let innermost = self
            .active_interrupts
            .iter()
            .max_by_key(|(_, active)| active.depth)
            .map(|(active_key, _)| *active_key);
        if innermost != Some(key) {
            return reject(
                report,
                ExternalRootDiagnostic(
                    "interrupt epoch turn names a preempted invocation whose nested entry is still live"
                        .into(),
                ),
            );
        }
        let Some(context) = record
            .stack
            .realization
            .input(record.root)
            .and_then(|input| {
                input
                    .realization_evidence()
                    .realization()
                    .realization()
                    .contexts
                    .iter()
                    .find(|context| context.context == entry.arrival_context)
            })
        else {
            return reject(
                report,
                ExternalRootDiagnostic(
                    "interrupt epoch turn cannot reconstruct the invocation's admitted arrival context"
                        .into(),
                ),
            );
        };
        // Canonical order keeps every stage's epochs contiguous, so the
        // distinct stage run is the admitted enter/body/exit progression.
        let mut stages = Vec::new();
        for epoch in &context.epochs {
            if stages.last() != Some(&epoch.stage) {
                stages.push(epoch.stage);
            }
        }
        let position = stages
            .iter()
            .position(|stage| *stage == entry.stage)
            .expect("the retained stage is always one the admitted context realizes");
        let Some(next) = stages.get(position + 1).copied() else {
            return reject(
                report,
                ExternalRootDiagnostic(format!(
                    "the admitted arrival context realizes no epoch stage beyond {:?}",
                    entry.stage,
                )),
            );
        };
        if next != report.turned_stage {
            return reject(
                report,
                ExternalRootDiagnostic(format!(
                    "interrupt epoch turn reports stage {:?} instead of the next realized stage {:?}",
                    report.turned_stage, next,
                )),
            );
        }
        self.active_interrupts
            .get_mut(&key)
            .expect("the innermost entry was found live above")
            .stage = report.turned_stage;
        Ok(report.turned_stage)
    }

    /// Admit the deriver-owned interrupt exit only after every source-visible
    /// obligation has returned to its exact provider state and the retained
    /// epoch stage has reached the terminal stage the invocation's admitted
    /// arrival context realizes.
    pub fn finish_interrupt_entry(
        &mut self,
        pending: PendingInterruptExit,
        control: InterruptMaskControl,
        acknowledgement: Option<CompletedInterruptAcknowledgement>,
    ) -> Result<CompletedInterruptEntry, Box<InterruptEntryFinishError>> {
        if self.halted_by.is_some() {
            return Err(Box::new(InterruptEntryFinishError {
                pending,
                control,
                acknowledgement,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt exit cannot resume an interrupted chain a fatal exception settle halted"
                        .into(),
                ),
            }));
        }
        let acknowledgement_matches = match (
            pending.acknowledgement_policy,
            pending.acknowledgement,
            acknowledgement.as_ref(),
        ) {
            (None, None, None) => true,
            (Some(policy), Some(identity), Some(completed)) => {
                completed.invocation_evidence == pending.invocation_evidence
                    && completed.root == pending.root
                    && completed.provider_execution == pending.provider_execution
                    && completed.invocation == pending.invocation
                    && completed.policy == policy
                    && completed.acknowledgement == identity
            }
            _ => false,
        };
        let record_matches = self.roots.get(&pending.root).is_some_and(|record| {
            record.installed_code == pending.installed_code
                && record.provider_execution == pending.provider_execution
                && record.acknowledgement_policy == pending.acknowledgement_policy
                && self
                    .root_evidence
                    .get(&pending.root)
                    .is_some_and(|evidence| evidence == &pending.invocation_evidence.installed_root)
        });
        let control_matches = control.invocation_evidence == pending.invocation_evidence
            && control.root == pending.root
            && control.invocation == pending.invocation
            && control.identity == pending.mask_control
            && control.initial_state == pending.initial_mask_state
            && control.current_state == pending.initial_mask_state
            && control.live_guards.is_empty();
        let active_key = (pending.root, pending.invocation);
        // A preempted invocation cannot settle while a nested entry is live:
        // the ledger admitted the child only as the innermost occurrence of
        // this exact invocation's chain.
        let live_nested_child = self
            .active_interrupts
            .values()
            .any(|entry| entry.interrupted == Some(active_key));
        if !record_matches
            || !control_matches
            || !acknowledgement_matches
            || live_nested_child
            || !self.active_interrupts.contains_key(&active_key)
        {
            return Err(Box::new(InterruptEntryFinishError {
                pending,
                control,
                acknowledgement,
                diagnostic: ExternalRootDiagnostic(
                    "interrupt exit requires the exact restored mask state and completed acknowledgement, with no nested invocation still live"
                        .into(),
                ),
            }));
        }
        let active_entry = self
            .active_interrupts
            .get(&active_key)
            .copied()
            .expect("the settle edge above requires the invocation to be live");
        // Settle rejoins the retained runtime context independently of the
        // obligation custody above: an invocation completes only at the
        // terminal epoch stage its admitted arrival context realizes. A
        // retained Enter or Body stage while the context realizes a later
        // stage leaves epochs unrealized — an unresolved disposition, not a
        // completed entry. The retained context, never a fresh provider
        // description, selects the epoch sequence, so the settle cannot
        // complete across a context boundary.
        let terminal_stage = self
            .roots
            .get(&pending.root)
            .and_then(|record| record.stack.realization.input(record.root))
            .and_then(|input| {
                input
                    .realization_evidence()
                    .realization()
                    .realization()
                    .contexts
                    .iter()
                    .find(|context| context.context == active_entry.arrival_context)
                    .and_then(|context| context.epochs.last())
            })
            .map(|epoch| epoch.stage);
        if terminal_stage != Some(active_entry.stage) {
            return Err(Box::new(InterruptEntryFinishError {
                pending,
                control,
                acknowledgement,
                diagnostic: ExternalRootDiagnostic(format!(
                    "interrupt exit retains epoch stage {:?} while the admitted arrival context realizes its terminal stage at {:?}",
                    active_entry.stage, terminal_stage,
                )),
            }));
        }
        self.active_interrupts.remove(&active_key);
        if active_entry.fatal {
            // The fault was recorded and ordinary work never resumes: every
            // still-live invocation stays held as halted evidence, so no
            // later entry, epoch turn, or settle can reach the interrupted
            // chain.
            self.halted_by = Some(active_key);
        }
        Ok(CompletedInterruptEntry {
            entry_receipt: pending.entry_receipt,
            root: pending.root,
            invocation: pending.invocation,
            arrival_context: pending.arrival_context,
            settled_stage: active_entry.stage,
            acknowledgement_receipt: acknowledgement.map(|completed| completed.receipt),
            fatal: active_entry.fatal,
        })
    }
}

/// Rejection of one reported interrupt epoch turn. The report returns so the
/// provider keeps its evidence custody and can correct or abandon the turn.
#[derive(Debug)]
pub struct InterruptEpochTurnError {
    report: InterruptEpochTurnReport,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptEpochTurnError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_report(self) -> InterruptEpochTurnReport {
        self.report
    }
}

#[derive(Debug)]
pub struct InterruptEntryStartError {
    receipt: InterruptEntryReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptEntryStartError {
    /// A dispatch-edge rejection that never reached the ledger's retained
    /// evidence — for example an arrival the published interrupt table does
    /// not arm. The receipt still comes back so the caller keeps its
    /// provider-evidence custody.
    pub(crate) fn unrouted(receipt: InterruptEntryReceipt, diagnostic: impl Into<String>) -> Self {
        Self {
            receipt,
            diagnostic: ExternalRootDiagnostic(diagnostic.into()),
        }
    }

    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_receipt(self) -> InterruptEntryReceipt {
        self.receipt
    }
}

#[derive(Debug)]
pub struct InterruptAcknowledgementError {
    acknowledgement: InterruptAcknowledgement,
    receipt: InterruptAcknowledgementReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptAcknowledgementError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InterruptAcknowledgement, InterruptAcknowledgementReceipt) {
        (self.acknowledgement, self.receipt)
    }
}

#[derive(Debug)]
pub struct InterruptEntryFinishError {
    pending: PendingInterruptExit,
    control: InterruptMaskControl,
    acknowledgement: Option<CompletedInterruptAcknowledgement>,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptEntryFinishError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PendingInterruptExit,
        InterruptMaskControl,
        Option<CompletedInterruptAcknowledgement>,
    ) {
        (self.pending, self.control, self.acknowledgement)
    }
}
