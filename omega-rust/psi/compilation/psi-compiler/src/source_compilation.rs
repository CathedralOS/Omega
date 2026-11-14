/*
    Resolver and typing continuation across admitted Build execution, generated
    source extension, and post-join Product policy admission.

    We pair the resolver frontier with its seeded typing base inside
    `SourceContinuation`, complete authored bodies into `CompletedSource`,
    extend completed sources with admitted generated units, and admit Product
    provider requests, root grants, declarations, and behavior exclusions.
*/

use crate::{PsiCompilationObserver, PsiCompilationStage};
use build_time_evaluation::{BuildTimeSelectionAuthority, PreCheckEvaluation};
use diagnostics::Diagnostic;
use std::sync::Arc;

mod activation {
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
        pub declaration_syntax: Arc<SyntaxTrees>,
        pub bindings: Vec<SourceScopedTopLevelBinding>,
        pub authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        pub build_helper_identity: std::borrow::Cow<'static, str>,
        pub recorded_authority:
            Option<typed_trees::declaration_authority::RecordedDeclarationAuthority>,
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
                    typed_trees::declaration_authority::RecordedDeclarationAuthority::has_authority,
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
                machine.body_status == typed_trees::machine::MachineBodyStatus::Deferred
            }) {
                return Err(vec![Diagnostic::error(
                    "selected retained source still has a deferred authored body",
                )]);
            }
            let references = typed_trees::references::collect_program_references(typing.typed());
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

    #[derive(Clone, Default, compiler_storage::Record)]
    pub(super) struct AuthoredCompletions {
        entries: Arc<Mutex<Vec<Completion>>>,
    }

    #[derive(compiler_storage::Record)]
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
            authority: Option<Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>>,
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
    use crate::source_checking::ProductRepresentationInput;
    use build_time_evaluation::native_bindings::{
        CapturedNativeDeclaration, NativeBindingSelection,
    };
    use build_time_evaluation::product_declarations::AdmittedProductDeclarations;
    use build_time_evaluation::provider_declarations::ProviderSelection;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredBehaviorExclusion, AuthoredRootGrant, CapturedNativeIntent, ExecutedEnumExclusion,
        ExecutedNativeBinding, ExecutedProductSelection, ProductDeclarationRequest,
    };
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::opaque_representations::OpaqueRepresentationSelection;

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
            formations: &[language_semantics::declaration_selection::NativeBindingFormation],
            executed: &[ExecutedNativeBinding],
        ) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>>
        {
            for formation in formations {
                let author = typed_trees::product_operand::resolve_build_request_author(
                    self.typed(),
                    formation.author,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
                let [operand] = formation.request.operands.as_ref() else {
                    return Err(vec![Diagnostic::error(
                        "native formation lost its exact requirement operand",
                    )]);
                };
                self.prepare_product_requirement_operand(operand, author, None)?;
            }
            build_time_evaluation::native_bindings::capture_package_intents(
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
            ProductRepresentationInput {
                activation: request.activation,
                declared: request.declared,
                executed: request.executed,
                retained: &[],
            }
            .require_none_requested()?;
            let providers = self.admit_product_provider_requests(
                request.activation,
                request.declared,
                request.executed,
            )?;
            let formations = self.product_formation_authors(request.declared, |request| {
                request.operation
                    == language_semantics::declaration_selection::BuildOperation::NativeBinding
            })?;
            for (request, author) in formations {
                let [operand] = request.operands.as_ref() else {
                    return Err(vec![Diagnostic::error(
                        "native binding lost its exact requirement operand",
                    )]);
                };
                self.prepare_product_requirement_operand(operand, author, None)?;
            }
            let typed = self.typed();
            let representations = Vec::new();
            let native_bindings = build_time_evaluation::native_bindings::collect(
                typed,
                request.activation,
                request.declared,
                request.native_bindings,
            )?;
            let grants = build_time_evaluation::product_policy::collect_product_root_grants(
                typed,
                request.activation,
                request.declared,
                request.executed,
            )?;
            let declarations = build_time_evaluation::product_declarations::admit(
                typed,
                request.activation,
                request.declared,
                request.executed,
            )?;
            let exclusions = build_time_evaluation::product_policy::collect_behavior_exclusions(
                typed,
                request.activation,
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
                exclusions,
            })
        }

        pub fn admit_captured_native_bindings(
            &mut self,
            declarations: &[CapturedNativeDeclaration],
            intents: &[CapturedNativeIntent],
        ) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
            build_time_evaluation::native_bindings::collect_captured(
                Arc::make_mut(&mut self.input).typing.typed_mut(),
                declarations,
                intents,
            )
        }
    }
}
mod provider_requests {
    use super::SourceContinuation;
    use build_time_evaluation::provider_declarations::{
        ProviderPlanningContext, ProviderSelection, collect_product_provider_selections,
    };
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
    use language_semantics::declaration_selection::{
        BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
    };
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::product_operand::ProductApplicationDemand;

