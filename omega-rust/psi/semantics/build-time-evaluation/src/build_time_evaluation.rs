//! Build-time evaluation lifecycle, on either side of name resolution.
//!
//! Evaluate syntax, retain its one-shot continuation, then finish typed work
//! under the same selection authority. Target filtering and provider selection
//! remain with the Omega coordinator between these two stages.

use std::sync::Arc;

use crate::{
    BuildTimeSelectionAuthority, FoldedArrayLength, PlacedViewRecord, PlanLaidRecord,
    SelectedBuildTimeBinaryOperator, SelectedBuildTimeProviderBody,
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
    let mut syntax_trees = crate::machine_execution::syntax_probes::normalize_generic_data(
        syntax_trees,
        sources,
        source_scoped_top_level_bindings,
        retained_base,
    )?;
    let plan_laid_records = plan_laid::desugar_plan_laid_value_types(&mut syntax_trees)?;
    Ok(PreResolutionEvaluation {
        syntax_trees,
        pre_check: PreCheckEvaluation {
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
#[must_use = "the matching typed tree must consume this pre-check continuation"]
pub struct PreCheckEvaluation {
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
    /// Keep value-dependent work together until exact boundary execution is
    /// selected. Independent ordinary build inputs retain their early route.
    pub fn evaluate_or_defer(
        self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<Option<Self>, Vec<diagnostics::Diagnostic>> {
        if const_lengths::evaluate_independent_lengths(typed, self.selection_authority.clone())?
            || range_endpoints::pending_endpoint_calls_need_operator_selection(
                typed,
                self.selection_authority.clone(),
            )?
            || const_domain_facts::pending_memberships_need_operator_selection(
                typed,
                self.selection_authority.clone(),
            )?
        {
            range_endpoints::defer_pending_endpoint_calls(typed)?;
            return Ok(Some(self));
        }
        self.evaluate(typed)?;
        Ok(None)
    }

    pub fn evaluate_extension_or_defer(
        mut self,
        typed: &mut typed_trees::TypedTrees,
        wire_schema_frontier: usize,
    ) -> Result<Option<Self>, Vec<diagnostics::Diagnostic>> {
        self.wire_schema_frontier = wire_schema_frontier;
        self.evaluate_or_defer(typed)
    }

    /// Finish the deferred const work under exact selected execution.
    /// `operators` supplies the sealed primitive-float meanings and
    /// `provider_bodies` the exact selected checked provider bodies; each
    /// retained occurrence keeps its own row so a stale or substituted
    /// selection can never stand in for the current one. The remaining typed
    /// const positions (range endpoints and concrete domain-fact memberships
    /// today) observe the same rows when the continuation resumes.
    pub fn evaluate_selected_operators(
        mut self,
        typed: &mut typed_trees::TypedTrees,
        selected: SelectedBuildTimeOperators<'_>,
    ) -> Result<Vec<FoldedArrayLength>, Vec<diagnostics::Diagnostic>> {
        let folds = const_lengths::evaluate_selected_array_lengths(
            typed,
            self.selection_authority.clone(),
            selected,
        )?;
        self.selected_operators = selected.operators.to_vec();
        self.provider_bodies = selected.provider_bodies.to_vec();
        self.evaluate(typed)?;
        Ok(folds)
    }

    /// Consume the exact continuation produced before name resolution.
    ///
    /// Omega may target-filter and type the returned syntax before this call,
    /// but the language-level evaluation order and selection authority remain
    /// owned by this continuation.
    pub fn evaluate(
        self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        const_lengths::evaluate_const_array_lengths(typed, self.selection_authority.clone())?;
        range_endpoints::evaluate_selected_range_endpoints(
            typed,
            self.selection_authority.clone(),
            SelectedBuildTimeOperators {
                operators: &self.selected_operators,
                provider_bodies: &self.provider_bodies,
            },
        )?;
        const_domain_facts::evaluate_selected_domain_facts(
            typed,
            self.selection_authority.clone(),
            SelectedBuildTimeOperators {
                operators: &self.selected_operators,
                provider_bodies: &self.provider_bodies,
            },
        )?;
        const_initializers::validate_retained_invocations(typed, self.selection_authority.clone())?;
        plan_laid::compute_plan_laid_layouts(
            typed,
            &self.plan_laid_records,
            self.selection_authority.clone(),
        )?;
        placed_views::validate_placed_view_plans(
            typed,
            &self.placed_view_records,
            self.selection_authority.clone(),
        )?;
        wire_plans::compute_wire_plans(typed, self.selection_authority, self.wire_schema_frontier)
    }

    /// Consume the continuation for syntax appended to an already evaluated
    /// typed checkpoint. Global pending const work remains detectable, while
    /// wire-plan publication is restricted to extension-owned schemas.
    pub fn evaluate_extension(
        mut self,
        typed: &mut typed_trees::TypedTrees,
        wire_schema_frontier: usize,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        self.wire_schema_frontier = wire_schema_frontier;
        self.evaluate(typed)
    }
}

#[cfg(test)]
mod tests;
