//! Installed terminal-entry stack binding and artifact-wide WCSU composition.
//!
//! This module owns exact local stack evidence, nesting relations, cycle and
//! dedicated-stack reentry rejection, transitive peak composition, and stable
//! composition fingerprints. It does not admit external roots, provision
//! runtime storage, or execute providers.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::{EntryStack, ValidatedBoundaryEntryPlan, ValidatedX86_64DeriverStub};
use executable_installation::{ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId};
use installation_evidence::{ObjectEvidence, StackDemandEvidence};
use layout_plans::EntryStubId;

use crate::identities::Fnv1a;
use crate::root_entry::root_admission::bind_terminal_function;
use crate::{
    BoundEpochStackComposition, ExternalRootDiagnostic, ExternalRootId, NestingRelationId,
    RootProviderId, StackValidationReceiptId,
};

/// A terminal-Psi stack closure bound to the exact installed bytes and entry
/// stub selected for one external root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledEntryStackDemand {
    psi: terminal_psi::TerminalPsiIdentity,
    architecture: target::Architecture,
    machine_entry: semantic_vocabulary::MachineId,
    ceiling_bytes: u64,
    stack_alignment: u64,
    contributing_machines: BTreeSet<semantic_vocabulary::MachineId>,
    admitted_stack_contribution_report_identities: BTreeSet<u64>,
    admitted_stack_contribution_commitments: BTreeSet<[u8; 32]>,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
}

impl InstalledEntryStackDemand {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    pub const fn machine_entry(&self) -> semantic_vocabulary::MachineId {
        self.machine_entry
    }

    pub const fn ceiling_bytes(&self) -> u64 {
        self.ceiling_bytes
    }

    pub const fn stack_alignment(&self) -> u64 {
        self.stack_alignment
    }

    pub const fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId> {
        &self.contributing_machines
    }

    pub const fn admitted_stack_contribution_report_identities(&self) -> &BTreeSet<u64> {
        &self.admitted_stack_contribution_report_identities
    }

    pub const fn admitted_stack_contribution_commitments(&self) -> &BTreeSet<[u8; 32]> {
        &self.admitted_stack_contribution_commitments
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub(super) fn matches_installed_entry(
        &self,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> bool {
        self.entry == entry
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

/// Bind one emitter-derived terminal stack closure to exact installed bytes
/// and the selected external entry stub.
pub fn bind_installed_entry_stack<
    TerminalArtifact: ObjectEvidence,
    StackDemand: StackDemandEvidence,
>(
    demand: &StackDemand,
    artifact: &TerminalArtifact,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<InstalledEntryStackDemand, ExternalRootDiagnostic> {
    if demand.psi() != artifact.psi() {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand does not name the terminal artifact's semantic identity".into(),
        ));
    }
    if demand.architecture() != artifact.architecture() {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand target does not match the terminal artifact architecture".into(),
        ));
    }
    if demand.ceiling_bytes() == 0
        || demand.stack_alignment() == 0
        || !demand.stack_alignment().is_power_of_two()
    {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand requires nonzero bytes and power-of-two alignment".into(),
        ));
    }
    let admitted_stack_contribution_report_identities =
        demand.admitted_stack_contribution_report_identities();
    let admitted_stack_contribution_commitments = demand.admitted_stack_contribution_commitments();
    if admitted_stack_contribution_report_identities.contains(&0)
        || admitted_stack_contribution_commitments.contains(&[0; 32])
        || admitted_stack_contribution_report_identities.is_empty()
            != admitted_stack_contribution_commitments.is_empty()
    {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand has incomplete or zero admitted same-stack provenance".into(),
        ));
    }
    let function_offset = artifact
        .function_text_offset(demand.entry())
        .ok_or_else(|| {
            ExternalRootDiagnostic(
                "terminal stack-demand entry is not present in the emitted artifact".into(),
            )
        })?;
    bind_terminal_function(artifact, installed_code, entry, function_offset)?;
    Ok(InstalledEntryStackDemand {
        psi: demand.psi(),
        architecture: demand.architecture(),
        machine_entry: demand.entry(),
        ceiling_bytes: demand.ceiling_bytes(),
        stack_alignment: u64::from(demand.stack_alignment()),
        contributing_machines: demand.contributing_machines().clone(),
        admitted_stack_contribution_report_identities,
        admitted_stack_contribution_commitments,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
    })
}

pub fn validate_installed_entry_stack(
    binding: &InstalledEntryStackDemand,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<(), ExternalRootDiagnostic> {
    if !binding.matches_installed_entry(installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand does not bind the selected installed code and entry".into(),
        ));
    }
    Ok(())
}

/// Untrusted emission coordinates for one installed deriver-owned entry stub.
///
/// The row names where the emitted stub bytes sit, where the member's text
/// sits, and where the sealed member `call rel32` field lies inside the stub
/// bytes. `bind_installed_deriver_stub_entry_stack` replays the sealed
/// call-target equation and the exact installed entry bytes — it does not
/// re-decode the byte recipe, which stays machine-emission's authority.
#[derive(Debug, Clone, Copy)]
pub struct X86_64DeriverStubEntryEmission<'a> {
    /// The entry interval's exact materialized bytes, member-call field
    /// sealed.
    pub resolved_bytes: &'a [u8],
    /// The member machine the stub's member call transfers to.
    pub member: semantic_vocabulary::MachineId,
    /// The stub's byte offset inside the installed text section — the
    /// contract's `identity.entry_offset`.
    pub stub_section_offset: u64,
    /// The member's byte offset inside the same section — the artifact's
    /// certified terminal function offset.
    pub member_section_offset: u64,
    /// Byte offset of the `call` opcode inside `resolved_bytes`.
    pub call_opcode_byte_offset: u16,
    /// Byte offset of the sealed rel32 field inside `resolved_bytes`.
    pub call_field_byte_offset: u16,
    /// Byte offset of the instruction following the call inside
    /// `resolved_bytes`.
    pub next_instruction_byte_offset: u16,
}

/// A member's terminal stack closure bound through its deriver-owned
/// installed entry: the member's own demand at the member's own text offset
/// plus the stub's deriver-owned frame between the delivered hardware frame
/// and the member call. Retains the contract fingerprint, the member's
/// section offset, and the sealed call displacement as audit coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledDeriverStubEntryStackDemand {
    psi: terminal_psi::TerminalPsiIdentity,
    architecture: target::Architecture,
    machine_entry: semantic_vocabulary::MachineId,
    member_ceiling_bytes: u64,
    stub_overhead_bytes: u64,
    ceiling_bytes: u64,
    stack_alignment: u64,
    contributing_machines: BTreeSet<semantic_vocabulary::MachineId>,
    admitted_stack_contribution_report_identities: BTreeSet<u64>,
    admitted_stack_contribution_commitments: BTreeSet<[u8; 32]>,
    stub_contract_report_fingerprint: u64,
    member_section_offset: u64,
    member_call_displacement: i32,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
}

impl InstalledDeriverStubEntryStackDemand {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    pub const fn machine_entry(&self) -> semantic_vocabulary::MachineId {
        self.machine_entry
    }

