// We answer declaration queries (`ProductQueries::describe`) at a paused Build
// occurrence without completing Build. The application supplies acquired
// dependency inputs rather than source IRs, and repeated queries over equal
// `ProductQueryInputs` reuse their assembled declaration frontier.

use build_settings::host::bindings::{BuildProductDescription, BuildProductQuery};
use diagnostics::Diagnostic;
use package_compilation::source_imports::{ProductQueryDependency, ProductQueryInputs};

pub struct ProductQueries {
    preparation: products::SourcePreparation,
    sources: Option<package_compilation::source_assembly::DeferredProductSources>,
    initial: source_compiler::SourceContinuation,
    inputs: Option<package_compilation::PackageCompilationInputs>,
    execution_profile: Option<target::TargetProfile>,
    /// The build dependencies the Build has prepared when it asks: a later
    /// stage of a staged Build calls their code, and a query that code makes
    /// is answered at its own occurrence.
    prepared: Vec<ProductQueryDependency>,
    retained: Option<(ProductQueryInputs, source_compiler::SourceContinuation)>,
}

impl ProductQueries {
    pub(super) fn new(
        frontier: &products::TypedCompilationFrontier,
        inputs: Option<package_compilation::PackageCompilationInputs>,
        execution_profile: Option<target::TargetProfile>,
        prepared: Vec<ProductQueryDependency>,
    ) -> Self {
        Self {
            preparation: frontier.preparation.clone(),
            sources: frontier
                .product_sources
                .as_ref()
                .map(|pending| pending.sources()),
            initial: frontier.source.clone(),
            inputs,
            execution_profile,
            prepared,
            retained: None,
        }
    }

    pub fn describe(
        &mut self,
        query: &BuildProductQuery,
        dependencies: Vec<ProductQueryDependency>,
    ) -> Result<BuildProductDescription, Vec<Diagnostic>> {
        super::compile_thread::run_on_compile_thread(|| self.describe_inline(query, dependencies))
    }

    fn describe_inline(
        &mut self,
        query: &BuildProductQuery,
        dependencies: Vec<ProductQueryDependency>,
    ) -> Result<BuildProductDescription, Vec<Diagnostic>> {
        let inputs = ProductQueryInputs::new(self.inputs.clone(), dependencies)
            .and_then(|inputs| inputs.with_prepared_build_dependencies(self.prepared.clone()))
            .map_err(refuse)?;
        if self
            .retained
            .as_ref()
            .is_none_or(|(previous, _)| *previous != inputs)
        {
            let source = match &self.sources {
                Some(sources) => {
                    let mut timings = artifacts::compile_timings::CompileTimings::default();
                    let syntax = sources.assemble_query(
                        &inputs,
                        &mut timings,
                        &mut *self.preparation.catalogue()?,
                    )?;
                    super::build_continuation::lower_typed_frontier(
                        self.preparation.clone(),
                        syntax.sources.selected_files().count(),
                        syntax,
                        self.execution_profile,
                        self.inputs.as_ref(),
                        &mut timings,
                        None,
                    )?
                    .source
                }
                None => self.initial.clone(),
            };
            self.retained = Some((inputs, source));
        }
        let source = &self
            .retained
            .as_ref()
            .ok_or_else(|| refuse("Product query has no prepared declaration frontier"))?
            .1;
        let author = self
            .initial
            .typed()
            .symbols
            .source_file(query.occurrence)
            .ok_or_else(|| refuse("Product query lost its authored source occurrence"))?;
        let source_map = source
            .typed()
            .symbols
            .source_map()
            .ok_or_else(|| refuse("Product query has no source custody"))?;
        let selected = source_map.rejoined_source_id(author).ok_or_else(|| {
            refuse("Product query author lost its admitted source correspondence")
        })?;
        let occurrence = source::SourceSpan::new(selected, query.occurrence.span);
        match query.operation {
            source_compiler::TerminalBuildQueryOperation::SchemaPath
            | source_compiler::TerminalBuildQueryOperation::ProviderPath => Err(refuse(
                "description inspection belongs to its issuing Build activation, not source resolution",
            )),
            source_compiler::TerminalBuildQueryOperation::Schema => source
                .describe_schema(occurrence, &query.path)
                .map(BuildProductDescription::Schema)
                .map_err(refuse),
            source_compiler::TerminalBuildQueryOperation::Provider => source
                .describe_provider(occurrence, &query.path)
                .map(BuildProductDescription::Provider)
                .map_err(refuse),
            source_compiler::TerminalBuildQueryOperation::Entry => source
                .describe_entry(
                    occurrence,
                    &query.path,
                    query
                        .slot
                        .as_deref()
                        .ok_or_else(|| refuse("Product entry query has no slot"))?,
                    &target::ProgramEntrySlots,
                )
                .map(BuildProductDescription::Entry)
                .map_err(refuse),
        }
    }
}

fn refuse(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}
