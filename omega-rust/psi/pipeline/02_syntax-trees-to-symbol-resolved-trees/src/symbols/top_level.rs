//! Symbol assignment for top-level declarations.
//!
//! `assign_top_level_symbols` is the first assignment pass that
//! `assign_symbols` and `assign_symbols_against_resolved_base` (in
//! `symbols/mod.rs`) run after the symbol table is built. Declaration symbols
//! are matched by position, not looked up by name: it walks the root's
//! children in the order `symbol_table::build_symbol_table` inserted them,
//! skips the builtin type and function symbols, and hands the remaining
//! children to one step per declaration kind, in this order:
//!
//! 1. domains (`domains`), then data (`data`);
//! 2. named conformances, done here: each takes its symbol, then
//!    `assign_conformance_parameter_symbols` assigns every conformance's type
//!    parameters and their signatures, and
//!    `attach_conformance_parameter_scopes` gives the machines that realize a
//!    closed conformance's inline and trait-default rows that conformance's
//!    lifetime and type parameters;
//! 3. machines, propositions, mathematical definitions, root operators,
//!    measures and traits, each in its own child module;
//! 4. wire schemas, done here.
//!
//! A declaration that already holds a valid symbol keeps it. Every step except
//! wire schemas also assigns the declaration's own children (parameters,
//! fields, states) by the same positional walk over that declaration's
//! children, and several resolve the type references in those children's
//! signatures. Machine and trait assignment return diagnostics, which this
//! function returns.
//!
//! Shared by the child modules and the conformance step rather than steps of
//! their own: `next_child_of_kind` takes
//! the next child and returns the invalid handle when its kind is not the
//! expected one; `assign_machine_parameter_signature_symbols` and
//! `assign_proposition_parameter_signature_symbols` assign the parameter
//! symbols and type references of machine-parameter and proposition-parameter
//! signatures.

mod data;
mod domains;
mod machines;
mod mathematical;
mod measures;
mod operators;
mod propositions;
mod traits;

use crate::symbol_resolved_trees::SymbolResolvedTrees;
use arena::Arena;
use symbols::{
    SymbolHandle, SymbolKind, SymbolTable, builtin_function_symbols, builtin_type_symbols,
};

use super::top_level::data::assign_data_symbols;
use super::top_level::domains::assign_domain_symbols;
use super::top_level::machines::assign_machine_symbols;
use super::top_level::mathematical::assign_mathematical_symbols;
use super::top_level::operators::assign_root_operator_symbols;
use super::top_level::propositions::assign_proposition_symbols;
use super::top_level::traits::assign_trait_symbols;

pub(super) fn assign_top_level_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) -> Vec<diagnostics::Diagnostic> {
    let mut root_children = symbols.child_handles(symbols.root()).into_iter().flatten();

    for _ in 0..builtin_type_symbols().len() {
        let _ = root_children.next();
    }
    for _ in 0..builtin_function_symbols().len() {
        let _ = root_children.next();
    }

    assign_domain_symbols(program, symbols, &mut root_children);
    assign_data_symbols(program, symbols, &mut root_children);
    let conformance_symbols = program
        .conformances
        .iter()
        .filter(|conformance| conformance.alias.is_some())
        .map(|_| next_child_of_kind(&mut root_children, symbols, SymbolKind::Conformance))
        .collect::<Vec<_>>();
    let mut conformance_symbols = conformance_symbols.into_iter();
    program.conformances.for_each_mut(|conformance| {
        if conformance.alias.is_some() && !conformance.symbol.is_valid() {
            conformance.symbol = conformance_symbols
                .next()
                .unwrap_or_else(SymbolHandle::invalid);
        }
    });
    assign_conformance_parameter_symbols(program, symbols);
    attach_conformance_parameter_scopes(program);
    let mut diagnostics = assign_machine_symbols(program, symbols, &mut root_children);
    assign_proposition_symbols(program, symbols, &mut root_children);
    assign_mathematical_symbols(program, symbols, &mut root_children);
    assign_root_operator_symbols(program, symbols, &mut root_children);
    measures::assign_measure_symbols(program, symbols, &mut root_children);
    diagnostics.extend(assign_trait_symbols(program, symbols, &mut root_children));

    program.wire_schemas.for_each_mut(|wire_schema| {
        if !wire_schema.symbol.is_valid() {
            wire_schema.symbol =
                next_child_of_kind(&mut root_children, symbols, SymbolKind::WireSchema);
        }
    });

    diagnostics
}

