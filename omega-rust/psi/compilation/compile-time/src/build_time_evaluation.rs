//! Build-time evaluation lifecycle, on either side of name resolution.
//!
//! Evaluate syntax, retain its one-shot continuation, then finish typed work
//! under the same selection authority. Target filtering and provider selection
//! remain with the Omega coordinator between these two stages.

use std::sync::Arc;
use typed_trees::references::DeclarationSelection;

/// The expression table of `program`. A reader that would otherwise name the
/// table in the middle of a long receiver chain names it here once, and the
/// chain keeps its receiver on one line.
pub(crate) fn expression_table(
    program: &typed_trees::TypedTrees,
) -> &typed_trees::expression::ExpressionTable {
    &program.expression_table
}

/// The type-reference table of `program`, named once for a reader whose
/// receiver chain would otherwise carry it in the middle.
pub(crate) fn type_reference_table(
    program: &typed_trees::TypedTrees,
) -> &typed_trees::types::TypeReferenceTable {
    &program.type_reference_table
}

/// The type-reference table of the syntax under check, named once for a reader
/// whose receiver chain would otherwise carry it in the middle. The tables are
/// the deref target of the syntax root, so both receivers coerce here.
pub(crate) fn type_references(
    tables: &syntax_trees::SyntaxTreeTables,
) -> &syntax_trees::types::TypeReferenceTable {
    &tables.type_references
}

/// Work ownership is narrower than executable lookup. Product work excludes
/// completed Build jobs but still needs shared toolchain declarations. Build
/// execution instead keeps exactly its admitted roster: a Product descriptor
/// operand must never promote a Product implementation into callable context.
#[derive(Clone, compiler_storage::Record)]
pub(crate) struct EvaluationDomain {
    pub(crate) layout_targets: Vec<layout_plans::LayoutTargetFacts>,
    pub(crate) layout_applications: Option<Vec<crate::LayoutApplicationTargetInput>>,
    work: DeclarationSelection,
    execution: DeclarationSelection,
    product_frontier: Option<Arc<typed_trees::TypedTrees>>,
    pub(crate) accounting: crate::evaluation_accounting::EvaluationAccounting,
}

// Accounting is per-attempt scratch, never semantic input. Projection and
// demanded-root clones share it only during the synchronous recorded precheck;
// returned deferred work drops it before any later admission can resume.
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
    pub(crate) fn with_accounting(
        mut self,
        accounting: crate::evaluation_accounting::EvaluationAccounting,
    ) -> Self {
        self.accounting = accounting;
        self
    }

    pub(crate) fn admission(
        &self,
        program: &typed_trees::TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> crate::BuildTimeAdmissionPlan {
        crate::BuildTimeAdmissionPlan::infer(program, authority)
            .with_accounting(self.accounting.clone())
    }
    pub(crate) fn selected(work: DeclarationSelection, execution: DeclarationSelection) -> Self {
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
        domain.layout_targets = self.layout_targets.clone();
        domain.layout_applications = self.layout_applications.clone();
        domain.product_frontier = self.product_frontier.clone();
        domain.accounting = self.accounting.clone();
        domain
    }

    pub(crate) fn execution_program(
        &self,
        original: &typed_trees::TypedTrees,
    ) -> typed_trees::TypedTrees {
        let mut program =
            validation::execution_projection::project_selected_program(original, &self.execution);
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
    BuildTimeSelectionAuthority, PlacedViewRecord, SelectedBuildTimeBinaryOperator,
    SelectedBuildTimeProviderBody, const_evaluation::const_applications,
    const_evaluation::const_domain_facts, const_evaluation::const_generic_calls,
    const_evaluation::const_generic_expressions, const_evaluation::const_initializers,
    const_evaluation::const_lengths, const_evaluation::data_layout_queries,
    const_evaluation::range_arguments, const_evaluation::range_endpoints, layouts::placed_views,
    layouts::plan_laid, layouts::wire_plans,
};

/// One pre-resolution evaluation request. Standalone syntax has no source
/// context; source-bound runs keep loader bindings and authority together.
pub struct BuildTimeEvaluationRequest<'source> {
    pub syntax_trees: syntax_trees::SyntaxTrees,
    pub source_context: BuildTimeSources<'source>,
}

