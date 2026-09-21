//! Compiler-owned source-assembly catalog metadata.
//!
//! Source parsing is target-agnostic today because target selection follows
//! syntax construction. This catalog still makes the accepted instruction
//! shape, target applicability, authority, operand constraints, machine-state
//! changes, availability, and register clobbers explicit.
//! Recognized instructions without a complete source contract remain refusal
//! entries rather than silently crossing the strict assembly surface.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInstructionAvailability {
    UserChecked,
    DeriverOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInstructionShape {
    JumpState,
    Halt,
    PortOut,
    PortIn,
    /// A data move between a writable Omega place and a readable value. The
    /// accepted form carries no memory-addressing operand: it lowers to an
    /// ordinary checked assignment, so provenance, permission and exact-type
    /// obligations are the assignment's own. Bracketed `[address]` spellings
    /// are not expressions and refuse before this shape applies.
    RegisterMove,
    /// A place-bearing memory transfer: the memory operand is spelled as an
    /// Omega place expression, so the access's provenance, permission and
    /// exact-type contract is the place's own — it lowers to the same checked
    /// place read or write an ordinary assignment performs. Bracketed
    /// `[address]` spellings are not expressions and refuse before this shape
    /// applies. Ordered variants (acquire/release) and width-suffixed,
    /// offset/unscaled, or multi-register forms are different contracts and
    /// stay refused.
    MemoryTransfer(AsmMemoryTransferKind),
    MemoryFence(AsmFenceKind),
    InterruptControl(AsmInterruptControlKind),
    FlagsSnapshot,
    FlagsRestore,
    MsrRead,
    MsrWrite,
    ControlRegisterRead(AsmControlRegister),
    ControlRegisterWrite(AsmControlRegister),
    /// AArch64 system-register read (`mrs`): the source register is folded
    /// into the mnemonic since an architectural register is not an operand
    /// place. The explicit destination receives the register's `u64` view.
    SystemRegisterRead(AsmSystemRegister),
    /// AArch64 system-register write (`msr`): the destination register is
    /// folded into the mnemonic; the explicit operand supplies the `u64`
    /// value.
    SystemRegisterWrite(AsmSystemRegister),
    /// Serializes the instruction stream itself rather than memory traffic:
    /// every prior instruction completes and instruction fetch re-synchronizes
    /// before the next instruction executes.
    InstructionSerialization(AsmInstructionSerializationKind),
    /// A scheduler/pipeline hint the core may legally elide; it never changes
    /// program semantics or machine-state obligations.
    SchedulingHint(AsmSchedulingHintKind),
    /// Cache/TLB maintenance on the machine's own caches: serializing and
    /// privileged, with no modeled operand place — the operation's subject is
    /// the cache hierarchy itself, not an addressable value. Members needing a
    /// memory operand (`invlpg`, `clflush`) stay refused until the catalog has
    /// a modeled memory operand contract.
    CacheOperation(AsmCacheOperationKind),
    DescriptorTableLoad,
    DerivedExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmMemoryTransferKind {
    /// `ldr destination, place`: the place's typed read assigns into the
    /// writable destination.
    Load,
    /// `str value, place`: the value's typed write assigns into the place.
    Store,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInstructionSerializationKind {
    /// x86_64 `serialize`: drains speculative execution and forces fetch to
    /// re-start after the instruction, bounding code-update races.
    Serialize,
    /// AArch64 `isb`: flushes the pipeline so later instructions re-fetch
    /// updated context (the AArch64 serialization barrier).
    InstructionSynchronizationBarrier,
}

impl AsmInstructionSerializationKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Serialize => "serialize",
            Self::InstructionSynchronizationBarrier => "isb",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::Serialize => "asm#serialize",
            Self::InstructionSynchronizationBarrier => "asm#isb",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Serialize, Self::InstructionSynchronizationBarrier]
            .into_iter()
            .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmCacheOperationKind {
    /// x86_64 `wbinvd`: writes back and invalidates all internal caches, then
    /// serializes the instruction stream. Ring-0 privileged; it carries no
    /// operand because its subject is the cache hierarchy, not a place.
    WriteBackInvalidate,
    /// x86_64 `invd`: invalidates all internal caches WITHOUT writing back
    /// modified lines — cached writes are dropped rather than committed.
    /// Serializing and ring-0 privileged; zero operands for the same reason
    /// `wbinvd` carries none.
    Invalidate,
    /// x86_64 `wbnoinvd`: writes back modified lines to memory but leaves
    /// them valid in the caches. Serializing and ring-0 privileged, zero
    /// operands.
    WriteBackNoInvalidate,
}

impl AsmCacheOperationKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::WriteBackInvalidate => "wbinvd",
            Self::Invalidate => "invd",
            Self::WriteBackNoInvalidate => "wbnoinvd",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::WriteBackInvalidate => "asm#wbinvd",
            Self::Invalidate => "asm#invd",
            Self::WriteBackNoInvalidate => "asm#wbnoinvd",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [
            Self::WriteBackInvalidate,
            Self::Invalidate,
            Self::WriteBackNoInvalidate,
        ]
        .into_iter()
        .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmSchedulingHintKind {
    /// x86_64 `pause`: spin-wait pipeline hint; improves a polling loop's
    /// sibling-thread behavior without changing its result.
    SpinPause,
    /// AArch64 `yield`: scheduling hint; the core may deschedule this thread.
    Yield,
    /// `nop` on both supported ISAs: a pipeline no-op the core may elide; it
    /// occupies an instruction slot without changing program semantics or
    /// machine-state obligations.
    Nop,
    /// AArch64 `wfe`: wait-for-event hint; the core may suspend until the
    /// event register is set, and is equally permitted to complete as a
    /// no-op — the suspended wait bounds nothing the source observes.
    WaitForEvent,
    /// AArch64 `wfi`: wait-for-interrupt hint; the core may suspend until an
    /// interrupt arrives, and is equally permitted to complete as a no-op.
    WaitForInterrupt,
    /// AArch64 `sev`: send-event hint; signals an event to all cores, and is
    /// equally permitted to complete as a no-op — the signal is advisory.
    SendEvent,
    /// AArch64 `sevl`: send-event-local hint; like `sev` but signaled only
    /// within the local cluster, and equally permitted to complete as a
    /// no-op.
    SendEventLocal,
}

impl AsmSchedulingHintKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::SpinPause => "pause",
            Self::Yield => "yield",
            Self::Nop => "nop",
            Self::WaitForEvent => "wfe",
            Self::WaitForInterrupt => "wfi",
            Self::SendEvent => "sev",
            Self::SendEventLocal => "sevl",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::SpinPause => "asm#pause",
            Self::Yield => "asm#yield",
            Self::Nop => "asm#nop",
            Self::WaitForEvent => "asm#wfe",
            Self::WaitForInterrupt => "asm#wfi",
            Self::SendEvent => "asm#sev",
            Self::SendEventLocal => "asm#sevl",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [
            Self::SpinPause,
            Self::Yield,
            Self::Nop,
            Self::WaitForEvent,
            Self::WaitForInterrupt,
            Self::SendEvent,
            Self::SendEventLocal,
        ]
        .into_iter()
        .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmControlRegister {
    Cr0,
    Cr2,
    Cr3,
    Cr4,
}

impl AsmControlRegister {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cr0 => "cr0",
            Self::Cr2 => "cr2",
            Self::Cr3 => "cr3",
            Self::Cr4 => "cr4",
        }
    }

    pub const fn read_mnemonic(self) -> &'static str {
        match self {
            Self::Cr0 => "read_cr0",
            Self::Cr2 => "read_cr2",
            Self::Cr3 => "read_cr3",
            Self::Cr4 => "read_cr4",
        }
    }

    pub const fn write_mnemonic(self) -> Option<&'static str> {
        match self {
            Self::Cr0 => Some("write_cr0"),
            Self::Cr2 => None,
            Self::Cr3 => Some("write_cr3"),
            Self::Cr4 => Some("write_cr4"),
        }
    }

    pub const fn read_intrinsic_name(self) -> &'static str {
        match self {
            Self::Cr0 => "asm#read_cr0",
            Self::Cr2 => "asm#read_cr2",
            Self::Cr3 => "asm#read_cr3",
            Self::Cr4 => "asm#read_cr4",
        }
    }

    pub const fn write_intrinsic_name(self) -> Option<&'static str> {
        match self {
            Self::Cr0 => Some("asm#write_cr0"),
            Self::Cr2 => None,
            Self::Cr3 => Some("asm#write_cr3"),
            Self::Cr4 => Some("asm#write_cr4"),
        }
    }

    pub fn from_read_mnemonic(name: &str) -> Option<Self> {
        [Self::Cr0, Self::Cr2, Self::Cr3, Self::Cr4]
            .into_iter()
            .find(|register| register.read_mnemonic() == name)
    }

    pub fn from_write_mnemonic(name: &str) -> Option<Self> {
        [Self::Cr0, Self::Cr3, Self::Cr4]
            .into_iter()
            .find(|register| register.write_mnemonic() == Some(name))
    }

    pub fn from_read_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Cr0, Self::Cr2, Self::Cr3, Self::Cr4]
            .into_iter()
            .find(|register| register.read_intrinsic_name() == name)
    }

    pub fn from_write_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Cr0, Self::Cr3, Self::Cr4]
            .into_iter()
            .find(|register| register.write_intrinsic_name() == Some(name))
    }
}

