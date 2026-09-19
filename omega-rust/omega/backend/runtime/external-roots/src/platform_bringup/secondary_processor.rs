//! Secondary-processor startup custody over an installed low-memory trampoline.
//!
//! A startup trampoline is ordinary admitted executable content: the
//! executable-installation ladder already proved its exact bytes, frozen
//! placement, and instruction-fetch visibility before an `InstalledCode`
//! could exist. This module adds the secondary-processor entry contract on
//! top of that installed occurrence. It does not emit the trampoline, invoke
//! the target boot protocol itself, or model a general multiprocessor policy:
//! it binds the installed placement to one profile's low-memory bound,
//! startup-vector granularity, and arrival machine regime, then accounts each
//! admitted processor's dedicated stack class and private state extent
//! separately before any startup invocation may be issued.
//!
//! Custody flows in one direction. `bind_secondary_processor_trampoline`
//! replays the retained installation evidence — nothing here learns an
//! address the installer did not already validate. `admit_secondary_processor`
//! joins the AP entry's validated boundary plan to a per-processor account,
//! minting the arrival-to-installed regime transition only when both ends
//! match the profile. `begin_secondary_processor_startup` issues the exact
//! vector carrier the target protocol consumes, and
//! `complete_secondary_processor_startup` accepts only a receipt naming that
//! exact carrier, judged on a three-way verdict: definite nondispatch
//! returns the account to pending custody so a fresh invocation can retry,
//! a confirmed arrival keeps the started processor's stack and state account
//! held, and an unconfirmed dispatch leaves the account invoked — neither
//! withdrawable nor reissuable — until a later definitive receipt answers
//! the outstanding carrier. `retire_secondary_processor` is the quiescence edge that
//! releases that hold: it consumes the started evidence together with a
//! provider quiescence receipt naming it exactly, and returns the complete
//! account only when the provider attests the processor no longer executes
//! on the accounted stack or private state. An incomplete drain keeps the
//! account held and hands the started evidence back for a later attempt.
//! Withdrawal is permitted only for a processor that has never been invoked,
//! returning its state extent and account intact. Neither edge retires the
//! trampoline: the ledger's borrow keeps the installed code unretirable.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::{EntryStack, MachineRegime, ValidatedBoundaryEntryPlan};
use executable_installation::{ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId};
use extents::Extent;
use layout_plans::{EntryStubId, MachineRegimeId};

use crate::{
    ExternalRootDiagnostic, SecondaryProcessorOccurrenceId, SecondaryProcessorQuiescenceReceiptId,
    SecondaryProcessorStartupInvocationId, SecondaryProcessorStartupProfileId,
    SecondaryProcessorStartupReceiptId,
};

/// Normalized compiler- or target-authored plan for one secondary-processor
/// startup protocol. The profile pins the admitted trampoline entry, the CPU
/// arrival regime the placement must carry, the installed regime the AP
/// entry's boundary must begin in, and the low-memory/alignment geometry the
/// realized placement must satisfy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryProcessorStartupProfile {
    identity: SecondaryProcessorStartupProfileId,
    startup_entry: EntryStubId,
    arrival_regime: MachineRegimeId,
    installed_regime: MachineRegime,
    low_memory_limit: u64,
    startup_alignment: u64,
}

impl SecondaryProcessorStartupProfile {
    pub fn new(
        identity: SecondaryProcessorStartupProfileId,
        startup_entry: EntryStubId,
        arrival_regime: MachineRegimeId,
        installed_regime: MachineRegime,
        low_memory_limit: u64,
        startup_alignment: u64,
    ) -> Result<Self, ExternalRootDiagnostic> {
        if !startup_alignment.is_power_of_two() {
            return Err(ExternalRootDiagnostic(
                "secondary-processor startup alignment must be a nonzero power of two".into(),
            ));
        }
        if low_memory_limit == 0 || !low_memory_limit.is_multiple_of(startup_alignment) {
            return Err(ExternalRootDiagnostic(
                "secondary-processor low-memory bound must be a nonzero multiple of the startup alignment"
                    .into(),
            ));
        }
        Ok(Self {
            identity,
            startup_entry,
            arrival_regime,
            installed_regime,
            low_memory_limit,
            startup_alignment,
        })
    }

    pub const fn identity(&self) -> SecondaryProcessorStartupProfileId {
        self.identity
    }

    pub const fn startup_entry(&self) -> EntryStubId {
        self.startup_entry
    }

    pub const fn arrival_regime(&self) -> MachineRegimeId {
        self.arrival_regime
    }

    pub const fn installed_regime(&self) -> MachineRegime {
        self.installed_regime
    }

    /// Exclusive upper bound on the trampoline's realized placement extent.
    pub const fn low_memory_limit(&self) -> u64 {
        self.low_memory_limit
    }

    /// Address granularity of the startup vector the target protocol
    /// consumes; the realized placement base must be a multiple of it.
    pub const fn startup_alignment(&self) -> u64 {
        self.startup_alignment
    }
}

