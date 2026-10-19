//! Resolve a Build operation's static operand path to its declaration symbol
//! over the typed program.

use crate::expression::StaticMachineArgument;
use crate::typed_trees::TypedTrees;
use symbols::JoinedPath;

/// Declaration roles of non-callable Product operands. A requirement member,
/// a boundary subject and a representation have different declaration roles;
/// none acquires ordinary host-call nameability through this selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductOperandKind {
    BoundarySubject,
    Provider,
    Requirement,
    Data,
    Representation,
}

/// A product description names a declaration, not an applied type, value or
/// evidence projection. Checking and later path resolution share this shape.
pub fn is_declaration_operand(argument: &StaticMachineArgument) -> bool {
    !argument.type_reference.is_valid()
        && argument.application.is_none()
        && argument.const_literal.is_none()
        && argument.evidence_projection.is_none()
        && !argument.path.is_empty()
}

/// Resolve one designated Product declaration with its actual role and source
/// scope. These descriptions name declarations, not applied host values;
/// conformance formation remains with the conformance application owner.
pub fn resolve_product_operand(
    typed: &TypedTrees,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
    kind: ProductOperandKind,
) -> Option<symbols::SymbolHandle> {
    let program = typed.product_declarations();
    if !is_declaration_operand(argument) {
        return None;
    }
    let path = argument.path.joined_path();
    let candidates = match kind {
        ProductOperandKind::BoundarySubject => program
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
            .collect::<Vec<_>>(),
        ProductOperandKind::Provider => program
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
            .collect(),
        ProductOperandKind::Requirement => program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .map(move |requirement| (requirement.symbol, definition.is_public))
            })
            .collect(),
        ProductOperandKind::Data => program
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .collect(),
        ProductOperandKind::Representation => program
            .conformances()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .collect(),
    };
    let token_bound_machines = if kind == ProductOperandKind::BoundarySubject {
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
