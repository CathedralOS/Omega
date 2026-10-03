//! Indexed scalar-element store through a mutable element view.
use super::{LiveDefinitions, observations};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::IntegerSign;
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::StructuralTypeShape;

/// An indexed element write names one view place verified mutable. The view
/// is a signature parameter or non-entry block parameter declared
/// `&mut [T]`, or a local whose establishment lent the collection under
/// `MutableBorrow` — the same descriptor derivation a read resolves, gated
/// here on the exclusive loan the verifier approved for the store.
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::ElementViewWrite {
        path,
        psi_operation,
        destination,
        index,
        value,
        length,
        obligation,
    } = operation
    else {
        return Err(invalid());
    };
    if live.lengths.get(length) != Some(destination) {
        return Err(invalid());
    }
    let values = observations::scalar_values(live, &prepared.scalar_parameters)?;
    let (view, view_structural_type) =
        crate::lowering::scalar::element_views::mutable_element_view_for_place(
            function,
            structural_types,
            &prepared.parameters,
            crate::lowering::scalar::element_views::ElementViewBackings::new(
                &live.structural_homes,
                &live.address_joins,
            ),
            &values,
            &live.lengths,
            *destination,
        )?;
    let StructuralTypeShape::ElementView { element } = &structural_types
        .get(&view_structural_type)
        .ok_or_else(invalid)?
        .shape
    else {
        return Err(invalid());
    };
    let Some(element_scalar) = terminal_semantics::element_view_leaf_scalar(
        structural_types.catalog().as_slice(),
        *element,
        path,
    ) else {
        return Err(invalid());
    };
    // Resolve the authored leaf value from the common dominating scalar roster.
    let index_value = *live.integers.get(index).ok_or_else(invalid)?;
    let element_value = super::scalar_sources::source(*value, function, live)?;
    if index_value.scalar_type().sign() != IntegerSign::Unsigned
        || index_value.scalar_type().bits() != 64
        || index_value.scalar_type().is_address()
        || element_scalar != element_value.scalar_type()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::ElementViewWrite {
        path: path.clone(),
        psi_operation: *psi_operation,
        destination: *destination,
        view,
        index: index_value.into_target_source(*index),
        value: element_value,
        length: *length,
        obligation: *obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