    /// The entry's total worst-case stack usage: the member's certified
    /// ceiling plus the deriver-owned stub overhead.
    pub const fn ceiling_bytes(&self) -> u64 {
        self.ceiling_bytes
    }

    /// The member's own certified stack ceiling, before stub overhead.
    pub const fn member_ceiling_bytes(&self) -> u64 {
        self.member_ceiling_bytes
    }

    /// Worst-case bytes the emitted entry/exit stub adds below the delivered
    /// hardware frame, as carried by the stub contract.
    pub const fn stub_overhead_bytes(&self) -> u64 {
        self.stub_overhead_bytes
    }

    pub const fn stack_alignment(&self) -> u64 {
        self.stack_alignment
    }

    pub const fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId> {
        &self.contributing_machines
    }

    pub const fn admitted_stack_contribution_report_identities(&self) -> &BTreeSet<u64> {
        &self.admitted_stack_contribution_report_identities
    }

    pub const fn admitted_stack_contribution_commitments(&self) -> &BTreeSet<[u8; 32]> {
        &self.admitted_stack_contribution_commitments
    }

    pub const fn stub_contract_report_fingerprint(&self) -> u64 {
        self.stub_contract_report_fingerprint
    }

    pub const fn member_section_offset(&self) -> u64 {
        self.member_section_offset
    }

    pub const fn member_call_displacement(&self) -> i32 {
        self.member_call_displacement
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub(super) fn matches_installed_entry(
        &self,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> bool {
        self.entry == entry
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

/// The member call is the emission recipe's `call rel32` — opcode 0xe8. The
/// binder replays the sealed displacement equation rather than trusting the
/// emission row's claimed member.
const MEMBER_CALL_OPCODE: u8 = 0xe8;

/// The stack alignment the stub's `and rsp, -16` normalization establishes
/// below the delivered hardware frame; the composed demand cannot assume
/// better than this from the interrupted stack pointer.
const STUB_ENTRY_ALIGNMENT: u64 = 16;

/// Bind one member's terminal stack demand through the deriver-owned entry
/// stub that occupies the selected installed entry.
///
/// Unlike `bind_installed_entry_stack`, the installed entry interval does not
/// hold the member's bytes: it holds the emitted stub, and the member's own
/// text sits at the artifact's certified function offset. The binding checks
/// the contract's exact installed-entry identity and admitted boundary plan,
/// the exact resolved entry bytes, the artifact's retained bytes, and replays
/// the sealed `call rel32` target equation from the stub's section offset to
/// the member's — so the returned demand cannot name a different member than
/// the stub actually transfers to.
pub fn bind_installed_deriver_stub_entry_stack<
    TerminalArtifact: ObjectEvidence,
    StackDemand: StackDemandEvidence,
>(
    demand: &StackDemand,
    artifact: &TerminalArtifact,
    stub: &ValidatedX86_64DeriverStub,
    emission: X86_64DeriverStubEntryEmission<'_>,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<InstalledDeriverStubEntryStackDemand, ExternalRootDiagnostic> {
    if demand.psi() != artifact.psi() {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand does not name the terminal artifact's semantic identity".into(),
        ));
    }
    if demand.architecture() != artifact.architecture() {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand target does not match the terminal artifact architecture".into(),
        ));
    }
    if demand.ceiling_bytes() == 0
        || demand.stack_alignment() == 0
        || !demand.stack_alignment().is_power_of_two()
    {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand requires nonzero bytes and power-of-two alignment".into(),
        ));
    }
    let admitted_stack_contribution_report_identities =
        demand.admitted_stack_contribution_report_identities();
    let admitted_stack_contribution_commitments = demand.admitted_stack_contribution_commitments();
    if admitted_stack_contribution_report_identities.contains(&0)
        || admitted_stack_contribution_commitments.contains(&[0; 32])
        || admitted_stack_contribution_report_identities.is_empty()
            != admitted_stack_contribution_commitments.is_empty()
    {
        return Err(ExternalRootDiagnostic(
            "terminal stack demand has incomplete or zero admitted same-stack provenance".into(),
        ));
    }
    if artifact.architecture() != installed_code.architecture()
        || installed_code.architecture() != target::Architecture::X86_64
    {
        return Err(ExternalRootDiagnostic(
            "a deriver-owned x86-64 entry stub requires an x86-64 terminal artifact and installed executable".into(),
        ));
    }
    if !installed_code.binds_exact_unrelocated_artifact_bytes(artifact.text_bytes()) {
        return Err(ExternalRootDiagnostic(
            "installed executable does not retain the exact relocation-free terminal artifact bytes"
                .into(),
        ));
    }
    let member_offset = u64::try_from(artifact.function_text_offset(demand.entry()).ok_or_else(
        || {
            ExternalRootDiagnostic(
                "terminal stack-demand entry is not present in the emitted artifact".into(),
            )
        },
    )?)
    .map_err(|_| {
        ExternalRootDiagnostic(
            "terminal function offset cannot be represented by installation metadata".into(),
        )
    })?;
    let contract = stub.stub();
    if contract.identity.artifact != installed_code.artifact().normalized_identity()
        || contract.identity.installed_code != installed_code.identity().normalized_identity()
        || contract.identity.entry != entry.normalized_identity()
        || contract.identity.boundary_plan_report_fingerprint
            != boundary.contract_report_fingerprint()
        || contract.identity.boundary_plan_commitment != boundary.contract_commitment_digest()
    {
        return Err(ExternalRootDiagnostic(
            "deriver stub contract does not name this exact installed entry and admitted boundary plan"
                .into(),
        ));
    }
    if !installed_code.binds_entry_offset(entry, contract.identity.entry_offset) {
        return Err(ExternalRootDiagnostic(
            "selected installed entry does not name the deriver stub's bound offset".into(),
        ));
    }
    if emission.member != demand.entry() {
        return Err(ExternalRootDiagnostic(
            "deriver stub member call does not name the stack-demand entry machine".into(),
        ));
    }
    if emission.member_section_offset != member_offset {
        return Err(ExternalRootDiagnostic(
            "deriver stub member section offset does not name the certified terminal function offset"
                .into(),
        ));
    }
    if emission.stub_section_offset != contract.identity.entry_offset {
        return Err(ExternalRootDiagnostic(
            "deriver stub emission row does not name the contract's entry offset".into(),
        ));
    }
    if !installed_code.binds_exact_materialized_entry_bytes(entry, emission.resolved_bytes) {
        return Err(ExternalRootDiagnostic(
            "installed entry interval does not hold the exact resolved deriver stub bytes".into(),
        ));
    }
    let opcode_offset = usize::from(emission.call_opcode_byte_offset);
    let field_offset = usize::from(emission.call_field_byte_offset);
    let field_end = field_offset.checked_add(4).ok_or_else(|| {
        ExternalRootDiagnostic("deriver stub member-call field offset overflowed".into())
    })?;
    if field_end != usize::from(emission.next_instruction_byte_offset) {
        return Err(ExternalRootDiagnostic(
            "deriver stub member-call field does not end at the next instruction".into(),
        ));
    }
    if emission.resolved_bytes.get(opcode_offset) != Some(&MEMBER_CALL_OPCODE) {
        return Err(ExternalRootDiagnostic(
            "deriver stub member call is not the recipe's `call rel32` opcode".into(),
        ));
    }
    let field = emission
        .resolved_bytes
        .get(field_offset..field_end)
        .ok_or_else(|| {
            ExternalRootDiagnostic(
                "deriver stub member-call field lies outside the entry interval".into(),
            )
        })?;
    let displacement = i32::from_le_bytes(field.try_into().expect("4-byte field"));
    let call_next = emission
        .stub_section_offset
        .checked_add(u64::from(emission.next_instruction_byte_offset))
        .ok_or_else(|| {
            ExternalRootDiagnostic("deriver stub member-call next offset overflowed".into())
        })?;
    if i128::from(call_next) + i128::from(displacement)
        != i128::from(emission.member_section_offset)
    {
        return Err(ExternalRootDiagnostic(
            "sealed member call does not resolve to the member's section offset".into(),
        ));
    }
    let stub_overhead_bytes = contract.peak_entry_overhead_bytes();
    let ceiling_bytes = demand
        .ceiling_bytes()
        .checked_add(stub_overhead_bytes)
        .ok_or_else(|| {
            ExternalRootDiagnostic("deriver stub entry stack ceiling overflowed".into())
        })?;
    // The member's demand is measured from the normalized stack the stub
    // establishes: the entry's composed alignment is the member's declared
    // alignment bounded below by the stub's own established alignment.
    let stack_alignment = u64::from(demand.stack_alignment()).max(STUB_ENTRY_ALIGNMENT);
    Ok(InstalledDeriverStubEntryStackDemand {
        psi: demand.psi(),
        architecture: demand.architecture(),
        machine_entry: demand.entry(),
        member_ceiling_bytes: demand.ceiling_bytes(),
        stub_overhead_bytes,
        ceiling_bytes,
        stack_alignment,
        contributing_machines: demand.contributing_machines().clone(),
        admitted_stack_contribution_report_identities,
        admitted_stack_contribution_commitments,
        stub_contract_report_fingerprint: stub.report_fingerprint(),
        member_section_offset: member_offset,
        member_call_displacement: displacement,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
    })
}

