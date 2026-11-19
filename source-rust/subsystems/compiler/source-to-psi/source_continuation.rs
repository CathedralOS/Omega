/*
    Resolver and typing continuation across admitted Build execution, generated
    source extension, and post-join Product policy admission.

    We pair the resolver frontier with its seeded typing base inside
    `SourceContinuation`, complete authored bodies into `CompletedSource`,
    extend completed sources with admitted generated units, and admit Product
    provider requests, root grants, declarations, and behavior exclusions.
*/

use crate::compile_time::{BuildTimeSelectionAuthority, PreCheckEvaluation};
use crate::{PsiCompilationObserver, PsiCompilationStage};
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::product_declarations;

mod activation;
use activation::SourceContext;

mod authored_completion;
use authored_completion::AuthoredCompletions;
mod product_policy;
mod provider_requests;
pub use product_policy::{AdmittedProductPolicy, ProductPolicyRequest};
mod storage;

#[derive(Clone)]
pub struct ResolveAndTypeRequest {
    pub syntax: crate::SourceSyntax,
    pub source_items: Vec<(source_text::SourceId, crate::SourceItems)>,
    pub root_sources: Vec<source_text::SourceId>,
    pub frontends: SourceFrontends,
    pub build_helper_identity: &'static str,
    pub build_sources: std::collections::HashSet<source_text::SourceId>,
    pub build_stage: Option<(source_text::SourceId, usize)>,
    pub sources: Arc<crate::source_inputs::SourceMap>,
    pub top_level_bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}

#[derive(Clone, record_storage::Record)]
pub struct SourceContinuation {
    input: Arc<SourceCompletionInput>,
    completions: AuthoredCompletions,
}

#[derive(Clone, record_storage::Record)]
struct SourceCompletionInput {
    typing: crate::typing::SeededTypingBase,
    pending_bodies: crate::resolution::DeferredResolvedFrontier,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    context: SourceContext,
}

#[derive(Clone, record_storage::Record)]
pub struct CompletedSource {
    declaration_syntax: Arc<syntax_trees::SyntaxTrees>,
    accounting: crate::compile_time::SourceCompletionAccounting,
    typing: crate::typing::SeededTypingBase,
    pending_pre_checks: Vec<PreCheckEvaluation>,
}

#[derive(Clone)]
pub struct SourceExtensionRequest {
    pub units: source_admission::LoadedSources,
    pub sources: Arc<crate::source_inputs::SourceMap>,
    pub first_source: source_text::SourceId,
    pub selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
}

pub struct PreparedSource {
    pub accounting: crate::compile_time::SourceCompletionAccounting,
    pub typed: crate::source_state::TypedTrees,
}

impl PreparedSource {
    pub fn admit_root_bindings(
        &self,
        executed: &[crate::declaration_selection::ExecutedRootBinding],
    ) -> Result<Vec<crate::RootBinding>, Vec<Diagnostic>> {
        product_declarations::collect_root_bindings(&self.typed, executed)
    }
}

