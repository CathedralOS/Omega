//! Run the source pipeline once over the complete file set for one stage.

#![forbid(unsafe_code)]

use diagnostics::Diagnostic;
use source_files::{LoadedSource, LoadedSources};

#[path = "pipeline/build_invocation.rs"]
mod authored_build;
pub use authored_build::{BuildInvocationJudgment, BuildStageSource, CheckedBuildInvocation};

#[path = "pipeline/modules.rs"]
mod stages;
pub use stages::*;

#[path = "pipeline/registered_tests.rs"]
pub mod registered_tests;

use product_source::{ProductSourceInputs, prepare_lower_and_check};
use crate::source_inputs::SourceMap;
use crate::source_symbols::SourceScopedTopLevelBinding;
use source_text::SourceId;
use std::sync::Arc;

pub type PublishedPsi = Arc<terminal_codec::CanonicalTerminalArtifact>;

/// Captured files include generated source from earlier stages.
/// Bindings belong to this compilation's selected packages and source identities.
pub struct StageSource<'a> {
    pub files: Arc<SourceMap>,
    pub roots: Vec<SourceId>,
    pub imports: Vec<SourceScopedTopLevelBinding>,
    pub selections: ProductSourceInputs<'a>,
    /// Costs of the Build that opened this Product, when it belongs to a project.
    pub build_accounting: Option<&'a evaluation::SourceCompletionAccounting>,
    pub declaration_authority:
        Option<&'a dyn declaration_scope::DeclarationSelectionAuthority>,
    pub build_host: semantic_vocabulary::source_selections::ProductTarget,
}

pub fn compile(source: StageSource<'_>) -> Result<PublishedPsi, Vec<Diagnostic>> {
    compile_reporting(source, None)
}

pub fn compile_reporting(
    source: StageSource<'_>,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<PublishedPsi, Vec<Diagnostic>> {
    publish_stage(source, None, advisories).map(|(artifact, _)| artifact)
}

/// Source choices for one deployment, judged by the same stage that published the Psi.
pub struct PublishedTargetProgram {
    artifact: PublishedPsi,
    providers: psi_provider_selections::publication::PublishedTargetProviders,
    exclusions: Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
}

impl PublishedTargetProgram {
    pub fn into_parts(self) -> (
        PublishedPsi,
        psi_provider_selections::publication::PublishedTargetProviders,
        Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
    ) {
        (self.artifact, self.providers, self.exclusions)
    }
}

pub fn compile_for_target(
    source: StageSource<'_>,
    target: &str,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    compile_for_target_reporting(source, target, None)
}

pub fn compile_for_target_reporting(
    source: StageSource<'_>,
    target: &str,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    if !source.selections.calling_targets.contains(&target) {
        return Err(vec![Diagnostic::error(format!(
            "source program did not select deployment target `{target}`"
        ))]);
    }
    let (artifact, checked) = publish_stage(source, None, advisories)?;
    published_target(artifact, checked, target)
}

fn published_target(
    artifact: PublishedPsi,
    checked: product_source::CompletedProductSource,
    target: &str,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    let demand = checked
        .prepare_provider_demand(target)?
        .receive(checked.terminal_publication(), Some(target))?;
    let providers = psi_provider_selections::publication::PublishedTargetProviders::new(
        &artifact, target, demand,
    );
    Ok(PublishedTargetProgram {
        artifact,
        providers,
        exclusions: checked.exclusions,
    })
}

pub fn inspect(source: StageSource<'_>, machine: &str) -> Result<PublishedPsi, Vec<Diagnostic>> {
    publish_stage(source, Some(machine), None).map(|(artifact, _)| artifact)
}

fn publish_stage(
    source: StageSource<'_>,
    inspection: Option<&str>,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<(PublishedPsi, product_source::CompletedProductSource), Vec<Diagnostic>> {
    let mut observer = UnmeasuredProduction;
    let parsed = lex_and_parse(&source, &mut observer)?;
    let (continuation, _) = parsed.prepare(&mut observer)?;
    let optimizations = source.selections.optimizations.clone();
    let standalone = evaluation::SourceCompletionAccounting::default();
    let checked = prepare_lower_and_check(
        continuation,
        source.selections,
        source.build_accounting.unwrap_or(&standalone),
        inspection,
        None,
        None,
        &mut observer,
    )?;
    let psi = checked.terminal_publication.publish(
        &optimizations, source.declaration_authority, &mut observer,
    )?;
    if let Some(advisories) = advisories {
        advisories.extend_from_slice(checked.terminal_publication.accepted()?.advisories());
    }
    Ok((Arc::clone(psi.artifact()), checked))
}

fn lex_and_parse(
    source: &StageSource<'_>,
    observer: &mut impl PsiCompilationObserver,
) -> Result<ResolveAndTypeRequest, Vec<Diagnostic>> {
    let Some(&primary) = source.roots.first() else {
        return Err(vec![Diagnostic::error("stage has no root source")]);
    };
    if source.roots.iter().any(|id| !source.files.source_is_selected(*id)) {
        return Err(vec![Diagnostic::error(
            "stage root is outside its selected sources",
        )]);
    }
    let files: LoadedSources = source.files.files()
        .filter(|file| source.files.source_is_selected(file.source_id))
        .map(|file| LoadedSource {
            source_id: file.source_id,
            path: file.path.clone(),
            source: Arc::clone(&file.source),
            origin: Some(file.origin),
            scope: file.dependency_scope,
        })
        .collect();
    let mut syntax = SourceSyntax::new(primary);
    let parsed = SourceInput(files).append(&mut syntax, observer)?;
    let source_items = parsed.sources.span_or_empty(parsed.batch)
        .iter()
        .map(|unit| (unit.source_id, unit.root_items.clone()))
        .collect();
    Ok(ResolveAndTypeRequest {
        syntax,
        source_items,
        root_sources: source.roots.clone(),
        sources: Arc::clone(&source.files),
        top_level_bindings: source.imports.clone(),
        selection_authority: source.selections.layout_selection_authority.clone(),
        build_helper_identity: source.build_host.canonical_name(),
        build_sources: Default::default(),
        build_stage: None,
        frontends: Default::default(),
    })
}
