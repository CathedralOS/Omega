//! Resolve declarations and type authored bodies. Retain both frontiers so Build queries
//! and generated source can resume from the same declarations and source identities.

use syntax_trees_to_symbol_resolved_trees::resolution;
use symbol_resolved_trees_to_typed_trees as typing;
use crate::compile_time_invocations::PreCheckEvaluation;
use declaration_scope::PackageDeclarationScope;
use artifacts::compile_timings::{CompileTimings, source as stages};
use diagnostics::{Diagnostic, DiagnosticStage};
use std::sync::Arc;

mod activation;
use activation::SourceContext;

mod authored_completion;
use authored_completion::AuthoredCompletions;
mod storage;

#[derive(Clone)]
pub struct ResolveAndTypeRequest {
    pub syntax: crate::SourceSyntax,
    pub source_items: Vec<(source_text::SourceId, crate::SourceItems)>,
    pub root_sources: Vec<source_text::SourceId>,
    pub frontends: SourceFrontends,
    pub build_helper_identity: &'static str,
    /// Whether this typing checks a Build or a Product. Retained typings are
    /// resumed only by a request of the same purpose.
    pub purpose: source_provenance::DependencyScope,
    pub sources: Arc<crate::source_inputs::SourceMap>,
    pub top_level_bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
    pub selection_authority: Option<Arc<PackageDeclarationScope>>,
}

#[derive(Clone, record_storage::Record)]
pub(crate) struct SourceTypingCheckpoint {
    input: Arc<SourceCompletionInput>,
    completions: AuthoredCompletions,
}

#[derive(Clone, record_storage::Record)]
struct SourceCompletionInput {
    typing: typing::SeededTypingBase,
    pending_bodies: resolution::DeferredResolvedFrontier,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    context: SourceContext,
}

#[derive(Clone, record_storage::Record)]
pub(crate) struct CompletedSource {
    pub accounting: crate::compile_time_invocations::SourceCompletionAccounting,
    pub typed: crate::source_state::TypedTrees,
}

impl ResolveAndTypeRequest {
    /// A dual-purpose file is checked as Build source and as Product source. Its selected
    /// Build declaration has one owner in the joined forest. All its other declarations
    /// remain visible to each stage under that stage's own imports.
    pub(crate) fn with_build_entry(
        mut self,
        entry: Option<&source_admission::RetainedDeclaration>,
    ) -> Self {
        let Some(declaration) = entry else {
            return self;
        };
        let entry = declaration.source();
        use source_provenance::DependencyScope;
        let counterpart = self.sources.selected_files().find(|file| {
            file.dependency_scope == DependencyScope::Product
                && file.path == entry.path
                && file.source == entry.source
                && file.origin == entry.origin
                && file.package_root == entry.package_root
                && file.resolution_stratum == entry.resolution_stratum
                && entry
                    .package_identity
                    .is_none_or(|id| file.package_identity == Some(id))
        });
        let Some(product) = counterpart else {
            return self;
        };
        let syntax = &mut self.syntax.trees;
        let Some((_, roots)) = self
            .source_items
            .iter_mut()
            .find(|(id, _)| *id == product.source_id)
        else {
            return self;
        };
        if let Some(handle) = roots.handles.iter().copied().find(|&handle| {
            syntax
                .root_item(handle)
                .as_machine()
                .is_some_and(|machine| {
                    machine.attached_data.is_none()
                        && machine.name.source_span().span == declaration.span()
                })
        }) {
            roots.handles.retain(|&item| item != handle);
            syntax.roots.items.retain(|&item| item != handle);
        }
        self
    }

    pub(crate) fn prepare(
        self,
        timings: &mut CompileTimings,
    ) -> Result<SourceTypingCheckpoint, Vec<Diagnostic>> {
        let scope = eager_body_scope(self.purpose);
        self.prepare_with_body_scope(
            scope,
            resolution::DeferredBodyScope::BuildAndToolchain,
            true,
            timings,
        )
    }

    pub(crate) fn prepare_declarations(
        self,
        timings: &mut CompileTimings,
    ) -> Result<SourceTypingCheckpoint, Vec<Diagnostic>> {
        self.prepare_with_body_scope(
            resolution::DeferredBodyScope::BuildOnly,
            resolution::DeferredBodyScope::BuildOnly,
            false,
            timings,
        )
    }