/// Verified installed-trampoline custody borrowed for one startup ledger.
///
/// Construction replays the complete retained installation evidence: the AP
/// entry is admitted by the artifact, the realized placement geometry is the
/// claimed low-memory window at the startup alignment, and the declared
/// placement constraints carry the profile's arrival regime. The borrow keeps
/// the installed trampoline unretirable while any AP entry remains possible.
#[derive(Debug)]
pub struct InstalledSecondaryProcessorTrampoline<'code> {
    installed_code: &'code InstalledCode,
    profile: SecondaryProcessorStartupProfile,
    startup_vector: u64,
    extent_base: u64,
    extent_length: u64,
}

impl InstalledSecondaryProcessorTrampoline<'_> {
    pub const fn profile(&self) -> &SecondaryProcessorStartupProfile {
        &self.profile
    }

    /// Startup vector the target protocol consumes: the verified placement
    /// base divided by the startup granularity. It is sealed evidence replayed
    /// from installed-code custody, never an independently supplied address.
    pub const fn startup_vector(&self) -> u64 {
        self.startup_vector
    }

    pub const fn extent_base(&self) -> u64 {
        self.extent_base
    }

    pub const fn extent_length(&self) -> u64 {
        self.extent_length
    }

    pub const fn installed_code(&self) -> &'static InstalledCode
    where
        Self: 'static,
    {
        self.installed_code
    }
}

/// Bind an installed trampoline to one startup profile.
///
/// `claimed_base`/`claimed_length` restate the realized placement geometry
/// the installer already chose; they are checked against retained
/// installed-code evidence, so a foreign or drifted claim rejects. The
/// returned vector is derived only from that verified geometry.
pub fn bind_secondary_processor_trampoline<'code>(
    profile: SecondaryProcessorStartupProfile,
    installed_code: &'code InstalledCode,
    claimed_base: u64,
    claimed_length: u64,
) -> Result<InstalledSecondaryProcessorTrampoline<'code>, Box<SecondaryProcessorBindError>> {
    let reject = |diagnostic: &str| {
        Err(Box::new(SecondaryProcessorBindError {
            profile: profile.clone(),
            diagnostic: ExternalRootDiagnostic(diagnostic.into()),
        }))
    };

    if installed_code
        .selected_entry_target(profile.startup_entry)
        .is_err()
    {
        return reject("installed trampoline does not admit the profile's startup entry");
    }
    if profile.installed_regime.architecture() != installed_code.architecture() {
        return reject(
            "profile's installed machine regime does not run on the installed trampoline's architecture",
        );
    }
    if !installed_code.binds_placement_geometry(claimed_base, claimed_length) {
        return reject(
            "claimed placement geometry does not match the exact installed trampoline extent",
        );
    }
    if !claimed_base.is_multiple_of(profile.startup_alignment) {
        return reject(
            "installed trampoline base is not aligned to the startup-vector granularity",
        );
    }
    if claimed_base
        .checked_add(claimed_length)
        .is_none_or(|end| end > profile.low_memory_limit)
    {
        return reject("installed trampoline extent exceeds the profile's low-memory bound");
    }

    let constraints = installed_code.placement_constraints();
    if constraints.machine_regime() != Some(profile.arrival_regime) {
        return reject(
            "installed trampoline placement does not carry the profile's arrival machine regime",
        );
    }
    if !constraints
        .alignment()
        .is_multiple_of(profile.startup_alignment)
    {
        return reject(
            "installed trampoline declared alignment is not a multiple of the startup-vector granularity",
        );
    }
    if constraints
        .permitted_range()
        .is_some_and(|range| range.end_exclusive() > profile.low_memory_limit)
    {
        return reject(
            "installed trampoline declared placement range exceeds the profile's low-memory bound",
        );
    }

    Ok(InstalledSecondaryProcessorTrampoline {
        installed_code,
        startup_vector: claimed_base / profile.startup_alignment,
        extent_base: claimed_base,
        extent_length: claimed_length,
        profile,
    })
}

#[derive(Debug)]
pub struct SecondaryProcessorBindError {
    profile: SecondaryProcessorStartupProfile,
    diagnostic: ExternalRootDiagnostic,
}

impl SecondaryProcessorBindError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_profile(self) -> SecondaryProcessorStartupProfile {
        self.profile
    }
}

/// One secondary processor's separately accounted entry resources: a
/// dedicated stack class with its worst-case usage demand and the private
/// extent backing that stack and per-CPU state. Construction checks only
/// account-local shape; the ledger owns cross-account custody checks.
#[derive(Debug)]
pub struct SecondaryProcessorAccount {
    processor: SecondaryProcessorOccurrenceId,
    boundary: ValidatedBoundaryEntryPlan,
    stack_class: u16,
    wcsu_bytes: u64,
    wcsu_alignment: u64,
    state: Extent,
}

