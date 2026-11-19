use super::{PreCheckEvaluation, ResolveAndTypeRequest};
use crate::compile_time::BuildTimeSelectionAuthority;
use crate::source_inputs::SourceMap;
use crate::source_symbols::SourceScopedTopLevelBinding;
use crate::typing::SeededTypingBase;
use diagnostics::Diagnostic;
use source_text::{SourceId, SourceSpan, Span};
use std::collections::HashSet;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;

#[derive(Clone)]
pub(super) struct SourceContext {
    pub declaration_syntax: Arc<SyntaxTrees>,
    pub bindings: Vec<SourceScopedTopLevelBinding>,
    pub authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    pub build_helper_identity: std::borrow::Cow<'static, str>,
    pub recorded_authority:
        Option<crate::source_state::declaration_authority::RecordedDeclarationAuthority>,
}

impl SourceContext {
    pub fn same_input(&self, other: &Self) -> bool {
        self.recorded_authority.is_none()
            && other.recorded_authority.is_none()
            && Arc::ptr_eq(&self.declaration_syntax, &other.declaration_syntax)
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
                crate::source_state::declaration_authority::RecordedDeclarationAuthority::has_authority,
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
    pub declaration_syntax: Arc<SyntaxTrees>,
    sources: Arc<SourceMap>,
}

impl ActivatedSource {
    pub fn remove_retained_syntax(
        &self,
        syntax: &mut SyntaxTrees,
        source_items: &[(SourceId, crate::SourceItems)],
    ) {
        let retained = source_items
            .iter()
            .filter(|(source, _)| self.sources.source_is_selected(*source))
            .flat_map(|(_, items)| items.handles.iter().copied())
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
        let source_id = syntax.source_id;
        let selected = std::mem::replace(syntax, SyntaxTrees::new(source_id));
        syntax.extend_from(&selected);
    }
}

pub(super) fn select_retained_source(
    request: &ResolveAndTypeRequest,
    context: &SourceContext,
    observer: &mut impl crate::PsiCompilationObserver,
) -> Result<Option<ActivatedSource>, Vec<Diagnostic>> {
    let mut candidates = Vec::new();
    for source in request.frontends.snapshots()? {
        let typing = &source.typing;
        let source_context = &source.context;
        let Some(previous) = typing.typed().symbols.source_map() else {
            continue;
        };
        if !request.sources.retains_source_storage(&previous) {
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
        if source_context
            .compatibility(context, &previous, &request.sources, &activation)
            .is_err()
        {
            continue;
        }
        candidates.push((count, source, Arc::new(activation)));
    }
    candidates.sort_unstable_by_key(|(count, _, _)| std::cmp::Reverse(*count));
    for (_, source, sources) in candidates {
        let declaration_syntax = Arc::clone(&source.context.declaration_syntax);
        let (typing, pending_pre_checks) = super::SourceContinuation::complete_selected(
            source,
            Arc::clone(&sources),
            context.bindings.clone(),
            observer,
        )?;
        let typing = typing.activate_sources(
            Arc::clone(&sources),
            Arc::clone(&request.sources),
            context.bindings.clone(),
        )?;
        if typing.typed().machines().iter().any(|machine| {
            machine.body_status == crate::source_state::machine::MachineBodyStatus::Deferred
        }) {
            return Err(vec![Diagnostic::error(
                "selected retained source still has a deferred authored body",
            )]);
        }
        let references =
            crate::source_state::references::collect_program_references(typing.typed());
        let selections = typing
            .typed()
            .authored_declaration_selections()
            .iter()
            .filter(|selection| typing.typed().checks_authored_selection(selection))
            .filter_map(|selection| match selection.target() {
                crate::source_state::AuthoredDeclarationSelectionTarget::Resolved(target) => {
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
            continue;
        }
        let mut pre_checks = pending_pre_checks
            .iter()
            .map(|pending| pending.activate_sources(typing.typed()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut authority_miss = None;
        for pending in &mut pre_checks {
            if let Err(reason) =
                pending.rejoin_source_authority(typing.typed(), context.authority.clone())
            {
                authority_miss = Some(reason);
                break;
            }
        }
        if authority_miss.is_some() {
            continue;
        }
        return Ok(Some(ActivatedSource {
            typing,
            pre_checks,
            declaration_syntax,
            sources,
        }));
    }
    Ok(None)
}
