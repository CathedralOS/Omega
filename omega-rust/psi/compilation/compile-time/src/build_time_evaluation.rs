//! Build-time evaluation lifecycle, on either side of name resolution.
//!
//! Evaluate syntax, retain its one-shot continuation, then finish typed work
//! under the same selection authority. Target filtering and provider selection
//! remain with the Omega coordinator between these two stages.

use std::sync::Arc;
use typed_trees::references::DeclarationSelection;

/// Work ownership is narrower than executable lookup. Product work excludes
/// completed Build jobs but still needs shared toolchain declarations. Build
/// execution instead keeps exactly its admitted roster: a Product descriptor
/// operand must never promote a Product implementation into callable context.
#[derive(Clone)]
pub(crate) struct EvaluationDomain {
    work: DeclarationSelection,
    execution: DeclarationSelection,
    product_frontier: Option<Arc<typed_trees::TypedTrees>>,
}

impl EvaluationDomain {
    pub(crate) fn selected(work: DeclarationSelection, execution: DeclarationSelection) -> Self {
        Self {
            work,
            execution,
            product_frontier: None,
        }
    }

    pub(crate) fn all() -> Self {
        Self {
            work: DeclarationSelection::all(),
            execution: DeclarationSelection::all(),
            product_frontier: None,
        }
    }

    pub(crate) fn contains(&self, symbol: symbols::SymbolHandle) -> bool {
        self.work.contains(symbol)
    }

    /// Pending layout records precede typed layout installation. Resolve their
    /// producing declaration uniquely before using its work ownership; a name
    /// alone is not permission to execute another policy.
    pub(crate) fn contains_data(
        &self,
        program: &typed_trees::TypedTrees,
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

    /// One early job demands its complete checking and executable closure,
    /// rather than unrelated pending application inputs. Product descriptions
    /// remain available with the original custody and grant no body demand.
    pub(crate) fn demanded_roots(
        &self,
        program: &typed_trees::TypedTrees,
        roots: &[symbols::SymbolHandle],
    ) -> Self {
        let execution = self.execution_program(program);
        let mut domain = crate::syntax_probes::PreparedEvaluationInputs::domain_for_program(
            &execution,
            roots,
            &[],
        );
        domain.execution = domain.execution.intersect(&self.execution);
        domain.product_frontier = self.product_frontier.clone();
        domain
    }

    pub(crate) fn execution_program(
        &self,
        original: &typed_trees::TypedTrees,
    ) -> typed_trees::TypedTrees {
        let mut program =
            typed_trees_to_checked_trees::project_selected_program(original, &self.execution);
        if let Some(frontier) = &self.product_frontier {
            program.product_declaration_frontier = Some(frontier.clone());
        }
        program
    }

    pub(crate) fn references(
        &self,
        program: &typed_trees::TypedTrees,
    ) -> typed_trees::references::ProgramReferences {
        typed_trees::references::collect_selected_program_references(program, &self.work)
    }

    pub(crate) fn include(&mut self, symbol: symbols::SymbolHandle) {
        self.work.include(symbol);
        self.execution.include(symbol);
    }
}

use crate::{
    BuildTimeSelectionAuthority, PlacedViewRecord, PlanLaidRecord, SelectedBuildTimeBinaryOperator,
    SelectedBuildTimeProviderBody, const_evaluation::const_applications,
    const_evaluation::const_domain_facts, const_evaluation::const_generic_calls,
    const_evaluation::const_generic_expressions, const_evaluation::const_initializers,
    const_evaluation::const_lengths, const_evaluation::range_arguments,
    const_evaluation::range_endpoints, layouts::placed_views, layouts::plan_laid,
    layouts::wire_plans,
};

/// Inputs retained by package-aware probes and generated extension evaluation.
///
/// Source-scoped bindings are borrowed for this run; the source map and selection
/// authority can be retained by evaluators. An extension may select nominal
/// arguments from its predecessor, never re-normalize that predecessor's templates.
pub struct BuildTimeSourceContext<'source> {
    pub sources: Arc<source::SourceMap>,
    pub source_scoped_top_level_bindings: &'source [symbols::SourceScopedTopLevelBinding],
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    pub retained_base: Option<&'source symbol_resolved_trees::SymbolResolvedTrees>,
}

/// One pre-resolution evaluation request. Standalone syntax has no source
/// context; source-bound runs keep loader bindings and authority together.
pub struct BuildTimeEvaluationRequest<'source> {
    pub syntax_trees: syntax_trees::SyntaxTrees,
    pub source_context: Option<BuildTimeSourceContext<'source>>,
}

