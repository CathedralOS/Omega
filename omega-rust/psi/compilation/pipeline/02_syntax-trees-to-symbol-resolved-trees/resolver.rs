//! Resolve parsed Omega names to declaration identities. Start with `resolve` for
//! a whole forest or `resolve_deferred` for a build that will request bodies.
//! Generated source extends a retained base. Build-time queries live in `pre_resolution`.
//! The full pass order is in `resolve_forest` and `settle_resolution` below.
#![forbid(unsafe_code)]

mod constants;
mod declarations;
mod deferred_bodies;
mod generated_source;
mod lowering;
mod names;
pub mod pre_resolution;
mod selections;
mod tables;

use crate::declarations::declaration_selection::ConstantSelection;
use crate::names::target_siblings::bind_sibling_calls;
use crate::names::type_predicates::retain_operands;
use declarations::machine_equations::SourceLinks;
use declarations::machine_equations::complete;
use declarations::machine_equations::required;
use declarations::machine_equations::validate_declarations;
pub use deferred_bodies::{DeferredResolvedFrontier, ResolvedBodyCompletion};
use diagnostics::Diagnostic;
pub use generated_source::{
    DeferredResolvedExtension, ExtensionRequest, resolve_deferred_extension, resolve_extension,
};
use lowering::lowerer::{ConstResolutionMode, Lowerer};
use lowering::machine::reject_duplicate_direct_token_bindings;
use lowering::machine::resolve_token_bound_domain_homes;
use lowering::mathematical::lower_mathematical_definitions;
use selections::body_calls::CallSelectionInput;
use selections::conformance_blocks::route_conformance_requirement_calls;
use selections::conformance_blocks::route_named_implementation_calls;
use source::SourceMap;
use std::mem::take;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolHandle;
use syntax_trees::SyntaxTrees;

/* Resolve every name in the forest to its exact declaration. */
pub fn resolve(request: ResolutionRequest) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let (syntax, lowerer) = begin_complete(request, None)?;
    resolve_forest(lowerer, &syntax).map(|(trees, _, _)| trees)
}

/*
    Index declarations without granting their implementations execution. The
    returned owner retains the exact prepared source for subsequent body demand.
*/
pub fn resolve_deferred(
    request: ResolutionRequest,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    resolve_deferred_with_declarations(request, None)
}

/*
    The route. Each phase is one call into its owner, in the one order their
    preconditions allow: operator homes need no symbols; constants need the
    table; the authored-selection ledger needs substituted constants; operator
    obligations need the ledger; every remaining selection needs all of it;
    domain homes of token-bearing machines need assigned attached symbols;
    duplicate machine token bindings compare settled operand identities.
*/
fn resolve_forest(
    mut lowerer: Lowerer,
    syntax: &SyntaxTrees,
) -> Result<
    (
        SymbolResolvedTrees,
        ConstantSelection<'static>,
        Option<SourceLinks>,
    ),
    Vec<Diagnostic>,
> {
    lowering::lower_items(&mut lowerer, syntax)?;
    lower_mathematical_definitions(&mut lowerer, syntax)?;
    selections::select_operator_homes(&mut lowerer)?;
    let constant_selection = lowerer.take_constant_selection()?;
    crate::names::assign(&mut lowerer, syntax)?;
    finish_resolution(&mut lowerer, syntax, &[], &[])?;
    let links = lowerer.equation_sources.take();
    Ok((lowerer.into_trees(), constant_selection, links))
}

/*
    One syntax forest and the custody it resolves under.

    The forest arrives by value: preparation
    rewrites syntax to syntax before any symbol exists, and only the rewritten
    forest carries the declarations the symbol table assigns identities to.
    Taking ownership keeps that rewrite from being paid for with a copy of the
    whole program and keeps orchestration code from retaining the unprepared
    sibling as if a later stage could still read it.
*/
pub struct ResolutionRequest {
    pub syntax: SyntaxTrees,
    /*
        Source custody for visibility and stratum checks. A source-free forest
        has none, and those checks stay permissive for it.
    */
    pub sources: Option<Arc<SourceMap>>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    /*
        Immutable declarations selected by this source activation. Early probes
        and complete resolution use the same namespace and executable bodies;
        retaining source coordinates alone cannot supply either.
    */
    pub retained_base: Option<Arc<SymbolResolvedTrees>>,
}

