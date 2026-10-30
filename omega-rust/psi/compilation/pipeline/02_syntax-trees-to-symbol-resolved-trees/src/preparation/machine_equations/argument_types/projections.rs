//! Declared field observations of completed generic results.
//!
//! A result can name an existing instance or retain a closed application until
//! later synthesis. Both select the declaration before reading its field; the
//! latter applies the ordinary equation tuple and constructor substitution.

use super::{
    ConstantSelection, DeclaredTypeOrigin, SourceLinks, closed_type, declared_type,
    expression_types,
};
use crate::preparation::{
    generic_data,
    type_equations::{self, EquationTemplate},
};
use diagnostics::Diagnostic;
use symbol_resolved_trees::{SymbolResolvedTrees, expression, types};
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::{
    SyntaxTrees,
    item::TypeParameterKind,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(super) fn member_types<'trees>(
    syntax: &mut SyntaxTrees,
    trees: &'trees SymbolResolvedTrees,
    sources: &SourceLinks,
    member: &expression::TableMemberExpression,
    selection: &ConstantSelection<'_>,
    pending: &mut Vec<Diagnostic>,
) -> Option<Vec<(TypeReferenceHandle, &'trees types::TypeReference)>> {
    if member.member_symbol.is_valid()
        && let Some(field) = declared_type(syntax, trees, sources, member.member_symbol, false)
    {
        return Some(vec![field]);
    }
    // Case payloads require their independently selected variant identity.
    if member.case_variant.is_some() {
        return None;
    }
    let receivers = expression_types(syntax, trees, sources, member.receiver, selection, pending)?;
    let mut fields = Vec::new();
    for (authored, resolved) in receivers {
        let mut reference = authored;
        loop {
            reference = match syntax.type_references.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. } => *referee,
                TypeReferenceNode::Constrained { base_type, .. } => *base_type,
                _ => break,
            };
        }
        let (selected, application) = match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Named(name) => (
                generic_data::selected_data_item(syntax, Some(selection), name),
                None,
            ),
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => (
                generic_data::selected_data_item(syntax, Some(selection), base_name),
                Some((
                    base_name.clone(),
                    syntax
                        .type_references
                        .type_reference_handles(*arguments)
                        .to_vec(),
                )),
            ),
            _ => (None, None),
        };
        let owner = selected
            .and_then(|declaration| {
                sources
                    .data_declarations
                    .iter()
                    .find(|(item, _)| *item == declaration)
            })
            .filter(|(_, ordinal)| *ordinal < trees.data_definitions.len())
            .map(|(_, ordinal)| trees.data_definitions[*ordinal].symbol)
            .or_else(|| closed_nominal_owner(trees, resolved))?;
        let field = trees.symbols.find_child_by_name_and_kind(
            owner,
            member.member.as_str(),
            SymbolKind::Field,
        )?;
        if let Some((base_name, arguments)) = application {
            let declaration = syntax.root_item(selected?).as_data()?.clone();
            let (field_type, resolved_type) =
                sources
                    .declared_types
                    .iter()
                    .find_map(|&(origin, authored)| {
                        if !matches!(origin, DeclaredTypeOrigin::Field(_)) {
                            return None;
                        }
                        let (symbol, resolved) = origin.declaration(trees)?;
                        (symbol == field).then_some((authored, resolved))
                    })?;
            let parameters = syntax
                .items
                .type_parameters(declaration.type_parameters)
                .to_vec();
            let names = parameters
                .iter()
                .map(|parameter| parameter.name.as_str(syntax).to_owned())
                .collect::<Vec<_>>();
            let const_types = parameters
                .iter()
                .map(|parameter| match parameter.kind {
                    TypeParameterKind::Const { type_reference } => Some(type_reference),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let equations = type_equations::classify_type_equations(
                syntax,
                &parameters,
                syntax.items.proof_facts(declaration.where_facts),
            );
            let name = declaration.name.as_str(syntax).to_owned();
            let template = EquationTemplate {
                kind: "data",
                name: &name,
                parameters: declaration.type_parameters,
                invocation_lifetimes: &[],
                parameter_names: &names,
                const_parameter_types: &const_types,
                type_equations: &equations,
            };
            let constants = generic_data::module_constants::lexical_integer_const_values(syntax);
            let mut warnings = Vec::new();
            let completed = type_equations::complete_equation_arguments(
                syntax,
                template,
                &base_name,
                &arguments,
                &constants,
                Some(selection),
                &mut warnings,
            )
            .and_then(|tuple| {
                type_equations::materialize_completed_type(
                    syntax,
                    template,
                    &base_name,
                    &tuple,
                    field_type,
                    &constants,
                    Some(selection),
                )
            });
            match completed {
                Ok(field_type) => fields.push((field_type, resolved_type)),
                Err(reason) => {
                    pending.push(reason);
                    return None;
                }
            }
        } else {
            fields.push(declared_type(syntax, trees, sources, field, false)?);
        }
    }
    Some(fields)
}

fn closed_nominal_owner(
    trees: &SymbolResolvedTrees,
    reference: &types::TypeReference,
) -> Option<SymbolHandle> {
    if !closed_type(trees, reference) {
        return None;
    }
    let mut reference = reference;
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
    match reference {
        types::TypeReference::Named { symbol, .. }
            if trees.data_definition_by_symbol(*symbol).is_some() =>
        {
            Some(*symbol)
        }
        _ => None,
    }
}