/// Evaluate syntax before resolution and retain the matching typed continuation.
pub fn evaluate_pre_resolution(
    request: BuildTimeEvaluationRequest<'_>,
) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
    let BuildTimeEvaluationRequest {
        syntax_trees,
        source_context,
    } = request;
    let (sources, source_scoped_top_level_bindings, selection_authority, retained_base) =
        match source_context {
            Some(context) => (
                Some(context.sources),
                context.source_scoped_top_level_bindings,
                context.selection_authority,
                context.retained_base,
            ),
            None => (None, &[][..], None, None),
        };
    let syntax_trees = const_initializers::evaluate(
        syntax_trees,
        sources.clone(),
        source_scoped_top_level_bindings,
        selection_authority.clone(),
    )?;
    let syntax_trees = const_generic_expressions::evaluate(
        syntax_trees,
        sources.clone(),
        source_scoped_top_level_bindings,
        selection_authority.as_deref(),
    )?;
    // Canonical ranges must exist before the const-call probe normalizes its
    // syntax clone: that clone's generic synthesis is itself an equation
    // customer, so retained endpoints cannot arrive after it.
    let syntax_trees = range_arguments::evaluate(
        syntax_trees,
        sources.clone(),
        source_scoped_top_level_bindings,
        selection_authority.clone(),
    )?;
    let mut syntax_trees = const_generic_calls::evaluate_const_generic_calls(
        syntax_trees,
        BuildTimeSources {
            sources: sources.clone(),
            source_scoped_top_level_bindings,
            selection_authority: selection_authority.clone(),
        },
    )?;
    syntax_trees_to_symbol_resolved_trees::pre_resolution::synthesize_trait_defaults(
        &mut syntax_trees,
        sources.clone(),
        source_scoped_top_level_bindings.to_vec(),
    )?;
    let placed_view_records = placed_views::desugar_placed_views(
        &mut syntax_trees,
        BuildTimeSources {
            sources: sources.clone(),
            source_scoped_top_level_bindings,
            selection_authority: selection_authority.clone(),
        },
    )?;
    let mut syntax_trees = crate::syntax_probes::normalize_generic_data(
        syntax_trees,
        sources,
        source_scoped_top_level_bindings,
        retained_base,
    )?;
    let plan_laid_records = plan_laid::desugar_plan_laid_value_types(&mut syntax_trees)?;
    Ok(PreResolutionEvaluation {
        syntax_trees,
        pre_check: PreCheckEvaluation {
            domain: EvaluationDomain::all(),
            wire_schema_frontier: 0,
            placed_view_records,
            plan_laid_records,
            selection_authority,
            selected_operators: Vec::new(),
            provider_bodies: Vec::new(),
        },
    })
}

/// Target-neutral syntax elaboration that must finish before name resolution.
///
/// Target selection remains an Omega orchestration concern and may run on the
/// returned syntax after this service has finished owning language-level
/// elaboration.
/// The build-time operators and provider bodies one evaluation runs with:
/// the selected binary operators and the ordinary checked provider bodies a
/// selected use means.
#[derive(Clone, Copy, Default)]
pub struct SelectedBuildTimeOperators<'a> {
    pub operators: &'a [SelectedBuildTimeBinaryOperator],
    pub provider_bodies: &'a [SelectedBuildTimeProviderBody],
}

/// The sources one syntax-phase evaluation resolves against: the source map,
/// the source-scoped top-level bindings, and the selection authority.
#[derive(Clone, Default)]
pub struct BuildTimeSources<'a> {
    pub sources: Option<Arc<source::SourceMap>>,
    pub source_scoped_top_level_bindings: &'a [symbols::SourceScopedTopLevelBinding],
    pub selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
}

/// A selection authority together with the invocation custody it admitted:
/// the pair a layout policy evaluation runs under when it is authorized.
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
    /// Normalize each generated unit against the same immutable predecessor.
    /// Siblings must not participate in one another's generic synthesis; only
    /// their already-normalized roots join the resolver's extension input.
    pub fn evaluate_extension(
        units: Vec<syntax_trees::SyntaxTrees>,
        first_source: source::SourceId,
        context: BuildTimeSourceContext<'_>,
    ) -> Result<(syntax_trees::SyntaxTrees, Vec<PreCheckEvaluation>), Vec<diagnostics::Diagnostic>>
    {
        let mut syntax = syntax_trees::SyntaxTrees::new(first_source);
        let mut pre_checks = Vec::with_capacity(units.len());
        for unit in units {
            let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
                syntax_trees: unit,
                source_context: Some(BuildTimeSourceContext {
                    sources: context.sources.clone(),
                    source_scoped_top_level_bindings: context.source_scoped_top_level_bindings,
                    selection_authority: context.selection_authority.clone(),
                    retained_base: context.retained_base,
                }),
            })?;
            let (unit, pre_check) = evaluated.into_syntax_and_pre_check();
            syntax.extend_from(&unit);
            pre_checks.push(pre_check);
        }
        Ok((syntax, pre_checks))
    }

    /// Separate the syntax consumed by target filtering and name resolution
    /// from the opaque continuation that owns the matching typed-tree work.
    pub fn into_syntax_and_pre_check(self) -> (syntax_trees::SyntaxTrees, PreCheckEvaluation) {
        (self.syntax_trees, self.pre_check)
    }
}

