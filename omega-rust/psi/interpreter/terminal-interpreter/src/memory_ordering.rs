//! A receiving host can explicitly select the interpreter's private sequential
//! memory model. Every ordinary storage operation completes before the next
//! one; consequently each catalog fence's exact ordering requirements hold.
//! This supplies neither shared/device storage, synchronization evidence nor
//! native hardware authority. Unsupported models must supply their own checked
//! execution route rather than treating a missing fence as Unit success.

use terminal_psi::{MemoryFenceKind, RegisterTargetApplicability};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalMemoryOrderingEnvironment {
    target: RegisterTargetApplicability,
}

impl TerminalMemoryOrderingEnvironment {
    /// Select a model for an already explicit concrete deployment target.
    /// This is a receiving-host choice, never inferred from an instruction.
    pub fn private_sequential(target: &str) -> Option<Self> {
        [
            RegisterTargetApplicability::X86_64,
            RegisterTargetApplicability::Aarch64,
        ]
        .into_iter()
        .find(|architecture| architecture.accepts_target_name(target))
        .map(|target| Self { target })
    }

    pub(crate) fn accepts_target(self, target: &str) -> bool {
        self.target.accepts_target_name(target)
    }

    pub(crate) fn supports(self, kind: MemoryFenceKind) -> bool {
        self.target == kind.target()
    }
}
