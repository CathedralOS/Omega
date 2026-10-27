//! Own the resolver/typing continuation across admitted Build execution.
//!
//! The resolver frontier and its seeded typing arenas are one owner. Consumers
//! cannot combine unrelated frontiers, complete bodies in a different order, or
//! resume generated source against a reconstructed predecessor. Build authority,
//! package custody and source acquisition remain with their respective owners.

use crate::{PsiCompilationObserver, PsiCompilationStage};
use build_time_evaluation::{BuildTimeSelectionAuthority, PreCheckEvaluation};
use diagnostics::Diagnostic;
use std::sync::Arc;

mod activation;
use activation::SourceContext;

mod authored_completion;
use authored_completion::AuthoredCompletions;

/// Parsed, source-scoped input after package admission and Build-helper selection.
#[derive(Clone)]
pub struct SourceCompilationRequest {
    pub syntax: syntax_trees::SyntaxTrees,
    pub source_items: Vec<(source::SourceId, Vec<syntax_trees::item::ItemHandle>)>,
    /// Consumer-owned sources are fresh work even when dependency coordinates match.
    pub root_sources: Vec<source::SourceId>,
    pub frontends: SourceFrontends,
    pub build_helper_identity: &'static str,
    pub sources: Arc<source::SourceMap>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}

/// Pending Product bodies paired with the exact declaration/Build typing base.
/// Cloning retains a reusable compilation input, never permission to execute Build.
#[derive(Clone)]
pub struct SourceContinuation {
    input: Arc<SourceCompletionInput>,
    completions: AuthoredCompletions,
}

/// Exact post-Build input, excluding the invocation's retention owner.
#[derive(Clone)]
struct SourceCompletionInput {
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_bodies: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    context: SourceContext,
}

/// The mutable inputs an independently admitted Build evaluator consumes.
/// Its generated declarations remain in this continuation's typing arena.
pub struct BuildSourceInput<'source> {
    pub typed: &'source mut typed_trees::TypedTrees,
    pub pre_checks: &'source mut Vec<PreCheckEvaluation>,
}

/// Authored bodies are complete; generated sources may extend this exact base.
#[derive(Clone)]
pub struct CompletedSource {
    accounting: build_time_evaluation::SourceCompletionAccounting,
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_pre_checks: Vec<PreCheckEvaluation>,
}

/// Package-admitted generated units and their combined source custody.
pub struct SourceExtensionRequest {
    pub units: Vec<syntax_trees::SyntaxTrees>,
    pub sources: Arc<source::SourceMap>,
    pub first_source: source::SourceId,
    pub selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
}

/// Complete Product typing and any deferred constant obligations, ready for checking.
pub struct PreparedSource {
    pub accounting: build_time_evaluation::SourceCompletionAccounting,
    pub typed: typed_trees::TypedTrees,
    pub pending_pre_checks: Vec<PreCheckEvaluation>,
}

