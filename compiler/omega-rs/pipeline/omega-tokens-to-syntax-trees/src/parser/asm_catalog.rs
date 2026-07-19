//! Compiler-owned source-assembly catalog metadata.
//!
//! Parsing is intentionally target-agnostic today: target selection happens
//! after syntax construction. The catalog still distinguishes instructions
//! whose contracts are user-dischargeable from entry/exit operations reserved
//! for compiler derivation. Instructions with no complete contract are not
//! accepted catalog entries; recognized refusal classes let the parser explain
//! why memory and hidden-control spellings cannot cross the strict surface.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AsmInstructionAvailability {
    UserChecked,
    DeriverOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AsmInstructionShape {
    JumpState,
    Halt,
    PortOut,
    PortIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AsmInstructionContract {
    pub availability: AsmInstructionAvailability,
    pub shape: AsmInstructionShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AsmInstructionRefusal {
    /// A return, call, or indirect branch would bypass Omega state edges.
    HiddenControlExit,
    /// No provenance/permission-bearing operand contract exists yet.
    UnmodeledMemoryAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AsmCatalogEntry {
    Contract(AsmInstructionContract),
    Refused(AsmInstructionRefusal),
}

pub(super) fn asm_catalog_entry(mnemonic: &str) -> Option<AsmCatalogEntry> {
    use AsmCatalogEntry::{Contract, Refused};
    use AsmInstructionAvailability::{DeriverOnly, UserChecked};
    use AsmInstructionRefusal::{HiddenControlExit, UnmodeledMemoryAccess};
    use AsmInstructionShape::{Halt, JumpState, PortIn, PortOut};

    let entry = match mnemonic {
        "jmp" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: JumpState,
        }),
        "hlt" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: Halt,
        }),
        "out" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: PortOut,
        }),
        "in" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: PortIn,
        }),

        // These are real catalog operations, but only derived entry/exit
        // machinery may discharge their complete state-plan contracts.
        "iretq" | "sysret" | "sysretq" | "eret" => Contract(AsmInstructionContract {
            availability: DeriverOnly,
            shape: JumpState,
        }),

        // These spell control edges which cannot be represented by the current
        // source form. Direct state jumps use the checked `jmp state(...)` arm.
        "ret" | "retq" | "retaa" | "retab" | "call" | "callq" | "br" | "blr" => {
            Refused(HiddenControlExit)
        }

        // Recognize common target spellings so they refuse for the semantic
        // reason, not as arbitrary unknown text. `mov` is included because its
        // operand mode may access memory; a future structured-operand catalog
        // will distinguish its register-only form.
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
        AsmCatalogEntry, AsmInstructionAvailability, AsmInstructionRefusal, asm_catalog_entry,
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
