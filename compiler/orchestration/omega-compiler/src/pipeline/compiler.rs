use crate::pipeline::artifacts::{
    remove_stale_phase_diagrams, write_backend_report, write_emission_plan,
    write_resolved_snapshot, write_syntax_snapshot, write_timings, write_typed_snapshot,
};
use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::compile_report::CompileReport;
use crate::pipeline::frontend::{
    discover_imports, extend_source_storage, lex_sources, load_sources, parse_sources,
};
use crate::pipeline::output::write_output;
use crate::pipeline::project::{project_roots, validate_selected_target};
use crate::pipeline::source::{ImportQueue, SourceStorage};
use crate::pipeline::stages::{
    assemble_syntax, check_program, emit_backend, ensure_emission_ready, plan_backend,
    plan_emission, resolve_program, typecheck_program,
};
use crate::pipeline::timing::CompileTimings;
use omega_artifacts::build_backend_surface_report;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;

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
        for root in project_roots(&self.options.root_path) {
            imports.seed(root);
        }
        let workers = WorkerPool::with_available_parallelism();
        let mut timings = CompileTimings::default();

        let mut source_storage = SourceStorage::default();

        while imports.has_pending() {
            let frontier = imports.take_frontier();
            let first_source_id = source_storage.next_source_id();
            let sources = timings.record("Stage 00: ImportQueue -> SourceFiles", || {
                load_sources(frontier, first_source_id)
            })?;
            let lexed = timings.record("Stage 01: SourceFiles -> TokenStreams", || {
                lex_sources(sources)
            })?;
            let parsed = timings.record("Stage 02: TokenStreams -> SyntaxTrees", || {
                parse_sources(lexed, &mut source_storage.syntax_trees)
            })?;
            let discovered_imports =
                timings.record("Stage 00: SyntaxTrees -> ImportQueue", || {
                    discover_imports(
                        &parsed,
                        &source_storage.syntax_trees,
                        &self.options.root_path,
                        self.options.target_name.as_deref(),
                    )
                })?;

            imports.enqueue(discovered_imports)?;
            timings.record("Stage 00: ParsedFrontier -> SourceStorage", || {
                extend_source_storage(&mut source_storage, parsed)
            })?;
        }

        timings.record("Stage 00: SourceStorage -> TargetSelection", || {
            validate_selected_target(&source_storage, self.options.target_name.as_deref())
        })?;
        remove_stale_phase_diagrams(&self.options)?;

        let source_file_count = source_storage.file_count();
        let syntax = timings.record("Stage 02: SourceStorage -> SyntaxTrees", || {
            assemble_syntax(source_storage)
        })?;
        write_syntax_snapshot(&self.options, &syntax.syntax_trees)?;
        let resolved = timings.record("Stage 03: SyntaxTrees -> SymbolResolvedTrees", || {
            resolve_program(syntax)
        })?;
        write_resolved_snapshot(&self.options, &resolved)?;
        let typed = timings.record("Stage 04: SymbolResolvedTrees -> TypedTrees", || {
            typecheck_program(resolved)
        })?;
        write_typed_snapshot(&self.options, &typed)?;
        let checked = timings.record("Stage 05: TypedTrees -> CheckedTrees", || {
            check_program(typed)
        })?;
        let backend_surface = build_backend_surface_report(&checked.program);
        let planned = timings.record("Stage 06-09: CheckedTrees -> BackendPlan", || {
            plan_backend(
                checked,
                self.options.target_name.as_deref(),
                workers.handle(),
            )
        })?;
        let emission_plan = timings.record("Stage 10: BackendPlan -> EmissionPlan", || {
            Ok(plan_emission(&planned))
        })?;
        if self.options.write_output {
            write_backend_report(&self.options, &backend_surface, &planned)?;
            write_emission_plan(&self.options, &emission_plan)?;
        }
        timings.record("Stage 10: EmissionPlan -> EmissionReady", || {
            ensure_emission_ready(&emission_plan)
        })?;
        let emitted = timings.record("Stage 11: BackendPlan -> MachineBytes", || {
            emit_backend(&planned)
        })?;

        if self.options.write_output {
            write_output(&self.options, emitted)?;
            write_timings(&self.options, timings.as_slice())?;
        }

        Ok(CompileReport {
            root_path: self.options.root_path,
            source_file_count,
            wrote_output: self.options.write_output,
        })
    }
}
