//! Timing labels for acquiring source and producing Psi.
use super::StageMeta;

pub const LOAD_FILES: StageMeta =
    StageMeta::new("Source acquisition", "SourcePaths", "SourceFiles");
pub const PARSE_FILES: StageMeta = StageMeta::new("Source ingress", "SourceFiles", "SyntaxTrees");
pub const RESOLVE: StageMeta = StageMeta::new("Psi 02", "SyntaxTrees", "SymbolResolvedTrees");
pub const TYPE: StageMeta = StageMeta::new("Psi 03", "SymbolResolvedTrees", "TypedTrees");
pub const PREPARE: StageMeta = StageMeta::new(
    "typed-source-preparation",
    "TypedTrees",
    "PreparedTypedTrees",
);
pub const LOWER: StageMeta = StageMeta::new("Psi 05", "PreparedTypedTrees", "LoweredProgram");
pub const CHECK: StageMeta = StageMeta::new("Psi 06", "LoweredProgram", "CheckedProgram");
pub const ACCEPT: StageMeta = StageMeta::new(
    "source acceptance",
    "CheckedProgram",
    "AcceptedTerminalSource",
);
pub const PROVIDER_SETTLEMENT: StageMeta =
    StageMeta::new("provider settlement", "SourcePublication", "ProviderWork");
pub const CHECK_SELECTIONS: StageMeta = StageMeta::new(
    "selection admission",
    "SourceSelections",
    "AdmittedSelections",
);
pub const ADVISORIES: StageMeta =
    StageMeta::new("advisories", "AcceptedTerminalSource", "Diagnostics");
/// Materialize the portable or invocation artifact from the accepted module.
pub const PUBLISH: StageMeta = StageMeta::new(
    "terminal-production/publication",
    "CheckedPublicationModule",
    "TerminalProgramProduct",
);
