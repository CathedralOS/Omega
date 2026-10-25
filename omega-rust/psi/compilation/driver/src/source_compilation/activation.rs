//! Retained source is usable only under an unchanged lookup environment.
//!
//! A source catalogue proves coordinate custody, not name lookup. A new file
//! in an exposed checked instance can introduce a competing declaration even
//! when the old file and its imports did not change. Such a request resolves
//! afresh; an unrelated consumer package does not invalidate its dependencies.
//! Current Build admission and subsequent language checking remain independent.

use super::{PreCheckEvaluation, SourceCompilationRequest};
use build_time_evaluation::BuildTimeSelectionAuthority;
use diagnostics::Diagnostic;
use source::{SourceId, SourceMap, SourceSpan, Span};
use std::collections::HashSet;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::SeededTypingBase;
use symbols::SourceScopedTopLevelBinding;
use syntax_trees::SyntaxTrees;

#[derive(Clone)]
pub(super) struct SourceContext {
    pub bindings: Vec<SourceScopedTopLevelBinding>,
    pub authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    pub build_helper_identity: &'static str,
}

impl SourceContext {
    pub fn same_input(&self, other: &Self) -> bool {
        self.bindings == other.bindings
            && self.build_helper_identity == other.build_helper_identity
            && match (&self.authority, &other.authority) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
    }

    fn compatibility(
        &self,
        current: &Self,
        previous: &SourceMap,
        sources: &SourceMap,
        activation: &SourceMap,
    ) -> Result<(), SourceReuseMiss> {
        if self.build_helper_identity != current.build_helper_identity {
            return Err(SourceReuseMiss::BuildHelperSelection);
        }
        for added in sources
            .selected_files()
            .filter(|file| !previous.source_is_selected(file.source_id))
        {
            let added_span = SourceSpan::new(added.source_id, Span::default());
            if previous.selected_files().any(|file| {
                sources.same_checked_instance(
                    added_span,
                    SourceSpan::new(file.source_id, Span::default()),
                )
            }) {
                return Err(SourceReuseMiss::LookupContributors);
            }
        }
        for file in activation.selected_files() {
            let old_bindings = self
                .bindings
                .iter()
                .filter(|binding| binding.reference_source() == file.source_id);
            let new_bindings = current
                .bindings
                .iter()
                .filter(|binding| binding.reference_source() == file.source_id);
            if !old_bindings.eq(new_bindings) {
                return Err(SourceReuseMiss::Imports);
            }
            let Some(requester) = file.package_identity else {
                continue;
            };
            if !sources.same_product_dependency_scope(previous, requester) {
                return Err(SourceReuseMiss::ProductAliases);
            }
            match (&self.authority, &current.authority) {
                (None, None) => {}
                (Some(old), Some(new)) => {
                    for owner in sources.files().filter_map(|file| file.package_identity) {
                        if old.allows_declaration_selection(requester, owner)
                            != new.allows_declaration_selection(requester, owner)
                        {
                            return Err(SourceReuseMiss::DeclarationAuthority);
                        }
                    }
                }
                _ => return Err(SourceReuseMiss::DeclarationAuthority),
            }
        }
        Ok(())
    }
}

enum SourceReuseMiss {
    BuildHelperSelection,
    LookupContributors,
    Imports,
    ProductAliases,
    DeclarationAuthority,
}

pub(super) struct ActivatedSource {
    pub typing: SeededTypingBase,
    pub pre_checks: Vec<PreCheckEvaluation>,
    sources: Arc<SourceMap>,
}

