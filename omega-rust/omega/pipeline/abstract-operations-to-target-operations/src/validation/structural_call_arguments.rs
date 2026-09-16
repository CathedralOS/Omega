//! Reconcile retained structural call arguments against source and callee identity.

use std::collections::BTreeMap;

use abstract_operations::{AbstractBoundaryResult, AbstractFunction, AbstractOperation};
use calling_conventions::{CallingPolicy, evaluate_call_plan};
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId};
use target::NativeTarget;
use target_operations::{
    NativeCallOrigin, TargetFunction, TargetStructuralArgument, TargetUnitOperation,
    TargetUnitScalarCallArgument,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration,
};

use super::structural_shapes;

struct TargetCall<'a> {
    origin: &'a NativeCallOrigin,
    callee: semantic_vocabulary::MachineId,
    call_plan: &'a calling_conventions::CallPlan,
    scalar_arguments: &'a [TargetUnitScalarCallArgument],
    arguments: &'a [TargetStructuralArgument],
}

/// The semantic referent declaration bound to one argument place: the root
/// structural type and the access that declaration grants. Operation
/// establishments own their storage; caller and block parameters keep their
/// declared access.
struct RootDeclaration {
    structural_type: StructuralTypeId,
    access: StructuralAccess,
}

pub(super) fn validate(
    source: &AbstractFunction,
    source_functions: &[AbstractFunction],
    target: &TargetFunction,
    declarations: &[StructuralTypeDeclaration],
    native_target: NativeTarget,
) -> Result<(), OperationId> {
    let target_calls = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let (psi_operation, origin, callee, call_plan, scalar_arguments, arguments) =
                match operation {
                    TargetUnitOperation::Call {
                        origin,
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    }
                    | TargetUnitOperation::StructuralScalarCall {
                        origin,
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    }
                    | TargetUnitOperation::StructuralResultCall {
                        origin,
                        psi_operation,
                        callee,
                        call_plan,
                        scalar_arguments,
                        arguments,
                        ..
                    } => (
                        *psi_operation,
                        origin,
                        *callee,
                        call_plan,
                        scalar_arguments.as_slice(),
                        arguments.as_slice(),
                    ),
                    _ => return None,
                };
            Some((
                psi_operation,
                TargetCall {
                    origin,
                    callee,
                    call_plan,
                    scalar_arguments,
                    arguments,
                },
            ))
        })
        .collect::<BTreeMap<_, _>>();

    let roots = canonical_roots(source);

    for operation in &source.operations {
        let (psi_operation, source_callee, scalar_arguments, structural_arguments) = match operation
        {
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                ..
            }
            | AbstractOperation::CallStructuralScalar {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                ..
            } => {
                if target_calls
                    .get(psi_operation)
                    .is_some_and(|call| call.origin != &NativeCallOrigin::Authored)
                {
                    return Err(*psi_operation);
                }
                (
                    *psi_operation,
                    *callee,
                    arguments.as_slice(),
                    structural_arguments.as_slice(),
                )
            }
            AbstractOperation::BoundaryCall {
                psi_operation,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                ..
            } => {
                let Some(call) = target_calls.get(psi_operation) else {
                    continue;
                };
                let NativeCallOrigin::InstalledProvider {
                    boundary: expected,
                    provider,
                    completion_claim_sources: sources,
                    completion_receipts: receipts,
                } = call.origin
                else {
                    return Err(*psi_operation);
                };
                if expected != boundary
                    || provider.boundary != *boundary
                    || provider.candidate != call.callee
                    || sources != completion_claim_sources
                    || receipts != completion_receipts
                    || call.scalar_arguments.len() != arguments.len()
                {
                    return Err(*psi_operation);
                }
                (
                    *psi_operation,
                    provider.candidate,
                    arguments.as_slice(),
                    structural_arguments.as_slice(),
                )
            }
            _ => continue,
        };
        let Some(target_call) = target_calls.get(&psi_operation) else {
            continue;
        };
        if target_call.callee != source_callee
            || target_call.arguments.len() != structural_arguments.len()
            || target_call.scalar_arguments.len() != scalar_arguments.len()
        {
            return Err(psi_operation);
        }
        let Some(callee) = source_functions
            .iter()
            .find(|function| function.machine == source_callee)
        else {
            return Err(psi_operation);
        };
        if callee.structural_parameters.len() != target_call.arguments.len()
            || callee.parameters.len() != scalar_arguments.len()
        {
            return Err(psi_operation);
        }
        // The embedded plan is not authority: independently re-derive the
        // callee's signature and evaluated plan so a substituted scalar row,
        // result placement, or plan detail cannot carry matching destinations.
        let Some(signature) = super::structural_signatures::signature(callee, declarations) else {
            return Err(psi_operation);
        };
        let Ok(expected_plan) =
            evaluate_call_plan(CallingPolicy::native_for_target(native_target), &signature)
        else {
            return Err(psi_operation);
        };
        if target_call.call_plan != &expected_plan {
            return Err(psi_operation);
        }
        for (position, ((actual, value), declared)) in target_call
            .scalar_arguments
            .iter()
            .zip(scalar_arguments)
            .zip(&callee.parameters)
            .enumerate()
        {
            let Ok(index) = u32::try_from(position) else {
                return Err(psi_operation);
            };
            if actual.parameter_index != index
                || actual.source.source_value() != *value
                || actual.scalar_type() != declared.scalar_type
                || actual.placement != expected_plan.parameters[position]
            {
                return Err(psi_operation);
            }
        }
        for (index, ((actual, semantic), declared)) in target_call
            .arguments
            .iter()
            .zip(structural_arguments)
            .zip(&callee.structural_parameters)
            .enumerate()
        {
            if !matches_argument_identity(actual, semantic, declared) {
                return Err(psi_operation);
            }
            let Some(referent) =
                structural_shapes::reconstruct(actual.structural_type, declarations).ok()
            else {
                return Err(psi_operation);
            };
            if actual.shape != structural_shapes::parameter_shape(referent, actual.access) {
                return Err(psi_operation);
            }
            let Some(destination) = expected_plan
                .parameters
                .get(scalar_arguments.len().saturating_add(index))
            else {
                return Err(psi_operation);
            };
            if actual.destination != *destination {
                return Err(psi_operation);
            }
            let Some(root) = roots.get(&semantic.place) else {
                continue;
            };
            // Static subloans carry a pointer to the reconstructed leaf, not
            // an array-view descriptor. Indexed paths need the same carrier
            // and offset replay as fields; owned indexed copies retain their
            // separate array transport metadata.
            let static_borrow = root.access != StructuralAccess::Owned
                && semantic.access != StructuralAccess::Owned
                && semantic.path.iter().all(|segment| {
                    matches!(
                        segment,
                        StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                    )
                });
            if static_borrow
                || semantic
                    .path
                    .iter()
                    .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
            {
                if actual.root_structural_type != root.structural_type {
                    return Err(psi_operation);
                }
                match structural_shapes::project_static_path(
                    root.structural_type,
                    &semantic.path,
                    declarations,
                ) {
                    Ok((projected_type, byte_offset)) => {
                        if !matches_projected_carrier(actual, projected_type, declarations)
                            || actual.source_byte_offset != byte_offset
                        {
                            return Err(psi_operation);
                        }
                    }
                    Err(_) => {
                        if !matches_bounded_byte_field(actual, root, declarations) {
                            return Err(psi_operation);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// A fixed byte array lends its original backing through a view descriptor.
/// The projected storage type therefore differs from the callee's view type;
/// reconstruct the adaptation instead of requiring identity or trusting the
/// producer's length and stride. Ordinary pointer arguments have no adaptation.
fn matches_projected_carrier(
    actual: &TargetStructuralArgument,
    projected: semantic_vocabulary::StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};
    if actual.structural_type == projected {
        return actual.fixed_array_length.is_none() && actual.element_stride.is_none();
    }
    let find_shape = |identity| {
        declarations
            .iter()
            .find(|declaration| declaration.id == identity)
            .map(|declaration| &declaration.shape)
    };
    let Some(StructuralTypeShape::FixedArray { element, length }) = find_shape(projected) else {
        return false;
    };
    actual.access == StructuralAccess::MutableBorrow
        && *length > 0
        && matches!(
            find_shape(*element),
            Some(StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Integer(integer)
            )) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                && integer.bits() == 8 && !integer.is_address()
        )
        && matches!(
            find_shape(actual.structural_type),
            Some(StructuralTypeShape::ByteSequence(
                ByteSequenceCarrier::BorrowedView
            ))
        )
        && actual.fixed_array_length == Some(*length)
        && actual.element_stride == Some(1)
}

/// A bounded inline byte field has no projected carrier identity of its own.
/// The argument still names the field's storage: its live length word and bytes
/// stay in place and the callee sees only the borrowed view. Reconstruct the
/// field's offset and capacity rather than trusting a substituted type or a
/// static-length descriptor's metadata.
fn matches_bounded_byte_field(
    actual: &TargetStructuralArgument,
    root: &RootDeclaration,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let Ok((field_offset, _capacity)) = structural_shapes::bounded_byte_field_geometry(
        root.structural_type,
        &actual.path,
        declarations,
    ) else {
        return false;
    };
    matches!(
        actual.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && match (root.access, actual.access) {
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow,
        )
        | (StructuralAccess::SharedBorrow, StructuralAccess::SharedBorrow) => true,
        _ => false,
    } && declarations.iter().any(|declaration| {
        declaration.id == actual.structural_type
            && declaration.shape
                == terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView,
                )
    }) && actual.source_byte_offset == field_offset
        && actual.fixed_array_length.is_none()
        && actual.element_stride.is_none()
}

fn matches_argument_identity(
    actual: &TargetStructuralArgument,
    semantic: &StructuralArgument,
    declared: &StructuralParameterDeclaration,
) -> bool {
    actual.place == semantic.place
        && actual.path == semantic.path
        && actual.access == semantic.access
        && actual.access == declared.access
        && actual.structural_type == declared.structural_type
}

/// Reconstruct each argument place's referent declaration in the same
/// precedence `structural_argument_sources::expected_sources` binds homes:
/// operation-established places, then non-entry block parameters, then
/// caller parameters. Every projected argument must replay its root type and
/// byte offset against the referent's own declaration, not only when that
/// referent happens to arrive as a machine parameter.
fn canonical_roots(source: &AbstractFunction) -> BTreeMap<PlaceId, RootDeclaration> {
    let mut roots = BTreeMap::new();
    for operation in &source.operations {
        let (place, structural_type) = match operation {
            AbstractOperation::EstablishPrimitiveLocal { result, .. }
            | AbstractOperation::EstablishRecord { result, .. }
            | AbstractOperation::EstablishScalarArray { result, .. }
            | AbstractOperation::EstablishScalarCase { result, .. }
            | AbstractOperation::ByteSequenceSubslice { result, .. }
            | AbstractOperation::CallStructural { result, .. } => {
                (result.place, result.structural_type)
            }
            AbstractOperation::EstablishByteSequenceLiteral {
                place,
                structural_type,
                ..
            } => (place.id, structural_type.id),
            AbstractOperation::BoundaryCall {
                result: AbstractBoundaryResult::Structural(result),
                ..
            } => (result.place, result.structural_type),
            _ => continue,
        };
        roots.entry(place).or_insert(RootDeclaration {
            structural_type,
            access: StructuralAccess::Owned,
        });
    }
    for entry in &source.block_entries {
        if entry.block == source.entry {
            continue;
        }
        for parameter in &entry.structural_parameters {
            roots.entry(parameter.place).or_insert(RootDeclaration {
                structural_type: parameter.structural_type,
                access: parameter.access,
            });
        }
    }
    for parameter in &source.structural_parameters {
        roots.entry(parameter.place).or_insert(RootDeclaration {
            structural_type: parameter.structural_type,
            access: parameter.access,
        });
    }
    roots
}
