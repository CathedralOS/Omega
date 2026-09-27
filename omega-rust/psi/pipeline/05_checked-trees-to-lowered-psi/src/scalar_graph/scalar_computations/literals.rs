//! Reserve each computed byte literal's place, then establish it at its formal
//! position. The literal owns no authored expression, so slots dedupe on the
//! exact byte payload and its declared view identity.
use super::{CheckedTrees, LoweringError, unsupported};
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Clone)]
pub(crate) struct Slot {
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
        let CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        else {
            continue;
        };
        for argument in plans
            .structural_arguments
            .span(structural_arguments)
            .ok_or(LoweringError::Unsupported(
                "computed literal has stale structural operands",
            ))?
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
            if let Some(existing) = slots.iter().find(|slot| slot.bytes == *bytes) {
                if existing.structural_type != declaration.id {
                    return unsupported("computed literal occurrence has conflicting types");
                }
                continue;
            }
            slots.push(Slot {
                bytes: bytes.to_vec(),
                place: super::super::place_id(super::super::allocate_dense(next_place)?),
                structural_type: declaration.id,
            });
        }
    }
    Ok(slots)
}
