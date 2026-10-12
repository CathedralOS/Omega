//! Own the resolver/typing continuation across admitted Build execution.
//!
//! The resolver frontier and its seeded typing arenas are one owner. Consumers
//! cannot combine unrelated frontiers, complete bodies in a different order, or
//! resume generated source against a reconstructed predecessor. Build authority,
//! package custody and source acquisition remain with their respective owners.
//! Successful early constant executions leave this owner beside pending work:
//! a folded literal cannot reconstruct the checked closure which produced it.
//! Build projection observations remain with their authentic Build source
//! judgment; Product observations never relabel that earlier checked owner.
//! Other value-only routes remain outside the observed roster.

use crate::{PsiCompilationObserver, PsiCompilationStage};
use build_time_evaluation::{BuildTimeSelectionAuthority, PreCheckEvaluation};
use diagnostics::Diagnostic;
use std::sync::Arc;

/// Parsed, source-scoped input after package admission and Build-helper selection.
pub struct SourceCompilationRequest {
    pub syntax: syntax_trees::SyntaxTrees,
    pub sources: Arc<source::SourceMap>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}

/// Pending Product bodies paired with the exact declaration/Build typing base.
/// Cloning retains a reusable compilation input, never permission to execute Build.
#[derive(Clone)]
pub struct SourceContinuation {
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_bodies: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    build_constant_inputs: Vec<build_time_evaluation::ExecutedCheckedConstantInput>,
}

/// The mutable inputs an independently admitted Build evaluator consumes.
/// Its generated declarations remain in this continuation's typing arena.
pub struct BuildSourceInput<'source> {
    typed: &'source mut typed_trees::TypedTrees,
    pre_checks: &'source mut Vec<PreCheckEvaluation>,
    constant_inputs: &'source mut Vec<build_time_evaluation::ExecutedCheckedConstantInput>,
    identity: BuildSourceInputIdentity,
}

/// Association of one actual continuation borrow, not a cache namespace.
/// Only SourceContinuation produces it alongside its real mutable owner.
#[derive(Clone)]
pub struct BuildSourceInputIdentity(Arc<BuildSourceInputOwner>);
struct BuildSourceInputOwner {
    _private: u8,
}
impl PartialEq for BuildSourceInputIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for BuildSourceInputIdentity {}
impl std::fmt::Debug for BuildSourceInputIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BuildSourceInputIdentity(..)")
    }
}

/// Produced from the actual continuation input after its ordinary constant
/// settlement. There is no separate program/identity tuple constructor.
pub struct BuildSourceProjection {
    pub(crate) projection: validation::execution_projection::BuildProgramProjection,
    pub(crate) identity: BuildSourceInputIdentity,
    pub(crate) constant_inputs: Arc<[build_time_evaluation::ExecutedCheckedConstantInput]>,
}
impl BuildSourceProjection {
    pub fn program(&self) -> &typed_trees::TypedTrees {
        self.projection.program()
    }
}
impl BuildSourceInput<'_> {
    pub fn typed(&self) -> &typed_trees::TypedTrees {
        self.typed
    }
    pub fn identity(&self) -> &BuildSourceInputIdentity {
        &self.identity
    }
    /// Existing trusted Build preparation mutates this continuation in place.
    /// This capability does not attest immutable pre-settlement body bytes;
    /// only the final ordinary check publishes the immutable checked owner.
    pub fn parts_mut(
        &mut self,
    ) -> (
        &mut typed_trees::TypedTrees,
        &mut Vec<PreCheckEvaluation>,
        &mut Vec<build_time_evaluation::ExecutedCheckedConstantInput>,
    ) {
        (self.typed, self.pre_checks, self.constant_inputs)
    }
    pub fn project(
        &self,
        entry: symbols::SymbolHandle,
    ) -> Result<BuildSourceProjection, Vec<Diagnostic>> {
        Ok(BuildSourceProjection {
            projection: validation::execution_projection::project_build_program(self.typed, entry)?,
            identity: self.identity.clone(),
            constant_inputs: Arc::from(self.constant_inputs.as_slice()),
        })
    }
}

/// Authored bodies are complete; generated sources may extend this exact base.
pub struct CompletedSource {
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    executed_constant_inputs: Vec<build_time_evaluation::ExecutedCheckedConstantInput>,
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
    pub typed: typed_trees::TypedTrees,
    pub pending_pre_checks: Vec<PreCheckEvaluation>,
    /// Successful early evaluations survive their destructive source folds.
    /// This observed roster does not cover Build projection or all earlier work.
    pub executed_constant_inputs: Vec<build_time_evaluation::ExecutedCheckedConstantInput>,
}