/// Evaluate syntax before resolution and retain the matching typed continuation.
pub fn evaluate_pre_resolution(
    request: BuildTimeEvaluationRequest<'_>,
) -> Result<PreResolutionEvaluation, Vec<diagnostics::Diagnostic>> {
    let BuildTimeEvaluationRequest {
        syntax_trees,
        source_context,
    } = request;
    // Every job sees the same declaration context. Source custody without its
    // retained declarations would turn an ordinary imported constant call into
    // Unknown; restarting just that job on fresh syntax loses the same bodies.
    let syntax_trees = const_initializers::evaluate(syntax_trees, &source_context)?;
    let syntax_trees = const_generic_expressions::evaluate(syntax_trees, &source_context)?;
    let syntax_trees =
        crate::const_evaluation::const_qualifications::prove_ready(syntax_trees, &source_context)?;
    // Canonical ranges must exist before the const-call probe normalizes its
    // syntax clone: that clone's generic synthesis is itself an equation
    // customer, so retained endpoints cannot arrive after it.
    let syntax_trees = range_arguments::evaluate(syntax_trees, &source_context)?;
    let mut syntax_trees =
        const_generic_calls::evaluate_const_generic_calls(syntax_trees, source_context.clone())?;
    syntax_trees_to_symbol_resolved_trees::pre_resolution::synthesize_trait_defaults(
        &mut syntax_trees,
        source_context.sources.clone(),
        source_context.source_scoped_top_level_bindings.to_vec(),
        source_context.retained_base(),
    )?;
    let placed_view_records =
        placed_views::desugar_placed_views(&mut syntax_trees, source_context.clone())?;
    let roots_before_generic = syntax_trees.root_item_handles().len();
    let mut syntax_trees = crate::syntax_probes::normalize_generic_data(
        syntax_trees,
        source_context.sources.clone(),
        source_context.source_scoped_top_level_bindings,
        source_context.retained_base(),
    )?;
    // Generic field extents become owned constant uses only after their exact
    // instance tuple has been copied. Close them through the same expression
    // and invocation admission used by ordinary static arguments.
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
            placed_view_records,
            selection_authority: source_context.selection_authority,
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

/// One source activation's lookup and invocation context. A predecessor lends
/// exact declarations and bodies, not its historical invocation authority.
/// Every syntax job borrows this same context; generated siblings share the
/// predecessor but never participate in each other's normalization.
#[derive(Clone, Default)]
pub struct BuildTimeSources<'a> {
    /// Parsed declaration context, separate from the cohort being elaborated.
    /// Lookup still selects through the receiving namespace and authority.
    pub declaration_syntax: Option<&'a syntax_trees::SyntaxTrees>,
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
        context: BuildTimeSources<'_>,
    ) -> Result<(syntax_trees::SyntaxTrees, Vec<PreCheckEvaluation>), Vec<diagnostics::Diagnostic>>
    {
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
    recorded_authority: Option<validation::RecordedDeclarationAuthority>,
    /// Work roots, never an arena copy: folded types, evidence and fresh
    /// instances publish in the canonical frontend's original storage.
    domain: EvaluationDomain,
    wire_schema_frontier: usize,
    placed_view_records: Vec<PlacedViewRecord>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    /// Exact selected rows supplied only when the deferred continuation is
    /// finished under Omega's settled plans. They stay empty on the early
    /// route so an unselected occurrence can never borrow selected custody.
    selected_operators: Vec<SelectedBuildTimeBinaryOperator>,
    provider_bodies: Vec<SelectedBuildTimeProviderBody>,
}

/// Typed evaluation has installed its plans; target-dependent query folding
/// may now complete the initializer values before their retained calls settle.
#[must_use = "finish the evaluated initializer declarations after query closure"]
pub struct EvaluatedPreCheck {
    domain: EvaluationDomain,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    selected_operators: Vec<SelectedBuildTimeBinaryOperator>,
    provider_bodies: Vec<SelectedBuildTimeProviderBody>,
}

impl EvaluatedPreCheck {
    pub fn finish(
        self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let _g = phase_profile::guard("evaluate_or_defer.finish");
        let selected = SelectedBuildTimeOperators {
            operators: &self.selected_operators,
            provider_bodies: &self.provider_bodies,
        };
        {
            let _g = phase_profile::guard("evaluate_or_defer.finish.settle_target_observations");
            const_initializers::settle_target_observations(
                typed,
                self.selection_authority.clone(),
                &self.domain,
                selected,
            )?;
        }
        let _g = phase_profile::guard("evaluate_or_defer.finish.validate_retained_invocations");
        const_initializers::validate_retained_invocations(
            typed,
            self.selection_authority,
            &self.domain,
            selected,
        )
    }
}

