/*
Terminal production retains the new checker's actual accepted closure, its frozen
typed owner and the exact artifact/evidence pair. AcceptedTerminalSource is the
production entrance; invocation identity is fixed before lowering and judgment.

Retained Build companions and application receipts
describe the data a producer must provide; none manufactures checking authority.
The obsolete checked-source production request interfaces have been removed.
*/

#![forbid(unsafe_code)]

mod accepted;
#[cfg(test)]
pub(crate) use accepted::DeclaredRequirementApplication;

mod invocation_site;
pub use invocation_site::{CheckedSourceCall, SourceInvocationSite};
mod application;
pub use crate::source_state::trait_definition::Supply as TerminalSourceSupply;
pub use lowered_source_publication::Compilation as TerminalSourceCompilation;
pub use lowered_source_publication::provider_selections::{
    ProviderSelectionInput as TerminalProviderSelectionInput,
    ProviderSelections as TerminalProviderSelections,
};
pub use lowered_source_publication::{
    CallingPolicySelections as TerminalCallingPolicySelections,
    ProgramEntryContractSelection as TerminalProgramEntryContractSelection,
};
pub use terminal_check_tables::program::SuppliedRequirement as TerminalSuppliedRequirement;
pub use accepted::{
    AcceptedTerminalSource, ConstantRead, DeclaredBoundaryRequirement, PreparedTerminalPublication,
};
pub use terminal_semantics::callable_values::CallableBoundaryImplementation;

mod build_bindings;
mod build_companions;
mod build_receiver;
pub use build_receiver::CheckedBuildReceiver;
pub(crate) use build_bindings::{build_filesystem, build_provider, build_queries, build_requests};
mod custody;
mod published_entry;
pub(crate) mod source_selection_publication;
mod source_selections;
pub use source_selections::TerminalSourceSelections;
pub(crate) mod production_selection {
    #[derive(Debug, Clone, Copy)]
    pub enum TerminalMachineSelection<'a> {
        Declarations,
        Name(&'a str),
        Symbol(symbols::SymbolHandle),
    }

    #[derive(Clone, Copy, Default)]
    pub struct TerminalProgramRoots<'a> {
        pub machines: &'a [symbols::SymbolHandle],
        pub boundaries: &'a [symbols::SymbolHandle],
    }
}
pub(crate) mod terminal_production {
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
}

pub use self::terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError,
};
pub use application::ProducedTerminalApplication;
pub use build_bindings::{
    ProviderSelectionOption, TerminalBuildBindingCall, TerminalBuildBindingOperand,
    TerminalBuildBindingOperation, TerminalBuildBindings, TerminalBuildNativeLocator,
    TerminalBuildProviderRequirement, TerminalBuildSelectionRequest, TerminalBuildValueCase,
    TerminalBuildValueField, TerminalBuildValueShape,
};
pub use build_companions::BuildProtocolUse;
pub use build_filesystem::{
    TerminalBuildFilesystem, TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation,
};
pub use build_provider::{BuildProviderDeclaration, ProducedBuildProvider};
pub use build_queries::{TerminalBuildQueryCall, TerminalBuildQueryOperation};
pub use build_requests::{
    BuildCallSite, TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildRequests,
};
pub use custody::{ProducedTerminalProgram, TerminalProductionCustody};
pub use production_selection::{TerminalMachineSelection, TerminalProgramRoots};
pub use terminal_codec::entry_receipt::application::{
    TerminalAdmissionRoot, TerminalApplicationCustody, TerminalCompositionRoots,
};
pub use terminal_psi::PreparedTerminalRoots;

#[cfg(test)]
use crate::frontend_fixture;
