use std::path::PathBuf;
use std::sync::Arc;

use crate::pipeline::phase_components::{
    BackendPlanner, Emitter, ImportDiscovery, LexerPhase, OutputWriter, ParserPhase, Resolver,
    SourceLoader, SyntaxAssembler, Typechecker, Validator,
};
use crate::pipeline::phase_products::{
    BackendPlan, EmittedProgram, ParsedSources, LoadedSources, LexedSources, AssembledSyntax,
    ResolvedProgram, TypedProgram, ValidatedProgram,
};
use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::import_queue::ImportQueue;
use crate::pipeline::source_file::SourceFile;
use crate::pipeline::source_storage::SourceStorage;
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
