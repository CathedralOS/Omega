//! Exact whole shared-view (qualified or not), bounded-owner loan, qualified
//! owned bounded-byte, address-join and plain owned telescopes; ownership
//! replays separately.
//!
//! A descriptor view or owned arrival binds a whole root of exactly its own
//! contract. An address join (`abstract_operations::control_flow::address_joins`)
//! instead borrows a readable, non-linear root or a static projection beneath
//! it: the argument keeps its edge access and names exactly the parameter's
//! referent type, and the root keeps its own owner. The current-ownership
//! replay separately pins every such root while the joined view can observe it.
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
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
                || (!plain(parameter)
                    && !owner_loan(parameter, types)
                    && !qualified_owned_bytes(parameter, types)
                    && !qualified_shared_view(parameter, types)
                    && !shared_view_carrier(parameter, types))
                || !types
                    .get(&parameter.structural_type)
                    .is_some_and(|declaration| {
                        parameter.access == StructuralAccess::Owned
                            || address_join(parameter, types)
                            || shared_view_carrier(parameter, types)
                            || declaration.shape.is_borrowed_byte_carrier()
                            || matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
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
                let source = source_contract(function, binding.argument.place, types);
                let exact_source = if qualified_shared_view(parameter, types) {
                    qualified_shared_view_source(function, &binding.argument, parameter)
                } else if owner_loan(parameter, types) || qualified_owned_bytes(parameter, types) {
                    // A bounded owner or its loan is forwarded whole
                    // from an identically declared signature or block
                    // parameter. Owned transfer moves the value and roster;
                    // the loan instead retains its exact access. Only shared
                    // access freezes contents; mutable arrival changes no
                    // declared predicate or currentness requirement.
                    binding.argument.path.is_empty()
                        && function
                            .structural_parameters
                            .iter()
                            .chain(
                                function
                                    .blocks
                                    .iter()
                                    .flat_map(|block| &block.structural_parameters),
                            )
                            .any(|source| {
                                source.place == binding.argument.place
                                    && source.structural_type == parameter.structural_type
                                    && source.access == parameter.access
                                    && source.multiplicity == parameter.multiplicity
                                    && source.qualifications == parameter.qualifications
                                    && source.projected_qualifications.is_empty()
                                    && !source.is_self
                            })
                } else if address_join(parameter, types) {
                    // The root lends its address; it neither changes owner nor
                    // widens past shared access, and the path names the join's
                    // exact referent type. A mutable join may lend the same
                    // carrier over a static projection: the subtree transfers
                    // the exact incoming mutable permission while the rest of
                    // the root stays with the caller.
                    abstract_operations::control_flow::address_joins::is_static_projection(
                        &binding.argument.path,
                    ) && source.is_some_and(|(root_type, access, multiplicity)| {
                        (if parameter.access == StructuralAccess::MutableBorrow {
                            access == StructuralAccess::MutableBorrow
                                || access == StructuralAccess::Owned
                                    && function.structural_places.iter().any(|place| {
                                        place.id == binding.argument.place
                                            && matches!(
                                                place.kind,
                                                StructuralPlaceKind::OperationResult { .. }
                                            )
                                    })
                        } else {
                            access != StructuralAccess::WriteOnlyBorrow
                        }) && multiplicity != StructuralMultiplicity::Linear
                            && abstract_operations::control_flow::address_joins::referent_type(
                                types.values().copied(),
                                root_type,
                                &binding.argument.path,
                            ) == Some(parameter.structural_type)
                    })
                } else if shared_view_carrier(parameter, types) {
                    // A shared loan over a view-carrying record observes the
                    // carrier without moving it: the bound root stays with its
                    // owner (a function value or a locally established record)
                    // while the parameter reads the stored descriptors. An
                    // address join never reaches here — it already bound its
                    // member path; a carrier binds only a whole-record root.
                    binding.argument.path.is_empty()
                        && source.is_some_and(|(root_type, _, multiplicity)| {
                            root_type == parameter.structural_type
                                && multiplicity != StructuralMultiplicity::Linear
                        })
                } else {
                    binding.argument.path.is_empty()
                        && source.is_some_and(|(source_type, access, multiplicity)| {
                            // A borrowed arrival carries permission over the
                                // source, not ownership of its descriptor. Its
                                // declaration does not erase the affine result's
                                // separate custody and cleanup obligations.
                                match parameter.access {
                                    StructuralAccess::Owned => source_type == parameter.structural_type
                                        && access == StructuralAccess::Owned
                                        && multiplicity == parameter.multiplicity,
                                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow => {
                                        let presentation = terminal_semantics::borrowed_referent_presentation(
                                            types.values().copied(), source_type, parameter.structural_type);
                                        let lends_backing = matches!(presentation,
                                            Some(terminal_semantics::BorrowedReferentPresentation::AddressOfBacking { .. }));
                                        presentation.is_some()
                                            && (access == StructuralAccess::Owned && lends_backing
                                                || matches!(access, StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)
                                                    && super::super::operation_contracts::structural_access_can_supply(access, parameter.access))
                                            && multiplicity != StructuralMultiplicity::Linear
                                    },
                                    StructuralAccess::WriteOnlyBorrow => false,
                                }
                        })
                };
                let claim_hit = function
                    .entry_claim_declarations
                    .iter()
                    .any(|claim| claim.input == binding.argument.place)
                    || function
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == binding.argument.place);
                if binding.parameter != parameter.place
                    || binding.argument.access != parameter.access
                    || !exact_source
                    || claim_hit
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

fn owner_loan(
    parameter: &StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    abstract_operations::control_flow::owner_loans::is_bounded_owner_loan(parameter, |identity| {
        types.get(&identity).copied()
    })
}

/// A shared borrow of a record whose runtime members are plain storage or
/// shared-view descriptors — the loan observes the carrier's stored views.
fn shared_view_carrier(
    parameter: &StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    abstract_operations::control_flow::owner_loans::is_shared_view_carrier_loan(
        parameter,
        |identity| types.get(&identity).copied(),
        &types.values().map(|row| (*row).clone()).collect::<Vec<_>>(),
    )
}

fn qualified_owned_bytes(
    parameter: &StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    !parameter.qualifications.is_empty()
        && abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter(
            parameter,
            |identity| types.get(&identity).copied(),
        )
}

/// A shared borrowed byte view keeping its declared domains (`&[u8] in Utf8`).
/// Terminal verification replayed those predicates at every arrival; the
/// frozen descriptor needs no other transport.
fn qualified_shared_view(
    parameter: &StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    !parameter.qualifications.is_empty()
        && abstract_operations::control_flow::owner_loans::is_shared_byte_view(
            parameter,
            |identity| types.get(&identity).copied(),
        )
}

/// A qualified shared byte view arrives whole: forwarded from an identically
/// declared signature or block parameter, or established from a literal whose
/// verified roster is exactly the parameter's qualifications.
fn qualified_shared_view_source(
    function: &PsiOptimizationFunction,
    argument: &terminal_psi::StructuralArgument,
    parameter: &StructuralParameterDeclaration,
) -> bool {
    if !argument.path.is_empty() {
        return false;
    }
    let forwarded = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .any(|source| {
            source.place == argument.place
                && source.structural_type == parameter.structural_type
                && source.access == parameter.access
                && source.multiplicity == parameter.multiplicity
                && source.qualifications == parameter.qualifications
                && source.projected_qualifications.is_empty()
                && !source.is_self
        });
    let literal = || {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| match &node.operation {
                O::EstablishByteSequenceLiteral {
                    place: declaration,
                    qualifications,
                    ..
                } => {
                    declaration.id == argument.place
                        && *qualifications == parameter.qualifications
                        && matches!(
                            declaration.kind,
                            StructuralPlaceKind::ByteSequenceLiteral { structural_type, .. }
                                if structural_type == parameter.structural_type
                        )
                }
                _ => false,
            })
    };
    forwarded || literal()
}