pub fn validate_installed_deriver_stub_entry_stack(
    binding: &InstalledDeriverStubEntryStackDemand,
    installed_code: &InstalledCode,
    entry: EntryStubId,
) -> Result<(), ExternalRootDiagnostic> {
    if !binding.matches_installed_entry(installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "deriver stub stack demand does not bind the selected installed code and entry".into(),
        ));
    }
    Ok(())
}

/// Exact evidence for one provider's local stack demand. Checked terminal
/// code contributes a byte- and entry-bound closure — either a member bound
/// directly at its own installed entry or one reached through the emitted
/// deriver stub that occupies the entry; an opaque provider must instead
/// retain its explicit admission receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackLocalEvidence {
    TerminalEntry(InstalledEntryStackDemand),
    /// A terminal body bound through the deriver-owned entry stub that
    /// occupies the selected installed entry.
    DeriverStubEntry(InstalledDeriverStubEntryStackDemand),
    AdmittedProvider {
        local_wcsu_bytes: u64,
        wcsu_alignment: u64,
        validation_receipt: StackValidationReceiptId,
    },
}

impl StackLocalEvidence {
    pub const fn local_wcsu_bytes(&self) -> u64 {
        match self {
            Self::TerminalEntry(binding) => binding.ceiling_bytes,
            Self::DeriverStubEntry(binding) => binding.ceiling_bytes,
            Self::AdmittedProvider {
                local_wcsu_bytes, ..
            } => *local_wcsu_bytes,
        }
    }

    pub const fn wcsu_alignment(&self) -> u64 {
        match self {
            Self::TerminalEntry(binding) => binding.stack_alignment,
            Self::DeriverStubEntry(binding) => binding.stack_alignment,
            Self::AdmittedProvider { wcsu_alignment, .. } => *wcsu_alignment,
        }
    }

    pub const fn provider_validation_receipt(&self) -> Option<StackValidationReceiptId> {
        match self {
            Self::TerminalEntry(_) | Self::DeriverStubEntry(_) => None,
            Self::AdmittedProvider {
                validation_receipt, ..
            } => Some(*validation_receipt),
        }
    }

    /// Whether the terminal-body evidence binds the exact selected installed
    /// code and entry. `None` for an admitted-provider row, which has no
    /// entry binding to check.
    pub(super) fn terminal_body_matches_installed_entry(
        &self,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> Option<bool> {
        match self {
            Self::TerminalEntry(binding) => {
                Some(binding.matches_installed_entry(installed_code, entry))
            }
            Self::DeriverStubEntry(binding) => {
                Some(binding.matches_installed_entry(installed_code, entry))
            }
            Self::AdmittedProvider { .. } => None,
        }
    }
}

/// One provider's validated local stack demand for an external entry.
///
/// `stack` is copied from the entry's normalized `StatePlan`; composition and
/// final root admission verify that it has not drifted from that source fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderStackSummary {
    pub root: ExternalRootId,
    pub provider: RootProviderId,
    pub stack: EntryStack,
    pub local_evidence: StackLocalEvidence,
}

impl ProviderStackSummary {
    pub fn from_admitted_provider(
        root: ExternalRootId,
        provider: RootProviderId,
        stack: EntryStack,
        local_wcsu_bytes: u64,
        wcsu_alignment: u64,
        validation_receipt: StackValidationReceiptId,
    ) -> Self {
        Self {
            root,
            provider,
            stack,
            local_evidence: StackLocalEvidence::AdmittedProvider {
                local_wcsu_bytes,
                wcsu_alignment,
                validation_receipt,
            },
        }
    }

    pub fn from_entry(
        root: ExternalRootId,
        provider: RootProviderId,
        stack: EntryStack,
        demand: InstalledEntryStackDemand,
    ) -> Self {
        Self {
            root,
            provider,
            stack,
            local_evidence: StackLocalEvidence::TerminalEntry(demand),
        }
    }

    pub fn from_deriver_stub_entry(
        root: ExternalRootId,
        provider: RootProviderId,
        stack: EntryStack,
        demand: InstalledDeriverStubEntryStackDemand,
    ) -> Self {
        Self {
            root,
            provider,
            stack,
            local_evidence: StackLocalEvidence::DeriverStubEntry(demand),
        }
    }

    pub const fn local_wcsu_bytes(&self) -> u64 {
        self.local_evidence.local_wcsu_bytes()
    }

    pub const fn wcsu_alignment(&self) -> u64 {
        self.local_evidence.wcsu_alignment()
    }
}

/// One possible asynchronous preemption in an artifact-wide nesting relation.
/// `preemptor` may enter while `interrupted` is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StackNestingEdge {
    pub interrupted: ExternalRootId,
    pub preemptor: ExternalRootId,
}

/// Exact architecture/provider nesting graph consumed by stack composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackNestingRelation {
    pub identity: NestingRelationId,
    pub edges: BTreeSet<StackNestingEdge>,
}