impl SecondaryProcessorAccount {
    pub fn new(
        processor: SecondaryProcessorOccurrenceId,
        boundary: ValidatedBoundaryEntryPlan,
        stack_class: u16,
        wcsu_bytes: u64,
        wcsu_alignment: u64,
        state: Extent,
    ) -> Result<Self, Box<SecondaryProcessorAccountError>> {
        let reject = |diagnostic: &str, state: Extent| {
            Err(Box::new(SecondaryProcessorAccountError {
                processor,
                boundary: boundary.clone(),
                stack_class,
                wcsu_bytes,
                wcsu_alignment,
                state,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };
        if wcsu_bytes == 0 || !wcsu_alignment.is_power_of_two() {
            return reject(
                "per-processor stack account requires nonzero demand and a power-of-two alignment",
                state,
            );
        }
        if state.length() < wcsu_bytes {
            return reject(
                "per-processor state extent is smaller than the accounted stack demand",
                state,
            );
        }
        if !state.base().is_multiple_of(wcsu_alignment) {
            return reject(
                "per-processor state extent does not satisfy the stack alignment",
                state,
            );
        }
        Ok(Self {
            processor,
            boundary,
            stack_class,
            wcsu_bytes,
            wcsu_alignment,
            state,
        })
    }

    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn stack_class(&self) -> u16 {
        self.stack_class
    }

    pub const fn wcsu_bytes(&self) -> u64 {
        self.wcsu_bytes
    }

    pub const fn wcsu_alignment(&self) -> u64 {
        self.wcsu_alignment
    }
}

#[derive(Debug)]
pub struct SecondaryProcessorAccountError {
    processor: SecondaryProcessorOccurrenceId,
    boundary: ValidatedBoundaryEntryPlan,
    stack_class: u16,
    wcsu_bytes: u64,
    wcsu_alignment: u64,
    state: Extent,
    diagnostic: ExternalRootDiagnostic,
}

impl SecondaryProcessorAccountError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        SecondaryProcessorOccurrenceId,
        ValidatedBoundaryEntryPlan,
        u16,
        u64,
        u64,
        Extent,
    ) {
        (
            self.processor,
            self.boundary,
            self.stack_class,
            self.wcsu_bytes,
            self.wcsu_alignment,
            self.state,
        )
    }
}

/// Minted evidence that one admitted processor's arrival regime reaches the
/// installed AP entry's validated initial regime. `installed_regime` is
/// replayed from the boundary plan, not restated by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondaryProcessorRegimeTransition {
    processor: SecondaryProcessorOccurrenceId,
    arrival_regime: MachineRegimeId,
    installed_regime: MachineRegime,
}

impl SecondaryProcessorRegimeTransition {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn arrival_regime(&self) -> MachineRegimeId {
        self.arrival_regime
    }

    pub const fn installed_regime(&self) -> MachineRegime {
        self.installed_regime
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecondaryProcessorPhase {
    Pending,
    Invoked {
        invocation: SecondaryProcessorStartupInvocationId,
    },
    Started {
        invocation: SecondaryProcessorStartupInvocationId,
        receipt: SecondaryProcessorStartupReceiptId,
    },
}

/// Retained per-processor row. The private state extent stays in custody, so
/// two processors can never be accounted onto one backing.
#[derive(Debug)]
pub struct SecondaryProcessorRecord {
    processor: SecondaryProcessorOccurrenceId,
    boundary: ValidatedBoundaryEntryPlan,
    stack_class: u16,
    wcsu_bytes: u64,
    wcsu_alignment: u64,
    state: Extent,
    transition: SecondaryProcessorRegimeTransition,
    phase: SecondaryProcessorPhase,
}

impl SecondaryProcessorRecord {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn stack_class(&self) -> u16 {
        self.stack_class
    }

    pub const fn wcsu_bytes(&self) -> u64 {
        self.wcsu_bytes
    }

    pub const fn wcsu_alignment(&self) -> u64 {
        self.wcsu_alignment
    }

    pub const fn transition(&self) -> SecondaryProcessorRegimeTransition {
        self.transition
    }

    /// Whether this processor completed its startup invocation. A started
    /// processor's account stays held: its stack and state remain occupied
    /// until `retire_secondary_processor` consumes a quiescence receipt.
    pub fn is_started(&self) -> bool {
        matches!(self.phase, SecondaryProcessorPhase::Started { .. })
    }
}

/// Exact carrier handed to the target boot protocol for one startup attempt.
/// The vector, entry, regime transition, and installed-code context are bound
/// at issuance; a completion receipt is only meaningful against this carrier.
#[derive(Debug, PartialEq, Eq)]
pub struct SecondaryProcessorStartupInvocation {
    invocation: SecondaryProcessorStartupInvocationId,
    processor: SecondaryProcessorOccurrenceId,
    startup_vector: u64,
    startup_entry: EntryStubId,
    transition: SecondaryProcessorRegimeTransition,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
}

impl SecondaryProcessorStartupInvocation {
    pub const fn invocation(&self) -> SecondaryProcessorStartupInvocationId {
        self.invocation
    }

    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn startup_vector(&self) -> u64 {
        self.startup_vector
    }

    pub const fn startup_entry(&self) -> EntryStubId {
        self.startup_entry
    }

    pub const fn transition(&self) -> SecondaryProcessorRegimeTransition {
        self.transition
    }
}

/// The provider's verdict on one issued startup carrier. The vector may
/// still be executing below the provider's visibility, so an ambiguous
/// `did it start` answer is not a refusal: only a definite nondispatch
/// returns the account to pending custody. An unconfirmed dispatch keeps
/// the account invoked and held — the carrier remains outstanding and only
/// a later definitive receipt against it resolves the attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryProcessorStartupVerdict {
    /// The processor definitely did not dispatch on the carrier's vector.
    DefiniteNondispatch,
    /// The provider cannot tell whether the vector dispatched; the
    /// outstanding carrier may still arrive.
    DispatchUnconfirmed,
    /// The processor is confirmed executing on the carrier's vector.
    ConfirmedArrival,
}

/// Provider result for one issued startup invocation. Only the ledger can
/// consume it: the receipt must name the exact carrier it answers.
#[derive(Debug, PartialEq, Eq)]
pub struct SecondaryProcessorStartupReceipt {
    identity: SecondaryProcessorStartupReceiptId,
    invocation: SecondaryProcessorStartupInvocationId,
    processor: SecondaryProcessorOccurrenceId,
    startup_vector: u64,
    verdict: SecondaryProcessorStartupVerdict,
}

impl SecondaryProcessorStartupReceipt {
    pub fn from_provider(
        identity: SecondaryProcessorStartupReceiptId,
        carrier: &SecondaryProcessorStartupInvocation,
        verdict: SecondaryProcessorStartupVerdict,
    ) -> Self {
        Self {
            identity,
            invocation: carrier.invocation,
            processor: carrier.processor,
            startup_vector: carrier.startup_vector,
            verdict,
        }
    }