/// One-shot continuation for target-neutral typed-tree evaluation.
///
/// The records and optional package selection authority are private so a
/// caller cannot accidentally rejoin records from one pre-resolution run to
/// another run or choose a different authority after name resolution.
///
/// A clone belongs to a clone of the same typed tree: one shared frontend
/// hands each realized target its own copy of both.
#[derive(Clone)]
#[must_use = "the matching typed tree must consume this pre-check continuation"]
pub struct PreCheckEvaluation {
    /// Work roots, never an arena copy: folded types, evidence and fresh
    /// instances publish in the canonical frontend's original storage.
    domain: EvaluationDomain,
    wire_schema_frontier: usize,
    placed_view_records: Vec<PlacedViewRecord>,
    plan_laid_records: Vec<PlanLaidRecord>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    /// Exact selected rows supplied only when the deferred continuation is
    /// finished under Omega's settled plans. They stay empty on the early
    /// route so an unselected occurrence can never borrow selected custody.
    selected_operators: Vec<SelectedBuildTimeBinaryOperator>,
    provider_bodies: Vec<SelectedBuildTimeProviderBody>,
}

impl PreCheckEvaluation {
    /// Retain the work owned by a declaration projection of this typed program.
    ///
    /// Syntax elaboration records precede symbol assignment. Join their generated
    /// names to the original declarations once, then select by exact symbol in
    /// the projected roster. Absence from the projection is legitimate; absence
    /// or ambiguity in the producing program is lost custody, never skipped work.
    /// The original continuation remains available for the completed Product.
    pub fn project(
        &self,
        original: &typed_trees::TypedTrees,
        projection: &typed_trees::TypedTrees,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        let selected = DeclarationSelection::from_program(projection);
        self.select(
            original,
            EvaluationDomain {
                work: self.domain.work.intersect(&selected),
                execution: self.domain.execution.intersect(&selected),
                product_frontier: projection.product_declaration_frontier.clone(),
            },
        )
    }

    /// Retain Product work after a Build projection completed successfully.
    /// Call only after its independent checking and fold replay succeeded.
    /// Newly synthesized Build declarations must be present in the completed
    /// projection, so subtraction carries their exact identities too.
    pub fn exclude_projection(
        &self,
        original: &typed_trees::TypedTrees,
        completed_projection: &typed_trees::TypedTrees,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        self.select(
            original,
            EvaluationDomain {
                work: self
                    .domain
                    .work
                    .excluding(&DeclarationSelection::from_program(completed_projection)),
                execution: self.domain.execution.clone(),
                product_frontier: self.domain.product_frontier.clone(),
            },
        )
    }

