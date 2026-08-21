mod data;
mod domains;
mod machines;
mod operators;
mod propositions;
mod traits;

use psi_arena::Arena;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{
    SymbolHandle, SymbolKind, SymbolTable, builtin_function_symbols, builtin_type_symbols,
};

use super::top_level::data::assign_data_symbols;
use super::top_level::domains::assign_domain_symbols;
use super::top_level::machines::assign_machine_symbols;
use super::top_level::operators::assign_root_operator_symbols;
use super::top_level::propositions::assign_proposition_symbols;
use super::top_level::traits::assign_trait_symbols;

pub(super) fn assign_top_level_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let mut root_children = symbols.child_handles(symbols.root()).into_iter().flatten();

    for _ in 0..builtin_type_symbols().len() {
        let _ = root_children.next();
    }
    for _ in 0..builtin_function_symbols().len() {
        let _ = root_children.next();
    }

    program.invariant_definitions.for_each_mut(|invariant| {
        invariant.symbol = next_child_of_kind(&mut root_children, symbols, SymbolKind::Invariant);
    });

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
        if conformance.alias.is_some() {
            conformance.symbol = conformance_symbols
                .next()
                .unwrap_or_else(SymbolHandle::invalid);
        }
    });
    assign_conformance_parameter_symbols(program, symbols);
    attach_conformance_parameter_scopes(program);
    assign_machine_symbols(program, symbols, &mut root_children);
    assign_proposition_symbols(program, symbols, &mut root_children);
    assign_root_operator_symbols(program, symbols, &mut root_children);
    assign_trait_symbols(program, symbols, &mut root_children);

    program.wire_schemas.for_each_mut(|wire_schema| {
        wire_schema.symbol =
            next_child_of_kind(&mut root_children, symbols, SymbolKind::WireSchema);
    });
}

fn attach_conformance_parameter_scopes(program: &mut SymbolResolvedTrees) {
    let scopes = program
        .conformances
        .iter()
        .filter_map(|conformance| {
            let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed {
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
                            psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline
                                | psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
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
                psi_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
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
                psi_symbol_resolved_trees::data::TypeParameterKind::Machine { mut contract } => {
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
                    psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract }
                }
                psi_symbol_resolved_trees::data::TypeParameterKind::Proposition {
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
                    psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { contract }
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
    data_type_parameters: &mut Arena<psi_symbol_resolved_trees::data::TypeParameter>,
    state_parameters: &mut Arena<psi_symbol_resolved_trees::signature::StateParameter>,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    contract: &mut psi_symbol_resolved_trees::signature::StateSignature,
    owner_symbol: SymbolHandle,
    inherited_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    self_symbol: SymbolHandle,
) {
    use psi_symbol_resolved_trees::data::TypeParameterKind;

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

    let mut local_type_parameters = inherited_type_parameters.to_vec();
    local_type_parameters
        .extend_from_slice(data_type_parameters.span_or_empty(contract.type_parameters));

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
    state_parameters: &mut Arena<psi_symbol_resolved_trees::signature::StateParameter>,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    contract: &mut psi_symbol_resolved_trees::data::PropositionParameterSignature,
    owner_symbol: SymbolHandle,
    inherited_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
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