    pub const fn identity(&self) -> SecondaryProcessorStartupReceiptId {
        self.identity
    }

    pub const fn verdict(&self) -> SecondaryProcessorStartupVerdict {
        self.verdict
    }
}

/// Evidence that one processor completed its startup invocation on the
/// verified vector and regime transition. It binds the installed-code
/// identity, context, and artifact the carrier was issued under, so the
/// quiescence edge can tell this ledger's started processor from a foreign
/// ledger's. It is the only input that can open retirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryProcessorStarted {
    processor: SecondaryProcessorOccurrenceId,
    invocation: SecondaryProcessorStartupInvocationId,
    receipt: SecondaryProcessorStartupReceiptId,
    startup_vector: u64,
    startup_entry: EntryStubId,
    transition: SecondaryProcessorRegimeTransition,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
}

impl SecondaryProcessorStarted {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn invocation(&self) -> SecondaryProcessorStartupInvocationId {
        self.invocation
    }

    pub const fn receipt(&self) -> SecondaryProcessorStartupReceiptId {
        self.receipt
    }

    pub const fn startup_vector(&self) -> u64 {
        self.startup_vector
    }

    pub const fn startup_entry(&self) -> EntryStubId {
        self.startup_entry
    }

    pub const fn transition(&self) -> SecondaryProcessorRegimeTransition {
        self.transition
    }
}

/// A definitely-undispatched startup attempt. The processor account
/// remains admitted: the retained stack class and state extent were never
/// consumed, so a fresh invocation may retry without re-binding the
/// trampoline. Only `DefiniteNondispatch` mints this; an unconfirmed
/// dispatch keeps the account invoked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryProcessorStartupRefusal {
    processor: SecondaryProcessorOccurrenceId,
    invocation: SecondaryProcessorStartupInvocationId,
    receipt: SecondaryProcessorStartupReceiptId,
}

impl SecondaryProcessorStartupRefusal {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn receipt(&self) -> SecondaryProcessorStartupReceiptId {
        self.receipt
    }
}

/// An unresolved startup attempt. The provider could not confirm whether
/// the vector dispatched, so the account stays invoked and held — it cannot
/// withdraw or accept a fresh invocation — and the carrier returns so a
/// later definitive receipt can still resolve it.
#[derive(Debug, PartialEq, Eq)]
pub struct SecondaryProcessorStartupUnconfirmed {
    carrier: SecondaryProcessorStartupInvocation,
    receipt: SecondaryProcessorStartupReceiptId,
}