    fn prepare_with_body_scope(
        mut self,
        eager_scope: resolution::DeferredBodyScope,
        followup_scope: resolution::DeferredBodyScope,
        retain_frontend: bool,
        timings: &mut CompileTimings,
    ) -> Result<SourceTypingCheckpoint, Vec<Diagnostic>> {
        let context = SourceContext {
            bindings: self.top_level_bindings.clone(),
            declaration_scope: self.selection_authority.clone(),
            build_helper_identity: self.build_helper_identity.into(),
            purpose: self.purpose,
        };
        let activation = activation::select_retained_source(&self, &context, timings)?;
        if let Some(activation) = &activation {
            activation.remove_retained_syntax(&mut self.syntax.trees, &self.source_items);
        }
        // Check the predecessor while its owner can still be borrowed. Report
        // any refusal after checking the successor to preserve diagnostic order.
        let retained_requests = activation.as_ref().map(|activation| {
            PreCheckEvaluation::require_recorded_requests(activation.typing.typed())
        });
        let syntax = self.syntax.trees;
        let pre_check = PreCheckEvaluation::new(self.selection_authority);
        let (mut typing, mut pending_bodies, mut pending_pre_checks) = if let Some(mut activation) =
            activation
        {
            let base = activation.typing.resolved_base_for_extension()?;
            let (typing, frontier) = extend_declarations(
                activation.typing,
                resolution::ExtensionRequest {
                    base,
                    syntax,
                    sources: self.sources,
                    // Activation already installed the complete binding set for these sources.
                    top_level_bindings: Vec::new(),
                },
                timings,
            )?;
            (typing, frontier, activation.pre_checks)
        } else {
            let frontier = resolve_eager_bodies(
                resolution::ResolutionRequest {
                    syntax,
                    sources: Some(self.sources),
                    top_level_bindings: self.top_level_bindings,
                    retained_base: None,
                },
                eager_scope,
                timings,
            )?;
            let typing = {
                timings
                    .record_result(stages::TYPE, || {
                        typing::lower_symbol_resolved_trees_to_seeded_base(frontier.shared_trees())
                            .map_err(|diagnostic| vec![diagnostic])
                    })
                    .map_err(|rows| DiagnosticStage::Typing.attribute(rows))?
            };
            (typing, frontier, Vec::new())
        };
        loop {
            let build_bodies = pending_bodies.pending_machines_in(followup_scope)?;
            if build_bodies.is_empty() {
                break;
            }
            pending_bodies =
                pending_bodies.with_prepared_allocations(typing.prepare_allocations()?)?;
            let completed = timings
                .record_result(stages::RESOLVE, || {
                    pending_bodies.complete_machine_bodies(&build_bodies)
                })
                .map_err(|rows| DiagnosticStage::Resolution.attribute(rows))?;
            (typing, pending_bodies) = timings
                .record_result(stages::TYPE, || type_completed_bodies(typing, completed))
                .map_err(|rows| DiagnosticStage::Typing.attribute(rows))?;
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
        let source = SourceTypingCheckpoint {
            input,
            completions: Default::default(),
        };
        if retain_frontend {
            self.frontends.retain(&source)?;
        }
        Ok(source)
    }
}

/// The bodies a fresh typing resolves before its first typed cohort.
///
/// Every completion round after that cohort repeats whole-program typing
/// passes, whatever its size. A Build typing is kept in its invocation's
/// shared frontends, so it resolves only Build and toolchain bodies and leaves
/// the others deferred for whichever later typing resumes it. The only
/// consumer of a Product typing, `complete`, completes every body,
/// so the Product resolves them all here and types them in one cohort.
fn eager_body_scope(purpose: source_provenance::DependencyScope) -> resolution::DeferredBodyScope {
    use resolution::DeferredBodyScope;
    use source_provenance::DependencyScope;
    match purpose {
        DependencyScope::Product => DeferredBodyScope::All,
        DependencyScope::Build => DeferredBodyScope::BuildAndToolchain,
    }
}

fn resolve_eager_bodies(
    request: resolution::ResolutionRequest,
    scope: resolution::DeferredBodyScope,
    timings: &mut CompileTimings,
) -> Result<resolution::DeferredResolvedFrontier, Vec<Diagnostic>> {
    let mut frontier = timings
        .record_result(stages::RESOLVE, || resolution::resolve_deferred(request))
        .map_err(|rows| DiagnosticStage::Resolution.attribute(rows))?;
    loop {
        let selected = frontier.pending_machines_in(scope)?;
        if selected.is_empty() {
            return Ok(frontier);
        }
        frontier = timings
            .record_result(stages::RESOLVE, || {
                frontier.complete_machine_bodies(&selected)
            })
            .map_err(|rows| DiagnosticStage::Resolution.attribute(rows))?
            .into_resolved_frontier();
    }
}

impl SourceTypingCheckpoint {
    fn complete_selected(
        input: Arc<SourceCompletionInput>,
        sources: Arc<crate::source_inputs::SourceMap>,
        bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
        timings: &mut CompileTimings,
    ) -> Result<typing::SeededTypingBase, Vec<Diagnostic>> {
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
            resolution::DeferredBodyScope::SelectedFiles,
            timings,
        )?;
        Ok(typing)
    }

