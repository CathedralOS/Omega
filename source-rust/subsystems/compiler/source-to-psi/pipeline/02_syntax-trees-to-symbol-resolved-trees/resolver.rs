//! Resolve parsed declarations and retain their pending machine bodies. Generated
//! and authored extensions use the same declaration index and body completion.
#![forbid(unsafe_code)]

mod constants;
mod declarations;
mod deferred_bodies;
mod generated_source;
mod lowering;
mod names;
#[cfg(any(test, feature = "test-support"))]
pub mod pre_resolution;
mod selections;
mod tables;

use crate::resolution::names::target_siblings::bind_sibling_calls;
use crate::resolution::names::type_predicates::retain_operands;
use crate::source_inputs::SourceMap;
use crate::source_resolved::SymbolResolvedTrees;
pub use declarations::dispatches::prepare as prepare_dispatches;
use declarations::machine_equations::complete;
use declarations::machine_equations::required;
use declarations::machine_equations::validate_declarations;
pub use deferred_bodies::{DeferredBodyScope, DeferredResolvedFrontier, ResolvedBodyCompletion};
use diagnostics::Diagnostic;
#[cfg(any(test, feature = "test-support"))]
pub use generated_source::DeferredResolvedExtension;
pub use generated_source::{ExtensionRequest, resolve_deferred_extension};
#[cfg(any(test, feature = "test-support"))]
pub use generated_source::resolve_extension;
use lowering::lowerer::{ConstResolutionMode, Lowerer};
use lowering::mathematical::lower_mathematical_definitions;
use selections::conformance_blocks::route_conformance_requirement_calls;
use selections::conformance_blocks::route_named_implementation_calls;
use names::domain_attachments::resolve_machine_domain_homes;
use selections::machine_token_bindings::reject_duplicate_direct_token_bindings;
use std::mem::take;
use std::sync::Arc;
use symbols::SymbolHandle;
use syntax_trees::SyntaxTrees;

/// Supply the named file edges a package driver would admit for a stage fixture.
/// Both paths name files in the fixture's source map, independent of insertion order.
#[cfg(any(test, feature = "test-support"))]
pub fn fixture_file_bindings(
    sources: &SourceMap,
    edges: &[(&str, &str, &str)],
) -> Vec<crate::source_symbols::SourceScopedTopLevelBinding> {
    let id = |path: &str| {
        sources
            .files()
            .find(|file| file.path == std::path::Path::new(path))
            .unwrap_or_else(|| panic!("missing fixture source {path}"))
            .source_id
    };
    edges
        .iter()
        .map(|&(requester, target, alias)| {
            crate::source_symbols::SourceScopedTopLevelBinding::file_import(
                id(requester),
                id(target),
                alias,
            )
        })
        .collect()
}

/* Resolve every name in the forest to its exact declaration. */
#[cfg(any(test, feature = "test-support"))]
pub fn resolve(request: ResolutionRequest) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    resolve_deferred(request)?.into_complete_trees()
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
    pub top_level_bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
    /*
        Immutable declarations selected by this source activation. Early probes
        and complete resolution use the same namespace and executable bodies;
        retaining source coordinates alone cannot supply either.
    */
    pub retained_base: Option<Arc<SymbolResolvedTrees>>,
}

impl ResolutionRequest {
    /* A source-free forest with no scoped top-level bindings. */
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(syntax: SyntaxTrees) -> Self {
        Self {
            syntax,
            sources: None,
            top_level_bindings: Vec::new(),
            retained_base: None,
        }
    }
}
/*
    Index declarations without granting their implementations execution. The
    returned owner retains the exact prepared source for subsequent body demand.
*/
pub fn resolve_deferred(
    request: ResolutionRequest,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    if let Some(sources) = request.sources.as_deref() {
        sources
            .validate_logical_filenames()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
    }
    let base = request.retained_base.clone();
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (mut syntax, mut lowerer) = begin(request, ConstResolutionMode::Complete)?;
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
            finish_resolution(&mut provisional, &syntax, &[])?;
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
    bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    index_declarations(&mut lowerer, &syntax)?;
    finish_resolution(&mut lowerer, &syntax, &[])?;
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
    crate::resolution::names::assign_declarations(lowerer, syntax)
}

/*
    Prepare the forest under its constant selector and a lowerer that owns the
    request's custody.
*/
fn begin(
    request: ResolutionRequest,
    constants: ConstResolutionMode,
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
    finish_resolved_tables(lowerer, &machines)?;
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
    resolve_machine_domain_homes(&mut lowerer.symbol_resolved_trees)?;
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
    crate::resolution::names::member_namespaces::reject_member_collisions(
        &lowerer.symbol_resolved_trees,
    )?;
    crate::resolution::names::public_interfaces::reject_private_types_in_public_signatures(
        &lowerer.symbol_resolved_trees,
    )?;
    Ok(())
}

fn finish_resolved_tables(
    lowerer: &mut Lowerer,
    machines: &[SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    route_conformance_requirement_calls(&mut lowerer.symbol_resolved_trees, machines)?;
    route_named_implementation_calls(&mut lowerer.symbol_resolved_trees, machines);
    crate::resolution::names::target_machine_families::bind(&mut lowerer.symbol_resolved_trees)?;
    bind_sibling_calls(&mut lowerer.symbol_resolved_trees, machines);
    crate::resolution::names::recursive_tail_calls::normalize(
        &mut lowerer.symbol_resolved_trees,
        machines,
    );
    crate::resolution::names::target_machine_families::prepare_premises(
        &mut lowerer.symbol_resolved_trees,
    )?;
    Ok(())
}

#[cfg(test)]
use crate::frontend_fixture;
#[cfg(test)]
mod tests;
