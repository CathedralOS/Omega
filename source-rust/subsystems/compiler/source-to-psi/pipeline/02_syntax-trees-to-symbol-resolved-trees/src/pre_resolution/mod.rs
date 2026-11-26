//! Select declarations needed by build-time evaluation before full resolution.
mod initializers;

pub use crate::resolution::constants::initializer_dependencies::{
    ConstInitializerDependencies, ResolvedMachineBodyDemand,
};
pub use crate::resolution::constants::initializer_normalization::InitializerCustodyPurpose;
pub use crate::resolution::constants::requires_const_initializer_evaluation;
use crate::resolution::declarations::generic_data::const_destination_requires_proof;
use crate::resolution::declarations::generic_data::domain_heads::normalize;
use crate::resolution::declarations::machine_equations::validate_declarations;
pub use crate::resolution::declarations::trait_defaults::synthesize_trait_defaults;
use crate::resolution::declarations::trait_defaults::synthesize_trait_defaults_with_selection;
use crate::resolution::lowering::lowerer::{ConstResolutionMode, Lowerer};
use crate::resolution::names::assign_lexical_inputs;
use crate::resolution::tables::body_expressions;
use crate::resolution::{
    DeferredResolvedFrontier, ResolutionRequest, begin, constants, declarations, finish_resolution,
    index_declarations, index_deferred,
};
use crate::source_inputs::SourceMap;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::expression::ExpressionHandle;
use crate::source_resolved::expression::ExpressionNode;
use diagnostics::Diagnostic;
pub use initializers::{
    ConstDestinationObligation, ConstInitializerPayload, ConstInitializerSelection,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::ConstDefinition;

/*
    Normalize numeric-probe declarations while retaining equation obligations
    and original implementations. The caller demands bodies by exact owner.
*/
pub fn resolve_deferred_numeric_probe(
    request: ResolutionRequest,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (syntax, lowerer) = begin(request, ConstResolutionMode::Complete)?;
    index_deferred(syntax, lowerer, sources, bindings)
}

/*
    One lexical selection drive beside its complete retained lookup catalog.
    Predecessor expressions are lookup history, not fresh selections in this
    drive. In particular, local type operands live outside deferred runtime
    bodies, so a traversal of executable roots cannot recover this inventory.
    The resolution producer retains its classified authored expressions instead.
    This product grants neither body typing nor execution authority.
*/
pub struct ConstArgumentSelection {
    program: SymbolResolvedTrees,
    expressions: Vec<ExpressionHandle>,
}

impl ConstArgumentSelection {
    pub fn original_call_target(&self, expression: ExpressionHandle) -> Option<SymbolHandle> {
        if !self.expressions.contains(&expression) {
            return None;
        }
        match self
            .program
            .tables
            .bodies
            .expressions
            .expression(expression)
        {
            ExpressionNode::Call(call) => {
                call.authored_call_binding.map(|binding| binding.original)
            }
            _ => None,
        }
    }

    pub fn program(&self) -> &SymbolResolvedTrees {
        &self.program
    }

    pub fn expressions(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.expressions.iter().copied().map(|expression| {
            (
                expression,
                body_expressions(&self.program).expression(expression),
            )
        })
    }
}

/*
    Resolve raw index expressions in their authored owners through normal
    lexical selection. This performs no generic instance synthesis or evaluation;
    callers must still admit every selected leaf and checked operator meaning.
    Local headers and nested type inputs borrow their original preceding-local
    scope; runtime bodies remain Deferred. This is a one-shot selection carrier,
    not a continuation whose declaration scratch can be executed or completed.
*/
pub fn resolve_const_argument_selection(
    request: ResolutionRequest,
) -> Result<ConstArgumentSelection, Vec<Diagnostic>> {
    let (syntax, mut lowerer) = begin(request, ConstResolutionMode::ArgumentSelection)?;
    index_declarations(&mut lowerer, &syntax)?;
    assign_lexical_inputs(&mut lowerer, &syntax)?;
    let expressions: Vec<_> = lowerer
        .pending_authored_expressions
        .iter()
        .map(|authored| authored.expression)
        .collect();
    finish_resolution(&mut lowerer, &syntax, &[])?;
    let program = lowerer.into_trees();
    Ok(ConstArgumentSelection {
        program,
        expressions,
    })
}

/*
    Retain unfinished constant values while resolving a declaration-check probe.
    No pending initializer becomes a substituted value or public identity. The
    consumer demands only its exact completed probe roots; their ordinary typed
    checking cannot read an unavailable initializer as a completed constant.
*/
pub fn resolve_deferred_initializer_probe(
    request: ResolutionRequest,
) -> Result<DeferredResolvedFrontier, Vec<Diagnostic>> {
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (syntax, lowerer) = begin(request, ConstResolutionMode::InitializerSelection)?;
    index_deferred(syntax, lowerer, sources, bindings)
}

/*
    Resolve declaration dependencies without inventing provisional values.
    Computed fixed-integer/Boolean initializers remain authored expression roots
    with no canonical encoding. Every operator occurrence remains an obligation
    for ordinary typed selection, not an assertion of builtin execution meaning.
    Only helpers reached from those roots acquire bodies. Dependency collection
    still requires every reached implementation to exist; deferred unrelated
    machines cannot affect preparation or become executable through this carrier.
*/
pub fn prepare_const_initializer_selection(
    request: ResolutionRequest,
) -> Result<ConstInitializerSelection, Vec<Diagnostic>> {
    let sources = request.sources.clone();
    let bindings = request.top_level_bindings.clone();
    let (syntax, lowerer) = begin(request, ConstResolutionMode::InitializerSelection)?;
    finish_const_initializer_selection(syntax, lowerer, sources, bindings)
}

/*
    Discover destination judgments before demanding their declaration graph.
    These are the original destinations, before the private forest deferred their
    qualifications. Selection and leaf classification only schedule work: every
    selected destination still returns through ordinary typed checking.
*/
pub fn prepare_const_qualification_selection<'authored>(
    request: ResolutionRequest,
    authored: &'authored SyntaxTrees,
    destinations: impl IntoIterator<Item = &'authored ConstDefinition>,
) -> Result<Option<ConstInitializerSelection>, Vec<Diagnostic>> {
    let destinations = destinations.into_iter().collect::<Vec<_>>();
    if destinations.is_empty() {
        return Ok(None);
    }
    let ResolutionRequest {
        mut syntax,
        sources,
        top_level_bindings,
        retained_base,
    } = request;
    declarations::prepare_statement_bodies(&mut syntax)?;
    declarations::target_families::normalize(&mut syntax, sources.as_deref())?;
    let constant_selection =
        declarations::declaration_selection::ConstantSelection::with_shared_base(
            &syntax,
            sources.clone(),
            top_level_bindings.clone(),
            retained_base.clone(),
        )?;
    normalize(&mut syntax, Some(&constant_selection))?;
    let mut demanded = false;
    for definition in destinations {
        if (definition.preparation().pending_qualification.is_some()
            || const_destination_requires_proof(authored, definition, Some(&constant_selection)))
            && constants::pending_const_initializer_leaves(
                authored,
                definition,
                &constant_selection,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
            })?
            .is_empty()
        {
            demanded = true;
            break;
        }
    }
    if !demanded {
        return Ok(None);
    }
    synthesize_trait_defaults_with_selection(&mut syntax, &constant_selection)?;
    validate_declarations(&syntax, Some(&constant_selection))?;
    let mut lowerer = Lowerer::new(sources.clone(), top_level_bindings.clone());
    lowerer.constant_selection = Some(constant_selection);
    lowerer.const_resolution_mode = ConstResolutionMode::InitializerSelection;
    if let Some(base) = retained_base {
        lowerer.seed_resolved_base(base)?;
    }
    finish_const_initializer_selection(syntax, lowerer, sources, top_level_bindings).map(Some)
}

fn finish_const_initializer_selection(
    syntax: SyntaxTrees,
    lowerer: Lowerer,
    sources: Option<Arc<SourceMap>>,
    bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
) -> Result<ConstInitializerSelection, Vec<Diagnostic>> {
    let mut frontier = index_deferred(syntax, lowerer, sources, bindings)?;
    let initializers = frontier
        .trees()
        .const_declarations
        .iter()
        .map(|declaration| {
            if declaration.authored_initializer.is_valid() {
                declaration.authored_initializer
            } else {
                declaration.initializer
            }
        })
        .filter(|initializer| initializer.is_valid())
        .collect::<Vec<_>>();
    loop {
        let demand = frontier.body_demand(&[], &initializers, None)?;
        if demand.deferred_machines.is_empty() {
            break;
        }
        frontier = frontier
            .complete_machine_bodies(&demand.deferred_machines)?
            .into_resolved_frontier();
    }
    let preparation = ConstInitializerSelection { frontier };
    preparation.validate_initializer_leaves(&preparation.frontier.syntax)?;
    Ok(preparation)
}