impl SecondaryProcessorStartupUnconfirmed {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.carrier.processor
    }

    pub const fn invocation(&self) -> SecondaryProcessorStartupInvocationId {
        self.carrier.invocation
    }

    pub const fn receipt(&self) -> SecondaryProcessorStartupReceiptId {
        self.receipt
    }

    /// The outstanding carrier, for a later definitive receipt to answer.
    pub fn into_carrier(self) -> SecondaryProcessorStartupInvocation {
        self.carrier
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SecondaryProcessorStartupOutcome {
    Started(SecondaryProcessorStarted),
    Refused(SecondaryProcessorStartupRefusal),
    Unconfirmed(SecondaryProcessorStartupUnconfirmed),
}

/// Provider attestation answering one started processor's retirement. The
/// provider owns how the processor is halted or parked; the ledger only
/// replays that the receipt names the exact started evidence and, when
/// `quiescent`, that no execution still reaches the account's dedicated
/// stack or private state.
#[derive(Debug, PartialEq, Eq)]
pub struct SecondaryProcessorQuiescenceReceipt {
    identity: SecondaryProcessorQuiescenceReceiptId,
    processor: SecondaryProcessorOccurrenceId,
    invocation: SecondaryProcessorStartupInvocationId,
    startup_receipt: SecondaryProcessorStartupReceiptId,
    startup_vector: u64,
    quiescent: bool,
}

impl SecondaryProcessorQuiescenceReceipt {
    pub fn from_provider(
        identity: SecondaryProcessorQuiescenceReceiptId,
        started: &SecondaryProcessorStarted,
        quiescent: bool,
    ) -> Self {
        Self {
            identity,
            processor: started.processor,
            invocation: started.invocation,
            startup_receipt: started.receipt,
            startup_vector: started.startup_vector,
            quiescent,
        }
    }

    pub const fn identity(&self) -> SecondaryProcessorQuiescenceReceiptId {
        self.identity
    }

    pub const fn quiescent(&self) -> bool {
        self.quiescent
    }
}

/// Returned custody for a retired processor: the complete account plus its
/// state extent, bound to the startup and quiescence evidence that closed
/// it, so nothing provisioned for the AP is dropped silently.
#[derive(Debug)]
pub struct SecondaryProcessorRetirement {
    processor: SecondaryProcessorOccurrenceId,
    invocation: SecondaryProcessorStartupInvocationId,
    startup_receipt: SecondaryProcessorStartupReceiptId,
    quiescence_receipt: SecondaryProcessorQuiescenceReceiptId,
    boundary: ValidatedBoundaryEntryPlan,
    stack_class: u16,
    wcsu_bytes: u64,
    wcsu_alignment: u64,
    state: Extent,
    transition: SecondaryProcessorRegimeTransition,
}

impl SecondaryProcessorRetirement {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn invocation(&self) -> SecondaryProcessorStartupInvocationId {
        self.invocation
    }

    pub const fn startup_receipt(&self) -> SecondaryProcessorStartupReceiptId {
        self.startup_receipt
    }

    pub const fn quiescence_receipt(&self) -> SecondaryProcessorQuiescenceReceiptId {
        self.quiescence_receipt
    }

    pub fn into_parts(
        self,
    ) -> (
        ValidatedBoundaryEntryPlan,
        u16,
        u64,
        u64,
        Extent,
        SecondaryProcessorRegimeTransition,
    ) {
        (
            self.boundary,
            self.stack_class,
            self.wcsu_bytes,
            self.wcsu_alignment,
            self.state,
            self.transition,
        )
    }
}

/// An incomplete drain. The provider accepted the retirement request but
/// could not attest quiescence, so the account stays held exactly as it was
/// and the started evidence returns for a later attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryProcessorQuiescenceRefusal {
    started: SecondaryProcessorStarted,
    receipt: SecondaryProcessorQuiescenceReceiptId,
}

impl SecondaryProcessorQuiescenceRefusal {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.started.processor
    }

    pub const fn receipt(&self) -> SecondaryProcessorQuiescenceReceiptId {
        self.receipt
    }

    pub fn into_started(self) -> SecondaryProcessorStarted {
        self.started
    }
}

#[derive(Debug)]
pub enum SecondaryProcessorQuiescenceOutcome {
    Retired(SecondaryProcessorRetirement),
    Held(SecondaryProcessorQuiescenceRefusal),
}

/// Returned custody for a never-invoked processor: the complete account plus
/// its state extent, so nothing provisioned for the AP is dropped silently.
#[derive(Debug)]
pub struct SecondaryProcessorWithdrawal {
    processor: SecondaryProcessorOccurrenceId,
    boundary: ValidatedBoundaryEntryPlan,
    stack_class: u16,
    wcsu_bytes: u64,
    wcsu_alignment: u64,
    state: Extent,
    transition: SecondaryProcessorRegimeTransition,
}

impl SecondaryProcessorWithdrawal {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub fn into_parts(
        self,
    ) -> (
        ValidatedBoundaryEntryPlan,
        u16,
        u64,
        u64,
        Extent,
        SecondaryProcessorRegimeTransition,
    ) {
        (
            self.boundary,
            self.stack_class,
            self.wcsu_bytes,
            self.wcsu_alignment,
            self.state,
            self.transition,
        )
    }
}

/// Owner of one installed trampoline's secondary-processor entry ledger.
///
/// The trampoline borrow keeps the installed code unretirable while any
/// account or invocation is live; the ledger never mints vectors, regimes, or
/// per-CPU resources — it replays and retains them.
#[derive(Debug)]
pub struct SecondaryProcessorStartupLedger<'code> {
    trampoline: InstalledSecondaryProcessorTrampoline<'code>,
    records: BTreeMap<SecondaryProcessorOccurrenceId, SecondaryProcessorRecord>,
    issued_invocations: BTreeSet<SecondaryProcessorStartupInvocationId>,
}

impl<'code> SecondaryProcessorStartupLedger<'code> {
    pub fn new(trampoline: InstalledSecondaryProcessorTrampoline<'code>) -> Self {
        Self {
            trampoline,
            records: BTreeMap::new(),
            issued_invocations: BTreeSet::new(),
        }
    }

