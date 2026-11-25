//! Check the complete Build program before its activation. The host may
//! inspect the checked program to admit filesystem access before publishing it.
use crate::compile_time_invocations::{InvocationSourceConsent, SourceCompletionAccounting};
use crate::driver::judge_lowered_source;
use crate::driver::prepare_typed_source::{
    SourceAdmission, SourcePreparationFailure, SourcePreparationRequest,
};
use crate::driver::resolve_and_type::ResolveAndTypeRequest;
use crate::source_state::TypedTrees;
use crate::{CompileTimings, ParsedStageSources, SourcePreparation};
use checked_program_to_psi as production;
use checked_program_to_psi::{
    AcceptedTerminalSource, BuildProtocolUse, ConstantRead, PreparedTerminalPublication,
    ProducedTerminalProgram, TerminalSourceCompilation,
};
use declaration_scope::PackageDeclarationScope;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
mod target_helpers;
use source_text::{SourceId, SourceSpan, Span};
use std::collections::HashSet;
use std::sync::Arc;
use symbols::SymbolHandle;

/// A Build import without a supplied module binding for this invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingBuildImport {
    pub source: SourceId,
    pub span: Span,
    pub target: crate::SourceImportTarget,
}

/// The parsed files selected by the package capture, including an explicit Build entry.
pub struct BuildStageSource {
    pub parsed: ParsedStageSources,
    pub build_sources: HashSet<SourceId>,
    pub pending_imports: Vec<PendingBuildImport>,
    pub entry: source_text::SourceSpan,
}

/// The complete typed source of one Build invocation.
pub struct TypedBuildInterval {
    pub program: Arc<TypedTrees>,
}

/// Source selected for declaration queries issued by a suspended Build.
pub struct ProductDeclarationSource(BuildStageSource);

impl ProductDeclarationSource {
    pub fn new(source: BuildStageSource) -> Self {
        Self(source)
    }

    pub fn parsed(&self) -> &ParsedStageSources {
        &self.0.parsed
    }

    pub fn prepare(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<PackageDeclarationScope>>,
    ) -> Result<crate::compile_product::ProductDeclarations, Vec<Diagnostic>> {
        let typed = self.0.typed_source(preparation, host, authority)?;
        Ok(crate::compile_product::ProductDeclarations { typed })
    }
}

impl BuildStageSource {
    pub fn prepare(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<PackageDeclarationScope>>,
    ) -> Result<TypedBuildInterval, Vec<Diagnostic>> {
        if !self.pending_imports.is_empty() {
            return Err(self
                .pending_imports
                .into_iter()
                .map(|import| {
                    Diagnostic::error(format!(
                        "Build import `{}` has no supplied source",
                        import.target
                    ))
                    .with_source_span(SourceSpan::new(import.source, import.span))
                })
                .collect());
        }
        let program = self.typed_source(preparation, host, authority)?;
        program
            .check_build_provider_declarations()
            .map_err(|error| vec![error])?;
        Ok(TypedBuildInterval { program })
    }

    /// Rejoin the selected Build entry in the assembled Product sources so the
    /// Build evaluator can check the constants demanded by that entry.
    pub fn rejoin_build_entry(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<PackageDeclarationScope>>,
        subject: Option<PackageKeyIdentity>,
        declaration: &source_admission::RetainedDeclaration,
    ) -> Result<(Arc<TypedTrees>, SymbolHandle), Vec<Diagnostic>> {
        let typed = self.typed_source(preparation, host, authority)?;
        let entry = crate::source_state::source_subjects::rejoin_build_activation(
            &typed,
            subject,
            declaration,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok((typed, entry))
    }

    fn typed_source(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<PackageDeclarationScope>>,
    ) -> Result<Arc<TypedTrees>, Vec<Diagnostic>> {
        let checkpoint = self
            .request(preparation, host, authority)?
            .prepare(&mut CompileTimings::default())?;
        Ok(checkpoint.shared_typed())
    }

    fn request(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<PackageDeclarationScope>>,
    ) -> Result<ResolveAndTypeRequest, Vec<Diagnostic>> {
        let entry = self.parsed.files.get(self.entry.source_id).map(|file| {
            source_admission::RetainedDeclaration::from_source_occurrence(
                file.clone(),
                self.entry.span,
                symbols::SymbolKind::Machine,
            )
        });
        let mut request = self.parsed.into_request(host, self.entry.source_id)?;
        request.frontends = preparation.frontends.clone();
        target_helpers::select_build_helpers(&mut request.syntax.trees, host, &self.build_sources);
        request.selection_authority = authority;
        Ok(request.with_build_entry(entry.as_ref()))
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
        declaration_scope: Option<Arc<PackageDeclarationScope>>,
        invocation_sources: Option<Arc<dyn InvocationSourceConsent>>,
    ) -> Result<BuildInvocationJudgment, SourcePreparationFailure> {
        let judged = SourcePreparationRequest {
            program: typed.clone(),
            accounting: Default::default(),
            admission: SourceAdmission {
                invocation_sources: invocation_sources.clone(),
                layout_selection_authority: declaration_scope,
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
        let constants = judged.accepted.constants_read();
        Ok(BuildInvocationJudgment {
            invocation: Self {
                accepted: judged.accepted,
                invocation_sources,
            },
            constants,
            accounting: judged.accounting,
        })
    }

    pub fn accepted(&self) -> &AcceptedTerminalSource {
        &self.accepted
    }

    pub fn prepare_publication(self) -> Result<PreparedBuildInvocation, Vec<Diagnostic>> {
        let publication = self
            .accepted
            .prepare_publication(self.invocation_sources.as_deref())?;
        Ok(PreparedBuildInvocation {
            accepted: self.accepted,
            publication,
        })
    }
}

/// Keeps the host operations inspected during admission with the code that will run.
pub struct PreparedBuildInvocation {
    accepted: AcceptedTerminalSource,
    publication: PreparedTerminalPublication,
}

impl PreparedBuildInvocation {
    pub fn build_protocol_use(&self) -> BuildProtocolUse {
        self.publication.build_protocol_use()
    }

    pub fn publish(
        self,
    ) -> Result<(AcceptedTerminalSource, ProducedTerminalProgram), Vec<Diagnostic>> {
        let produced = self
            .publication
            .publish(&[], &mut CompileTimings::default())?;
        Ok((self.accepted, produced))
    }
}

impl crate::StageSource<'_> {
    /// Check the selected package and the declared implementation before installing its code in Build.
    pub fn produce_build_provider(
        self,
        deployment: &str,
        declaration_path: &str,
    ) -> Result<production::ProducedBuildProvider, Vec<Diagnostic>> {
        let package = self.product.subject.ok_or_else(|| {
            vec![Diagnostic::error(
                "Build provider production requires an acquired package",
            )]
        })?;
        if !self.product.calling_targets.contains(&deployment) {
            return Err(vec![Diagnostic::error(format!(
                "Build provider source did not select deployment target `{deployment}`"
            ))]);
        }
        let mut timings = crate::CompileTimings::default();
        let (product, resolved) = self.resolve(&mut timings)?;
        let checked = product.judge(
            resolved,
            crate::compile_product::ProductEntry::BuildBindings,
            &mut SourceCompletionAccounting::default(),
            &mut timings,
        )?;
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
        let supplied = judge_lowered_source::SourcePublication::judge(
            accepted.prepared().clone(),
            roster,
            publication.optimizations().clone(),
            &mut timings,
        )?;
        supplied
            .accepted()?
            .produce_build_provider(package, declaration_path)
    }
}
