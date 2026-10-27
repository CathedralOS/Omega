//! Declared type observations for one exact selected ordinary call.
//!
//! Resolution owns the declaration selection; lowering records handle links to
//! the corresponding authored type. These links survive provisional resolution
//! without reselecting by a source coordinate or a leaf spelling. An initializer,
//! literal element, flow predicate or private postcondition supplies no type.
//! Open callee results first consume the exact selected argument substitution
//! from the shared structural solver. Unapplied/underdetermined results remain
//! obligations; this reader never inspects a callee body or replaces the later
//! compatibility, formation, property and invocation-contract checks.

use super::{ConstantSelection, SourceLinks};
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use symbol_resolved_trees::{SymbolResolvedTrees, expression, statement, types};
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::{SyntaxTrees, types::TypeReferenceHandle};

mod projections;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclaredTypeOrigin {
    Parameter(Handle<symbol_resolved_trees::signature::StateParameter>),
    Result(Handle<symbol_resolved_trees::state::State>),
    Field(Handle<symbol_resolved_trees::data::DataMember>),
    Payload(Handle<symbol_resolved_trees::data::DataField>),
    Constant(usize),
}

impl DeclaredTypeOrigin {
    fn declaration(
        self,
        trees: &SymbolResolvedTrees,
    ) -> Option<(SymbolHandle, &types::TypeReference)> {
        let declarations = &trees.tables.declarations;
        match self {
            Self::Parameter(handle) => {
                let parameter = declarations.state_parameters.get(handle);
                Some((parameter.symbol, &parameter.type_reference))
            }
            Self::Result(handle) => {
                let state = trees.machine_state(handle);
                Some((state.symbol, state.return_type.as_ref()?))
            }
            Self::Field(handle) => {
                let symbol_resolved_trees::data::DataMember::Field(field) =
                    declarations.data_members.get(handle)
                else {
                    return None;
                };
                Some((field.symbol, &field.type_reference))
            }
            Self::Payload(handle) => {
                let field = declarations.data_payload_fields.get(handle);
                Some((field.symbol, &field.type_reference))
            }
            Self::Constant(index) => {
                let constant = &trees.roots.const_declarations[index];
                Some((constant.symbol, &constant.declared_type))
            }
        }
    }
}

fn closed_type(trees: &SymbolResolvedTrees, root: &types::TypeReference) -> bool {
    let binder = |symbol: SymbolHandle| {
        symbol.is_valid()
            && matches!(
                trees.symbols.get(symbol).kind,
                SymbolKind::TypeParameter
                    | SymbolKind::Parameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::ConformanceParameter
            )
    };
    let mut pending = vec![root];
    while let Some(reference) = pending.pop() {
        match reference {
            types::TypeReference::Named { symbol, .. }
            | types::TypeReference::DynamicTrait { symbol, .. } => {
                if binder(*symbol) {
                    return false;
                }
            }
            types::TypeReference::Reference(reference) => {
                pending.push(trees.child_type_reference(reference.referee))
            }
            types::TypeReference::Slice(slice) => {
                pending.push(trees.child_type_reference(slice.element_type))
            }
            types::TypeReference::FixedArray(array) => {
                match &array.length {
                    types::FixedArrayLength::Literal(_) => {}
                    types::FixedArrayLength::ConstParameter { symbol, .. }
                        if symbol.is_valid()
                            && trees.symbols.get(*symbol).kind == SymbolKind::Const => {}
                    _ => return false,
                }
                pending.push(trees.child_type_reference(array.element_type));
            }
            types::TypeReference::Constrained(constrained) => {
                pending.push(trees.child_type_reference(constrained.base_type));
                for constraint in trees
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                {
                    if let types::TypeConstraint::Domain(domain) = constraint {
                        pending.extend(
                            domain
                                .arguments
                                .handles()
                                .map(|handle| trees.child_type_reference(handle)),
                        );
                    }
                }
            }
            types::TypeReference::Generic(generic) => {
                if binder(generic.base_symbol) {
                    return false;
                }
                pending.extend(
                    generic
                        .arguments
                        .handles()
                        .map(|handle| trees.child_type_reference(handle)),
                );
            }
            types::TypeReference::ConstExpression(_) => return false,
            types::TypeReference::Unit => {}
        }
    }
    true
}