    pub const fn trampoline(&self) -> &InstalledSecondaryProcessorTrampoline<'code> {
        &self.trampoline
    }

    pub fn record(
        &self,
        processor: SecondaryProcessorOccurrenceId,
    ) -> Option<&SecondaryProcessorRecord> {
        self.records.get(&processor)
    }

    pub fn records(&self) -> impl Iterator<Item = &SecondaryProcessorRecord> {
        self.records.values()
    }

    /// Admit one processor's separately accounted stack and state.
    ///
    /// The entry boundary must begin in the profile's installed regime and
    /// arrive on the account's own dedicated stack class: a fresh processor
    /// has no interrupted stack to borrow, and a provider-selected stack
    /// would not be accounted per CPU. Cross-account custody rejects shared
    /// classes and overlapping state backing.
    pub fn admit_secondary_processor(
        &mut self,
        account: SecondaryProcessorAccount,
    ) -> Result<&SecondaryProcessorRecord, Box<SecondaryProcessorAdmissionError>> {
        let reject = |diagnostic: &str, account: SecondaryProcessorAccount| {
            Err(Box::new(SecondaryProcessorAdmissionError {
                account,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };

        let profile = &self.trampoline.profile;
        if account.boundary.plan().state.initial_regime != profile.installed_regime {
            return reject(
                "secondary-processor entry does not begin in the profile's installed machine regime",
                account,
            );
        }
        let arrives_on_accounted_stack = match account.boundary.plan().state.stack {
            EntryStack::Dedicated { class } => class == account.stack_class,
            EntryStack::Interrupted | EntryStack::ProviderSelected => false,
        };
        if !arrives_on_accounted_stack {
            return reject(
                "secondary-processor entry must arrive on the account's own dedicated stack class",
                account,
            );
        }
        if self.records.contains_key(&account.processor) {
            return reject(
                "secondary-processor occurrence is already admitted",
                account,
            );
        }
        if self
            .records
            .values()
            .any(|record| record.stack_class == account.stack_class)
        {
            return reject(
                "dedicated stack class is already accounted to another processor",
                account,
            );
        }
        let state_end = account
            .state
            .base()
            .checked_add(account.state.length())
            .expect("extent geometry was validated non-wrapping");
        let overlaps_held_state = self.records.values().any(|record| {
            record.state.address_space() == account.state.address_space()
                && record.state.base() < state_end
                && account.state.base() < record.state.end()
        });
        if overlaps_held_state {
            return reject(
                "per-processor state extent overlaps another admitted processor's backing",
                account,
            );
        }

        let transition = SecondaryProcessorRegimeTransition {
            processor: account.processor,
            arrival_regime: profile.arrival_regime,
            installed_regime: account.boundary.plan().state.initial_regime,
        };
        let processor = account.processor;
        self.records.insert(
            processor,
            SecondaryProcessorRecord {
                processor: account.processor,
                boundary: account.boundary,
                stack_class: account.stack_class,
                wcsu_bytes: account.wcsu_bytes,
                wcsu_alignment: account.wcsu_alignment,
                state: account.state,
                transition,
                phase: SecondaryProcessorPhase::Pending,
            },
        );
        Ok(self
            .records
            .get(&processor)
            .expect("admitted secondary-processor record remains retained"))
    }

    /// Issue the exact startup carrier the target boot protocol consumes for
    /// one admitted, never-invoked processor.
    pub fn begin_secondary_processor_startup(
        &mut self,
        processor: SecondaryProcessorOccurrenceId,
        invocation: SecondaryProcessorStartupInvocationId,
    ) -> Result<SecondaryProcessorStartupInvocation, SecondaryProcessorStartError> {
        let reject = |diagnostic: &str| {
            Err(SecondaryProcessorStartError {
                processor,
                invocation,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            })
        };
        if self.issued_invocations.contains(&invocation) {
            return reject("secondary-processor startup invocation was already issued");
        }
        let Some(record) = self.records.get_mut(&processor) else {
            return reject("secondary-processor occurrence is not admitted");
        };
        if record.phase != SecondaryProcessorPhase::Pending {
            return reject("secondary-processor startup requires a pending, never-invoked account");
        }
        record.phase = SecondaryProcessorPhase::Invoked { invocation };
        self.issued_invocations.insert(invocation);
        Ok(SecondaryProcessorStartupInvocation {
            invocation,
            processor,
            startup_vector: self.trampoline.startup_vector,
            startup_entry: self.trampoline.profile.startup_entry,
            transition: record.transition,
            installed_code: self.trampoline.installed_code.identity(),
            installed_code_context: self.trampoline.installed_code.receipt_context(),
            artifact: self.trampoline.installed_code.artifact(),
        })
    }

    /// Accept the provider receipt answering one exact issued carrier. A
    /// definite nondispatch returns the account to pending custody; a
    /// confirmed arrival marks the processor running and keeps its account
    /// held; an unconfirmed dispatch leaves the account invoked and returns
    /// the carrier for a later definitive answer.
    pub fn complete_secondary_processor_startup(
        &mut self,
        carrier: SecondaryProcessorStartupInvocation,
        receipt: SecondaryProcessorStartupReceipt,
    ) -> Result<SecondaryProcessorStartupOutcome, Box<SecondaryProcessorCompletionError>> {
        let reject = |diagnostic: &str,
                      carrier: SecondaryProcessorStartupInvocation,
                      receipt: SecondaryProcessorStartupReceipt| {
            Err(Box::new(SecondaryProcessorCompletionError {
                carrier,
                receipt,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };

        let exact_carrier = carrier.installed_code == self.trampoline.installed_code.identity()
            && carrier.installed_code_context == self.trampoline.installed_code.receipt_context()
            && carrier.artifact == self.trampoline.installed_code.artifact()
            && carrier.startup_vector == self.trampoline.startup_vector
            && carrier.startup_entry == self.trampoline.profile.startup_entry;
        if !exact_carrier {
            return reject(
                "secondary-processor startup carrier is foreign, stale, or drifted",
                carrier,
                receipt,
            );
        }
        let Some(record) = self.records.get(&carrier.processor) else {
            return reject(
                "secondary-processor startup carrier names no admitted processor",
                carrier,
                receipt,
            );
        };
        if !matches!(record.phase, SecondaryProcessorPhase::Invoked { .. }) {
            return reject(
                "secondary-processor startup completion is reordered or replayed",
                carrier,
                receipt,
            );
        }
        let exact_receipt = receipt.invocation == carrier.invocation
            && receipt.processor == carrier.processor
            && receipt.startup_vector == carrier.startup_vector
            && receipt.processor == record.processor;
        if !exact_receipt {
            return reject(
                "secondary-processor startup receipt does not bind the exact issued carrier",
                carrier,
                receipt,
            );
        }

        let record = self
            .records
            .get_mut(&carrier.processor)
            .expect("validated secondary-processor record remains retained");
        match receipt.verdict {
            SecondaryProcessorStartupVerdict::ConfirmedArrival => {
                record.phase = SecondaryProcessorPhase::Started {
                    invocation: carrier.invocation,
                    receipt: receipt.identity,
                };
                Ok(SecondaryProcessorStartupOutcome::Started(
                    SecondaryProcessorStarted {
                        processor: record.processor,
                        invocation: carrier.invocation,
                        receipt: receipt.identity,
                        startup_vector: carrier.startup_vector,
                        startup_entry: carrier.startup_entry,
                        transition: record.transition,
                        installed_code: carrier.installed_code,
                        installed_code_context: carrier.installed_code_context,
                        artifact: carrier.artifact,
                    },
                ))
            }
            SecondaryProcessorStartupVerdict::DispatchUnconfirmed => {
                // The dispatch may still be in flight: the account stays
                // invoked, so it is neither withdrawable nor reissuable, and
                // the carrier returns so a later definitive receipt against
                // it resolves the same outstanding attempt.
                Ok(SecondaryProcessorStartupOutcome::Unconfirmed(
                    SecondaryProcessorStartupUnconfirmed {
                        carrier,
                        receipt: receipt.identity,
                    },
                ))
            }
            SecondaryProcessorStartupVerdict::DefiniteNondispatch => {
                record.phase = SecondaryProcessorPhase::Pending;
                Ok(SecondaryProcessorStartupOutcome::Refused(
                    SecondaryProcessorStartupRefusal {
                        processor: record.processor,
                        invocation: carrier.invocation,
                        receipt: receipt.identity,
                    },
                ))
            }
        }
    }

    /// Withdraw one never-invoked processor, returning its complete account
    /// and state extent. Invoked and started processors keep their account:
    /// an outstanding invocation may still land, and a started processor's
    /// stack cannot be reclaimed without a later quiescence edge.
    pub fn withdraw_secondary_processor(
        &mut self,
        processor: SecondaryProcessorOccurrenceId,
    ) -> Result<SecondaryProcessorWithdrawal, SecondaryProcessorWithdrawError> {
        let reject = |diagnostic: &str| {
            Err(SecondaryProcessorWithdrawError {
                processor,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            })
        };
        let Some(record) = self.records.get(&processor) else {
            return reject("secondary-processor occurrence is not admitted");
        };
        if record.phase != SecondaryProcessorPhase::Pending {
            return reject(
                "secondary-processor withdrawal requires a pending, never-invoked account",
            );
        }
        let record = self
            .records
            .remove(&processor)
            .expect("validated secondary-processor record remains retained");
        Ok(SecondaryProcessorWithdrawal {
            processor: record.processor,
            boundary: record.boundary,
            stack_class: record.stack_class,
            wcsu_bytes: record.wcsu_bytes,
            wcsu_alignment: record.wcsu_alignment,
            state: record.state,
            transition: record.transition,
        })
    }

    /// Retire one started processor on a provider quiescence receipt,
    /// returning its complete account and state extent.
    ///
    /// The started evidence must be this ledger's own — same installed
    /// occurrence, vector, and entry — and must name the account's current
    /// startup: a pending or invoked account has nothing to retire, and
    /// evidence from an earlier startup of a re-admitted processor is stale.
    /// The receipt must bind that exact started evidence. Every check runs
    /// before any ledger state changes; a rejection returns both inputs. A
    /// receipt that does not attest quiescence leaves the account held and
    /// hands the started evidence back.
    pub fn retire_secondary_processor(
        &mut self,
        started: SecondaryProcessorStarted,
        receipt: SecondaryProcessorQuiescenceReceipt,
    ) -> Result<SecondaryProcessorQuiescenceOutcome, Box<SecondaryProcessorRetirementError>> {
        let reject = |diagnostic: &str,
                      started: SecondaryProcessorStarted,
                      receipt: SecondaryProcessorQuiescenceReceipt| {
            Err(Box::new(SecondaryProcessorRetirementError {
                started,
                receipt,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };

        let exact_started = started.installed_code == self.trampoline.installed_code.identity()
            && started.installed_code_context == self.trampoline.installed_code.receipt_context()
            && started.artifact == self.trampoline.installed_code.artifact()
            && started.startup_vector == self.trampoline.startup_vector
            && started.startup_entry == self.trampoline.profile.startup_entry;
        if !exact_started {
            return reject(
                "secondary-processor started evidence is foreign, stale, or drifted",
                started,
                receipt,
            );
        }
        let Some(record) = self.records.get(&started.processor) else {
            return reject(
                "secondary-processor started evidence names no admitted processor",
                started,
                receipt,
            );
        };
        let SecondaryProcessorPhase::Started {
            invocation,
            receipt: startup_receipt,
        } = record.phase
        else {
            return reject(
                "secondary-processor retirement requires a started account",
                started,
                receipt,
            );
        };
        let names_current_startup = invocation == started.invocation
            && startup_receipt == started.receipt
            && record.transition == started.transition;
        if !names_current_startup {
            return reject(
                "secondary-processor started evidence does not name the account's current startup",
                started,
                receipt,
            );
        }
        let exact_receipt = receipt.processor == started.processor
            && receipt.invocation == started.invocation
            && receipt.startup_receipt == started.receipt
            && receipt.startup_vector == started.startup_vector;
        if !exact_receipt {
            return reject(
                "secondary-processor quiescence receipt does not bind the exact started evidence",
                started,
                receipt,
            );
        }

        if !receipt.quiescent {
            return Ok(SecondaryProcessorQuiescenceOutcome::Held(
                SecondaryProcessorQuiescenceRefusal {
                    started,
                    receipt: receipt.identity,
                },
            ));
        }
        let record = self
            .records
            .remove(&started.processor)
            .expect("validated secondary-processor record remains retained");
        Ok(SecondaryProcessorQuiescenceOutcome::Retired(
            SecondaryProcessorRetirement {
                processor: record.processor,
                invocation: started.invocation,
                startup_receipt: started.receipt,
                quiescence_receipt: receipt.identity,
                boundary: record.boundary,
                stack_class: record.stack_class,
                wcsu_bytes: record.wcsu_bytes,
                wcsu_alignment: record.wcsu_alignment,
                state: record.state,
                transition: record.transition,
            },
        ))
    }
}

#[derive(Debug)]
pub struct SecondaryProcessorAdmissionError {
    account: SecondaryProcessorAccount,
    diagnostic: ExternalRootDiagnostic,
}

impl SecondaryProcessorAdmissionError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_account(self) -> SecondaryProcessorAccount {
        self.account
    }
}

#[derive(Debug)]
pub struct SecondaryProcessorStartError {
    processor: SecondaryProcessorOccurrenceId,
    invocation: SecondaryProcessorStartupInvocationId,
    diagnostic: ExternalRootDiagnostic,
}

impl SecondaryProcessorStartError {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn invocation(&self) -> SecondaryProcessorStartupInvocationId {
        self.invocation
    }

    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
}

#[derive(Debug)]
pub struct SecondaryProcessorCompletionError {
    carrier: SecondaryProcessorStartupInvocation,
    receipt: SecondaryProcessorStartupReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl SecondaryProcessorCompletionError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        SecondaryProcessorStartupInvocation,
        SecondaryProcessorStartupReceipt,
    ) {
        (self.carrier, self.receipt)
    }
}

#[derive(Debug)]
pub struct SecondaryProcessorWithdrawError {
    processor: SecondaryProcessorOccurrenceId,
    diagnostic: ExternalRootDiagnostic,
}

impl SecondaryProcessorWithdrawError {
    pub const fn processor(&self) -> SecondaryProcessorOccurrenceId {
        self.processor
    }

    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }
}

#[derive(Debug)]
pub struct SecondaryProcessorRetirementError {
    started: SecondaryProcessorStarted,
    receipt: SecondaryProcessorQuiescenceReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl SecondaryProcessorRetirementError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        SecondaryProcessorStarted,
        SecondaryProcessorQuiescenceReceipt,
    ) {
        (self.started, self.receipt)
    }
}

#[cfg(test)]
mod tests;