impl ActivatedSource {
    pub fn remove_retained_syntax(
        &self,
        syntax: &mut SyntaxTrees,
        source_items: &[(SourceId, Vec<syntax_trees::item::ItemHandle>)],
    ) {
        let retained = source_items
            .iter()
            .filter(|(source, _)| self.sources.source_is_selected(*source))
            .flat_map(|(_, items)| items.iter().copied())
            .collect::<HashSet<_>>();
        syntax.roots.items.retain(|item| !retained.contains(item));
        let items = &syntax.tables.items;
        syntax.roots.mathematical_definitions.retain(|handle| {
            let span = items.mathematical_definition(*handle).name.source_span();
            span.span.start == span.span.end || !self.sources.source_is_selected(span.source_id)
        });
        let expressions = &syntax.tables.expressions;
        syntax.roots.constant_extent_probes.retain(|(handle, _)| {
            let span = expressions.source_span(*handle);
            span.span.start == span.span.end || !self.sources.source_is_selected(span.source_id)
        });
        // Pre-resolution also enumerates syntax type/expression tables. Copy
        // only this new cohort's rooted syntax; inactive parser pages must not
        // become fresh work beside the retained resolved/typed declarations.
        let source_id = syntax.source_id;
        let selected = std::mem::replace(syntax, SyntaxTrees::new(source_id));
        syntax.extend_from(&selected);
        for (expression, public) in &selected.roots.constant_extent_probes {
            let copied = syntax.copy_expression_from(&selected, *expression);
            syntax.roots.constant_extent_probes.push((copied, *public));
        }
    }
}

pub(super) fn select_retained_source(
    request: &SourceCompilationRequest,
    context: &SourceContext,
) -> Result<Option<ActivatedSource>, Vec<Diagnostic>> {
    // An anonymous request has no explicit consumer identity to separate from
    // its dependencies here. Its existing exact-frontier path remains valid.
    if request.root_package.is_none() {
        return Ok(None);
    }
    phase_profile::count(
        "source_activation.candidates",
        request.previous.len() as u64,
    );
    let mut candidates = Vec::new();
    for source in request
        .previous
        .iter()
        .filter_map(|check| check.source.as_ref())
    {
        let Some(previous) = source.typed().symbols.source_map() else {
            continue;
        };
        if !request.sources.retains_source_storage(&previous) {
            phase_profile::count("source_activation.coordinate_misses", 1);
            continue;
        }
        let mut activation = request.sources.as_ref().clone();
        activation.retain_selected_sources(|file| {
            previous.source_is_selected(file.source_id)
                && (request.root_package.is_none() || file.package_identity != request.root_package)
        });
        let count = activation.selected_files().count();
        if count == 0 {
            continue;
        }
        if let Err(reason) =
            source
                .context
                .compatibility(context, &previous, &request.sources, &activation)
        {
            phase_profile::count(
                match reason {
                    SourceReuseMiss::BuildHelperSelection => "source_activation.miss.build_helpers",
                    SourceReuseMiss::LookupContributors => {
                        "source_activation.miss.lookup_contributors"
                    }
                    SourceReuseMiss::Imports => "source_activation.miss.imports",
                    SourceReuseMiss::ProductAliases => "source_activation.miss.product_aliases",
                    SourceReuseMiss::DeclarationAuthority => "source_activation.miss.authority",
                },
                1,
            );
            continue;
        }
        candidates.push((count, source, Arc::new(activation)));
    }
    candidates.sort_unstable_by_key(|(count, _, _)| std::cmp::Reverse(*count));
    for (count, source, sources) in candidates {
        let typing = source.typing.clone().activate_sources(
            Arc::clone(&sources),
            Arc::clone(&request.sources),
            context.bindings.clone(),
        )?;
        // Removal cannot leave a retained resolved use naming an absent source.
        // A changed closure takes ordinary resolution, never a source rejection.
        let references = typed_trees::references::collect_program_references(typing.typed());
        // Folded calls and descriptor selections are not executable references,
        // but their exact declaration targets must remain available as well.
        let selections = typing
            .typed()
            .authored_declaration_selections()
            .iter()
            .filter(|selection| typing.typed().checks_authored_selection(selection))
            .filter_map(|selection| match selection.target() {
                typed_trees::AuthoredDeclarationSelectionTarget::Resolved(target) => {
                    Some(target.selected_symbol())
                }
                _ => None,
            });
        if references
            .symbols
            .iter()
            .copied()
            .chain(selections)
            .any(|symbol| {
                typing
                    .typed()
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .is_some_and(|span| !sources.source_is_selected(span.source_id))
            })
        {
            phase_profile::count("source_activation.miss.reference_closure", 1);
            continue;
        }
        let pre_checks = source
            .pending_pre_checks
            .iter()
            .map(|pending| pending.activate_sources(source.typed(), typing.typed()))
            .collect::<Result<_, _>>()?;
        phase_profile::count("source_activation.retained_sources", count as u64);
        return Ok(Some(ActivatedSource {
            typing,
            pre_checks,
            sources,
        }));
    }
    Ok(None)
}
