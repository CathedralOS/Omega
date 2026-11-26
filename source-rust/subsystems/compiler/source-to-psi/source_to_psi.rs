//! Check or publish Omega source as Psi. Each request below names its stopping
//! point or entry. The source driver owns the numbered transformations.

#![forbid(unsafe_code)]

mod driver;

pub use compile_product::ProductDeclarations;
pub use driver::compile_build::{
    BuildInvocationJudgment, BuildStageSource, CheckedBuildInvocation, PendingBuildImport,
    PreparedBuildInvocation, ProductDeclarationSource, TypedBuildInterval,
};
pub use driver::package_handoff::{
    InterpretedProviderSelections, PackageHandoffDeclarations, package_handoff_declarations,
};
pub use driver::parse_source_batch::{
    ParsedSourceBatch, ParsedSourceSummary, RetainedSourceParse, SourceDeclarations, SourceImport,
    SourceImportTarget, SourceInput, SourceItems, SourceSyntax,
};
pub use driver::stage_sources::ParsedStageSources;
pub use driver::{compile_product, compile_time_invocations, registered_tests};
pub use representations::{PublishedTargetProgram, RuntimePermissionDeclarations, publication};

pub mod representations;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use artifacts::compile_timings::CompileTimings;
pub use driver::prepare_typed_source::SourcePreparationFailure;
pub use driver::resolve_and_type::SourcePreparation;
pub use source_declarations::source_selections::RootBinding;
pub use source_declarations::source_selections::WireCompatibilityDemand;

use representations::{declaration_selection, source_inputs, source_state, source_symbols};

use compile_product::{ProductEntry, ProductSourceInputs};
use diagnostics::Diagnostic;
use driver::resolve_and_type::SourceTypingCheckpoint;
use std::sync::Arc;

pub type PublishedPsi = Arc<terminal_codec::CanonicalTerminalArtifact>;

/// Captured files include generated source from earlier stages.
/// Bindings belong to this compilation's selected packages and source identities.
pub struct StageSource<'a> {
    /// Source requests own the parser tables. Boxing them keeps saved Psi requests small.
    pub parsed: Box<ParsedStageSources>,
    pub product: ProductSourceInputs<'a>,
}

/// Return the source judgment's advisories as soon as the Product is checked.
pub fn check(
    source: StageSource<'_>,
    timings: &mut CompileTimings,
) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let (product, resolved) = source.resolve(timings)?;
    let checked = product.judge(
        resolved,
        ProductEntry::BuildBindings,
        &mut Default::default(),
        timings,
    )?;
    checked.admit_dependency_uses()?;
    checked.terminal_publication.check(
        product.declaration_scope.as_deref().map(|scope| scope as _),
        timings,
    )
}

pub fn compile(
    source: StageSource<'_>,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<PublishedPsi, Vec<Diagnostic>> {
    driver::publish_stage(
        source,
        ProductEntry::BuildBindings,
        advisories,
        &mut Default::default(),
        &mut CompileTimings::default(),
    )
    .and_then(|(artifact, checked)| {
        checked.admit_dependency_uses()?;
        Ok(artifact)
    })
}

pub fn compile_for_target(
    source: StageSource<'_>,
    target: &str,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    compile_for_target_with_timings(source, target, advisories, &mut CompileTimings::default())
}

pub fn compile_for_target_with_timings(
    source: StageSource<'_>,
    target: &str,
    advisories: Option<&mut Vec<Diagnostic>>,
    timings: &mut CompileTimings,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    let (artifact, checked, subject) = publish_deployment(
        source,
        target,
        ProductEntry::BuildBindings,
        advisories,
        &mut Default::default(),
        timings,
    )?;
    driver::published_target(artifact, &checked, target, subject)
}

/// Check the source once and publish it for each of `targets`, in order.
///
/// Lowering and checking do not depend on which deployment is asked for, so
/// every deployment shares one artifact and receives its own provider supply.
/// This is how checking for one target still checks the other targets' Build
/// rows (build/configuration.md, "Multi-target compilation").
pub fn compile_for_targets_with_timings(
    source: StageSource<'_>,
    targets: &[&str],
    advisories: Option<&mut Vec<Diagnostic>>,
    timings: &mut CompileTimings,
) -> Result<Vec<PublishedTargetProgram>, Vec<Diagnostic>> {
    let Some(&first) = targets.first() else {
        return Err(vec![Diagnostic::error(
            "source publication requires at least one deployment target",
        )]);
    };
    if let Some(missing) = targets
        .iter()
        .find(|target| !source.product.calling_targets.contains(target))
    {
        return Err(vec![Diagnostic::error(format!(
            "source program did not select deployment target `{missing}`"
        ))]);
    }
    let (artifact, checked, subject) = publish_deployment(
        source,
        first,
        ProductEntry::BuildBindings,
        advisories,
        &mut Default::default(),
        timings,
    )?;
    targets
        .iter()
        .map(|target| driver::published_target(Arc::clone(&artifact), &checked, target, subject))
        .collect()
}

