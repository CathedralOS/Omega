/*
Terminal production retains the new checker's actual accepted closure, its frozen
typed owner and the exact artifact/evidence pair. AcceptedTerminalSource is the
production entrance; invocation identity is fixed before lowering and judgment.

Retained Build companions and application receipts
describe the data a producer must provide; none manufactures checking authority.
The obsolete checked-source production request interfaces have been removed.
*/

#![forbid(unsafe_code)]

// Crate-wide aliases for sibling stages and the shared test fixtures.
mod package_review;
mod source_publication;
pub use source_publication::SourcePublication;
pub mod provider_choice_inputs;
pub mod provider_supply;
#[cfg(test)]
use lowered_program_to_checked_program as checking;
#[cfg(test)]
use source_admission::LoadedSource;
use source_declarations::declaration_selection;
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
use lowered_source_publication::source_roster::SourceCompilationRoster;
use source_symbol_table::{source_inputs, source_symbols};
#[cfg(test)]
use symbol_resolved_trees_to_typed_trees as typing;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
use typed_trees::source_state;
#[cfg(test)]
use prepared_typed_trees_to_lowered_program as lowering;
#[cfg(test)]
use typed_trees_to_prepared_typed_trees as preparation;
#[cfg(test)]
#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

#[cfg(test)]
fn judge_fixture(
    source: lowering::PreparedTypedTrees,
    compilation: lowering::Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<AcceptedTerminalSource, Vec<diagnostics::Diagnostic>> {
    let lowered = lowering::lower_for_publication(source, compilation, publication)?;
    let (judgment, _) = checking::judge_publication(lowered, Default::default())?;
    AcceptedTerminalSource::from_judgment(judgment)
}

mod accepted;
#[cfg(test)]
pub(crate) use accepted::DeclaredRequirementApplication;

mod invocation_site;
pub use invocation_site::{CheckedSourceCall, SourceInvocationSite};
mod application;
pub use crate::source_state::trait_definition::Supply as TerminalSourceSupply;
pub use accepted::{
    AcceptedTerminalSource, ConstantRead, DeclaredBoundaryRequirement, PreparedTerminalPublication,
    boundary_requirements_declared_for,
};
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
