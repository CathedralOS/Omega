//! Call-bound placement obligations stay separate from physical transport.
//! The argument ordinal selects the call's existing source/path/ABI row. A
//! captured interpretation or caller formal relation answers the exact callee
//! demand; neither a static row nor correspondence mints an invocation loan.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAccessCallBinding {
    pub input: terminal_psi::TerminalAccessViewInput,
    pub argument_index: u32,
    pub association: TargetAccessCallAssociation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAccessCallAssociation {
    /// Replayed from the current descriptor, including its original backing.
    Captured {
        schema: semantic_vocabulary::StructuralTypeId,
        carrier: semantic_vocabulary::StructuralTypeId,
    },
    /// A symbolic invocation obligation; every actual incoming call must
    /// independently answer this row. It is not a concrete source residence.
    CallerFormal(terminal_psi::TerminalAccessViewInput),
}

/// Native descriptor replay keeps the interpreted backing independently from
/// a symbolic activation obligation. Neither branch chooses runtime pointers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAccessInterpretation {
    Captured(abstract_operations::AccessReferenceBacking),
    Formal(terminal_psi::TerminalAccessViewInput),
}
