//! Reserve each computed byte literal's place, then establish it at its formal
//! position. Each checked call operand owns its establishment, even when two
//! operands contain identical bytes.
use super::{CheckedTrees, LoweringError, unsupported};
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Clone)]
pub(crate) struct Slot {
    pub(crate) source_call: arena::Handle<checked_trees::FlowCallFact>,
    pub(crate) argument_ordinal: usize,
    pub(crate) bytes: Vec<u8>,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
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
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut slots: Vec<Slot> = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        let (CheckedScalarComputationKind::Call {
            source_call,
            structural_arguments,
            ..
        }
        | CheckedScalarComputationKind::BoundaryCall {
            source_call,
            structural_arguments,
            ..
        }) = plans.nodes.get(handle).kind
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
            let declaration = types
                .iter()
                .find(|declaration| declaration.identity == argument.type_identity)
                .ok_or(LoweringError::Unsupported(
                    "computed literal has no structural type namespace",
                ))?;
            if !matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
            ) {
                return unsupported("computed literal is not a borrowed byte view");
            }
            if let Some(existing) = slots.iter().find(|slot| {
                slot.source_call == source_call && slot.argument_ordinal == argument_ordinal
            }) {
                if existing.structural_type != declaration.id || existing.bytes.as_slice() != bytes
                {
                    return unsupported("computed literal occurrence has conflicting types");
                }
                continue;
            }
            slots.push(Slot {
                source_call,
                argument_ordinal,
                bytes: bytes.to_vec(),
                place: super::super::place_id(super::super::allocate_dense(next_place)?),
                structural_type: declaration.id,
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
        let terminal_psi::OperationKind::EstablishByteSequenceLiteral {
            destination,
            bytes,
            qualifications,
        } = &operation.kind
        else {
            continue;
        };
        let Some(slot) = slots.iter().find(|slot| slot.place == *destination) else {
            // Ordinary call operands and named locals publish their own slots.
            continue;
        };
        if operation.result != terminal_psi::OperationResult::Unit
            || *bytes != slot.bytes
            || !qualifications.is_empty()
            || declarations
                .iter()
                .any(|declaration| declaration.id == *destination)
        {
            return unsupported("computed literal establishment disagrees with its reservation");
        }
        let declaration_ordinal =
            crate::emission::next_byte_sequence_literal_ordinal(declarations)?;
        declarations.push(terminal_psi::StructuralPlaceDeclaration {
            id: slot.place,
            kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type: slot.structural_type,
            },
        });
    }
    Ok(())
}
