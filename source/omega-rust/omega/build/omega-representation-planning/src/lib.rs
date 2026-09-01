#![forbid(unsafe_code)]

//! Compiler-owned closure of provider-backed opaque value representations.
//!
//! Packages may publish ordinary named conformances, but only the exact
//! authoritative build machine can activate one as an opaque representation.
//! This crate validates that relationship and retains its closed conformance
//! identity. Physical shape is derived by downstream target consumers from
//! the selected concrete carrier; source never supplies ABI numbers.

use std::path::Path;

pub use omega_representation_selections::{
    OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection, selection_for_opaque,
};
use psi_diagnostics::Diagnostic;
use psi_source::{SourceOrigin, SourceSpan};
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::TypeParameterKind;

mod carrier_closure;

const REPRESENTATION_TRAIT_NAME: &str = "OpaqueRepresentation";
const REPRESENTATION_TRAIT_SOURCE: &str = "core/representation.omg";
const SELECTION_MARKER: &str = "select_representation";

/// Harvest representation selections from the one build machine already
/// admitted by project discovery. Calls elsewhere remain inert syntax.
pub fn harvest_opaque_representation_selections(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |target: &str,
                      arguments: &[psi_typed_trees::expression::StaticMachineArgument],
                      source_span: SourceSpan| {
        if target != SELECTION_MARKER {
            return;
        }
        match close_selection(typed, machine.symbol, arguments, source_span) {
            Ok(selection) => {
                if let Some(prior) =
                    selections
                        .iter()
                        .find(|prior: &&OpaqueRepresentationSelection| {
                            prior.opaque() == selection.opaque()
                        })
                {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "build selects opaque representation `{}` more than once (first selection uses `{}`)",
                            typed.symbols.display_path(selection.opaque(), "::"),
                            typed.symbols.display_path(prior.application().declaration, "::"),
                        ))
                        .with_source_span(source_span),
                    );
                } else {
                    selections.push(selection);
                }
            }
            Err(diagnostic) => diagnostics.push(diagnostic.with_source_span(source_span)),
        }
    };

    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                psi_typed_trees::statement::StatementNode::Expression(expression) => {
                    if let psi_typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    {
                        record(
                            call.target.as_str(),
                            &call.machine_arguments,
                            typed.expression_table.source_span(*expression),
                        );
                    }
                }
                psi_typed_trees::statement::StatementNode::Call(call) => {
                    record(
                        call.target.as_str(),
                        &call.machine_arguments,
                        call.source_span,
                    );
                }
                _ => {}
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
}

fn close_selection(
    typed: &TypedTrees,
    selecting_machine: SymbolHandle,
    arguments: &[psi_typed_trees::expression::StaticMachineArgument],
    source_span: SourceSpan,
) -> Result<OpaqueRepresentationSelection, Diagnostic> {
    let [opaque_argument, conformance_argument] = arguments else {
        return Err(Diagnostic::error(
            "opaque representation selection must retain exactly one opaque data path and one named conformance path",
        ));
    };
    let opaque = opaque_argument.symbol;
    let opaque_name = opaque_argument.display_name();
    let Some(opaque_definition) = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == opaque)
    else {
        return Err(Diagnostic::error(format!(
            "opaque representation subject `{opaque_name}` does not resolve to an exact data declaration"
        )));
    };
    if opaque_definition.supply_mode != psi_language_semantics::DataSupplyMode::BoundaryOpaque {
        return Err(Diagnostic::error(format!(
            "representation subject `{opaque_name}` is not boundary-opaque data"
        )));
    }

    if typed.symbols.get(conformance_argument.symbol).kind != SymbolKind::Conformance {
        return Err(Diagnostic::error(format!(
            "opaque representation `{}` does not resolve to an exact named conformance",
            conformance_argument.display_name(),
        )));
    }
    let application = psi_typed_trees_to_checked_trees::close_conformance_application(
        typed,
        conformance_argument,
    )?;
    let conformance = typed
        .conformances()
        .iter()
        .find(|candidate| candidate.symbol == application.declaration)
        .ok_or_else(|| Diagnostic::error("selected opaque conformance disappeared"))?;
    if !is_compiler_owned_opaque_representation_trait(typed, conformance.trait_symbol)
        || application.trait_definition != conformance.trait_symbol
    {
        return Err(Diagnostic::error(format!(
            "conformance `{}` does not satisfy the exact compiler-owned `OpaqueRepresentation` trait",
            conformance_argument.display_name(),
        )));
    }
    let trait_arguments = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    let [trait_argument] = trait_arguments else {
        return Err(Diagnostic::error(format!(
            "conformance `{}` does not retain exactly one opaque representation argument",
            conformance_argument.display_name(),
        )));
    };
    if typed.type_reference_table.type_symbol(*trait_argument) != opaque {
        return Err(Diagnostic::error(format!(
            "conformance `{}` represents `{}`, not selected opaque declaration `{opaque_name}`",
            conformance_argument.display_name(),
            typed.display_type_reference(*trait_argument),
        )));
    }
    let carrier = conformance.carrier_symbol;
    let Some(carrier_definition) = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == carrier)
    else {
        return Err(Diagnostic::error(format!(
            "conformance `{}` has no exact concrete data carrier",
            conformance_argument.display_name(),
        )));
    };
    if carrier_definition.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape {
        return Err(Diagnostic::error(format!(
            "opaque representation carrier `{}` must be ordinary checked-shape data",
            carrier_definition.name,
        )));
    }
    if !typed.data_type_parameters(carrier_definition).is_empty()
        || !carrier_definition.lifetime_parameters.is_empty()
    {
        return Err(Diagnostic::error(format!(
            "opaque representation carrier `{}` is not target-closed",
            carrier_definition.name,
        )));
    }
    carrier_closure::validate_inert_carrier(typed, carrier_definition)?;

    Ok(OpaqueRepresentationSelection::from_validated_application(
        opaque,
        carrier,
        application,
        OpaqueRepresentationLifecycleDisposition::Inert,
        selecting_machine,
        source_span,
    ))
}

/// Whether `symbol` is the exact compiler-owned, capability-free
/// `OpaqueRepresentation<Opaque>` relationship. Package review uses the same
/// check when publishing producer availability; source spelling alone never
/// establishes this role.
pub fn is_compiler_owned_opaque_representation_trait(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> bool {
    let Some(definition) = typed.traits().iter().find(|trait_| trait_.symbol == symbol) else {
        return false;
    };
    let parameters = typed.trait_type_parameters(definition);
    if definition.name.as_str() != REPRESENTATION_TRAIT_NAME
        || definition.is_boundary
        || definition.lifetime_parameters.len() != 0
        || parameters.len() != 1
        || !matches!(parameters[0].kind, TypeParameterKind::Type)
        || !definition.conformance_bounds.is_empty()
        || !typed.trait_requirements(definition).is_empty()
        || !typed.trait_machine_signatures(definition).is_empty()
    {
        return false;
    }
    let Some(span) = typed.symbols.symbol_source_span(symbol) else {
        return false;
    };
    let Some(source) = typed.symbols.source_file(span) else {
        return false;
    };
    source.origin == SourceOrigin::Toolchain
        && source
            .path
            .ends_with(Path::new(REPRESENTATION_TRAIT_SOURCE))
}
