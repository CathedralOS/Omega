use super::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralProjectionPolicy {
    Unit,
    EmptyOnly,
    Projected,
    Boundary,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StructuralSourceContract<'a> {
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) multiplicity: terminal_psi::StructuralMultiplicity,
    pub(crate) access: terminal_psi::StructuralAccess,
    qualifications: &'a [StructuralDomainId],
    projected_qualifications: &'a [terminal_psi::StructuralPathQualification],
}

impl StructuralSourceContract<'_> {
    pub(crate) fn is_unqualified(&self) -> bool {
        self.qualifications.is_empty() && self.projected_qualifications.is_empty()
    }

    pub(crate) fn carries_qualification(
        &self,
        path: &[terminal_psi::StructuralPathSegment],
        domain: StructuralDomainId,
    ) -> bool {
        if path.is_empty() {
            self.qualifications.contains(&domain)
        } else {
            self.projected_qualifications
                .iter()
                .any(|qualification| qualification.path == path && qualification.domain == domain)
        }
    }
}

pub(crate) fn structural_arguments_match(
    caller: &PsiOptimizationFunction,
    arguments: &[terminal_psi::StructuralArgument],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    projection: StructuralProjectionPolicy,
    allow_byte_literal: bool,
) -> bool {
    if arguments.len() != parameters.len() {
        return false;
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let Some(source) = structural_source_contract(caller, argument.place, allow_byte_literal)
        else {
            return false;
        };
        let path_shape_matches = match projection {
            StructuralProjectionPolicy::Unit => {
                argument.path.is_empty()
                    || is_nonempty_field_path(&argument.path)
                    // Write-only subloans may interleave fields and literal
                    // indexes. Exact resolution and materiality below govern
                    // their shape, not a fixed path-depth roster.
                    || argument.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
                    || matches!(
                        argument.path.as_slice(),
                        [terminal_psi::StructuralPathSegment::FixedIndex(_)]
                            | [
                                terminal_psi::StructuralPathSegment::FixedIndex(_),
                                terminal_psi::StructuralPathSegment::FixedIndex(_),
                            ]
                    )
                    || (source.access == terminal_psi::StructuralAccess::Owned
                        && argument.access == terminal_psi::StructuralAccess::Owned
                        && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && is_partial_affine_path(types, source.structural_type, &argument.path))
            }
            StructuralProjectionPolicy::EmptyOnly => argument.path.is_empty(),
            StructuralProjectionPolicy::Projected => true,
            StructuralProjectionPolicy::Boundary => true,
        };
        let Some(actual_type) =
            resolve_structural_path(types, source.structural_type, &argument.path)
        else {
            return false;
        };
        let fixed_byte_view = projection == StructuralProjectionPolicy::Unit
            && (argument.path.is_empty() || is_nonempty_field_path(&argument.path))
            && source.access == terminal_psi::StructuralAccess::MutableBorrow
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.qualifications.is_empty()
            && source.projected_qualifications.is_empty()
            && argument.access == terminal_psi::StructuralAccess::MutableBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty()
            && !caller.entry_claim_declarations.iter().any(|claim| claim.input == argument.place)
            && !caller.content_entry_claims.iter().any(|claim| claim.input.root == argument.place)
            && matches!(types.get(&parameter.structural_type).map(|declaration| &declaration.shape),
                Some(terminal_psi::StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)))
            && types.get(&actual_type).is_some_and(|declaration| {
                let terminal_psi::StructuralTypeShape::FixedArray { element, length: 1.. } = declaration.shape else {
                    return false;
                };
                matches!(types.get(&element).map(|declaration| &declaration.shape),
                    Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)))
                        if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 8 && !integer.is_address())
            });
        if !path_shape_matches
            || (actual_type != parameter.structural_type && !fixed_byte_view)
            || argument.access != parameter.access
            || !structural_access_can_supply(source.access, argument.access)
        {
            return false;
        }
        let indexed_write_only_path_is_material = || {
            !argument.path.iter().any(|segment| {
                matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_))
            }) || (is_material_write_only_type(types, source.structural_type)
                && types.get(&actual_type).is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                            | terminal_psi::StructuralTypeShape::Record { .. }
                    )
                }))
        };
        let unrestricted_write_only_subloan = !argument.path.is_empty()
            && argument.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && matches!(
                source.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && indexed_write_only_path_is_material();
        let unrestricted_mutable_field = is_nonempty_field_path(&argument.path)
            && argument.access == terminal_psi::StructuralAccess::MutableBorrow
            && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
            && source.access == terminal_psi::StructuralAccess::MutableBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        let unrestricted_shared_field = is_nonempty_field_path(&argument.path)
            && argument.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        // Terminal admits indexed write-only arguments only as unrestricted
        // material subloans. Linear fallback cannot supply that authority.
        if projection == StructuralProjectionPolicy::Unit
            && argument.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && argument.path.iter().any(|segment| {
                matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_))
            })
            && !unrestricted_write_only_subloan
        {
            return false;
        }
        // The shared loan is unrestricted; the original affine owner remains
        // live and retains its independent transfer or cleanup obligation.
        let shared_affine_loan = argument.path.is_empty()
            && argument.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.access == terminal_psi::StructuralAccess::Owned
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            && (caller
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == argument.place)
                || caller
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .any(|node| {
                        matches!(&node.operation,
                    O::CallStructural { result, .. } | O::EstablishRecord { result, .. }
                        if result.place == argument.place)
                    }));
        let actual_multiplicity =
            if shared_affine_loan || record_loan(caller, argument, parameter, types) {
                terminal_psi::StructuralMultiplicity::Unrestricted
            } else if argument.path.is_empty() {
                source.multiplicity
            } else if unrestricted_write_only_subloan
                || unrestricted_mutable_field
                || unrestricted_shared_field
            {
                terminal_psi::StructuralMultiplicity::Unrestricted
            } else if parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && is_partial_affine_path(types, source.structural_type, &argument.path)
            {
                terminal_psi::StructuralMultiplicity::Affine
            } else {
                terminal_psi::StructuralMultiplicity::Linear
            };
        if actual_multiplicity != parameter.multiplicity
            || parameter
                .qualifications
                .iter()
                .any(|qualification| !source.carries_qualification(&argument.path, *qualification))
            || parameter
                .projected_qualifications
                .iter()
                .any(|qualification| {
                    let mut path = argument.path.clone();
                    path.extend(qualification.path.iter().cloned());
                    !source.carries_qualification(&path, qualification.domain)
                })
            || (projection == StructuralProjectionPolicy::Unit
                && !argument.path.is_empty()
                && (!source.qualifications.is_empty()
                    || ((unrestricted_write_only_subloan || unrestricted_mutable_field)
                        && !parameter.qualifications.is_empty())))
        {
            return false;
        }
    }
    for first in 0..arguments.len() {
        for second in first + 1..arguments.len() {
            let left = &arguments[first];
            let right = &arguments[second];
            if left.place == right.place
                && structural_paths_may_overlap(&left.path, &right.path)
                && (structural_access_is_exclusive(left.access)
                    || structural_access_is_exclusive(right.access)
                    // Moving an affine owner is exclusive even when the other
                    // argument only borrows a child. Block arrivals retain that
                    // obligation just like parameters and operation results.
                    || ((left.access == terminal_psi::StructuralAccess::Owned
                        || right.access == terminal_psi::StructuralAccess::Owned)
                        && structural_source_contract(caller, left.place, allow_byte_literal)
                            .is_some_and(|source| source.access == terminal_psi::StructuralAccess::Owned
                                && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine)))
            {
                return false;
            }
        }
    }
    true
}

