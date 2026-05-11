use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::compile_report::CompileReport;
use crate::pipeline::phase_components::{
    BackendPlanner, Emitter, ImportDiscovery, LexerPhase, OutputWriter, ParserPhase, Resolver,
    SourceLoader, SyntaxAssembler, Typechecker, Validator,
};
use crate::pipeline::source::{ImportQueue, SourceStorage};
use omega_core::diagnostics::Diagnostic;

pub fn compile(options: CompileOptions) -> Result<CompileReport, Vec<Diagnostic>> {
    Compiler::new(options).compile()
}

pub struct Compiler {
    options: CompileOptions,
}

impl Compiler {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }

    pub fn compile(self) -> Result<CompileReport, Vec<Diagnostic>> {
        let mut imports = ImportQueue::default();
        imports.seed(self.options.root_path.clone());

        let source_loader = SourceLoader;
        let lexer = LexerPhase;
        let parser = ParserPhase;
        let import_discovery = ImportDiscovery;
        let mut source_storage = SourceStorage::default();
        let syntax_assembler = SyntaxAssembler;
        let resolver = Resolver;
        let typechecker = Typechecker;
        let validator = Validator;
        let backend_planner = BackendPlanner;
        let emitter = Emitter;
        let output_writer = OutputWriter;

        while imports.has_pending() {
            let frontier = imports.take_frontier();
            let first_file_id = source_storage.next_file_id();
            let sources = source_loader.load(frontier, first_file_id)?;
            let lexed = lexer.lex(sources)?;
            let parsed = parser.parse(lexed)?;
            let discovered_imports = import_discovery.discover(
                &parsed,
                &self.options.root_path,
                self.options.target_name.as_deref(),
            )?;

            imports.enqueue(discovered_imports)?;
            source_storage.extend(parsed)?;
        }

        let syntax = syntax_assembler.assemble(&source_storage)?;
        let resolved = resolver.resolve(syntax)?;
        let typed = typechecker.typecheck(resolved)?;
        let validated = validator.validate(typed)?;
        let planned = backend_planner.plan(validated)?;
        let emitted = emitter.emit(planned)?;

        if self.options.write_output {
            output_writer.write(emitted)?;
        }

        Ok(CompileReport {
            root_path: self.options.root_path,
            source_file_count: source_storage.file_count(),
            wrote_output: self.options.write_output,
        })
    }
}
