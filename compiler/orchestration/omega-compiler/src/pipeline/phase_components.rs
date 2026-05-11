use std::path::PathBuf;

use crate::pipeline::phase_products::{
    AssembledSyntax, BackendPlan, DiscoveredImports, EmittedProgram, LexedSources, LoadedSources,
    ParsedSources, ResolvedProgram, TypedProgram, ValidatedProgram,
};
use crate::pipeline::source::SourceStorage;
use omega_core::diagnostics::Diagnostic;

pub struct SourceLoader;
pub struct LexerPhase;
pub struct ParserPhase;
pub struct ImportDiscovery;
pub struct SyntaxAssembler;
pub struct Resolver;
pub struct Typechecker;
pub struct Validator;
pub struct BackendPlanner;
pub struct Emitter;
pub struct OutputWriter;

impl SourceLoader {
    pub fn load(&self, _frontier: Vec<PathBuf>) -> Result<LoadedSources, Vec<Diagnostic>> {
        todo!("load frontier source files from disk")
    }
}

impl LexerPhase {
    pub fn lex(&self, _sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
        todo!("lex loaded source files into token streams")
    }
}

impl ParserPhase {
    pub fn parse(&self, _lexed: LexedSources) -> Result<ParsedSources, Vec<Diagnostic>> {
        todo!("parse token streams into per-file syntax")
    }
}

impl ImportDiscovery {
    pub fn discover(&self, _parsed: &ParsedSources) -> Result<DiscoveredImports, Vec<Diagnostic>> {
        todo!("discover imports from parsed per-file syntax")
    }
}

impl SyntaxAssembler {
    pub fn assemble(&self, _sources: &SourceStorage) -> Result<AssembledSyntax, Vec<Diagnostic>> {
        todo!("assemble all parsed source files into a whole-program syntax product")
    }
}

impl Resolver {
    pub fn resolve(&self, _syntax: AssembledSyntax) -> Result<ResolvedProgram, Vec<Diagnostic>> {
        todo!("resolve symbols over assembled whole-program syntax")
    }
}

impl Typechecker {
    pub fn typecheck(&self, _resolved: ResolvedProgram) -> Result<TypedProgram, Vec<Diagnostic>> {
        todo!("typecheck resolved program")
    }
}

impl Validator {
    pub fn validate(&self, _typed: TypedProgram) -> Result<ValidatedProgram, Vec<Diagnostic>> {
        todo!("validate typed program")
    }
}

impl BackendPlanner {
    pub fn plan(&self, _validated: ValidatedProgram) -> Result<BackendPlan, Vec<Diagnostic>> {
        todo!("plan backend from validated program")
    }
}

impl Emitter {
    pub fn emit(&self, _plan: BackendPlan) -> Result<EmittedProgram, Vec<Diagnostic>> {
        todo!("emit backend plan")
    }
}

impl OutputWriter {
    pub fn write(&self, _emitted: EmittedProgram) -> Result<(), Vec<Diagnostic>> {
        todo!("persist emitted output bytes")
    }
}
