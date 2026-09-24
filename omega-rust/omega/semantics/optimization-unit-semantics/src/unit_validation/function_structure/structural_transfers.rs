//! Exact whole shared-view, shared address-join and plain owned telescopes;
//! ownership replays separately.
//!
//! A descriptor view or owned arrival binds a whole root of exactly its own
//! contract. An address join (`abstract_operations::control_flow::address_joins`)
//! instead borrows a readable, non-linear root or a static projection beneath
//! it: the argument keeps shared access and names exactly the parameter's
//! referent type, and the root keeps its own owner. The current-ownership
//! replay separately pins every such root while the joined view can observe it.
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
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
                            || address_join(parameter, types)
                            || matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    ByteSequenceCarrier::BorrowedView
                                ) | StructuralTypeShape::ElementView { .. }
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
                let source = source_contract(function, binding.argument.place);
                let exact_source = if address_join(parameter, types) {
                    // The root lends its address; it neither changes owner nor
                    // widens past shared access, and the path names the join's
                    // exact referent type.
                    abstract_operations::control_flow::address_joins::is_static_projection(
                        &binding.argument.path,
                    ) && source.is_some_and(|(root_type, access, multiplicity)| {
                        access != StructuralAccess::WriteOnlyBorrow
                            && multiplicity != StructuralMultiplicity::Linear
                            && abstract_operations::control_flow::address_joins::referent_type(
                                types.values().copied(),
                                root_type,
                                &binding.argument.path,
                            ) == Some(parameter.structural_type)
                    })
                } else {
                    binding.argument.path.is_empty()
                        && source
                            == Some((
                                parameter.structural_type,
                                parameter.access,
                                parameter.multiplicity,
                            ))
                };
                if binding.parameter != parameter.place
                    || binding.argument.access != parameter.access
                    || !exact_source
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

fn address_join(
    parameter: &StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    abstract_operations::control_flow::address_joins::is_address_join(parameter, |identity| {
        types.get(&identity).copied()
    })
}

/// Block parameters carry no qualification roster, so a structural join merges
/// nothing a boundary requirement could consume: the joined value's roster is
/// the empty intersection, and a qualified operand must reach its consumer
/// through the dominating source contract rather than through the join.
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
            O::CallStructural { result, .. }
            | O::EstablishScalarCase { result, .. }
            | O::EstablishRecord { result, .. }
                if result.place == place
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty() =>
            {
                // Producer identity, dominance and successful-completion ownership
                // are independently replayed by operation and frontier validation.
                Some((
                    result.structural_type,
                    StructuralAccess::Owned,
                    result.multiplicity,
                ))
            }
            O::ByteSequenceSubslice { result, .. }
            | O::EstablishElementView { result, .. }
            | O::ElementViewSubslice { result, .. }
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
