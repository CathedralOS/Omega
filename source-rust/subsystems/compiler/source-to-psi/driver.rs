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
) -> Result<(PublishedPsi, CompletedProductSource), Vec<Diagnostic>> {
    let (product, continuation) = source.resolve()?;
    let checked = product.judge(continuation, entry)?;
    let psi = product.publish(&checked)?;
    if let Some(advisories) = advisories {
        advisories.extend_from_slice(checked.terminal_publication.accepted()?.advisories());
    }
    Ok((psi, checked))
}

impl<'a> StageSource<'a> {
    pub(crate) fn resolve(
        self,
    ) -> Result<(ProductSourceInputs<'a>, SourceTypingCheckpoint), Vec<Diagnostic>> {
        let primary = self
            .parsed
            .roots
            .first()
            .copied()
            .ok_or_else(|| vec![Diagnostic::error("Product stage has no root source")])?;
        let mut request = self
            .parsed
            .into_request(self.product.build_host.canonical_name(), primary)?;
        request.selection_authority = self.product.layout_selection_authority.clone();
        let continuation = request
            .with_build_entry(self.product.policy.activation)
            .prepare(&mut CompileTimings::default())?;
        Ok((self.product, continuation))
    }
}

impl ProductSourceInputs<'_> {
    pub(crate) fn judge(
        &self,
        continuation: SourceTypingCheckpoint,
        entry: ProductEntry<'_>,
    ) -> Result<CompletedProductSource, Vec<Diagnostic>> {
        prepare_lower_and_check(
            continuation,
            self.clone(),
            entry,
            &mut CompileTimings::default(),
        )
    }

    pub(crate) fn publish(
        &self,
        checked: &CompletedProductSource,
    ) -> Result<PublishedPsi, Vec<Diagnostic>> {
        let psi = checked.terminal_publication.publish(
            &self.optimizations,
            self.declaration_authority,
            &mut CompileTimings::default(),
        )?;
        Ok(Arc::clone(psi.artifact()))
    }
}

pub(crate) fn published_target(
    artifact: PublishedPsi,
    checked: compile_product::CompletedProductSource,
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
        exclusions: checked.exclusions,
        runtime_declarations: RuntimePermissionDeclarations {
            source: checked.program,
            has_root_grants: !checked.grants.is_empty(),
            has_assumed_bindings: !checked.declarations.assumed_bindings.is_empty(),
        },
    })
}
