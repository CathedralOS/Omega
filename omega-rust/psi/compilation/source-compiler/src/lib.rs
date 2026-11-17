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
    Start at `source_parsing` (`source_parsing.rs`) for admitted source frontiers and snippet
    parsing, `source_compilation` (`SourceCompilationRequest`,
    `SourceContinuation`, and `CompletedSource`) for the resolver/typing
    continuation and Product policy admission, `source_checking`
    (`SourcePreparationRequest` and `FrozenSource`) before lowered-form
    checking. Accepted source publication and artifact retention belong to
    `terminal-production`.
*/

#![forbid(unsafe_code)]

mod product_descriptions;
mod root_bindings;
mod source_checking;
mod source_compilation;
pub(crate) mod source_parsing;

pub use source_checking::{
    FrozenSource, ProductRepresentationInput, SourceAdmission, SourcePreparationFailure,
    SourcePreparationRequest,
};
pub use source_parsing::{
    ParsedText, SourceInput, SourceParseError, load_source_files, parse_text,
};

pub use source_compilation::{
    AdmittedProductPolicy, BuildSourceInput, CompletedSource, PreparedSource, ProductPolicyRequest,
    SourceCompilationRequest, SourceContinuation, SourceExtensionRequest, SourceFrontends,
};

pub use terminal_production::{
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

#[cfg(test)]
#[path = "../../../../../tests/support/frontend.rs"]
mod frontend_fixture;
