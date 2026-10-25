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

/// Parsed, source-scoped input after package admission and Build-helper selection.
pub struct SourceCompilationRequest {
    pub syntax: syntax_trees::SyntaxTrees,
    pub source_items: Vec<(source::SourceId, Vec<syntax_trees::item::ItemHandle>)>,
    pub sources: Arc<source::SourceMap>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    pub previous: Vec<crate::RetainedCheck>,
    pub root_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Opaque identity of the already selected Build-helper context. Psi does
    /// not interpret targets; it must not mix differently selected predecessors.
    pub build_helper_identity: &'static str,
}

/// Pending Product bodies paired with the exact declaration/Build typing base.
/// Cloning retains a reusable compilation input, never permission to execute Build.
#[derive(Clone)]
pub struct SourceContinuation {
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
pub struct CompletedSource {
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
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    context: SourceContext,
}

impl PreparedSource {
    pub(crate) fn typed(&self) -> &typed_trees::TypedTrees {
        self.typing.typed()
    }

    pub(crate) fn has_deferred_recipes(&self) -> bool {
        !self.pending_pre_checks.is_empty()
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
        let activation = activation::select_retained_source(&self, &context)?;
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
            .transpose()?;
        let evaluated = build_time_evaluation::evaluate_pre_resolution(
            build_time_evaluation::BuildTimeEvaluationRequest {
                syntax_trees: self.syntax,
                source_context: Some(build_time_evaluation::BuildTimeSourceContext {
                    sources: Arc::clone(&self.sources),
                    source_scoped_top_level_bindings: &self.top_level_bindings,
                    selection_authority: self.selection_authority,
                    retained_base: resolved_base.as_ref(),
                }),
            },
        )?;
        let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
        let (mut typing, mut pending_bodies) =
            if let (Some(typing), Some(base)) = (retained, resolved_base) {
                let extension = observer.record_result(PsiCompilationStage::Resolution, || {
                    syntax_trees_to_symbol_resolved_trees::resolve_deferred_extension(
                        syntax_trees_to_symbol_resolved_trees::ExtensionRequest {
                            base,
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
            typing,
            pending_bodies,
            pending_pre_checks,
            context,
        })
    }
}

impl SourceContinuation {
    /// Whether two admitted Build continuations have exactly the same remaining
    /// source work. A common pre-Build origin alone is insufficient: Build may
    /// have elaborated declarations or consumed different evaluation obligations.
    pub fn same_completion_input(&self, other: &Self) -> bool {
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

    /// Borrow declaration descriptions for package custody; this is not executable input.
    pub fn typed(&self) -> &typed_trees::TypedTrees {
        self.typing.typed()
    }

    pub fn build_input(&mut self) -> BuildSourceInput<'_> {
        BuildSourceInput {
            typed: self.typing.typed_mut(),
            pre_checks: &mut self.pending_pre_checks,
        }
    }

    /// Consume the post-Build continuation and complete every authored body once.
    pub fn complete_authored(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<CompletedSource, Vec<Diagnostic>> {
        let Self {
            mut typing,
            mut pending_bodies,
            pending_pre_checks,
            context,
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
        for pre_check in pending_pre_checks {
            if let Some(pending) = pre_check.evaluate_or_defer(typing.typed_mut())? {
                deferred.push(pending);
            }
        }
        Ok(CompletedSource {
            typing,
            pending_pre_checks: deferred,
            context,
        })
    }
}

impl CompletedSource {
    pub fn finish(self) -> Arc<PreparedSource> {
        Arc::new(PreparedSource {
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
        let resolved_base = self.typing.resolved_base_for_extension()?;
        let (syntax, pre_checks) =
            build_time_evaluation::PreResolutionEvaluation::evaluate_extension(
                extension.units,
                extension.first_source,
                build_time_evaluation::BuildTimeSourceContext {
                    sources: Arc::clone(&extension.sources),
                    source_scoped_top_level_bindings: &[],
                    selection_authority: Some(Arc::clone(&extension.selection_authority)),
                    retained_base: Some(&resolved_base),
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
        self.typing = observer.record_result(PsiCompilationStage::Typing, || {
            self.typing.lower_extension(seeded)
        })?;
        self.pending_pre_checks
            .extend(PreCheckEvaluation::evaluate_extension_or_defer(
                pre_checks,
                wire_schema_frontier,
                self.typing.typed_mut(),
            )?);
        self.context.authority = Some(extension.selection_authority);
        Ok(Arc::new(PreparedSource {
            typing: self.typing,
            pending_pre_checks: self.pending_pre_checks,
            context: self.context,
        }))
    }
}
