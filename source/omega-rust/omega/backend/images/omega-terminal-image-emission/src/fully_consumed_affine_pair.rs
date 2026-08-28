//! Exact function-scope replay for the no-residual affine `[T; 2]` carrier.

use omega_terminal_machine_code::{
    TerminalInternalUnitCallRecord, TerminalUnitAffineCleanupRecord,
    TerminalUnitParameterHomeRecord,
};
use omega_terminal_target_operations::TerminalCallSiteOwner;
use psi_terminal::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};

pub(crate) fn exact_fully_consumed_affine_pair(
    parameter_homes: &[TerminalUnitParameterHomeRecord],
    calls: &[TerminalInternalUnitCallRecord],
    cleanup: Option<&TerminalUnitAffineCleanupRecord>,
) -> bool {
    let ([home], [first, second], Some(cleanup)) = (parameter_homes, calls, cleanup) else {
        return false;
    };
    if home.multiplicity != StructuralMultiplicity::Affine
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
        return false;
    }
    let Some(root) = cleanup
        .structural_types
        .iter()
        .find(|declaration| declaration.id == home.structural_type)
    else {
        return false;
    };
    let StructuralTypeShape::FixedArray { element, length: 2 } = root.shape else {
        return false;
    };
    if !matches!(
        cleanup
            .structural_types
            .iter()
            .find(|declaration| declaration.id == element)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Record { .. })
    ) {
        return false;
    }
    let moved_index = |call: &TerminalInternalUnitCallRecord, ordinal: usize| {
        let [argument] = call.arguments.as_slice() else {
            return None;
        };
        let [StructuralPathSegment::FixedIndex(index @ (0 | 1))] = argument.path.as_slice() else {
            return None;
        };
        let stride = argument.element_stride?;
        let expected_stride = u32::from(argument.shape.byte_size)
            .checked_next_multiple_of(u32::from(argument.shape.alignment))?;
        (matches!(call.owner, TerminalCallSiteOwner::Operation(_))
            && call.operation_ordinal == ordinal
            && call.result.is_none()
            && call.structural_result.is_none()
            && call.claim_transfers.is_empty()
            && argument.place == home.place
            && argument.access == StructuralAccess::Owned
            && argument.root_structural_type == home.structural_type
            && argument.structural_type == element
            && argument.fixed_array_length == Some(2)
            && stride == expected_stride
            && argument.source == home.source
            && argument.source.shape == home.shape
            && argument.source.shape.alignment == argument.shape.alignment
            && argument.source_home_byte_offset == home.byte_offset
            && u32::from(argument.source.shape.byte_size) == stride.checked_mul(2)?
            && argument.source_byte_offset == stride.checked_mul(u32::try_from(*index).ok()?)?)
        .then_some((*index, argument.shape, stride))
    };
    let Some(first_index) = moved_index(first, 0) else {
        return false;
    };
    let Some(second_index) = moved_index(second, 1) else {
        return false;
    };
    (first_index.0 != second_index.0)
        && first.owner != second.owner
        && first_index.1 == second_index.1
        && first_index.2 == second_index.2
        && first
            .code_offset
            .checked_add(first.byte_count)
            .is_some_and(|end| end <= second.code_offset)
}