/// Exact canonical inputs retained behind every stack-composition result.
///
/// Compact fingerprints remain useful report keys, but are not admission
/// evidence: two distinct nesting graphs or provider summaries must remain
/// distinguishable even if their compact fingerprints collide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StackCompositionEvidence {
    relation: StackNestingRelation,
    pub(super) summaries: BTreeMap<ExternalRootId, ProviderStackSummary>,
}

/// Provisioning domain produced from the one normalized `EntryStack` fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StackDomain {
    Interrupted,
    Dedicated { class: u16 },
    ProviderSelected,
}

/// Canonical transitive stack result for one external root. Private fields
/// prevent an unaudited caller-authored composed WCSU from entering the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedStackDemand {
    root: ExternalRootId,
    root_provider: RootProviderId,
    relation: NestingRelationId,
    stack: EntryStack,
    local_wcsu_bytes: u64,
    pub(crate) composed_wcsu_bytes: u64,
    wcsu_alignment: u64,
    pub(crate) contributing_roots: BTreeSet<ExternalRootId>,
    validation_receipts: BTreeSet<StackValidationReceiptId>,
    pub(super) composition_evidence: StackCompositionEvidence,
    pub(crate) non_authoritative_artifact_composition_report_fingerprint: u64,
    pub(crate) non_authoritative_composition_report_fingerprint: u64,
}

impl ComposedStackDemand {
    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn root_provider(&self) -> RootProviderId {
        self.root_provider
    }

    pub const fn relation(&self) -> NestingRelationId {
        self.relation
    }

    pub const fn stack(&self) -> EntryStack {
        self.stack
    }

    pub const fn local_wcsu_bytes(&self) -> u64 {
        self.local_wcsu_bytes
    }

    pub const fn composed_wcsu_bytes(&self) -> u64 {
        self.composed_wcsu_bytes
    }

    pub const fn wcsu_alignment(&self) -> u64 {
        self.wcsu_alignment
    }

    pub const fn contributing_roots(&self) -> &BTreeSet<ExternalRootId> {
        &self.contributing_roots
    }

    pub const fn validation_receipts(&self) -> &BTreeSet<StackValidationReceiptId> {
        &self.validation_receipts
    }

    pub const fn non_authoritative_composition_report_fingerprint(&self) -> u64 {
        self.non_authoritative_composition_report_fingerprint
    }

    pub const fn non_authoritative_artifact_composition_report_fingerprint(&self) -> u64 {
        self.non_authoritative_artifact_composition_report_fingerprint
    }

    pub fn summary_evidence(
        &self,
    ) -> impl Iterator<Item = (&ExternalRootId, &ProviderStackSummary)> {
        self.composition_evidence.summaries.iter()
    }
}

/// Canonical artifact-wide WCSU result. Per-domain provisioning takes the
/// maximum of roots that begin in that domain; sequential entries reuse the
/// same storage instead of being summed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStackComposition {
    relation: NestingRelationId,
    demands: BTreeMap<ExternalRootId, ComposedStackDemand>,
    domain_wcsu_bytes: BTreeMap<StackDomain, u64>,
    domain_alignments: BTreeMap<StackDomain, u64>,
    non_authoritative_composition_report_fingerprint: u64,
}

impl ArtifactStackComposition {
    pub const fn relation(&self) -> NestingRelationId {
        self.relation
    }

    pub fn demand(&self, root: ExternalRootId) -> Option<&ComposedStackDemand> {
        self.demands.get(&root)
    }

    pub fn domain_wcsu_bytes(&self, domain: StackDomain) -> Option<u64> {
        self.domain_wcsu_bytes.get(&domain).copied()
    }

    pub fn domain_alignment(&self, domain: StackDomain) -> Option<u64> {
        self.domain_alignments.get(&domain).copied()
    }

    pub const fn non_authoritative_composition_report_fingerprint(&self) -> u64 {
        self.non_authoritative_composition_report_fingerprint
    }
}

/// Stack provisioning admitted for one external root. The stack domain itself
/// remains the single value in `BoundaryEntryPlan::state.stack`; this column
/// adds a ceiling and the sealed artifact-wide composition that refines it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackResourceColumn {
    pub ceiling_bytes: u64,
    pub realization: BoundEpochStackComposition,
    pub validation_receipt: StackValidationReceiptId,
}

/// Compose every provider stack summary under one exact artifact-wide nesting
/// relation. Interrupted-stack entries add (with alignment) to the active
/// domain. Dedicated entries switch domains. Re-entering a dedicated class
/// that is already active is rejected because provisioning cannot make such a
/// reset-style stack switch preserve the suspended frames.
pub fn compose_artifact_stacks<'a>(
    relation: &StackNestingRelation,
    summaries: impl IntoIterator<Item = &'a ProviderStackSummary>,
) -> Result<ArtifactStackComposition, ExternalRootDiagnostic> {
    let mut by_root = BTreeMap::new();
    for summary in summaries {
        if summary.local_wcsu_bytes() == 0 {
            return Err(ExternalRootDiagnostic(format!(
                "provider stack summary for root 0x{:016x} has zero local WCSU",
                summary.root.normalized_identity()
            )));
        }
        if summary.wcsu_alignment() == 0 || !summary.wcsu_alignment().is_power_of_two() {
            return Err(ExternalRootDiagnostic(format!(
                "provider stack summary for root 0x{:016x} has alignment {} instead of a nonzero power of two",
                summary.root.normalized_identity(),
                summary.wcsu_alignment()
            )));
        }
        if by_root.insert(summary.root, summary).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "provider stack summary for root 0x{:016x} is duplicated",
                summary.root.normalized_identity()
            )));
        }
    }
    if by_root.is_empty() {
        return Err(ExternalRootDiagnostic(
            "artifact stack composition requires at least one provider summary".into(),
        ));
    }

    let mut outgoing: BTreeMap<ExternalRootId, Vec<ExternalRootId>> = BTreeMap::new();
    for edge in &relation.edges {
        if !by_root.contains_key(&edge.interrupted) {
            return Err(ExternalRootDiagnostic(format!(
                "stack nesting relation references missing interrupted root 0x{:016x}",
                edge.interrupted.normalized_identity()
            )));
        }
        let preemptor = by_root.get(&edge.preemptor).ok_or_else(|| {
            ExternalRootDiagnostic(format!(
                "stack nesting relation references missing preemptor root 0x{:016x}",
                edge.preemptor.normalized_identity()
            ))
        })?;
        if preemptor.stack == EntryStack::ProviderSelected {
            return Err(ExternalRootDiagnostic(format!(
                "provider-selected stack for nested root 0x{:016x} does not determine whether the active stack is shared or switched",
                edge.preemptor.normalized_identity()
            )));
        }
        outgoing
            .entry(edge.interrupted)
            .or_default()
            .push(edge.preemptor);
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for root in by_root.keys().copied() {
        reject_stack_cycle(root, &outgoing, &mut visiting, &mut visited)?;
    }
    for root in by_root.keys().copied() {
        let mut active_classes = BTreeSet::new();
        reject_dedicated_stack_reentry(root, &outgoing, &by_root, &mut active_classes)?;
    }

    let composition_evidence = StackCompositionEvidence {
        relation: relation.clone(),
        summaries: by_root
            .iter()
            .map(|(root, summary)| (*root, (*summary).clone()))
            .collect(),
    };
    let non_authoritative_input_report_fingerprint =
        non_authoritative_stack_inputs_report_fingerprint(relation, &by_root);
    let mut demands = BTreeMap::new();
    let mut domain_wcsu_bytes = BTreeMap::new();
    let mut domain_alignments = BTreeMap::new();
    for (root, summary) in &by_root {
        let mut contributing_roots = BTreeSet::from([*root]);
        let mut validation_receipts = summary
            .local_evidence
            .provider_validation_receipt()
            .into_iter()
            .collect();
        let (composed_wcsu_bytes, wcsu_alignment) = compose_active_stack_peak(
            *root,
            summary.local_wcsu_bytes(),
            summary.wcsu_alignment(),
            &outgoing,
            &by_root,
            &mut contributing_roots,
            &mut validation_receipts,
        )?;
        let domain = stack_domain(summary.stack);
        domain_wcsu_bytes
            .entry(domain)
            .and_modify(|bytes: &mut u64| *bytes = (*bytes).max(composed_wcsu_bytes))
            .or_insert(composed_wcsu_bytes);
        domain_alignments
            .entry(domain)
            .and_modify(|alignment: &mut u64| *alignment = (*alignment).max(wcsu_alignment))
            .or_insert(wcsu_alignment);

        let mut report_fingerprint = Fnv1a::new();
        report_fingerprint.u64(non_authoritative_input_report_fingerprint);
        report_fingerprint.u64(root.normalized_identity());
        report_fingerprint.u64(composed_wcsu_bytes);
        report_fingerprint.u64(wcsu_alignment);
        for contributor in &contributing_roots {
            report_fingerprint.u64(contributor.normalized_identity());
        }
        demands.insert(
            *root,
            ComposedStackDemand {
                root: *root,
                root_provider: summary.provider,
                relation: relation.identity,
                stack: summary.stack,
                local_wcsu_bytes: summary.local_wcsu_bytes(),
                composed_wcsu_bytes,
                wcsu_alignment,
                contributing_roots,
                validation_receipts,
                composition_evidence: composition_evidence.clone(),
                non_authoritative_artifact_composition_report_fingerprint:
                    non_authoritative_input_report_fingerprint,
                non_authoritative_composition_report_fingerprint: report_fingerprint.finish(),
            },
        );
    }
    Ok(ArtifactStackComposition {
        relation: relation.identity,
        demands,
        domain_wcsu_bytes,
        domain_alignments,
        non_authoritative_composition_report_fingerprint:
            non_authoritative_input_report_fingerprint,
    })
}

