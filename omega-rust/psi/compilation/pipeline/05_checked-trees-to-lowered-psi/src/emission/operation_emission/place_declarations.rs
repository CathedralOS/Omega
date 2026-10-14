//! Derive the `OperationResult` place declarations a leaf copy needs from the
//! operations themselves.
//!
//! Leaf-copy producers can mint their result places inside operation
//! emission — after the machine frames have already reserved declaration
//! identities — so no frame channel can carry them. Rather than threading a
//! private drain through every machine assembly, this pass materializes the
//! declaration from the emitted operation row: producer identity and
//! structural type are exactly what the verifier's foundation check compares
//! against, and the abstract-operations lowering refuses the copy when the
//! declaration is absent. The scan runs once per emitted module, ahead of
//! validation; declarations a frame already supplied retain their provenance.
//! Canonicalize the complete roster even when every copy was predeclared:
//! assembly can append dense result or provider places after high-ID copy
//! places. Whether this scan adds a declaration does not determine ordering.

use std::collections::BTreeSet;

use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{StructuralPlaceDeclaration, TerminalModule};

pub(crate) fn reconcile_leaf_copy_place_declarations(module: &mut TerminalModule) {
    for machine in &mut module.machines {
        let mut declared: BTreeSet<_> = machine
            .structural_places
            .iter()
            .map(|place| place.id)
            .collect();
        let mut additions = Vec::new();
        for block in &machine.blocks {
            for operation in &block.operations {
                if !matches!(
                    operation.kind,
                    terminal_psi::structural_leaf_copy_pair_pattern!()
                ) {
                    continue;
                }
                let Some(result) = operation.result.structural() else {
                    continue;
                };
                if !declared.insert(result.place) {
                    continue;
                }
                additions.push(StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    },
                });
            }
        }
        machine.structural_places.extend(additions);
        machine.structural_places.sort_by_key(|place| place.id);
    }
}

/// Structural storage exists at an emitted result, independently of the source
/// expression or body representation that selected its producer.
/// The operation-result place-declaration row every structural-place scan
/// emits for an operation that produces a structural result.
pub(crate) fn operation_result_declaration(
    operation: &terminal_psi::Operation,
) -> Option<StructuralPlaceDeclaration> {
    let result = operation.result.as_structural()?;
    Some(StructuralPlaceDeclaration {
        id: result.place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation.id,
            structural_type: result.structural_type,
        },
    })
}

pub(crate) fn operation_results<'a>(
    operations: impl Iterator<Item = &'a terminal_psi::Operation>,
) -> impl Iterator<Item = StructuralPlaceDeclaration> {
    operations.filter_map(operation_result_declaration)
}

/// A body may already have declared an ordinary result encountered by this
/// scan. Reuse only the exact producer and carrier; a conflicting identity is
/// an emission error, not a reason to replace the earlier declaration.
pub(crate) fn retain_operation_results<'a>(
    operations: impl Iterator<Item = &'a terminal_psi::Operation>,
    declarations: &mut Vec<StructuralPlaceDeclaration>,
) -> Result<(), crate::lowering_error::LoweringError> {
    for declaration in operation_results(operations) {
        if let Some(existing) = declarations
            .iter()
            .find(|existing| existing.id == declaration.id)
        {
            if *existing != declaration {
                return crate::lowering_error::unsupported(
                    "computed result conflicts with an existing place declaration",
                );
            }
        } else {
            declarations.push(declaration);
        }
    }
    Ok(())
}

/// Computed structural continuations publish arrivals after frame reservation.
/// Derive their declarations from the actual emitted block and ordered formal,
/// retaining existing provenance only when it names that exact same arrival.
pub(crate) fn retain_block_parameters<'a>(
    blocks: impl Iterator<Item = &'a terminal_psi::Block>,
    declarations: &mut Vec<StructuralPlaceDeclaration>,
) -> Result<(), crate::lowering_error::LoweringError> {
    for block in blocks {
        for (position, parameter) in block.structural_parameters.iter().enumerate() {
            let position = u32::try_from(position).map_err(|_| {
                crate::lowering_error::LoweringError::Unsupported(
                    "computed structural parameter position overflows",
                )
            })?;
            if parameter.position != position {
                return crate::lowering_error::unsupported(
                    "computed structural parameter positions are not dense",
                );
            }
            let declaration = StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::BlockParameter {
                    block: block.id,
                    position,
                },
            };
            if let Some(existing) = declarations
                .iter()
                .find(|existing| existing.id == declaration.id)
            {
                if *existing != declaration {
                    return crate::lowering_error::unsupported(
                        "computed parameter conflicts with an existing place declaration",
                    );
                }
            } else {
                declarations.push(declaration);
            }
        }
    }
    Ok(())
}
