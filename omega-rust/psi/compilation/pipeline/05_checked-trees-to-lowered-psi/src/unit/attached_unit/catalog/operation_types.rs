//! Types named by an operation, independently of its body's control shape.
//! A descriptor temporary and an uninitialized local need their declarations
//! even when neither type occurs in a signature or an earlier value binding.
use super::*;

pub(in crate::unit::attached_unit) fn collect_operation_type_roots(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    roots: &mut Vec<String>,
) -> Result<(), LoweringError> {
    for argument in operation.call_structural_arguments() {
        roots.push(argument.type_identity.clone());
        roots.extend(argument.borrowed_root_type_identity().map(str::to_owned));
        if let Some(storage) = argument.subslice_storage_type_identity()
            && !storage.is_empty()
        {
            roots.push(storage.to_owned());
        }
    }
    if let Some((result, _)) = operation.discardable_structural_result() {
        roots.push(result.type_identity.clone());
    }
    match operation {
        CheckedUnitEffectOperationPlan::CaptureAssignmentDestination { destination, .. } => {
            roots.push(destination.type_identity.clone());
        }
        CheckedUnitEffectOperationPlan::EstablishReference { result, source, .. } => {
            roots.push(result.type_identity.clone());
            roots.push(source.type_identity.clone());
        }
        CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. } => {
            roots.push(result.type_identity.clone());
        }
        CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } => {
            roots.push(result.type_identity.clone());
            if let Some(storage) = source.subslice_storage_type_identity()
                && !storage.is_empty()
            {
                roots.push(storage.to_owned());
            }
        }
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } => {
            collect_value_roots(checked, *value, roots)?;
            if establishes_byte_element_view(checked, plans, &result.type_identity, *value)? {
                roots.push(BYTE_VIEW_ELEMENT_TYPE_IDENTITY.to_owned());
            }
        }
        CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { type_identity, .. } => {
            roots.push(type_identity.clone());
        }
        CheckedUnitEffectOperationPlan::MoveStructuralField { result, source, .. } => {
            roots.push(result.type_identity.clone());
            roots.push(source.type_identity.clone());
        }
        CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
            destination:
                checked_trees::CheckedStoreDestination {
                    leaf:
                        checked_trees::CheckedStoreLeaf::WindowRepair { type_identity, .. }
                        | checked_trees::CheckedStoreLeaf::LocalRebind { type_identity },
                    ..
                },
            value: checked_trees::CheckedStoreValue::Place(value),
            ..
        }) => {
            roots.push(type_identity.clone());
            roots.push(value.type_identity.clone());
        }
        CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
            value: checked_trees::CheckedStoreValue::Bytes(value),
            ..
        }) => {
            for (_, node) in value.nodes.iter() {
                if let checked_trees::CheckedByteSequenceValue::Descriptor { source, .. } = node
                    && !matches!(source.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }) {
                    roots.push(source.type_identity.clone());
                }
            }
        }
        _ => {}
    }
    Ok(())
}
