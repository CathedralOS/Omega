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
    /// Freshly acquired parsed declarations remain lookup context when their
    /// resolved/typed owners leave the new cohort's worklist.
    pub declaration_syntax: Arc<SyntaxTrees>,
    /// Exact inherited normalization lookup, including prepared instances from
    /// every preceding cohort. These roots never become current work.
    pub normalization_context: Option<Arc<SyntaxTrees>>,
    pub bindings: Vec<SourceScopedTopLevelBinding>,
    pub authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    pub build_helper_identity: std::borrow::Cow<'static, str>,
    pub recorded_authority: Option<validation::RecordedDeclarationAuthority>,
}

impl SourceContext {
    pub fn same_input(&self, other: &Self) -> bool {
        self.recorded_authority.is_none()
            && other.recorded_authority.is_none()
            && Arc::ptr_eq(&self.declaration_syntax, &other.declaration_syntax)
            && match (&self.normalization_context, &other.normalization_context) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
            && self.bindings == other.bindings
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
            if current.recorded_authority.is_some() {
                return Err(SourceReuseMiss::DeclarationAuthority);
            }
            let old_has_authority = self.recorded_authority.as_ref().map_or(
                self.authority.is_some(),
                validation::RecordedDeclarationAuthority::has_authority,
            );
            if old_has_authority != current.authority.is_some() {
                return Err(SourceReuseMiss::DeclarationAuthority);
            }
            if let Some(new) = &current.authority {
                for owner in sources
                    .selected_files()
                    .filter_map(|file| file.package_identity)
                {
                    let old = match &self.recorded_authority {
                        Some(recorded) => recorded.allows(requester, owner),
                        None => self
                            .authority
                            .as_ref()
                            .map(|old| old.allows_declaration_selection(requester, owner)),
                    }
                    .ok_or(SourceReuseMiss::DeclarationAuthority)?;
                    if old != new.allows_declaration_selection(requester, owner) {
                        return Err(SourceReuseMiss::DeclarationAuthority);
                    }
                }
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
    template_context: Arc<SyntaxTrees>,
}

impl ActivatedSource {
    pub fn remove_retained_syntax(
        &self,
        syntax: &mut SyntaxTrees,
        source_items: &[(SourceId, Vec<syntax_trees::item::ItemHandle>)],
    ) -> SyntaxTrees {
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
        // The prepared predecessor also owns already materialized generic
        // instances. Reusing only raw parsed templates would synthesize their
        // variants twice in the receiving symbol world.
        let prepared = self.template_context.as_ref();
        let items = prepared
            .root_item_handles()
            .iter()
            .copied()
            .filter(|item| {
                use syntax_trees::item::Item;
                let span = match prepared.root_item(*item) {
                    Item::Data(value) => Some(value.name.source_span()),
                    Item::Const(value) => Some(value.name.source_span()),
                    Item::Domain(value) => Some(value.name.source_span()),
                    Item::Machine(value) => Some(value.name.source_span()),
                    Item::Trait(value) => Some(value.name.source_span()),
                    Item::Module(value) => prepared
                        .items
                        .identifier_path_members(value.path)
                        .first()
                        .map(|name| name.source_span()),
                    Item::Use(value) => prepared
                        .items
                        .identifier_path_members(value.path)
                        .first()
                        .map(|name| name.source_span()),
                    Item::Package(value) => prepared
                        .items
                        .identifier_path_members(value.path)
                        .first()
                        .map(|name| name.source_span()),
                    Item::Operator(value) => prepared
                        .items
                        .identifier_path_members(value.name)
                        .first()
                        .map(|name| name.source_span()),
                    Item::Measure(value) => prepared
                        .items
                        .identifier_path_members(value.name)
                        .first()
                        .map(|name| name.source_span()),
                    Item::Capability(value) => Some(value.name.source_span()),
                    Item::Proposition(value) => Some(value.name.source_span()),
                    Item::Conformance(value) => Some(
                        value
                            .alias
                            .as_ref()
                            .unwrap_or(&value.trait_name)
                            .source_span(),
                    ),
                };
                span.is_some_and(|span| self.sources.source_is_selected(span.source_id))
            })
            .collect::<Vec<_>>();
        let mathematics = prepared
            .root_mathematical_definition_handles()
            .iter()
            .copied()
            .filter(|handle| {
                self.sources.source_is_selected(
                    prepared
                        .root_mathematical_definition(*handle)
                        .name
                        .source_span()
                        .source_id,
                )
            })
            .collect::<Vec<_>>();
        let mut context = SyntaxTrees::new(prepared.source_id);
        context.extend_selected_roots_from(prepared, &items, &mathematics);
        context
    }
}

pub(super) fn select_retained_source(
    request: &SourceCompilationRequest,
    context: &SourceContext,
    observer: &mut impl crate::PsiCompilationObserver,
) -> Result<Option<ActivatedSource>, Vec<Diagnostic>> {
    phase_profile::count(
        "source_activation.candidates",
        request.frontends.snapshots()?.len() as u64,
    );
    let mut candidates = Vec::new();
    for source in request.frontends.snapshots()? {
        let typing = &source.typing;
        let source_context = &source.context;
        let Some(previous) = typing.typed().symbols.source_map() else {
            continue;
        };
        if !request.sources.retains_source_storage(&previous) {
            phase_profile::count("source_activation.coordinate_misses", 1);
            continue;
        }
        let mut activation = request.sources.as_ref().clone();
        activation.retain_selected_sources(|file| {
            previous.source_is_selected(file.source_id)
                && !request.root_sources.contains(&file.source_id)
        });
        let count = activation.selected_files().count();
        if count == 0 {
            continue;
        }
        if let Err(reason) =
            source_context.compatibility(context, &previous, &request.sources, &activation)
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
        let prepared = source.pending_bodies.generic_normalization_context();
        let template_context = match &source.context.normalization_context {
            Some(inherited) => {
                // New cohort roots and inherited lookup roots are disjoint by
                // construction: normalization removes its borrowed input roots
                // before publishing the resolved cohort.
                let mut complete = inherited.as_ref().clone();
                complete.extend_from(&prepared);
                Arc::new(complete)
            }
            None => prepared,
        };
        let (typing, pending_pre_checks) = super::SourceContinuation::complete_selected(
            source,
            Arc::clone(&sources),
            context.bindings.clone(),
            observer,
        )?;
        let original = typing.clone();
        let typing = typing.activate_sources(
            Arc::clone(&sources),
            Arc::clone(&request.sources),
            context.bindings.clone(),
        )?;
        if typing
            .typed()
            .machines()
            .iter()
            .any(|machine| machine.body_status == typed_trees::machine::MachineBodyStatus::Deferred)
        {
            return Err(vec![Diagnostic::error(
                "selected retained source still has a deferred authored body",
            )]);
        }
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
        let mut pre_checks = pending_pre_checks
            .iter()
            .map(|pending| pending.activate_sources(original.typed(), typing.typed()))
            .collect::<Result<Vec<_>, _>>()?;
        // Projection removes the old consumer's work before authority rejoins.
        // Exact whole-frontier hits keep the stronger full-context comparison.
        let mut authority_miss = None;
        for pending in &mut pre_checks {
            if let Err(reason) =
                pending.rejoin_source_authority(typing.typed(), context.authority.clone())
            {
                authority_miss = Some(reason);
                break;
            }
        }
        if let Some(reason) = authority_miss {
            phase_profile::count("source_activation.miss.authority", 1);
            if phase_profile::enabled() {
                eprintln!("retained pending source work ignored: {reason}");
            }
            continue;
        }
        phase_profile::count("source_activation.retained_sources", count as u64);
        return Ok(Some(ActivatedSource {
            typing,
            pre_checks,
            sources,
            template_context,
        }));
    }
    Ok(None)
}