impl PreCheckEvaluation {
    /// A disk candidate has no live authority. Only an equal recorded package
    /// scope and the matching typed source owner permit rebinding it.
    pub fn rejoin_source_authority(
        &mut self,
        program: &typed_trees::TypedTrees,
        authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<(), String> {
        if let Some(recorded) = &self.recorded_authority {
            self.selection_authority = match recorded {
                // A continuation can carry pre-reconciliation jobs beside
                // later package-owned jobs. Fresh evaluation preserves each
                // producer's mode; the enclosing source context separately
                // rejoins its complete current authority. Absence here is not
                // a wildcard in that outer comparison.
                validation::RecordedDeclarationAuthority::Absent => None,
                validation::RecordedDeclarationAuthority::Packages(_) => {
                    // Inherited jobs keep their projected work and executable
                    // roster when another source adds Product declarations.
                    // Compare every requester in both rosters, including any
                    // explicit selected realization outside those roots. An
                    // All execution roster deliberately remains conservative.
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
                        .filter_map(|symbol| program.symbols.symbol_provenance_source_span(symbol))
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
                            checked_trees::CheckedValueOrigin::NestedExpression { parent } => {
                                Some(parent)
                            }
                            _ => None,
                        };
                        for occurrence in std::iter::once(expression)
                            .chain(parent)
                            .chain(operands.iter().copied())
                        {
                            match program.expression_table.try_source_span(occurrence) {
                                Some(site) if site.span.start < site.span.end => sites.push(site),
                                _ => needs_full_scope = true,
                            }
                        }
                    }
                    // A shared synthesized layout can have several callers in
                    // different packages. Each application retains its own
                    // requester even though the policy result has one owner.
                    sites.extend(
                        self.placed_view_records
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
                    // Descriptor lookup is inert, but its original source
                    // custody still participates in the comparison. Keep that
                    // frontier conservative until its exact demands narrow it.
                    if let Some(frontier) = &self.domain.product_frontier {
                        let sources = frontier
                            .symbols
                            .source_map()
                            .ok_or("retained Product descriptor frontier has no source context")?;
                        recorded.require_current(authority.as_deref(), &sources)?;
                    }
                    authority
                }
            };
            self.recorded_authority = None;
        }
        Ok(())
    }