    pub(super) fn shared_typed(&self) -> Arc<typed_trees::TypedTrees> {
        self.input.typing.shared_typed()
    }

    #[cfg(test)]
    pub(crate) fn typed(&self) -> &crate::source_state::TypedTrees {
        self.input.typing.typed()
    }

    pub(crate) fn into_typed(self) -> crate::source_state::TypedTrees {
        Arc::unwrap_or_clone(self.input).typing.into_typed()
    }

    pub(crate) fn complete(
        self,
        timings: &mut CompileTimings,
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
            ..
        } = Arc::unwrap_or_clone(owned_input);
        (typing, pending_bodies) = complete_pending_bodies(
            typing,
            pending_bodies,
            resolution::DeferredBodyScope::All,
            timings,
        )?;
        drop(pending_bodies);
        PreCheckEvaluation::record_continuations(pending_pre_checks, typing.typed_mut())?;
        let source = CompletedSource {
            typed: typing.into_typed(),
            accounting: Default::default(),
        };
        completions.retain(input, &source)?;
        Ok(source)
    }
}

// Resolution retains pending syntax; typing receives only the completed trees.
// The driver joins their exact checkpoints before either stage resumes.
fn type_completed_bodies(
    typing: typing::SeededTypingBase,
    completed: resolution::ResolvedBodyCompletion,
) -> Result<
    (
        typing::SeededTypingBase,
        resolution::DeferredResolvedFrontier,
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

/// Resolve and type an appended source forest against its exact predecessor.
fn extend_declarations(
    typing: typing::SeededTypingBase,
    request: resolution::ExtensionRequest,
    timings: &mut CompileTimings,
) -> Result<
    (
        typing::SeededTypingBase,
        resolution::DeferredResolvedFrontier,
    ),
    Vec<Diagnostic>,
> {
    let extension = timings
        .record_result(stages::RESOLVE, || {
            resolution::resolve_deferred_extension(request)
        })
        .map_err(|rows| DiagnosticStage::Resolution.attribute(rows))?;
    timings
        .record_result(stages::TYPE, || {
            let (source, frontier) = extension
                .rebase_for_typed_continuation(typing.typed().authored_declaration_selections())?;
            Ok((typing.append_resolved(source)?, frontier))
        })
        .map_err(|rows| DiagnosticStage::Typing.attribute(rows))
}

fn complete_pending_bodies(
    mut typing: typing::SeededTypingBase,
    mut pending_bodies: resolution::DeferredResolvedFrontier,
    scope: resolution::DeferredBodyScope,
    timings: &mut CompileTimings,
) -> Result<
    (
        typing::SeededTypingBase,
        resolution::DeferredResolvedFrontier,
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
            timings
                .record_result(stages::RESOLVE, || {
                    pending_bodies.complete_machine_bodies(&selected)
                })
                .map_err(|rows| DiagnosticStage::Resolution.attribute(rows))?
        };
        (typing, pending_bodies) = {
            timings
                .record_result(stages::TYPE, || type_completed_bodies(typing, resolved))
                .map_err(|rows| DiagnosticStage::Typing.attribute(rows))?
        };
    }
    Ok((typing, pending_bodies))
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
    pub(crate) fn retain(&self, source: &SourceTypingCheckpoint) -> Result<(), Vec<Diagnostic>> {
        self.entries
            .lock()
            .map_err(|_| {
                vec![Diagnostic::error(
                    "source frontend retention owner is poisoned",
                )]
            })?
            .push(Arc::clone(&source.input));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
