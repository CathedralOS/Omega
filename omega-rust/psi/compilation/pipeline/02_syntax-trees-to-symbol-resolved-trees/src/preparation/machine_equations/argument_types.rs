//! Declared type observations for one exact selected ordinary call.
//!
//! Resolution owns the declaration selection; lowering records handle links to
//! the corresponding authored type. These links survive provisional resolution
//! without reselecting by a source coordinate or a leaf spelling. An initializer,
//! literal element, flow predicate or private postcondition supplies no type.
//! Open schemas remain obligations: this reader does not invent a closed type
//! for a generic callee's unapplied result or replace later compatibility checks.

use super::SourceLinks;
use arena::{Handle, HandleSpan};
use symbol_resolved_trees::{SymbolResolvedTrees, expression, statement, types};
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::{SyntaxTrees, types::TypeReferenceHandle};

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

fn expression_types<'trees>(
    syntax: &SyntaxTrees,
    trees: &'trees SymbolResolvedTrees,
    sources: &SourceLinks,
    expression: expression::ExpressionHandle,
) -> Option<Vec<(TypeReferenceHandle, &'trees types::TypeReference)>> {
    match trees.tables.bodies.expressions.expression(expression) {
        expression::ExpressionNode::Name(name) => {
            declared_type(syntax, trees, sources, name.symbol, false)
                .map(|reference| vec![reference])
        }
        expression::ExpressionNode::Member(member) => {
            if member.member_symbol.is_valid() {
                return declared_type(syntax, trees, sources, member.member_symbol, false)
                    .map(|reference| vec![reference]);
            }
            // General value projections can wait until typing to acquire a
            // member symbol. Their declared receiver still selects one exact
            // nominal owner here. The selector names a child of THAT owner;
            // neither a global leaf-name search nor initializer shape is used.
            // Case payloads require their separately selected case identity.
            if member.case_variant.is_some() {
                return None;
            }
            let mut fields = Vec::new();
            for (_, mut reference) in expression_types(syntax, trees, sources, member.receiver)? {
                loop {
                    reference = match reference {
                        types::TypeReference::Reference(reference) => {
                            trees.child_type_reference(reference.referee)
                        }
                        types::TypeReference::Constrained(reference) => {
                            trees.child_type_reference(reference.base_type)
                        }
                        _ => break,
                    };
                }
                let types::TypeReference::Named { symbol, .. } = reference else {
                    return None;
                };
                trees.data_definition_by_symbol(*symbol)?;
                let field = trees.symbols.find_child_by_name_and_kind(
                    *symbol,
                    member.member.as_str(),
                    SymbolKind::Field,
                )?;
                fields.push(declared_type(syntax, trees, sources, field, false)?);
            }
            Some(fields)
        }
        expression::ExpressionNode::Call(call) => {
            declared_type(syntax, trees, sources, call.target_symbol, true)
                .map(|reference| vec![reference])
        }
        expression::ExpressionNode::Match(dispatch) => {
            let mut references = Vec::new();
            for arm in trees.tables.bodies.expressions.match_arms(dispatch.arms) {
                references.extend(expression_types(syntax, trees, sources, arm.value)?);
            }
            (!references.is_empty()).then_some(references)
        }
        // A value without a closed declared type cannot anchor an early
        // equation. In particular, array elements and a literal's magnitude
        // do not determine its generic application's type or extent.
        _ => None,
    }
}

fn constraints(
    syntax: &SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    target: SymbolHandle,
    arguments: HandleSpan<expression::ExpressionHandle>,
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
        if let Some(actuals) = expression_types(syntax, trees, sources, *argument) {
            observations.extend(actuals.into_iter().map(|(actual, _)| (*formal, actual)));
        }
    }
    observations
}

pub(super) fn call_constraints(
    syntax: &SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    call: &expression::TableCallExpression,
    occurrence: Option<expression::ExpressionHandle>,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle)> {
    let mut observations = constraints(syntax, trees, sources, call.target_symbol, call.arguments);
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
    observations
}

pub(super) fn statement_constraints(
    syntax: &SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    call: &statement::Call,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle)> {
    constraints(syntax, trees, sources, call.target_symbol, call.arguments)
}
