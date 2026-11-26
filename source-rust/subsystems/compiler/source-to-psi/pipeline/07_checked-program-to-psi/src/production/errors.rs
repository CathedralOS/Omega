use diagnostics::Diagnostic;

pub use terminal_codec::entry_receipt::ProgramEntryTerminalReceiptError;

#[derive(Debug)]
pub enum TerminalArtifactProductionError {
    BuildContext(Diagnostic),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    EntryReceipt(ProgramEntryTerminalReceiptError),
}

impl TerminalArtifactProductionError {
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        vec![self.diagnostic()]
    }

    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::BuildContext(error) => Some(error.clone()),
            _ => None,
        }
        .unwrap_or_else(|| {
            Diagnostic::error(format!("terminal-artifact production failed: {self}"))
        })
    }
}

diagnostics::debug_display!(TerminalArtifactProductionError);

impl std::error::Error for TerminalArtifactProductionError {}
