//! Compile the Build source selected for one planning stage. The host may
//! inspect the checked program to admit filesystem access before publishing it.
use crate::compile_time_invocations::{
    BuildTimeSelectionAuthority, InvocationSourceConsent, SourceCompletionAccounting,
};
use crate::driver::judge_lowered_source;
use crate::driver::prepare_typed_source::{
    SourceAdmission, SourcePreparationFailure, SourcePreparationRequest,
};
use crate::driver::resolve_and_type::ResolveAndTypeRequest;
use crate::source_state::TypedTrees;
use crate::{CompileTimings, ParsedStageSources, SourcePreparation, SourceTypingCheckpoint};
use checked_program_to_psi::production;
use checked_program_to_psi::production::{
    AcceptedTerminalSource, ConstantRead, ProducedTerminalProgram, TerminalProductionCustody,
    TerminalSourceCompilation,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
mod target_helpers;
mod waits;
use source_text::SourceId;
use std::collections::HashSet;
use std::sync::Arc;
use symbols::SymbolHandle;
pub use waits::BuildStage;

/// The parsed files selected by the package capture, including an explicit Build entry.
pub struct BuildStageSource {
    pub parsed: ParsedStageSources,
    pub build_sources: HashSet<SourceId>,
    pub entry: source_text::SourceSpan,
}

/// One typed interval of the Build entry and the exact waits used to project it.
pub struct TypedBuildInterval {
    pub program: Arc<TypedTrees>,
    /// Its successor, preparation sites, frame parameters, and live homes saved at this boundary.
    pub plan: BuildStage,
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
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<crate::compile_product::ProductDeclarations, Vec<Diagnostic>> {
        let typed = self.0.typed_declarations(preparation, host, authority)?;
        Ok(crate::compile_product::ProductDeclarations { typed })
    }
}

impl BuildStageSource {
    pub fn prepare(
        self,
        previous: Option<&BuildStage>,
        facts: &[crate::ReturnedScalarFact],
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<TypedBuildInterval, Vec<Diagnostic>> {
        let (checkpoint, plan) =
            self.resolve_entry_plan(previous, facts, preparation, host, authority)?;
        Ok(TypedBuildInterval {
            program: checkpoint.shared_typed(),
            plan,
        })
    }

    /// Rejoin the selected Build entry in the assembled Product sources so the
    /// Build evaluator can check the constants demanded by that entry.
    pub fn rejoin_build_entry(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        subject: Option<PackageKeyIdentity>,
        declaration: &source_admission::RetainedDeclaration,
    ) -> Result<(Arc<TypedTrees>, SymbolHandle), Vec<Diagnostic>> {
        let typed = self.typed_declarations(preparation, host, authority)?;
        let entry = crate::source_state::source_subjects::rejoin_build_activation(
            &typed,
            subject,
            declaration,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok((typed, entry))
    }

    fn typed_declarations(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<Arc<TypedTrees>, Vec<Diagnostic>> {
        let checkpoint = self
            .request(preparation, host, authority)?
            .prepare(&mut CompileTimings::default())?;
        Ok(checkpoint.shared_typed())
    }

    fn resolve_entry_plan(
        self,
        previous: Option<&BuildStage>,
        facts: &[crate::ReturnedScalarFact],
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<(SourceTypingCheckpoint, BuildStage), Vec<Diagnostic>> {
        let entry = self.entry;
        let mut request = self.request(preparation, host, authority)?;
        let index = match previous {
            None => 0,
            Some(stage) => stage.next_index().ok_or_else(|| {
                vec![Diagnostic::error(
                    "the selected Build stage has already finished",
                )]
            })?,
        };
        let excluded = previous.map_or(&[][..], |stage| stage.excluded_homes.as_slice());
        let plan = waits::EntryPlan::new(&request.syntax.trees, entry, index, excluded)?;
        let candidates = if previous.is_none() {
            plan.registration_candidates(&request.syntax.trees)
        } else {
            Vec::new()
        };
        if candidates.is_empty() {
            return prepare_stage(request, entry, &plan, facts);
        }
        // The ordinary selected program already resolves each candidate's type.
        // Reproject only when a user declaration shadows the toolchain name.
        request.syntax.share_pages();
        let mut probe = request.clone();
        probe.frontends = request.frontends.fork()?;
        let excluded = match prepare_stage(probe, entry, &plan, facts) {
            Ok((source, stage)) => {
                let excluded = unsupported_registration_homes(source.typed(), entry, &candidates)?;
                if excluded.is_empty() {
                    request.frontends.retain(&source)?;
                    return Ok((source, stage));
                }
                excluded
            }
            Err(diagnostics) => {
                // A speculative save can itself be ill-typed for a shadowing
                // declaration. Its signature still determines whether to keep
                // the original diagnostic or return to that local's ordinary lifetime.
                let mut header = request.clone();
                header.frontends = request.frontends.fork()?;
                let unsupported = plan
                    .project_frame_parameters(&mut header.syntax.trees)
                    .and_then(|()| header.prepare(&mut CompileTimings::default()))
                    .and_then(|source| {
                        unsupported_registration_homes(source.typed(), entry, &candidates)
                    });
                match unsupported {
                    Ok(excluded) if excluded.is_empty() => return Err(diagnostics),
                    Ok(excluded) => excluded,
                    Err(_) => candidates.iter().map(|&(_, home)| home).collect(),
                }
            }
        };
        let plan = waits::EntryPlan::new(&request.syntax.trees, entry, index, &excluded)?;
        prepare_stage(request, entry, &plan, facts)
    }

    fn request(
        self,
        preparation: &SourcePreparation,
        host: &'static str,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
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

fn prepare_stage(
    request: ResolveAndTypeRequest,
    entry: source_text::SourceSpan,
    plan: &waits::EntryPlan,
    facts: &[crate::ReturnedScalarFact],
) -> Result<(SourceTypingCheckpoint, BuildStage), Vec<Diagnostic>> {
    let prepare = |mut request: ResolveAndTypeRequest, use_frame| -> Result<_, Vec<Diagnostic>> {
        let stage = plan.project(&mut request.syntax.trees, use_frame, facts)?;
        request.syntax.partial_build_entry = stage.next_index().map(|_| entry);
        let source = request.prepare(&mut CompileTimings::default())?;
        Ok((source, stage))
    };
    let Some(wait) = plan.optional_wait(&request.syntax.trees) else {
        return prepare(request, false);
    };
    // Both projections use the ordinary resolver. Only the chosen projection
    // can supply checked declarations to the next stage.
    let mut candidate = request.clone();
    candidate.frontends = request.frontends.fork()?;
    let whole = prepare(candidate, false);
    let after_wait = |span: source_text::SourceSpan| {
        span.source_id == entry.source_id && span.span.start > wait.end
    };
    let in_build_package = |span: source_text::SourceSpan| {
        request.sources.source_is_selected(span.source_id)
            && request.sources.same_checked_instance(entry, span)
    };
    let needs_scope = match &whole {
        // A helper's position says nothing about when it runs. The prefix
        // checks every helper an early call reaches using its original scope.
        Ok((source, _)) => source.missing_name_uses().any(|(owner, site)| match owner {
            Some(owner) if owner == entry => after_wait(site),
            owner => in_build_package(owner.unwrap_or(site)),
        }),
        // Typing can fail before it records the declaration that owns a diagnostic.
        // Retry the prefix for errors in local Build sources, wherever the helper is written.
        Err(rows) => rows
            .iter()
            .all(|row| row.source_span.is_some_and(in_build_package)),
    };
    if needs_scope {
        return prepare(request, true);
    }
    let (source, stage) = whole?;
    request.frontends.retain(&source)?;
    Ok((source, stage))
}

/// A same-spelling user type keeps its original lifetime in one program.
fn unsupported_registration_homes(
    typed: &TypedTrees,
    entry: source_text::SourceSpan,
    candidates: &[(usize, usize)],
) -> Result<Vec<usize>, Vec<Diagnostic>> {
    use crate::source_state::types::TypeReferenceNode;
    let machine = typed
        .machines()
        .iter()
        .find(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(entry))
        .and_then(|machine| typed.machine_states(machine).first())
        .ok_or_else(|| vec![Diagnostic::error("selected Build entry has no typed frame")])?;
    let params = typed.state_parameters(machine);
    let types = &typed.type_reference_table;
    let mut excluded = Vec::new();
    for &(position, home) in candidates {
        let slot = params.get(position).ok_or_else(|| {
            vec![Diagnostic::error(
                "selected Build entry lost a candidate frame parameter",
            )]
        })?;
        let admitted = matches!(types.type_reference(slot.type_reference), TypeReferenceNode::Reference { referee, .. }
            if matches!(types.type_reference(*referee), TypeReferenceNode::Named { symbol, .. }
                if typed.is_exact_toolchain_build_prelude_data(*symbol, "DependencyRegistration")));
        if !admitted {
            excluded.push(home);
        }
    }
    Ok(excluded)
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
        // Only the final Build stage owes the requested whole-package check.
        // Every stage still checks the selected entry and everything it calls.
        let subjects = if let Some(prefix) = typed.roots.partial_build_entry {
            if !typed.symbols.symbols().nodes().is_valid(entry)
                || typed.symbols.symbol_source_span(entry) != Some(prefix)
            {
                return Err(SourcePreparationFailure {
                    diagnostics: vec![Diagnostic::error(
                        "Build prefix belongs to a different entry",
                    )],
                    accounting: Default::default(),
                });
            }
            &[]
        } else {
            subjects
        };
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
            &mut CompileTimings::default(),
        )?;
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
        let (product, resolved) = self.resolve()?;
        let checked = product.judge(
            resolved,
            crate::compile_product::ProductEntry::BuildBindings,
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
            accepted.source().clone(),
            roster,
            publication.optimizations().clone(),
        )?;
        supplied
            .accepted()?
            .produce_build_provider(package, declaration_path)
    }
}
