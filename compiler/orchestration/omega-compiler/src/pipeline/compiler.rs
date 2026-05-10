use std::path::PathBuf;
use std::sync::Arc;

use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::import_queue::ImportQueue;
use crate::pipeline::source_file::SourceFile;
use crate::source::SourceMap;
use omega_core::diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub files: Vec<SourceFile>,
    pub sources: Arc<SourceMap>,
}

pub fn compile(options: CompileOptions) -> Result<CompileOutput, Vec<Diagnostic>> {
    Compiler::new(options).compile()
}

pub struct Compiler {
    options: CompileOptions,
    imports: ImportQueue,
    source_loader: SourceLoader,
    lexer: LexerPhase,
    parser: ParserPhase,
    import_discovery: ImportDiscovery,
    source_storage: SourceStorage,
    syntax_assembler: SyntaxAssembler,
    resolver: Resolver,
    typechecker: Typechecker,
    validator: Validator,
    backend_planner: BackendPlanner,
    emitter: Emitter,
    output_writer: OutputWriter,
}

impl Compiler {
    pub fn new(options: CompileOptions) -> Self {
        let mut imports = ImportQueue::default();
        imports.seed(options.root_path.clone());

        Self {
            options,
            imports,
            source_loader: SourceLoader,
            lexer: LexerPhase,
            parser: ParserPhase,
            import_discovery: ImportDiscovery,
            source_storage: SourceStorage::default(),
            syntax_assembler: SyntaxAssembler,
            resolver: Resolver,
            typechecker: Typechecker,
            validator: Validator,
            backend_planner: BackendPlanner,
            emitter: Emitter,
            output_writer: OutputWriter,
        }
    }

    pub fn compile(mut self) -> Result<CompileOutput, Vec<Diagnostic>> {
        self.imports.seed(self.options.root_path.clone());

        while self.imports.has_pending() {
            let frontier = self.imports.take_frontier();
            let sources = self.source_loader.load(frontier)?;
            let lexed = self.lexer.lex(sources)?;
            let parsed = self.parser.parse(lexed)?;
            let imports = self.import_discovery.discover(&parsed)?;

            self.imports.enqueue(imports)?;
            self.source_storage.extend(parsed)?;
        }

        let syntax = self.syntax_assembler.assemble(&self.source_storage)?;
        let resolved = self.resolver.resolve(syntax)?;
        let typed = self.typechecker.typecheck(resolved)?;
        let validated = self.validator.validate(typed)?;
        let planned = self.backend_planner.plan(validated)?;
        let emitted = self.emitter.emit(planned)?;
        
        if self.options.write_output {
            self.output_writer.write(emitted)?;
        }

        Ok(CompileOutput {
            files: self.source_storage.files,
            sources: Arc::new(self.source_storage.sources),
        })
    }
}

struct SourceLoader;
struct LexerPhase;
struct ParserPhase;
struct ImportDiscovery;
struct SyntaxAssembler;
struct Resolver;
struct Typechecker;
struct Validator;
struct BackendPlanner;
struct Emitter;
struct OutputWriter;

#[derive(Default)]
struct SourceStorage {
    files: Vec<SourceFile>,
    sources: SourceMap,
}

impl SourceStorage {
    fn extend(&mut self, parsed: ParsedSources) -> Result<(), Vec<Diagnostic>> {
        let _ = parsed;
        todo!("append parsed source files into compiler-owned storage")
    }
}

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
    fn load(&self, _frontier: Vec<PathBuf>) -> Result<LoadedSources, Vec<Diagnostic>> {
        todo!("load frontier source files from disk")
    }
}

impl LexerPhase {
    fn lex(&self, _sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
        todo!("lex loaded source files into token streams")
    }
}

impl ParserPhase {
    fn parse(&self, _lexed: LexedSources) -> Result<ParsedSources, Vec<Diagnostic>> {
        todo!("parse token streams into per-file syntax")
    }
}

impl ImportDiscovery {
    fn discover(&self, _parsed: &ParsedSources) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        todo!("discover imports from parsed per-file syntax")
    }
}

impl SyntaxAssembler {
    fn assemble(&self, _sources: &SourceStorage) -> Result<AssembledSyntax, Vec<Diagnostic>> {
        todo!("assemble all parsed source files into a whole-program syntax product")
    }
}

impl Resolver {
    fn resolve(&self, _syntax: AssembledSyntax) -> Result<ResolvedProgram, Vec<Diagnostic>> {
        todo!("resolve symbols over assembled whole-program syntax")
    }
}

impl Typechecker {
    fn typecheck(&self, _resolved: ResolvedProgram) -> Result<TypedProgram, Vec<Diagnostic>> {
        todo!("typecheck resolved program")
    }
}

impl Validator {
    fn validate(&self, _typed: TypedProgram) -> Result<ValidatedProgram, Vec<Diagnostic>> {
        todo!("validate typed program")
    }
}

impl BackendPlanner {
    fn plan(&self, _validated: ValidatedProgram) -> Result<BackendPlan, Vec<Diagnostic>> {
        todo!("plan backend from validated program")
    }
}

impl Emitter {
    fn emit(&self, _plan: BackendPlan) -> Result<EmittedProgram, Vec<Diagnostic>> {
        todo!("emit backend plan")
    }
}

impl OutputWriter {
    fn write(&self, _emitted: EmittedProgram) -> Result<(), Vec<Diagnostic>> {
        todo!("persist emitted output bytes")
    }
}