/// AArch64 system registers accessible through the structured
/// `read_<sysreg>`/`write_<sysreg>` spellings (architecturally `mrs`/`msr`).
/// The set is the EL1 control, translation and thread set a kernel reaches
/// for plus its read-only syndrome registers; ELR_EL1/SPSR_EL1 stay out —
/// writing them participates in a mode transition (eret's regime contract),
/// which AsmInstructionContract does not model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmSystemRegister {
    SctlrEl1,
    TcrEl1,
    Ttbr0El1,
    Ttbr1El1,
    MairEl1,
    VbarEl1,
    TpidrEl1,
    EsrEl1,
    FarEl1,
}

impl AsmSystemRegister {
    const ALL: [Self; 9] = [
        Self::SctlrEl1,
        Self::TcrEl1,
        Self::Ttbr0El1,
        Self::Ttbr1El1,
        Self::MairEl1,
        Self::VbarEl1,
        Self::TpidrEl1,
        Self::EsrEl1,
        Self::FarEl1,
    ];

    /// Syndrome registers are read-only in the catalog: they describe the
    /// last exception rather than machine configuration.
    const WRITABLE: [Self; 7] = [
        Self::SctlrEl1,
        Self::TcrEl1,
        Self::Ttbr0El1,
        Self::Ttbr1El1,
        Self::MairEl1,
        Self::VbarEl1,
        Self::TpidrEl1,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::SctlrEl1 => "sctlr_el1",
            Self::TcrEl1 => "tcr_el1",
            Self::Ttbr0El1 => "ttbr0_el1",
            Self::Ttbr1El1 => "ttbr1_el1",
            Self::MairEl1 => "mair_el1",
            Self::VbarEl1 => "vbar_el1",
            Self::TpidrEl1 => "tpidr_el1",
            Self::EsrEl1 => "esr_el1",
            Self::FarEl1 => "far_el1",
        }
    }

    pub const fn read_mnemonic(self) -> &'static str {
        match self {
            Self::SctlrEl1 => "read_sctlr_el1",
            Self::TcrEl1 => "read_tcr_el1",
            Self::Ttbr0El1 => "read_ttbr0_el1",
            Self::Ttbr1El1 => "read_ttbr1_el1",
            Self::MairEl1 => "read_mair_el1",
            Self::VbarEl1 => "read_vbar_el1",
            Self::TpidrEl1 => "read_tpidr_el1",
            Self::EsrEl1 => "read_esr_el1",
            Self::FarEl1 => "read_far_el1",
        }
    }

    pub const fn write_mnemonic(self) -> Option<&'static str> {
        match self {
            Self::SctlrEl1 => Some("write_sctlr_el1"),
            Self::TcrEl1 => Some("write_tcr_el1"),
            Self::Ttbr0El1 => Some("write_ttbr0_el1"),
            Self::Ttbr1El1 => Some("write_ttbr1_el1"),
            Self::MairEl1 => Some("write_mair_el1"),
            Self::VbarEl1 => Some("write_vbar_el1"),
            Self::TpidrEl1 => Some("write_tpidr_el1"),
            Self::EsrEl1 | Self::FarEl1 => None,
        }
    }

    pub const fn read_intrinsic_name(self) -> &'static str {
        match self {
            Self::SctlrEl1 => "asm#read_sctlr_el1",
            Self::TcrEl1 => "asm#read_tcr_el1",
            Self::Ttbr0El1 => "asm#read_ttbr0_el1",
            Self::Ttbr1El1 => "asm#read_ttbr1_el1",
            Self::MairEl1 => "asm#read_mair_el1",
            Self::VbarEl1 => "asm#read_vbar_el1",
            Self::TpidrEl1 => "asm#read_tpidr_el1",
            Self::EsrEl1 => "asm#read_esr_el1",
            Self::FarEl1 => "asm#read_far_el1",
        }
    }

    pub const fn write_intrinsic_name(self) -> Option<&'static str> {
        match self {
            Self::SctlrEl1 => Some("asm#write_sctlr_el1"),
            Self::TcrEl1 => Some("asm#write_tcr_el1"),
            Self::Ttbr0El1 => Some("asm#write_ttbr0_el1"),
            Self::Ttbr1El1 => Some("asm#write_ttbr1_el1"),
            Self::MairEl1 => Some("asm#write_mair_el1"),
            Self::VbarEl1 => Some("asm#write_vbar_el1"),
            Self::TpidrEl1 => Some("asm#write_tpidr_el1"),
            Self::EsrEl1 | Self::FarEl1 => None,
        }
    }

    pub fn from_read_mnemonic(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|register| register.read_mnemonic() == name)
    }

    pub fn from_write_mnemonic(name: &str) -> Option<Self> {
        Self::WRITABLE
            .into_iter()
            .find(|register| register.write_mnemonic() == Some(name))
    }

    pub fn from_read_intrinsic_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|register| register.read_intrinsic_name() == name)
    }

    pub fn from_write_intrinsic_name(name: &str) -> Option<Self> {
        Self::WRITABLE
            .into_iter()
            .find(|register| register.write_intrinsic_name() == Some(name))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmTargetApplicability {
    Any,
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmAuthorityRequirement {
    None,
    MachineOwner,
    PortIo,
    IdtControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmFenceKind {
    Load,
    Store,
    Full,
}

impl AsmFenceKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Load => "lfence",
            Self::Store => "sfence",
            Self::Full => "mfence",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::Load => "asm#lfence",
            Self::Store => "asm#sfence",
            Self::Full => "asm#mfence",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Load, Self::Store, Self::Full]
            .into_iter()
            .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmMemoryOrdering {
    None,
    Fence(AsmFenceKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInterruptControlKind {
    Disable,
    Enable,
}

impl AsmInterruptControlKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Disable => "cli",
            Self::Enable => "sti",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::Disable => "asm#cli",
            Self::Enable => "asm#sti",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Disable, Self::Enable]
            .into_iter()
            .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInterruptFlagEffect {
    None,
    /// Clear RFLAGS.IF before the following instruction executes.
    Disable,
    /// Set RFLAGS.IF, with maskable interrupts recognized only after the
    /// instruction following STI has executed.
    EnableAfterNextInstruction,
    /// Restore IF from the explicit saved-flags operand.
    RestoreFromOperand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmFlagsDataFlow {
    None,
    /// Snapshot RFLAGS into the instruction's explicit destination operand.
    SnapshotToOperand,
    /// Restore the architecturally writable RFLAGS fields from an explicit
    /// source operand. The realized sequence keeps RSP balanced.
    RestoreFromOperand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmOperandAccess {
    Read,
    ReadPlace,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsmOperandConstraint {
    /// Source-facing role used in diagnostics (`port`, `value`, ...).
    pub role: &'static str,
    /// Exact architectural register the realized sequence presents to the
    /// instruction. This is a register constraint, not source register syntax.
    pub target_register: &'static str,
    pub access: AsmOperandAccess,
    pub expected_type_name: &'static str,
    /// Literals are admitted only when this bound is present.
    pub maximum_literal: Option<u64>,
}

impl AsmOperandConstraint {
    pub const fn read(
        role: &'static str,
        target_register: &'static str,
        expected_type_name: &'static str,
        maximum_literal: u64,
    ) -> Self {
        Self {
            role,
            target_register,
            access: AsmOperandAccess::Read,
            expected_type_name,
            maximum_literal: Some(maximum_literal),
        }
    }

    pub const fn write_place(
        role: &'static str,
        target_register: &'static str,
        expected_type_name: &'static str,
    ) -> Self {
        Self {
            role,
            target_register,
            access: AsmOperandAccess::Write,
            expected_type_name,
            maximum_literal: None,
        }
    }

    pub const fn read_place(
        role: &'static str,
        target_register: &'static str,
        expected_type_name: &'static str,
    ) -> Self {
        Self {
            role,
            target_register,
            access: AsmOperandAccess::ReadPlace,
            expected_type_name,
            maximum_literal: None,
        }
    }

    pub const fn expected_type_name(self) -> &'static str {
        self.expected_type_name
    }

    pub const fn maximum_literal(self) -> Option<u64> {
        self.maximum_literal
    }

    pub const fn requires_writable_place(self) -> bool {
        matches!(self.access, AsmOperandAccess::Write)
    }

    pub const fn requires_place(self) -> bool {
        matches!(
            self.access,
            AsmOperandAccess::ReadPlace | AsmOperandAccess::Write
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsmInstructionContract {
    pub availability: AsmInstructionAvailability,
    pub shape: AsmInstructionShape,
    pub target: AsmTargetApplicability,
    pub required_authority: AsmAuthorityRequirement,
    /// Source-order operands. These are target-register constraints, not
    /// permissive numeric coercions.
    pub operands: &'static [AsmOperandConstraint],
    /// Ordering established by the instruction. Kept separate from service
    /// reach: a CPU fence orders memory but does not contact a provider.
    pub memory_ordering: AsmMemoryOrdering,
    /// Architectural interrupt-flag transition. STI's one-instruction delay
    /// is part of the contract rather than being flattened into "enabled".
    pub interrupt_flag_effect: AsmInterruptFlagEffect,
    /// Explicit RFLAGS value flow. This distinguishes a compiler-balanced
    /// snapshot/restore from exposing raw stack-mutating push/pop operations.
    pub flags_data_flow: AsmFlagsDataFlow,
    /// Registers changed by the realized instruction sequence. This includes
    /// compiler scratch registers used to materialize structured operands.
    pub clobbers: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInstructionRefusal {
    /// A return, call, or indirect branch would bypass Omega state edges.
    HiddenControlExit,
    /// No provenance/permission-bearing operand contract exists yet.
    UnmodeledMemoryAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmCatalogEntry {
    Contract(AsmInstructionContract),
    Refused(AsmInstructionRefusal),
}

const NO_OPERANDS: &[AsmOperandConstraint] = &[];
const PORT_OUT_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::read("port", "dx", "u16", u16::MAX as u64),
    AsmOperandConstraint::read("value", "al", "u8", u8::MAX as u64),
];
const PORT_IN_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::write_place("destination", "al", "u8"),
    AsmOperandConstraint::read("port", "dx", "u16", u16::MAX as u64),
];
const FLAGS_SNAPSHOT_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "rflags",
    "u64",
)];
const FLAGS_RESTORE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read_place(
    "saved flags",
    "rflags",
    "u64",
)];
const MSR_READ_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::write_place("destination", "edx:eax", "u64"),
    AsmOperandConstraint::read("MSR index", "ecx", "u32", u32::MAX as u64),
];
const MSR_WRITE_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::read("MSR index", "ecx", "u32", u32::MAX as u64),
    AsmOperandConstraint::read("value", "edx:eax", "u64", u64::MAX),
];
const CR0_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "cr0",
    "u64",
)];
const CR2_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "cr2",
    "u64",
)];
const CR3_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "cr3",
    "u64",
)];
const CR4_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "cr4",
    "u64",
)];
const CR0_WRITE_OPERANDS: &[AsmOperandConstraint] =
    &[AsmOperandConstraint::read("value", "cr0", "u64", u64::MAX)];