fn attach_conformance_parameter_scopes(program: &mut SymbolResolvedTrees) {
    let scopes = program
        .conformances
        .iter()
        .filter_map(|conformance| {
            let crate::symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed {
                rows,
            } = &conformance.implementation
            else {
                return None;
            };
            Some((
                rows.iter()
                    .filter(|row| {
                        matches!(
                            row.source,
                            crate::symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline
                                | crate::symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
                        )
                    })
                    .filter_map(|row| row.provisional_realization_ordinal)
                    .collect::<Vec<_>>(),
                conformance.lifetime_parameters.clone(),
                conformance.type_parameters,
            ))
        })
        .collect::<Vec<_>>();
    let mut ordinal = 0usize;
    program.machines.for_each_mut(|machine| {
        if let Some((_, lifetimes, parameters)) = scopes
            .iter()
            .find(|(realizations, _, _)| realizations.contains(&ordinal))
        {
            machine.lifetime_parameters = lifetimes.clone();
            machine.type_parameters = *parameters;
        }
        ordinal += 1;
    });
}

fn assign_conformance_parameter_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let type_constraints = &program.tables.types.constraints;
    let declarations = &mut program.tables.declarations;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;

    program.roots.conformances.for_each_mut(|conformance| {
        let mut children = symbols
            .child_handles(conformance.symbol)
            .into_iter()
            .flatten();
        for parameter in data_type_parameters.span_mut_or_empty(conformance.type_parameters) {
            let kind = match parameter.kind {
                crate::symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                crate::symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
                    SymbolKind::PropositionParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            parameter.symbol = next_child_of_kind(&mut children, symbols, kind);
        }
        let local_type_parameters = data_type_parameters
            .span_or_empty(conformance.type_parameters)
            .to_vec();

        for index in 0..conformance.type_parameters.len() {
            let (parameter_symbol, kind) = {
                let parameter =
                    &data_type_parameters.span_or_empty(conformance.type_parameters)[index];
                (parameter.symbol, parameter.kind.clone())
            };
            let resolved_kind = match kind {
                crate::symbol_resolved_trees::data::TypeParameterKind::Machine { mut contract } => {
                    if let Some(signature) = contract.structural_mut() {
                        assign_machine_parameter_signature_symbols(
                            symbols,
                            data_type_parameters,
                            state_parameters,
                            child_type_references,
                            type_constraints,
                            signature,
                            parameter_symbol,
                            &local_type_parameters,
                            conformance.symbol,
                        );
                    }
                    crate::symbol_resolved_trees::data::TypeParameterKind::Machine { contract }
                }
                crate::symbol_resolved_trees::data::TypeParameterKind::Proposition {
                    mut contract,
                } => {
                    assign_proposition_parameter_signature_symbols(
                        symbols,
                        state_parameters,
                        child_type_references,
                        type_constraints,
                        &mut contract,
                        parameter_symbol,
                        &local_type_parameters,
                        conformance.symbol,
                    );
                    crate::symbol_resolved_trees::data::TypeParameterKind::Proposition { contract }
                }
                other => other,
            };
            data_type_parameters.span_mut_or_empty(conformance.type_parameters)[index].kind =
                resolved_kind;
        }
    });
}

