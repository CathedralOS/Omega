//! Exact function-scope replay for bounded affine fixed-array call carriers.

use omega_machine_code::{
    InternalUnitCallRecord, UnitAffineCleanupRecord, UnitParameterHomeRecord,
};
use omega_target_operations::CallSiteOwner;
use psi_terminal::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};

pub(crate) fn exact_fully_consumed_affine_pair(
    parameter_homes: &[UnitParameterHomeRecord],
    calls: &[InternalUnitCallRecord],
    cleanup: Option<&UnitAffineCleanupRecord>,
) -> bool {
    exact_affine_array_calls(parameter_homes, calls, cleanup, 2)
        .is_some_and(|indexes| indexes.as_slice() == [0, 1] || indexes.as_slice() == [1, 0])
}

pub(crate) fn exact_partially_consumed_affine_array(
    parameter_homes: &[UnitParameterHomeRecord],
    calls: &[InternalUnitCallRecord],
    cleanup: Option<&UnitAffineCleanupRecord>,
) -> bool {
    exact_affine_array_calls(parameter_homes, calls, cleanup, 3)
        .is_some_and(|indexes| matches!(indexes.len(), 1 | 2))
        || exact_affine_array_calls(parameter_homes, calls, cleanup, 4)
            .is_some_and(|indexes| indexes.len() == 2)
}

fn exact_affine_array_calls(
    parameter_homes: &[UnitParameterHomeRecord],
    calls: &[InternalUnitCallRecord],
    cleanup: Option<&UnitAffineCleanupRecord>,
    expected_length: u64,
) -> Option<Vec<u64>> {
    let ([home], Some(cleanup)) = (parameter_homes, cleanup) else {
        return None;
    };
    if !matches!((expected_length, calls.len()), (2, 2) | (3, 1 | 2) | (4, 2))
        || home.multiplicity != StructuralMultiplicity::Affine
        || !cleanup.locals.is_empty()
        || cleanup
            .structural_types
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        || cleanup
            .structural_types
            .iter()
            .any(|declaration| declaration.identity.is_empty())
        || cleanup
            .structural_types
            .iter()
            .map(|declaration| declaration.identity.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != cleanup.structural_types.len()
    {
        return None;
    }
    let root = cleanup
        .structural_types
        .iter()
        .find(|declaration| declaration.id == home.structural_type)?;
    let StructuralTypeShape::FixedArray { element, length } = root.shape else {
        return None;
    };
    if length != expected_length {
        return None;
    }
    if !matches!(
        cleanup
            .structural_types
            .iter()
            .find(|declaration| declaration.id == element)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Record { .. })
    ) {
        return None;
    }
    let moved_index = |call: &InternalUnitCallRecord, ordinal: usize| {
        let [argument] = call.arguments.as_slice() else {
            return None;
        };
        let [StructuralPathSegment::FixedIndex(index)] = argument.path.as_slice() else {
            return None;
        };
        let stride = argument.element_stride?;
        let expected_stride = u32::from(argument.shape.byte_size)
            .checked_next_multiple_of(u32::from(argument.shape.alignment))?;
        (matches!(call.owner, CallSiteOwner::Operation(_))
            && call.operation_ordinal == ordinal
            && call.result.is_none()
            && call.structural_result.is_none()
            && call.claim_transfers.is_empty()
            && argument.place == home.place
            && argument.access == StructuralAccess::Owned
            && argument.root_structural_type == home.structural_type
            && argument.structural_type == element
            && *index < expected_length
            && argument.fixed_array_length == Some(expected_length)
            && stride == expected_stride
            && argument.source == home.source
            && argument.source.shape == home.shape
            && argument.source.shape.alignment == argument.shape.alignment
            && argument.source_home_byte_offset == home.byte_offset
            && u32::from(argument.source.shape.byte_size)
                == stride.checked_mul(u32::try_from(expected_length).ok()?)?
            && argument.source_byte_offset == stride.checked_mul(u32::try_from(*index).ok()?)?)
        .then_some((*index, argument.shape, stride))
    };
    let moved = calls
        .iter()
        .enumerate()
        .map(|(ordinal, call)| moved_index(call, ordinal))
        .collect::<Option<Vec<_>>>()?;
    let (first_index, first_shape, first_stride) = *moved.first()?;
    if moved
        .iter()
        .any(|(_, shape, stride)| *shape != first_shape || *stride != first_stride)
        || moved
            .iter()
            .map(|(index, _, _)| *index)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != moved.len()
        || calls.windows(2).any(|pair| {
            pair[0].owner == pair[1].owner
                || pair[0]
                    .code_offset
                    .checked_add(pair[0].byte_count)
                    .is_none_or(|end| end > pair[1].code_offset)
        })
    {
        return None;
    }
    let mut indexes = Vec::with_capacity(moved.len());
    indexes.push(first_index);
    indexes.extend(moved.iter().skip(1).map(|(index, _, _)| *index));
    Some(indexes)
}
