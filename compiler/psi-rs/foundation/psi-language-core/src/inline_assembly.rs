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
    MemoryFence(AsmFenceKind),
    InterruptControl(AsmInterruptControlKind),
    FlagsSnapshot,
    FlagsRestore,
    MsrRead,
    MsrWrite,
    ControlRegisterRead(AsmControlRegister),
    ControlRegisterWrite(AsmControlRegister),
    DescriptorTableLoad,
    DerivedExit,
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
const NO_CLOBBERS: &[&str] = &[];
const PORT_OUT_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r11", "r15"];
const PORT_IN_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r15"];
const FLAGS_OPERAND_CLOBBERS: &[&str] = &["r10", "r15"];
const MSR_READ_CLOBBERS: &[&str] = &["rax", "rcx", "rdx", "r10", "r11", "r15"];
const MSR_WRITE_CLOBBERS: &[&str] = &["rax", "rcx", "rdx", "r10", "r11", "r15"];
const CONTROL_REGISTER_READ_CLOBBERS: &[&str] = &["r10", "r15"];
const CONTROL_REGISTER_WRITE_CLOBBERS: &[&str] = &["rax", "r10", "r11", "r15"];
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
    use AsmInstructionShape::{
        DerivedExit, DescriptorTableLoad, FlagsRestore, FlagsSnapshot, Halt, InterruptControl,
        JumpState, MemoryFence, MsrRead, MsrWrite, PortIn, PortOut,
    };
    use AsmInterruptControlKind::{Disable, Enable};
    use AsmInterruptFlagEffect::{
        Disable as DisablesInterrupts, EnableAfterNextInstruction, None as NoInterruptChange,
        RestoreFromOperand as RestoreInterruptFlag,
    };
    use AsmMemoryOrdering::{Fence, None as NoOrdering};
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
        "ret" | "retq" | "retaa" | "retab" | "call" | "callq" | "br" | "blr" => {
            Refused(HiddenControlExit)
        }

        // Recognize common target spellings so they refuse for the semantic
        // reason, not as arbitrary unknown text. `mov` is included because its
        // operand mode may access memory; structured operand decoding will
        // eventually distinguish its register-only form.
        "mov" | "movq" | "ldr" | "str" | "ldp" | "stp" | "push" | "pop" => {
            Refused(UnmodeledMemoryAccess)
        }
        _ => return None,
    };
    Some(entry)
}

#[cfg(test)]
mod tests {
    use super::{
        AsmAuthorityRequirement, AsmCatalogEntry, AsmControlRegister, AsmFenceKind,
        AsmFlagsDataFlow, AsmInstructionAvailability, AsmInstructionRefusal, AsmInstructionShape,
        AsmInterruptFlagEffect, AsmMemoryOrdering, AsmOperandAccess, AsmTargetApplicability,
        asm_catalog_entry,
    };

    #[test]
    fn catalog_separates_user_and_deriver_availability() {
        let AsmCatalogEntry::Contract(hlt) = asm_catalog_entry("hlt").expect("hlt contract") else {
            panic!("hlt must be a contracted instruction");
        };
        assert_eq!(hlt.availability, AsmInstructionAvailability::UserChecked);
        assert_eq!(
            hlt.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );

        let AsmCatalogEntry::Contract(iretq) = asm_catalog_entry("iretq").expect("iretq contract")
        else {
            panic!("iretq must be a contracted instruction");
        };
        assert_eq!(iretq.availability, AsmInstructionAvailability::DeriverOnly);

        let AsmCatalogEntry::Contract(lidt) = asm_catalog_entry("lidt").expect("lidt contract")
        else {
            panic!("lidt must be a contracted instruction");
        };
        assert_eq!(lidt.availability, AsmInstructionAvailability::DeriverOnly);
        assert_eq!(lidt.shape, AsmInstructionShape::DescriptorTableLoad);
        assert_eq!(lidt.required_authority, AsmAuthorityRequirement::IdtControl);
        assert_eq!(lidt.operands.len(), 1);
        assert!(lidt.operands[0].requires_place());
        assert_eq!(lidt.operands[0].target_register, "r10");
        assert_eq!(lidt.clobbers, &["r10"]);
    }

