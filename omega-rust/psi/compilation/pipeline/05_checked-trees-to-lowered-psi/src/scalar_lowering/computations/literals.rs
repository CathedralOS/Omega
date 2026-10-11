//! Reserve each computed byte literal's place, then establish it at its formal
//! position. Each checked call operand owns its establishment, even when two
//! operands contain identical bytes.
use super::{CheckedTrees, LoweringError, unsupported};
use crate::emission::structural_byte_sequence_store::ByteLiteral;
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use terminal_psi::StructuralTypeDeclaration;

#[derive(Clone)]
pub(crate) struct Slot {
    pub(crate) source_call: arena::Handle<checked_trees::FlowCallFact>,
    pub(crate) argument_ordinal: usize,
    pub(crate) literal: ByteLiteral,
}

/// A `ByteSequenceLiteral` source carries its exact payload; anything else is
/// a different structural source family.
pub(crate) fn source_bytes(argument: &CheckedUnitStructuralArgumentPlan) -> Option<&[u8]> {
    match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } => {
            Some(bytes.as_slice())
        }
        _ => None,
    }
}

pub(crate) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut slots: Vec<Slot> = Vec::new();
    for handle in super::machine_computation_handles(checked, machine)? {
        let CheckedScalarComputationKind::Call {
            source_call,
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        else {
            continue;
        };
        for (argument_ordinal, argument) in plans
            .structural_arguments
            .span(structural_arguments)
            .ok_or(LoweringError::Unsupported(
                "computed literal has stale structural operands",
            ))?
            .iter()
            .enumerate()
        {
            let CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                continue;
            };
            let Some(bytes) = source_bytes(argument) else {
                continue;
            };
            let declaration = crate::terminal_identities::structural_declaration_by_identity(
                types,
                identities,
                checked,
                argument.type_identity,
                "computed literal has no structural type namespace",
            )?;
            if let Some(existing) = slots.iter().find(|slot| {
                slot.source_call == source_call && slot.argument_ordinal == argument_ordinal
            }) {
                if existing.literal.structural_type != declaration.id
                    || existing.literal.bytes.as_slice() != bytes
                {
                    return unsupported("computed literal occurrence has conflicting types");
                }
                continue;
            }
            slots.push(Slot {
                source_call,
                argument_ordinal,
                literal: ByteLiteral::reserve(
                    identities,
                    bytes,
                    declaration.id,
                    types,
                    next_place,
                )?,
            });
        }
    }
    Ok(slots)
}

/// Publish only established reservations into the machine's existing literal
/// namespace. Unit literal operations carry no result type, so their exact
/// prepared slot supplies the declaration without granting result custody.
pub(crate) fn retain_declarations<'a>(
    slots: &[Slot],
    operations: impl Iterator<Item = &'a terminal_psi::Operation>,
    declarations: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
) -> Result<(), LoweringError> {
    for operation in operations {
        if let Some(result) = operation.result.structural() {
            if slots.iter().any(|slot| slot.literal.place == result.place) {
                declarations.push(terminal_psi::StructuralPlaceDeclaration {
                    id: result.place,
                    kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    },
                });
            }
            continue;
        }
        let terminal_psi::OperationKind::EstablishByteSequenceLiteral { destination, .. } =
            &operation.kind
        else {
            continue;
        };
        let Some(structural_type) = slots
            .iter()
            .find_map(|slot| slot.literal.literal_type(*destination))
        else {
            // Ordinary call operands and named locals publish their own slots.
            continue;
        };
        let declaration_ordinal =
            crate::emission::next_byte_sequence_literal_ordinal(declarations)?;
        declarations.push(terminal_psi::StructuralPlaceDeclaration {
            id: *destination,
            kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            },
        });
    }
    Ok(())
}