    impl SourceContinuation {
        pub(super) fn admit_product_provider_requests(
            &mut self,
            activation: Option<SymbolHandle>,
            declared: &[Arc<ProductDeclarationRequest>],
            executed: &[ExecutedProductSelection],
        ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
            let formations = self.product_formation_authors(declared, has_provider_application)?;
            for (request, author) in formations {
                self.prepare_provider_request(
                    &request,
                    author,
                    None,
                    ProductApplicationDemand::FormationOnly,
                )?;
            }
            for event in executed
                .iter()
                .filter(|event| has_provider_application(&event.request))
            {
                let author = typed_trees::product_operand::resolve_build_request_author(
                    self.typed(),
                    event.selecting_machine.ok_or_else(|| {
                        vec![Diagnostic::error(
                            "executed provider recipe lost its original author",
                        )]
                    })?,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
                self.prepare_provider_request(
                    &event.request,
                    author,
                    event.static_environment.as_deref(),
                    ProductApplicationDemand::Selected,
                )?;
            }
            collect_product_provider_selections(
                ProviderPlanningContext::new(self.typed(), source::DependencyScope::Product),
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
            let occurrences =
                typed_trees::product_operand::collect_product_selection_occurrences(typed);
            let mut formations = Vec::new();
            for request in declared.iter().filter(|request| selects(request)) {
                let mut owners = occurrences
                    .iter()
                    .filter(|occurrence| {
                        typed.same_product_request_source(&occurrence.request, request)
                    })
                    .map(|occurrence| typed.symbols.authored_origin(occurrence.machine));
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

        fn prepare_provider_request(
            &mut self,
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
                typed_trees_to_typed_trees::prepare_product_operand_application(
                    Arc::make_mut(&mut self.input).typing.typed_mut(),
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

    fn has_provider_application(request: &ProductDeclarationRequest) -> bool {
        request.operation == BuildOperation::ProviderSelection
            && request
                .operands
                .iter()
                .any(|operand| operand.application.is_some())
    }
}
pub use product_policy::{AdmittedProductPolicy, ProductPolicyRequest};
mod storage {
    use super::{SourceCompletionInput, SourceContext, SourceContinuation};
    use build_time_evaluation::BuildTimeSelectionAuthority;
    use compiler_storage::{Record, RecordError, RecordReader, RecordWriter};
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
                        captured = typed_trees::declaration_authority::RecordedDeclarationAuthority::capture(
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
pub struct SourceCompilationRequest {
    pub syntax: syntax_trees::SyntaxTrees,
    pub source_items: Vec<(source::SourceId, Vec<syntax_trees::item::ItemHandle>)>,
    pub root_sources: Vec<source::SourceId>,
    pub frontends: SourceFrontends,
    pub build_helper_identity: &'static str,
    pub sources: Arc<source::SourceMap>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}

#[derive(Clone, compiler_storage::Record)]
pub struct SourceContinuation {
    input: Arc<SourceCompletionInput>,
    completions: AuthoredCompletions,
}

#[derive(Clone, compiler_storage::Record)]
struct SourceCompletionInput {
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_bodies: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    pending_pre_checks: Vec<PreCheckEvaluation>,
    context: SourceContext,
}

pub struct BuildSourceInput<'source> {
    pub typed: &'source mut typed_trees::TypedTrees,
    pub pre_checks: &'source mut Vec<PreCheckEvaluation>,
}

#[derive(Clone, compiler_storage::Record)]
pub struct CompletedSource {
    declaration_syntax: Arc<syntax_trees::SyntaxTrees>,
    accounting: build_time_evaluation::SourceCompletionAccounting,
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pending_pre_checks: Vec<PreCheckEvaluation>,
}

pub struct SourceExtensionRequest {
    pub units: Vec<syntax_trees::SyntaxTrees>,
    pub sources: Arc<source::SourceMap>,
    pub first_source: source::SourceId,
    pub selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
}

pub struct PreparedSource {
    pub accounting: build_time_evaluation::SourceCompletionAccounting,
    pub typed: typed_trees::TypedTrees,
}

impl PreparedSource {
    pub fn admit_root_bindings(
        &self,
        executed: &[language_semantics::declaration_selection::ExecutedRootBinding],
    ) -> Result<Vec<language_semantics::declaration_selection::RootBinding>, Vec<Diagnostic>> {
        crate::root_bindings::collect_root_bindings(&self.typed, executed)
    }
}

impl SourceCompilationRequest {
    pub fn prepare(
        mut self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<SourceContinuation, Vec<Diagnostic>> {
        let context = SourceContext {
            declaration_syntax: Arc::new(self.syntax.clone()),
            bindings: self.top_level_bindings.clone(),
            authority: self.selection_authority.clone(),
            build_helper_identity: self.build_helper_identity.into(),
            recorded_authority: None,
        };
        let activation = activation::select_retained_source(&self, &context, observer)?;
        if let Some(activation) = &activation {
            activation.remove_retained_syntax(&mut self.syntax, &self.source_items);
        }
        let retained_program = activation
            .as_ref()
            .map(|activation| activation.typing.typed().clone());
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
        let evaluated = build_time_evaluation::evaluate_pre_resolution(
            build_time_evaluation::BuildTimeEvaluationRequest {
                syntax_trees: self.syntax,
                source_context: build_time_evaluation::BuildTimeSources {
                    sources: Some(Arc::clone(&self.sources)),
                    source_scoped_top_level_bindings: &self.top_level_bindings,
                    selection_authority: self.selection_authority,
                    predecessor: retained.clone(),
                    declaration_syntax: Some(&context.declaration_syntax),
                    retained_declaration_syntax: retained_declaration_syntax.as_deref(),
                },
            },
        )?;
        let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
        let (mut typing, mut pending_bodies) = if let (Some(typing), Some(base)) =
            (retained, resolved_base)
        {
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
                let (source, frontier) = extension.rebase_for_typed_continuation(
                    typing.typed().authored_declaration_selections(),
                )?;
                Ok::<_, Vec<Diagnostic>>((typing.append_resolved(source)?, frontier))
            })?
        } else {
            let mut frontier = {
                observer.record_result(PsiCompilationStage::Resolution, || {
                    syntax_trees_to_symbol_resolved_trees::resolve_deferred(
                        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                            syntax,
                            sources: Some(self.sources),
                            top_level_bindings: self.top_level_bindings,
                            retained_base: None,
                        },
                    )
                })?
            };
            loop {
                let selected = build_bodies(&frontier);
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
                        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
                            frontier.shared_trees(),
                        ).map_err(|diagnostic| vec![diagnostic])
                    })?
            };
            (typing, frontier)
        };
        loop {
            let build_bodies = build_bodies(&pending_bodies);
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
        let pre_check = match retained_program {
            Some(previous) => pre_check.exclude_completed_work(typing.typed(), &previous)?,
            None => pre_check,
        };
        pending_pre_checks.push(pre_check);
        let input = Arc::new(SourceCompletionInput {
            typing,
            pending_bodies,
            pending_pre_checks,
            context,
        });
        self.frontends.retain(Arc::clone(&input))?;
        Ok(SourceContinuation {
            input,
            completions: Default::default(),
        })
    }
}

impl SourceContinuation {
    fn complete_selected(
        input: Arc<SourceCompletionInput>,
        sources: Arc<source::SourceMap>,
        bindings: Vec<symbols::SourceScopedTopLevelBinding>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<
        (
            symbol_resolved_trees_to_typed_trees::SeededTypingBase,
            Vec<PreCheckEvaluation>,
        ),
        Vec<Diagnostic>,
    > {
        let mut typing = input.typing.clone();
        let pending_bodies = input
            .pending_bodies
            .clone()
            .with_prepared_allocations(typing.prepare_allocations()?)?;
        let (mut pending_bodies, activation) =
            pending_bodies.activate_sources(Arc::clone(&sources), bindings)?;
        typing = typing.activate_body_completion(activation)?;
        loop {
            pending_bodies =
                pending_bodies.with_prepared_allocations(typing.prepare_allocations()?)?;
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
                    type_completed_bodies(typing, resolved)
                })?;
        }
        Ok((typing, input.pending_pre_checks.clone()))
    }

    pub fn describe_schema(
        &self,
        _occurrence: source::SourceSpan,
        _path: &[u8],
    ) -> Result<language_semantics::declaration_selection::DescribedProductSchema, String> {
        Err("no product route yet: product declaration description".into())
    }

    pub fn describe_provider(
        &self,
        _occurrence: source::SourceSpan,
        _path: &[u8],
    ) -> Result<language_semantics::declaration_selection::DescribedProductProvider, String> {
        Err("no product route yet: product declaration description".into())
    }

    pub fn describe_entry(
        &self,
        _occurrence: source::SourceSpan,
        _path: &[u8],
        _slot: &[u8],
        _slots: &dyn semantic_vocabulary::ProgramEntrySourceCatalog,
    ) -> Result<language_semantics::declaration_selection::DescribedProductEntry, String> {
        Err("no product route yet: product declaration description".into())
    }

    pub fn same_completion_input(&self, other: &Self) -> bool {
        self.input.same_completion_input(&other.input)
    }

    pub fn typed(&self) -> &typed_trees::TypedTrees {
        self.input.typing.typed()
    }

    pub fn rejoin_machine(
        &self,
        previous: &Self,
        selected: symbols::SymbolHandle,
    ) -> Result<symbols::SymbolHandle, Vec<Diagnostic>> {
        let refuse = |reason| vec![Diagnostic::error(reason)];
        let declaration =
            symbols::RetainedDeclaration::capture(&previous.typed().symbols, selected)
                .map_err(refuse)?;
        declaration
            .rejoin(
                &self.typed().symbols,
                self.typed().machines().iter().map(|machine| machine.symbol),
            )
            .map_err(refuse)
    }

    pub fn prepare_product_requirement_operand(
        &mut self,
        operand: &language_semantics::declaration_selection::ProductDeclarationOperand,
        author: symbols::SymbolHandle,
        static_environment: Option<
            &language_semantics::declaration_selection::product_operands::ProductStaticEnvironment,
        >,
    ) -> Result<typed_trees_to_typed_trees::PreparedProductRequirement, Vec<Diagnostic>> {
        typed_trees_to_typed_trees::prepare_product_requirement_operand(
            Arc::make_mut(&mut self.input)
                .typing
                .typed_mut()
                .product_declarations_mut(),
            operand,
            author,
            static_environment,
        )
    }

    pub fn rejoin_product_provider_selection(
        &mut self,
        selection: &build_time_evaluation::provider_declarations::RetainedProviderSelection,
        candidates: &mut symbols::SourceSpanCandidates,
    ) -> Result<build_time_evaluation::provider_declarations::ProviderSelection, Diagnostic> {
        selection.rejoin(
            Arc::make_mut(&mut self.input).typing.typed_mut(),
            source::DependencyScope::Product,
            &[],
            candidates,
        )
    }

    pub fn build_input(&mut self) -> BuildSourceInput<'_> {
        let input = Arc::make_mut(&mut self.input);
        BuildSourceInput {
            typed: input.typing.typed_mut(),
            pre_checks: &mut input.pending_pre_checks,
        }
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
        let (typing, pending_bodies) = complete_authored_bodies(typing, pending_bodies, observer)?;
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
        (typing, pending_bodies) = complete_authored_bodies(typing, pending_bodies, observer)?;
        drop(pending_bodies);
        let mut deferred = Vec::new();
        let mut accounting = build_time_evaluation::SourceCompletionAccounting::default();
        for pre_check in pending_pre_checks {
            let recorded = pre_check.evaluate_or_defer_recorded(typing.typed_mut())?;
            accounting.append(recorded.accounting);
            if let Some(pending) = recorded.pending {
                deferred.push(pending);
            }
        }
        let source = CompletedSource {
            declaration_syntax: context.declaration_syntax,
            typing,
            pending_pre_checks: deferred,
            accounting,
        };
        completions.retain(input, &source)?;
        Ok(source)
    }
}

// Resolution retains pending syntax; typing receives only the completed trees.
// The driver joins their exact checkpoints before either stage resumes.
fn type_completed_bodies(
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    completed: syntax_trees_to_symbol_resolved_trees::ResolvedBodyCompletion,
) -> Result<
    (
        symbol_resolved_trees_to_typed_trees::SeededTypingBase,
        syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
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

fn complete_authored_bodies(
    mut typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    mut pending_bodies: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    observer: &mut impl PsiCompilationObserver,
) -> Result<
    (
        symbol_resolved_trees_to_typed_trees::SeededTypingBase,
        syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    ),
    Vec<Diagnostic>,
> {
    loop {
        pending_bodies = pending_bodies.with_prepared_allocations(typing.prepare_allocations()?)?;
        let selected = pending_bodies.pending_machines().collect::<Vec<_>>();
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
    pub fn authored_program(&self) -> &typed_trees::TypedTrees {
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
        let wire_schema_frontier = self.typing.typed().wire_schemas().len();
        let resolved_base = self.typing.resolved_base_for_extension()?;
        let (syntax, pre_checks) =
            build_time_evaluation::PreResolutionEvaluation::evaluate_extension(
                extension.units,
                extension.first_source,
                build_time_evaluation::BuildTimeSources {
                    sources: Some(Arc::clone(&extension.sources)),
                    source_scoped_top_level_bindings: &[],
                    selection_authority: Some(extension.selection_authority),
                    predecessor: Some(self.typing.clone()),
                    declaration_syntax: Some(&self.declaration_syntax),
                    retained_declaration_syntax: None,
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
        let mut typed = observer
            .record_result(PsiCompilationStage::Typing, || {
                self.typing.lower_extension(seeded)
            })?
            .into_typed();
        PreCheckEvaluation::evaluate_extension_or_defer(
            pre_checks,
            wire_schema_frontier,
            &mut typed,
        )?;
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

fn build_bodies(
    frontier: &syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
) -> Vec<symbols::SymbolHandle> {
    frontier
        .pending_machines()
        .filter(|symbol| {
            frontier
                .trees()
                .symbols
                .symbol_provenance_source_span(*symbol)
                .and_then(|span| frontier.trees().symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        || file.dependency_scope == source::DependencyScope::Build
                })
        })
        .collect()
}

#[cfg(test)]
mod resolution_handoff_tests {
    use super::*;

    fn deferred(
        text: &str,
    ) -> (
        symbol_resolved_trees_to_typed_trees::SeededTypingBase,
        syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
        Arc<source::SourceMap>,
    ) {
        let mut sources = source::SourceMap::default();
        let id = sources
            .add("completion/main.omg".into(), text.to_owned())
            .source_id;
        let sources = Arc::new(sources);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let frontier =
            syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_deferred_numeric_probe(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                    syntax,
                    sources: Some(Arc::clone(&sources)),
                    top_level_bindings: Vec::new(),
                    retained_base: None,
                },
            )
            .unwrap();
        let typing =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
                frontier.shared_trees(),
            )
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
        use typed_trees::deferred_refusals::DeferredRequest;

        let text = "machine length() -> u64 { 4 }\n\
                    machine helper() { let bytes: [u8; length()]; }\n";
        let (mut typing, frontier, sources) = deferred(text);
        let source_id = sources.files().next().unwrap().source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let (_, pending) = build_time_evaluation::evaluate_pre_resolution(
            build_time_evaluation::BuildTimeEvaluationRequest {
                syntax_trees: syntax,
                source_context: build_time_evaluation::BuildTimeSources {
                    sources: Some(Arc::clone(&sources)),
                    ..Default::default()
                },
            },
        )
        .unwrap()
        .into_syntax_and_pre_check();
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
        let (program, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            typing.typed().clone(),
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .unwrap();
        let refused = crate::AcceptedTerminalSource::judge(
            Arc::new(program),
            crate::TerminalSourceCompilation {
                entries: &[helper],
                subjects: &[],
                purpose: source::DependencyScope::Product,
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