pub(super) fn next_child_of_kind(
    children: &mut impl Iterator<Item = SymbolHandle>,
    symbols: &SymbolTable,
    kind: SymbolKind,
) -> SymbolHandle {
    let Some(child) = children.next() else {
        return SymbolHandle::invalid();
    };

    if symbols.get(child).kind == kind {
        child
    } else {
        SymbolHandle::invalid()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_machine_parameter_signature_symbols(
    symbols: &SymbolTable,
    data_type_parameters: &mut Arena<crate::symbol_resolved_trees::data::TypeParameter>,
    state_parameters: &mut Arena<crate::symbol_resolved_trees::signature::StateParameter>,
    child_type_references: &mut Arena<crate::symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<crate::symbol_resolved_trees::types::TypeConstraint>,
    contract: &mut crate::symbol_resolved_trees::signature::StateSignature,
    owner_symbol: SymbolHandle,
    inherited_type_parameters: &[crate::symbol_resolved_trees::data::TypeParameter],
    self_symbol: SymbolHandle,
) {
    use crate::symbol_resolved_trees::data::TypeParameterKind;

    contract.symbol = owner_symbol;
    let mut children = symbols.child_handles(owner_symbol).into_iter().flatten();

    for parameter in data_type_parameters.span_mut_or_empty(contract.type_parameters) {
        let kind = match parameter.kind {
            TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
            TypeParameterKind::Proposition { .. } => SymbolKind::PropositionParameter,
            _ => SymbolKind::TypeParameter,
        };
        parameter.symbol = next_child_of_kind(&mut children, symbols, kind);
    }

    // Type lookup reads the first matching declaration. A nested signature's
    // own binders shadow inherited owner binders without changing either
    // declaration identity; unshadowed owner names remain in scope.
    let mut local_type_parameters = data_type_parameters
        .span_or_empty(contract.type_parameters)
        .to_vec();
    local_type_parameters.extend_from_slice(inherited_type_parameters);

    let nested_count = contract.type_parameters.len();
    for index in 0..nested_count {
        let (parameter_symbol, kind) = {
            let parameter = &data_type_parameters.span_or_empty(contract.type_parameters)[index];
            (parameter.symbol, parameter.kind.clone())
        };
        let resolved_kind = match kind {
            TypeParameterKind::Type => TypeParameterKind::Type,
            TypeParameterKind::Const { mut type_reference } => {
                crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &local_type_parameters,
                    self_symbol,
                    &mut type_reference,
                );
                TypeParameterKind::Const { type_reference }
            }
            TypeParameterKind::Value { mut type_reference } => {
                crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &local_type_parameters,
                    self_symbol,
                    &mut type_reference,
                );
                TypeParameterKind::Value { type_reference }
            }
            TypeParameterKind::Machine { mut contract } => {
                if let Some(signature) = contract.structural_mut() {
                    assign_machine_parameter_signature_symbols(
                        symbols,
                        data_type_parameters,
                        state_parameters,
                        child_type_references,
                        type_constraints,
                        signature,
                        parameter_symbol,
                        &local_type_parameters,
                        self_symbol,
                    );
                }
                TypeParameterKind::Machine { contract }
            }
            TypeParameterKind::Proposition { mut contract } => {
                assign_proposition_parameter_signature_symbols(
                    symbols,
                    state_parameters,
                    child_type_references,
                    type_constraints,
                    &mut contract,
                    parameter_symbol,
                    &local_type_parameters,
                    self_symbol,
                );
                TypeParameterKind::Proposition { contract }
            }
        };
        data_type_parameters.span_mut_or_empty(contract.type_parameters)[index].kind =
            resolved_kind;
    }

    for parameter in state_parameters.span_mut_or_empty(contract.parameters) {
        parameter.symbol = next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            &local_type_parameters,
            self_symbol,
            &mut parameter.type_reference,
        );
    }
    if let Some(return_type) = &mut contract.return_type {
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            &local_type_parameters,
            self_symbol,
            return_type,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_proposition_parameter_signature_symbols(
    symbols: &SymbolTable,
    state_parameters: &mut Arena<crate::symbol_resolved_trees::signature::StateParameter>,
    child_type_references: &mut Arena<crate::symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<crate::symbol_resolved_trees::types::TypeConstraint>,
    contract: &mut crate::symbol_resolved_trees::data::PropositionParameterSignature,
    owner_symbol: SymbolHandle,
    inherited_type_parameters: &[crate::symbol_resolved_trees::data::TypeParameter],
    self_symbol: SymbolHandle,
) {
    let mut children = symbols.child_handles(owner_symbol).into_iter().flatten();
    for parameter in state_parameters.span_mut_or_empty(contract.parameters) {
        parameter.symbol = next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            inherited_type_parameters,
            self_symbol,
            &mut parameter.type_reference,
        );
    }
}
