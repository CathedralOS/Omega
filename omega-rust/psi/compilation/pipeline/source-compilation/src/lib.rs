//! The coordinator owns the active source request. Each source page streams through
//! lexing and parsing in `source_parsing`. Resolution and typing share a
//! continuation in `source_compilation` so Build can activate further source.
//!
//! `SourcePreparationRequest::judge` evaluates the requested work, prepares
//! the typed program and runs the lowered-form checker. The accepted-source
//! factory in the Psi publication stage binds that judgment to its source.
//! `provider_supply` closes the logical choices that portable publication
//! carries to target planning.

#![forbid(unsafe_code)]

pub use checked_terminal_psi_to_published_psi::provider_choice_inputs;
pub use source_declarations::{
    declaration_selection, source_inputs, source_symbols, source_tables,
};
#[cfg(any(test, feature = "test-support"))]
pub use symbol_resolved_trees::source_resolved;
pub use typed_trees::source_state;
pub use typed_trees::{
    constrained_type_reference_pair, digest_commitment, executable_leaf_expression_pattern,
    literal_expression_pattern, spelled_literal_expression_pattern,
};

pub mod evaluation;

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use checked_terminal_psi_to_published_psi::production;
#[cfg(any(test, feature = "test-support"))]
pub use checked_terminal_psi_to_published_psi::production;
#[cfg(any(test, feature = "test-support"))]
pub use source_lexer as lexing;
#[cfg(any(test, feature = "test-support"))]
pub use source_parser as parsing;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use symbol_resolved_trees_to_typed_trees::typing;
#[cfg(any(test, feature = "test-support"))]
pub use symbol_resolved_trees_to_typed_trees::typing;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use syntax_trees_to_symbol_resolved_trees::resolution;
#[cfg(any(test, feature = "test-support"))]
pub use syntax_trees_to_symbol_resolved_trees::resolution;
#[cfg(any(test, feature = "test-support"))]
pub use terminal_psi_to_checked_terminal_psi::checking;
#[cfg(any(test, feature = "test-support"))]
pub use typed_trees_to_terminal_psi::lowering;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use typed_trees_to_typed_trees::preparation;
#[cfg(any(test, feature = "test-support"))]
pub use typed_trees_to_typed_trees::preparation;

pub use checked_terminal_psi_to_published_psi::SourcePublication;

mod source_supply;
pub use source_supply::{
    ChosenDefaultLayout, ProductSupplySource, SourceSupplyTarget, SuppliedProduct, SupplyJudgment,
    TargetSupplySource, judge_supplied_product, prepare_supplied_target,
};

mod source_preparation;
pub use source_preparation::SourcePreparation;

pub use checked_terminal_psi_to_published_psi::source_roster::{
    SourceCallingContext, SourceCallingInputs, SourceCompilationRoster, SourceNormalizationInputs,
    SourceProgramEntryInput,
};

mod product_descriptions;
mod root_bindings;
mod source_checking;
mod source_compilation;
mod source_syntax;
pub use source_syntax::{
    ParsedSourceBatch, ParsedSourceSummary, RetainedSourceParse, SourceDeclarations, SourceImport,
    SourceInput, SourceItems, SourceSyntax,
};
mod build_stages;
pub(crate) mod source_parsing;
mod target_helpers;

pub use ::source_syntax::{ParsedText, SourceParseError, parse_text};
pub use source_checking::{
    FrozenSource, JudgedSource, ProductRepresentationInput, SourceAdmission,
    SourcePreparationFailure, SourcePreparationRequest,
};

pub use source_compilation::{
    AdmittedProductPolicy, CompletedSource, PreparedSource, ProductPolicyRequest,
    SourceCompilationRequest, SourceContinuation, SourceExtensionRequest, SourceFrontends,
};

pub use crate::production::{
    AcceptedTerminalSource, BuildCallSite, CallableBoundaryImplementation, ConstantRead,
    DeclaredBoundaryRequirement, PortableTerminalProgram, PreparedTerminalRoots,
    ProducedBuildProvider, ProducedTerminalApplication, ProducedTerminalProgram,
    ProgramEntryTerminalReceiptError, PsiCompilationObserver, PsiCompilationStage,
    RequirementReviewMetadata, TerminalAdmissionRoot, TerminalApplicationCustody,
    TerminalArtifactProductionError, TerminalBuildBindingCall, TerminalBuildBindingOperation,
    TerminalBuildBindings, TerminalBuildFilesystem, TerminalBuildFilesystemCall,
    TerminalBuildFilesystemOperation, TerminalBuildNativeLocator, TerminalBuildProviderRequirement,
    TerminalBuildQueryCall, TerminalBuildQueryOperation, TerminalBuildRequestCall,
    TerminalBuildRequestOperation, TerminalBuildRequests, TerminalBuildSelectionRequest,
    TerminalBuildValueCase, TerminalBuildValueField, TerminalBuildValueShape,
    TerminalCallingPolicySelections, TerminalCompositionRoots, TerminalMachineSelection,
    TerminalProductionCustody, TerminalProgramEntryContractSelection, TerminalProgramRoots,
    TerminalProviderSelectionInput, TerminalProviderSelections, TerminalSourceCompilation,
    TerminalSourceSupply, TerminalSuppliedRequirement, UnmeasuredProduction,
};

/// Logical provider closure; native planning consumes only its accepted result.
pub use checked_terminal_psi_to_published_psi::provider_supply;

pub mod source_provider_choices;

pub mod entry_contracts;
pub mod entry_shapes;
pub mod product_source;
pub mod source_targets;
mod typed_frontier;
pub use typed_frontier::TypedFrontier;

mod unbuilt_source;
pub use unbuilt_source::{UnbuiltSourceInputs, prepare_unbuilt_frontier};

/// Inspect preparation in counterexample tests without granting acceptance.
#[cfg(any(test, feature = "test-support"))]
pub fn prepare_source_for_test(
    request: SourcePreparationRequest<'_>,
) -> Result<FrozenSource, SourcePreparationFailure> {
    request.prepare()
}

#[cfg(test)]
extern crate self as source_compiler;
#[cfg(test)]
#[path = "../../../../../../tests/support/frontend.rs"]
mod frontend_fixture;