fn reject_stack_cycle(
    root: ExternalRootId,
    outgoing: &BTreeMap<ExternalRootId, Vec<ExternalRootId>>,
    visiting: &mut BTreeSet<ExternalRootId>,
    visited: &mut BTreeSet<ExternalRootId>,
) -> Result<(), ExternalRootDiagnostic> {
    if visited.contains(&root) {
        return Ok(());
    }
    if !visiting.insert(root) {
        return Err(ExternalRootDiagnostic(format!(
            "stack nesting relation contains a cycle through root 0x{:016x}",
            root.normalized_identity()
        )));
    }
    if let Some(preemptors) = outgoing.get(&root) {
        for preemptor in preemptors {
            reject_stack_cycle(*preemptor, outgoing, visiting, visited)?;
        }
    }
    visiting.remove(&root);
    visited.insert(root);
    Ok(())
}

fn reject_dedicated_stack_reentry(
    root: ExternalRootId,
    outgoing: &BTreeMap<ExternalRootId, Vec<ExternalRootId>>,
    summaries: &BTreeMap<ExternalRootId, &ProviderStackSummary>,
    active_classes: &mut BTreeSet<u16>,
) -> Result<(), ExternalRootDiagnostic> {
    let stack = summaries
        .get(&root)
        .expect("nesting root has summary")
        .stack;
    let inserted = match stack {
        EntryStack::Dedicated { class } => {
            if !active_classes.insert(class) {
                return Err(ExternalRootDiagnostic(format!(
                    "stack nesting path re-enters active dedicated class {} at root 0x{:016x}",
                    class,
                    root.normalized_identity()
                )));
            }
            Some(class)
        }
        EntryStack::Interrupted | EntryStack::ProviderSelected => None,
    };
    if let Some(preemptors) = outgoing.get(&root) {
        for preemptor in preemptors {
            reject_dedicated_stack_reentry(*preemptor, outgoing, summaries, active_classes)?;
        }
    }
    if let Some(class) = inserted {
        active_classes.remove(&class);
    }
    Ok(())
}

fn compose_active_stack_peak(
    root: ExternalRootId,
    current_bytes: u64,
    current_alignment: u64,
    outgoing: &BTreeMap<ExternalRootId, Vec<ExternalRootId>>,
    summaries: &BTreeMap<ExternalRootId, &ProviderStackSummary>,
    contributing_roots: &mut BTreeSet<ExternalRootId>,
    validation_receipts: &mut BTreeSet<StackValidationReceiptId>,
) -> Result<(u64, u64), ExternalRootDiagnostic> {
    let mut peak = current_bytes;
    let mut alignment = current_alignment;
    if let Some(preemptors) = outgoing.get(&root) {
        for preemptor in preemptors {
            let summary = summaries
                .get(preemptor)
                .expect("nesting preemptor has summary");
            if summary.stack != EntryStack::Interrupted {
                continue;
            }
            contributing_roots.insert(*preemptor);
            validation_receipts.extend(summary.local_evidence.provider_validation_receipt());
            let aligned = align_up_checked(current_bytes, summary.wcsu_alignment())?;
            let nested_bytes =
                aligned
                    .checked_add(summary.local_wcsu_bytes())
                    .ok_or_else(|| {
                        ExternalRootDiagnostic("stack WCSU composition addition overflowed".into())
                    })?;
            let (nested_peak, nested_alignment) = compose_active_stack_peak(
                *preemptor,
                nested_bytes,
                current_alignment.max(summary.wcsu_alignment()),
                outgoing,
                summaries,
                contributing_roots,
                validation_receipts,
            )?;
            peak = peak.max(nested_peak);
            alignment = alignment.max(nested_alignment);
        }
    }
    Ok((peak, alignment))
}

pub(super) fn align_up_checked(value: u64, alignment: u64) -> Result<u64, ExternalRootDiagnostic> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
        .ok_or_else(|| ExternalRootDiagnostic("stack WCSU alignment overflowed".into()))
}

const fn stack_domain(stack: EntryStack) -> StackDomain {
    match stack {
        EntryStack::Interrupted => StackDomain::Interrupted,
        EntryStack::Dedicated { class } => StackDomain::Dedicated { class },
        EntryStack::ProviderSelected => StackDomain::ProviderSelected,
    }
}

