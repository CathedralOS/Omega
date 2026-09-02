use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralProjectionPolicy {
    Unit,
    EmptyOnly,
    Projected,
    Boundary,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StructuralSourceContract<'a> {
    structural_type: StructuralTypeId,
    multiplicity: psi_terminal::StructuralMultiplicity,
    access: psi_terminal::StructuralAccess,
    qualifications: &'a [StructuralDomainId],
    projected_qualifications: &'a [psi_terminal::StructuralPathQualification],
}

impl StructuralSourceContract<'_> {
    pub(crate) fn carries_qualification(
        &self,
        path: &[psi_terminal::StructuralPathSegment],
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
    arguments: &[psi_terminal::StructuralArgument],
    parameters: &[psi_terminal::StructuralParameterDeclaration],
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
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
                    || matches!(
                        argument.path.as_slice(),
                        [psi_terminal::StructuralPathSegment::FixedIndex(_)]
                            | [
                                psi_terminal::StructuralPathSegment::FixedIndex(_),
                                psi_terminal::StructuralPathSegment::FixedIndex(_),
                            ]
                    )
                    || is_nonempty_field_path(&argument.path)
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
        if !path_shape_matches
            || actual_type != parameter.structural_type
            || argument.access != parameter.access
            || !structural_access_can_supply(source.access, argument.access)
        {
            return false;
        }
        let unrestricted_write_only_field = is_nonempty_field_path(&argument.path)
            && argument.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && parameter.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && source.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && parameter.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
            && source.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted;
        let unrestricted_shared_field = is_nonempty_field_path(&argument.path)
            && argument.access == psi_terminal::StructuralAccess::SharedBorrow
            && parameter.access == psi_terminal::StructuralAccess::SharedBorrow
            && parameter.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
            && source.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted;
        let actual_multiplicity = if argument.path.is_empty() {
            source.multiplicity
        } else if unrestricted_write_only_field || unrestricted_shared_field {
            psi_terminal::StructuralMultiplicity::Unrestricted
        } else if parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
            && source.multiplicity == psi_terminal::StructuralMultiplicity::Affine
            && is_bounded_partial_affine_path(types, source.structural_type, &argument.path)
        {
            psi_terminal::StructuralMultiplicity::Affine
        } else {
            psi_terminal::StructuralMultiplicity::Linear
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
                && !source.qualifications.is_empty())
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
                    || structural_access_is_exclusive(right.access))
            {
                return false;
            }
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
                        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
                        access: psi_terminal::StructuralAccess::Owned,
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
            let result = match &node.operation {
                O::EstablishPayloadlessCase { result, .. }
                | O::EstablishAffineScalarRecord { result, .. }
                | O::CallStructural { result, .. } => result,
                _ => return None,
            };
            (result.place == place).then_some(StructuralSourceContract {
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access: psi_terminal::StructuralAccess::Owned,
                qualifications: &result.qualifications,
                projected_qualifications: &result.projected_qualifications,
            })
        })
}

pub(crate) fn structural_access_can_supply(
    source: psi_terminal::StructuralAccess,
    presented: psi_terminal::StructuralAccess,
) -> bool {
    match source {
        psi_terminal::StructuralAccess::Owned => true,
        psi_terminal::StructuralAccess::SharedBorrow => {
            presented == psi_terminal::StructuralAccess::SharedBorrow
        }
        psi_terminal::StructuralAccess::MutableBorrow => matches!(
            presented,
            psi_terminal::StructuralAccess::SharedBorrow
                | psi_terminal::StructuralAccess::MutableBorrow
                | psi_terminal::StructuralAccess::WriteOnlyBorrow
        ),
        psi_terminal::StructuralAccess::WriteOnlyBorrow => {
            presented == psi_terminal::StructuralAccess::WriteOnlyBorrow
        }
    }
}

pub(crate) fn structural_access_is_exclusive(access: psi_terminal::StructuralAccess) -> bool {
    matches!(
        access,
        psi_terminal::StructuralAccess::MutableBorrow
            | psi_terminal::StructuralAccess::WriteOnlyBorrow
    )
}

pub(crate) fn structural_paths_may_overlap(
    left: &[psi_terminal::StructuralPathSegment],
    right: &[psi_terminal::StructuralPathSegment],
) -> bool {
    left.iter().zip(right).all(|(left, right)| left == right)
}

pub(crate) fn is_nonempty_field_path(path: &[psi_terminal::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, psi_terminal::StructuralPathSegment::Field(_)))
}

pub(crate) fn is_bounded_partial_affine_path(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[psi_terminal::StructuralPathSegment],
) -> bool {
    is_nonempty_field_path(path)
        || (matches!(path, [psi_terminal::StructuralPathSegment::FixedIndex(_)])
            && types.get(&root).is_some_and(|declaration| {
                matches!(
                    (&declaration.shape, path),
                    (
                        psi_terminal::StructuralTypeShape::FixedArray { length: 2, .. },
                        [psi_terminal::StructuralPathSegment::FixedIndex(0 | 1)]
                    ) | (
                        psi_terminal::StructuralTypeShape::FixedArray { length: 3, .. },
                        [psi_terminal::StructuralPathSegment::FixedIndex(0 | 1 | 2)]
                    ) | (
                        psi_terminal::StructuralTypeShape::FixedArray { length: 4, .. },
                        [psi_terminal::StructuralPathSegment::FixedIndex(
                            0 | 1 | 2 | 3
                        )]
                    )
                )
            }))
        || (matches!(path, [psi_terminal::StructuralPathSegment::FixedIndex(_), psi_terminal::StructuralPathSegment::FixedIndex(_)])
            && types.get(&root).is_some_and(|declaration| {
                let psi_terminal::StructuralTypeShape::FixedArray { length: 2, element } = declaration.shape else {
                    return false;
                };
                let Some(inner) = types.get(&element) else {
                    return false;
                };
                let psi_terminal::StructuralTypeShape::FixedArray { length: inner_length @ (3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13), .. } = inner.shape else {
                    return false;
                };
                matches!(path, [psi_terminal::StructuralPathSegment::FixedIndex(outer), psi_terminal::StructuralPathSegment::FixedIndex(index)] if *outer < 2 && *index < inner_length)
            }))
}
