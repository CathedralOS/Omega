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

/// One source file of a unit-test program, admitted at the base stratum.
#[cfg(test)]
struct FixtureFile<'a> {
    path: &'a str,
    text: &'a str,
    root: &'a str,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    origin: source_admission::SourceOrigin,
    scope: source_provenance::DependencyScope,
}

/// Admit, lex, parse, resolve and type the files in order. `bindings` receives
/// each file's source id and returns the scoped top-level bindings to resolve.
#[cfg(test)]
fn fixture_trees(
    files: &[FixtureFile<'_>],
    bindings: impl FnOnce(&[source_text::SourceId]) -> Vec<source_symbols::SourceScopedTopLevelBinding>,
) -> (
    typed_trees::source_state::TypedTrees,
    Vec<source_text::SourceId>,
) {
    let mut sources = source_inputs::SourceMap::default();
    let mut syntax = None;
    let mut ids = Vec::with_capacity(files.len());
    for file in files {
        let id = sources
            .add_checked_instance(
                file.path.into(),
                file.text,
                file.root.into(),
                file.package,
                file.origin,
                source_admission::SourceResolutionStratum::Base,
                file.scope,
            )
            .source_id;
        ids.push(id);
        let tokens = frontend_fixture::Lexer::new(file.text).tokenize().unwrap();
        match &mut syntax {
            None => {
                syntax = Some(frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap())
            }
            Some(syntax) => {
                frontend_fixture::parse(syntax, id, &tokens).unwrap();
            }
        }
    }
    let mut request = resolution::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(std::sync::Arc::new(sources));
    request.top_level_bindings = bindings(&ids);
    let resolved = resolution::resolve(request).unwrap();
    (typing::lower_symbol_resolved_trees(&resolved).unwrap(), ids)
}

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

/// Judge a prepared program whose only entry is the realized machine `entry`.
#[cfg(test)]
fn judge_entry(
    program: lowering::PreparedTypedTrees,
    entry: &str,
    subjects: &[Option<semantic_vocabulary::PackageKeyIdentity>],
    purpose: source_provenance::DependencyScope,
    publication: terminal_codec::TerminalPublication,
) -> Result<AcceptedTerminalSource, Vec<diagnostics::Diagnostic>> {
    let entry = program
        .trees()
        .realized_machine_named(entry)
        .unwrap()
        .symbol;
    judge_fixture(
        program,
        lowering::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects,
            purpose,
            selected: &[],
            bindings: &[],
            calling: None,
            providers: None,
            supplies: &[],
        },
        publication,
    )
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