fn non_authoritative_stack_inputs_report_fingerprint(
    relation: &StackNestingRelation,
    summaries: &BTreeMap<ExternalRootId, &ProviderStackSummary>,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(relation.identity.normalized_identity());
    hash.u64(summaries.len() as u64);
    for summary in summaries.values() {
        hash.u64(summary.root.normalized_identity());
        hash.u64(summary.provider.normalized_identity());
        fingerprint_entry_stack(&mut hash, summary.stack);
        fingerprint_stack_local_evidence(&mut hash, &summary.local_evidence);
    }
    hash.u64(relation.edges.len() as u64);
    for edge in &relation.edges {
        hash.u64(edge.interrupted.normalized_identity());
        hash.u64(edge.preemptor.normalized_identity());
    }
    hash.finish()
}

pub(super) fn fingerprint_stack_local_evidence(hash: &mut Fnv1a, evidence: &StackLocalEvidence) {
    match evidence {
        StackLocalEvidence::TerminalEntry(binding) => {
            hash.u64(0);
            hash.u64(u64::from(binding.psi.vocabulary_marker.get()));
            hash.bytes(binding.psi.program_fingerprint.as_bytes());
            hash.u64(match binding.architecture {
                target::Architecture::X86_64 => 1,
                target::Architecture::Aarch64 => 2,
            });
            hash.u64(binding.machine_entry.get());
            hash.u64(binding.ceiling_bytes);
            hash.u64(binding.stack_alignment);
            hash.u64(binding.contributing_machines.len() as u64);
            for machine in &binding.contributing_machines {
                hash.u64(machine.get());
            }
            hash.u64(binding.admitted_stack_contribution_report_identities.len() as u64);
            for report_identity in &binding.admitted_stack_contribution_report_identities {
                hash.u64(*report_identity);
            }
            hash.u64(binding.admitted_stack_contribution_commitments.len() as u64);
            for commitment in &binding.admitted_stack_contribution_commitments {
                hash.bytes(commitment);
            }
            hash.u64(binding.installed_code.normalized_identity());
            hash.u64(binding.artifact.normalized_identity());
            hash.u64(binding.entry.normalized_identity());
        }
        StackLocalEvidence::DeriverStubEntry(binding) => {
            hash.u64(2);
            hash.u64(u64::from(binding.psi.vocabulary_marker.get()));
            hash.bytes(binding.psi.program_fingerprint.as_bytes());
            hash.u64(match binding.architecture {
                target::Architecture::X86_64 => 1,
                target::Architecture::Aarch64 => 2,
            });
            hash.u64(binding.machine_entry.get());
            hash.u64(binding.member_ceiling_bytes);
            hash.u64(binding.stub_overhead_bytes);
            hash.u64(binding.ceiling_bytes);
            hash.u64(binding.stack_alignment);
            hash.u64(binding.contributing_machines.len() as u64);
            for machine in &binding.contributing_machines {
                hash.u64(machine.get());
            }
            hash.u64(binding.admitted_stack_contribution_report_identities.len() as u64);
            for report_identity in &binding.admitted_stack_contribution_report_identities {
                hash.u64(*report_identity);
            }
            hash.u64(binding.admitted_stack_contribution_commitments.len() as u64);
            for commitment in &binding.admitted_stack_contribution_commitments {
                hash.bytes(commitment);
            }
            hash.u64(binding.stub_contract_report_fingerprint);
            hash.u64(binding.member_section_offset);
            hash.bytes(&binding.member_call_displacement.to_le_bytes());
            hash.u64(binding.installed_code.normalized_identity());
            hash.u64(binding.artifact.normalized_identity());
            hash.u64(binding.entry.normalized_identity());
        }
        StackLocalEvidence::AdmittedProvider {
            local_wcsu_bytes,
            wcsu_alignment,
            validation_receipt,
        } => {
            hash.u64(1);
            hash.u64(*local_wcsu_bytes);
            hash.u64(*wcsu_alignment);
            hash.u64(validation_receipt.normalized_identity());
        }
    }
}

pub(super) fn fingerprint_entry_stack(hash: &mut Fnv1a, stack: EntryStack) {
    match stack {
        EntryStack::Interrupted => hash.u64(0),
        EntryStack::Dedicated { class } => {
            hash.u64(1);
            hash.u64(u64::from(class));
        }
        EntryStack::ProviderSelected => hash.u64(2),
    }
}

#[cfg(test)]
mod deriver_stub_tests {
    use super::*;
    use crate::tests::{installed_code_in_placement, root_id};
    use crate::{
        ArrivalStackRealizationOrigin, NestingRelationId, StackDomain, StackNestingRelation,
        X86_64GateProfileValidationReceiptId, bind_x86_64_target_direct_entry_stack_realization,
        compose_bound_entry_stack_epochs, produce_x86_64_installed_hardware_entry_facts,
        validate_x86_64_installed_gate_profile_roster,
    };
    use calling_conventions::{
        ArrivalContextId, BoundaryEntryPlan, CallSignature, CallingPolicy, EntryControl,
        EntryStack, MachineRegime, MachineState, MachineStateSet, Preemption, RegisterSet,
        StatePlan, ValueShape, X86_64ArrivalMechanism, X86_64GateKind, X86_64InstalledGateArrival,
        X86_64InstalledGateRealization, X86_64InstalledGateTssRealization,
        X86_64InstalledInterruptStack, X86_64InstalledPrivilegeStack,
        X86_64InstalledTaskStateSegmentRealization, derive_x86_64_entry_exit_stub,
        evaluate_call_plan, validate_boundary_entry_plan,
    };
    use executable_installation::InstalledCode;
    use installation_evidence::{ObjectEvidence, StackDemandEvidence};

    const MEMBER_CEILING: u64 = 64;
    const STUB_OFFSET: u64 = 16;
    const STUB_LEN: usize = 48;
    const MEMBER_OFFSET: u64 = 96;

    /// Artifact fixture whose member text sits away from the installed
    /// entry's stub interval — the shape `bind_installed_entry_stack` cannot
    /// bind and this module exists to bind.
    struct StubArtifact {
        identity: terminal_psi::TerminalPsiIdentity,
        member: semantic_vocabulary::MachineId,
        member_offset: usize,
        bytes: Vec<u8>,
    }

    impl ObjectEvidence for StubArtifact {
        fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
            self.identity
        }

        fn target(&self) -> target::NativeTarget {
            target::NativeTarget::linux_x64()
        }

        fn text_bytes(&self) -> &[u8] {
            &self.bytes
        }