/// Plain block parameters carry no qualification roster. Qualified bounded
/// owners use the exact whole-value route above rather than this fallback.
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
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
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
        return (plain(parameter) || parameter.is_self).then_some((
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
            O::EstablishByteSequenceConcatenation { result, .. }
            | O::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            | O::EstablishCase { result, .. }
            | O::EstablishRecord { result, .. }
            | O::EstablishZeroedStructural { result, .. }
            | O::EstablishScalarArray { result, .. }
            | O::StructuralLeafCopy { result, .. }
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
                // Initialized arrays own backing just as zeroed aggregates do;
                // lending that address does not transfer or copy their payload.
                // A view-descriptor result (an `&[T]` member copy or a returned
                // shared view) rides on its loan's shared access instead:
                // relocating the descriptor never lends owned backing.
                let access = types
                    .get(&result.structural_type)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::ElementView { .. }
                                | StructuralTypeShape::ByteSequence(
                                    ByteSequenceCarrier::BorrowedView { .. },
                                )
                        )
                    })
                    .then_some(StructuralAccess::SharedBorrow)
                    .unwrap_or(StructuralAccess::Owned);
                Some((result.structural_type, access, result.multiplicity))
            }
            operation @ (O::ByteSequenceSubslice { result, .. }
            | O::EstablishElementView { result, .. }
            | O::ElementViewSubslice { result, .. })
                if result.place == place
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty() =>
            {
                // The same exact producer/source chain supplies a call's or
                // edge's view permission; an affine result alone grants none.
                let source =
                    super::super::operation_contracts::structural_operation_result_contract(
                        function, operation,
                    )?;
                Some((source.structural_type, source.access, source.multiplicity))
            }
            _ => None,
        })
}
