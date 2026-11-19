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

use crate::source_judgment;
pub use source_judgment::SourcePublication;
#[cfg(any(test, feature = "test-support"))]
pub use source_judgment::{judge_source_roster, judge_terminal_source};

pub use crate::product::provider_settlement::{
    ChosenDefaultLayout, ProductSupplySource, SourceSupplyTarget, SuppliedProduct, SupplyJudgment,
    TargetSupplySource, judge_supplied_product, prepare_supplied_target,
};

pub use typed_trees_to_lowered_program::lowering::source_roster::{
    SourceCallingContext, SourceCallingInputs, SourceCompilationRoster, SourceNormalizationInputs,
    SourceProgramEntryInput,
};

#[path = "pipeline/01_tokens-to-syntax-trees/parsed_files.rs"]
pub(crate) mod source_syntax;
pub use source_syntax::{
    ParsedSourceBatch, ParsedSourceSummary, RetainedSourceParse, SourceDeclarations, SourceImport,
    SourceInput, SourceItems, SourceSyntax,
};
#[path = "pipeline/02_syntax-trees-to-symbol-resolved-trees/build_stages.rs"]
pub(crate) mod build_stages;
#[path = "pipeline/01_tokens-to-syntax-trees/streaming.rs"]
pub(crate) mod source_parsing;
#[path = "pipeline/02_syntax-trees-to-symbol-resolved-trees/build_helpers.rs"]
pub(crate) mod target_helpers;

pub use ::source_syntax::{ParsedText, SourceParseError, parse_text};
pub use crate::source_preparation::{ProductRepresentationInput, SourcePreparationFailure};
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use crate::source_preparation::{SourceAdmission, SourcePreparationRequest};
#[cfg(any(test, feature = "test-support"))]
pub use crate::source_preparation::{SourceAdmission, SourcePreparationRequest};
#[cfg(any(test, feature = "test-support"))]
pub use source_judgment::{FrozenSource, JudgedSource};

pub use crate::source_continuation::{
    AdmittedProductPolicy, CompletedSource, PreparedSource, ProductPolicyRequest,
    SourceContinuation, SourceExtensionRequest, SourceFrontends,
    SourcePreparation,
};

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use crate::source_continuation::ResolveAndTypeRequest;
#[cfg(any(test, feature = "test-support"))]
pub use crate::source_continuation::ResolveAndTypeRequest;

#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use crate::production::{TerminalProductionCustody, TerminalSourceCompilation};
#[cfg(any(test, feature = "test-support"))]
pub use crate::production::{TerminalProductionCustody, TerminalSourceCompilation};

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
    TerminalProgramEntryContractSelection, TerminalProgramRoots,
    TerminalProviderSelectionInput, TerminalProviderSelections,
    TerminalSourceSupply, TerminalSuppliedRequirement, UnmeasuredProduction,
};

/// Logical provider closure; native planning consumes only its accepted result.
pub use checked_program_to_psi::provider_supply;

pub use crate::product::TypedFrontier;


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
#[path = "../../../../tests/support/frontend_operations.rs"]
pub(crate) mod frontend_fixture;
#[cfg(test)]
#[path = "../../../../tests/support/fixture_repository.rs"]
pub(crate) mod fixture_repository;
#[cfg(test)]
#[path = "../tests/support/schema_customer_source.rs"]
pub(crate) mod schema_customer_source;
