//! Describe the Product declarations mounted for a paused Build call and bind
//! each acquired graph to the Product registration that made it visible.
use super::{PlanningServices, PlanningSource, refusal, stage_source};
use build_declarations::{BuildRequest, DependencyPurpose};
use build_execution::host::bindings::{BuildProductDescription, BuildProductQuery};
use build_execution::{BuildDependencySelection, CompileFailure, TerminalBuildActivation};
use package_compilation::source_imports::{ProductQueryDependency, ProductQueryInputs};
use source_compiler::{SourceContinuation, TerminalBuildQueryOperation};
use std::sync::Arc;

impl PlanningSource {
    pub(in crate::planning) fn describe_product(
        &mut self,
        activation: &TerminalBuildActivation,
        query: &BuildProductQuery,
        prepared: &[ProductQueryDependency],
        services: &mut impl PlanningServices,
    ) -> Result<(BuildProductDescription, Vec<BuildDependencySelection>), CompileFailure> {
        let dependencies = services.query_dependencies(activation, query)?;
        let mut issued = activation
            .requests()
            .iter()
            .filter_map(|issued| match issued.request() {
                BuildRequest::Dependency {
                    purpose: DependencyPurpose::Product,
                    alias,
                    ..
                } => Some(alias.as_ref()),
                _ => None,
            });
        let mut selections = Vec::with_capacity(dependencies.len());
        for (request_ordinal, dependency) in dependencies.iter().enumerate() {
            let alias = issued.next().ok_or_else(|| {
                refusal("Product query received an unregistered dependency".into())
            })?;
            let package = dependency.inputs.root();
            let alias = alias
                .map(|alias| alias.as_str())
                .or_else(|| dependency.inputs.package_name(package));
            if alias != Some(dependency.alias.as_str()) || dependency.generated.is_some() {
                return Err(refusal(
                    "Product query dependency differs from its issued authored source registration"
                        .into(),
                ));
            }
            selections.push(BuildDependencySelection {
                purpose: DependencyPurpose::Product,
                request_ordinal,
                package,
            });
        }
        if issued.next().is_some() {
            return Err(refusal(
                "Product query is missing an issued dependency".into(),
            ));
        }

        let named = query
            .operation
            .requires_named_source()
            .then(|| self.named_source(activation, services))
            .transpose()?;
        let inputs = ProductQueryInputs::new(self.original.clone(), dependencies)
            .and_then(|inputs| inputs.with_prepared_build_dependencies(prepared.to_vec()))
            .and_then(|inputs| inputs.with_named_source(named))
            .map_err(refusal)?;
        if self
            .query_frontier
            .as_ref()
            .is_none_or(|(previous, _)| previous != &inputs)
        {
            let mut assembled = self.deferred.assemble_query(
                &inputs,
                &mut self.timings,
                &mut *self.preparation.catalogue()?,
            )?;
            let source = stage_source(&mut assembled)?.prepare_product_query(
                &self.preparation,
                self.profile.target_name(),
                self.original
                    .as_ref()
                    .map(|inputs| Arc::new(inputs.clone()) as Arc<_>),
            )?;
            self.query_frontier = Some((inputs, source));
        }
        let (_, source) = self
            .query_frontier
            .as_ref()
            .expect("the query frontier was prepared");
        let description = describe(source, &self.initial, query).map_err(refusal)?;
        Ok((description, selections))
    }
}

fn describe(
    source: &SourceContinuation,
    author: &SourceContinuation,
    query: &BuildProductQuery,
) -> Result<BuildProductDescription, String> {
    let (occurrence, selecting) =
        source.rejoin_product_query_origin(author, query.occurrence, query.selecting_machine)?;
    match query.operation {
        TerminalBuildQueryOperation::Parameter => source
            .describe_parameter(
                occurrence,
                selecting.ok_or("parameter query lost its author")?,
                &query.path,
            )
            .map(BuildProductDescription::Parameter),
        TerminalBuildQueryOperation::Field => source
            .describe_field(
                occurrence,
                selecting.ok_or("field query lost its author")?,
                &query.path,
            )
            .map(BuildProductDescription::Field),
        TerminalBuildQueryOperation::Schema => source
            .describe_schema(occurrence, &query.path)
            .map(BuildProductDescription::Schema),
        TerminalBuildQueryOperation::Provider => source
            .describe_provider(occurrence, &query.path)
            .map(BuildProductDescription::Provider),
        TerminalBuildQueryOperation::Entry => source
            .describe_entry(
                occurrence,
                &query.path,
                query
                    .slot
                    .as_deref()
                    .ok_or("Product entry query has no slot")?,
                &target::ProgramEntrySlots,
            )
            .map(BuildProductDescription::Entry),
        TerminalBuildQueryOperation::SchemaPath | TerminalBuildQueryOperation::ProviderPath => {
            Err("description inspection belongs to its issuing Build activation".into())
        }
    }
}
