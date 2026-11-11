/*
    Target-neutral compile-time admission, evaluation, and source/typed result
    elaboration for Omega constants, layout plans, access views, and Build
    declarations.

    We admit compile-time machine closures against checked or pre-check programs,
    evaluate closed generic expressions, constant initializers, fixed-array
    lengths, range endpoints, layout plans, and access views through the
    Terminal and checked interpreters, and admit Build-issued product, provider,
    and native-binding declarations against their source requests.
*/

/*
    Compile-time evaluation must elaborate constants, geometry, and Build
    declarations under Psi's target-neutral language rules before target
    realization, while keeping Build host effects and target scheduling in
    Omega. Centralizing admission, temporary syntax probes, layout validation,
    and receipt accounting here prevents pipeline stages from reinterpreting
    compile-time execution or trusting unverified fold results.
*/

/*
    Start at `build_time_evaluation` (`evaluate_pre_resolution` and
    `PreCheckEvaluation`) in this file for the two-phase pre-resolution and
    typed pre-check lifecycle. `const_evaluation`, `const_generic_expressions`,
    and `const_initializers` evaluate closed constants and generic arguments;
    `layouts` evaluates and installs layout plans, `LayoutStorage` applications,
    and access views; `machine_execution` owns closure admission and build
    machine preparation; `product` admits Build declarations about the Product;
    `scalar_applications` evaluates Terminal scalar applications;
    `syntax_probes` runs isolated frontend probes; and `evaluation_accounting`
    records execution receipts.
*/

#![forbid(unsafe_code)]

pub use product::assumed_bindings;
pub use product::test_groups;
mod const_generic_expressions;
mod const_initializers;
pub(crate) mod build_time_evaluation {
    use crate::const_evaluation::constant_recipes::retain_deferred_constant_recipes;
    use crate::evaluation_accounting::EvaluationAccounting;
    use crate::syntax_probes::PreparedEvaluationInputs;
    use checked_trees::CheckedValueOrigin;
    use compiler_storage::{RecordError, RecordReader};
    use layout_plans::LayoutTargetFacts;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::references::{DeclarationSelection, ProgramReferences};

    pub(crate) fn expression_table(
        program: &TypedTrees,
    ) -> &typed_trees::expression::ExpressionTable {
        &program.expression_table
    }

    pub(crate) fn type_reference_table(
        program: &TypedTrees,
    ) -> &typed_trees::types::TypeReferenceTable {
        &program.type_reference_table
    }

    pub(crate) fn type_references(
        tables: &syntax_trees::SyntaxTreeTables,
    ) -> &syntax_trees::types::TypeReferenceTable {
        &tables.type_references
    }

    #[derive(Clone, compiler_storage::Record)]
    pub(crate) struct EvaluationDomain {
        pub(crate) layout_targets: Vec<LayoutTargetFacts>,
        pub(crate) layout_applications: Option<Vec<crate::LayoutApplicationTargetInput>>,
        work: DeclarationSelection,
        execution: DeclarationSelection,
        product_frontier: Option<Arc<TypedTrees>>,
        pub(crate) accounting: EvaluationAccounting,
    }

    impl PartialEq for EvaluationDomain {
        fn eq(&self, other: &Self) -> bool {
            self.layout_targets == other.layout_targets
                && self.layout_applications == other.layout_applications
                && self.work == other.work
                && self.execution == other.execution
                && self.product_frontier == other.product_frontier
        }
    }
    impl Eq for EvaluationDomain {}

    impl EvaluationDomain {
        pub(crate) fn with_accounting(mut self, accounting: EvaluationAccounting) -> Self {
            self.accounting = accounting;
            self
        }

        pub(crate) fn admission(
            &self,
            program: &TypedTrees,
            authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        ) -> crate::BuildTimeAdmissionPlan {
            crate::BuildTimeAdmissionPlan::infer(program, authority)
                .with_accounting(self.accounting.clone())
        }
        pub(crate) fn selected(
            work: DeclarationSelection,
            execution: DeclarationSelection,
        ) -> Self {
            Self {
                layout_targets: Vec::new(),
                layout_applications: None,
                work,
                execution,
                product_frontier: None,
                accounting: Default::default(),
            }
        }

