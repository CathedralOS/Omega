pub use checked_program_to_psi::provider_choice_inputs;
pub use source_declarations::declaration_selection;
pub use source_symbol_table::{source_inputs, source_symbols, source_tables};
#[cfg(any(test, feature = "test-support"))]
pub use symbol_resolved_trees::source_resolved;
pub use typed_trees::source_state;
pub use symbol_resolved_trees_to_typed_trees::product_declarations::RootBinding;
pub use typed_trees_to_typed_trees::preparation::WireCompatibilityDemand;
pub use typed_trees::{
    constrained_type_reference_pair, digest_commitment, executable_leaf_expression_pattern,
    literal_expression_pattern, spelled_literal_expression_pattern,
};

#[path = "evaluation.rs"]
pub mod evaluation;

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use checked_program_to_psi::production;
#[cfg(any(test, feature = "test-support"))]
pub use checked_program_to_psi::production;
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
pub use lowered_program_to_checked_program::checking;
#[cfg(any(test, feature = "test-support"))]
pub use typed_trees_to_lowered_program::lowering;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use typed_trees_to_typed_trees::preparation;
#[cfg(any(test, feature = "test-support"))]
pub use typed_trees_to_typed_trees::preparation;

#[path = "prepare_and_check/source_judgment.rs"]
mod source_judgment;
pub use source_judgment::{judge_source_roster, judge_terminal_source, SourcePublication};

#[path = "prepare_and_check/provider_settlement.rs"]
pub(crate) mod source_supply;
pub use source_supply::{
    ChosenDefaultLayout, ProductSupplySource, SourceSupplyTarget, SuppliedProduct, SupplyJudgment,
    TargetSupplySource, judge_supplied_product, prepare_supplied_target,
};

pub use typed_trees_to_lowered_program::lowering::source_roster::{
    SourceCallingContext, SourceCallingInputs, SourceCompilationRoster, SourceNormalizationInputs,
    SourceProgramEntryInput,
};

#[path = "04_typed-trees-to-typed-trees/prepare_for_checking.rs"]
pub(crate) mod source_checking;
#[path = "03_symbol-resolved-trees-to-typed-trees/resolve_and_type.rs"]
pub(crate) mod source_compilation;
#[path = "01_tokens-to-syntax-trees/parsed_files.rs"]
pub(crate) mod source_syntax;
pub use source_syntax::{
    ParsedSourceBatch, ParsedSourceSummary, RetainedSourceParse, SourceDeclarations, SourceImport,
    SourceInput, SourceItems, SourceSyntax,
};
#[path = "02_syntax-trees-to-symbol-resolved-trees/build_stages.rs"]
pub(crate) mod build_stages;
#[path = "01_tokens-to-syntax-trees/streaming.rs"]
pub(crate) mod source_parsing;
#[path = "02_syntax-trees-to-symbol-resolved-trees/build_helpers.rs"]
pub(crate) mod target_helpers;

pub use ::source_syntax::{ParsedText, SourceParseError, parse_text};
pub use source_checking::{
    FrozenSource, JudgedSource, ProductRepresentationInput, SourceAdmission,
    SourcePreparationFailure, SourcePreparationRequest,
};

pub use source_compilation::{
    AdmittedProductPolicy, CompletedSource, PreparedSource, ProductPolicyRequest,
    ResolveAndTypeRequest, SourceContinuation, SourceExtensionRequest, SourceFrontends,
    SourcePreparation,
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
pub use checked_program_to_psi::provider_supply;

#[path = "prepare_and_check/provider_choices.rs"]
pub mod source_provider_choices;

#[path = "prepare_and_check/entry_contracts.rs"]
pub mod entry_contracts;
#[path = "prepare_and_check/entry_shapes.rs"]
pub mod entry_shapes;
#[path = "prepare_and_check.rs"]
pub mod product_source;
#[path = "prepare_and_check/source_targets.rs"]
pub mod source_targets;
#[path = "representations/typed-trees/typed_frontier.rs"]
pub(crate) mod typed_frontier;
pub use typed_frontier::TypedFrontier;

#[path = "04_typed-trees-to-typed-trees/unbuilt_source.rs"]
pub(crate) mod unbuilt_source;
pub use unbuilt_source::{UnbuiltSourceInputs, prepare_unbuilt_frontier};

/// Inspect preparation in counterexample tests without granting acceptance.
#[cfg(any(test, feature = "test-support"))]
pub fn prepare_source_for_test(
    request: SourcePreparationRequest<'_>,
) -> Result<FrozenSource, SourcePreparationFailure> {
    request.prepare()
}

#[cfg(test)]
use source_files::LoadedSource;
#[cfg(test)]
#[path = "../../../../../tests/support/frontend_operations.rs"]
pub(crate) mod frontend_fixture;
#[cfg(test)]
#[path = "../../../../../tests/support/fixture_repository.rs"]
pub(crate) mod fixture_repository;
#[cfg(test)]
#[path = "../../tests/support/schema_customer_source.rs"]
pub(crate) mod schema_customer_source;
