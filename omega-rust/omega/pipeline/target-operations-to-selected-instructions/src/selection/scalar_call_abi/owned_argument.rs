//! Rejoin owned array actuals to their exact incoming fragments or produced home.
use super::*;
use legalized_operations::LegalizedScalarInstructionKind;

pub(super) fn validate_owned_arguments(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
) -> Option<()> {
    if call.source != legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit
        || !call.claim_transfers.is_empty()
    {
        return None;
    }
    for argument in &call.arguments {
        let LegalizedScalarArgument::Structural { semantic, target } = argument else {
            continue;
        };
        if semantic.access != StructuralAccess::Owned {
            continue;
        }
        let (_, _, shape) =
            crate::selection::scalar_array_input::shape(source, target.structural_type)?;
        if semantic.access != StructuralAccess::Owned
            || !semantic.path.is_empty()
            || target.place != semantic.place
            || target.access != semantic.access
            || target.path != semantic.path
            || target.root_structural_type != target.structural_type
            || target.shape != shape
            || target.destination.shape != shape
            || target.source_byte_offset != 0
            || target.fixed_array_length.is_some()
            || target.element_stride.is_some()
            || !crate::selection::aggregate_result_input::direct_fragments(&target.destination)
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
                let result = match &producer.kind {
                    LegalizedScalarInstructionKind::EstablishScalarArray { result, .. } => {
                        crate::selection::scalar_array_input::elements(source, producer)?;
                        result
                    }
                    LegalizedScalarInstructionKind::Call(produced) => {
                        crate::selection::aggregate_result_input::call_result(source, produced)?.0
                    }
                    _ => return None,
                };
                if result.place != semantic.place
                    || result.structural_type != target.structural_type
                    || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !result.claims.is_empty()
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
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
                if parameter.semantic.access != StructuralAccess::Owned
                    || parameter.semantic.multiplicity
                        != terminal_psi::StructuralMultiplicity::Unrestricted
                    || parameter.semantic.structural_type != target.structural_type
                    || !parameter.semantic.qualifications.is_empty()
                    || !parameter.semantic.projected_qualifications.is_empty()
                    || placement != &parameter.target.placement
                    || placement.shape != shape
                    || !crate::selection::aggregate_result_input::direct_fragments(placement)
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
