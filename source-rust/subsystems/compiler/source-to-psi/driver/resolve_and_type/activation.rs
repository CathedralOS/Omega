use super::{PreCheckEvaluation, ResolveAndTypeRequest};
use declaration_scope::PackageDeclarationScope;
use crate::source_inputs::SourceMap;
use crate::source_symbols::SourceScopedTopLevelBinding;
use symbol_resolved_trees_to_typed_trees::typing::SeededTypingBase;
use diagnostics::Diagnostic;
use source_text::{SourceId, SourceSpan, Span};
use std::collections::HashSet;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;

#[derive(Clone)]
pub(super) struct SourceContext {
    pub bindings: Vec<SourceScopedTopLevelBinding>,
    pub declaration_scope: Option<Arc<PackageDeclarationScope>>,
    pub build_helper_identity: std::borrow::Cow<'static, str>,
}

impl SourceContext {
    pub fn same_input(&self, other: &Self) -> bool {
        self.bindings == other.bindings
            && self.build_helper_identity == other.build_helper_identity
            && self.declaration_scope == other.declaration_scope
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
        }
        PackageDeclarationScope::require_current(
            self.declaration_scope.as_deref(),
            current.declaration_scope.as_deref(),
            activation
                .selected_files()
                .filter(|file| !file.origin.is_toolchain())
                .filter_map(|file| file.package_identity),
        )
        .map_err(|_| SourceReuseMiss::DeclarationAuthority)
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
    timings: &mut crate::CompileTimings,
) -> Result<Option<ActivatedSource>, Vec<Diagnostic>> {
    let mut candidates = Vec::new();
    for source in request.frontends.snapshots()? {
        let typing = &source.typing;
        let source_context = &source.context;
        // Earlier compilers could save a checked prefix under the complete
        // source's identity. Check that source afresh instead of reusing it.
        if typing.typed().roots.partial_build_entry.is_some()
            || !typing.typed().parked_build_locals.is_empty()
        {
            continue;
        }
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
        let (typing, pending_pre_checks) = super::SourceTypingCheckpoint::complete_selected(
            source,
            Arc::clone(&sources),
            context.bindings.clone(),
            timings,
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
                pending.rejoin_source_authority(typing.typed(), context.declaration_scope.clone())
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
            sources,
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::TypedTrees;
    use crate::source_symbols::SymbolTableBuilder;
    use semantic_vocabulary::PackageKeyIdentity;
    use source_admission::{SourceOrigin, SourceResolutionStratum};
    use source_provenance::DependencyScope;

    #[test]
    fn fresh_and_reopened_source_check_the_selected_packages_current_grants() {
        let [root, helper, product, backup] =
            [1, 2, 3, 4].map(|id| PackageKeyIdentity::from_digest([id; 32]).unwrap());
        let scope = |edges: &[(PackageKeyIdentity, PackageKeyIdentity)]| {
            Arc::new(PackageDeclarationScope::from_direct_dependencies(
                &[root, helper, product, backup],
                edges.iter().copied(),
            ))
        };
        for (purpose, granted, other) in [
            (DependencyScope::Build, helper, product),
            (DependencyScope::Product, product, helper),
        ] {
            let original = scope(&[(root, granted), (root, backup)]);
            let mut sources = SourceMap::default();
            sources.add_checked_instance(
                "root/entry.omg".into(),
                "",
                "root".into(),
                Some(root),
                SourceOrigin::User,
                SourceResolutionStratum::Base,
                purpose,
            );
            sources.retain_product_dependency_scope([
                (root, "service", product),
                (root, "backup", backup),
            ]);
            let sources = Arc::new(sources);
            let mut retargeted = sources.as_ref().clone();
            retargeted.retain_product_dependency_scope([
                (root, "service", backup),
                (root, "backup", product),
            ]);
            let mut typed = TypedTrees {
                symbols: SymbolTableBuilder::with_sources(Some(sources.clone())).finish(),
                ..TypedTrees::default()
            };
            let fresh = SourceContext {
                bindings: vec![],
                declaration_scope: Some(original.clone()),
                build_helper_identity: "".into(),
            };
            let pending = PreCheckEvaluation::new(Some(original));
            let limits = record_storage::RecordLimits::default();
            let reopened =
                record_storage::decode(&record_storage::encode(&fresh, limits).unwrap(), limits)
                    .unwrap();
            let reopened_pending =
                record_storage::decode(&record_storage::encode(&pending, limits).unwrap(), limits)
                    .unwrap();
            for (context, requests) in [(fresh.clone(), pending), (reopened, reopened_pending)] {
                assert!(matches!(
                    context.compatibility(&fresh, &sources, &retargeted, &sources),
                    Err(SourceReuseMiss::ProductAliases)
                ));
                let saved = context.declaration_scope.as_ref().unwrap();
                assert!(saved.allows_declaration_selection(root, granted));
                assert!(!saved.allows_declaration_selection(root, other));
                for (edges, allowed) in [
                    (
                        vec![(root, granted), (root, backup), (other, granted)],
                        true,
                    ),
                    (vec![(root, backup)], false),
                    (vec![(root, backup), (root, other)], false),
                ] {
                    let current = SourceContext {
                        declaration_scope: Some(scope(&edges)),
                        ..fresh.clone()
                    };
                    assert_eq!(
                        context
                            .compatibility(&current, &sources, &sources, &sources)
                            .is_ok(),
                        allowed
                    );
                    let mut pending = requests.clone();
                    assert_eq!(
                        pending
                            .rejoin_source_authority(&typed, current.declaration_scope)
                            .is_ok(),
                        allowed
                    );
                    assert_eq!(
                        PreCheckEvaluation::record_continuations(vec![pending], &mut typed).is_ok(),
                        allowed
                    );
                }
            }
        }
    }
}
