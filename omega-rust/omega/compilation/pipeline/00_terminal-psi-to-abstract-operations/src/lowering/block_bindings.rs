//! Admit whole shared or exclusive byte views, address joins over plain
//! referents or borrowed bounded byte owners, and plain owned structural
//! arrivals. An owner loan, a shared byte view, or a whole owned bounded-byte
//! value may carry qualifications. Loans retain their declared access;
//! only shared loans freeze their referent. An owned arrival moves its complete `{length, bytes}` payload and
//! exact predicate roster, which Terminal verification replayed at every edge.
//!
//! An address join (see `abstract_operations::control_flow::address_joins`)
//! is the only borrowed arrival an edge may bind through a projected path:
//! its carrier is the address of that static place, so `&a.left` and
//! `&b.right` can meet in one shared parameter, and `&mut a.left` and
//! `&mut b.right` in one mutable parameter, without copying either leaf. A
//! projected argument transfers exactly its join's access, as every later
//! stage replays it. Descriptor views and owned arrivals still bind whole
//! roots; their projected edges have no downstream carrier.

use terminal_psi::{TerminalModule, Terminator};

#[cfg(test)]
mod tests;

use super::LoweringError;

/// Check the complete module before selecting any machine lowering family.
/// Ranked, ordinary, optimizer, and provider replay share this entrance; a
/// descriptor-only transfer need not contain any fenced byte operation.
pub(super) fn validate_structural_block_bindings(
    module: &TerminalModule,
) -> Result<(), LoweringError> {
    for machine in &module.machines {
        for block in &machine.blocks {
            if block
                .structural_parameters
                .iter()
                .enumerate()
                .any(|(position, parameter)| {
                    block.id == machine.entry
                        || parameter.position as usize != position
                        || parameter.is_self
                        || (!parameter.qualifications.is_empty()
                            && !abstract_operations::control_flow::owner_loans::is_bounded_owner_loan_in(
                                parameter,
                                &module.structural_types,
                            )
                            && !qualified_owned_bounded_bytes(module, parameter)
                            && !abstract_operations::control_flow::owner_loans::is_shared_byte_view_in(
                                parameter,
                                &module.structural_types,
                            ))
                        || !parameter.projected_qualifications.is_empty()
                        || !module.structural_types.iter().any(|declaration| {
                            declaration.id == parameter.structural_type
                                && match parameter.access {
                                    terminal_psi::StructuralAccess::Owned => matches!(
                                        parameter.multiplicity,
                                        terminal_psi::StructuralMultiplicity::Affine
                                            | terminal_psi::StructuralMultiplicity::Unrestricted
                                    ),
                                    terminal_psi::StructuralAccess::SharedBorrow
                                    | terminal_psi::StructuralAccess::MutableBorrow => {
                                        abstract_operations::control_flow::address_joins::is_address_join_in(
                                            parameter,
                                            &module.structural_types,
                                        ) || abstract_operations::control_flow::owner_loans::is_shared_view_carrier_loan_in(
                                            parameter,
                                            &module.structural_types,
                                        ) || parameter.multiplicity
                                            == terminal_psi::StructuralMultiplicity::Unrestricted
                                            && (declaration.shape.is_borrowed_byte_carrier()
                                                || matches!(declaration.shape,
                                                    terminal_psi::StructuralTypeShape::ElementView { .. }))
                                    }
                                    terminal_psi::StructuralAccess::WriteOnlyBorrow => false,
                                }
                        })
                })
            {
                return Err(LoweringError::UnsupportedStructuralBlockParameters {
                    machine: machine.id,
                    block: block.id,
                });
            }
            // Pair each argument with its destination declaration: whether a
            // path may appear depends on the parameter's carrier. A missing
            // target or arity mismatch leaves every projected argument closed.
            let unsupported =
                |target: semantic_vocabulary::BlockId,
                 arguments: &[terminal_psi::StructuralArgument]| {
                    let parameters = machine
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == target)
                        .map(|candidate| candidate.structural_parameters.as_slice())
                        .filter(|parameters| parameters.len() == arguments.len());
                    arguments.iter().enumerate().any(|(position, argument)| {
                        unsupported_argument(
                            argument,
                            parameters.and_then(|parameters| parameters.get(position)),
                            &module.structural_types,
                        )
                    })
                };
            let unsupported_edge = match &block.terminator {
                Terminator::Jump {
                    edge,
                    target,
                    structural_arguments,
                    ..
                } => unsupported(*target, structural_arguments).then_some(*edge),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [when_true, when_false]
                    .into_iter()
                    .find(|successor| {
                        unsupported(successor.target, &successor.structural_arguments)
                    })
                    .map(|successor| successor.edge),
                Terminator::StructuralCase { .. }
                | Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnUnitPartialAffine { .. }
                | Terminator::ReturnUnitNominalAffine { .. }
                | Terminator::ReturnStructural { .. }
                | Terminator::Crash { .. } => None,
            };
            if let Some(edge) = unsupported_edge {
                return Err(LoweringError::UnsupportedStructuralSuccessorArguments {
                    machine: machine.id,
                    edge,
                });
            }
        }
    }
    Ok(())
}

fn qualified_owned_bounded_bytes(
    module: &TerminalModule,
    parameter: &terminal_psi::StructuralParameterDeclaration,
) -> bool {
    !parameter.qualifications.is_empty()
        && abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
            parameter,
            &module.structural_types,
        )
        && terminal_semantics::byte_parameter_predicates(module, parameter).is_some()
}

fn unsupported_argument(
    argument: &terminal_psi::StructuralArgument,
    parameter: Option<&terminal_psi::StructuralParameterDeclaration>,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    let projected_address = matches!(
        argument.access,
        terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
    )
        && abstract_operations::control_flow::address_joins::is_static_projection(&argument.path)
        && parameter.is_some_and(|parameter| {
            parameter.access == argument.access
                && abstract_operations::control_flow::address_joins::is_address_join_in(
                    parameter, types,
                )
        });
    (!argument.path.is_empty() && !projected_address)
        || !matches!(
            argument.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::Owned
        )
}
