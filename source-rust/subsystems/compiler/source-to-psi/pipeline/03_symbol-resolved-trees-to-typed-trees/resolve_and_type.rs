/*
    Resolver and typing continuation across admitted Build execution, generated
    source extension, and post-join Product policy admission.

    We pair the resolver frontier with its seeded typing base inside
    `SourceContinuation`, complete authored bodies into `CompletedSource`,
    extend completed sources with admitted generated units, and admit Product
    provider requests, root grants, declarations, and behavior exclusions.
*/

use crate::evaluation::{BuildTimeSelectionAuthority, PreCheckEvaluation};
use crate::{PsiCompilationObserver, PsiCompilationStage};
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::product_declarations;

mod activation {
    use super::{PreCheckEvaluation, ResolveAndTypeRequest};
    use crate::evaluation::BuildTimeSelectionAuthority;
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
}
use activation::SourceContext;

mod authored_completion {
    use std::sync::{Arc, Mutex};

    use diagnostics::Diagnostic;

    use super::{CompletedSource, SourceCompletionInput};

    #[derive(Clone, Default, record_storage::Record)]
    pub(super) struct AuthoredCompletions {
        entries: Arc<Mutex<Vec<Completion>>>,
    }

    #[derive(record_storage::Record)]
    struct Completion {
        input: Arc<SourceCompletionInput>,
        source: CompletedSource,
    }

    impl AuthoredCompletions {
        pub(super) fn find(
            &self,
            input: &SourceCompletionInput,
        ) -> Result<Option<CompletedSource>, Vec<Diagnostic>> {
            let entries = self.entries.lock().map_err(|_| unavailable())?;
            let Some(completion) = entries
                .iter()
                .find(|entry| entry.input.same_completion_input(input))
            else {
                return Ok(None);
            };
            completion
                .source
                .accounting
                .validate_charges()
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let mut source = completion.source.clone();
            source.accounting.mark_reused();
            Ok(Some(source))
        }

        pub(super) fn retain(
            &self,
            input: Arc<SourceCompletionInput>,
            source: &CompletedSource,
        ) -> Result<(), Vec<Diagnostic>> {
            if !source.accounting.has_complete_usage() {
                return Ok(());
            }
            source
                .accounting
                .validate_charges()
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            self.entries
                .lock()
                .map_err(|_| unavailable())?
                .push(Completion {
                    input,
                    source: source.clone(),
                });
            Ok(())
        }
    }

    fn unavailable() -> Vec<Diagnostic> {
        vec![Diagnostic::error(
            "authored source completion retention owner is poisoned",
        )]
    }

    impl AuthoredCompletions {
        pub(super) fn rejoin_current_authority(
            &self,
            authority: Option<Arc<dyn crate::evaluation::BuildTimeSelectionAuthority>>,
        ) -> Result<Self, String> {
            let stored = self
                .entries
                .lock()
                .map_err(|_| "authored source completion retention owner is poisoned")?;
            let mut entries = Vec::new();
            for entry in stored.iter() {
                let Ok(input) = entry.input.rejoin_current_authority(authority.clone()) else {
                    continue;
                };
                let mut source = entry.source.clone();
                if source.pending_pre_checks.iter_mut().any(|pending| {
                    pending
                        .rejoin_source_authority(source.typing.typed(), authority.clone())
                        .is_err()
                }) {
                    continue;
                }
                entries.push(Completion { input, source });
            }
            Ok(Self {
                entries: Arc::new(Mutex::new(entries)),
            })
        }
    }
}
use authored_completion::AuthoredCompletions;
mod product_policy {
    use super::SourceContinuation;
    use crate::declaration_selection::CapturedNativeIntent;
    use crate::declaration_selection::ExecutedEnumExclusion;
    use crate::declaration_selection::ExecutedNativeBinding;
    use crate::declaration_selection::ExecutedProductSelection;
    use crate::declaration_selection::ProductDeclarationRequest;
    use crate::evaluation::native_bindings::{CapturedNativeDeclaration, NativeBindingSelection};
    use crate::evaluation::product_declarations::AdmittedProductDeclarations;
    use crate::evaluation::provider_declarations::ProviderSelection;
    use crate::source_state::opaque_representations::OpaqueRepresentationSelection;
    use behavior_exclusion_requests::AuthoredBehaviorExclusion;
    use diagnostics::Diagnostic;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use symbols::root_grants::AuthoredRootGrant;

