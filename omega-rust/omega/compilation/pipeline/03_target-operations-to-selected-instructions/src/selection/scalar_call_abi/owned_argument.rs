//! Rejoin owned aggregate actuals to exact incoming, produced or block-owned backing.
use super::{
    LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction, StructuralAccess,
    evaluate_call_plan,
};
use legalized_operations::LegalizedScalarInstructionKind;

pub(super) fn validate_owned_arguments(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    site: semantic_vocabulary::InvocationSite,
) -> Option<()> {
    super::input_site::use_cut(source, site)?;
    for argument in &call.arguments {
        let LegalizedScalarArgument::Structural {
            semantic,
            target,
            indices,
            ..
        } = argument
        else {
            continue;
        };
        if semantic.access != StructuralAccess::Owned {
            continue;
        }
        let shape = super::owned_value_shape(source, target.structural_type)?;
        let types = &source.structural.as_ref()?.structural_types;
        let (selected, offset, elements) =
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
            || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
                indices, &elements,
            )
            || target.indices.len() != indices.len()
            || target
                .indices
                .iter()
                .zip(indices)
                .any(|(target, selected)| {
                    target.operand.source_value() != selected.operand.value
                        || target.operand.scalar_type() != selected.operand.scalar_type
                        || target.stride != selected.stride
                })
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
                let (_, _, producer) =
                    super::input_site::producer_before_use(source, psi_operation, site)?;
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
                // The producer's computation is independently selected and
                // replayed at its own instruction. Call transport rejoins its
                // declared result, not a whitelist of how the value was made.
                // Keep identity separate from `home`'s borrowing eligibility:
                // whole owned arguments may carry qualifications or a Mixed
                // shape without acquiring permission to borrow that value.
                let (actual_producer, result) =
                    crate::selection::aggregate_result_input::operation_result(
                        source,
                        semantic.place,
                    )?;
                // The result's signature preconditions were already bound
                // against the destination roster at call-edge replay; the
                // transport here re-checks only what selection owns.
                if actual_producer != psi_operation
                    || result.place != semantic.place
                    || result.structural_type != target.root_structural_type
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
                    || parameter.structural_type != target.root_structural_type
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
                // The incoming ABI placement describes the complete root,
                // not the selected child's layout. The exact projection and
                // byte offset were rejoined above against that root type.
                let root_shape = super::owned_value_shape(source, target.root_structural_type)?;
                if parameter.semantic.access != StructuralAccess::Owned
                    || parameter.semantic.structural_type != target.root_structural_type
                    || placement != &parameter.target.placement
                    || placement.shape != root_shape
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
    let expected =
        evaluate_call_plan(source.call_plan.policy, &super::call_abi_signature(call)).ok()?;
    (expected == call.call_plan).then_some(())
}
