use std::sync::Arc;

use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::import_queue::ImportQueue;
use crate::pipeline::source_file::SourceFile;
use crate::source::SourceMap;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutput {
    pub files: Vec<SourceFile>,
    pub sources: Arc<SourceMap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub files: Vec<SourceFile>,
    pub sources: Arc<SourceMap>,
}

pub fn check(options: CompileOptions) -> Result<CheckOutput, Vec<Diagnostic>> {
    Compiler::new(options).check()
}

pub fn compile(options: CompileOptions) -> Result<CompileOutput, Vec<Diagnostic>> {
    Compiler::new(options).compile()
}

pub struct Compiler {
    options: CompileOptions,
    imports: ImportQueue,
    source_loader: SourceLoader,
    workers: WorkerPool,
    files: Vec<SourceFile>,
    sources: SourceMap,
}

impl Compiler {
    pub fn new(options: CompileOptions) -> Self {
        let mut imports = ImportQueue::default();
        imports.seed(options.root_path.clone());

        Self {
            options,
            imports,
            source_loader: SourceLoader,
            workers: WorkerPool::with_available_parallelism(),
            files: Vec::new(),
            sources: SourceMap::default(),
        }
    }

    pub fn check(mut self) -> Result<CheckOutput, Vec<Diagnostic>> {
        self.compile_frontend()?;
        Ok(self.finish_check())
    }

    pub fn compile(mut self) -> Result<CompileOutput, Vec<Diagnostic>> {
        self.compile_frontend()?;

        let syntax = self.assemble_syntax()?;
        let resolved = self.resolve(syntax)?;
        let typed = self.typecheck(resolved)?;
        let validated = self.validate(typed)?;
        let planned = self.plan_backend(validated)?;
        let emitted = self.emit(planned)?;

        Ok(self.finish_compile(emitted))
    }

    pub fn compile_frontend(&mut self) -> Result<(), Vec<Diagnostic>> {
        while self.imports.has_pending() {
            let frontier = self.imports.take_frontier();
            let sources = self.source_loader.load(frontier)?;
            let lexed = self.lex_sources(sources)?;
            let parsed = self.parse_sources(lexed)?;
            let imports = self.discover_imports(&parsed)?;

            self.imports.enqueue(imports)?;
            self.extend_sources(parsed)?;
        }

        Ok(())
    }

    fn lex_sources(&mut self, sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
        let _ = &self.workers;
        let _ = &self.options;
        let _ = sources;
        todo!("lex source batch into token streams")
    }

    fn parse_sources(&mut self, lexed: LexedSources) -> Result<ParsedSources, Vec<Diagnostic>> {
        let _ = lexed;
        todo!("parse token streams into per-file syntax")
    }

    fn discover_imports(
        &self,
        parsed: &ParsedSources,
    ) -> Result<Vec<std::path::PathBuf>, Vec<Diagnostic>> {
        let _ = parsed;
        todo!("discover imports from parsed per-file syntax")
    }

    fn extend_sources(&mut self, parsed: ParsedSources) -> Result<(), Vec<Diagnostic>> {
        let _ = parsed;
        todo!("append parsed files into compiler-owned source storage")
    }

    fn assemble_syntax(&mut self) -> Result<AssembledSyntax, Vec<Diagnostic>> {
        todo!("assemble all parsed source files into a whole-program syntax product")
    }

    fn resolve(&mut self, syntax: AssembledSyntax) -> Result<ResolvedProgram, Vec<Diagnostic>> {
        let _ = syntax;
        todo!("resolve symbols over assembled whole-program syntax")
    }

    fn typecheck(&mut self, resolved: ResolvedProgram) -> Result<TypedProgram, Vec<Diagnostic>> {
        let _ = resolved;
        todo!("typecheck resolved program")
    }

    fn validate(&mut self, typed: TypedProgram) -> Result<ValidatedProgram, Vec<Diagnostic>> {
        let _ = typed;
        todo!("validate typed program")
    }

    fn plan_backend(
        &mut self,
        validated: ValidatedProgram,
    ) -> Result<BackendPlan, Vec<Diagnostic>> {
        let _ = validated;
        todo!("plan backend from validated program")
    }

    fn emit(&mut self, plan: BackendPlan) -> Result<EmittedProgram, Vec<Diagnostic>> {
        let _ = plan;
        todo!("emit backend plan")
    }

    fn finish_check(self) -> CheckOutput {
        CheckOutput {
            files: self.files,
            sources: Arc::new(self.sources),
        }
    }

    fn finish_compile(self, _emitted: EmittedProgram) -> CompileOutput {
        CompileOutput {
            files: self.files,
            sources: Arc::new(self.sources),
        }
    }
}

struct SourceLoader;
struct LoadedSources;
struct LexedSources;
struct ParsedSources;
struct AssembledSyntax;
struct ResolvedProgram;
struct TypedProgram;
struct ValidatedProgram;
struct BackendPlan;
struct EmittedProgram;

impl SourceLoader {
    fn load(&self, _frontier: Vec<std::path::PathBuf>) -> Result<LoadedSources, Vec<Diagnostic>> {
        todo!("load frontier source files from disk")
    }
}
