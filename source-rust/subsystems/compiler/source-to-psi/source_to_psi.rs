//! Run the source pipeline once over the complete file set for one stage.

#![forbid(unsafe_code)]

use diagnostics::Diagnostic;
mod parse_files;
pub use parse_files::{
    ParsedSourceBatch, ParsedSourceSummary, RetainedSourceParse, SourceDeclarations, SourceImport,
    SourceInput, SourceItems, SourceSyntax,
};
mod stage_sources;
pub use stage_sources::ParsedStageSources;

pub use lowered_program_to_checked_program::checking::ReturnedScalarFact;
mod build_invocation;
pub mod compile_time;
pub mod product;
pub use build_invocation::{
    BuildInvocationJudgment, BuildStage, BuildStageSource, CheckedBuildInvocation,
    ProductDeclarationSource,
};
mod package_handoff;
pub use package_handoff::{PackageHandoffDeclarations, package_handoff_declarations};

mod source_continuation;
mod source_judgment;
mod source_preparation;
mod stages;
pub use stages::*;

pub mod registered_tests;

use product::{ProductEntry, ProductSourceInputs, prepare_lower_and_check};
use std::sync::Arc;

pub type PublishedPsi = Arc<terminal_codec::CanonicalTerminalArtifact>;

/// Captured files include generated source from earlier stages.
/// Bindings belong to this compilation's selected packages and source identities.
pub struct StageSource<'a> {
    pub parsed: ParsedStageSources,
    pub product: ProductSourceInputs<'a>,
}

/// Return the source judgment's advisories as soon as the Product is checked.
pub fn check(source: StageSource<'_>) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let (product, resolved) = source.resolve()?;
    let checked = product.judge(resolved, ProductEntry::BuildBindings)?;
    Ok(checked
        .terminal_publication
        .accepted()?
        .advisories()
        .to_vec())
}

pub fn compile(
    source: StageSource<'_>,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<PublishedPsi, Vec<Diagnostic>> {
    publish_stage(source, ProductEntry::BuildBindings, advisories).map(|(artifact, _)| artifact)
}

/// Source choices for one deployment, judged by the same stage that published the Psi.
pub struct PublishedTargetProgram {
    subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    artifact: PublishedPsi,
    providers: provider_supply::PublishedTargetProviders,
    exclusions: Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
    runtime_declarations: RuntimePermissionDeclarations,
}

/// The checked declarations used to publish this particular deployment.
#[derive(Clone)]
pub struct RuntimePermissionDeclarations {
    source: Arc<source_state::FrozenTypedTrees>,
    has_root_grants: bool,
    has_assumed_bindings: bool,
}

impl RuntimePermissionDeclarations {
    pub fn requirement_path(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        service_path: &str,
        callable: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        let source = &self.source;
        let symbols = &source.symbols;
        let mut services = source.traits().iter().filter(|service| {
            service.is_boundary
                && symbols.symbol_package_identity(service.symbol) == Some(package)
                && symbols.display_path(service.symbol, "::") == service_path
        });
        let (Some(service), None) = (services.next(), services.next()) else {
            return Err(vec![Diagnostic::error(format!(
                "runtime permission has no unique checked service declaration for `{service_path}`"
            ))]);
        };
        let signatures = source.trait_machine_signatures(service);
        let mut methods = signatures.iter().filter(|method| {
            symbols.symbol_package_identity(method.symbol) == Some(package)
                && source.trait_requirement_callable_identity_text(service, method) == callable
        });
        let (Some(method), None) = (methods.next(), methods.next()) else {
            return Err(vec![Diagnostic::error(format!(
                "runtime permission has no unique checked method `{callable}` on `{service_path}`"
            ))]);
        };
        Ok(symbols.display_path(method.symbol, "::"))
    }
}

impl PublishedTargetProgram {
    pub fn subject(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.subject
    }

    pub fn runtime_permission_declarations(
        &self,
    ) -> Result<RuntimePermissionDeclarations, Vec<Diagnostic>> {
        if self.runtime_declarations.has_root_grants {
            return Err(vec![Diagnostic::error(
                "the accepted policy schema cannot compare this Build's authored root grants",
            )]);
        }
        if self.runtime_declarations.has_assumed_bindings {
            return Err(vec![Diagnostic::error(
                "the checked Build's assumed binding lacks its canonical review request",
            )]);
        }
        Ok(self.runtime_declarations.clone())
    }

    pub fn into_parts(
        self,
    ) -> (
        PublishedPsi,
        provider_supply::PublishedTargetProviders,
        Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
    ) {
        (self.artifact, self.providers, self.exclusions)
    }
}

pub fn compile_for_target(
    source: StageSource<'_>,
    target: &str,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    if !source.product.calling_targets.contains(&target) {
        return Err(vec![Diagnostic::error(format!(
            "source program did not select deployment target `{target}`"
        ))]);
    }
    let subject = source.product.subject;
    let (artifact, checked) = publish_stage(source, ProductEntry::BuildBindings, advisories)?;
    published_target(artifact, checked, target, subject)
}

fn published_target(
    artifact: PublishedPsi,
    checked: product::CompletedProductSource,
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

pub fn inspect(source: StageSource<'_>, machine: &str) -> Result<PublishedPsi, Vec<Diagnostic>> {
    publish_stage(source, ProductEntry::Inspection(machine), None).map(|(artifact, _)| artifact)
}

fn publish_stage(
    source: StageSource<'_>,
    entry: ProductEntry<'_>,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<(PublishedPsi, product::CompletedProductSource), Vec<Diagnostic>> {
    let (product, continuation) = source.resolve()?;
    let checked = product.judge(continuation, entry)?;
    let psi = product.publish(&checked)?;
    if let Some(advisories) = advisories {
        advisories.extend_from_slice(checked.terminal_publication.accepted()?.advisories());
    }
    Ok((psi, checked))
}

impl<'a> StageSource<'a> {
    fn resolve(self) -> Result<(ProductSourceInputs<'a>, SourceContinuation), Vec<Diagnostic>> {
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
    fn judge(
        &self,
        continuation: SourceContinuation,
        entry: ProductEntry<'_>,
    ) -> Result<product::CompletedProductSource, Vec<Diagnostic>> {
        prepare_lower_and_check(
            continuation,
            self.clone(),
            entry,
            &mut CompileTimings::default(),
        )
    }

    fn publish(
        &self,
        checked: &product::CompletedProductSource,
    ) -> Result<PublishedPsi, Vec<Diagnostic>> {
        let psi = checked.terminal_publication.publish(
            &self.optimizations,
            self.declaration_authority,
            &mut CompileTimings::default(),
        )?;
        Ok(Arc::clone(psi.artifact()))
    }
}