    pub struct ProductPolicyRequest<'request> {
        pub activation: Option<SymbolHandle>,
        pub declared: &'request [Arc<ProductDeclarationRequest>],
        pub executed: &'request [ExecutedProductSelection],
        pub native_bindings: &'request [ExecutedNativeBinding],
        pub exclusions: &'request [ExecutedEnumExclusion],
    }

    pub struct AdmittedProductPolicy {
        pub providers: Vec<ProviderSelection>,
        pub representations: Vec<OpaqueRepresentationSelection>,
        pub native_bindings: Vec<NativeBindingSelection>,
        pub grants: Vec<AuthoredRootGrant>,
        pub declarations: AdmittedProductDeclarations,
        pub exclusions: Vec<AuthoredBehaviorExclusion>,
    }

    impl SourceContinuation {
        pub fn capture_package_native_bindings(
            &mut self,
            package: semantic_vocabulary::PackageKeyIdentity,
            formations: &[crate::declaration_selection::NativeBindingFormation],
            executed: &[ExecutedNativeBinding],
        ) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>>
        {
            if formations.is_empty() && executed.is_empty() {
                return Ok((Vec::new(), Vec::new()));
            }
            let source = Arc::clone(&self.input);
            let lookups = crate::source_state::product_operand::ProductSourceLookups::new(
                source.typing.typed(),
            );
            for formation in formations {
                let author = lookups
                    .authors
                    .resolve(formation.author)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                let [operand] = formation.request.operands.as_ref() else {
                    return Err(vec![Diagnostic::error(
                        "native formation lost its exact requirement operand",
                    )]);
                };
                crate::preparation::prepare_product_requirement_operand(
                    Arc::make_mut(&mut self.input).typing.typed_mut(),
                    &lookups,
                    operand,
                    author,
                    None,
                )?;
            }
            crate::evaluation::native_bindings::capture_package_intents(
                self.typed(),
                package,
                formations,
                executed,
            )
        }

        pub fn admit_product_policy(
            &mut self,
            request: ProductPolicyRequest<'_>,
        ) -> Result<AdmittedProductPolicy, Vec<Diagnostic>> {
            use crate::declaration_selection::BuildOperation;
            let formations = self.product_formation_authors(request.declared, |request| {
                matches!(
                    request.operation,
                    BuildOperation::NativeBinding
                        | BuildOperation::ServiceAudit
                        | BuildOperation::TestGroupRegistration
                        | BuildOperation::TestGroupService
                ) || super::provider_requests::has_provider_application(request)
            })?;
            let providers = self.admit_product_provider_requests(
                request.activation,
                request.declared,
                request.executed,
                &formations,
            )?;
            let declaration_formations = formations
                .iter()
                .filter(|(request, _)| {
                    matches!(
                        request.operation,
                        BuildOperation::ServiceAudit
                            | BuildOperation::TestGroupRegistration
                            | BuildOperation::TestGroupService
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            let mut native_formations = formations
                .iter()
                .filter(|(request, _)| request.operation == BuildOperation::NativeBinding)
                .peekable();
            if native_formations.peek().is_some() {
                let source = Arc::clone(&self.input);
                let lookups = crate::source_state::product_operand::ProductSourceLookups::new(
                    source.typing.typed(),
                );
                for (request, author) in native_formations {
                    let [operand] = request.operands.as_ref() else {
                        return Err(vec![Diagnostic::error(
                            "native binding lost its exact requirement operand",
                        )]);
                    };
                    crate::preparation::prepare_product_requirement_operand(
                        Arc::make_mut(&mut self.input).typing.typed_mut(),
                        &lookups,
                        operand,
                        *author,
                        None,
                    )?;
                }
            }
            let typed = self.typed();
            let representations = super::product_declarations::collect_opaque_representations(
                typed,
                request.activation,
                request.declared,
                request.executed,
            )?;
            let native_bindings = crate::evaluation::native_bindings::collect(
                typed,
                request.activation,
                request.declared,
                request.native_bindings,
                &formations,
            )?;
            let grants = super::product_declarations::collect_product_root_grants(
                typed,
                request.activation,
                request.declared,
                request.executed,
            )?;
            let declarations = crate::evaluation::product_declarations::admit(
                typed,
                request.activation,
                request.declared,
                request.executed,
                &declaration_formations,
            )?;
            crate::evaluation::product_declarations::require_supported_exclusions(
                request.declared,
                request.executed,
                request.exclusions,
            )?;
            Ok(AdmittedProductPolicy {
                providers,
                representations,
                native_bindings,
                grants,
                declarations,
                exclusions: Vec::new(),
            })
        }

        pub fn admit_captured_native_bindings(
            &mut self,
            declarations: &[CapturedNativeDeclaration],
            intents: &[CapturedNativeIntent],
        ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
            crate::evaluation::native_bindings::collect_captured(
                Arc::make_mut(&mut self.input).typing.typed_mut(),
                declarations,
                intents,
            )
        }
    }
}
mod provider_requests {
    use super::SourceContinuation;
    use crate::declaration_selection::BuildOperation;
    use crate::declaration_selection::ExecutedProductSelection;
    use crate::declaration_selection::ProductDeclarationRequest;
    use crate::declaration_selection::product_operands::ProductStaticEnvironment;
    use crate::evaluation::provider_declarations::{
        ProviderPlanningContext, ProviderSelection, collect_product_provider_selections,
    };
    use crate::source_state::product_operand::ProductApplicationDemand;
    use diagnostics::Diagnostic;
    use std::sync::Arc;
    use symbols::SymbolHandle;

    impl SourceContinuation {
        pub(super) fn admit_product_provider_requests(
            &mut self,
            activation: Option<SymbolHandle>,
            declared: &[Arc<ProductDeclarationRequest>],
            executed: &[ExecutedProductSelection],
            formations: &[(Arc<ProductDeclarationRequest>, SymbolHandle)],
        ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
            if formations
                .iter()
                .any(|(request, _)| has_provider_application(request))
                || executed
                    .iter()
                    .any(|event| has_provider_application(&event.request))
            {
                // Applying an operand mutates the consuming program, not the source
                // declarations and lexical authors that authorized this batch.
                let source = Arc::clone(&self.input);
                let lookups = crate::source_state::product_operand::ProductSourceLookups::new(
                    source.typing.typed(),
                );
                for (request, author) in formations
                    .iter()
                    .filter(|(request, _)| has_provider_application(request))
                {
                    self.prepare_provider_request(
                        &lookups,
                        request,
                        *author,
                        None,
                        ProductApplicationDemand::FormationOnly,
                    )?;
                }
                for event in executed
                    .iter()
                    .filter(|event| has_provider_application(&event.request))
                {
                    let author = lookups
                        .authors
                        .resolve(event.selecting_machine.ok_or_else(|| {
                            vec![Diagnostic::error(
                                "executed provider recipe lost its original author",
                            )]
                        })?)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    self.prepare_provider_request(
                        &lookups,
                        &event.request,
                        author,
                        event.static_environment.as_deref(),
                        ProductApplicationDemand::Selected,
                    )?;
                }
            }
            collect_product_provider_selections(
                ProviderPlanningContext::new(
                    self.typed(),
                    source_provenance::DependencyScope::Product,
                ),
                activation,
                declared,
                executed,
            )
        }

        pub(super) fn product_formation_authors(
            &self,
            declared: &[Arc<ProductDeclarationRequest>],
            selects: impl Fn(&ProductDeclarationRequest) -> bool,
        ) -> Result<Vec<(Arc<ProductDeclarationRequest>, SymbolHandle)>, Vec<Diagnostic>> {
            let typed = self.typed();
            let declared = declared
                .iter()
                .filter(|request| selects(request))
                .collect::<Vec<_>>();
            if declared.is_empty() {
                return Ok(Vec::new());
            }
            // One source roster serves this policy batch. The span narrows the
            // join; the complete request still proves lexical author custody.
            let occurrences =
                crate::source_state::product_operand::collect_product_selection_occurrences(typed);
            formation_authors(typed, &declared, &occurrences)
        }

        fn prepare_provider_request(
            &mut self,
            source: &crate::source_state::product_operand::ProductSourceLookups<'_>,
            request: &ProductDeclarationRequest,
            author: SymbolHandle,
            static_environment: Option<&ProductStaticEnvironment>,
            demand: ProductApplicationDemand,
        ) -> Result<(), Vec<Diagnostic>> {
            for (operand_index, operand) in request.operands.iter().enumerate() {
                let role = request
                    .operation
                    .product_operand_role(operand_index)
                    .ok_or_else(|| {
                        vec![Diagnostic::error("provider recipe lost its operand role")]
                    })?;
                crate::preparation::prepare_product_operand_application(
                    Arc::make_mut(&mut self.input).typing.typed_mut(),
                    source,
                    operand,
                    role,
                    author,
                    static_environment,
                    demand,
                )?;
            }
            Ok(())
        }
    }

    fn formation_authors(
        typed: &crate::source_state::TypedTrees,
        declared: &[&Arc<ProductDeclarationRequest>],
        occurrences: &[crate::source_state::product_operand::ProductSelectionOccurrence],
    ) -> Result<Vec<(Arc<ProductDeclarationRequest>, SymbolHandle)>, Vec<Diagnostic>> {
        let source_key = |span: source_text::SourceSpan| {
            let span = typed.symbols.rejoin_product_occurrence(span);
            (span.source_id.0, span.span.start, span.span.end)
        };
        let mut occurrences = occurrences
            .iter()
            .map(|occurrence| (source_key(occurrence.request.source_span), occurrence))
            .collect::<Vec<_>>();
        occurrences.sort_by_key(|(span, _)| *span);
        let mut formations = Vec::new();
        for &request in declared {
            let key = source_key(request.source_span);
            let first = occurrences.partition_point(|(span, _)| *span < key);
            let last = occurrences.partition_point(|(span, _)| *span <= key);
            let mut owners = occurrences[first..last]
                .iter()
                .filter(|(_, occurrence)| {
                    typed.same_product_request_source(&occurrence.request, request)
                })
                .map(|(_, occurrence)| typed.symbols.authored_origin(occurrence.machine));
            let owner = owners.next().ok_or_else(|| {
                vec![
                    Diagnostic::error(
                        "Product declaration recipe lost its original authored body occurrence",
                    )
                    .with_source_span(request.source_span),
                ]
            })?;
            if owners.any(|other| other != owner) {
                return Err(vec![
                    Diagnostic::error(
                        "Product declaration recipe has multiple original lexical owners",
                    )
                    .with_source_span(request.source_span),
                ]);
            }
            formations.push((Arc::clone(request), owner));
        }
        Ok(formations)
    }

    pub(super) fn has_provider_application(request: &ProductDeclarationRequest) -> bool {
        matches!(
            request.operation,
            BuildOperation::ProviderSelection | BuildOperation::BoundaryAcceptance
        ) && request
            .operands
            .iter()
            .any(|operand| operand.application.is_some())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::declaration_selection::ProductDeclarationOperand;
        use crate::source_state::product_operand::ProductSelectionOccurrence;

        #[test]
        fn formation_authors_keep_complete_requests_and_lexical_owners() {
            let mut builder = crate::source_symbols::SymbolTableBuilder::new();
            let root = builder.insert_root(
                symbols::SymbolKind::Root,
                symbols::SymbolNameRef::Static("root"),
            );
            let owners = builder.insert_children(
                root,
                ["first", "second"].map(|name| {
                    (
                        symbols::SymbolKind::Machine,
                        symbols::SymbolNameRef::Static(name),
                    )
                }),
            );
            let typed = crate::source_state::TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            let span = source_text::SourceSpan::new(
                source_text::SourceId(0),
                source_text::Span::new(4, 9),
            );
            let request = Arc::new(ProductDeclarationRequest {
                operation: BuildOperation::ServiceAudit,
                source_span: span,
                operands: vec![ProductDeclarationOperand {
                    path: "Boundary".into(),
                    source_span: span,
                    ..Default::default()
                }]
                .into(),
            });
            let occurrence = |request, machine| ProductSelectionOccurrence {
                request,
                machine,
                state: SymbolHandle::invalid(),
                expression: crate::source_state::expression::ExpressionHandle::invalid(),
            };
            let mut other = request.as_ref().clone();
            other.operands[0].path = "Other".into();
            let mut occurrences = vec![
                occurrence(Arc::new(other), owners.handle_at(1)),
                occurrence(Arc::clone(&request), owners.start()),
                occurrence(Arc::clone(&request), owners.start()),
            ];
            let authors = formation_authors(&typed, &[&request], &occurrences).unwrap();
            assert_eq!(authors.len(), 1);
            assert_eq!(authors[0].1, owners.start());
            occurrences.push(occurrence(Arc::clone(&request), owners.handle_at(1)));
            assert!(
                formation_authors(&typed, &[&request], &occurrences).unwrap_err()[0]
                    .message
                    .contains("multiple original lexical owners")
            );
            assert!(
                formation_authors(&typed, &[&request], &[]).unwrap_err()[0]
                    .message
                    .contains("lost its original authored body occurrence")
            );
        }
    }
}
pub use product_policy::{AdmittedProductPolicy, ProductPolicyRequest};
mod storage {
    use super::{SourceCompletionInput, SourceContext, SourceContinuation};
    use crate::evaluation::BuildTimeSelectionAuthority;
    use record_storage::{Record, RecordError, RecordReader, RecordWriter};
    use std::sync::Arc;

    impl Record for SourceContext {
        fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
            output.record(|output| {
                self.declaration_syntax.write(output)?;
                self.bindings.write(output)?;
                self.build_helper_identity.write(output)?;
                let captured;
                let recorded = match &self.recorded_authority {
                    Some(recorded) => recorded,
                    None => {
                        captured = crate::source_state::declaration_authority::RecordedDeclarationAuthority::capture(
                            self.authority.as_deref(),
                        )
                        .map_err(|reason| output.error(reason))?;
                        &captured
                    }
                };
                recorded.write(output)
            })
        }
        fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
            input.record(|input| {
                Ok(Self {
                    declaration_syntax: Record::read(input)?,
                    bindings: Record::read(input)?,
                    build_helper_identity: Record::read(input)?,
                    authority: None,
                    recorded_authority: Some(Record::read(input)?),
                })
            })
        }
    }
    impl SourceCompletionInput {
        pub(super) fn rejoin_current_authority(
            self: &Arc<Self>,
            authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        ) -> Result<Arc<Self>, String> {
            let Some(recorded) = &self.context.recorded_authority else {
                return Ok(self.clone());
            };
            let sources = self
                .typing
                .typed()
                .symbols
                .source_map()
                .ok_or("restored source continuation has no source context")?;
            recorded.require_current(authority.as_deref(), &sources)?;
            let mut input = self.as_ref().clone();
            for pending in &mut input.pending_pre_checks {
                pending.rejoin_source_authority(input.typing.typed(), authority.clone())?;
            }
            input.context.authority = authority;
            input.context.recorded_authority = None;
            Ok(Arc::new(input))
        }
    }
    impl SourceContinuation {
        pub fn rejoin_current_authority(
            self,
            authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        ) -> Result<Self, String> {
            if self.input.context.recorded_authority.is_none() {
                return Ok(self);
            }
            Ok(Self {
                input: self.input.rejoin_current_authority(authority.clone())?,
                completions: self.completions.rejoin_current_authority(authority)?,
            })
        }
    }
}

