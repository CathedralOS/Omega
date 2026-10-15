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
                && (matches!(row.shape, StructuralTypeShape::ElementView { access, .. }
                    if access.is_none_or(|access| match multiplicity {
                        StructuralMultiplicity::Unrestricted => access == terminal_psi::StructuralAccess::SharedBorrow,
                        StructuralMultiplicity::Affine => access == terminal_psi::StructuralAccess::MutableBorrow,
                        StructuralMultiplicity::Linear => false,
                    }))
                    // A `&[u8]` view is the same establishment over `u8`
                    // elements; the byte carrier keeps no element field.
                    || row.shape.is_borrowed_byte_view()
                    || (row.shape.is_borrowed_byte_carrier()
                        && fixed_descriptor_establishment(module, machine, operation, &row.shape)))
        }) && machine.structural_places.iter().any(|row| {
        row.id == result.place
            && matches!(row.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type)
    })
}

fn fixed_descriptor_establishment(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    shape: &StructuralTypeShape,
) -> bool {
    let (source, element, region) = match &operation.kind {
        OperationKind::EstablishElementView {
            source,
            element,
            region,
            ..
        }
        | OperationKind::EstablishMutableElementView {
            source,
            element,
            region,
            ..
        } => (source, *element, *region),
        _ => return false,
    };
    let extent = place_structural_type(machine, source.place)
        .and_then(|root| validate_structural_path(module, root, &source.path).ok())
        .and_then(|collection| {
            terminal_semantics::collection_backing(
                module.structural_types.iter(),
                collection,
                element,
            )
        })
        .map(|backing| backing.extent);
    terminal_semantics::collection_view_descriptor(
        shape,
        extent,
        region,
        &source.path,
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == element),
    )
    .is_some()
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
    let OperationKind::ElementViewSubslice { source, .. } = &operation.kind else {
        unreachable!("dispatched validate_element_view_subslice")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("element-view subslice requires a structural result");
    };
    if !matches!(
        result.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) || !element_view_result_place(module, machine, operation, result, result.multiplicity)
    {
        return malformed("element-view subslice requires its exact borrowed result place");
    }
    if source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return malformed("element subslice requires readable descriptor access");
    }
    let Some(root) = place_structural_type(machine, source.place) else {
        return malformed("element subslice has no source declaration");
    };
    let selected = validate_structural_path(module, root, &source.path)?;
    let same_element = module
        .structural_type(selected)
        .zip(module.structural_type(result.structural_type))
        .is_some_and(|(source, result)| {
            matches!((&source.shape, &result.shape),
            (StructuralTypeShape::ElementView { element: source, .. },
             StructuralTypeShape::ElementView { element: result, .. }) if source == result)
        });
    if !same_element
        || (result.multiplicity == StructuralMultiplicity::Affine
            && !matches!(
                source.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
            ))
    {
        return malformed("element subslice must preserve its selected descriptor and access");
    }
    // Mutable range holders are affine, as mutable whole views are. Their
    // source authority, bounds and lifetime remain independent verifier work.
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_element_view_length(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ElementViewLength { source } = &operation.kind else {
        unreachable!("dispatched validate_element_view_length")
    };
    if source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return malformed("element-view length requires readable descriptor access");
    }
    let Some(root) = place_structural_type(machine, source.place) else {
        return malformed("element-view length has no source declaration");
    };
    let selected = validate_structural_path(module, root, &source.path)?;
    if !module.structural_type(selected).is_some_and(|declaration| {
        matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
    }) {
        return malformed("element-view length requires the exact selected element descriptor");
    }
    super::validate_u64_length_result(
        operation,
        "element-view length requires an unsigned 64-bit scalar result",
    )
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
                (true, StructuralTypeShape::ElementView { element, .. }) => Some(*element),
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
