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
    pub sources: Arc<source::SourceMap>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    pub previous: Vec<crate::RetainedCheck>,
    /// Frontend work retained after dependency Build, before Product judgment.
    pub pending_sources: Vec<SourceContinuation>,
    pub root_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Opaque identity of the already selected Build-helper context. Psi does
    /// not interpret targets; it must not mix differently selected predecessors.
    pub build_helper_identity: &'static str,
}

/// Pending Product bodies paired with the exact declaration/Build typing base.
/// Cloning retains a reusable compilation input, never permission to execute Build.
#[derive(Clone)]
pub struct SourceContinuation {
    input: SourceCompletionInput,
    completions: AuthoredCompletions,
}

/// Exact post-Build input, excluding the invocation's retention owner.
#[derive(Clone)]
struct SourceCompletionInput {
    inherited_accounting: build_time_evaluation::SourceCompletionAccounting,
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_bodies: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    context: SourceContext,
}

impl std::fmt::Debug for SourceContinuation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceContinuation")
            .field("machine_count", &self.input.typing.typed().machines().len())
            .field(
                "pending_body_count",
                &self.input.pending_bodies.pending_machines().count(),
            )
            .field("pre_check_count", &self.input.pending_pre_checks.len())
            .finish()
    }
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
    context: SourceContext,
}

/// Package-admitted generated units and their combined source custody.
pub struct SourceExtensionRequest {
    pub units: Vec<syntax_trees::SyntaxTrees>,
    pub sources: Arc<source::SourceMap>,
    pub first_source: source::SourceId,
    pub selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
}

/// Complete Product typing and its exact resolver/typing origin, ready for checking.
/// A checked dependency needs this owner as well as its language judgment: bare
/// typed trees cannot establish which authored source frontier may be extended.
#[derive(Clone)]
pub struct PreparedSource {
    accounting: build_time_evaluation::SourceCompletionAccounting,
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    context: SourceContext,
}

impl PreparedSource {
    pub fn accounting(&self) -> &build_time_evaluation::SourceCompletionAccounting {
        &self.accounting
    }

    pub(crate) fn typed(&self) -> &typed_trees::TypedTrees {
        self.typing.typed()
    }

    /// Transfer the coupled origin to checking. Deferred target recipes remain
    /// with that origin; retaining them grants no execution authority.
    pub fn into_check_input(self: Arc<Self>) -> crate::SourceCheckInput {
        crate::SourceCheckInput::from_source(self)
    }

    pub(crate) fn into_parts(self) -> (typed_trees::TypedTrees, Vec<PreCheckEvaluation>) {
        (self.typing.into_typed(), self.pending_pre_checks)
    }
}

impl std::fmt::Debug for PreparedSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedSource")
            .field("typing", &self.typing)
            .field("deferred_recipe_count", &self.pending_pre_checks.len())
            .finish()
    }
}