impl SourceCompilationRequest {
    pub fn prepare(
        mut self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<SourceContinuation, Vec<Diagnostic>> {
        let _profile = phase_profile::guard("psi.source.prepare");
        let context = SourceContext {
            bindings: self.top_level_bindings.clone(),
            authority: self.selection_authority.clone(),
            build_helper_identity: self.build_helper_identity,
        };
        let activation = activation::select_retained_source(&self, &context, observer)?;
        if let Some(activation) = &activation {
            activation.remove_retained_syntax(&mut self.syntax, &self.source_items);
        }
        let retained_program = activation
            .as_ref()
            .map(|activation| activation.typing.typed().clone());
        let (mut retained, mut pending_pre_checks) = match activation {
            Some(activation) => (Some(activation.typing), activation.pre_checks),
            None => (None, Vec::new()),
        };
        let resolved_base = retained
            .as_mut()
            .map(|typing| typing.resolved_base_for_extension())
            .transpose()?
            .map(Arc::new);
        let evaluated = build_time_evaluation::evaluate_pre_resolution(
            build_time_evaluation::BuildTimeEvaluationRequest {
                syntax_trees: self.syntax,
                source_context: build_time_evaluation::BuildTimeSources {
                    sources: Some(Arc::clone(&self.sources)),
                    source_scoped_top_level_bindings: &self.top_level_bindings,
                    selection_authority: self.selection_authority,
                    predecessor: retained.clone(),
                },
            },
        )?;
        let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
        let (mut typing, mut pending_bodies) =
            if let (Some(typing), Some(base)) = (retained, resolved_base) {
                let extension = observer.record_result(PsiCompilationStage::Resolution, || {
                    syntax_trees_to_symbol_resolved_trees::resolve_deferred_extension(
                        syntax_trees_to_symbol_resolved_trees::ExtensionRequest {
                            base: Arc::unwrap_or_clone(base),
                            syntax,
                            sources: self.sources,
                            top_level_bindings: self.top_level_bindings,
                        },
                    )
                })?;
                observer.record_result(PsiCompilationStage::Typing, || {
                    typing.append_deferred(extension)
                })?
            } else {
                let mut frontier =
                    observer.record_result(PsiCompilationStage::Resolution, || {
                        syntax_trees_to_symbol_resolved_trees::resolve_deferred(
                            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                                syntax,
                                sources: Some(self.sources),
                                top_level_bindings: self.top_level_bindings,
                                retained_base: None,
                            },
                        )
                    })?;
                loop {
                    let selected = build_bodies(&frontier);
                    if selected.is_empty() {
                        break;
                    }
                    frontier = observer
                        .record_result(PsiCompilationStage::Resolution, || {
                            frontier.complete_machine_bodies(&selected)
                        })?
                        .into_resolved_frontier();
                }
                let typing = observer.record_result(PsiCompilationStage::Typing, || {
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
                    frontier.shared_trees(),
                ).map_err(|diagnostic| vec![diagnostic])
            })?;
                (typing, frontier)
            };
        // Build may use its admitted declaration context. Product bodies stay
        // pending: completing them here would demand unrelated constants/layouts.
        loop {
            let build_bodies = build_bodies(&pending_bodies);
            if build_bodies.is_empty() {
                break;
            }
            pending_bodies = typing.join_prepared_allocations(pending_bodies)?;
            let completed = observer.record_result(PsiCompilationStage::Resolution, || {
                pending_bodies.complete_machine_bodies(&build_bodies)
            })?;
            (typing, pending_bodies) = observer
                .record_result(PsiCompilationStage::Typing, || {
                    typing.complete_bodies(completed)
                })?;
            // Completion can append attached implementations. Those use the
            // same scope test, not a second Product declaration selection.
        }
        let pre_check = match retained_program {
            Some(previous) => pre_check.exclude_completed_work(typing.typed(), &previous)?,
            None => pre_check,
        };
        pending_pre_checks.push(pre_check);
        // Only the canonical pre-Build input is shared. A Build mutation forks
        // the whole input, including pending recipes; probe products never enter here.
        let input = Arc::new(SourceCompletionInput {
            typing,
            pending_bodies,
            pending_pre_checks,
            context,
        });
        self.frontends.retain(Arc::clone(&input))?;
        Ok(SourceContinuation {
            input,
            completions: Default::default(),
        })
    }
}

