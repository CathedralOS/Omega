//! Describe declarations at a paused Build occurrence, without completing Build.

use build_evaluation::terminal_bindings::{BuildProductDescription, BuildProductQuery};
use diagnostics::Diagnostic;
use package_compilation::source_imports::{ProductQueryDependency, ProductQueryInputs};

/// Invocation-owned query context. The application supplies acquired dependency
/// inputs, never source IRs or a synthetic completed package. Repeated queries
/// over the same inputs share their declaration frontier.
pub struct ProductQueries {
    sources: Option<package_compilation::source_assembly::DeferredProductSources>,
    initial: psi_compiler::SourceContinuation,
    inputs: Option<package_compilation::PackageCompilationInputs>,
    execution_profile: Option<target::TargetProfile>,
    retained: Option<(ProductQueryInputs, psi_compiler::SourceContinuation)>,
}

impl ProductQueries {
    pub(super) fn new(
        frontier: &compilation_products::TypedCompilationFrontier,
        inputs: Option<package_compilation::PackageCompilationInputs>,
        execution_profile: Option<target::TargetProfile>,
    ) -> Self {
        Self {
            sources: frontier
                .product_sources
                .as_ref()
                .map(|pending| pending.sources()),
            initial: frontier.source.clone(),
            inputs,
            execution_profile,
            retained: None,
        }
    }

    pub fn describe(
        &mut self,
        query: &BuildProductQuery,
        dependencies: Vec<ProductQueryDependency>,
    ) -> Result<BuildProductDescription, Vec<Diagnostic>> {
        let inputs = ProductQueryInputs::new(self.inputs.clone(), dependencies).map_err(refuse)?;
        if self
            .retained
            .as_ref()
            .is_none_or(|(previous, _)| *previous != inputs)
        {
            let source = match &self.sources {
                Some(sources) => {
                    let mut timings = artifacts::compile_timings::CompileTimings::default();
                    let syntax = sources.assemble_query(&inputs, &mut timings)?;
                    super::build_continuation::lower_typed_frontier(
                        syntax.sources.len(),
                        syntax,
                        self.execution_profile,
                        self.inputs.as_ref(),
                        &mut timings,
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
        let mut matches = source_map.files().filter(|candidate| {
            candidate.path == author.path
                && candidate.source == author.source
                && candidate.package_root == author.package_root
                && candidate.package_identity == author.package_identity
                && candidate.origin == author.origin
                && candidate.resolution_stratum == author.resolution_stratum
                && candidate.dependency_scope == author.dependency_scope
        });
        let selected = matches
            .next()
            .ok_or_else(|| refuse("Product query author is absent from the query frontier"))?;
        if matches.next().is_some() {
            return Err(refuse(
                "Product query author is ambiguous in the query frontier",
            ));
        }
        let occurrence = source::SourceSpan::new(selected.source_id, query.occurrence.span);
        match query.operation {
            psi_compiler::TerminalBuildQueryOperation::Entry => source
                .describe_entry(
                    occurrence,
                    &query.path,
                    query
                        .slot
                        .as_deref()
                        .ok_or_else(|| refuse("Product entry query has no slot"))?,
                    &provider_planning::program_entry::ProductEntryQueryCompatibility,
                )
                .map(BuildProductDescription::Entry)
                .map_err(refuse),
        }
    }
}

fn refuse(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}
