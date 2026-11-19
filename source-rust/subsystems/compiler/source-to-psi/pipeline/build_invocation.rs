//! Compile the Build source selected for one planning stage. The host may
//! inspect the checked program to admit filesystem access before publishing it.
use crate::evaluation::{
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
        ResolveAndTypeRequest {
            syntax: self.syntax,
            source_items: self.source_items,
            root_sources: self.roots,
            frontends: preparation.frontends.clone(),
            build_helper_identity: host,
            build_sources: self.build_sources,
            build_stage: Some((self.entry, index)),
            sources: self.sources,
            top_level_bindings: self.imports,
            selection_authority: authority,
        }
        .prepare(&mut UnmeasuredProduction)
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