impl SourceCompilationRequest {
    pub fn prepare(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<SourceContinuation, Vec<Diagnostic>> {
        let evaluated = build_time_evaluation::evaluate_pre_resolution(
            build_time_evaluation::BuildTimeEvaluationRequest {
                syntax_trees: self.syntax,
                source_context: Some(build_time_evaluation::BuildTimeSourceContext {
                    sources: Arc::clone(&self.sources),
                    source_scoped_top_level_bindings: &self.top_level_bindings,
                    selection_authority: self.selection_authority,
                    retained_base: None,
                }),
            },
        )?;
        let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
        let mut pending_bodies = observer.record_result(PsiCompilationStage::Resolution, || {
            syntax_trees_to_symbol_resolved_trees::resolve_deferred(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                    syntax,
                    sources: Some(self.sources),
                    top_level_bindings: self.top_level_bindings,
                },
            )
        })?;
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
            pending_bodies = observer
                .record_result(PsiCompilationStage::Resolution, || {
                    pending_bodies.complete_machine_bodies(&build_bodies)
                })?
                .into_resolved_frontier();
            // Completion can append attached implementations. Those use the
            // same scope test, not a second Product declaration selection.
        }
        let typing = observer.record_result(PsiCompilationStage::Typing, || {
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
                pending_bodies.shared_trees(),
            )
            .map_err(|diagnostic| vec![diagnostic])
        })?;
        Ok(SourceContinuation {
            typing,
            pending_bodies,
            pending_pre_checks: vec![pre_check],
            build_constant_inputs: Vec::new(),
        })
    }
}

impl SourceContinuation {
    /// Whether two admitted Build continuations have exactly the same remaining
    /// source work. A common pre-Build origin alone is insufficient: Build may
    /// have elaborated declarations or consumed different evaluation obligations.
    pub fn same_completion_input(&self, other: &Self) -> bool {
        self.typing == other.typing
            && self.pending_bodies == other.pending_bodies
            && self.build_constant_inputs == other.build_constant_inputs
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
            constant_inputs: &mut self.build_constant_inputs,
            identity: BuildSourceInputIdentity(Arc::new(BuildSourceInputOwner { _private: 0 })),
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
            // The completed Build retains its own original input and root.
            // Product observations cannot relabel that checked source owner.
            build_constant_inputs: _,
        } = self;
        loop {
            // Build may append concrete declarations. Join their exact suffix
            // before body resolution allocates more symbols in the same owner.
            pending_bodies = typing.join_generated_symbols(pending_bodies)?;
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
        let mut executed_constant_inputs = Vec::new();
        for pre_check in pending_pre_checks {
            if let Some(pending) =
                pre_check.evaluate_or_defer(typing.typed_mut(), &mut executed_constant_inputs)?
            {
                deferred.push(pending);
            }
        }
        Ok(CompletedSource {
            typing,
            pending_pre_checks: deferred,
            executed_constant_inputs,
        })
    }
}

impl CompletedSource {
    pub fn finish(self) -> PreparedSource {
        PreparedSource {
            typed: self.typing.into_typed(),
            pending_pre_checks: self.pending_pre_checks,
            executed_constant_inputs: self.executed_constant_inputs,
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
        let wire_schema_frontier = self.typing.typed().wire_schemas().len();
        let resolved_base = self.typing.resolved_base_for_extension()?;
        let (syntax, pre_checks) =
            build_time_evaluation::PreResolutionEvaluation::evaluate_extension(
                extension.units,
                extension.first_source,
                build_time_evaluation::BuildTimeSourceContext {
                    sources: Arc::clone(&extension.sources),
                    source_scoped_top_level_bindings: &[],
                    selection_authority: Some(extension.selection_authority),
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
        let mut typed = observer.record_result(PsiCompilationStage::Typing, || {
            self.typing.lower_extension(seeded)
        })?;
        self.pending_pre_checks
            .extend(PreCheckEvaluation::evaluate_extension_or_defer(
                pre_checks,
                wire_schema_frontier,
                &mut typed,
                &mut self.executed_constant_inputs,
            )?);
        Ok(PreparedSource {
            typed,
            pending_pre_checks: self.pending_pre_checks,
            executed_constant_inputs: self.executed_constant_inputs,
        })
    }
}