        pub(crate) fn all() -> Self {
            Self {
                layout_targets: Vec::new(),
                layout_applications: None,
                work: DeclarationSelection::all(),
                execution: DeclarationSelection::all(),
                product_frontier: None,
                accounting: Default::default(),
            }
        }

        pub(crate) fn contains(&self, symbol: SymbolHandle) -> bool {
            self.work.contains(symbol)
        }

        pub(crate) fn contains_data(
            &self,
            program: &TypedTrees,
            name: &str,
        ) -> Result<bool, Vec<diagnostics::Diagnostic>> {
            let mut declarations = program
                .data_definitions()
                .iter()
                .filter(|declaration| declaration.name.as_str() == name);
            let (Some(declaration), None) = (declarations.next(), declarations.next()) else {
                return Err(vec![diagnostics::Diagnostic::error(format!(
                    "pre-check record `{name}` lost its unique producing data declaration",
                ))]);
            };
            Ok(self.contains(declaration.symbol))
        }

        pub(crate) fn work(&self) -> &DeclarationSelection {
            &self.work
        }

        pub(crate) fn demanded_roots(&self, program: &TypedTrees, roots: &[SymbolHandle]) -> Self {
            let execution = self.execution_program(program);
            let mut domain = PreparedEvaluationInputs::domain_for_program(&execution, roots, &[]);
            domain.execution = domain.execution.intersect(&self.execution);
            domain.layout_targets = self.layout_targets.clone();
            domain.layout_applications = self.layout_applications.clone();
            domain.product_frontier = self.product_frontier.clone();
            domain.accounting = self.accounting.clone();
            domain
        }

        pub(crate) fn execution_program(&self, original: &TypedTrees) -> TypedTrees {
            let mut program = validation::execution_projection::project_selected_program(
                original,
                &self.execution,
            );
            if let Some(frontier) = &self.product_frontier {
                program.product_declaration_frontier = Some(frontier.clone());
            }
            program
        }

        pub(crate) fn references(&self, program: &TypedTrees) -> ProgramReferences {
            typed_trees::references::collect_selected_program_references(program, &self.work)
        }

        pub(crate) fn include(&mut self, symbol: SymbolHandle) {
            self.work.include(symbol);
            self.execution.include(symbol);
        }
    }

    use crate::{
        AccessViewRecord, BuildTimeSelectionAuthority, SelectedBuildTimeBinaryOperator,
        SelectedBuildTimeProviderBody, const_evaluation::const_applications,
        const_evaluation::const_domain_facts, const_evaluation::const_generic_calls,
        const_evaluation::const_generic_expressions, const_evaluation::const_initializers,
        const_evaluation::const_lengths, const_evaluation::data_layout_queries,
        const_evaluation::range_endpoints, layouts::access_views, layouts::plan_laid,
    };