impl SourceCompilationRequest {
    pub fn prepare(
        mut self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<SourceContinuation, Vec<Diagnostic>> {
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
        let (mut retained, mut pending_pre_checks, inherited_accounting) = match activation {
            Some(activation) => (
                Some(activation.typing),
                activation.pre_checks,
                activation.accounting,
            ),
            None => (None, Vec::new(), Default::default()),
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
                let frontier = observer.record_result(PsiCompilationStage::Resolution, || {
                    syntax_trees_to_symbol_resolved_trees::resolve_deferred(
                        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                            syntax,
                            sources: Some(self.sources),
                            top_level_bindings: self.top_level_bindings,
                            retained_base: None,
                        },
                    )
                })?;
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
            let build_bodies = pending_bodies
                .pending_machines()
                .filter(|symbol| {
                    pending_bodies
                        .trees()
                        .symbols
                        .symbol_provenance_source_span(*symbol)
                        .and_then(|span| pending_bodies.trees().symbols.source_file(span))
                        .is_some_and(|file| {
                            file.origin == source::SourceOrigin::Toolchain
                                || file.dependency_scope == source::DependencyScope::Build
                        })
                })
                .collect::<Vec<_>>();
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
        Ok(SourceContinuation {
            input: SourceCompletionInput {
                inherited_accounting,
                typing,
                pending_bodies,
                pending_pre_checks,
                context,
            },
            completions: Default::default(),
        })
    }
}

impl SourceContinuation {
    /// Complete only the old bodies this consumer actually selected, retaining
    /// their original syntax/ordinals until completion. Lookup changes first:
    /// checking references afterward cannot repair a stale ambiguity diagnostic.
    fn complete_selected(
        self,
        sources: Arc<source::SourceMap>,
        bindings: Vec<symbols::SourceScopedTopLevelBinding>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<
        (
            symbol_resolved_trees_to_typed_trees::SeededTypingBase,
            Vec<PreCheckEvaluation>,
            build_time_evaluation::SourceCompletionAccounting,
        ),
        Vec<Diagnostic>,
    > {
        let (mut typing, mut pending_bodies) = self.input.typing.activate_body_completion(
            self.input.pending_bodies,
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
        Ok((
            typing,
            self.input.pending_pre_checks,
            self.input.inherited_accounting,
        ))
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
        let refuse = |reason| vec![Diagnostic::error(reason)];
        let span = previous
            .typed()
            .symbols
            .symbol_source_span(selected)
            .ok_or_else(|| refuse("selected machine has no retained declaration coordinate"))?;
        let original = previous
            .typed()
            .symbols
            .source_file(span)
            .ok_or_else(|| refuse("selected machine lost its original source"))?;
        let current = self
            .typed()
            .symbols
            .source_file(span)
            .ok_or_else(|| refuse("selected machine has no source in the new context"))?;
        if original.source != current.source
            || original.source_id != current.source_id
            || original.path != current.path
            || original.package_root != current.package_root
            || original.origin != current.origin
            || original.resolution_stratum != current.resolution_stratum
            || original.dependency_scope != current.dependency_scope
            || original
                .package_identity
                .is_some_and(|owner| Some(owner) != current.package_identity)
        {
            return Err(refuse(
                "selected machine source changed between compilation contexts",
            ));
        }
        let mut matches = self.typed().machines().iter().filter(|machine| {
            self.typed().symbols.symbol_source_span(machine.symbol) == Some(span)
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
        BuildSourceInput {
            typed: self.input.typing.typed_mut(),
            pre_checks: &mut self.input.pending_pre_checks,
        }
    }

    /// Consume the post-Build continuation and complete every authored body once.
    pub fn complete_authored(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<CompletedSource, Vec<Diagnostic>> {
        if let Some(source) = self.completions.find(&self.input)? {
            return Ok(source);
        }
        let input = self.input.clone();
        let Self {
            input:
                SourceCompletionInput {
                    inherited_accounting,
                    mut typing,
                    mut pending_bodies,
                    pending_pre_checks,
                    context,
                },
            completions,
        } = self;
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
        let mut accounting = inherited_accounting;
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
            context,
            accounting,
        };
        completions.retain(input, &source)?;
        Ok(source)
    }
}

impl CompletedSource {
    pub fn finish(self) -> Arc<PreparedSource> {
        Arc::new(PreparedSource {
            accounting: self.accounting,
            typing: self.typing,
            pending_pre_checks: self.pending_pre_checks,
            context: self.context,
        })
    }

    /// Type a later generated stratum against the completed authored predecessor.
    /// Unit normalization shares that predecessor; generated siblings cannot
    /// participate in one another's generic synthesis.
    pub fn extend(
        mut self,
        extension: SourceExtensionRequest,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Arc<PreparedSource>, Vec<Diagnostic>> {
        let wire_schema_frontier = self.typing.typed().wire_schemas().len();
        let resolved_base = Arc::new(self.typing.resolved_base_for_extension()?);
        let (syntax, pre_checks) =
            build_time_evaluation::PreResolutionEvaluation::evaluate_extension(
                extension.units,
                extension.first_source,
                build_time_evaluation::BuildTimeSources {
                    sources: Some(Arc::clone(&extension.sources)),
                    source_scoped_top_level_bindings: &[],
                    selection_authority: Some(Arc::clone(&extension.selection_authority)),
                    predecessor: Some(self.typing.clone()),
                },
            )?;
        let seeded = observer.record_result(PsiCompilationStage::Resolution, || {
            syntax_trees_to_symbol_resolved_trees::resolve_extension(
                syntax_trees_to_symbol_resolved_trees::ExtensionRequest {
                    base: Arc::unwrap_or_clone(resolved_base),
                    syntax,
                    sources: extension.sources,
                    top_level_bindings: Vec::new(),
                },
            )
        })?;
        self.typing = observer.record_result(PsiCompilationStage::Typing, || {
            self.typing.lower_extension(seeded)
        })?;
        let (pending, accounting) = PreCheckEvaluation::evaluate_extension_or_defer(
            pre_checks,
            wire_schema_frontier,
            self.typing.typed_mut(),
        )?;
        self.pending_pre_checks.extend(pending);
        self.accounting.append(accounting);
        self.context.authority = Some(extension.selection_authority);
        Ok(Arc::new(PreparedSource {
            accounting: self.accounting,
            typing: self.typing,
            pending_pre_checks: self.pending_pre_checks,
            context: self.context,
        }))
    }
}

impl SourceCompletionInput {
    fn same_completion_input(&self, other: &Self) -> bool {
        self.typing == other.typing
            && self
                .inherited_accounting
                .same_receipts(&other.inherited_accounting)
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
