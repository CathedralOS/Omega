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