#[derive(Clone)]
pub struct ResolveAndTypeRequest {
    pub syntax: crate::SourceSyntax,
    pub source_items: Vec<(source_text::SourceId, crate::SourceItems)>,
    pub root_sources: Vec<source_text::SourceId>,
    pub frontends: SourceFrontends,
    pub build_helper_identity: &'static str,
    pub build_sources: std::collections::HashSet<source_text::SourceId>,
    pub build_stage: Option<(source_text::SourceId, usize)>,
    pub sources: Arc<crate::source_inputs::SourceMap>,
    pub top_level_bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}

#[derive(Clone, record_storage::Record)]
pub struct SourceContinuation {
    input: Arc<SourceCompletionInput>,
    completions: AuthoredCompletions,
}

#[derive(Clone, record_storage::Record)]
struct SourceCompletionInput {
    typing: crate::typing::SeededTypingBase,
    pending_bodies: crate::resolution::DeferredResolvedFrontier,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    context: SourceContext,
}

#[derive(Clone, record_storage::Record)]
pub struct CompletedSource {
    declaration_syntax: Arc<syntax_trees::SyntaxTrees>,
    accounting: crate::evaluation::SourceCompletionAccounting,
    typing: crate::typing::SeededTypingBase,
    pending_pre_checks: Vec<PreCheckEvaluation>,
}

