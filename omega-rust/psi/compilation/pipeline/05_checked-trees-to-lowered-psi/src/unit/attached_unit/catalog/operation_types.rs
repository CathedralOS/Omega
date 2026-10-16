//! Types named by an operation, independently of its body's control shape.
//! A descriptor temporary and an uninitialized local need their declarations
//! even when neither type occurs in a signature or an earlier value binding.
use super::*;

pub(in crate::unit::attached_unit) fn collect_operation_type_roots(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
) -> Result<(), LoweringError> {
    for argument in operation.call_structural_arguments() {
        roots.push(argument.type_identity);
        roots.extend(argument.borrowed_root_type_identity());
        if let Some(storage) = argument.subslice_storage_type_identity()
            && storage.is_valid()
        {
            roots.push(storage);
        }
    }
    if let Some((result, _)) = operation.discardable_structural_result() {
        roots.push(result.type_identity);
    }
    match operation {
        CheckedUnitEffectOperationPlan::CaptureAssignmentDestination { destination, .. } => {
            roots.push(destination.type_identity);
        }
        CheckedUnitEffectOperationPlan::EstablishReference { result, source, .. } => {
            roots.push(result.type_identity);
            roots.push(source.type_identity);
        }
        CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { result, .. },
        )
        | CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ByteSequence { result, .. },
        ) => {
            roots.push(result.type_identity);
        }
        CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } => {
            roots.push(result.type_identity);
            if let Some(storage) = source.subslice_storage_type_identity()
                && storage.is_valid()
            {
                roots.push(storage);
            }
        }
        CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
        ) => {
            // A temporary owns its declared carrier even when the value graph
            // names only borrowed backing. A formal's ingress type cannot
            // substitute for this stored descriptor's type.
            roots.push(result.type_identity);
            collect_value_roots(checked, *value, roots)?;
            if establishes_byte_element_view(checked, plans, result.type_identity, *value)? {
                if let Some(identity) =
                    crate::terminal_identities::byte_view_element_identity(plans.structural_types())
                {
                    roots.push(identity);
                }
            }
        }
        CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::Primitive { type_identity, .. },
        ) => {
            roots.push(*type_identity);
        }
        CheckedUnitEffectOperationPlan::MoveStructuralField { result, source, .. } => {
            roots.push(result.type_identity);
            roots.push(source.type_identity);
        }
        CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
            destination:
                checked_trees::CheckedStoreDestination {
                    leaf:
                        checked_trees::CheckedStoreLeaf::StructuralStore { type_identity, .. }
                        | checked_trees::CheckedStoreLeaf::OwnedRebind { type_identity },
                    ..
                },
            value: checked_trees::CheckedStoreValue::Place(value),
            ..
        }) => {
            roots.push(*type_identity);
            roots.push(value.type_identity.clone());
        }
        CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
            value: checked_trees::CheckedStoreValue::Bytes(value),
            ..
        }) => {
            for (_, node) in value.nodes.iter() {
                if let checked_trees::CheckedByteSequenceValue::Descriptor { source, .. } = node
                    && !matches!(source.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }) {
                    roots.push(source.type_identity);
                }
            }
        }
        _ => {}
    }
    Ok(())
}
