use std::path::PathBuf;

pub struct LoadedSources;
pub struct LexedSources;
pub struct ParsedSources;
pub struct AssembledSyntax;
pub struct ResolvedProgram;
pub struct TypedProgram;
pub struct ValidatedProgram;
pub struct BackendPlan;
pub struct EmittedProgram;

pub type DiscoveredImports = Vec<PathBuf>;