const CR3_WRITE_OPERANDS: &[AsmOperandConstraint] =
    &[AsmOperandConstraint::read("value", "cr3", "u64", u64::MAX)];
const CR4_WRITE_OPERANDS: &[AsmOperandConstraint] =
    &[AsmOperandConstraint::read("value", "cr4", "u64", u64::MAX)];
// AArch64 system-register access folds the named register into the source
// spelling (`read_sctlr_el1` realizes as `mrs`, `write_sctlr_el1` as `msr`),
// so each contract carries a single `u64` place or value operand whose
// architectural target is the register itself.
const SCTLR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "sctlr_el1",
    "u64",
)];
const TCR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "tcr_el1",
    "u64",
)];
const TTBR0_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "ttbr0_el1",
    "u64",
)];
const TTBR1_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "ttbr1_el1",
    "u64",
)];
const MAIR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "mair_el1",
    "u64",
)];
const VBAR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "vbar_el1",
    "u64",
)];
const TPIDR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "tpidr_el1",
    "u64",
)];
const ESR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "esr_el1",
    "u64",
)];
const FAR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "far_el1",
    "u64",
)];
const SCTLR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read(
    "value",
    "sctlr_el1",
    "u64",
    u64::MAX,
)];
const TCR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read(
    "value",
    "tcr_el1",
    "u64",
    u64::MAX,
)];
const TTBR0_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read(
    "value",
    "ttbr0_el1",
    "u64",
    u64::MAX,
)];
const TTBR1_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read(
    "value",
    "ttbr1_el1",
    "u64",
    u64::MAX,
)];
const MAIR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read(
    "value",
    "mair_el1",
    "u64",
    u64::MAX,
)];
const VBAR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read(
    "value",
    "vbar_el1",
    "u64",
    u64::MAX,
)];
const TPIDR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read(
    "value",
    "tpidr_el1",
    "u64",
    u64::MAX,
)];
const NO_CLOBBERS: &[&str] = &[];
const PORT_OUT_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r11", "r15"];
const PORT_IN_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r15"];
const FLAGS_OPERAND_CLOBBERS: &[&str] = &["r10", "r15"];
const MSR_READ_CLOBBERS: &[&str] = &["rax", "rcx", "rdx", "r10", "r11", "r15"];
const MSR_WRITE_CLOBBERS: &[&str] = &["rax", "rcx", "rdx", "r10", "r11", "r15"];
const CONTROL_REGISTER_READ_CLOBBERS: &[&str] = &["r10", "r15"];
const CONTROL_REGISTER_WRITE_CLOBBERS: &[&str] = &["rax", "r10", "r11", "r15"];
const SYSTEM_REGISTER_READ_CLOBBERS: &[&str] = &["x9", "x15"];
const SYSTEM_REGISTER_WRITE_CLOBBERS: &[&str] = &["x9", "x10", "x11", "x15"];
const IDT_DESCRIPTOR_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read_place(
    "IDT descriptor",
    "r10",
    "IdtDescriptor",
)];
const IDT_DESCRIPTOR_CLOBBERS: &[&str] = &["r10"];