    fn select(
        &self,
        original: &typed_trees::TypedTrees,
        domain: EvaluationDomain,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        let mut plan_laid_records = Vec::new();
        for record in &self.plan_laid_records {
            if domain.contains_data(original, &record.synthetic_name)? {
                plan_laid_records.push(record.clone());
            }
        }
        let mut placed_view_records = Vec::new();
        for record in &self.placed_view_records {
            if domain.contains_data(original, &record.synthetic_name)? {
                placed_view_records.push(record.clone());
            }
        }
        // Evaluation mutates the canonical roster, so the checkpoint remains
        // in canonical coordinates. The root selection independently limits it.
        original
            .wire_schemas()
            .get(..self.wire_schema_frontier)
            .ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "pre-check wire frontier exceeds its producing declarations",
                )]
            })?;
        Ok(Self {
            domain,
            wire_schema_frontier: self.wire_schema_frontier,
            placed_view_records,
            plan_laid_records,
            selection_authority: self.selection_authority.clone(),
            selected_operators: self.selected_operators.clone(),
            provider_bodies: self.provider_bodies.clone(),
        })
    }

    /// Keep value-dependent work together until exact boundary execution is
    /// selected. Independent ordinary build inputs retain their early route.
    pub fn evaluate_or_defer(
        self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<Option<Self>, Vec<diagnostics::Diagnostic>> {
        if const_lengths::evaluate_independent_lengths(
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
        )? {
            range_endpoints::defer_pending_endpoint_calls_in(typed, &self.domain)?;
            const_applications::defer_pending_const_applications(typed, &self.domain)?;
            validation::resolve_selected_dynamic_call_targets(typed, self.domain.work())?;
            return Ok(Some(self));
        }
        self.evaluate(typed)?;
        Ok(None)
    }

    /// Complete one generated cohort in unit order, retaining deferred work
    /// with its own wire frontier. Evaluation may append to the predecessor,
    /// never rewrite it, even when provider-dependent work remains deferred.
    pub fn evaluate_extension_or_defer(
        pre_checks: Vec<Self>,
        retained: &typed_trees::TypedTrees,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<Vec<Self>, Vec<diagnostics::Diagnostic>> {
        let mut wire_schema_frontier = retained.wire_schemas().len();
        let mut pending = Vec::new();
        for mut pre_check in pre_checks {
            pre_check.wire_schema_frontier = wire_schema_frontier;
            if let Some(pre_check) = pre_check.evaluate_or_defer(typed)? {
                pending.push(pre_check);
            }
            wire_schema_frontier = typed.wire_schemas().len();
        }
        if !symbol_resolved_trees_to_typed_trees::retained_typed_base_is_exact_prefix(
            retained, typed,
        ) {
            return Err(vec![diagnostics::Diagnostic::error(
                "generated-source pre-check evaluation changed the retained typed base",
            )]);
        }
        Ok(pending)
    }

    /// Finish the deferred const work under exact selected execution.
    /// `operators` supplies the sealed primitive-float meanings and
    /// `provider_bodies` the exact selected checked provider bodies; each
    /// retained occurrence keeps its own row so a stale or substituted
    /// selection can never stand in for the current one. The remaining typed
    /// const positions (range endpoints, concrete domain-fact memberships, and
    /// retained const-generic applications) observe the same rows when the
    /// continuation resumes.
    pub fn evaluate_selected_operators(
        mut self,
        typed: &mut typed_trees::TypedTrees,
        selected: SelectedBuildTimeOperators<'_>,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        const_lengths::evaluate_selected_array_lengths(
            typed,
            self.selection_authority.clone(),
            selected,
            &self.domain,
        )?;
        self.selected_operators = selected.operators.to_vec();
        self.provider_bodies = selected.provider_bodies.to_vec();
        self.evaluate(typed)
    }

    /// Consume the exact continuation produced before name resolution.
    ///
    /// Omega may target-filter and type the returned syntax before this call,
    /// but the language-level evaluation order and selection authority remain
    /// owned by this continuation.
    pub fn evaluate(
        mut self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        const_lengths::evaluate_const_array_lengths_in(
            typed,
            self.selection_authority.clone(),
            &self.domain,
        )?;
        range_endpoints::evaluate_selected_range_endpoints(
            typed,
            self.selection_authority.clone(),
            SelectedBuildTimeOperators {
                operators: &self.selected_operators,
                provider_bodies: &self.provider_bodies,
            },
            &self.domain,
        )?;
        const_domain_facts::evaluate_selected_domain_facts(
            typed,
            self.selection_authority.clone(),
            SelectedBuildTimeOperators {
                operators: &self.selected_operators,
                provider_bodies: &self.provider_bodies,
            },
            &self.domain,
        )?;
        const_applications::evaluate_selected_const_applications(
            typed,
            self.selection_authority.clone(),
            SelectedBuildTimeOperators {
                operators: &self.selected_operators,
                provider_bodies: &self.provider_bodies,
            },
            &mut self.domain,
        )?;
        const_initializers::validate_retained_invocations(
            typed,
            self.selection_authority.clone(),
            &self.domain,
        )?;
        plan_laid::compute_plan_laid_layouts_in(
            typed,
            &self.plan_laid_records,
            self.selection_authority.clone(),
            &self.domain,
        )?;
        placed_views::validate_placed_view_plans_in(
            typed,
            &self.placed_view_records,
            self.selection_authority.clone(),
            &self.domain,
        )?;
        wire_plans::compute_wire_plans_in(
            typed,
            self.selection_authority,
            self.wire_schema_frontier,
            &self.domain,
        )?;
        // Placed-field accessor targets become available with their evaluated
        // plans. Publish the elaborated program with those calls already bound,
        // including for Build execution before the final checked-tree pass.
        validation::resolve_selected_dynamic_call_targets(typed, self.domain.work())
    }
}