/// Indexed non-observing projection cannot traverse stored references or
/// descriptors. The validated catalog is acyclic; shared children are inspected
/// once without imposing a private depth limit.
fn is_material_write_only_type(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = types.get(&current) else {
            return false;
        };
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if field.relevance.is_erased() {
                        return false;
                    }
                    match field.field_type {
                        StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_) => {}
                        StructuralFieldType::Structural(child) => pending.push(child),
                        StructuralFieldType::ByteSequence(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::Erased { .. } => return false,
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => return false,
        }
    }
    true
}

pub(crate) fn structural_source_contract(
    caller: &PsiOptimizationFunction,
    place: PlaceId,
    allow_byte_literal: bool,
) -> Option<StructuralSourceContract<'_>> {
    caller
        .structural_parameters
        .iter()
        .chain(
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place)
        .map(|parameter| StructuralSourceContract {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &parameter.qualifications,
            projected_qualifications: &parameter.projected_qualifications,
        })
        .or_else(|| structural_operation_result_contract(caller, place))
        .or_else(|| {
            allow_byte_literal.then_some(())?;
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find_map(|node| {
                    let O::EstablishByteSequenceLiteral {
                        place: declaration,
                        structural_type,
                        ..
                    } = &node.operation
                    else {
                        return None;
                    };
                    (declaration.id == place).then_some(StructuralSourceContract {
                        structural_type: structural_type.id,
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        qualifications: &[],
                        projected_qualifications: &[],
                    })
                })
        })
}

fn structural_operation_result_contract(
    caller: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralSourceContract<'_>> {
    caller
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| {
            let (result, access) = match &node.operation {
                // Activation-local storage supplies loans, never owned escape.
                O::EstablishPrimitiveLocal { result, .. } => {
                    (result, terminal_psi::StructuralAccess::MutableBorrow)
                }
                O::ByteSequenceSubslice { result, .. } => {
                    (result, terminal_psi::StructuralAccess::SharedBorrow)
                }
                O::EstablishScalarArray { result, .. }
                | O::EstablishScalarCase { result, .. }
                | O::EstablishRecord { result, .. }
                | O::CallStructural { result, .. }
                | O::BoundaryCall {
                    result: abstract_operations::AbstractBoundaryResult::Structural(result),
                    ..
                } => (result, terminal_psi::StructuralAccess::Owned),
                _ => return None,
            };
            (result.place == place).then_some(StructuralSourceContract {
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access,
                qualifications: &result.qualifications,
                projected_qualifications: &result.projected_qualifications,
            })
        })
}

pub(crate) fn structural_access_can_supply(
    source: terminal_psi::StructuralAccess,
    presented: terminal_psi::StructuralAccess,
) -> bool {
    match source {
        terminal_psi::StructuralAccess::Owned => true,
        terminal_psi::StructuralAccess::SharedBorrow => {
            presented == terminal_psi::StructuralAccess::SharedBorrow
        }
        terminal_psi::StructuralAccess::MutableBorrow => matches!(
            presented,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ),
        terminal_psi::StructuralAccess::WriteOnlyBorrow => {
            presented == terminal_psi::StructuralAccess::WriteOnlyBorrow
        }
    }
}

pub(crate) fn structural_access_is_exclusive(access: terminal_psi::StructuralAccess) -> bool {
    matches!(
        access,
        terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    )
}

pub(crate) fn structural_paths_may_overlap(
    left: &[terminal_psi::StructuralPathSegment],
    right: &[terminal_psi::StructuralPathSegment],
) -> bool {
    left.iter().zip(right).all(|(left, right)| left == right)
}

pub(crate) fn is_nonempty_field_path(path: &[terminal_psi::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
}

pub(crate) fn is_partial_affine_path(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> bool {
    !path.is_empty() && resolve_structural_path(types, root, path).is_some()
}