pub fn asm_catalog_entry(mnemonic: &str) -> Option<AsmCatalogEntry> {
    use AsmAuthorityRequirement::{
        IdtControl as IdtControlAuthority, MachineOwner, None as NoAuthority,
        PortIo as PortIoAuthority,
    };
    use AsmCatalogEntry::{Contract, Refused};
    use AsmFenceKind::{Full, Load, Store};
    use AsmFlagsDataFlow::{
        None as NoFlagsDataFlow, RestoreFromOperand as RestoreFlags,
        SnapshotToOperand as SnapshotFlags,
    };
    use AsmInstructionAvailability::{DeriverOnly, UserChecked};
    use AsmInstructionRefusal::{HiddenControlExit, UnmodeledMemoryAccess};
    use AsmInstructionSerializationKind::{InstructionSynchronizationBarrier, Serialize};
    use AsmInstructionShape::{
        CacheOperation, DerivedExit, DescriptorTableLoad, FlagsRestore, FlagsSnapshot, Halt,
        InstructionSerialization, InterruptControl, JumpState, MemoryFence, MemoryTransfer,
        MsrRead, MsrWrite, PortIn, PortOut, RegisterMove, SchedulingHint,
    };
    use AsmInterruptControlKind::{Disable, Enable};
    use AsmInterruptFlagEffect::{
        Disable as DisablesInterrupts, EnableAfterNextInstruction, None as NoInterruptChange,
        RestoreFromOperand as RestoreInterruptFlag,
    };
    use AsmMemoryOrdering::{Fence, None as NoOrdering};
    use AsmSchedulingHintKind::{
        Nop, SendEvent, SendEventLocal, SpinPause, WaitForEvent, WaitForInterrupt, Yield,
    };
    use AsmTargetApplicability::{Aarch64, Any, X86_64};

    if let Some(register) = AsmControlRegister::from_read_mnemonic(mnemonic) {
        let operands = match register {
            AsmControlRegister::Cr0 => CR0_READ_OPERANDS,
            AsmControlRegister::Cr2 => CR2_READ_OPERANDS,
            AsmControlRegister::Cr3 => CR3_READ_OPERANDS,
            AsmControlRegister::Cr4 => CR4_READ_OPERANDS,
        };
        return Some(Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: AsmInstructionShape::ControlRegisterRead(register),
            target: X86_64,
            required_authority: MachineOwner,
            operands,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: CONTROL_REGISTER_READ_CLOBBERS,
        }));
    }
    if let Some(register) = AsmControlRegister::from_write_mnemonic(mnemonic) {
        let operands = match register {
            AsmControlRegister::Cr0 => CR0_WRITE_OPERANDS,
            AsmControlRegister::Cr2 => unreachable!("CR2 has no source write form"),
            AsmControlRegister::Cr3 => CR3_WRITE_OPERANDS,
            AsmControlRegister::Cr4 => CR4_WRITE_OPERANDS,
        };
        return Some(Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: AsmInstructionShape::ControlRegisterWrite(register),
            target: X86_64,
            required_authority: MachineOwner,
            operands,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: CONTROL_REGISTER_WRITE_CLOBBERS,
        }));
    }
    if let Some(register) = AsmSystemRegister::from_read_mnemonic(mnemonic) {
        let operands = match register {
            AsmSystemRegister::SctlrEl1 => SCTLR_EL1_READ_OPERANDS,
            AsmSystemRegister::TcrEl1 => TCR_EL1_READ_OPERANDS,
            AsmSystemRegister::Ttbr0El1 => TTBR0_EL1_READ_OPERANDS,
            AsmSystemRegister::Ttbr1El1 => TTBR1_EL1_READ_OPERANDS,
            AsmSystemRegister::MairEl1 => MAIR_EL1_READ_OPERANDS,
            AsmSystemRegister::VbarEl1 => VBAR_EL1_READ_OPERANDS,
            AsmSystemRegister::TpidrEl1 => TPIDR_EL1_READ_OPERANDS,
            AsmSystemRegister::EsrEl1 => ESR_EL1_READ_OPERANDS,
            AsmSystemRegister::FarEl1 => FAR_EL1_READ_OPERANDS,
        };
        return Some(Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: AsmInstructionShape::SystemRegisterRead(register),
            target: Aarch64,
            required_authority: MachineOwner,
            operands,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: SYSTEM_REGISTER_READ_CLOBBERS,
        }));
    }
    if let Some(register) = AsmSystemRegister::from_write_mnemonic(mnemonic) {
        let operands = match register {
            AsmSystemRegister::SctlrEl1 => SCTLR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::TcrEl1 => TCR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::Ttbr0El1 => TTBR0_EL1_WRITE_OPERANDS,
            AsmSystemRegister::Ttbr1El1 => TTBR1_EL1_WRITE_OPERANDS,
            AsmSystemRegister::MairEl1 => MAIR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::VbarEl1 => VBAR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::TpidrEl1 => TPIDR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::EsrEl1 | AsmSystemRegister::FarEl1 => {
                unreachable!("syndrome registers have no source write form")
            }
        };
        // Register writes with architecturally deferred effect (translation
        // base, memory attributes, control state) synchronize context through
        // a caller-sequenced `isb`; the contract records no implicit fence.
        return Some(Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: AsmInstructionShape::SystemRegisterWrite(register),
            target: Aarch64,
            required_authority: MachineOwner,
            operands,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: SYSTEM_REGISTER_WRITE_CLOBBERS,
        }));
    }

    let entry = match mnemonic {
        "jmp" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: JumpState,
            target: Any,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "hlt" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: Halt,
            target: Any,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "out" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: PortOut,
            target: X86_64,
            required_authority: PortIoAuthority,
            operands: PORT_OUT_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: PORT_OUT_CLOBBERS,
        }),
        "in" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: PortIn,
            target: X86_64,
            required_authority: PortIoAuthority,
            operands: PORT_IN_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: PORT_IN_CLOBBERS,
        }),
        "lfence" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: MemoryFence(Load),
            target: X86_64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: Fence(Load),
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "sfence" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: MemoryFence(Store),
            target: X86_64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: Fence(Store),
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "mfence" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: MemoryFence(Full),
            target: X86_64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: Fence(Full),
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "cli" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: InterruptControl(Disable),
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: DisablesInterrupts,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "sti" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: InterruptControl(Enable),
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: EnableAfterNextInstruction,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "pushfq" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: FlagsSnapshot,
            target: X86_64,
            required_authority: NoAuthority,
            operands: FLAGS_SNAPSHOT_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: SnapshotFlags,
            clobbers: FLAGS_OPERAND_CLOBBERS,
        }),
        "popfq" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: FlagsRestore,
            target: X86_64,
            required_authority: MachineOwner,
            operands: FLAGS_RESTORE_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: RestoreInterruptFlag,
            flags_data_flow: RestoreFlags,
            clobbers: FLAGS_OPERAND_CLOBBERS,
        }),
        "rdmsr" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: MsrRead,
            target: X86_64,
            required_authority: MachineOwner,
            operands: MSR_READ_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: MSR_READ_CLOBBERS,
        }),
        "wrmsr" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: MsrWrite,
            target: X86_64,
            required_authority: MachineOwner,
            operands: MSR_WRITE_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: MSR_WRITE_CLOBBERS,
        }),

        // Instruction-stream serialization and scheduling-hint directives.
        // Neither reads nor mutates modeled machine state, so they carry no
        // authority requirement -- `serialize`/`isb` bound reordering (the
        // realized sequence is the instruction itself), while `pause`/`yield`
        // are legal to elide entirely.
        "serialize" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: InstructionSerialization(Serialize),
            target: X86_64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "isb" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: InstructionSerialization(InstructionSynchronizationBarrier),
            target: Aarch64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "pause" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: SchedulingHint(SpinPause),
            target: X86_64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "yield" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: SchedulingHint(Yield),
            target: Aarch64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "nop" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: SchedulingHint(Nop),
            target: Any,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        // The remaining AArch64 architectural HINT encodings: `wfe`/`wfi`
        // suspend-until-event and `sev`/`sevl` signal one — every member is
        // architecturally permitted to complete as a no-op, so each carries
        // the same empty contract as `yield`.
        "wfe" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: SchedulingHint(WaitForEvent),
            target: Aarch64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "wfi" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: SchedulingHint(WaitForInterrupt),
            target: Aarch64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "sev" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: SchedulingHint(SendEvent),
            target: Aarch64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "sevl" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: SchedulingHint(SendEventLocal),
            target: Aarch64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        // Cache maintenance acts on the machine's own caches rather than a
        // modeled place: serializing, machine-owner operations whose operand
        // list and clobber list are both empty. `invd` drops modified lines
        // without writeback; `wbnoinvd` writes back without invalidating.
        "wbinvd" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: CacheOperation(AsmCacheOperationKind::WriteBackInvalidate),
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "invd" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: CacheOperation(AsmCacheOperationKind::Invalidate),
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "wbnoinvd" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: CacheOperation(AsmCacheOperationKind::WriteBackNoInvalidate),
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),

        // This remains deriver-only: an admitted provider supplies the
        // descriptor operand under the instruction's checked authority
        // contract, never as an unrestricted source address.
        "lidt" => Contract(AsmInstructionContract {
            availability: DeriverOnly,
            shape: DescriptorTableLoad,
            target: X86_64,
            required_authority: IdtControlAuthority,
            operands: IDT_DESCRIPTOR_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: IDT_DESCRIPTOR_CLOBBERS,
        }),

        // These are real catalog operations, but only derived entry/exit
        // machinery may discharge their complete state-plan contracts.
        "iretq" | "sysret" | "sysretq" => Contract(AsmInstructionContract {
            availability: DeriverOnly,
            shape: DerivedExit,
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "eret" => Contract(AsmInstructionContract {
            availability: DeriverOnly,
            shape: DerivedExit,
            target: Aarch64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),

        // These spell control edges which cannot be represented by the current
        // source form. Direct state jumps use the checked `jmp state(...)` arm.
        // The list covers the common return/call/branch spellings on both
        // supported ISAs — x86 near/far/AT&T and operand-size forms including
        // the interrupt-return spellings (`iret*` separate from the contracted
        // `iretq` deriver), the whole conditional-branch (`j*`) and `loop`
        // families, software interrupts, and the AArch64
        // branch/compare-and-branch/test-and-branch family including the
        // branch-consistent `bc.cond` head and the pointer-authenticated
        // branch/return spellings (`b.cond` spellings already refuse at their
        // `b` mnemonic head) — so each refuses for the semantic reason rather
        // than as arbitrary unknown text. Supervisor traps (`svc`/`hvc`/`smc`/
        // `brk` and the x86 `syscall`/`sysenter`/`sysexit` ring calls) are
        // service-admission candidates, not hidden exits, and stay
        // unrecognized here.
        "ret" | "retq" | "retn" | "retw" | "retaa" | "retab" | "retf" | "lret" | "iret"
        | "iretd" | "iretw" | "call" | "callq" | "callf" | "lcall" | "jmpq" | "jmpf" | "jmpl"
        | "ljmp" | "ljmpl" | "br" | "blr" | "b" | "bl" | "bx" | "blx" | "bc" | "braa" | "brab"
        | "braaz" | "brabz" | "blraa" | "blrab" | "blraaz" | "blrabz" | "eretaa" | "eretab"
        | "drps" | "cbz" | "cbnz" | "tbz" | "tbnz" | "loop" | "loope" | "loopne" | "loopz"
        | "loopnz" | "jcxz" | "jecxz" | "jrcxz" | "int" | "int1" | "int3" | "into" | "je"
        | "jne" | "jz" | "jnz" | "ja" | "jae" | "jb" | "jbe" | "jna" | "jnae" | "jnb" | "jnbe"
        | "jg" | "jge" | "jl" | "jle" | "jng" | "jnge" | "jnl" | "jnle" | "jo" | "jno" | "js"
        | "jns" | "jp" | "jpe" | "jnp" | "jpo" | "jc" | "jnc" => Refused(HiddenControlExit),

        // The register-only move is the structured decoding of `mov`: both
        // operands are ordinary Omega expressions (a writable destination place
        // and a readable value), so the copy's provenance, permission and
        // exact-type contract is the ordinary assignment's. A bracketed
        // `[address]` operand still refuses as unmodeled memory access, and
        // an authorized view spells its access as an ordinary indexed place.
        "mov" | "movq" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: RegisterMove,
            target: Any,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),

        // The canonical unordered memory transfers admit once their memory
        // operand is a structured Omega place rather than a bracketed address:
        // the place's own provenance, permission and exact-type contract is
        // the model, and the instruction lowers to the same checked place
        // access an ordinary assignment performs. Width-suffixed forms
        // (`ldrb`/`strh`/...) are a different contract — an element-width
        // access is not a full-place copy — and offset/unscaled, ordered and
        // multi-register forms each carry their own contract, so they stay
        // refused below.
        "ldr" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: MemoryTransfer(AsmMemoryTransferKind::Load),
            target: Aarch64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "str" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: MemoryTransfer(AsmMemoryTransferKind::Store),
            target: Aarch64,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),

        // Recognize common memory-addressing spellings so they refuse for the
        // semantic reason, not as arbitrary unknown text. The list covers the
        // AArch64 width/signed/unscaled/unprivileged variants, the non-temporal
        // pair forms, the RCpc/limited-ordering acquire-release spellings, the
        // complete exclusive and LSE read-modify-write ordering grids, the
        // 64-byte accelerator block forms, the NEON structure load/store
        // spells, x86 exchange and compare-exchange forms, the implicit-operand
        // string and port-string instructions (bare and width-suffixed — the
        // `movsd`/`cmpsd` SSE scalar spellings stay unrecognized since those
        // mnemonics have a register-only form), the AT&T stack and flag-store
        // forms, the far-pointer loads, the xsave/fxsave state families, the
        // descriptor-table memory operands, the memory-destination
        // non-temporal stores, and frame setup — each always reads or writes
        // memory, so no spelling here is a register-only contract candidate.
        // Address-arithmetic (`lea`), ordering (`dmb`/`dsb`), cache/TLB
        // maintenance (`cl*`/`tlbi`/`ic`/`dc`), and SIMD register-only moves
        // do not access memory or belong to a different contract family, and
        // stay unrecognized rather than borrowing this refusal.
        "ldp" | "stp" | "ldnp" | "stnp" | "push" | "pop" | "pushq" | "popq" | "pushw" | "pushl"
        | "pushf" | "pushfd" | "pusha" | "pushal" | "pushad" | "popa" | "popal" | "popad"
        | "popw" | "popl" | "popf" | "popfd" | "enter" | "leave" | "ldrb" | "ldrh" | "ldrsb"
        | "ldrsh" | "ldrsw" | "strb" | "strh" | "ldur" | "stur" | "ldurb" | "ldurh" | "ldursb"
        | "ldursh" | "ldursw" | "sturb" | "sturh" | "ldtr" | "ldtrb" | "ldtrh" | "ldtrsb"
        | "ldtrsh" | "ldtrsw" | "sttr" | "sttrb" | "sttrh" | "ldapr" | "ldaprb" | "ldaprh"
        | "ldaprsb" | "ldaprsh" | "ldaprsw" | "ldapur" | "ldapurb" | "ldapurh" | "ldapursb"
        | "ldapursh" | "ldapursw" | "stlur" | "stlurb" | "stlurh" | "ldlar" | "ldlarb"
        | "ldlarh" | "stllr" | "stllrb" | "stllrh" | "ldxr" | "ldxrb" | "ldxrh" | "stxr"
        | "stxrb" | "stxrh" | "ldax" | "ldaxr" | "ldaxrb" | "ldaxrh" | "stlxr" | "stlxrb"
        | "stlxrh" | "ldxp" | "stxp" | "ldaxp" | "stlxp" | "ldar" | "ldarb" | "ldarh" | "stlr"
        | "stlrb" | "stlrh" | "ld64b" | "st64b" | "st64bv" | "st64bv0" | "ld1" | "st1" | "ld2"
        | "st2" | "ld3" | "st3" | "ld4" | "st4" | "ld1r" | "ld2r" | "ld3r" | "ld4r" | "swp"
        | "swpb" | "swph" | "swpa" | "swpal" | "swpl" | "swpab" | "swpah" | "swpalb" | "swpalh"
        | "swplb" | "swplh" | "cas" | "casb" | "cash" | "casa" | "casal" | "casl" | "casab"
        | "casah" | "caslb" | "caslh" | "casalb" | "casalh" | "casp" | "caspa" | "caspal"
        | "caspl" | "ldadd" | "ldaddb" | "ldaddh" | "ldadda" | "ldaddab" | "ldaddah" | "ldaddl"
        | "ldaddlb" | "ldaddlh" | "ldaddal" | "ldaddalb" | "ldaddalh" | "ldclr" | "ldclrb"
        | "ldclrh" | "ldclra" | "ldclrab" | "ldclrah" | "ldclrl" | "ldclrlb" | "ldclrlh"
        | "ldclral" | "ldclralb" | "ldclralh" | "ldeor" | "ldeorb" | "ldeorh" | "ldeora"
        | "ldeorab" | "ldeorah" | "ldeorl" | "ldeorlb" | "ldeorlh" | "ldeoral" | "ldeoralb"
        | "ldeoralh" | "ldset" | "ldsetb" | "ldseth" | "ldseta" | "ldsetab" | "ldsetah"
        | "ldsetl" | "ldsetlb" | "ldsetlh" | "ldsetal" | "ldsetalb" | "ldsetalh" | "ldsmax"
        | "ldsmaxb" | "ldsmaxh" | "ldsmaxa" | "ldsmaxab" | "ldsmaxah" | "ldsmaxl" | "ldsmaxlb"
        | "ldsmaxlh" | "ldsmaxal" | "ldsmaxalb" | "ldsmaxalh" | "ldsmin" | "ldsminb"
        | "ldsminh" | "ldsmina" | "ldsminab" | "ldsminah" | "ldsminl" | "ldsminlb" | "ldsminlh"
        | "ldsminal" | "ldsminalb" | "ldsminalh" | "ldumax" | "ldumaxb" | "ldumaxh" | "ldumaxa"
        | "ldumaxab" | "ldumaxah" | "ldumaxl" | "ldumaxlb" | "ldumaxlh" | "ldumaxal"
        | "ldumaxalb" | "ldumaxalh" | "ldumin" | "lduminb" | "lduminh" | "ldumina" | "lduminab"
        | "lduminah" | "lduminl" | "lduminlb" | "lduminlh" | "lduminal" | "lduminalb"
        | "lduminalh" | "xchg" | "xadd" | "cmpxchg" | "cmpxchg8b" | "cmpxchg16b" | "xlat"
        | "xlatb" | "lds" | "les" | "lss" | "lfs" | "lgs" | "sgdt" | "sidt" | "lgdt" | "movnti"
        | "movntq" | "movntdq" | "movntdqa" | "bound" | "fxsave" | "fxrstor" | "xsave"
        | "xsavec" | "xsaves" | "xsaveopt" | "xrstor" | "xrstors" | "movs" | "movsb" | "movsw"
        | "movsq" | "lods" | "lodsb" | "lodsw" | "lodsq" | "lodsd" | "stos" | "stosb" | "stosw"
        | "stosq" | "stosd" | "scas" | "scasb" | "scasw" | "scasq" | "scasd" | "cmps" | "cmpsb"
        | "cmpsw" | "cmpsq" | "ins" | "outs" | "insb" | "insw" | "insd" | "outsb" | "outsw"
        | "outsd" => Refused(UnmodeledMemoryAccess),
        _ => return None,
    };
    Some(entry)
}

#[cfg(test)]
mod tests;
