//! Run one requested source compilation. Parse batches stream through 00–01.
//! Build can pause the 02–03 resolver and typer to query declarations or extend
//! the package scope. Product and Build both prepare typed source and use the
//! same 05–06 judgment before publishing through 07.
//! Compile-time calls run checked invocations and return their values to preparation.

pub(crate) mod compile_build;
pub mod compile_product;
pub mod compile_time_invocations;
pub(crate) mod judge_lowered_source;
pub(crate) mod package_handoff;
pub(crate) mod parse_source_batch;
pub(crate) mod prepare_typed_source;
pub mod registered_tests;
pub(crate) mod resolve_and_type;
pub(crate) mod stage_sources;

use crate::{
    CompileTimings, PublishedPsi, PublishedTargetProgram, RuntimePermissionDeclarations,
    SourceTypingCheckpoint, StageSource,
};
use compile_product::{
    CompletedProductSource, ProductEntry, ProductSourceInputs, prepare_lower_and_check,
};
use diagnostics::Diagnostic;
use std::sync::Arc;

pub(crate) fn publish_stage(
    source: StageSource<'_>,
    entry: ProductEntry<'_>,
    advisories: Option<&mut Vec<Diagnostic>>,
    accounting: &mut crate::compile_time_invocations::SourceCompletionAccounting,
    timings: &mut CompileTimings,
) -> Result<(PublishedPsi, CompletedProductSource), Vec<Diagnostic>> {
    let (product, continuation) = source.resolve(timings)?;
    let checked = product.judge(continuation, entry, accounting, timings)?;
    let psi = product.publish(&checked, timings)?;
    if let Some(advisories) = advisories {
        // The checker searches for its advisories only when they are read,
        // so their cost is recorded here rather than in Psi 06.
        let found = timings.record_result(artifacts::compile_timings::source::ADVISORIES, || {
            Ok::<_, Vec<Diagnostic>>(checked.terminal_publication.advisories())
        })?;
        advisories.extend_from_slice(found);
    }
    Ok((psi, checked))
}

impl<'a> StageSource<'a> {
    pub(crate) fn resolve(
        self,
        timings: &mut CompileTimings,
    ) -> Result<(ProductSourceInputs<'a>, SourceTypingCheckpoint), Vec<Diagnostic>> {
        self.resolve_with(timings, |request, timings| request.prepare(timings))
    }

    /// Resolve and type declarations for dependency Build handoff without
    /// eagerly typing Product bodies that handoff never reads.
    pub(crate) fn resolve_declarations(
        self,
        timings: &mut CompileTimings,
    ) -> Result<(ProductSourceInputs<'a>, SourceTypingCheckpoint), Vec<Diagnostic>> {
        self.resolve_with(timings, |request, timings| {
            request.prepare_declarations(timings)
        })
    }

    fn resolve_with(
        self,
        timings: &mut CompileTimings,
        prepare: impl FnOnce(
            resolve_and_type::ResolveAndTypeRequest,
            &mut CompileTimings,
        ) -> Result<SourceTypingCheckpoint, Vec<Diagnostic>>,
    ) -> Result<(ProductSourceInputs<'a>, SourceTypingCheckpoint), Vec<Diagnostic>> {
        let primary = self
            .parsed
            .roots
            .first()
            .copied()
            .ok_or_else(|| vec![Diagnostic::error("Product stage has no root source")])?;
        let mut request = self
            .parsed
            .into_request(
                self.product.build_host.canonical_name(),
                source_provenance::DependencyScope::Product,
                primary,
            )?;
        request.selection_authority = self.product.declaration_scope.clone();
        let continuation = prepare(
            request.with_build_entry(self.product.policy.activation),
            timings,
        )?;
        Ok((self.product, continuation))
    }
}

impl ProductSourceInputs<'_> {
    pub(crate) fn judge(
        &self,
        continuation: SourceTypingCheckpoint,
        entry: ProductEntry<'_>,
        accounting: &mut crate::compile_time_invocations::SourceCompletionAccounting,
        timings: &mut CompileTimings,
    ) -> Result<CompletedProductSource, Vec<Diagnostic>> {
        prepare_lower_and_check(continuation, self.clone(), entry, accounting, timings)
    }

    pub(crate) fn publish(
        &self,
        checked: &CompletedProductSource,
        timings: &mut CompileTimings,
    ) -> Result<PublishedPsi, Vec<Diagnostic>> {
        checked.terminal_publication.publish(
            &self.optimizations,
            self.declaration_scope.as_deref().map(|scope| scope as _),
            timings,
        )
    }
}

/// One deployment of a checked source judgment. Several deployments of the
/// same judgment share its artifact and differ only in their provider supply.
pub(crate) fn published_target(
    artifact: PublishedPsi,
    checked: &compile_product::CompletedProductSource,
    target: &str,
    subject: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    let providers = checked
        .prepare_provider_demand(target)?
        .for_published_target(checked.terminal_publication(), target)?;
    Ok(PublishedTargetProgram {
        subject,
        artifact,
        providers,
        exclusions: checked.exclusions.clone(),
        runtime_declarations: RuntimePermissionDeclarations {
            source: Arc::clone(&checked.program),
            root_grants: checked.grants.as_slice().into(),
        },
    })
}