impl SourceContinuation {
    /// Complete only the old bodies this consumer actually selected, retaining
    /// their original syntax/ordinals until completion. Lookup changes first:
    /// checking references afterward cannot repair a stale ambiguity diagnostic.
    fn complete_selected(
        input: Arc<SourceCompletionInput>,
        sources: Arc<source::SourceMap>,
        bindings: Vec<symbols::SourceScopedTopLevelBinding>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<
        (
            symbol_resolved_trees_to_typed_trees::SeededTypingBase,
            Vec<PreCheckEvaluation>,
        ),
        Vec<Diagnostic>,
    > {
        let (mut typing, mut pending_bodies) = input.typing.clone().activate_body_completion(
            input.pending_bodies.clone(),
            Arc::clone(&sources),
            bindings,
        )?;
        loop {
            pending_bodies = typing.join_prepared_allocations(pending_bodies)?;
            let selected = pending_bodies
                .pending_machines()
                .filter(|symbol| {
                    pending_bodies
                        .trees()
                        .symbols
                        .symbol_provenance_source_span(*symbol)
                        .is_some_and(|span| sources.source_is_selected(span.source_id))
                })
                .collect::<Vec<_>>();
            if selected.is_empty() {
                break;
            }
            let resolved = observer.record_result(PsiCompilationStage::Resolution, || {
                pending_bodies.complete_machine_bodies(&selected)
            })?;
            (typing, pending_bodies) = observer
                .record_result(PsiCompilationStage::Typing, || {
                    typing.complete_bodies(resolved)
                })?;
        }
        // Pre-checks may depend on the receiving Build's representation choices.
        // Keep their exact work; projection and ordinary evaluation follow later.
        Ok((typing, input.pending_pre_checks.clone()))
    }

    /// Whether two admitted Build continuations have exactly the same remaining
    /// source work. A common pre-Build origin alone is insufficient: Build may
    /// have elaborated declarations or consumed different evaluation obligations.
    pub fn same_completion_input(&self, other: &Self) -> bool {
        self.input.same_completion_input(&other.input)
    }

    /// Borrow declaration descriptions for package custody; this is not executable input.
    pub fn typed(&self) -> &typed_trees::TypedTrees {
        self.input.typing.typed()
    }

    /// Locate the same authored machine in a distinct source context. This
    /// transfers selection, never a checking judgment or execution authority.
    /// Anonymous ownership may become final; already supplied owners, source
    /// bytes, and checked-instance metadata must remain unchanged.
    pub fn rejoin_machine(
        &self,
        previous: &Self,
        selected: symbols::SymbolHandle,
    ) -> Result<symbols::SymbolHandle, Vec<Diagnostic>> {
        self.rejoin_declared_machine(previous.typed(), selected)
    }

    /// Issued descriptions belong to the frozen Product declaration frontier,
    /// independently of the Build execution projection's symbol allocation.
    pub fn rejoin_product_machine(
        &self,
        previous: &Self,
        selected: symbols::SymbolHandle,
    ) -> Result<symbols::SymbolHandle, Vec<Diagnostic>> {
        self.rejoin_declared_machine(previous.typed().product_declarations(), selected)
    }

    fn rejoin_declared_machine(
        &self,
        previous: &typed_trees::TypedTrees,
        selected: symbols::SymbolHandle,
    ) -> Result<symbols::SymbolHandle, Vec<Diagnostic>> {
        let refuse = |reason| vec![Diagnostic::error(reason)];
        let span = previous
            .symbols
            .symbol_source_span(selected)
            .ok_or_else(|| refuse("selected machine has no retained declaration coordinate"))?;
        let original = previous
            .symbols
            .source_file(span)
            .ok_or_else(|| refuse("selected machine lost its original source"))?;
        let sources = self
            .typed()
            .symbols
            .source_map()
            .ok_or_else(|| refuse("selected machine has no source map in the new context"))?;
        let current_source = sources
            .rejoined_source_id(original)
            .ok_or_else(|| refuse("selected machine lost its admitted source correspondence"))?;
        let current_span = source::SourceSpan::new(current_source, span.span);
        let mut matches = self.typed().machines().iter().filter(|machine| {
            self.typed().symbols.symbol_source_span(machine.symbol) == Some(current_span)
        });
        let selected = matches
            .next()
            .ok_or_else(|| refuse("selected machine declaration is absent from the new context"))?;
        if matches.next().is_some() {
            return Err(refuse(
                "selected machine declaration is ambiguous in the new context",
            ));
        }
        Ok(selected.symbol)
    }

    pub fn build_input(&mut self) -> BuildSourceInput<'_> {
        let input = Arc::make_mut(&mut self.input);
        BuildSourceInput {
            typed: input.typing.typed_mut(),
            pre_checks: &mut input.pending_pre_checks,
        }
    }

