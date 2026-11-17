/*
    Target-neutral source parsing, resolution, typing, and preparation. This
    facade supplies frozen typed input to the lowered-form checker; it produces
    no source-checker judgment or selected product cache.
*/

/*
    Separating target-neutral Psi stage sequencing from Omega package discovery,
    Build host execution, and native code generation ensures every compile-time
    and product consumer traverses the exact same lexical, typing, checking, and
    portable Terminal production rules without circular coordinator dependencies.
*/

/*
    Start at `source_parsing` (below) for admitted source frontiers and snippet
    parsing, `source_compilation` (`SourceCompilationRequest`,
    `SourceContinuation`, and `CompletedSource`) for the resolver/typing
    continuation and Product policy admission, `source_checking`
    (`SourcePreparationRequest` and `FrozenSource`) before lowered-form
    checking. Accepted source publication and artifact retention belong to
    `terminal-production`.
*/

#![forbid(unsafe_code)]

pub mod declaration_selection;
pub mod provider_choice_inputs;
pub mod source_inputs;
#[cfg(not(any(test, feature = "test-support")))]
#[path = "source_resolved/src/lib.rs"]
pub(crate) mod source_resolved;
#[cfg(any(test, feature = "test-support"))]
#[path = "source_resolved/src/lib.rs"]
pub mod source_resolved;
mod source_selections;
pub mod source_symbols;
pub mod source_tables;

// Source preparation owns compilation of evaluation requests; only accepted
// immutable invocations cross into artifact-evaluation.
// Source representation and its application builders share the settlement
// owner's lifetime. Build inspection still enters through source admission;
// accepted portable and native products never retain this mutable state.
#[path = "source_state/src/lib.rs"]
pub mod source_state;

pub mod evaluation;

#[cfg(not(any(test, feature = "test-support")))]
#[path = "operations/checking/src/lib.rs"]
pub(crate) mod checking;
#[cfg(any(test, feature = "test-support"))]
#[path = "operations/checking/src/lib.rs"]
pub mod checking;
#[cfg(not(any(test, feature = "test-support")))]
#[path = "operations/lexing/lexer.rs"]
pub(crate) mod lexing;
#[cfg(any(test, feature = "test-support"))]
#[path = "operations/lexing/lexer.rs"]
pub mod lexing;
#[cfg(not(any(test, feature = "test-support")))]
#[path = "operations/lowering/src/lib.rs"]
pub(crate) mod lowering;
#[cfg(any(test, feature = "test-support"))]
#[path = "operations/lowering/src/lib.rs"]
pub mod lowering;
#[cfg(not(any(test, feature = "test-support")))]
#[path = "operations/parsing/parser.rs"]
pub(crate) mod parsing;
#[cfg(any(test, feature = "test-support"))]
#[path = "operations/parsing/parser.rs"]
pub mod parsing;
#[cfg(not(any(test, feature = "test-support")))]
#[path = "operations/preparation/src/lib.rs"]
pub(crate) mod preparation;
#[cfg(any(test, feature = "test-support"))]
#[path = "operations/preparation/src/lib.rs"]
pub mod preparation;
#[cfg(not(any(test, feature = "test-support")))]
#[path = "operations/production/src/lib.rs"]
pub(crate) mod production;
#[cfg(any(test, feature = "test-support"))]
#[path = "operations/production/src/lib.rs"]
pub mod production;
#[cfg(not(any(test, feature = "test-support")))]
#[path = "operations/resolution/src/lib.rs"]
pub(crate) mod resolution;
#[cfg(any(test, feature = "test-support"))]
#[path = "operations/resolution/src/lib.rs"]
pub mod resolution;
#[cfg(not(any(test, feature = "test-support")))]
#[path = "operations/typing/src/lib.rs"]
pub(crate) mod typing;
#[cfg(any(test, feature = "test-support"))]
#[path = "operations/typing/src/lib.rs"]
pub mod typing;

mod source_publication;
pub use source_publication::SourcePublication;

mod source_supply;
pub use source_supply::{
    ChosenDefaultLayout, ProductSupplySource, SourceSupplyTarget, SuppliedProduct, SupplyJudgment,
    TargetSupplySource, judge_supplied_product, prepare_supplied_target,
};

mod source_preparation;
pub use source_preparation::SourcePreparation;

mod source_roster;
pub use source_roster::{
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
mod target_helpers;
pub(crate) mod source_parsing;

pub use source_checking::{
    FrozenSource, JudgedSource, ProductRepresentationInput, SourceAdmission,
    SourcePreparationFailure, SourcePreparationRequest,
};
pub use source_parsing::{ParsedText, SourceParseError, parse_text};

pub use source_compilation::{
    AdmittedProductPolicy, CompletedSource, PreparedSource, ProductPolicyRequest,
    SourceCompilationRequest, SourceContinuation, SourceExtensionRequest, SourceFrontends,
};

pub use crate::production::{
    AcceptedTerminalSource, BuildCallSite, CallableBoundaryImplementation, ConstantRead,
    DeclaredBoundaryRequirement, PortableTerminalProgram, PreparedTerminalRoots,
    ProducedBuildProvider, ProducedTerminalApplication, ProducedTerminalProgram,
    ProgramEntryTerminalReceiptError, PsiCompilationObserver, PsiCompilationStage,
    TerminalAdmissionRoot, TerminalApplicationCustody, TerminalArtifactProductionError,
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildBindings,
    TerminalBuildFilesystem, TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation,
    TerminalBuildNativeLocator, TerminalBuildProviderRequirement, TerminalBuildQueryCall,
    TerminalBuildQueryOperation, TerminalBuildRequestCall, TerminalBuildRequestOperation,
    TerminalBuildRequests, TerminalBuildSelectionRequest, TerminalBuildValueCase,
    TerminalBuildValueField, TerminalBuildValueShape, TerminalCallingPolicySelections,
    TerminalCompositionRoots, TerminalMachineSelection, TerminalProductionCustody,
    TerminalProgramEntryContractSelection, TerminalProgramRoots, TerminalProviderSelectionInput,
    TerminalProviderSelections, TerminalSourceCompilation, TerminalSourceSupply,
    TerminalSuppliedRequirement, UnmeasuredProduction,
};

/// Logical provider closure; native planning consumes only its accepted result.
pub mod provider_supply;

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
