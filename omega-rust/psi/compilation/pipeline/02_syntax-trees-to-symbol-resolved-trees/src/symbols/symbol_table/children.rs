//! Child symbols for each kind of top-level declaration.
//!
//! `build_symbol_table` and `extend_symbol_table` (in `symbol_table.rs`)
//! insert one root symbol per declaration, then call the matching
//! `insert_*_symbol_children` function to add that declaration's parameters,
//! fields, states and other named children beneath it. Machine children are
//! inserted later, by `insert_selected_machine_children`, once all root
//! identities exist. A qualified machine does not inherit its carrier's fields.
//!
//! The child modules hold the insertion for builtin types, data, machines,
//! mathematical definitions, domains and operators (both in `operators`),
//! propositions and traits. This file holds the two small cases, measures and
//! named conformances, and `insert_machine_parameter_signature_children`,
//! which the conformance, data, machine and trait insertions use for
//! machine-parameter signatures.
//!
//! `symbols::top_level` matches these children to declarations by position,
//! walking them in the order they are inserted here, so the insertion order
//! and the assignment order must agree.

mod builtin;
mod data;
mod machines;
mod mathematical;
mod operators;
mod propositions;
mod traits;

pub(super) use builtin::insert_builtin_type_symbol_children;
pub(super) use data::insert_data_symbol_children;
pub(super) use machines::insert_machine_symbol_children;
pub(super) use mathematical::insert_mathematical_symbol_children;
pub(super) use operators::{insert_domain_symbol_children, insert_operator_symbol_children};
pub(super) use propositions::insert_proposition_symbol_children;
pub(super) use traits::insert_trait_symbol_children;

use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolTableAppender, SymbolTableBuilder};

use super::names::{parameter_seeds, type_parameter_seeds};

pub(super) fn insert_measure_symbol_children(
    builder: &mut impl SymbolTableAppender,
    measure_symbol: SymbolHandle,
    measure: &symbol_resolved_trees::measure::MeasureDefinition,
    has_sources: bool,
) {
    builder.insert_children(
        measure_symbol,
        parameter_seeds(measure.parameter.iter(), has_sources),
    );
}

/// Insert the lexical children owned by a machine-parameter signature. Its
/// generic parameters precede its value parameters, mirroring ordinary
/// machine symbol layout. Nested machine parameters recursively own their
/// own signature children.
/// Every declaration's type-parameter walk ends the same way: each
/// Machine-kind parameter's structural contract grows its own children.
fn insert_machine_type_parameter_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    parameters: &[symbol_resolved_trees::data::TypeParameter],
    children: &mut impl Iterator<Item = SymbolHandle>,
    has_sources: bool,
) {
    for parameter in parameters {
        let parameter_symbol = children.next();
        if let (
            Some(parameter_symbol),
            symbol_resolved_trees::data::TypeParameterKind::Machine { contract },
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

fn insert_machine_parameter_signature_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    owner_symbol: SymbolHandle,
    signature: &symbol_resolved_trees::signature::StateSignature,
    has_sources: bool,
) {
    let children = builder.insert_children(
        owner_symbol,
        type_parameter_seeds(
            program.data_type_parameters(signature.type_parameters),
            false,
            has_sources,
        )
        .chain(parameter_seeds(
            program.state_parameters(signature.parameters),
            has_sources,
        )),
    );

    let mut children = SymbolTableBuilder::child_handles(children);
    insert_machine_type_parameter_children(
        builder,
        program,
        program.data_type_parameters(signature.type_parameters),
        &mut children,
        has_sources,
    );
}

pub(in crate::symbols::symbol_table) fn insert_conformance_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    conformance_symbol: SymbolHandle,
    conformance: &symbol_resolved_trees::trait_definition::Conformance,
    has_sources: bool,
) {
    let children = builder.insert_children(
        conformance_symbol,
        type_parameter_seeds(
            program.data_type_parameters(conformance.type_parameters),
            true,
            has_sources,
        ),
    );

    let mut children = SymbolTableBuilder::child_handles(children);
    for parameter in program.data_type_parameters(conformance.type_parameters) {
        let Some(parameter_symbol) = children.next() else {
            break;
        };
        match &parameter.kind {
            symbol_resolved_trees::data::TypeParameterKind::Machine { contract } => {
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
            symbol_resolved_trees::data::TypeParameterKind::Proposition { contract } => {
                builder.insert_children(
                    parameter_symbol,
                    parameter_seeds(program.state_parameters(contract.parameters), has_sources),
                );
            }
            _ => {}
        }
    }
}
