use super::*;

/// The shared result-place shape for element-view producers: an exact
/// unqualified `ElementView` structural result owned by this operation,
/// spelling the custody multiplicity its loan class declares (`Unrestricted`
/// for shared views, `Affine` for the exclusive mutable loan).
fn element_view_result_place(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    result: &terminal_psi::StructuralOperationResult,
    multiplicity: StructuralMultiplicity,
) -> bool {
    result.multiplicity == multiplicity
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && module.structural_types.iter().any(|row| {
            row.id == result.structural_type
                && (matches!(row.shape, StructuralTypeShape::ElementView { .. })
                    // A `&[u8]` view is the same establishment over `u8`
                    // elements; the byte carrier keeps no element field.
                    || row.shape.is_borrowed_byte_view())
        }) && machine.structural_places.iter().any(|row| {
        row.id == result.place
            && matches!(row.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type)
    })
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_establish_element_view(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishElementView { .. } = &operation.kind else {
        unreachable!("dispatched validate_establish_element_view")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("element-view establishment requires a structural result");
    };
    if !element_view_result_place(
        module,
        machine,
        operation,
        result,
        StructuralMultiplicity::Unrestricted,
    ) {
        return malformed("element-view establishment requires its exact borrowed result place");
    }
    // Full module validation independently checks source access, the
    // collection's declared element type, custody, and establishment order.
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_establish_mutable_element_view(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishMutableElementView { .. } = &operation.kind else {
        unreachable!("dispatched validate_establish_mutable_element_view")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("mutable element-view establishment requires a structural result");
    };
    if !element_view_result_place(
        module,
        machine,
        operation,
        result,
        StructuralMultiplicity::Affine,
    ) {
        return malformed(
            "mutable element-view establishment requires its exact borrowed result place",
        );
    }
    // Full module validation independently checks the mutable source access,
    // the collection's declared element type, custody, and establishment
    // order.
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_element_view_subslice(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ElementViewSubslice { .. } = &operation.kind else {
        unreachable!("dispatched validate_element_view_subslice")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("element-view subslice requires a structural result");
    };
    if !matches!(
        result.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !element_view_result_place(module, machine, operation, result, result.multiplicity)
    {
        return malformed("element-view subslice requires its exact borrowed result place");
    }
    // Mutable range holders are affine, as mutable whole views are. Their
    // source authority, bounds and lifetime remain independent verifier work.
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_element_view_length(
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ElementViewLength { .. } = &operation.kind else {
        unreachable!("dispatched validate_element_view_length")
    };
    let expected = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
    );
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return malformed("element-view length requires an unsigned 64-bit scalar result");
    }
    // Full module validation independently checks source custody and the
    // result's exact extent provenance.
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_element_view_read(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ElementViewRead { source, path, .. } = &operation.kind else {
        unreachable!("dispatched validate_element_view_read")
    };
    let Some(source_type) = place_structural_type(machine, *source) else {
        return malformed("element-view read has an undeclared source place");
    };
    let Some(element) =
        module
            .structural_types
            .iter()
            .find_map(|row| match (row.id == source_type, &row.shape) {
                (true, StructuralTypeShape::ElementView { element }) => Some(*element),
                _ => None,
            })
    else {
        return malformed("element-view read requires an element-view source");
    };
    let Some(expected) =
        terminal_semantics::element_view_leaf_scalar(&module.structural_types, element, path)
    else {
        return malformed("element-view read requires a scalar element leaf");
    };
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return malformed("element-view read requires the element leaf's scalar result");
    }
    // Full module validation independently checks length provenance, custody,
    // operand types and dominance before encoding or after decoding.
    Ok(())
}
