//! Compile the Build source selected for one planning stage. The host may
//! inspect the checked program to admit filesystem access before publishing it.
use crate::compile_time::{
    BuildTimeSelectionAuthority, InvocationSourceConsent, SourceCompletionAccounting,
};
use crate::source_inputs::SourceMap;
use crate::source_state::TypedTrees;
use crate::source_symbols::SourceScopedTopLevelBinding;
use crate::{
    AcceptedTerminalSource, ConstantRead, ProducedTerminalProgram, ResolveAndTypeRequest,
    SourceAdmission, SourceContinuation, SourceItems, SourcePreparation, SourcePreparationFailure,
    SourcePreparationRequest, SourceSyntax, TerminalProductionCustody, TerminalSourceCompilation,
    UnmeasuredProduction,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use source_declarations::build_stage::BuildStage;
use source_text::SourceId;
use std::collections::HashSet;
use std::sync::Arc;
use symbols::SymbolHandle;

/// The parsed files selected by the package capture, including an explicit Build entry.
pub struct BuildStageSource {
    pub syntax: SourceSyntax,
    pub source_items: Vec<(SourceId, SourceItems)>,
    pub roots: Vec<SourceId>,
    pub build_sources: HashSet<SourceId>,
    pub sources: Arc<SourceMap>,
    pub imports: Vec<SourceScopedTopLevelBinding>,
    pub entry: SourceId,
}

impl BuildStageSource {
    pub fn prepare(
        self,
        index: usize,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<(SourceContinuation, BuildStage), Vec<Diagnostic>> {
        let entry = self.entry;
        self.request(preparation, host, authority, Some((entry, index)))
            .prepare(&mut UnmeasuredProduction)
    }

    /// Product descriptions use the complete declarations visible at the paused Build call.
    pub fn prepare_product_query(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<SourceContinuation, Vec<Diagnostic>> {
        self.request(preparation, host, authority, None)
            .prepare(&mut UnmeasuredProduction)
            .map(|(declarations, _)| declarations)
    }

    fn request(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        build_stage: Option<(SourceId, usize)>,
    ) -> ResolveAndTypeRequest {
        ResolveAndTypeRequest {
            syntax: self.syntax,
            source_items: self.source_items,
            root_sources: self.roots,
            frontends: preparation.frontends.clone(),
            build_helper_identity: host,
            build_sources: self.build_sources,
            build_stage,
            sources: self.sources,
            top_level_bindings: self.imports,
            selection_authority: authority,
        }
    }
}

pub struct BuildInvocationJudgment {
    pub invocation: CheckedBuildInvocation,
    pub constants: Vec<ConstantRead>,
    pub accounting: SourceCompletionAccounting,
}

/// Carries the source consent used to check this invocation through publication.
pub struct CheckedBuildInvocation {
    accepted: AcceptedTerminalSource,
    invocation_sources: Option<Arc<dyn InvocationSourceConsent>>,
}

impl CheckedBuildInvocation {
    pub fn judge(
        typed: &TypedTrees,
        entry: SymbolHandle,
        subjects: &[Option<PackageKeyIdentity>],
        invocation_sources: Option<Arc<dyn InvocationSourceConsent>>,
    ) -> Result<BuildInvocationJudgment, SourcePreparationFailure> {
        let judged = SourcePreparationRequest {
            program: typed.clone(),
            accounting: Default::default(),
            admission: SourceAdmission {
                invocation_sources: invocation_sources.clone(),
                ..Default::default()
            },
            compilation: TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects,
                purpose: source_provenance::DependencyScope::Build,
                selected: &[],
                calling: None,
                providers: None,
                bindings: &[],
                supplies: &[],
            },
        }
        .judge(terminal_codec::TerminalPublication::Invocation)?;
        Ok(BuildInvocationJudgment {
            invocation: Self {
                accepted: judged.accepted,
                invocation_sources,
            },
            constants: judged.constants,
            accounting: judged.accounting,
        })
    }

    pub fn accepted(&self) -> &AcceptedTerminalSource {
        &self.accepted
    }

    pub fn publish(
        self,
    ) -> Result<(AcceptedTerminalSource, ProducedTerminalProgram), Vec<Diagnostic>> {
        let produced = self.accepted.produce(
            TerminalProductionCustody {
                invocation_sources: self.invocation_sources.as_deref(),
                ..Default::default()
            },
            &mut UnmeasuredProduction,
        )?;
        Ok((self.accepted, produced))
    }
}

impl crate::StageSource<'_> {
    /// Check the selected package and the declared implementation before installing its code in Build.
    pub fn produce_build_provider(
        &self,
        deployment: &str,
        declaration_path: &str,
    ) -> Result<crate::ProducedBuildProvider, Vec<Diagnostic>> {
        let package = self.selections.subject.ok_or_else(|| {
            vec![Diagnostic::error(
                "Build provider production requires an acquired package",
            )]
        })?;
        if !self.selections.calling_targets.contains(&deployment) {
            return Err(vec![Diagnostic::error(format!(
                "Build provider source did not select deployment target `{deployment}`"
            ))]);
        }
        let checked = self.judge(self.resolve()?, None, None)?;
        let publication = checked.terminal_publication();
        let accepted = publication.accepted()?;
        let supply = AcceptedTerminalSource::build_backing_supply(
            accepted.source(),
            package,
            declaration_path,
        )?;
        let mut roster = publication.unpublished_roster()?.clone();
        if !roster.supplies.contains(&supply) {
            roster.supplies.push(supply);
        }
        let supplied = crate::SourcePublication::judge(
            accepted.source().clone(),
            roster,
            publication.optimizations().clone(),
        )?;
        supplied
            .accepted()?
            .produce_build_provider(package, declaration_path)
    }
}
