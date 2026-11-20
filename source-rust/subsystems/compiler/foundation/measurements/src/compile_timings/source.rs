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
/// Materialize the portable or invocation artifact from the accepted module.
pub const PUBLISH: StageMeta = StageMeta::new(
    "terminal-production/publication",
    "CheckedPublicationModule",
    "TerminalProgramProduct",
);
pub const ENTRY_RECEIPT: StageMeta = StageMeta::new(
    "terminal-production/entry-receipt",
    "CanonicalTerminalArtifact",
    "CheckedProgramEntryTerminalReceipt",
);