pub fn review_package(
    source: StageSource<'_>,
    target: &str,
    accounting: &mut crate::compile_time_invocations::SourceCompletionAccounting,
) -> Result<(CheckedPackageDeclarations, PublishedTargetProgram), Vec<Diagnostic>> {
    let package = source.product.subject.ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires a named source subject",
        )]
    })?;
    let (artifact, mut checked, subject) = publish_deployment(
        source,
        target,
        ProductEntry::PackageReview,
        None,
        accounting,
        &mut CompileTimings::default(),
    )?;
    checked.retain_package_review_bindings(package, target)?;
    let declarations = CheckedPackageDeclarations {
        published_program: artifact.view().identity(),
        publication: Arc::clone(checked.terminal_publication()),
        explicit_providers: checked.explicit_providers.clone(),
        native_supplies: checked.native_supplies.clone(),
        service_audits: std::mem::take(&mut checked.declarations.service_audits),
        root_grants: checked.grants.clone(),
    };
    let target = driver::published_target(artifact, &checked, target, subject)?;
    Ok((declarations, target))
}

fn publish_deployment(
    source: StageSource<'_>,
    target: &str,
    entry: ProductEntry<'_>,
    advisories: Option<&mut Vec<Diagnostic>>,
    accounting: &mut crate::compile_time_invocations::SourceCompletionAccounting,
    timings: &mut CompileTimings,
) -> Result<
    (
        PublishedPsi,
        compile_product::CompletedProductSource,
        Option<semantic_vocabulary::PackageKeyIdentity>,
    ),
    Vec<Diagnostic>,
> {
    if !source.product.calling_targets.contains(&target) {
        return Err(vec![Diagnostic::error(format!(
            "source program did not select deployment target `{target}`"
        ))]);
    }
    let subject = source.product.subject;
    let (artifact, checked) =
        driver::publish_stage(source, entry, advisories, accounting, timings)?;
    Ok((artifact, checked, subject))
}

/// A complete source judgment and its deployment share one frozen owner.
pub struct CheckedPackageDeclarations {
    published_program: terminal_psi::TerminalPsiIdentity,
    explicit_providers: Vec<compile_product::provider_declarations::ProviderSelection>,
    native_supplies: Vec<checked_program_to_psi::provider_choice_inputs::NativeBuildSupply>,
    service_audits: Vec<symbols::SymbolHandle>,
    root_grants: Vec<symbols::root_grants::AuthoredRootGrant>,
    publication: Arc<driver::judge_lowered_source::SourcePublication>,
}

impl CheckedPackageDeclarations {
    pub fn published_program(&self) -> terminal_psi::TerminalPsiIdentity {
        self.published_program
    }
    pub fn explicit_providers(
        &self,
    ) -> &[compile_product::provider_declarations::ProviderSelection] {
        &self.explicit_providers
    }
    pub fn native_supplies(
        &self,
    ) -> &[checked_program_to_psi::provider_choice_inputs::NativeBuildSupply] {
        &self.native_supplies
    }
    pub fn service_audits(&self) -> &[symbols::SymbolHandle] {
        &self.service_audits
    }
    pub fn root_grants(&self) -> &[symbols::root_grants::AuthoredRootGrant] {
        &self.root_grants
    }
    pub fn accepted(&self) -> Result<&publication::AcceptedTerminalSource, Vec<Diagnostic>> {
        self.publication.accepted()
    }

    pub fn supplied_declarations(
        &self,
        target: &str,
    ) -> Result<Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>, Vec<Diagnostic>> {
        let roster = self.publication.roster()?;
        let source = self.accepted()?.source();
        let mut declarations: Vec<_> = roster
            .supplies
            .iter()
            .map(|supply| (supply.slot, supply.provider))
            .collect();
        // A provider already realized during normalization needs no additional
        // body judgment during settlement. Both belong to this accepted roster.
        for choice in &roster.normalization.providers {
            if choice.target == target {
                let schema = choice.schema.scope(source).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked provider selection lost its source schema",
                    )]
                })?;
                declarations.push((schema.declaration.symbol, choice.provider));
            }
        }
        Ok(declarations)
    }
}

pub fn inspect(source: StageSource<'_>, machine: &str) -> Result<PublishedPsi, Vec<Diagnostic>> {
    driver::publish_stage(
        source,
        ProductEntry::Inspection(machine),
        None,
        &mut Default::default(),
        &mut CompileTimings::default(),
    )
    .and_then(|(artifact, checked)| {
        checked.admit_dependency_uses()?;
        Ok(artifact)
    })
}