pub struct SourceExtensionRequest {
    pub units: source_admission::LoadedSources,
    pub sources: Arc<crate::source_inputs::SourceMap>,
    pub first_source: source_text::SourceId,
    pub selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
}

pub struct PreparedSource {
    pub accounting: crate::evaluation::SourceCompletionAccounting,
    pub typed: crate::source_state::TypedTrees,
}

impl PreparedSource {
    pub fn admit_root_bindings(
        &self,
        executed: &[crate::declaration_selection::ExecutedRootBinding],
    ) -> Result<Vec<crate::RootBinding>, Vec<Diagnostic>> {
        product_declarations::collect_root_bindings(&self.typed, executed)
    }
}

impl ResolveAndTypeRequest {
    /// Resolve every selected source and type all bodies in this stage.
    pub fn resolve_and_type(self, observer: &mut impl PsiCompilationObserver) -> Result<PreparedSource, Vec<Diagnostic>> {
        let (source, _) = self.prepare(observer)?;
        Ok(source.complete_authored(observer)?.finish())
    }

    pub fn prepare(
        mut self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<
        (
            SourceContinuation,
            source_declarations::build_stage::BuildStage,
        ),
        Vec<Diagnostic>,
    > {
        crate::target_helpers::select_build_helpers(
            &mut self.syntax.trees,
            self.build_helper_identity,
            &self.build_sources,
        );
        // Only the requested Build stage is resolved and checked. A Product
        // request keeps the entry whole because it does not execute it.
        let build_stage = match self.build_stage {
            Some((source, stage)) => {
                crate::build_stages::select_build_stage(&mut self.syntax.trees, source, stage)?
            }
            None => source_declarations::build_stage::BuildStage::WHOLE,
        };
        let context = SourceContext {
            declaration_syntax: Arc::new(self.syntax.trees.clone()),
            bindings: self.top_level_bindings.clone(),
            authority: self.selection_authority.clone(),
            build_helper_identity: self.build_helper_identity.into(),
            recorded_authority: None,
        };
        let activation = activation::select_retained_source(&self, &context, observer)?;
        if let Some(activation) = &activation {
            activation.remove_retained_syntax(&mut self.syntax.trees, &self.source_items);
        }
        // Check the predecessor while its owner can still be borrowed. Report
        // any refusal after checking the successor to preserve diagnostic order.
        let retained_requests = activation.as_ref().map(|activation| {
            PreCheckEvaluation::require_recorded_requests(activation.typing.typed())
        });
        let retained_declaration_syntax = activation
            .as_ref()
            .map(|activation| Arc::clone(&activation.declaration_syntax));
        let (mut retained, mut pending_pre_checks) = match activation {
            Some(activation) => (Some(activation.typing), activation.pre_checks),
            None => (None, Vec::new()),
        };
        let resolved_base = retained
            .as_mut()
            .map(|typing| typing.resolved_base_for_extension())
            .transpose()?;
        let syntax = self.syntax.trees;
        let pre_check = PreCheckEvaluation::new(self.selection_authority);
        let (mut typing, mut pending_bodies) =
            if let (Some(typing), Some(base), Some(base_declarations)) =
                (retained, resolved_base, retained_declaration_syntax)
            {
                let extension = observer.record_result(PsiCompilationStage::Resolution, || {
                    crate::resolution::resolve_deferred_extension(
                        crate::resolution::ExtensionRequest {
                            base,
                            base_declarations,
                            syntax,
                            sources: self.sources,
                            top_level_bindings: self.top_level_bindings,
                        },
                    )
                })?;
                observer.record_result(PsiCompilationStage::Typing, || {
                    let (source, frontier) = extension.rebase_for_typed_continuation(
                        typing.typed().authored_declaration_selections(),
                    )?;
                    Ok::<_, Vec<Diagnostic>>((typing.append_resolved(source)?, frontier))
                })?
            } else {
                let mut frontier = {
                    observer.record_result(PsiCompilationStage::Resolution, || {
                        crate::resolution::resolve_deferred(crate::resolution::ResolutionRequest {
                            syntax,
                            sources: Some(self.sources),
                            top_level_bindings: self.top_level_bindings,
                            retained_base: None,
                        })
                    })?
                };
                loop {
                    let selected = frontier.pending_machines_in(
                        crate::resolution::DeferredBodyScope::BuildAndToolchain,
                    )?;
                    if selected.is_empty() {
                        break;
                    }
                    frontier = observer
                        .record_result(PsiCompilationStage::Resolution, || {
                            frontier.complete_machine_bodies(&selected)
                        })?
                        .into_resolved_frontier();
                }
                let typing = {
                    observer.record_result(PsiCompilationStage::Typing, || {
                        crate::typing::lower_symbol_resolved_trees_to_seeded_base(
                            frontier.shared_trees(),
                        )
                        .map_err(|diagnostic| vec![diagnostic])
                    })?
                };
                (typing, frontier)
            };
        loop {
            let build_bodies = pending_bodies
                .pending_machines_in(crate::resolution::DeferredBodyScope::BuildAndToolchain)?;
            if build_bodies.is_empty() {
                break;
            }
            pending_bodies =
                pending_bodies.with_prepared_allocations(typing.prepare_allocations()?)?;
            let completed = observer.record_result(PsiCompilationStage::Resolution, || {
                pending_bodies.complete_machine_bodies(&build_bodies)
            })?;
            (typing, pending_bodies) = observer
                .record_result(PsiCompilationStage::Typing, || {
                    type_completed_bodies(typing, completed)
                })?;
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
        self.frontends.retain(Arc::clone(&input))?;
        Ok((
            SourceContinuation {
                input,
                completions: Default::default(),
            },
            build_stage,
        ))
    }
}

impl SourceContinuation {
    fn complete_selected(
        input: Arc<SourceCompletionInput>,
        sources: Arc<crate::source_inputs::SourceMap>,
        bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<(crate::typing::SeededTypingBase, Vec<PreCheckEvaluation>), Vec<Diagnostic>> {
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
            crate::resolution::DeferredBodyScope::SelectedFiles,
            observer,
        )?;
        Ok((typing, input.pending_pre_checks.clone()))
    }

    /// Rejoin a Build query's authored locations to this admitted declaration
    /// frontier. Source correspondence is resolved by the owner of both states.
    pub fn rejoin_product_query_origin(
        &self,
        authored: &Self,
        occurrence: source_text::SourceSpan,
        selecting: Option<source_text::SourceSpan>,
    ) -> Result<(source_text::SourceSpan, Option<source_text::SourceSpan>), String> {
        let author = authored
            .typed()
            .symbols
            .source_file(occurrence)
            .ok_or("Product query lost its authored source occurrence")?;
        let source_map = self
            .typed()
            .symbols
            .source_map()
            .ok_or("Product query has no source custody")?;
        let selected = source_map
            .rejoined_source_id(author)
            .ok_or("Product query author lost its admitted source correspondence")?;
        let occurrence = source_text::SourceSpan::new(selected, occurrence.span);
        let selecting = selecting
            .map(|span| {
                let author = authored
                    .typed()
                    .symbols
                    .source_file(span)
                    .ok_or("declaration query lost its selecting source")?;
                let selected = source_map
                    .rejoined_source_id(author)
                    .ok_or("declaration query lost its selecting source correspondence")?;
                Ok::<_, String>(source_text::SourceSpan::new(selected, span.span))
            })
            .transpose()?;
        Ok((occurrence, selecting))
    }

    pub fn describe_parameter(
        &self,
        occurrence: source_text::SourceSpan,
        selecting: source_text::SourceSpan,
        path: &[u8],
    ) -> Result<semantic_vocabulary::ParameterDescription, String> {
        product_declarations::describe_parameter(self.typed(), occurrence, selecting, path)
    }

    pub fn describe_field(
        &self,
        occurrence: source_text::SourceSpan,
        selecting: source_text::SourceSpan,
        path: &[u8],
    ) -> Result<semantic_vocabulary::FieldInfoDescription, String> {
        product_declarations::describe_field(self.typed(), occurrence, selecting, path)
    }

    pub fn describe_schema(
        &self,
        occurrence: source_text::SourceSpan,
        path: &[u8],
    ) -> Result<crate::declaration_selection::DescribedProductSchema, String> {
        product_declarations::describe_schema(self.typed(), occurrence, path)
    }

    pub fn describe_provider(
        &self,
        occurrence: source_text::SourceSpan,
        path: &[u8],
    ) -> Result<crate::declaration_selection::DescribedProductProvider, String> {
        product_declarations::describe_provider(self.typed(), occurrence, path)
    }

    pub fn describe_entry(
        &self,
        occurrence: source_text::SourceSpan,
        path: &[u8],
        slot: &[u8],
        slots: &dyn semantic_vocabulary::ProgramEntrySourceCatalog,
    ) -> Result<crate::declaration_selection::DescribedProductEntry, String> {
        product_declarations::describe_entry(self.typed(), occurrence, path, slot, slots)
    }

    pub fn same_completion_input(&self, other: &Self) -> bool {
        self.input.same_completion_input(&other.input)
    }

    pub fn typed(&self) -> &crate::source_state::TypedTrees {
        self.input.typing.typed()
    }

    pub fn rejoin_machine(
        &self,
        previous: &Self,
        selected: symbols::SymbolHandle,
    ) -> Result<symbols::SymbolHandle, Vec<Diagnostic>> {
        let refuse = |reason| vec![Diagnostic::error(reason)];
        let declaration =
            crate::source_symbols::retain_declaration(&previous.typed().symbols, selected)
                .map_err(refuse)?;
        crate::source_symbols::rejoin_declaration(
            &declaration,
            &self.typed().symbols,
            self.typed().machines().iter().map(|machine| machine.symbol),
        )
        .map_err(refuse)
    }

    pub fn rejoin_product_provider_selection(
        &mut self,
        selection: &crate::evaluation::provider_declarations::RetainedProviderSelection,
        candidates: &mut crate::source_symbols::SourceSpanCandidates,
    ) -> Result<crate::evaluation::provider_declarations::ProviderSelection, Diagnostic> {
        selection.rejoin(
            Arc::make_mut(&mut self.input).typing.typed_mut(),
            source_provenance::DependencyScope::Product,
            &[],
            candidates,
        )
    }

    pub fn materialize_authored_bodies(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Self, Vec<Diagnostic>> {
        let Self { input, completions } = self;
        let SourceCompletionInput {
            typing,
            pending_bodies,
            pending_pre_checks,
            context,
        } = Arc::unwrap_or_clone(input);
        let (typing, pending_bodies) = complete_pending_bodies(
            typing,
            pending_bodies,
            crate::resolution::DeferredBodyScope::All,
            observer,
        )?;
        Ok(Self {
            input: Arc::new(SourceCompletionInput {
                typing,
                pending_bodies,
                pending_pre_checks,
                context,
            }),
            completions,
        })
    }

    pub fn complete_authored(
        self,
        observer: &mut impl PsiCompilationObserver,
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
            context,
        } = Arc::unwrap_or_clone(owned_input);
        (typing, pending_bodies) = complete_pending_bodies(
            typing,
            pending_bodies,
            crate::resolution::DeferredBodyScope::All,
            observer,
        )?;
        drop(pending_bodies);
        PreCheckEvaluation::record_continuations(pending_pre_checks, typing.typed_mut())?;
        let source = CompletedSource {
            declaration_syntax: context.declaration_syntax,
            typing,
            pending_pre_checks: Vec::new(),
            accounting: Default::default(),
        };
        completions.retain(input, &source)?;
        Ok(source)
    }
}

// Resolution retains pending syntax; typing receives only the completed trees.
// The driver joins their exact checkpoints before either stage resumes.
fn type_completed_bodies(
    typing: crate::typing::SeededTypingBase,
    completed: crate::resolution::ResolvedBodyCompletion,
) -> Result<
    (
        crate::typing::SeededTypingBase,
        crate::resolution::DeferredResolvedFrontier,
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

fn complete_pending_bodies(
    mut typing: crate::typing::SeededTypingBase,
    mut pending_bodies: crate::resolution::DeferredResolvedFrontier,
    scope: crate::resolution::DeferredBodyScope,
    observer: &mut impl PsiCompilationObserver,
) -> Result<
    (
        crate::typing::SeededTypingBase,
        crate::resolution::DeferredResolvedFrontier,
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
            observer.record_result(PsiCompilationStage::Resolution, || {
                pending_bodies.complete_machine_bodies(&selected)
            })?
        };
        (typing, pending_bodies) = {
            observer.record_result(PsiCompilationStage::Typing, || {
                type_completed_bodies(typing, resolved)
            })?
        };
    }
    Ok((typing, pending_bodies))
}

impl CompletedSource {
    pub fn authored_program(&self) -> &crate::source_state::TypedTrees {
        self.typing.typed()
    }

