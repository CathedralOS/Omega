use super::{PreCheckEvaluation, ResolveAndTypeRequest};
use crate::source_inputs::SourceMap;
use crate::source_symbols::SourceScopedTopLevelBinding;
use declaration_scope::PackageDeclarationScope;
use diagnostics::Diagnostic;
use source_provenance::DependencyScope;
use source_text::{SourceId, SourceSpan, Span};
use std::collections::HashSet;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::SeededTypingBase;
use syntax_trees::SyntaxTrees;

#[derive(Clone)]
pub(super) struct SourceContext {
    pub bindings: Vec<SourceScopedTopLevelBinding>,
    pub declaration_scope: Option<Arc<PackageDeclarationScope>>,
    pub build_helper_identity: std::borrow::Cow<'static, str>,
    pub purpose: DependencyScope,
}

impl SourceContext {
    pub fn same_input(&self, other: &Self) -> bool {
        self.bindings == other.bindings
            && self.build_helper_identity == other.build_helper_identity
            && self.purpose == other.purpose
            && self.declaration_scope == other.declaration_scope
    }

    fn compatibility(
        &self,
        current: &Self,
        previous: &SourceMap,
        sources: &SourceMap,
        activation: &SourceMap,
    ) -> Result<(), SourceReuseMiss> {
        // A Build typing selects its host's Build helpers and completes only
        // Build bodies; a Product typing selects no helpers. Typings of
        // different purposes never share a helper selection, so a Product
        // never resumes a Build's typing, nor a Build a Product's.
        if self.purpose != current.purpose
            || self.build_helper_identity != current.build_helper_identity
        {
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
        // The retained requests answer only for the files this candidate
        // typed. Rejoin them before completing any body, so a candidate whose
        // packages lost a grant costs nothing.
        let mut pre_checks = source.pending_pre_checks.clone();
        if pre_checks.iter_mut().any(|pending| {
            pending
                .rejoin_source_authority(&activation, context.declaration_scope.clone())
                .is_err()
        }) {
            continue;
        }
        candidates.push((count, source, Arc::new(activation), pre_checks));
    }
    candidates.sort_by_key(|(count, ..)| std::cmp::Reverse(*count));
    for (_, source, sources, pre_checks) in candidates {
        let typing = super::SourceTypingCheckpoint::complete_selected(
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
        let pre_checks = pre_checks
            .iter()
            .map(|pending| pending.activate_sources(typing.typed()))
            .collect::<Result<Vec<_>, _>>()?;
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
    use semantic_vocabulary::PackageKeyIdentity;
    use source_admission::{SourceOrigin, SourceResolutionStratum};
    use source_provenance::DependencyScope;

    #[test]
    fn retained_source_checks_the_selected_packages_current_grants() {
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
            let fresh = SourceContext {
                bindings: vec![],
                declaration_scope: Some(original.clone()),
                build_helper_identity: "".into(),
                purpose,
            };
            let requests = PreCheckEvaluation::new(Some(original));
            let context = fresh.clone();
            assert!(matches!(
                context.compatibility(&fresh, &sources, &retargeted, &sources),
                Err(SourceReuseMiss::ProductAliases)
            ));
            // The same files typed for the other purpose are never resumed,
            // even when every other input agrees.
            let other_purpose = SourceContext {
                purpose: match purpose {
                    DependencyScope::Build => DependencyScope::Product,
                    DependencyScope::Product => DependencyScope::Build,
                },
                ..fresh.clone()
            };
            assert!(matches!(
                context.compatibility(&other_purpose, &sources, &sources, &sources),
                Err(SourceReuseMiss::BuildHelperSelection)
            ));
            assert!(!context.same_input(&other_purpose));
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
                        .rejoin_source_authority(&sources, current.declaration_scope)
                        .is_ok(),
                    allowed
                );
                assert_eq!(
                    PreCheckEvaluation::require_rejoined_authorities(&[pending]).is_ok(),
                    allowed
                );
            }
        }
    }

    /// A dependency's retained typing answers for its own files. The client
    /// that resumes it adds files of a package the dependency's scope never
    /// named; those are typed afresh under the client's scope, so they must
    /// not refuse the dependency's requests. A lost grant still refuses them.
    #[test]
    fn retained_requests_rejoin_only_the_packages_of_the_files_they_typed() {
        let [dependency, library, client] =
            [1, 2, 3].map(|id| PackageKeyIdentity::from_digest([id; 32]).unwrap());
        let add = |sources: &mut SourceMap, path: &str, root: &str, package| {
            sources.add_checked_instance(
                path.into(),
                "",
                root.into(),
                Some(package),
                SourceOrigin::User,
                SourceResolutionStratum::Base,
                DependencyScope::Product,
            );
        };
        let mut retained = SourceMap::default();
        add(
            &mut retained,
            "dependency/lib.omg",
            "dependency",
            dependency,
        );
        let mut request = retained.clone();
        add(&mut request, "client/main.omg", "client", client);
        let saved = Arc::new(PackageDeclarationScope::from_direct_dependencies(
            &[dependency, library],
            [(dependency, library)],
        ));
        let scope = |edges: &[(PackageKeyIdentity, PackageKeyIdentity)]| {
            Some(Arc::new(PackageDeclarationScope::from_direct_dependencies(
                &[dependency, library, client],
                edges.iter().copied(),
            )))
        };
        let keeps = scope(&[(dependency, library), (client, dependency)]);
        let loses = scope(&[(client, dependency)]);
        let pending = PreCheckEvaluation::new(Some(saved));

        let mut whole_request = pending.clone();
        assert!(
            whole_request
                .rejoin_source_authority(&request, keeps.clone())
                .is_err()
        );
        let mut resumed = pending.clone();
        resumed
            .rejoin_source_authority(&retained, keeps.clone())
            .unwrap();
        assert!(resumed.same_pending_work(&{
            let mut again = pending.clone();
            again.rejoin_source_authority(&retained, keeps).unwrap();
            again
        }));
        let mut lost = pending;
        assert!(lost.rejoin_source_authority(&retained, loses).is_err());
        assert!(!lost.same_pending_work(&resumed));
    }
}
