//! Resolve the executed provider choices where the preparing Build can name its product.
use super::{PlanningSource, refusal};
use build_declarations::BuildEntryPath;
use build_execution::{CompileFailure, EvaluatedBuild};
use package_compilation::source_imports::{ProductQueryInputs, SuppliedBuildLibrary};
use semantic_vocabulary::source_selections::ProductTarget;
use source_compiler::{InterpretedProviderSelections, compile_product::ProductPolicyInputs};
use std::sync::Arc;

impl PlanningSource {
    pub(crate) fn successor_provider_selections(
        &mut self,
        entry: &BuildEntryPath,
        libraries: &[SuppliedBuildLibrary],
        evaluated: &EvaluatedBuild,
    ) -> Result<
        (
            InterpretedProviderSelections,
            Option<package_compilation::CompletedAnonymousSource>,
        ),
        CompileFailure,
    > {
        let build = &evaluated.configuration;
        let mut events = build
            .product_selection_requests
            .iter()
            .filter(|event| {
                event
                    .target
                    .is_none_or(|target| target == ProductTarget::Interpreter)
            })
            .cloned()
            .collect::<Vec<_>>();
        if events.is_empty() {
            return Ok((Default::default(), None));
        }
        let authors = self.query_authors.as_ref().ok_or_else(|| {
            refusal("the preparing Build has no recorded selection authors".into())
        })?;
        let dependencies = libraries
            .iter()
            .map(|library| library.dependency().clone())
            .collect();
        let incoming = self
            .supplied_libraries
            .iter()
            .map(|library| library.dependency().clone())
            .collect();
        let mut inputs = ProductQueryInputs::new(self.original.clone(), dependencies)
            .and_then(|inputs| inputs.with_prepared_build_dependencies(incoming))
            .map_err(refusal)?
            .with_successor_entry(entry.clone());
        for operand in events
            .iter()
            .flat_map(|event| event.request.operands.iter())
        {
            let author = authors.file_at(operand.source_span).ok_or_else(|| {
                refusal("the preparing Build lost the author of a provider operand".into())
            })?;
            inputs = inputs.with_declaration_path(
                author,
                operand.source_span.span,
                operand.path.as_bytes(),
            );
        }
        let (assembled, completed) = self.deferred.assemble_returned_successor(
            &inputs,
            &mut self.timings,
            &mut *self.preparation.catalogue()?,
        )?;
        let projected = &assembled.parsed().files;
        let activation_owner = build
            .selected_build_declaration()
            .and_then(|entry| projected.rejoined_source_id(entry.source()))
            .and_then(|id| projected.get(id))
            .and_then(|file| file.package_identity);
        let mut failure = None;
        let mut rejoin = |span: source_text::SourceSpan| match authors
            .file_at(span)
            .and_then(|source| projected.rejoined_source_id(source))
        {
            Some(id) => source_text::SourceSpan::new(id, span.span),
            None => {
                failure = Some(refusal(
                    "a provider selection no longer belongs to its preparing Build source".into(),
                ));
                span
            }
        };
        for event in &mut events {
            Arc::make_mut(&mut event.request).remap_source_spans(&mut rejoin);
            event.selecting_machine = event.selecting_machine.map(&mut rejoin);
            if let Some(environment) = &mut event.static_environment {
                Arc::make_mut(environment).remap_source_spans(&mut rejoin);
            }
        }
        if let Some(failure) = failure {
            return Err(failure);
        }
        let subject = completed.as_ref().map(|source| source.owner());
        let scope = SuppliedBuildLibrary::declaration_scope(libraries, subject).map(Arc::new);
        let declarations =
            assembled.prepare(&self.preparation, self.profile.target_name(), scope)?;
        let declared = events
            .iter()
            .map(|event| Arc::clone(&event.request))
            .collect::<Vec<_>>();
        let choices = declarations.interpreted_provider_selections(
            ProductPolicyInputs {
                activation_owner,
                activation: build.selected_build_declaration(),
                declared: &declared,
                executed: &events,
                ..Default::default()
            },
            subject,
        )?;
        Ok((choices, completed))
    }
}
