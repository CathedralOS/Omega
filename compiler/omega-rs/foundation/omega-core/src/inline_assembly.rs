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
    DerivedExit,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmOperandAccess {
    Read,
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

    pub const fn expected_type_name(self) -> &'static str {
        self.expected_type_name
    }

    pub const fn maximum_literal(self) -> Option<u64> {
        self.maximum_literal
    }

    pub const fn requires_writable_place(self) -> bool {
        matches!(self.access, AsmOperandAccess::Write)
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
const NO_CLOBBERS: &[&str] = &[];
const PORT_OUT_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r11"];
const PORT_IN_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r15"];

pub fn asm_catalog_entry(mnemonic: &str) -> Option<AsmCatalogEntry> {
    use AsmCatalogEntry::{Contract, Refused};
    use AsmAuthorityRequirement::{MachineOwner, None as NoAuthority, PortIo as PortIoAuthority};
    use AsmInstructionAvailability::{DeriverOnly, UserChecked};
    use AsmInstructionRefusal::{HiddenControlExit, UnmodeledMemoryAccess};
    use AsmFenceKind::{Full, Load, Store};
    use AsmInstructionShape::{
        DerivedExit, Halt, InterruptControl, JumpState, MemoryFence, PortIn, PortOut,
    };
    use AsmInterruptControlKind::{Disable, Enable};
    use AsmInterruptFlagEffect::{
        Disable as DisablesInterrupts, EnableAfterNextInstruction, None as NoInterruptChange,
    };
    use AsmMemoryOrdering::{Fence, None as NoOrdering};
    use AsmTargetApplicability::{Aarch64, Any, X86_64};

    let entry = match mnemonic {
        "jmp" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: JumpState,
            target: Any,
            required_authority: NoAuthority,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
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
            clobbers: NO_CLOBBERS,
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
        AsmAuthorityRequirement, AsmCatalogEntry, AsmFenceKind, AsmInstructionAvailability,
        AsmInstructionRefusal, AsmInterruptFlagEffect, AsmMemoryOrdering, AsmOperandAccess,
        AsmTargetApplicability, asm_catalog_entry,
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
        assert_eq!(out.clobbers, &["rax", "rdx", "r10", "r11"]);

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