        fn function_text_offset(&self, machine: semantic_vocabulary::MachineId) -> Option<usize> {
            (machine == self.member).then_some(self.member_offset)
        }
    }

    struct MemberStackDemand {
        identity: terminal_psi::TerminalPsiIdentity,
        member: semantic_vocabulary::MachineId,
        ceiling: u64,
        alignment: u32,
        contributing: BTreeSet<semantic_vocabulary::MachineId>,
    }

    impl StackDemandEvidence for MemberStackDemand {
        fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
            self.identity
        }

        fn architecture(&self) -> target::Architecture {
            target::Architecture::X86_64
        }

        fn entry(&self) -> semantic_vocabulary::MachineId {
            self.member
        }

        fn ceiling_bytes(&self) -> u64 {
            self.ceiling
        }

        fn stack_alignment(&self) -> u32 {
            self.alignment
        }

        fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId> {
            &self.contributing
        }

        fn admitted_stack_contribution_report_identities(&self) -> BTreeSet<u64> {
            BTreeSet::new()
        }

        fn admitted_stack_contribution_commitments(&self) -> BTreeSet<[u8; 32]> {
            BTreeSet::new()
        }
    }

    fn psi(seed: u8) -> terminal_psi::TerminalPsiIdentity {
        terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([seed; 32]),
        }
    }

    fn dedicated_interrupt_boundary(class: u16) -> ValidatedBoundaryEntryPlan {
        // Eight word parameters overflow the register quadrant: the outgoing
        // stack-argument area is nonzero, so the member-call frame is
        // exercised.
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 8],
            result: None,
        };
        let mut call =
            evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature).expect("call plan");
        call.ordinary_clobbers = RegisterSet::new([
            calling_conventions::MachineRegister::X86Rax,
            calling_conventions::MachineRegister::X86Rcx,
            calling_conventions::MachineRegister::X86Rdx,
        ]);
        call.entry_control = EntryControl::InterruptReturn;
        let interrupted = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::VectorRegisters,
        ]);
        let saved = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        validate_boundary_entry_plan(
            BoundaryEntryPlan {
                call,
                state: StatePlan {
                    initial_regime: MachineRegime::X86Long64,
                    interrupted_state: interrupted,
                    saved_state: saved,
                    restored_state: saved,
                    permitted_transitive_use: MachineStateSet::new([
                        MachineState::GeneralRegisters,
                        MachineState::Flags,
                    ]),
                    stack: EntryStack::Dedicated { class },
                    preemption: Preemption::Masked,
                },
            },
            &signature,
        )
        .expect("validated boundary")
    }

    /// The emitted stub bytes as installed: the `call rel32` opcode sits at
    /// `CALL_OP` inside the interval and seals the member's section offset.
    const CALL_OP: u16 = 20;

    fn stub_bytes(member_section_offset: u64) -> Vec<u8> {
        let mut bytes = vec![0u8; STUB_LEN];
        bytes[usize::from(CALL_OP)] = 0xe8;
        let next = STUB_OFFSET + u64::from(CALL_OP) + 5;
        let displacement = i64::try_from(member_section_offset).expect("section offset")
            - i64::try_from(next).expect("next");
        let displacement = i32::try_from(displacement).expect("rel32");
        bytes[usize::from(CALL_OP) + 1..usize::from(CALL_OP) + 5]
            .copy_from_slice(&displacement.to_le_bytes());
        bytes
    }

    fn installed_stub_image() -> (InstalledCode, Vec<u8>) {
        let mut image = vec![0u8; 160];
        image[STUB_OFFSET as usize..STUB_OFFSET as usize + STUB_LEN]
            .copy_from_slice(&stub_bytes(MEMBER_OFFSET));
        image[MEMBER_OFFSET as usize..MEMBER_OFFSET as usize + 32].fill(0x41);
        let constraints = layout_plans::PlacementConstraints::new(
            Some(
                layout_plans::PlacementAddressRange::new(0x1000, 0x1_0000)
                    .expect("placement range"),
            ),
            4096,
            layout_plans::PlacementPhase::PostHandoff,
            None,
            Some(
                layout_plans::ArtifactInstallationScopeId::from_normalized_identity(61)
                    .expect("installation scope"),
            ),
        )
        .expect("placement constraints");
        (
            installed_code_in_placement(
                0x8b1,
                EntryStubId::from_normalized_identity(0x8b0).expect("entry identity"),
                image.clone(),
                300,
                target::Architecture::X86_64,
                constraints,
                0x1000,
                4096,
            ),
            image,
        )
    }

    struct Fixture {
        member: semantic_vocabulary::MachineId,
        artifact: StubArtifact,
        demand: MemberStackDemand,
        entry: EntryStubId,
        code: InstalledCode,
        boundary: ValidatedBoundaryEntryPlan,
        realization: X86_64InstalledGateTssRealization,
    }

    fn fixture() -> Fixture {
        let member = semantic_vocabulary::MachineId::new(0x5a).expect("machine identity");
        let entry = EntryStubId::from_normalized_identity(0x8b0).expect("entry identity");
        let identity = psi(0x5b);
        let (code, image) = installed_stub_image();
        let artifact = StubArtifact {
            identity,
            member,
            member_offset: MEMBER_OFFSET as usize,
            bytes: image,
        };
        let demand = MemberStackDemand {
            identity,
            member,
            ceiling: MEMBER_CEILING,
            alignment: 8,
            contributing: BTreeSet::from([member]),
        };
        let boundary = dedicated_interrupt_boundary(11);
        let realization = X86_64InstalledGateTssRealization {
            gate: X86_64InstalledGateRealization {
                vector: 0x20,
                gate: X86_64GateKind::Interrupt,
                entry_identity: entry.normalized_identity(),
                entry_privilege: 0,
                interrupt_stack_table_slot: Some(2),
            },
            tss: X86_64InstalledTaskStateSegmentRealization {
                privilege_stacks: vec![X86_64InstalledPrivilegeStack {
                    entry_privilege: 0,
                    dedicated_class: 11,
                }],
                interrupt_stacks: vec![X86_64InstalledInterruptStack {
                    slot: 2,
                    dedicated_class: 11,
                }],
            },
            arrivals: vec![X86_64InstalledGateArrival {
                context: ArrivalContextId::new(1).expect("arrival context"),
                mechanism: X86_64ArrivalMechanism::ExternalInterrupt,
                interrupted_privilege: 3,
            }],
        };
        Fixture {
            member,
            artifact,
            demand,
            entry,
            code,
            boundary,
            realization,
        }
    }

    fn arrival_and_contract(
        fixture: &Fixture,
    ) -> (
        crate::InstalledX86_64TargetDerivedHardwareArrival,
        ValidatedX86_64DeriverStub,
    ) {
        let roster = validate_x86_64_installed_gate_profile_roster(
            &fixture.boundary,
            &fixture.code,
            fixture.entry,
            fixture.realization.clone(),
            root_id(
                0x8b2,
                X86_64GateProfileValidationReceiptId::from_normalized_identity,
            ),
        )
        .expect("gate/profile roster");
        let arrival = produce_x86_64_installed_hardware_entry_facts(
            &fixture.boundary,
            &fixture.code,
            fixture.entry,
            STUB_OFFSET,
            &roster,
            fixture.realization.clone(),
        )
        .expect("installed x86-64 target-derived arrival facts");
        let stub = derive_x86_64_entry_exit_stub(arrival.facts(), &fixture.boundary)
            .expect("deriver stub contract");
        (arrival, stub)
    }

    fn emission_for(
        member: semantic_vocabulary::MachineId,
    ) -> X86_64DeriverStubEntryEmission<'static> {
        X86_64DeriverStubEntryEmission {
            resolved_bytes: Box::leak(stub_bytes(MEMBER_OFFSET).into_boxed_slice()),
            member,
            stub_section_offset: STUB_OFFSET,
            member_section_offset: MEMBER_OFFSET,
            call_opcode_byte_offset: CALL_OP,
            call_field_byte_offset: CALL_OP + 1,
            next_instruction_byte_offset: CALL_OP + 5,
        }
    }

    #[test]
    fn deriver_stub_entry_stack_binds_and_composes_dedicated_demand() {
        let fixture = fixture();
        // The legacy binder cannot express this entry: the installed entry
        // interval holds stub bytes at offset 16 while the member's own text
        // sits at offset 96, so entry-offset binding must reject. The
        // deriver-stub binding exists to cover exactly this installed shape.
        let error = bind_installed_entry_stack(
            &fixture.demand,
            &fixture.artifact,
            &fixture.code,
            fixture.entry,
        )
        .expect_err("a stub-occupied entry interval cannot bind member text directly");
        assert!(
            error.0.contains("certified terminal function offset"),
            "{error}"
        );

        let (arrival, stub) = arrival_and_contract(&fixture);
        let emission = emission_for(fixture.member);
        let installed = bind_installed_deriver_stub_entry_stack(
            &fixture.demand,
            &fixture.artifact,
            &stub,
            emission,
            &fixture.boundary,
            &fixture.code,
            fixture.entry,
        )
        .expect("deriver stub stack demand binds");

        assert_eq!(installed.machine_entry(), fixture.member);
        assert_eq!(installed.member_ceiling_bytes(), MEMBER_CEILING);
        let overhead = stub.stub().peak_entry_overhead_bytes();
        assert_eq!(installed.stub_overhead_bytes(), overhead);
        assert_eq!(installed.ceiling_bytes(), MEMBER_CEILING + overhead);
        // The member declares 8-byte alignment; the composed demand reports
        // the stub's established 16-byte normalization.
        assert_eq!(installed.stack_alignment(), 16);
        assert_eq!(installed.member_section_offset(), MEMBER_OFFSET);
        assert!(overhead >= 15 * 8 + stub.stub().member_call_frame.reserved_bytes);

        let root = root_id(0x8b3, ExternalRootId::from_normalized_identity);
        let provider = root_id(0x8b4, RootProviderId::from_normalized_identity);
        let summary = ProviderStackSummary::from_deriver_stub_entry(
            root,
            provider,
            fixture.boundary.plan().state.stack,
            installed,
        );
        let bound = bind_x86_64_target_direct_entry_stack_realization(
            &summary,
            &fixture.boundary,
            &fixture.code,
            fixture.entry,
            &arrival,
        )
        .expect("target-direct realization binds through the stub entry");
        assert_eq!(
            bound.realization_evidence().arrival_origin(),
            ArrivalStackRealizationOrigin::X86_64TargetRule
        );
        assert_eq!(
            bound.realization_evidence().adapter_origin(),
            crate::AdapterStackRealizationOrigin::None
        );

        let composition = compose_bound_entry_stack_epochs(
            &StackNestingRelation {
                identity: root_id(0x8b5, NestingRelationId::from_normalized_identity),
                edges: BTreeSet::new(),
            },
            [&bound],
        )
        .expect("deriver-stub entry demand composes");
        // The dedicated critical stack carries the architectural IST-switch
        // arrival frame aligned up to the stub's established 16-byte
        // normalization (align16(40) = 48), plus the deriver overhead plus
        // the member's own demand — the stack column no longer rides the
        // opaque-adapter shape.
        let demand = composition.demand(root).expect("root demand");
        assert_eq!(
            demand
                .domain(StackDomain::Dedicated { class: 11 })
                .expect("dedicated domain")
                .bytes,
            48 + MEMBER_CEILING + overhead,
        );
    }

    #[test]
    fn deriver_stub_entry_stack_rejects_unbound_member_and_entry() {
        let fixture = fixture();
        let (arrival, stub) = arrival_and_contract(&fixture);

        // Member call row naming a machine that is not the demand's entry.
        let mut emission = emission_for(fixture.member);
        emission.member = semantic_vocabulary::MachineId::new(0x5b).expect("foreign member");
        let error = bind_installed_deriver_stub_entry_stack(
            &fixture.demand,
            &fixture.artifact,
            &stub,
            emission,
            &fixture.boundary,
            &fixture.code,
            fixture.entry,
        )
        .expect_err("member row must name the demand's entry machine");
        assert!(error.0.contains("entry machine"), "{error}");

        // Member section offset that is not the artifact's certified offset.
        let mut emission = emission_for(fixture.member);
        emission.member_section_offset = MEMBER_OFFSET + 32;
        let error = bind_installed_deriver_stub_entry_stack(
            &fixture.demand,
            &fixture.artifact,
            &stub,
            emission,
            &fixture.boundary,
            &fixture.code,
            fixture.entry,
        )
        .expect_err("member offset must name the certified function offset");
        assert!(
            error.0.contains("certified terminal function offset"),
            "{error}"
        );

        // A stub whose sealed call resolves somewhere other than the member:
        // the entry bytes are installed exactly, the row is consistent, but
        // the replayed equation rejects the wrong target.
        let foreign_target_bytes = stub_bytes(128);
        let mut emission = emission_for(fixture.member);
        emission.resolved_bytes = Box::leak(foreign_target_bytes.into_boxed_slice());
        let error = bind_installed_deriver_stub_entry_stack(
            &fixture.demand,
            &fixture.artifact,
            &stub,
            emission,
            &fixture.boundary,
            &fixture.code,
            fixture.entry,
        )
        .expect_err("entry bytes that do not match the installed interval");
        assert!(error.0.contains("resolved deriver stub bytes"), "{error}");

        // A boundary other than the one the contract sealed.
        let foreign_boundary = dedicated_interrupt_boundary(12);
        let error = bind_installed_deriver_stub_entry_stack(
            &fixture.demand,
            &fixture.artifact,
            &stub,
            emission_for(fixture.member),
            &foreign_boundary,
            &fixture.code,
            fixture.entry,
        )
        .expect_err("the contract's admitted boundary plan must be exact");
        assert!(error.0.contains("admitted boundary plan"), "{error}");

        // The composed summary bound to a different installed entry.
        let installed = bind_installed_deriver_stub_entry_stack(
            &fixture.demand,
            &fixture.artifact,
            &stub,
            emission_for(fixture.member),
            &fixture.boundary,
            &fixture.code,
            fixture.entry,
        )
        .expect("deriver stub stack demand binds");
        let summary = ProviderStackSummary::from_deriver_stub_entry(
            root_id(0x8b6, ExternalRootId::from_normalized_identity),
            root_id(0x8b7, RootProviderId::from_normalized_identity),
            fixture.boundary.plan().state.stack,
            installed,
        );
        let other_entry = EntryStubId::from_normalized_identity(0x8bf).expect("other entry");
        let error = bind_x86_64_target_direct_entry_stack_realization(
            &summary,
            &fixture.boundary,
            &fixture.code,
            other_entry,
            &arrival,
        )
        .expect_err("a summary naming another installed entry must reject");
        assert!(error.0.contains("different installed entry"), "{error}");
    }
}
