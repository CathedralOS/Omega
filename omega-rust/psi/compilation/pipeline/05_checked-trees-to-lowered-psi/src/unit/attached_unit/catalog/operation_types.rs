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
    for argument in operation.structural_operands() {
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
        CheckedUnitEffectOperationPlan::AtomicAccess(access) => {
            collect_projection_type_roots(
                plans,
                access.receiver_root_type_identity,
                &access.receiver.path,
                roots,
            )?;
        }
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

/// Concrete storage crossed by one performed projection. Reference declarations
/// alone remain identity-only: only a Referent step demands that definition.
/// This inventory grants no access, membership, lifetime or bounds evidence.
fn collect_projection_type_roots(
    plans: UnitPlans<'_>,
    mut identity: typed_trees::type_identity::TypeIdentityId,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
    roots: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
) -> Result<(), LoweringError> {
    use checked_trees::{
        CheckedUnitStructuralPathSegment as Step, CheckedUnitStructuralTypeShape as Shape,
    };
    let mut selected_case = None;
    roots.push(identity);
    for step in path {
        let declaration = selected_declaration(plans, identity)?;
        match step {
            Step::Case(selected) => {
                let cases = match &declaration.shape {
                    Shape::Sum { cases } | Shape::Mixed { cases, .. } => cases,
                    _ => return unsupported("atomic receiver case lost its declared namespace"),
                };
                selected_case = Some(
                    cases
                        .iter()
                        .find(|case| case.identity == *selected)
                        .ok_or(LoweringError::Unsupported(
                            "atomic receiver case declaration missing",
                        ))?
                        .fields
                        .as_slice(),
                );
                continue;
            }
            Step::Field(selected) => {
                let fields = match selected_case.take() {
                    Some(fields) => fields,
                    None => match &declaration.shape {
                        Shape::Record { fields, .. } | Shape::Mixed { fields, .. } => fields,
                        _ => {
                            return unsupported(
                                "atomic receiver field lost its declared container",
                            );
                        }
                    },
                };
                identity = fields
                    .iter()
                    .find(|field| field.identity == *selected)
                    .and_then(|field| field.field_type.structural_type_identity())
                    .ok_or(LoweringError::Unsupported(
                        "atomic receiver field lost its structural type",
                    ))?;
            }
            Step::Referent => {
                let Shape::Reference {
                    referent_identity, ..
                } = &declaration.shape
                else {
                    return unsupported("atomic receiver pointer crossing lost its descriptor");
                };
                identity = *referent_identity;
            }
            Step::FixedIndex(_) => {
                identity = match &declaration.shape {
                    Shape::FixedArray {
                        element_type_identity,
                        ..
                    }
                    | Shape::BorrowedSliceView {
                        element_type_identity,
                        ..
                    } => *element_type_identity,
                    _ => return unsupported("atomic receiver index lost its collection type"),
                };
            }
            _ => return unsupported("atomic receiver type demand has no static storage path"),
        }
        roots.push(identity);
    }
    Ok(())
}
