//! Rejoin owned aggregate actuals to exact incoming, produced or block-owned backing.
use super::{
    CallSignature, LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction,
    StructuralAccess, evaluate_call_plan,
};
use legalized_operations::LegalizedScalarInstructionKind;

pub(super) fn validate_owned_arguments(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
) -> Option<()> {
    if !call.claim_transfers.is_empty() {
        return None;
    }
    for argument in &call.arguments {
        let LegalizedScalarArgument::Structural {
            semantic, target, ..
        } = argument
        else {
            continue;
        };
        if semantic.access != StructuralAccess::Owned {
            continue;
        }
        let shape = super::owned_value_shape(source, target.structural_type)?;
        let types = &source.structural.as_ref()?.structural_types;
        let (selected, offset, indices) =
            crate::structural_inputs::structural_reference_input::runtime_projection(
                target.root_structural_type,
                &semantic.path,
                types,
            )?;
        if semantic.access != StructuralAccess::Owned
            || target.place != semantic.place
            || target.access != semantic.access
            || target.path != semantic.path
            || selected != target.structural_type
            || target.shape != shape
            || target.destination.shape != shape
            || target.source_byte_offset != offset
            || !indices.is_empty()
            || !target.indices.is_empty()
            || target.fixed_array_length.is_some()
            || target.element_stride.is_some()
            || !crate::selection::aggregate_result_input::owned_argument_placement(
                &target.destination,
            )
        {
            return None;
        }
        match target.source {
            target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
                if psi_operation == operation {
                    return None;
                }
                let producer = source
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .find(|row| row.operation == psi_operation)?;
                // An empty-record local transports no bytes: its
                // establishment must mint exactly this place of exactly this
                // empty type.
                if let LegalizedScalarInstructionKind::EstablishTrivialAffineLocal {
                    place,
                    structural_type,
                } = &producer.kind
                {
                    if place.id != semantic.place
                        || structural_type.id != target.root_structural_type
                        || !semantic.path.is_empty()
                        || shape.byte_size != 0
                    {
                        return None;
                    }
                    continue;
                }
                let result = match &producer.kind {
                    LegalizedScalarInstructionKind::EstablishCase { result, .. } => {
                        crate::selection::aggregate_result_input::fields(source, producer)?;
                        result
                    }
                    LegalizedScalarInstructionKind::EstablishRecord { result, .. } => {
                        crate::selection::record_input::fields(source, producer)?;
                        result
                    }
                    LegalizedScalarInstructionKind::EstablishScalarArray { result, .. } => {
                        crate::selection::scalar_array_input::elements(source, producer)?;
                        result
                    }
                    // The copy instruction validates its source extent. The
                    // call consumes its fresh result home, not that source.
                    LegalizedScalarInstructionKind::StructuralLeafCopy { result, .. } => result,
                    LegalizedScalarInstructionKind::Call(produced) => {
                        crate::selection::aggregate_result_input::call_result(source, produced)?.0
                    }
                    _ => return None,
                };
                // The result's signature preconditions were already bound
                // against the destination roster at call-edge replay; the
                // transport here re-checks only what selection owns.
                if result.place != semantic.place
                    || result.structural_type != target.root_structural_type
                    || result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                    || !result.claims.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || (!semantic.path.is_empty() && !result.qualifications.is_empty())
                {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
                let mut parameters = source.blocks.iter().flat_map(|owner| {
                    owner
                        .structural_parameters
                        .iter()
                        .filter(move |parameter| parameter.place == place)
                        .map(move |parameter| (owner.id, parameter))
                });
                let (owner, parameter) = parameters.next()?;
                if parameters.next().is_some()
                    || block != owner
                    || block == source.entry_block
                    || place != semantic.place
                    || parameter.access != StructuralAccess::Owned
                    || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                    || parameter.structural_type != target.root_structural_type
                    || !parameter.projected_qualifications.is_empty()
                    || (!semantic.path.is_empty() && !parameter.qualifications.is_empty())
                    || !source
                        .structural
                        .as_ref()?
                        .structural_places
                        .iter()
                        .any(|declaration| {
                            declaration.id == place
                                && declaration.kind
                                    == semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                                        block,
                                        position: parameter.position,
                                    }
                        })
                {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::Placement(ref placement) => {
                let parameter = source
                    .structural
                    .as_ref()?
                    .parameters
                    .iter()
                    .find(|parameter| parameter.semantic.place == semantic.place)?;
                if !semantic.path.is_empty()
                    || parameter.semantic.access != StructuralAccess::Owned
                    || parameter.semantic.multiplicity
                        == terminal_psi::StructuralMultiplicity::Linear
                    || parameter.semantic.structural_type != target.structural_type
                    || !parameter.semantic.projected_qualifications.is_empty()
                    || placement != &parameter.target.placement
                    || placement.shape != shape
                    || !crate::selection::aggregate_result_input::owned_argument_placement(
                        placement,
                    )
                {
                    return None;
                }
            }
            _ => return None,
        }
    }
    let expected = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: call
                .arguments
                .iter()
                .map(|argument| argument.placement().shape)
                .collect(),
            result: call
                .result_placement
                .as_ref()
                .map(|placement| placement.shape),
        },
    )
    .ok()?;
    (expected == call.call_plan).then_some(())
}