fn declared_type<'trees>(
    syntax: &SyntaxTrees,
    trees: &'trees SymbolResolvedTrees,
    sources: &SourceLinks,
    symbol: SymbolHandle,
    result: bool,
) -> Option<(TypeReferenceHandle, &'trees types::TypeReference)> {
    if !symbol.is_valid() {
        return None;
    }
    for &(origin, authored) in &sources.declared_types {
        if result != matches!(origin, DeclaredTypeOrigin::Result(_)) {
            continue;
        }
        if let Some((selected, reference)) = origin.declaration(trees)
            && selected == symbol
            && closed_type(trees, reference)
        {
            return Some((authored, reference));
        }
    }
    if !result {
        for &(authored, resolved) in &sources.statements {
            if let statement::Statement::LocalData(local) =
                trees.tables.declarations.state_statements.get(resolved)
                && local.symbol == symbol
                && !local.type_is_inferred
                && closed_type(trees, &local.type_reference)
                && let syntax_trees::statement::StatementNode::LocalData(authored_local) =
                    syntax.statements.statement(authored)
            {
                return authored_local
                    .type_reference
                    .is_valid()
                    .then_some((authored_local.type_reference, &local.type_reference));
            }
        }
    }
    None
}

/// Early type observations are evidence for inference, not a second call
/// admission boundary. A producer whose telescope cannot yet be observed keeps
/// its diagnostic here; an independently determined consumer may proceed to
/// ordinary checking of the unchanged call. If the consumer also lacks a tuple,
/// its refusal includes the retained producer explanation.
pub(super) struct CallTypeObservations {
    pub(super) types: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
    pub(super) pending_diagnostics: Vec<Diagnostic>,
}

fn expression_types<'trees>(
    syntax: &mut SyntaxTrees,
    trees: &'trees SymbolResolvedTrees,
    sources: &SourceLinks,
    expression: expression::ExpressionHandle,
    selection: &ConstantSelection<'_>,
    pending_diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<(TypeReferenceHandle, &'trees types::TypeReference)>> {
    let observed = match trees.tables.bodies.expressions.expression(expression) {
        expression::ExpressionNode::Name(name) => {
            declared_type(syntax, trees, sources, name.symbol, false)
                .map(|reference| vec![reference])
        }
        expression::ExpressionNode::Borrow(borrow) => {
            let targets = expression_types(
                syntax,
                trees,
                sources,
                borrow.target,
                selection,
                pending_diagnostics,
            )?;
            let mut borrowed = Vec::with_capacity(targets.len());
            for (target, declaration) in targets {
                // A borrow of a reference is a reborrow of its referent. Keep
                // nested declared structure and its custody; the new outer
                // region is elided and ordinary loan checking admits access.
                let referee = match syntax.type_references.type_reference(target) {
                    syntax_trees::types::TypeReferenceNode::Reference { referee, .. } => *referee,
                    _ => target,
                };
                let reference = syntax
                    .type_references
                    .insert_reference(referee, borrow.access);
                // The resolved declaration anchors nominal selection; the
                // authored reference carries this expression's constructor.
                borrowed.push((reference, declaration));
            }
            Some(borrowed)
        }
        expression::ExpressionNode::Member(member) => projections::member_types(
            syntax,
            trees,
            sources,
            member,
            selection,
            pending_diagnostics,
        ),
        expression::ExpressionNode::Call(call) => {
            if let Some(reference) = declared_type(syntax, trees, sources, call.target_symbol, true)
            {
                return Some(vec![reference]);
            }
            // A generic declaration's open return type is not an observation.
            // Complete its exact selected telescope from declared argument
            // types first, then carry the substituted result to the consumer.
            // This descends only through argument expressions, never callee
            // bodies, so recursion in the source call graph needs no unfolding.
            let Some(index) = super::selected_machine(trees, call.target_symbol) else {
                return None;
            };
            let Some((authored_result, resolved_result)) =
                sources
                    .declared_types
                    .iter()
                    .find_map(|&(origin, authored)| {
                        if !matches!(origin, DeclaredTypeOrigin::Result(_)) {
                            return None;
                        }
                        let (symbol, resolved) = origin.declaration(trees)?;
                        (symbol == call.target_symbol).then_some((authored, resolved))
                    })
            else {
                return None;
            };
            let Some((original, _)) = sources
                .expressions
                .iter()
                .find(|(_, resolved)| *resolved == expression)
            else {
                return None;
            };
            let syntax_trees::expression::ExpressionNode::Call(authored) =
                syntax.expressions.expression(*original).clone()
            else {
                return None;
            };
            let ordinary_types =
                call_constraints(syntax, trees, sources, call, Some(expression), selection);
            pending_diagnostics.extend(ordinary_types.pending_diagnostics);
            let completed = super::complete_arguments(
                syntax,
                index,
                &authored.target,
                &authored.machine_arguments,
                Some(authored_result),
                &ordinary_types.types,
                sources,
                selection,
            );
            match completed {
                Ok((_, result)) => result.map(|result| vec![(result, resolved_result)]),
                Err(reason) => {
                    pending_diagnostics.push(reason);
                    None
                }
            }
        }
        expression::ExpressionNode::Match(dispatch) => {
            let mut references = Vec::new();
            for arm in trees.tables.bodies.expressions.match_arms(dispatch.arms) {
                let Some(observed) = expression_types(
                    syntax,
                    trees,
                    sources,
                    arm.value,
                    selection,
                    pending_diagnostics,
                ) else {
                    return None;
                };
                references.extend(observed);
            }
            (!references.is_empty()).then_some(references)
        }
        // A value without a closed declared type cannot anchor an early
        // equation. In particular, array elements and a literal's magnitude
        // do not determine its generic application's type or extent.
        _ => None,
    };
    observed
}

