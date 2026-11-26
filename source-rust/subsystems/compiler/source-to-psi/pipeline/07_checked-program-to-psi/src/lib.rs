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
mod build;
mod production;
pub mod provider_choice_inputs;
pub mod provider_supply;
mod source;
#[cfg(test)]
use lowered_program_to_checked_program as checking;
use lowered_source_publication::source_roster::SourceCompilationRoster;
#[cfg(test)]
use prepared_typed_trees_to_lowered_program as lowering;
#[cfg(test)]
use source_admission::LoadedSource;
use source_declarations::declaration_selection;
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
use source_symbol_table::{source_inputs, source_symbols};
#[cfg(test)]
use symbol_resolved_trees_to_typed_trees as typing;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
use typed_trees::source_state;
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

#[cfg(test)]
pub(crate) use production::accepted::DeclaredRequirementApplication;

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
pub use production::accepted::{
    AcceptedTerminalSource, ConstantRead, DeclaredBoundaryRequirement, PreparedTerminalPublication,
    boundary_requirements_declared_for,
};
pub use production::invocation_site::{CheckedSourceCall, SourceInvocationSite};
pub use terminal_check_tables::program::SuppliedRequirement as TerminalSuppliedRequirement;
pub use terminal_semantics::callable_values::CallableBoundaryImplementation;

pub(crate) use build::bindings::{build_filesystem, build_provider, build_queries, build_requests};
pub use build::receiver::CheckedBuildReceiver;
pub use source::publication::SourcePublication;
pub use source::selections::TerminalSourceSelections;

pub use build::bindings::{
    ProviderSelectionOption, TerminalBuildBindingCall, TerminalBuildBindingOperand,
    TerminalBuildBindingOperation, TerminalBuildBindings, TerminalBuildNativeLocator,
    TerminalBuildNativeLocatorKind, TerminalBuildProviderRequirement,
    TerminalBuildSelectionRequest, TerminalBuildValueCase, TerminalBuildValueField,
    TerminalBuildValueShape,
};
pub use build_filesystem::{
    TerminalBuildFilesystem, TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation,
};
pub use build_provider::{BuildProviderDeclaration, ProducedBuildProvider};
pub use build_queries::{TerminalBuildQueryCall, TerminalBuildQueryOperation};
pub use build_requests::{
    BuildCallSite, TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildRequests,
};
pub use production::application::ProducedTerminalApplication;
pub use production::custody::{ProducedTerminalProgram, TerminalProductionCustody};
pub use production::errors::{ProgramEntryTerminalReceiptError, TerminalArtifactProductionError};
pub use production::selections::{TerminalMachineSelection, TerminalProgramRoots};
pub use terminal_codec::entry_receipt::application::{
    TerminalAdmissionRoot, TerminalApplicationCustody, TerminalCompositionRoots,
};
pub use terminal_psi::PreparedTerminalRoots;