    pub fn finish(self) -> PreparedSource {
        PreparedSource {
            accounting: self.accounting,
            typed: self.typing.into_typed(),
        }
    }

    pub fn extend(
        mut self,
        extension: SourceExtensionRequest,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<PreparedSource, Vec<Diagnostic>> {
        // Parse admitted generated bytes before touching the retained typed
        // frontier, preserving the extension's original error precedence.
        let units = crate::source_parsing::SourceInput(extension.units).into_units(observer)?;
        let wire_schema_frontier = self.typing.typed().wire_schemas().len();
        let resolved_base = self.typing.resolved_base_for_extension()?;
        let pre_checks =
            vec![PreCheckEvaluation::new(Some(extension.selection_authority)); units.len()];
        let mut units = units.into_iter();
        let mut syntax = units
            .next()
            .unwrap_or_else(|| syntax_trees::SyntaxTrees::new(extension.first_source));
        for unit in units {
            syntax.adopt_page(unit);
        }
        let generation = u32::try_from(syntax.page_count())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| vec![Diagnostic::error("syntax page identity overflow")])?;
        syntax.begin_page(generation);
        let seeded = observer.record_result(PsiCompilationStage::Resolution, || {
            crate::resolution::resolve_extension(crate::resolution::ExtensionRequest {
                base: resolved_base,
                base_declarations: Arc::clone(&self.declaration_syntax),
                syntax,
                sources: extension.sources,
                top_level_bindings: Vec::new(),
            })
        })?;
        let mut typed = observer
            .record_result(PsiCompilationStage::Typing, || {
                self.typing.lower_extension(seeded)
            })?
            .into_typed();
        PreCheckEvaluation::record_extension(pre_checks, wire_schema_frontier, &mut typed)?;
        Ok(PreparedSource {
            accounting: self.accounting,
            typed,
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

#[cfg(test)]
mod resolution_handoff_tests {
    use super::*;

    fn deferred(
        text: &str,
    ) -> (
        crate::typing::SeededTypingBase,
        crate::resolution::DeferredResolvedFrontier,
        Arc<crate::source_inputs::SourceMap>,
    ) {
        let mut sources = crate::source_inputs::SourceMap::default();
        let id = sources
            .add("completion/main.omg".into(), text.to_owned())
            .source_id;
        let sources = Arc::new(sources);
        let tokens = crate::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let frontier = crate::resolution::pre_resolution::resolve_deferred_numeric_probe(
            crate::resolution::ResolutionRequest {
                syntax,
                sources: Some(Arc::clone(&sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            },
        )
        .unwrap();
        let typing =
            crate::typing::lower_symbol_resolved_trees_to_seeded_base(frontier.shared_trees())
                .unwrap();
        (typing, frontier, sources)
    }

    #[test]
    fn resolution_handoff_completes_each_batch_without_changing_its_predecessor() {
        let (mut typing, mut frontier, sources) = deferred(
            "machine first(value: u64) -> u64 { value } machine second(value: u64) -> u64 { value }",
        );
        frontier = frontier
            .with_prepared_allocations(typing.prepare_allocations().unwrap())
            .unwrap();
        let (activated, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
        frontier = activated;
        typing = typing.activate_body_completion(activation).unwrap();
        let root = frontier.trees().symbols.root();
        assert!(std::ptr::eq(
            typing.typed().symbols.get(root),
            frontier.trees().symbols.get(root),
        ));
        let pending: Vec<_> = frontier.pending_machines().collect();
        assert_eq!(pending.len(), 2);
        for selected in pending {
            frontier = frontier
                .with_prepared_allocations(typing.prepare_allocations().unwrap())
                .unwrap();
            let predecessor = typing.shared_resolved();
            let snapshot = predecessor.as_ref().clone();
            let completed = frontier.complete_machine_bodies(&[selected]).unwrap();
            (typing, frontier) = type_completed_bodies(typing, completed).unwrap();
            assert_eq!(predecessor.as_ref(), &snapshot);
            assert_eq!(typing.resolved(), frontier.trees());
        }
        assert_eq!(frontier.pending_machines().count(), 0);
        assert_eq!(typing.typed().machines.len(), 2);
    }

    #[test]
    fn nonempty_selected_completion_retains_prior_refusal_rows() {
        use crate::source_state::deferred_refusals::DeferredRequest;

        let (mut typing, frontier, sources) = deferred(
            "machine length() -> u64 { 4 } \
             machine first() { let bytes: [u8; length()]; } \
             machine second() { let bytes: [u8; length()]; }",
        );
        let frontier = frontier
            .with_prepared_allocations(typing.prepare_allocations().unwrap())
            .unwrap();
        let (mut frontier, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
        typing = typing.activate_body_completion(activation).unwrap();
        let owner = |name: &str| {
            frontier
                .pending_machines()
                .find(|symbol| frontier.trees().symbols.name(*symbol) == name)
                .unwrap()
        };
        let first = owner("first");
        let second = owner("second");
        for selected in [first, second] {
            frontier = frontier
                .with_prepared_allocations(typing.prepare_allocations().unwrap())
                .unwrap();
            let predecessor = typing.clone();
            let completed = frontier.complete_machine_bodies(&[selected]).unwrap();
            (typing, frontier) = type_completed_bodies(typing, completed).unwrap();
            assert!(predecessor.typed().deferred_refusal(selected).is_none());
            let introduced: Vec<_> = typing.typed().deferred_refusals_of(selected).collect();
            assert!(introduced.iter().all(|row| row.declaration == selected));
            assert_eq!(
                introduced.iter().map(|row| row.request).collect::<Vec<_>>(),
                [
                    DeferredRequest::TypeExpressionEvaluation,
                    DeferredRequest::ArrayExtentEvaluation
                ]
            );
            PreCheckEvaluation::require_recorded_requests(typing.typed()).unwrap();
            for retained in &predecessor.typed().deferred_refusals {
                let successor = typing
                    .typed()
                    .deferred_refusals_of(retained.declaration)
                    .find(|row| row.request == retained.request)
                    .unwrap();
                assert!(std::ptr::eq(retained, successor));
            }
            assert_eq!(typing.resolved(), frontier.trees());
        }
        assert_eq!(typing.typed().deferred_refusals.len(), 4);
        assert_eq!(frontier.pending_machines().count(), 1);
    }

    #[test]
    fn resolution_handoff_refuses_cross_paired_completion_and_activation() {
        let (typing, _, _) = deferred("machine first() -> u64 { 1 }");
        let (_, frontier, sources) = deferred("machine second() -> u64 { 2 }");
        let selected: Vec<_> = frontier.pending_machines().collect();
        let completed = frontier.clone().complete_machine_bodies(&selected).unwrap();
        let error = type_completed_bodies(typing.clone(), completed).unwrap_err();
        assert!(
            error.iter().any(|diagnostic| diagnostic
                .message
                .contains("exact retained resolved predecessor")),
            "{error:?}"
        );
        let (_, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
        let error = typing.activate_body_completion(activation).unwrap_err();
        assert!(
            error.iter().any(|diagnostic| diagnostic
                .message
                .contains("exact retained resolved predecessor")),
            "{error:?}"
        );
    }

    #[test]
    fn completed_body_work_is_recorded_before_retained_source_activation() {
        use crate::source_state::deferred_refusals::DeferredRequest;

        let text = "machine length() -> u64 { 4 }\n\
                    machine helper() { let bytes: [u8; length()]; }\n";
        let (mut typing, frontier, sources) = deferred(text);
        let pending = PreCheckEvaluation::new(None);
        PreCheckEvaluation::defer_declared_requests(typing.typed_mut()).unwrap();
        let frontier = frontier
            .with_prepared_allocations(typing.prepare_allocations().unwrap())
            .unwrap();
        let (frontier, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
        typing = typing.activate_body_completion(activation).unwrap();
        let frontier = frontier
            .with_prepared_allocations(typing.prepare_allocations().unwrap())
            .unwrap();
        let helper = frontier
            .pending_machines()
            .find(|symbol| frontier.trees().symbols.name(*symbol) == "helper")
            .unwrap();
        assert!(typing.typed().deferred_refusal(helper).is_none());
        let completed = frontier.complete_machine_bodies(&[helper]).unwrap();
        let (typing, _) = type_completed_bodies(typing, completed).unwrap();

        pending
            .activate_sources(typing.typed())
            .expect("new declaration-owned work is recorded before source activation");
        assert!(
            typing
                .typed()
                .deferred_refusals_of(helper)
                .any(|row| row.request == DeferredRequest::ArrayExtentEvaluation)
        );
        let unserved = typing.typed().deferred_refusal(helper).unwrap().request;
        let (program, _) = crate::preparation::prepare_typed_trees(
            typing.typed().clone(),
            &crate::preparation::PreparationRequest::settled(),
        )
        .unwrap();
        let refused = crate::AcceptedTerminalSource::judge(
            Arc::new(program),
            crate::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[helper],
                subjects: &[],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect_err("recording the request cannot admit the helper that needs its evaluation");
        assert!(
            refused
                .iter()
                .any(|diagnostic| diagnostic.message.contains(unserved.construct())),
            "{refused:?}"
        );
    }
}
