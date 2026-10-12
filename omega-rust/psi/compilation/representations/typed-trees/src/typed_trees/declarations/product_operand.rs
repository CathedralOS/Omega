//! Resolve a Build operation's static operand path to its declaration symbol
//! over the typed program.

use crate::expression::StaticMachineArgument;
use crate::typed_trees::TypedTrees;
use symbols::JoinedPath;

/// A product description names a declaration, not an applied type, value or
/// evidence projection. Checking and later path resolution share this shape.
pub fn is_declaration_operand(argument: &StaticMachineArgument) -> bool {
    !argument.type_reference.is_valid()
        && argument.application.is_none()
        && argument.const_literal.is_none()
        && argument.evidence_projection.is_none()
        && !argument.path.is_empty()
}

/// Resolve the declaration a Build operand path names: subject position joins
/// boundaries and top-level requirement machines, while non-subject position
/// joins data declarations and non-generic conformances. A path that fails the
/// operand shape (typed, literal, projected, or empty) is not a
/// product operand at all.
pub fn resolve_product_operand(
    typed: &TypedTrees,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
    subject: bool,
) -> Option<symbols::SymbolHandle> {
    let program = typed.product_declarations();
    if argument.type_reference.is_valid()
        || argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
        || argument.path.is_empty()
    {
        return None;
    }
    let path = argument.path.joined_path();
    let candidates = if subject {
        program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .machines()
                    .iter()
                    .filter(|machine| {
                        machine.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                    })
                    .map(|machine| (machine.symbol, machine.is_public)),
            )
            .chain(
                program
                    .all_operators()
                    .filter(|operator| operator.is_boundary)
                    .map(|operator| (operator.symbol, operator.is_public)),
            )
            .collect::<Vec<_>>()
    } else {
        program
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .conformances()
                    .iter()
                    .filter(|definition| {
                        definition.type_parameters.is_empty()
                            && definition.lifetime_parameters.is_empty()
                    })
                    .map(|definition| (definition.symbol, definition.is_public)),
            )
            .collect()
    };
    let token_bound_machines = if subject {
        program
            .machine_token_bindings()
            .iter()
            .filter(|binding| binding.is_boundary)
            .map(|binding| binding.symbol)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    program.symbols.find_build_subject_declaration_from_source(
        &path,
        occurrence,
        candidates,
        &token_bound_machines,
    )
}