impl ResolveAndTypeRequest {
    pub fn prepare(
        mut self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<
        (
            SourceContinuation,
            source_declarations::build_stage::BuildStage,
        ),
        Vec<Diagnostic>,
    > {
        crate::target_helpers::select_build_helpers(
            &mut self.syntax.trees,
            self.build_helper_identity,
            &self.build_sources,
        );
        // Only the requested Build stage is resolved and checked. A Product
        // request keeps the entry whole because it does not execute it.
        let build_stage = match self.build_stage {
            Some((source, stage)) => {
                crate::build_stages::select_build_stage(&mut self.syntax.trees, source, stage)?
            }
            None => source_declarations::build_stage::BuildStage::WHOLE,
        };
        let context = SourceContext {
            declaration_syntax: Arc::new(self.syntax.trees.clone()),
            bindings: self.top_level_bindings.clone(),
            authority: self.selection_authority.clone(),
            build_helper_identity: self.build_helper_identity.into(),
            recorded_authority: None,
        };
        let activation = activation::select_retained_source(&self, &context, observer)?;
        if let Some(activation) = &activation {
            activation.remove_retained_syntax(&mut self.syntax.trees, &self.source_items);
        }
        // Check the predecessor while its owner can still be borrowed. Report
        // any refusal after checking the successor to preserve diagnostic order.
        let retained_requests = activation.as_ref().map(|activation| {
            PreCheckEvaluation::require_recorded_requests(activation.typing.typed())
        });
        let retained_declaration_syntax = activation
            .as_ref()
            .map(|activation| Arc::clone(&activation.declaration_syntax));
        let (mut retained, mut pending_pre_checks) = match activation {
            Some(activation) => (Some(activation.typing), activation.pre_checks),
            None => (None, Vec::new()),
        };
        let resolved_base = retained
            .as_mut()
            .map(|typing| typing.resolved_base_for_extension())
            .transpose()?;
        let syntax = self.syntax.trees;
        let pre_check = PreCheckEvaluation::new(self.selection_authority);
        let (mut typing, mut pending_bodies) =
            if let (Some(typing), Some(base), Some(base_declarations)) =
                (retained, resolved_base, retained_declaration_syntax)
            {
                let extension = observer.record_result(PsiCompilationStage::Resolution, || {
                    crate::resolution::resolve_deferred_extension(
                        crate::resolution::ExtensionRequest {
                            base,
                            base_declarations,
                            syntax,
                            sources: self.sources,
                            top_level_bindings: self.top_level_bindings,
                        },
                    )
                })?;
                observer.record_result(PsiCompilationStage::Typing, || {
                    let (source, frontier) = extension.rebase_for_typed_continuation(
                        typing.typed().authored_declaration_selections(),
                    )?;
                    Ok::<_, Vec<Diagnostic>>((typing.append_resolved(source)?, frontier))
                })?
            } else {
                let mut frontier = {
                    observer.record_result(PsiCompilationStage::Resolution, || {
                        crate::resolution::resolve_deferred(crate::resolution::ResolutionRequest {
                            syntax,
                            sources: Some(self.sources),
                            top_level_bindings: self.top_level_bindings,
                            retained_base: None,
                        })
                    })?
                };
                loop {
                    let selected = frontier.pending_machines_in(
                        crate::resolution::DeferredBodyScope::BuildAndToolchain,
                    )?;
                    if selected.is_empty() {
                        break;
                    }
                    frontier = observer
                        .record_result(PsiCompilationStage::Resolution, || {
                            frontier.complete_machine_bodies(&selected)
                        })?
                        .into_resolved_frontier();
                }
                let typing = {
                    observer.record_result(PsiCompilationStage::Typing, || {
                        crate::typing::lower_symbol_resolved_trees_to_seeded_base(
                            frontier.shared_trees(),
                        )
                        .map_err(|diagnostic| vec![diagnostic])
                    })?
                };
                (typing, frontier)
            };
        loop {
            let build_bodies = pending_bodies
                .pending_machines_in(crate::resolution::DeferredBodyScope::BuildAndToolchain)?;
            if build_bodies.is_empty() {
                break;
            }
            pending_bodies =
                pending_bodies.with_prepared_allocations(typing.prepare_allocations()?)?;
            let completed = observer.record_result(PsiCompilationStage::Resolution, || {
                pending_bodies.complete_machine_bodies(&build_bodies)
            })?;
            (typing, pending_bodies) = observer
                .record_result(PsiCompilationStage::Typing, || {
                    type_completed_bodies(typing, completed)
                })?;
        }
        PreCheckEvaluation::defer_declared_requests(typing.typed_mut())?;
        if let Some(retained_requests) = retained_requests {
            PreCheckEvaluation::require_recorded_requests(typing.typed())?;
            retained_requests?;
        }
        pending_pre_checks.push(pre_check);
        let input = Arc::new(SourceCompletionInput {
            typing,
            pending_bodies,
            pending_pre_checks,
            context,
        });
        self.frontends.retain(Arc::clone(&input))?;
        Ok((
            SourceContinuation {
                input,
                completions: Default::default(),
            },
            build_stage,
        ))
    }
}

impl SourceContinuation {
    fn complete_selected(
        input: Arc<SourceCompletionInput>,
        sources: Arc<crate::source_inputs::SourceMap>,
        bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<(crate::typing::SeededTypingBase, Vec<PreCheckEvaluation>), Vec<Diagnostic>> {
        let mut typing = input.typing.clone();
        let pending_bodies = input
            .pending_bodies
            .clone()
            .with_prepared_allocations(typing.prepare_allocations()?)?;
        let (pending_bodies, activation) = pending_bodies.activate_sources(sources, bindings)?;
        typing = typing.activate_body_completion(activation)?;
        let (typing, _) = complete_pending_bodies(
            typing,
            pending_bodies,
            crate::resolution::DeferredBodyScope::SelectedFiles,
            observer,
        )?;
        Ok((typing, input.pending_pre_checks.clone()))
    }

    /// Rejoin a Build query's authored locations to this admitted declaration
    /// frontier. Source correspondence is resolved by the owner of both states.
    pub fn rejoin_product_query_origin(
        &self,
        authored: &Self,
        occurrence: source_text::SourceSpan,
        selecting: Option<source_text::SourceSpan>,
    ) -> Result<(source_text::SourceSpan, Option<source_text::SourceSpan>), String> {
        let author = authored
            .typed()
            .symbols
            .source_file(occurrence)
            .ok_or("Product query lost its authored source occurrence")?;
        let source_map = self
            .typed()
            .symbols
            .source_map()
            .ok_or("Product query has no source custody")?;
        let selected = source_map
            .rejoined_source_id(author)
            .ok_or("Product query author lost its admitted source correspondence")?;
        let occurrence = source_text::SourceSpan::new(selected, occurrence.span);
        let selecting = selecting
            .map(|span| {
                let author = authored
                    .typed()
                    .symbols
                    .source_file(span)
                    .ok_or("declaration query lost its selecting source")?;
                let selected = source_map
                    .rejoined_source_id(author)
                    .ok_or("declaration query lost its selecting source correspondence")?;
                Ok::<_, String>(source_text::SourceSpan::new(selected, span.span))
            })
            .transpose()?;
        Ok((occurrence, selecting))
    }

    pub fn describe_parameter(
        &self,
        occurrence: source_text::SourceSpan,
        selecting: source_text::SourceSpan,
        path: &[u8],
    ) -> Result<semantic_vocabulary::ParameterDescription, String> {
        product_declarations::describe_parameter(self.typed(), occurrence, selecting, path)
    }

    pub fn describe_field(
        &self,
        occurrence: source_text::SourceSpan,
        selecting: source_text::SourceSpan,
        path: &[u8],
    ) -> Result<semantic_vocabulary::FieldInfoDescription, String> {
        product_declarations::describe_field(self.typed(), occurrence, selecting, path)
    }

    pub fn describe_schema(
        &self,
        occurrence: source_text::SourceSpan,
        path: &[u8],
    ) -> Result<crate::declaration_selection::DescribedProductSchema, String> {
        product_declarations::describe_schema(self.typed(), occurrence, path)
    }

    pub fn describe_provider(
        &self,
        occurrence: source_text::SourceSpan,
        path: &[u8],
    ) -> Result<crate::declaration_selection::DescribedProductProvider, String> {
        product_declarations::describe_provider(self.typed(), occurrence, path)
    }

    pub fn describe_entry(
        &self,
        occurrence: source_text::SourceSpan,
        path: &[u8],
        slot: &[u8],
        slots: &dyn semantic_vocabulary::ProgramEntrySourceCatalog,
    ) -> Result<crate::declaration_selection::DescribedProductEntry, String> {
        product_declarations::describe_entry(self.typed(), occurrence, path, slot, slots)
    }

    pub fn same_completion_input(&self, other: &Self) -> bool {
        self.input.same_completion_input(&other.input)
    }

    pub fn typed(&self) -> &crate::source_state::TypedTrees {
        self.input.typing.typed()
    }

    pub fn rejoin_machine(
        &self,
        previous: &Self,
        selected: symbols::SymbolHandle,
    ) -> Result<symbols::SymbolHandle, Vec<Diagnostic>> {
        let refuse = |reason| vec![Diagnostic::error(reason)];
        let declaration =
            crate::source_symbols::retain_declaration(&previous.typed().symbols, selected)
                .map_err(refuse)?;
        crate::source_symbols::rejoin_declaration(
            &declaration,
            &self.typed().symbols,
            self.typed().machines().iter().map(|machine| machine.symbol),
        )
        .map_err(refuse)
    }

    pub fn materialize_authored_bodies(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Self, Vec<Diagnostic>> {
        let Self { input, completions } = self;
        let SourceCompletionInput {
            typing,
            pending_bodies,
            pending_pre_checks,
            context,
        } = Arc::unwrap_or_clone(input);
        let (typing, pending_bodies) = complete_pending_bodies(
            typing,
            pending_bodies,
            crate::resolution::DeferredBodyScope::All,
            observer,
        )?;
        Ok(Self {
            input: Arc::new(SourceCompletionInput {
                typing,
                pending_bodies,
                pending_pre_checks,
                context,
            }),
            completions,
        })
    }

    pub fn complete_authored(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<CompletedSource, Vec<Diagnostic>> {
        if let Some(source) = self.completions.find(&self.input)? {
            return Ok(source);
        }
        let input = self.input.clone();
        let Self {
            input: owned_input,
            completions,
        } = self;
        let SourceCompletionInput {
            mut typing,
            mut pending_bodies,
            pending_pre_checks,
            context,
        } = Arc::unwrap_or_clone(owned_input);
        (typing, pending_bodies) = complete_pending_bodies(
            typing,
            pending_bodies,
            crate::resolution::DeferredBodyScope::All,
            observer,
        )?;
        drop(pending_bodies);
        PreCheckEvaluation::record_continuations(pending_pre_checks, typing.typed_mut())?;
        let source = CompletedSource {
            declaration_syntax: context.declaration_syntax,
            typing,
            pending_pre_checks: Vec::new(),
            accounting: Default::default(),
        };
        completions.retain(input, &source)?;
        Ok(source)
    }
}

// Resolution retains pending syntax; typing receives only the completed trees.
// The driver joins their exact checkpoints before either stage resumes.
fn type_completed_bodies(
    typing: crate::typing::SeededTypingBase,
    completed: crate::resolution::ResolvedBodyCompletion,
) -> Result<
    (
        crate::typing::SeededTypingBase,
        crate::resolution::DeferredResolvedFrontier,
    ),
    Vec<Diagnostic>,
> {
    let (source, frontier) = completed.rebase_authored_selections_for_typed_completion(
        typing.typed().authored_declaration_selections(),
    ).map_err(|(_, error)| vec![Diagnostic::error(format!(
        "authored-body completion could not join its retained typed selection ledger: {error:?}"
    ))])?;
    let mut typing = typing.complete_bodies(source)?;
    // Completing a body can introduce evaluation work. Record it on the
    // declaration before retained source is narrowed or activated, so unused
    // helpers stay deferred and demanding one still raises its refusal.
    PreCheckEvaluation::defer_declared_requests(typing.typed_mut())?;
    Ok((typing, frontier))
}

fn complete_pending_bodies(
    mut typing: crate::typing::SeededTypingBase,
    mut pending_bodies: crate::resolution::DeferredResolvedFrontier,
    scope: crate::resolution::DeferredBodyScope,
    observer: &mut impl PsiCompilationObserver,
) -> Result<
    (
        crate::typing::SeededTypingBase,
        crate::resolution::DeferredResolvedFrontier,
    ),
    Vec<Diagnostic>,
> {
    loop {
        pending_bodies = pending_bodies.with_prepared_allocations(typing.prepare_allocations()?)?;
        let selected = pending_bodies.pending_machines_in(scope)?;
        if selected.is_empty() {
            break;
        }
        let resolved = {
            observer.record_result(PsiCompilationStage::Resolution, || {
                pending_bodies.complete_machine_bodies(&selected)
            })?
        };
        (typing, pending_bodies) = {
            observer.record_result(PsiCompilationStage::Typing, || {
                type_completed_bodies(typing, resolved)
            })?
        };
    }
    Ok((typing, pending_bodies))
}

impl CompletedSource {
    pub fn authored_program(&self) -> &crate::source_state::TypedTrees {
        self.typing.typed()
    }

    pub fn finish(self) -> PreparedSource {
        PreparedSource {
            accounting: self.accounting,
            typed: self.typing.into_typed(),
        }
    }

    pub fn extend(
        mut self,
        extension: SourceExtensionRequest,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<PreparedSource, Vec<Diagnostic>> {
        // Parse admitted generated bytes before touching the retained typed
        // frontier, preserving the extension's original error precedence.
        let units = crate::source_parsing::SourceInput(extension.units).into_units(observer)?;
        let wire_schema_frontier = self.typing.typed().wire_schemas().len();
        let resolved_base = self.typing.resolved_base_for_extension()?;
        let pre_checks =
            vec![PreCheckEvaluation::new(Some(extension.selection_authority)); units.len()];
        let mut units = units.into_iter();
        let mut syntax = units
            .next()
            .unwrap_or_else(|| syntax_trees::SyntaxTrees::new(extension.first_source));
        for unit in units {
            syntax.adopt_page(unit);
        }
        let generation = u32::try_from(syntax.page_count())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| vec![Diagnostic::error("syntax page identity overflow")])?;
        syntax.begin_page(generation);
        let seeded = observer.record_result(PsiCompilationStage::Resolution, || {
            crate::resolution::resolve_extension(crate::resolution::ExtensionRequest {
                base: resolved_base,
                base_declarations: Arc::clone(&self.declaration_syntax),
                syntax,
                sources: extension.sources,
                top_level_bindings: Vec::new(),
            })
        })?;
        let mut typed = observer
            .record_result(PsiCompilationStage::Typing, || {
                self.typing.lower_extension(seeded)
            })?
            .into_typed();
        PreCheckEvaluation::record_extension(pre_checks, wire_schema_frontier, &mut typed)?;
        Ok(PreparedSource {
            accounting: self.accounting,
            typed,
        })
    }
}

impl SourceCompletionInput {
    fn same_completion_input(&self, other: &Self) -> bool {
        self.typing == other.typing
            && self.context.same_input(&other.context)
            && self.pending_bodies == other.pending_bodies
            && self.pending_pre_checks.len() == other.pending_pre_checks.len()
            && self
                .pending_pre_checks
                .iter()
                .zip(&other.pending_pre_checks)
                .all(|(left, right)| left.same_pending_work(right))
    }
}

/// The admitted files and reusable frontend results for one invocation.
#[derive(Clone, Default)]
pub struct SourcePreparation {
    catalogue: Arc<std::sync::Mutex<crate::source_inputs::SourceCatalogue>>,
    pub frontends: SourceFrontends,
}

impl SourcePreparation {
    pub fn catalogue(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, crate::source_inputs::SourceCatalogue>, Vec<Diagnostic>> {
        self.catalogue
            .lock()
            .map_err(|_| vec![Diagnostic::error("source catalogue owner is poisoned")])
    }
}

#[derive(Clone, Default)]
pub struct SourceFrontends {
    entries: Arc<std::sync::Mutex<Vec<Arc<SourceCompletionInput>>>>,
}
impl SourceFrontends {
    pub fn retained_count(&self) -> Result<usize, Vec<Diagnostic>> {
        self.snapshots().map(|entries| entries.len())
    }

    pub fn fork(&self) -> Result<Self, Vec<Diagnostic>> {
        Ok(Self {
            entries: Arc::new(std::sync::Mutex::new(self.snapshots()?)),
        })
    }

    pub fn replace_with(&self, other: &Self) -> Result<(), Vec<Diagnostic>> {
        let entries = other.snapshots()?;
        *self.entries.lock().map_err(|_| {
            vec![Diagnostic::error(
                "source frontend retention owner is poisoned",
            )]
        })? = entries;
        Ok(())
    }
    fn snapshots(&self) -> Result<Vec<Arc<SourceCompletionInput>>, Vec<Diagnostic>> {
        self.entries
            .lock()
            .map(|entries| entries.clone())
            .map_err(|_| {
                vec![Diagnostic::error(
                    "source frontend retention owner is poisoned",
                )]
            })
    }
    fn retain(&self, input: Arc<SourceCompletionInput>) -> Result<(), Vec<Diagnostic>> {
        self.entries
            .lock()
            .map_err(|_| {
                vec![Diagnostic::error(
                    "source frontend retention owner is poisoned",
                )]
            })?
            .push(input);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
