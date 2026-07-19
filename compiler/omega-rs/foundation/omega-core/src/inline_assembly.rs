//! Compiler-owned source-assembly catalog metadata.
//!
//! Source parsing is target-agnostic today because target selection follows
//! syntax construction. This catalog still makes the accepted instruction
//! shape, operand constraints, availability, and register clobbers explicit.
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
    DerivedExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmOperandConstraint {
    /// x86 `DX`: an exact `u16` place or a literal in `0..=65535`.
    PortU16,
    /// x86 `AL`: an exact `u8` place or a literal in `0..=255`.
    ByteU8,
    /// A mutable place whose declared type is exactly `u8`.
    WritableBytePlace,
}

impl AsmOperandConstraint {
    pub const fn expected_type_name(self) -> &'static str {
        match self {
            Self::PortU16 => "u16",
            Self::ByteU8 | Self::WritableBytePlace => "u8",
        }
    }

    pub const fn maximum_literal(self) -> Option<u64> {
        match self {
            Self::PortU16 => Some(u16::MAX as u64),
            Self::ByteU8 => Some(u8::MAX as u64),
            Self::WritableBytePlace => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsmInstructionContract {
    pub availability: AsmInstructionAvailability,
    pub shape: AsmInstructionShape,
    /// Source-order operands. These are target-register constraints, not
    /// permissive numeric coercions.
    pub operands: &'static [AsmOperandConstraint],
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
const PORT_OUT_OPERANDS: &[AsmOperandConstraint] =
    &[AsmOperandConstraint::PortU16, AsmOperandConstraint::ByteU8];
const PORT_IN_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::WritableBytePlace,
    AsmOperandConstraint::PortU16,
];
const NO_CLOBBERS: &[&str] = &[];
const PORT_OUT_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r11"];
const PORT_IN_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r15"];

pub fn asm_catalog_entry(mnemonic: &str) -> Option<AsmCatalogEntry> {
    use AsmCatalogEntry::{Contract, Refused};
    use AsmInstructionAvailability::{DeriverOnly, UserChecked};
    use AsmInstructionRefusal::{HiddenControlExit, UnmodeledMemoryAccess};
    use AsmInstructionShape::{DerivedExit, Halt, JumpState, PortIn, PortOut};

    let entry = match mnemonic {
        "jmp" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: JumpState,
            operands: NO_OPERANDS,
            clobbers: NO_CLOBBERS,
        }),
        "hlt" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: Halt,
            operands: NO_OPERANDS,
            clobbers: NO_CLOBBERS,
        }),
        "out" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: PortOut,
            operands: PORT_OUT_OPERANDS,
            clobbers: PORT_OUT_CLOBBERS,
        }),
        "in" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: PortIn,
            operands: PORT_IN_OPERANDS,
            clobbers: PORT_IN_CLOBBERS,
        }),

        // These are real catalog operations, but only derived entry/exit
        // machinery may discharge their complete state-plan contracts.
        "iretq" | "sysret" | "sysretq" | "eret" => Contract(AsmInstructionContract {
            availability: DeriverOnly,
            shape: DerivedExit,
            operands: NO_OPERANDS,
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
        AsmCatalogEntry, AsmInstructionAvailability, AsmInstructionRefusal, AsmOperandConstraint,
        asm_catalog_entry,
    };

    #[test]
    fn catalog_separates_user_and_deriver_availability() {
        let AsmCatalogEntry::Contract(hlt) = asm_catalog_entry("hlt").expect("hlt contract") else {
            panic!("hlt must be a contracted instruction");
        };
        assert_eq!(hlt.availability, AsmInstructionAvailability::UserChecked);

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
        assert_eq!(
            out.operands,
            &[AsmOperandConstraint::PortU16, AsmOperandConstraint::ByteU8]
        );
        assert_eq!(out.clobbers, &["rax", "rdx", "r10", "r11"]);

        let AsmCatalogEntry::Contract(input) = asm_catalog_entry("in").expect("in contract") else {
            panic!("in must be a contracted instruction");
        };
        assert_eq!(
            input.operands,
            &[
                AsmOperandConstraint::WritableBytePlace,
                AsmOperandConstraint::PortU16
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
}