impl ResolutionRequest {
    /* A source-free forest with no scoped top-level bindings. */
    pub fn new(syntax: SyntaxTrees) -> Self {
        Self {
            syntax,
            sources: None,
            top_level_bindings: Vec::new(),
            retained_base: None,
        }
    }
}
fn resolve_deferred_with_declarations(
    request: ResolutionRequest,
    base_declarations: Option<&SyntaxTrees>,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    let base = request.retained_base.clone();
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (mut syntax, mut lowerer) =
        begin_with_declarations(request, ConstResolutionMode::Complete, base_declarations)?;
    if required(&syntax, base.as_deref()) {
        loop {
            let mut provisional = Lowerer::new(sources.clone(), bindings.clone());
            provisional.constant_selection = lowerer.constant_selection.clone();
            provisional.const_resolution_mode = ConstResolutionMode::ArgumentSelection;
            provisional.equation_sources = Some(Default::default());
            if let Some(base) = &base {
                provisional.seed_resolved_base(Arc::clone(base))?;
            }
            index_declarations(&mut provisional, &syntax)?;
            finish_resolution(&mut provisional, &syntax, &[], &[])?;
            let selection = provisional.take_constant_selection()?;
            let links = provisional.equation_sources.take().unwrap_or_default();
            let before = syntax.root_item_handles().len();
            complete(
                &mut syntax,
                &provisional.symbol_resolved_trees,
                &links,
                &selection,
            )?;
            if syntax.root_item_handles().len() == before {
                break;
            }
        }
        lowerer.equation_sources = Some(Default::default());
    }
    lowerer.structural_type_equations_pending = false;
    index_deferred(syntax, lowerer, sources, bindings)
}

fn index_deferred(
    syntax: SyntaxTrees,
    mut lowerer: Lowerer,
    sources: Option<Arc<SourceMap>>,
    bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    index_declarations(&mut lowerer, &syntax)?;
    finish_resolution(&mut lowerer, &syntax, &[], &[])?;
    let pending = take(&mut lowerer.pending_machine_bodies);
    let pending_type_inputs =
        lowering::type_inputs::bind(&lowerer.symbol_resolved_trees, &lowerer.pending_type_inputs)?;
    let selection = lowerer.take_constant_selection()?;
    let hoist_counter = lowerer.hoist_counter;
    let const_resolution_mode = lowerer.const_resolution_mode;
    let structural_type_equations_pending = lowerer.structural_type_equations_pending;
    let equation_sources = lowerer.equation_sources.take().map(|mut sources| {
        sources.expressions.clear();
        sources.statements.clear();
        sources
    });
    let supplied_operand_owners = take(&mut lowerer.supplied_operand_owners);
    let declaration_inputs = take(&mut lowerer.declaration_inputs);
    Ok(DeferredResolvedFrontier {
        syntax: Arc::new(syntax),
        trees: Arc::new(lowerer.into_trees()),
        pending,
        pending_type_inputs,
        supplied_operand_owners,
        declaration_inputs,
        selection,
        sources,
        bindings,
        hoist_counter,
        const_resolution_mode,
        structural_type_equations_pending,
        equation_sources,
    })
}

fn index_declarations(lowerer: &mut Lowerer, syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    lowerer.defer_machine_bodies = true;
    lowering::lower_items(lowerer, syntax)?;
    lower_mathematical_definitions(lowerer, syntax)?;
    selections::select_operator_homes(lowerer)?;
    crate::names::assign_declarations(lowerer, syntax)
}

/*
    Equation completion uses ordinary selection with identical source and base
    custody. Its private links are discarded before the final lowering pass.
*/
fn begin_complete(
    request: ResolutionRequest,
    base_declarations: Option<&SyntaxTrees>,
) -> Result<(SyntaxTrees, Lowerer), Vec<Diagnostic>> {
    let base = request.retained_base.clone();
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (mut syntax, mut lowerer) =
        begin_with_declarations(request, ConstResolutionMode::Complete, base_declarations)?;
    if required(&syntax, base.as_deref()) {
        let mut provisional = Lowerer::new(sources, bindings);
        provisional.constant_selection = lowerer.constant_selection.take();
        provisional.const_resolution_mode = ConstResolutionMode::ArgumentSelection;
        provisional.equation_sources = Some(Default::default());
        if let Some(base) = &base {
            provisional.seed_resolved_base(Arc::clone(base))?;
        }
        let (trees, selection, links) = resolve_forest(provisional, &syntax)?;
        complete(&mut syntax, &trees, &links.unwrap_or_default(), &selection)?;
        lowerer.constant_selection = Some(selection);
    }
    lowerer.structural_type_equations_pending = false;
    Ok((syntax, lowerer))
}

/*
    Prepare the forest under its constant selector and a lowerer that owns the
    request's custody.
*/
fn begin(
    request: ResolutionRequest,
    constants: ConstResolutionMode,
) -> Result<(SyntaxTrees, Lowerer), Vec<Diagnostic>> {
    begin_with_declarations(request, constants, None)
}

