//! Run the source pipeline once over the complete file set for one stage.

#![forbid(unsafe_code)]

use diagnostics::Diagnostic;
use source_files::{LoadedSource, LoadedSources};

#[path = "pipeline/build_invocation.rs"]
mod authored_build;
pub use authored_build::{BuildInvocationJudgment, BuildStageSource, CheckedBuildInvocation};
#[path = "pipeline/package_handoff.rs"]
mod package_handoff;
pub use package_handoff::{PackageHandoffDeclarations, package_handoff_declarations};

#[path = "pipeline/modules.rs"]
mod stages;
pub use stages::*;

#[path = "pipeline/registered_tests.rs"]
pub mod registered_tests;

use crate::source_inputs::SourceMap;
use crate::source_symbols::SourceScopedTopLevelBinding;
use product_source::{ProductSourceInputs, prepare_lower_and_check};
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
    pub declaration_authority: Option<&'a dyn declaration_scope::DeclarationSelectionAuthority>,
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
    subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    artifact: PublishedPsi,
    providers: psi_provider_selections::publication::PublishedTargetProviders,
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
    let subject = source.selections.subject;
    let (artifact, checked) = publish_stage(source, None, advisories)?;
    published_target(artifact, checked, target, subject)
}

fn published_target(
    artifact: PublishedPsi,
    checked: product_source::CompletedProductSource,
    target: &str,
    subject: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
    let demand = checked
        .prepare_provider_demand(target)?
        .receive(checked.terminal_publication(), Some(target))?;
    let providers = psi_provider_selections::publication::PublishedTargetProviders::new(
        &artifact, target, demand,
    );
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
    publish_stage(source, Some(machine), None).map(|(artifact, _)| artifact)
}

fn publish_stage(
    source: StageSource<'_>,
    inspection: Option<&str>,
    advisories: Option<&mut Vec<Diagnostic>>,
) -> Result<(PublishedPsi, product_source::CompletedProductSource), Vec<Diagnostic>> {
    let continuation = source.resolve()?;
    let checked = source.judge(continuation, inspection, None)?;
    let psi = source.publish(&checked)?;
    if let Some(advisories) = advisories {
        advisories.extend_from_slice(checked.terminal_publication.accepted()?.advisories());
    }
    Ok((psi, checked))
}

impl StageSource<'_> {
    fn resolve(&self) -> Result<SourceContinuation, Vec<Diagnostic>> {
        let mut observer = UnmeasuredProduction;
        let parsed = lex_and_parse(self, &mut observer)?;
        parsed
            .prepare(&mut observer)
            .map(|(continuation, _)| continuation)
    }

    fn judge(
        &self,
        continuation: SourceContinuation,
        inspection: Option<&str>,
        test: Option<(&registered_tests::TestRoot, &str)>,
    ) -> Result<product_source::CompletedProductSource, Vec<Diagnostic>> {
        prepare_lower_and_check(
            continuation,
            self.selections.clone(),
            self.build_accounting.unwrap_or(&Default::default()),
            inspection,
            test,
            None,
            &mut UnmeasuredProduction,
        )
    }

    fn publish(
        &self,
        checked: &product_source::CompletedProductSource,
    ) -> Result<PublishedPsi, Vec<Diagnostic>> {
        let psi = checked.terminal_publication.publish(
            &self.selections.optimizations,
            self.declaration_authority,
            &mut UnmeasuredProduction,
        )?;
        Ok(Arc::clone(psi.artifact()))
    }
}

fn lex_and_parse(
    source: &StageSource<'_>,
    observer: &mut impl PsiCompilationObserver,
) -> Result<ResolveAndTypeRequest, Vec<Diagnostic>> {
    let Some(&primary) = source.roots.first() else {
        return Err(vec![Diagnostic::error("stage has no root source")]);
    };
    if source
        .roots
        .iter()
        .any(|id| !source.files.source_is_selected(*id))
    {
        return Err(vec![Diagnostic::error(
            "stage root is outside its selected sources",
        )]);
    }
    let files: LoadedSources = source
        .files
        .files()
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
    let source_items = parsed
        .sources
        .span_or_empty(parsed.batch)
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