    pub struct BuildTimeEvaluationRequest<'source> {
        pub syntax_trees: syntax_trees::SyntaxTrees,
        pub source_context: BuildTimeSources<'source>,
    }

    pub fn evaluate_pre_resolution(
        request: BuildTimeEvaluationRequest<'_>,
    ) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
        let BuildTimeEvaluationRequest {
            syntax_trees,
            source_context,
        } = request;
        let syntax_trees =
            crate::syntax_probes::normalize_type_aliases(syntax_trees, &source_context)?;
        let syntax_trees = const_initializers::evaluate(syntax_trees, &source_context)?;
        let syntax_trees = const_generic_expressions::evaluate(syntax_trees, &source_context)?;
        let syntax_trees = crate::const_evaluation::const_qualifications::prove_ready(
            syntax_trees,
            &source_context,
        )?;
        let mut syntax_trees = const_generic_calls::evaluate_const_generic_calls(
            syntax_trees,
            source_context.clone(),
        )?;
        syntax_trees_to_symbol_resolved_trees::pre_resolution::synthesize_trait_defaults(
            &mut syntax_trees,
            source_context.sources.clone(),
            source_context.source_scoped_top_level_bindings.to_vec(),
            source_context.retained_base(),
        )?;
        let access_view_records =
            access_views::desugar_access_views(&mut syntax_trees, source_context.clone())?;
        let roots_before_generic = syntax_trees.root_item_handles().len();
        let mut syntax_trees = crate::syntax_probes::normalize_generic_data(
            syntax_trees,
            source_context.sources.clone(),
            source_context.source_scoped_top_level_bindings,
            source_context.retained_base(),
            [
                source_context.declaration_syntax,
                source_context.retained_declaration_syntax,
            ],
        )?;
        if syntax_trees.root_item_handles().len() > roots_before_generic {
            syntax_trees = const_generic_expressions::evaluate(syntax_trees, &source_context)?;
            syntax_trees = const_generic_calls::evaluate_const_generic_calls(
                syntax_trees,
                source_context.clone(),
            )?;
        }
        Ok(PreResolutionEvaluation {
            syntax_trees,
            pre_check: PreCheckEvaluation {
                recorded_authority: None,
                domain: EvaluationDomain::all(),
                wire_schema_frontier: 0,
                access_view_records,
                selection_authority: source_context.selection_authority,
                selected_operators: Vec::new(),
                provider_bodies: Vec::new(),
            },
        })
    }

    #[derive(Clone, Copy, Default)]
    pub struct SelectedBuildTimeOperators<'a> {
        pub operators: &'a [SelectedBuildTimeBinaryOperator],
        pub provider_bodies: &'a [SelectedBuildTimeProviderBody],
    }

    #[derive(Clone, Default)]
    pub struct BuildTimeSources<'a> {
        pub declaration_syntax: Option<&'a syntax_trees::SyntaxTrees>,
        pub retained_declaration_syntax: Option<&'a syntax_trees::SyntaxTrees>,
        pub sources: Option<Arc<source::SourceMap>>,
        pub source_scoped_top_level_bindings: &'a [symbols::SourceScopedTopLevelBinding],
        pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        pub predecessor: Option<symbol_resolved_trees_to_typed_trees::SeededTypingBase>,
    }

    impl BuildTimeSources<'_> {
        pub(crate) fn retained_base(&self) -> Option<&symbol_resolved_trees::SymbolResolvedTrees> {
            self.predecessor
                .as_ref()
                .map(|predecessor| predecessor.resolved())
        }

        pub(crate) fn resolution_request(
            &self,
            syntax: syntax_trees::SyntaxTrees,
        ) -> syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax,
                sources: self.sources.clone(),
                top_level_bindings: self.source_scoped_top_level_bindings.to_vec(),
                retained_base: self
                    .predecessor
                    .as_ref()
                    .map(|predecessor| predecessor.shared_resolved()),
            }
        }
    }

    #[derive(Clone)]
    pub struct AuthorizedSelection {
        pub authority: Arc<dyn BuildTimeSelectionAuthority>,
        pub custody: crate::BuildTimeInvocationCustody,
    }

    #[must_use = "pre-resolution syntax and its matching pre-check continuation must stay paired"]
    pub struct PreResolutionEvaluation {
        syntax_trees: syntax_trees::SyntaxTrees,
        pre_check: PreCheckEvaluation,
    }

    impl PreResolutionEvaluation {
        pub fn evaluate_extension(
            units: Vec<syntax_trees::SyntaxTrees>,
            first_source: source::SourceId,
            context: BuildTimeSources<'_>,
        ) -> Result<
            (syntax_trees::SyntaxTrees, Vec<PreCheckEvaluation>),
            Vec<diagnostics::Diagnostic>,
        > {
            let mut syntax: Option<syntax_trees::SyntaxTrees> = None;
            let mut pre_checks = Vec::with_capacity(units.len());
            for unit in units {
                let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
                    syntax_trees: unit,
                    source_context: context.clone(),
                })?;
                let (unit, pre_check) = evaluated.into_syntax_and_pre_check();
                match &mut syntax {
                    Some(syntax) => syntax.adopt_page(unit),
                    None => syntax = Some(unit),
                }
                pre_checks.push(pre_check);
            }
            let mut syntax = syntax.unwrap_or_else(|| syntax_trees::SyntaxTrees::new(first_source));
            let extension_generation = u32::try_from(syntax.page_count())
                .ok()
                .and_then(|generation| generation.checked_add(1))
                .expect("generated syntax page identity overflow");
            syntax.begin_page(extension_generation);
            Ok((syntax, pre_checks))
        }

        pub fn into_syntax_and_pre_check(self) -> (syntax_trees::SyntaxTrees, PreCheckEvaluation) {
            (self.syntax_trees, self.pre_check)
        }
    }

    #[derive(Clone)]
    #[must_use = "the matching typed tree must consume this pre-check continuation"]
    pub struct PreCheckEvaluation {
        recorded_authority: Option<validation::RecordedDeclarationAuthority>,
        domain: EvaluationDomain,
        wire_schema_frontier: usize,
        access_view_records: Vec<AccessViewRecord>,
        selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        selected_operators: Vec<SelectedBuildTimeBinaryOperator>,
        provider_bodies: Vec<SelectedBuildTimeProviderBody>,
    }

    #[must_use = "finish the evaluated initializer declarations after query closure"]
    pub struct EvaluatedPreCheck {
        domain: EvaluationDomain,
        selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        selected_operators: Vec<SelectedBuildTimeBinaryOperator>,
        provider_bodies: Vec<SelectedBuildTimeProviderBody>,
    }

    impl EvaluatedPreCheck {
        pub fn finish(self, typed: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
            let selected = SelectedBuildTimeOperators {
                operators: &self.selected_operators,
                provider_bodies: &self.provider_bodies,
            };
            {
                const_initializers::settle_target_observations(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                    selected,
                )?;
            }
            const_initializers::validate_retained_invocations(
                typed,
                self.selection_authority,
                &self.domain,
                selected,
            )
        }
    }

    impl PreCheckEvaluation {
        pub fn rejoin_source_authority(
            &mut self,
            program: &TypedTrees,
            authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        ) -> Result<(), String> {
            if let Some(recorded) = &self.recorded_authority {
                self.selection_authority = match recorded {
                    validation::RecordedDeclarationAuthority::Absent => None,
                    validation::RecordedDeclarationAuthority::Packages(_) => {
                        let declarations =
                            typed_trees::references::collect_program_declarations(program)
                                .into_iter()
                                .filter(|symbol| {
                                    self.domain.work.contains(*symbol)
                                        || self.domain.execution.contains(*symbol)
                                })
                                .chain(
                                    self.selected_operators
                                        .iter()
                                        .map(|operator| operator.requirement),
                                )
                                .chain(self.provider_bodies.iter().flat_map(|provider| {
                                    [
                                        provider.requirement,
                                        provider.provider_machine,
                                        provider.provider_state,
                                        provider.provider_declaration,
                                    ]
                                }));
                        let mut sites = declarations
                            .filter_map(|symbol| {
                                program.symbols.symbol_provenance_source_span(symbol)
                            })
                            .collect::<Vec<_>>();
                        let uses = self
                            .selected_operators
                            .iter()
                            .map(|operator| {
                                (
                                    operator.expression,
                                    operator.origin,
                                    operator.operands.as_slice(),
                                )
                            })
                            .chain(self.provider_bodies.iter().map(|provider| {
                                (
                                    provider.expression,
                                    provider.origin,
                                    provider.operands.as_slice(),
                                )
                            }));
                        let mut needs_full_scope = false;
                        for (expression, origin, operands) in uses {
                            if let Some(machine) = origin.machine_symbol() {
                                match program.symbols.symbol_provenance_source_span(machine) {
                                    Some(site) => sites.push(site),
                                    None => needs_full_scope = true,
                                }
                            }
                            let parent = match origin {
                                CheckedValueOrigin::NestedExpression { parent } => Some(parent),
                                _ => None,
                            };
                            for occurrence in std::iter::once(expression)
                                .chain(parent)
                                .chain(operands.iter().copied())
                            {
                                match program.expression_table.try_source_span(occurrence) {
                                    Some(site) if site.span.start < site.span.end => {
                                        sites.push(site)
                                    }
                                    _ => needs_full_scope = true,
                                }
                            }
                        }
                        sites.extend(
                            self.access_view_records
                                .iter()
                                .flat_map(|record| record.invocation_sources.iter().copied()),
                        );
                        recorded.require_current_sites(
                            authority.as_deref(),
                            program,
                            sites.into_iter(),
                        )?;
                        if needs_full_scope {
                            let sources = program
                                .symbols
                                .source_map()
                                .ok_or("retained selected use has no complete source context")?;
                            recorded.require_current(authority.as_deref(), &sources)?;
                        }
                        if let Some(frontier) = &self.domain.product_frontier {
                            let sources = frontier.symbols.source_map().ok_or(
                                "retained Product descriptor frontier has no source context",
                            )?;
                            recorded.require_current(authority.as_deref(), &sources)?;
                        }
                        authority
                    }
                };
                self.recorded_authority = None;
            }
            Ok(())
        }

        pub fn same_pending_work(&self, other: &Self) -> bool {
            self.recorded_authority.is_none()
                && other.recorded_authority.is_none()
                && self.domain == other.domain
                && self.wire_schema_frontier == other.wire_schema_frontier
                && self.access_view_records == other.access_view_records
                && self.selected_operators == other.selected_operators
                && self.provider_bodies == other.provider_bodies
                && match (&self.selection_authority, &other.selection_authority) {
                    (None, None) => true,
                    (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                    _ => false,
                }
        }

        pub fn project(
            &self,
            original: &TypedTrees,
            projection: &TypedTrees,
        ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
            let selected = DeclarationSelection::from_program(projection);
            self.select(
                original,
                EvaluationDomain {
                    layout_targets: self.domain.layout_targets.clone(),
                    layout_applications: self.domain.layout_applications.clone(),
                    work: self.domain.work.intersect(&selected),
                    execution: self.domain.execution.intersect(&selected),
                    product_frontier: projection.product_declaration_frontier.clone(),
                    accounting: self.domain.accounting.clone(),
                },
            )
        }

        pub fn activate_sources(
            &self,
            original: &TypedTrees,
            activation: &TypedTrees,
        ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
            let mut pending = self.project(original, activation)?;
            let selected = DeclarationSelection::from_program(activation);
            pending.wire_schema_frontier = original.wire_schemas()[..self.wire_schema_frontier]
                .iter()
                .filter(|schema| selected.contains(schema.symbol))
                .count();
            Ok(pending)
        }

        pub fn exclude_completed_work(
            &self,
            original: &TypedTrees,
            completed_projection: &TypedTrees,
        ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
            self.select(
                original,
                EvaluationDomain {
                    layout_targets: self.domain.layout_targets.clone(),
                    layout_applications: self.domain.layout_applications.clone(),
                    work: self
                        .domain
                        .work
                        .excluding(&DeclarationSelection::from_program(completed_projection)),
                    execution: self.domain.execution.clone(),
                    product_frontier: self.domain.product_frontier.clone(),
                    accounting: self.domain.accounting.clone(),
                },
            )
        }

        fn select(
            &self,
            original: &TypedTrees,
            domain: EvaluationDomain,
        ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
            let mut access_view_records = Vec::new();
            for record in &self.access_view_records {
                if domain.contains_data(original, &record.synthetic_name)? {
                    access_view_records.push(record.clone());
                }
            }
            original
                .wire_schemas()
                .get(..self.wire_schema_frontier)
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "pre-check wire frontier exceeds its producing declarations",
                    )]
                })?;
            Ok(Self {
                recorded_authority: self.recorded_authority.clone(),
                domain,
                wire_schema_frontier: self.wire_schema_frontier,
                access_view_records,
                selection_authority: self.selection_authority.clone(),
                selected_operators: self.selected_operators.clone(),
                provider_bodies: self.provider_bodies.clone(),
            })
        }

        pub fn evaluate_or_defer_recorded(
            mut self,
            typed: &mut TypedTrees,
        ) -> Result<crate::RecordedPreCheck, Vec<diagnostics::Diagnostic>> {
            let accounting = EvaluationAccounting::recording();
            self.domain.accounting = accounting.clone();
            let mut pending = self.evaluate_or_defer(typed)?;
            if let Some(pending) = &mut pending {
                pending.domain.accounting = Default::default();
            }
            Ok(crate::RecordedPreCheck {
                pending,
                accounting: accounting
                    .finish()
                    .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?,
            })
        }

        pub fn evaluate_or_defer(
            mut self,
            typed: &mut TypedTrees,
        ) -> Result<Option<Self>, Vec<diagnostics::Diagnostic>> {
            if self.recorded_authority.is_some() {
                return Err(vec![diagnostics::Diagnostic::error(
                    "restored pre-check has not rejoined current declaration authority",
                )]);
            }
            self.complete_access_views(typed)?;
            const_initializers::retain_deferred_entries(typed, &self.domain);
            let defer = {
                const_lengths::evaluate_independent_lengths(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )? || range_endpoints::pending_endpoint_calls_need_operator_selection(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )? || const_domain_facts::pending_memberships_need_operator_selection(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )? || const_applications::pending_const_applications_need_operator_selection(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )? || data_layout_queries::has_pending_target_observations(
                    typed,
                    self.domain.work(),
                )
            };
            if defer {
                range_endpoints::defer_pending_endpoint_calls_in(typed, &self.domain)?;
                const_applications::defer_pending_const_applications(typed, &self.domain)?;
                const_applications::realize_symbolic_applications(typed, &mut self.domain)?;
                retain_deferred_constant_recipes(typed, &self.domain)?;
                validation::resolve_selected_dynamic_call_targets(typed, self.domain.work())?;
                return Ok(Some(self));
            }
            self.evaluate(typed)?;
            Ok(None)
        }

        pub fn evaluate_extension_or_defer(
            pre_checks: Vec<Self>,
            mut wire_schema_frontier: usize,
            typed: &mut TypedTrees,
        ) -> Result<Vec<Self>, Vec<diagnostics::Diagnostic>> {
            let mut pending = Vec::new();
            for mut pre_check in pre_checks {
                pre_check.wire_schema_frontier = wire_schema_frontier;
                if let Some(pre_check) = pre_check.evaluate_or_defer(typed)? {
                    pending.push(pre_check);
                }
                wire_schema_frontier = typed.wire_schemas().len();
            }
            Ok(pending)
        }

        pub(crate) fn with_layout_targets(mut self, targets: &[LayoutTargetFacts]) -> Self {
            self.domain.layout_targets = targets.to_vec();
            self
        }

        pub(crate) fn with_layout_applications(
            mut self,
            inputs: Option<&[crate::LayoutApplicationTargetInput]>,
        ) -> Self {
            self.domain.layout_applications = inputs.map(<[_]>::to_vec);
            self
        }

        pub(crate) fn evaluate_selected_operators(
            mut self,
            typed: &mut TypedTrees,
            selected: SelectedBuildTimeOperators<'_>,
        ) -> Result<EvaluatedPreCheck, Vec<diagnostics::Diagnostic>> {
            if self.recorded_authority.is_some() {
                return Err(vec![diagnostics::Diagnostic::error(
                    "restored pre-check has not rejoined current declaration authority",
                )]);
            }
            self.complete_access_views(typed)?;
            const_lengths::evaluate_selected_array_lengths(
                typed,
                self.selection_authority.clone(),
                selected,
                &self.domain,
            )?;
            self.selected_operators = selected.operators.to_vec();
            self.provider_bodies = selected.provider_bodies.to_vec();
            self.evaluate_work(typed)
        }

        pub fn evaluate(self, typed: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
            if self.recorded_authority.is_some() {
                return Err(vec![diagnostics::Diagnostic::error(
                    "restored pre-check has not rejoined current declaration authority",
                )]);
            }
            self.evaluate_work(typed)?.finish(typed)
        }

        fn evaluate_work(
            mut self,
            typed: &mut TypedTrees,
        ) -> Result<EvaluatedPreCheck, Vec<diagnostics::Diagnostic>> {
            self.complete_access_views(typed)?;
            const_initializers::retain_deferred_entries(typed, &self.domain);
            const_lengths::evaluate_const_array_lengths_in(
                typed,
                self.selection_authority.clone(),
                &self.domain,
            )?;
            {
                range_endpoints::evaluate_selected_range_endpoints(
                    typed,
                    self.selection_authority.clone(),
                    SelectedBuildTimeOperators {
                        operators: &self.selected_operators,
                        provider_bodies: &self.provider_bodies,
                    },
                    &self.domain,
                )
            }?;
            {
                const_domain_facts::evaluate_selected_domain_facts(
                    typed,
                    self.selection_authority.clone(),
                    SelectedBuildTimeOperators {
                        operators: &self.selected_operators,
                        provider_bodies: &self.provider_bodies,
                    },
                    &self.domain,
                )
            }?;
            {
                const_applications::evaluate_selected_const_applications(
                    typed,
                    self.selection_authority.clone(),
                    SelectedBuildTimeOperators {
                        operators: &self.selected_operators,
                        provider_bodies: &self.provider_bodies,
                    },
                    &mut self.domain,
                )
            }?;
            plan_laid::compute_plan_laid_layouts_in(
                typed,
                self.selection_authority.clone(),
                &self.domain,
            )?;
            typed.retain_symbolic_constant_recipes();
            validation::resolve_selected_dynamic_call_targets(typed, self.domain.work())?;
            Ok(EvaluatedPreCheck {
                domain: self.domain,
                selection_authority: self.selection_authority,
                selected_operators: self.selected_operators,
                provider_bodies: self.provider_bodies,
            })
        }

        fn complete_access_views(
            &mut self,
            typed: &mut TypedTrees,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            if self.access_view_records.is_empty() {
                return Ok(());
            }
            access_views::validate_access_view_plans_in(
                typed,
                &self.access_view_records,
                self.selection_authority.clone(),
                &self.domain,
            )?;
            self.access_view_records.clear();
            Ok(())
        }
    }

    impl compiler_storage::Record for PreCheckEvaluation {
        fn write(&self, output: &mut compiler_storage::RecordWriter) -> Result<(), RecordError> {
            output.record(|output| {
                self.domain.write(output)?;
                self.wire_schema_frontier.write(output)?;
                self.access_view_records.write(output)?;
                let captured;
                let authority = match &self.recorded_authority {
                    Some(authority) => authority,
                    None => {
                        captured = validation::RecordedDeclarationAuthority::capture(
                            self.selection_authority.as_deref(),
                        )
                        .map_err(|reason| output.error(reason))?;
                        &captured
                    }
                };
                authority.write(output)?;
                self.selected_operators.write(output)?;
                self.provider_bodies.write(output)
            })
        }
        fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
            input.record(|input| {
                Ok(Self {
                    domain: compiler_storage::Record::read(input)?,
                    wire_schema_frontier: compiler_storage::Record::read(input)?,
                    access_view_records: compiler_storage::Record::read(input)?,
                    recorded_authority: Some(compiler_storage::Record::read(input)?),
                    selection_authority: None,
                    selected_operators: compiler_storage::Record::read(input)?,
                    provider_bodies: compiler_storage::Record::read(input)?,
                })
            })
        }
    }
}
mod const_evaluation;
mod evaluation_accounting;
pub use evaluation_accounting::{
    CanonicalScalarEvaluationReceipt, PureEvaluationOutcome, PureEvaluationProgram,
    PureEvaluationReceipt, PureEvaluationRecord, PureTerminalEvaluationOutcome,
    PureTerminalEvaluationReceipt, RecordedPreCheck, SourceCompletionAccounting,
};
mod layouts;
mod machine_execution;
mod product;
pub use product::product_declarations;
pub use product::product_policy;
pub use product::provider_declarations;
pub(crate) mod reflection {
    pub use validation::reflection::*;
}
mod scalar_applications;
pub use scalar_applications::{
    ScalarScopeEvaluation, complete_scalar_applications, evaluate_scalar_applications,
    evaluate_scalar_applications_with_address_model,
};
mod syntax_probes;
/* Build-time evaluation collects landing diagnostics beside its value.
Warnings are printed for the author; an error (an implicit integer landing
after a fractional intermediate, numeric_values.md) rejects the program, so
no evaluator site may print it and continue with the folded value. */
pub(crate) fn report_evaluation_diagnostics<'a>(
    diagnostics: impl IntoIterator<Item = &'a diagnostics::Diagnostic>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut errors = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.is_error() {
            if !errors.contains(diagnostic) {
                errors.push(diagnostic.clone());
            }
        } else {
            eprintln!("{diagnostic}");
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
pub(crate) mod unique {
    pub(crate) fn extend_unique<T: PartialEq>(
        target: &mut Vec<T>,
        additions: impl IntoIterator<Item = T>,
    ) {
        for addition in additions {
            push_unique(target, addition);
        }
    }

    pub(crate) fn push_unique<T: PartialEq>(target: &mut Vec<T>, addition: T) {
        if !target.contains(&addition) {
            target.push(addition);
        }
    }
}
pub(crate) mod worklist {
    pub(crate) fn unvisited<T: PartialEq + Copy>(
        frontier: &mut Vec<T>,
        visited: &mut Vec<T>,
    ) -> Option<T> {
        while let Some(item) = frontier.pop() {
            if !visited.contains(&item) {
                visited.push(item);
                return Some(item);
            }
        }
        None
    }
}

pub use language_semantics::wire::WireCompatibilityDemand;

pub use build_time_values::BuildTimeValue;
pub use checked_interpreter::{
    CURRENT_EVALUATION_SEMANTICS, EvaluationUsage, MeasuredEvaluation,
    SelectedBuildTimeBinaryOperator, SelectedBuildTimeMachineBody,
};
pub use const_evaluation::const_domain_facts::evaluate_const_domain_facts;
pub use const_evaluation::const_generic_calls::evaluate_const_generic_calls;
pub use const_evaluation::const_lengths::{
    evaluate_const_array_lengths, evaluate_zero_argument_machine,
};
pub use const_evaluation::data_layout_queries::{
    LayoutApplicationTargetInput, LayoutQueryMeasurements, LayoutQueryProvider,
};
pub use const_evaluation::range_endpoints::evaluate_const_range_endpoints;
pub use const_evaluation::selected_constants::SelectedConstantEvaluationRequest;
pub use layouts::access_lifecycles::{admit_access_lifecycle_plans, retain_access_lifecycle_plans};
pub use layouts::access_views::{AccessViewRecord, desugar_access_views};
pub use layouts::layout_plans::compute_native_layout_plan_for_machine;
pub use layouts::plan_laid::{
    FixedLayoutTargetScope, evaluate_fixed_layout_applications, has_fixed_layout_applications,
};
pub use machine_execution::admission::{
    BuildTimeAdmissionPlan, BuildTimeAdmissionRejection, BuildTimeInvocationCustody,
    BuildTimeProgram, BuildTimeSelectionAuthority,
};
pub use machine_execution::build_activation::{
    admit_toolchain_build_activation, is_exact_toolchain_build_prelude_data,
};
pub use machine_execution::build_machines::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineCapturedFilesystemRoot,
    BuildMachineExecutionMode, BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemGrants,
    BuildMachineFilesystemMetadataLayout, BuildMachineFilesystemSponsor,
    BuildMachineFilesystemSponsorLimits, CheckedBuildProgram, PreparedBuildMachineEntry,
    PreparedBuildMachineProgram,
};
pub use machine_execution::selected_operators::{
    SelectedBuildTimeProviderBody, validate_selected_operators, validate_selected_provider_bodies,
};
pub use reflection::{
    CaseDescription, DeclarationDescription, FieldDescription, FieldInfo, MemberKind,
    MemberSelectionKey, NominalReferenceDescription, RuntimeVisitationPlan, SchemaNode,
    SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, ScopedSelectionReceiver, SelectionChoice,
    SelectionCoverage, SelectionProjection, SelectionRecord, SelectionRequirement,
    SelectionSnapshot, SemanticSchemaGraph, TypeParameterDescription, VisitationCall,
    VisitationOperation, construct_semantic_schema_graph, replay_runtime_visitation_plan,
    replay_selection_snapshot, replay_semantic_schema_graph,
};

pub use build_time_evaluation::{
    AuthorizedSelection, BuildTimeEvaluationRequest, BuildTimeSources, EvaluatedPreCheck,
    PreCheckEvaluation, PreResolutionEvaluation, SelectedBuildTimeOperators,
    evaluate_pre_resolution,
};

pub use machine_execution::selected_target_bodies::{
    SelectedBuildTimeTargetBody, prepare_selected_target_family_calls,
};

pub use validation::native_locators;
pub use validation::product_entries;

pub use product::native_bindings;