    /// Compare pending work from the same source-preparation owner. An authority
    /// object is opaque: only a shared object can establish the same selection
    /// context here. This is input matching, never permission to execute work.
    pub fn same_pending_work(&self, other: &Self) -> bool {
        self.recorded_authority.is_none()
            && other.recorded_authority.is_none()
            && self.domain == other.domain
            && self.wire_schema_frontier == other.wire_schema_frontier
            && self.placed_view_records == other.placed_view_records
            && self.selected_operators == other.selected_operators
            && self.provider_bodies == other.provider_bodies
            && match (&self.selection_authority, &other.selection_authority) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
    }

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
                layout_targets: self.domain.layout_targets.clone(),
                layout_applications: self.domain.layout_applications.clone(),
                work: self.domain.work.intersect(&selected),
                execution: self.domain.execution.intersect(&selected),
                product_frontier: projection.product_declaration_frontier.clone(),
                accounting: self.domain.accounting.clone(),
            },
        )
    }

    /// A source activation changes the canonical declaration roster, unlike a
    /// private checking projection. Rebase its positional wire checkpoint onto
    /// the selected prefix while retaining exact work and execution owners.
    pub fn activate_sources(
        &self,
        original: &typed_trees::TypedTrees,
        activation: &typed_trees::TypedTrees,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        let mut pending = self.project(original, activation)?;
        let selected = DeclarationSelection::from_program(activation);
        pending.wire_schema_frontier = original.wire_schemas()[..self.wire_schema_frontier]
            .iter()
            .filter(|schema| selected.contains(schema.symbol))
            .count();
        Ok(pending)
    }

    /// Remove work completed by another owned cohort, not its execution context.
    /// The caller retains that cohort's remaining recipes independently. For a
    /// Build projection, checking and fold replay must succeed before excluding
    /// its work; synthesized declarations belong to that completed roster too.
    pub fn exclude_completed_work(
        &self,
        original: &typed_trees::TypedTrees,
        completed_projection: &typed_trees::TypedTrees,
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
        original: &typed_trees::TypedTrees,
        domain: EvaluationDomain,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
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
            recorded_authority: self.recorded_authority.clone(),
            domain,
            wire_schema_frontier: self.wire_schema_frontier,
            placed_view_records,
            selection_authority: self.selection_authority.clone(),
            selected_operators: self.selected_operators.clone(),
            provider_bodies: self.provider_bodies.clone(),
        })
    }

    /// Execute the existing precheck route with explicit invocation accounting.
    /// A deferred continuation does not retain the scratch collector: later
    /// selected-provider work belongs to its own completion/admission.
    pub fn evaluate_or_defer_recorded(
        mut self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<crate::RecordedPreCheck, Vec<diagnostics::Diagnostic>> {
        let accounting = crate::evaluation_accounting::EvaluationAccounting::recording();
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

    /// Keep value-dependent work together until exact boundary execution is
    /// selected. Independent ordinary build inputs retain their early route.
    pub fn evaluate_or_defer(
        mut self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<Option<Self>, Vec<diagnostics::Diagnostic>> {
        if self.recorded_authority.is_some() {
            return Err(vec![diagnostics::Diagnostic::error(
                "restored pre-check has not rejoined current declaration authority",
            )]);
        }
        const_initializers::retain_deferred_entries(typed, &self.domain);
        let defer = {
            let _g = phase_profile::guard("evaluate_or_defer.check_defer");
            phase_profile::scope("evaluate_or_defer.check_defer.independent_lengths", || {
                const_lengths::evaluate_independent_lengths(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )
            })? || phase_profile::scope("evaluate_or_defer.check_defer.endpoint_calls", || {
                range_endpoints::pending_endpoint_calls_need_operator_selection(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )
            })? || phase_profile::scope("evaluate_or_defer.check_defer.memberships", || {
                const_domain_facts::pending_memberships_need_operator_selection(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )
            })? || phase_profile::scope("evaluate_or_defer.check_defer.const_applications", || {
                const_applications::pending_const_applications_need_operator_selection(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )
            })? || data_layout_queries::has_pending_target_observations(typed, self.domain.work())
        };
        if defer {
            let _g = phase_profile::guard("evaluate_or_defer.defer_path");
            range_endpoints::defer_pending_endpoint_calls_in(typed, &self.domain)?;
            const_applications::defer_pending_const_applications(typed, &self.domain)?;
            // A const-generic application of a recipe call becomes a symbolic
            // instance whose extents carry the call. Its argument keeps the
            // pending mark set above: the instance's recorded origin still
            // spells the application, and no consumer may evaluate it early.
            const_applications::realize_symbolic_applications(typed, &mut self.domain)?;
            // The shared check admits exactly these positions as recipes and
            // the target closure reads their values back by the same roster.
            crate::const_evaluation::constant_recipes::retain_deferred_constant_recipes(
                typed,
                &self.domain,
            )?;
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
        mut wire_schema_frontier: usize,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<Vec<Self>, Vec<diagnostics::Diagnostic>> {
        let mut pending = Vec::new();
        for mut pre_check in pre_checks {
            pre_check.wire_schema_frontier = wire_schema_frontier;
            if let Some(pre_check) = pre_check.evaluate_or_defer(typed)? {
                pending.push(pre_check);
            }
            wire_schema_frontier = typed.wire_schemas().len();
        }
        // Each admitted cohort owns its evaluation work. Do not replay the
        // preceding typing stage's whole-tree storage comparison here.
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
    pub(crate) fn with_layout_targets(
        mut self,
        targets: &[layout_plans::LayoutTargetFacts],
    ) -> Self {
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

    pub fn evaluate_selected_operators(
        mut self,
        typed: &mut typed_trees::TypedTrees,
        selected: SelectedBuildTimeOperators<'_>,
    ) -> Result<EvaluatedPreCheck, Vec<diagnostics::Diagnostic>> {
        if self.recorded_authority.is_some() {
            return Err(vec![diagnostics::Diagnostic::error(
                "restored pre-check has not rejoined current declaration authority",
            )]);
        }
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

    /// Consume the exact continuation produced before name resolution.
    ///
    /// Omega may target-filter and type the returned syntax before this call,
    /// but the language-level evaluation order and selection authority remain
    /// owned by this continuation.
    pub fn evaluate(
        self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if self.recorded_authority.is_some() {
            return Err(vec![diagnostics::Diagnostic::error(
                "restored pre-check has not rejoined current declaration authority",
            )]);
        }
        self.evaluate_work(typed)?.finish(typed)
    }

    fn evaluate_work(
        mut self,
        typed: &mut typed_trees::TypedTrees,
    ) -> Result<EvaluatedPreCheck, Vec<diagnostics::Diagnostic>> {
        let _g = phase_profile::guard("evaluate_or_defer.evaluate_work");
        const_initializers::retain_deferred_entries(typed, &self.domain);
        phase_profile::scope(
            "evaluate_or_defer.evaluate_work.const_array_lengths",
            || {
                const_lengths::evaluate_const_array_lengths_in(
                    typed,
                    self.selection_authority.clone(),
                    &self.domain,
                )
            },
        )?;
        phase_profile::scope("evaluate_or_defer.evaluate_work.range_endpoints", || {
            range_endpoints::evaluate_selected_range_endpoints(
                typed,
                self.selection_authority.clone(),
                SelectedBuildTimeOperators {
                    operators: &self.selected_operators,
                    provider_bodies: &self.provider_bodies,
                },
                &self.domain,
            )
        })?;
        phase_profile::scope("evaluate_or_defer.evaluate_work.domain_facts", || {
            const_domain_facts::evaluate_selected_domain_facts(
                typed,
                self.selection_authority.clone(),
                SelectedBuildTimeOperators {
                    operators: &self.selected_operators,
                    provider_bodies: &self.provider_bodies,
                },
                &self.domain,
            )
        })?;
        phase_profile::scope("evaluate_or_defer.evaluate_work.const_applications", || {
            const_applications::evaluate_selected_const_applications(
                typed,
                self.selection_authority.clone(),
                SelectedBuildTimeOperators {
                    operators: &self.selected_operators,
                    provider_bodies: &self.provider_bodies,
                },
                &mut self.domain,
            )
        })?;
        phase_profile::scope("evaluate_or_defer.evaluate_work.plan_laid_layouts", || {
            plan_laid::compute_plan_laid_layouts_in(
                typed,
                self.selection_authority.clone(),
                &self.domain,
            )
        })?;
        phase_profile::scope("evaluate_or_defer.evaluate_work.placed_view_plans", || {
            placed_views::validate_placed_view_plans_in(
                typed,
                &self.placed_view_records,
                self.selection_authority.clone(),
                &self.domain,
            )
        })?;
        phase_profile::scope("evaluate_or_defer.evaluate_work.wire_plans", || {
            wire_plans::compute_wire_plans_in(
                typed,
                self.selection_authority.clone(),
                self.wire_schema_frontier,
                &self.domain,
            )
        })?;
        // Every position this evaluation folded leaves the recipe roster;
        // only still-symbolic positions remain for a later target closure.
        typed.retain_symbolic_constant_recipes();
        // Installed view plans bind their accessor targets before any later
        // continuation consumes them; initializer settlement waits for queries.
        phase_profile::scope(
            "evaluate_or_defer.evaluate_work.dynamic_call_targets",
            || validation::resolve_selected_dynamic_call_targets(typed, self.domain.work()),
        )?;
        Ok(EvaluatedPreCheck {
            domain: self.domain,
            selection_authority: self.selection_authority,
            selected_operators: self.selected_operators,
            provider_bodies: self.provider_bodies,
        })
    }
}

impl compiler_storage::Record for PreCheckEvaluation {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            self.domain.write(output)?;
            self.wire_schema_frontier.write(output)?;
            self.placed_view_records.write(output)?;
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
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            Ok(Self {
                domain: compiler_storage::Record::read(input)?,
                wire_schema_frontier: compiler_storage::Record::read(input)?,
                placed_view_records: compiler_storage::Record::read(input)?,
                recorded_authority: Some(compiler_storage::Record::read(input)?),
                selection_authority: None,
                selected_operators: compiler_storage::Record::read(input)?,
                provider_bodies: compiler_storage::Record::read(input)?,
            })
        })
    }
}
