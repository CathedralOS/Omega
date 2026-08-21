mod builtin;
mod data;
mod machines;
mod operators;
mod propositions;
mod traits;

pub(super) use builtin::insert_builtin_type_symbol_children;
pub(super) use data::insert_data_symbol_children;
pub(super) use machines::insert_machine_symbol_children;
pub(super) use operators::{insert_domain_symbol_children, insert_operator_symbol_children};
pub(super) use propositions::insert_proposition_symbol_children;
pub(super) use traits::insert_trait_symbol_children;

use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};

use super::names::symbol_seed;

/// Insert the lexical children owned by a machine-parameter signature. Its
/// generic parameters precede its value parameters, mirroring ordinary
/// machine symbol layout. Nested machine parameters recursively own their
/// own signature children.
fn insert_machine_parameter_signature_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    owner_symbol: SymbolHandle,
    signature: &psi_symbol_resolved_trees::signature::StateSignature,
    has_sources: bool,
) {
    let children = builder.insert_children(
        owner_symbol,
        program
            .data_type_parameters(signature.type_parameters)
            .iter()
            .map(|parameter| {
                let kind = match parameter.kind {
                    psi_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                        SymbolKind::MachineParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                symbol_seed(kind, &parameter.name, has_sources)
            })
            .chain(
                program
                    .state_parameters(signature.parameters)
                    .iter()
                    .map(|parameter| {
                        symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                    }),
            ),
    );

    let mut children = SymbolTableBuilder::child_handles(children);
    for parameter in program.data_type_parameters(signature.type_parameters) {
        let parameter_symbol = children.next();
        if let (
            Some(parameter_symbol),
            psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract },
        ) = (parameter_symbol, &parameter.kind)
            && let Some(contract) = contract.structural()
        {
            insert_machine_parameter_signature_children(
                builder,
                program,
                parameter_symbol,
                contract,
                has_sources,
            );
        }
    }
}

pub(in crate::symbols::symbol_table) fn insert_conformance_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    conformance_symbol: SymbolHandle,
    conformance: &psi_symbol_resolved_trees::trait_definition::Conformance,
    has_sources: bool,
) {
    let children = builder.insert_children(
        conformance_symbol,
        program
            .data_type_parameters(conformance.type_parameters)
            .iter()
            .map(|parameter| {
                let kind = match parameter.kind {
                    psi_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                        SymbolKind::MachineParameter
                    }
                    psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
                        SymbolKind::PropositionParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                symbol_seed(kind, &parameter.name, has_sources)
            }),
    );

    let mut children = SymbolTableBuilder::child_handles(children);
    for parameter in program.data_type_parameters(conformance.type_parameters) {
        let Some(parameter_symbol) = children.next() else {
            break;
        };
        match &parameter.kind {
            psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract } => {
                if let Some(contract) = contract.structural() {
                    insert_machine_parameter_signature_children(
                        builder,
                        program,
                        parameter_symbol,
                        contract,
                        has_sources,
                    );
                }
            }
            psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { contract } => {
                builder.insert_children(
                    parameter_symbol,
                    program
                        .state_parameters(contract.parameters)
                        .iter()
                        .map(|parameter| {
                            symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                        }),
                );
            }
            _ => {}
        }
    }
}
