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
    multiplicity: terminal_psi::StructuralMultiplicity,
    access: terminal_psi::StructuralAccess,
    qualifications: &'a [StructuralDomainId],
    projected_qualifications: &'a [terminal_psi::StructuralPathQualification],
}

impl StructuralSourceContract<'_> {
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
        if !path_shape_matches
            || actual_type != parameter.structural_type
            || argument.access != parameter.access
            || !structural_access_can_supply(source.access, argument.access)
        {
            return false;
        }
        let unrestricted_write_only_field = is_nonempty_field_path(&argument.path)
            && argument.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        let unrestricted_shared_field = is_nonempty_field_path(&argument.path)
            && argument.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        let actual_multiplicity = if argument.path.is_empty() {
            source.multiplicity
        } else if unrestricted_write_only_field || unrestricted_shared_field {
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
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                        access: terminal_psi::StructuralAccess::Owned,
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
                access: terminal_psi::StructuralAccess::Owned,
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
