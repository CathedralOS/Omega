/*
    Mathematical `let`/`boundary let` declarations: name resolution inside
    their signature surfaces and transparent bodies.

    Every declaration contributes its binder/parameter children as the local
    scope. `Ordinary` mathematical type arms resolve through the shared type
    pass with the binders as local type parameters — so `u: core::Level` binds
    inside `A: core::Type<u>` — and a value binder named where applied data
    takes a value binds there too, so `Length` binds inside `Vector<Length,
    Total>`. `Application` argument expressions and
    `Definition` bodies share the proposition walk and the common callable
    declaration resolver. Arrow binders are retained as
    authored names only; the typed-tree telescope owns their scoping.
*/

use crate::source_resolved::data::TypeParameterKind;
use crate::source_resolved::mathematical::{MathematicalBody, MathematicalType};
use crate::source_symbols::SymbolTable;

use super::propositions::assign_expression_symbols;
use super::type_references::{
    assign_type_reference_symbol_with_locals_and_constraints, bind_binder_value_arguments,
};

pub(super) fn assign_mathematical_expression_symbols(
    program: &mut crate::source_resolved::SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
    let mathematical_parameters = &program.tables.declarations.mathematical_parameters;
    let mathematical_types = &mut program.tables.declarations.mathematical_types;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    let type_constraints = &program.tables.types.constraints;
    let expressions = &mut program.tables.bodies.expressions;
    let definitions = &program.roots.mathematical_definitions;
    // The parameters of each data declaration that takes a value. A
    // definition's type that applies one names the definition's own value
    // binders where those parameters stand.
    let indexed_data = program
        .roots
        .data_definitions
        .iter()
        .filter_map(|data| {
            let parameters = data_type_parameters.span_or_empty(data.type_parameters);
            parameters
                .iter()
                .any(|parameter| {
                    matches!(
                        parameter.kind,
                        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                    )
                })
                .then(|| (data.symbol, parameters.to_vec()))
        })
        .collect::<Vec<_>>();
    let declared = |symbol| {
        indexed_data
            .iter()
            .find(|(data, _)| *data == symbol)
            .map(|(_, parameters)| parameters.clone())
    };

    for definition in definitions {
        let local_type_parameters = data_type_parameters
            .span_or_empty(definition.binders)
            .to_vec();

        for binder in data_type_parameters.span_mut_or_empty(definition.binders) {
            let type_reference = match &mut binder.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => type_reference,
                _ => continue,
            };
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                type_reference,
            );
        }

        let mut worklist = vec![definition.result];
        for parameter in mathematical_parameters.span_or_empty(definition.parameters) {
            worklist.push(parameter.ty);
        }
        while let Some(handle) = worklist.pop() {
            let mut node = mathematical_types.get(handle).clone();
            match &mut node {
                MathematicalType::Ordinary(reference) => {
                    assign_type_reference_symbol_with_locals_and_constraints(
                        symbols,
                        child_type_references,
                        type_constraints,
                        &local_type_parameters,
                        reference,
                    );
                    bind_binder_value_arguments(
                        &declared,
                        child_type_references,
                        &local_type_parameters,
                        reference,
                    );
                }
                MathematicalType::Arrow {
                    domain, codomain, ..
                } => {
                    worklist.push(*domain);
                    worklist.push(*codomain);
                }
                MathematicalType::Application { callee, arguments } => {
                    worklist.push(*callee);
                    for argument in expressions.expression_handles(*arguments).to_vec() {
                        assign_expression_symbols(
                            expressions,
                            child_type_references,
                            type_constraints,
                            symbols,
                            definition.symbol,
                            &local_type_parameters,
                            argument,
                        );
                    }
                }
            }
            *mathematical_types.get_mut(handle) = node;
        }

        let mut body_type_parameters = local_type_parameters.clone();
        for parameter in mathematical_parameters.span_or_empty(definition.parameters) {
            body_type_parameters.push(crate::source_resolved::data::TypeParameter {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                kind: TypeParameterKind::Type,
                bounds: Default::default(),
            });
        }

        let (arrow_binders, term) = match definition.body {
            MathematicalBody::Definition(term) => (arena::HandleSpan::empty(), term),
            MathematicalBody::DependentArrow { binders, body } => (binders, body),
            MathematicalBody::Assumption => continue,
        };
        let mut arrow_worklist: Vec<_> = mathematical_parameters
            .span_or_empty(arrow_binders)
            .iter()
            .map(|parameter| parameter.ty)
            .collect();
        while let Some(handle) = arrow_worklist.pop() {
            let mut node = mathematical_types.get(handle).clone();
            match &mut node {
                MathematicalType::Ordinary(reference) => {
                    assign_type_reference_symbol_with_locals_and_constraints(
                        symbols,
                        child_type_references,
                        type_constraints,
                        &body_type_parameters,
                        reference,
                    );
                    bind_binder_value_arguments(
                        &declared,
                        child_type_references,
                        &body_type_parameters,
                        reference,
                    );
                }
                MathematicalType::Arrow {
                    domain, codomain, ..
                } => {
                    arrow_worklist.push(*domain);
                    arrow_worklist.push(*codomain);
                }
                MathematicalType::Application { callee, arguments } => {
                    arrow_worklist.push(*callee);
                    for argument in expressions.expression_handles(*arguments).to_vec() {
                        assign_expression_symbols(
                            expressions,
                            child_type_references,
                            type_constraints,
                            symbols,
                            definition.symbol,
                            &body_type_parameters,
                            argument,
                        );
                    }
                }
            }
            *mathematical_types.get_mut(handle) = node;
        }
        assign_expression_symbols(
            expressions,
            child_type_references,
            type_constraints,
            symbols,
            definition.symbol,
            &body_type_parameters,
            term,
        );
    }
}