    /// Consume the post-Build continuation and complete every authored body once.
    pub fn complete_authored(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<CompletedSource, Vec<Diagnostic>> {
        let _profile = phase_profile::guard("psi.source.complete_authored");
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
            context: _,
        } = Arc::unwrap_or_clone(owned_input);
        loop {
            // Build may append concrete declarations. Join their exact suffix
            // before body resolution allocates more symbols in the same owner.
            pending_bodies = typing.join_prepared_allocations(pending_bodies)?;
            let selected = pending_bodies.pending_machines().collect::<Vec<_>>();
            if selected.is_empty() {
                break;
            }
            let resolved = observer.record_result(PsiCompilationStage::Resolution, || {
                pending_bodies.complete_machine_bodies(&selected)
            })?;
            (typing, pending_bodies) = observer
                .record_result(PsiCompilationStage::Typing, || {
                    typing.complete_bodies(resolved)
                })?;
        }
        let mut deferred = Vec::new();
        let mut accounting = build_time_evaluation::SourceCompletionAccounting::default();
        for pre_check in pending_pre_checks {
            let recorded = pre_check.evaluate_or_defer_recorded(typing.typed_mut())?;
            accounting.append(recorded.accounting);
            if let Some(pending) = recorded.pending {
                deferred.push(pending);
            }
        }
        let source = CompletedSource {
            typing,
            pending_pre_checks: deferred,
            accounting,
        };
        completions.retain(input, &source)?;
        Ok(source)
    }
}

impl CompletedSource {
    /// Complete authored Product declarations before this Build's generated
    /// extension. Selection consumers may resolve inert requests here; the
    /// later extension retains these same arenas and cannot supply new names.
    pub fn authored_program(&self) -> &typed_trees::TypedTrees {
        self.typing.typed()
    }

    pub fn finish(self) -> PreparedSource {
        let _profile = phase_profile::guard("psi.source.finish");
        PreparedSource {
            accounting: self.accounting,
            typed: self.typing.into_typed(),
            pending_pre_checks: self.pending_pre_checks,
        }
    }

    /// Type a later generated stratum against the completed authored predecessor.
    /// Unit normalization shares that predecessor; generated siblings cannot
    /// participate in one another's generic synthesis.
    pub fn extend(
        mut self,
        extension: SourceExtensionRequest,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<PreparedSource, Vec<Diagnostic>> {
        let _profile = phase_profile::guard("psi.source.extend");
        let wire_schema_frontier = self.typing.typed().wire_schemas().len();
        let resolved_base = self.typing.resolved_base_for_extension()?;
        let (syntax, pre_checks) =
            build_time_evaluation::PreResolutionEvaluation::evaluate_extension(
                extension.units,
                extension.first_source,
                build_time_evaluation::BuildTimeSources {
                    sources: Some(Arc::clone(&extension.sources)),
                    source_scoped_top_level_bindings: &[],
                    selection_authority: Some(extension.selection_authority),
                    predecessor: Some(self.typing.clone()),
                },
            )?;
        let seeded = observer.record_result(PsiCompilationStage::Resolution, || {
            syntax_trees_to_symbol_resolved_trees::resolve_extension(
                syntax_trees_to_symbol_resolved_trees::ExtensionRequest {
                    base: resolved_base,
                    syntax,
                    sources: extension.sources,
                    top_level_bindings: Vec::new(),
                },
            )
        })?;
        let mut typed = observer
            .record_result(PsiCompilationStage::Typing, || {
                self.typing.lower_extension(seeded)
            })?
            .into_typed();
        self.pending_pre_checks
            .extend(PreCheckEvaluation::evaluate_extension_or_defer(
                pre_checks,
                wire_schema_frontier,
                &mut typed,
            )?);
        Ok(PreparedSource {
            accounting: self.accounting,
            typed,
            pending_pre_checks: self.pending_pre_checks,
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

/// Invocation-owned canonical frontend inputs. Retention supplies declarations
/// and pending bodies, never a language judgment, Build authority or free charge.
/// Stored inputs contain no reference to this container, avoiding ownership cycles.
#[derive(Clone, Default)]
pub struct SourceFrontends {
    entries: Arc<std::sync::Mutex<Vec<Arc<SourceCompletionInput>>>>,
}
impl SourceFrontends {
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

fn build_bodies(
    frontier: &syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
) -> Vec<symbols::SymbolHandle> {
    frontier
        .pending_machines()
        .filter(|symbol| {
            frontier
                .trees()
                .symbols
                .symbol_provenance_source_span(*symbol)
                .and_then(|span| frontier.trees().symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        || file.dependency_scope == source::DependencyScope::Build
                })
        })
        .collect()
}
