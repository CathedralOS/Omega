//! Mathematical `let`/`boundary let` declarations: name resolution inside
//! their signature surfaces and transparent bodies.
//!
//! Every declaration contributes its binder/parameter children as the local
//! scope. `Ordinary` mathematical type arms resolve through the shared type
//! pass with the binders as local type parameters — so `u: core::Level` binds
//! inside `A: core::Type<u>` — while `Application` argument expressions and
//! `Definition` bodies share the proposition walk with a call-target set that
//! includes other mathematical declarations. Arrow binders are retained as
//! authored names only; the typed-tree telescope owns their scoping.

use symbol_resolved_trees::data::TypeParameterKind;
use symbol_resolved_trees::mathematical::{MathematicalBody, MathematicalType};
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::propositions::assign_expression_symbols;
use super::type_references::assign_type_reference_symbol_with_locals_and_self_type;

/// Top-level kinds a bare call target inside a mathematical term may name:
/// another mathematical declaration first (the migrating surface), then the
/// proposition family it is replacing during the transition.
const MATHEMATICAL_CALL_KINDS: &[SymbolKind] =
    &[SymbolKind::MathematicalDefinition, SymbolKind::Proposition];

pub(super) fn assign_mathematical_expression_symbols(
    program: &mut symbol_resolved_trees::SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
    let mathematical_parameters = &program.tables.declarations.mathematical_parameters;
    let mathematical_types = &mut program.tables.declarations.mathematical_types;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    let expressions = &mut program.tables.bodies.expressions;
    let definitions = &program.roots.mathematical_definitions;

    for definition in definitions {
        // Binder records are already shared `TypeParameter`s; they are the
        // local scope for every carrier, parameter, result, and body of this
        // declaration. Cloned up front so the carrier pass may mutate the
        // arena in place.
        let local_type_parameters = data_type_parameters
            .span_or_empty(definition.binders)
            .to_vec();

        // Binder carrier types (`N: usize`, `u: core::Level`,
        // `A: core::Type<u>`) resolve against the declaration's binders.
        for binder in data_type_parameters.span_mut_or_empty(definition.binders) {
            let type_reference = match &mut binder.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => type_reference,
                _ => continue,
            };
            assign_type_reference_symbol_with_locals_and_self_type(
                symbols,
                child_type_references,
                &local_type_parameters,
                SymbolHandle::invalid(),
                type_reference,
            );
        }

        // The result and every parameter type walk the declaration-local
        // mathematical type tree; `Ordinary` arms resolve in place and
        // `Application` arguments resolve as proof-surface expressions.
        let mut worklist = vec![definition.result];
        for parameter in mathematical_parameters.span_or_empty(definition.parameters) {
            worklist.push(parameter.ty);
        }
        while let Some(handle) = worklist.pop() {
            let mut node = mathematical_types.get(handle).clone();
            match &mut node {
                MathematicalType::Ordinary(reference) => {
                    assign_type_reference_symbol_with_locals_and_self_type(
                        symbols,
                        child_type_references,
                        &local_type_parameters,
                        SymbolHandle::invalid(),
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
                            symbols,
                            definition.symbol,
                            MATHEMATICAL_CALL_KINDS,
                            &local_type_parameters,
                            argument,
                        );
                    }
                }
            }
            *mathematical_types.get_mut(handle) = node;
        }

        let MathematicalBody::Definition(term) = definition.body else {
            continue;
        };
        assign_expression_symbols(
            expressions,
            child_type_references,
            symbols,
            definition.symbol,
            MATHEMATICAL_CALL_KINDS,
            &local_type_parameters,
            term,
        );
    }
}