/*
    `begin` for an extension whose retained base was resolved from
    `base_declarations`: alias expansion reads the base's authored alias
    declarations there.
*/
fn begin_with_declarations(
    request: ResolutionRequest,
    constants: ConstResolutionMode,
    base_declarations: Option<&SyntaxTrees>,
) -> Result<(SyntaxTrees, Lowerer), Vec<Diagnostic>> {
    let ResolutionRequest {
        syntax,
        sources,
        top_level_bindings,
        retained_base,
    } = request;
    let prepared = declarations::prepare(
        syntax,
        sources.clone(),
        top_level_bindings.clone(),
        constants,
        retained_base.clone(),
        base_declarations,
    )?;
    validate_declarations(&prepared.syntax, Some(&prepared.constant_selection))?;
    let mut lowerer = Lowerer::new(sources, top_level_bindings);
    lowerer.constant_selection = Some(prepared.constant_selection);
    lowerer.const_resolution_mode = constants;
    if let Some(base) = retained_base {
        lowerer.seed_resolved_base(base)?;
    }
    Ok((prepared.syntax, lowerer))
}

/*
    One normalization route for new declarations and completed implementations.
    Source activation can shrink roots while retaining backing coordinates;
    rebuilding those tables would move the typed continuation's ledger frontier.
    Exact completed owners append their newly available bodies beside that prefix.
*/
fn finish_resolution(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    inputs: &[CallSelectionInput],
    completed_machines: &[SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    let predecessor = lowerer.seed.as_ref().map(|seed| Arc::clone(&seed.source));
    settle_resolution(lowerer, syntax)?;
    let mut completed_machines = completed_machines.to_vec();
    completed_machines.extend_from_slice(&lowerer.completed_machines);
    let symbol_key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
    let mut completed_keys = completed_machines
        .iter()
        .map(|symbol| symbol_key(*symbol))
        .collect::<Vec<_>>();
    completed_keys.sort_unstable();
    let machines = lowerer
        .symbol_resolved_trees
        .machines
        .iter()
        .filter(|machine| {
            predecessor
                .as_ref()
                .is_none_or(|previous| previous.machine(machine.symbol).is_none())
                || completed_keys
                    .binary_search(&symbol_key(machine.symbol))
                    .is_ok()
        })
        .map(|machine| machine.symbol)
        .collect::<Vec<_>>();
    match predecessor {
        Some(predecessor) => lowerer
            .symbol_resolved_trees
            .append_completed_body_tables(&predecessor, &completed_machines),
        None => lowerer.symbol_resolved_trees.rebuild_tables(),
    }
    finish_resolved_tables(lowerer, inputs, &machines)?;
    selections::callable_inference::infer(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_callable_types,
    )?;
    lowerer.pending_callable_types.clear();
    Ok(())
}

/*
    Shared name, obligation and declaration settlement precedes both complete
    table normalization and append-only authored implementation completion.
*/
fn settle_resolution(lowerer: &mut Lowerer, syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    deferred_bodies::complete_callable_type_dependencies(lowerer, syntax)?;
    constants::finalize_const_declarations(
        syntax,
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_declarations,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    deferred_bodies::complete_initializer_receipt_dependencies(lowerer, syntax)?;
    resolve_token_bound_domain_homes(&mut lowerer.symbol_resolved_trees);
    constants::finalize(lowerer)?;
    selections::finalize_authored_selections(lowerer)?;
    let expressions = lowerer
        .pending_authored_expressions
        .iter()
        .map(|pending| pending.expression)
        .collect::<Vec<_>>();
    retain_operands(&mut lowerer.symbol_resolved_trees, &expressions)?;

    constants::finalize_operator_obligations(lowerer)?;
    selections::finalize(lowerer)?;
    reject_duplicate_direct_token_bindings(&mut lowerer.symbol_resolved_trees)?;
    Ok(())
}

fn finish_resolved_tables(
    lowerer: &mut Lowerer,
    inputs: &[CallSelectionInput],
    machines: &[SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    route_conformance_requirement_calls(&mut lowerer.symbol_resolved_trees, inputs, machines)?;
    route_named_implementation_calls(&mut lowerer.symbol_resolved_trees, inputs, machines);
    crate::names::target_machine_families::bind(&mut lowerer.symbol_resolved_trees)?;
    bind_sibling_calls(&mut lowerer.symbol_resolved_trees, inputs, machines);
    crate::names::recursive_tail_calls::normalize(&mut lowerer.symbol_resolved_trees, machines);
    crate::names::target_machine_families::prepare_premises(&mut lowerer.symbol_resolved_trees)?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/support/frontend.rs"]
mod frontend_fixture;
#[cfg(test)]
mod tests;
