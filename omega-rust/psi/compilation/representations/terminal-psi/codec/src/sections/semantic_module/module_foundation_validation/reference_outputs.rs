//! Validate foreign completion coordinates at external decoding, not between
//! producer stages. The promise supplies backing only after provider admission;
//! these rows alone neither authorize a call nor prove provider compliance.

use super::{CodecError, TerminalModule, malformed};
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryMachineDeclaration, BoundaryReferenceDestination,
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};

pub(super) fn validate(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<(), CodecError> {
    if !terminal_semantics::descriptor_results::provider_outputs_cover_writable_slots(
        &module.structural_types,
        boundary,
    ) {
        return malformed("boundary reference parameter contract has incomplete leaf coverage");
    }
    let parameter = |position: u32| {
        boundary
            .structural_parameters
            .get(position as usize)
            .filter(|parameter| parameter.position == position)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "boundary reference contract names an absent parameter",
            ))
    };
    let mut destinations = std::collections::BTreeSet::new();
    for output in &boundary.reference_outputs {
        if !destinations.insert((output.destination, output.path.clone())) {
            return malformed("boundary reference contract repeats an output destination");
        }
        let (root, multiplicity) = match output.destination {
            BoundaryReferenceDestination::Parameter(position) => {
                let parameter = parameter(position)?;
                if !matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) {
                    return malformed(
                        "boundary reference output has no writable parameter storage",
                    );
                }
                (parameter.structural_type, parameter.multiplicity)
            }
        };
        validate_leaf(
            module,
            root,
            multiplicity,
            &output.path,
            output.access,
            false,
        )?;
        if output.dependencies.is_empty() {
            return malformed("boundary reference output has no lifetime anchor");
        }
        let mut dependencies = std::collections::BTreeSet::new();
        for dependency in &output.dependencies {
            if !dependencies.insert(dependency) {
                return malformed("boundary reference output repeats a lifetime anchor");
            }
            match dependency {
                BoundaryLifetimeDependency::Referent {
                    argument_index,
                    access,
                } => {
                    let parameter = parameter(*argument_index)?;
                    if *access == StructuralAccess::Owned || parameter.access != *access {
                        return malformed(
                            "boundary reference lifetime anchor differs from formal access",
                        );
                    }
                }
                BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path,
                    access,
                } => {
                    let parameter = parameter(*argument_index)?;
                    if parameter.access == StructuralAccess::WriteOnlyBorrow {
                        return malformed(
                            "boundary reference lifetime anchor reads write-only storage",
                        );
                    }
                    validate_leaf(
                        module,
                        parameter.structural_type,
                        parameter.multiplicity,
                        path,
                        *access,
                        parameter.access == StructuralAccess::SharedBorrow,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_leaf(
    module: &TerminalModule,
    root: semantic_vocabulary::StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
    shared_storage: bool,
) -> Result<(), CodecError> {
    // A completion names stored leaves, not a recipe for following the new
    // pointer. Lifetime input referents use their distinct dependency variant.
    if path.iter().any(|segment| {
        !matches!(
            segment,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return malformed("boundary reference contract has a nonstorage projection");
    }
    let selected =
        terminal_semantics::runtime_structural_path_tip(module.structural_types.iter(), root, path)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "boundary reference contract has an invalid storage path",
            ))?;
    let declared = match selected.shape {
        StructuralTypeShape::Reference { access, .. } => Some(access),
        _ => terminal_semantics::descriptor_results::descriptor_value_access(
            module.structural_types.iter(),
            selected.id,
            multiplicity,
        ),
    };
    let declared = if shared_storage {
        match declared {
            Some(StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow) => {
                Some(StructuralAccess::SharedBorrow)
            }
            _ => None,
        }
    } else {
        declared
    };
    if access == StructuralAccess::Owned || declared != Some(access) {
        return malformed(
            "boundary reference contract endpoint has different access or is not a descriptor",
        );
    }
    Ok(())
}