    #[test]
    fn port_contracts_pin_operands_and_realized_clobbers() {
        let AsmCatalogEntry::Contract(out) = asm_catalog_entry("out").expect("out contract") else {
            panic!("out must be a contracted instruction");
        };
        assert_eq!(out.target, AsmTargetApplicability::X86_64);
        assert_eq!(out.required_authority, AsmAuthorityRequirement::PortIo);
        assert_eq!(
            out.operands
                .iter()
                .map(|operand| (
                    operand.role,
                    operand.target_register,
                    operand.access,
                    operand.expected_type_name,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("port", "dx", AsmOperandAccess::Read, "u16"),
                ("value", "al", AsmOperandAccess::Read, "u8"),
            ]
        );
        assert_eq!(out.clobbers, &["rax", "rdx", "r10", "r11", "r15"]);

        let AsmCatalogEntry::Contract(input) = asm_catalog_entry("in").expect("in contract") else {
            panic!("in must be a contracted instruction");
        };
        assert_eq!(
            input
                .operands
                .iter()
                .map(|operand| (
                    operand.role,
                    operand.target_register,
                    operand.access,
                    operand.expected_type_name,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("destination", "al", AsmOperandAccess::Write, "u8"),
                ("port", "dx", AsmOperandAccess::Read, "u16"),
            ]
        );
        assert_eq!(input.clobbers, &["rax", "rdx", "r10", "r15"]);
    }

    #[test]
    fn flags_contracts_are_explicit_and_stack_balanced_by_lowering() {
        let AsmCatalogEntry::Contract(snapshot) =
            asm_catalog_entry("pushfq").expect("pushfq contract")
        else {
            panic!("pushfq must be contracted");
        };
        assert_eq!(snapshot.required_authority, AsmAuthorityRequirement::None);
        assert_eq!(
            snapshot.flags_data_flow,
            AsmFlagsDataFlow::SnapshotToOperand
        );
        assert_eq!(snapshot.operands[0].access, AsmOperandAccess::Write);
        assert_eq!(snapshot.operands[0].expected_type_name, "u64");
        assert_eq!(snapshot.clobbers, &["r10", "r15"]);

        let AsmCatalogEntry::Contract(restore) =
            asm_catalog_entry("popfq").expect("popfq contract")
        else {
            panic!("popfq must be contracted");
        };
        assert_eq!(
            restore.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(
            restore.flags_data_flow,
            AsmFlagsDataFlow::RestoreFromOperand
        );
        assert_eq!(
            restore.interrupt_flag_effect,
            AsmInterruptFlagEffect::RestoreFromOperand
        );
        assert_eq!(restore.operands[0].access, AsmOperandAccess::ReadPlace);
        assert_eq!(restore.clobbers, &["r10", "r15"]);
    }

    #[test]
    fn msr_contracts_pin_structured_value_flow_and_machine_authority() {
        let AsmCatalogEntry::Contract(read) = asm_catalog_entry("rdmsr").expect("rdmsr contract")
        else {
            panic!("rdmsr must be contracted");
        };
        assert_eq!(read.target, AsmTargetApplicability::X86_64);
        assert_eq!(
            read.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(read.operands[0].access, AsmOperandAccess::Write);
        assert_eq!(read.operands[0].target_register, "edx:eax");
        assert_eq!(read.operands[1].expected_type_name, "u32");
        assert_eq!(read.clobbers, &["rax", "rcx", "rdx", "r10", "r11", "r15"]);

        let AsmCatalogEntry::Contract(write) = asm_catalog_entry("wrmsr").expect("wrmsr contract")
        else {
            panic!("wrmsr must be contracted");
        };
        assert_eq!(
            write.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(write.operands[0].target_register, "ecx");
        assert_eq!(write.operands[1].expected_type_name, "u64");
        assert_eq!(write.clobbers, &["rax", "rcx", "rdx", "r10", "r11", "r15"]);
    }

    #[test]
    fn control_register_contracts_pin_exact_u64_flow_and_machine_authority() {
        for register in [
            AsmControlRegister::Cr0,
            AsmControlRegister::Cr2,
            AsmControlRegister::Cr3,
            AsmControlRegister::Cr4,
        ] {
            let AsmCatalogEntry::Contract(read) = asm_catalog_entry(register.read_mnemonic())
                .expect("control-register read contract")
            else {
                panic!("control-register read must be contracted");
            };
            assert_eq!(
                read.shape,
                AsmInstructionShape::ControlRegisterRead(register)
            );
            assert_eq!(read.target, AsmTargetApplicability::X86_64);
            assert_eq!(
                read.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(read.operands[0].access, AsmOperandAccess::Write);
            assert_eq!(read.operands[0].target_register, register.name());
            assert_eq!(read.operands[0].expected_type_name, "u64");
            assert_eq!(read.clobbers, &["r10", "r15"]);
        }

        for register in [
            AsmControlRegister::Cr0,
            AsmControlRegister::Cr3,
            AsmControlRegister::Cr4,
        ] {
            let mnemonic = register
                .write_mnemonic()
                .expect("writable control register");
            let AsmCatalogEntry::Contract(write) =
                asm_catalog_entry(mnemonic).expect("control-register write contract")
            else {
                panic!("control-register write must be contracted");
            };
            assert_eq!(
                write.shape,
                AsmInstructionShape::ControlRegisterWrite(register)
            );
            assert_eq!(
                write.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(write.operands[0].access, AsmOperandAccess::Read);
            assert_eq!(write.operands[0].target_register, register.name());
            assert_eq!(write.operands[0].expected_type_name, "u64");
            assert_eq!(write.clobbers, &["rax", "r10", "r11", "r15"]);
        }

        assert_eq!(asm_catalog_entry("write_cr2"), None);
    }

    #[test]
    fn catalog_names_semantic_refusal_classes() {
        assert_eq!(
            asm_catalog_entry("ret"),
            Some(AsmCatalogEntry::Refused(
                AsmInstructionRefusal::HiddenControlExit
            ))
        );
        assert_eq!(
            asm_catalog_entry("ldr"),
            Some(AsmCatalogEntry::Refused(
                AsmInstructionRefusal::UnmodeledMemoryAccess
            ))
        );
        assert_eq!(asm_catalog_entry("db"), None);
    }

    #[test]
    fn fence_contracts_pin_ordering_without_invented_clobbers() {
        for (mnemonic, kind) in [
            ("lfence", AsmFenceKind::Load),
            ("sfence", AsmFenceKind::Store),
            ("mfence", AsmFenceKind::Full),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("fence contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(contract.memory_ordering, AsmMemoryOrdering::Fence(kind));
            assert_eq!(contract.target, AsmTargetApplicability::X86_64);
            assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }
    }

    #[test]
    fn interrupt_control_contracts_pin_authority_and_delayed_sti_semantics() {
        for (mnemonic, flag_effect) in [
            ("cli", AsmInterruptFlagEffect::Disable),
            ("sti", AsmInterruptFlagEffect::EnableAfterNextInstruction),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("interrupt-control contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(contract.target, AsmTargetApplicability::X86_64);
            assert_eq!(
                contract.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(contract.interrupt_flag_effect, flag_effect);
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }
    }
}
