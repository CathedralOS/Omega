mod data;
mod domains;
mod machines;
mod operators;
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
    assign_machine_symbols(program, symbols, &mut root_children);
    assign_root_operator_symbols(program, symbols, &mut root_children);
    assign_trait_symbols(program, symbols, &mut root_children);

    program.wire_schemas.for_each_mut(|wire_schema| {
        wire_schema.symbol =
            next_child_of_kind(&mut root_children, symbols, SymbolKind::WireSchema);
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
                assign_machine_parameter_signature_symbols(
                    symbols,
                    data_type_parameters,
                    state_parameters,
                    child_type_references,
                    type_constraints,
                    &mut contract,
                    parameter_symbol,
                    &local_type_parameters,
                    self_symbol,
                );
                TypeParameterKind::Machine { contract }
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
