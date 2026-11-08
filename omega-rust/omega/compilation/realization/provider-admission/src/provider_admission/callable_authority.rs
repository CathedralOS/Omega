//! Native admission's immutable callable-closure witness.
//!
//! The public closure receipt is free descriptive data. This carrier is minted
//! only by actual provider admission after its complete closure review, beside
//! the original canonical artifact and selected entry. It grants no installed
//! entry, runtime capability or historical era. Receivers independently replay
//! its leaves under their own physical and optional permission policy.

#[derive(Debug, Clone)]
pub struct AdmittedNativeCallableAuthority {
    subject: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    review: effects::TerminalAuthorityClosureReviewReceipt,
}
impl AdmittedNativeCallableAuthority {
    pub(super) fn from_admission(
        subject: terminal_psi::TerminalPsiIdentity,
        entry: semantic_vocabulary::MachineId,
        review: effects::TerminalAuthorityClosureReviewReceipt,
    ) -> Self {
        Self {
            subject,
            entry,
            review,
        }
    }
    pub const fn subject(&self) -> terminal_psi::TerminalPsiIdentity {
        self.subject
    }
    pub const fn entry(&self) -> semantic_vocabulary::MachineId {
        self.entry
    }
    pub const fn review(&self) -> &effects::TerminalAuthorityClosureReviewReceipt {
        &self.review
    }
}
