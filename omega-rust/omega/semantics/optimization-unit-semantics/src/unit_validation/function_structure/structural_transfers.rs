//! Exact whole shared-view and plain owned telescopes; ownership replays separately.
use super::*;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeShape,
};

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch;
    let mut places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        if block.id == function.entry && !block.structural_parameters.is_empty() {
            return Err(invalid());
        }
        for (position, parameter) in block.structural_parameters.iter().enumerate() {
            if parameter.position as usize != position
                || !places.insert(parameter.place)
                || !plain(parameter)
                || !types
                    .get(&parameter.structural_type)
                    .is_some_and(|declaration| {
                        parameter.access == StructuralAccess::Owned
                            || matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    ByteSequenceCarrier::BorrowedView
                                )
                            )
                    })
                || !function.structural_places.iter().any(|place| {
                    place.id == parameter.place
                        && place.kind
                            == (StructuralPlaceKind::BlockParameter {
                                block: block.id,
                                position: parameter.position,
                            })
                })
                || function
                    .entry_claim_declarations
                    .iter()
                    .any(|claim| claim.input == parameter.place)
                || function
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
            {
                return Err(invalid());
            }
        }
    }
    for block in &function.blocks {
        for edge in block.nodes.iter().flat_map(|node| &node.successors) {
            let destination = function
                .blocks
                .iter()
                .find(|block| block.id == edge.target)
                .ok_or(OptimizationUnitValidationError::UnknownSuccessor {
                    machine: function.machine,
                    block: block.id,
                    target: edge.target,
                })?;
            if edge.structural_bindings.len() != destination.structural_parameters.len() {
                return Err(invalid());
            }
            for (binding, parameter) in edge
                .structural_bindings
                .iter()
                .zip(&destination.structural_parameters)
            {
                if binding.parameter != parameter.place
                    || !binding.argument.path.is_empty()
                    || binding.argument.access != parameter.access
                    || source_contract(function, binding.argument.place)
                        != Some((
                            parameter.structural_type,
                            parameter.access,
                            parameter.multiplicity,
                        ))
                    || function
                        .entry_claim_declarations
                        .iter()
                        .any(|claim| claim.input == binding.argument.place)
                    || function
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == binding.argument.place)
                {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}

fn plain(parameter: &StructuralParameterDeclaration) -> bool {
    !parameter.is_self
        && match parameter.access {
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow => {
                parameter.multiplicity == StructuralMultiplicity::Unrestricted
            }
            StructuralAccess::Owned => matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            ),
            StructuralAccess::WriteOnlyBorrow => false,
        }
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
}

fn source_contract(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<(StructuralTypeId, StructuralAccess, StructuralMultiplicity)> {
    if let Some(parameter) = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place)
    {
        return plain(parameter).then_some((
            parameter.structural_type,
            parameter.access,
            parameter.multiplicity,
        ));
    }
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishByteSequenceLiteral {
                place: declaration, ..
            } if declaration.id == place => match declaration.kind {
                StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                } => Some((
                    structural_type,
                    StructuralAccess::SharedBorrow,
                    StructuralMultiplicity::Unrestricted,
                )),
                _ => None,
            },
            O::ByteSequenceSubslice { result, .. }
                if result.place == place
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty() =>
            {
                Some((
                    result.structural_type,
                    StructuralAccess::SharedBorrow,
                    StructuralMultiplicity::Unrestricted,
                ))
            }
            _ => None,
        })
}