fn constraints(
    syntax: &mut SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    target: SymbolHandle,
    arguments: HandleSpan<expression::ExpressionHandle>,
    selection: &ConstantSelection<'_>,
    pending_diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle)> {
    let mut observations = Vec::new();
    let Some((_, state)) = trees
        .tables
        .declarations
        .machine_states
        .iter()
        .find(|(_, state)| state.symbol == target)
    else {
        return observations;
    };
    let arguments = trees
        .tables
        .bodies
        .expressions
        .expression_handles(arguments);
    if arguments.len() != state.parameters.count() as usize {
        return observations;
    }
    for (parameter, argument) in state.parameters.handles().zip(arguments) {
        let Some((_, formal)) = sources
            .declared_types
            .iter()
            .find(|(origin, _)| *origin == DeclaredTypeOrigin::Parameter(parameter))
        else {
            continue;
        };
        if let Some(actuals) = expression_types(
            syntax,
            trees,
            sources,
            *argument,
            selection,
            pending_diagnostics,
        ) {
            observations.extend(actuals.into_iter().map(|(actual, _)| (*formal, actual)));
        }
    }
    observations
}

pub(super) fn call_constraints(
    syntax: &mut SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    call: &expression::TableCallExpression,
    occurrence: Option<expression::ExpressionHandle>,
    selection: &ConstantSelection<'_>,
) -> CallTypeObservations {
    let mut pending_diagnostics = Vec::new();
    let mut observations = constraints(
        syntax,
        trees,
        sources,
        call.target_symbol,
        call.arguments,
        selection,
        &mut pending_diagnostics,
    );
    let formal = sources
        .declared_types
        .iter()
        .find_map(|&(origin, authored)| {
            if let DeclaredTypeOrigin::Result(_) = origin
                && origin
                    .declaration(trees)
                    .is_some_and(|(symbol, _)| symbol == call.target_symbol)
            {
                Some(authored)
            } else {
                None
            }
        });
    if let (Some(formal), Some(occurrence)) = (formal, occurrence) {
        for &(authored, resolved) in &sources.statements {
            if let statement::Statement::LocalData(local) =
                trees.tables.declarations.state_statements.get(resolved)
                && local.initial_value == occurrence
                && !local.type_is_inferred
                && closed_type(trees, &local.type_reference)
                && let syntax_trees::statement::StatementNode::LocalData(local) =
                    syntax.statements.statement(authored)
                && local.type_reference.is_valid()
            {
                observations.push((formal, local.type_reference));
            }
        }
    }
    CallTypeObservations {
        types: observations,
        pending_diagnostics,
    }
}

pub(super) fn statement_constraints(
    syntax: &mut SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    call: &statement::Call,
    selection: &ConstantSelection<'_>,
) -> CallTypeObservations {
    let mut pending_diagnostics = Vec::new();
    let types = constraints(
        syntax,
        trees,
        sources,
        call.target_symbol,
        call.arguments,
        selection,
        &mut pending_diagnostics,
    );
    CallTypeObservations {
        types,
        pending_diagnostics,
    }
}
