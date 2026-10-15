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
//! An owned projection instead carries exactly its typed subtree into an affine
//! or linear arrival. Geometry supplies no claim: current-ownership replay must
//! relocate the selected claims before cleanup and leave sibling custody intact.
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeShape,
};

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
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
                    && !qualified_owned_arrival(parameter, types, domains)
                    && !address_join(parameter, types)
                    && !(parameter.qualifications.is_empty()
                        && terminal_semantics::view_descriptor_parameter(
                            parameter,
                            types.values().copied(),
                        ))
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
        for edge in block.successors() {
            let destination = function.block(edge.target).ok_or(
                OptimizationUnitValidationError::UnknownSuccessor {
                    machine: function.machine,
                    block: block.id,
                    target: edge.target,
                },
            )?;
            if edge.structural_bindings.len() != destination.structural_parameters.len() {
                return Err(invalid());
            }
            for (binding, parameter) in edge
                .structural_bindings
                .iter()
                .zip(&destination.structural_parameters)
            {
                let source = source_contract(function, binding.argument.place, types);
                let exact_source = if qualified_owned_arrival(parameter, types, domains) {
                    // An owned arrival delivers the exact whole value or
                    // selected subtree, including its source-carried domain
                    // roster. Geometry is not issuance: operation validation
                    // checks the producer and ownership replay transfers the
                    // live occurrence before selected-edge cleanup.
                    super::super::operation_contracts::structural_source_contract(
                        function, binding.argument.place,
                    ).is_some_and(|source| {
                        source.access == StructuralAccess::Owned
                            && binding.argument.access == StructuralAccess::Owned
                            && if binding.argument.path.is_empty() {
                                source.structural_type == parameter.structural_type
                                    && source.multiplicity == parameter.multiplicity
                            } else {
                                source.multiplicity == StructuralMultiplicity::Affine
                                    && matches!(parameter.multiplicity,
                                        StructuralMultiplicity::Affine | StructuralMultiplicity::Linear)
                                    && crate::unit_validation::structural_catalog::resolve_structural_path(
                                        types, source.structural_type, &binding.argument.path,
                                    ) == Some(parameter.structural_type)
                            }
                            && source.supplies_qualifications(
                                &binding.argument.path,
                                &parameter.qualifications,
                                &parameter.projected_qualifications,
                            )
                    })
                } else if qualified_shared_view(parameter, types) {
                    qualified_shared_view_source(function, &binding.argument, parameter)
                } else if owner_loan(parameter, types) || qualified_owned_bytes(parameter, types) {
                    // A bounded owner or its loan is forwarded whole
                    // from an identically declared signature or block
                    // parameter. Owned transfer moves the value and roster;
                    // the loan instead retains its exact access. Only shared
                    // access freezes contents; mutable arrival changes no
                    // declared predicate or currentness requirement. The same
                    // evidence arrives projected: the argument names one
                    // declared byte leaf whose carrier is the parameter's and
                    // whose field predicates imply the parameter's roster, or
                    // copies that leaf whole — the producer's recorded path
                    // carries the qualification's provenance.
                    let forwarded = binding.argument.path.is_empty()
                        && function
                            .structural_parameters
                            .iter()
                            .chain(function.block_structural_parameters())
                            .any(|source| {
                                source.place == binding.argument.place
                                    && source.structural_type == parameter.structural_type
                                    && source.access == parameter.access
                                    && source.multiplicity == parameter.multiplicity
                                    && source.qualifications == parameter.qualifications
                                    && source.projected_qualifications.is_empty()
                            });
                    let copied = binding.argument.path.is_empty()
                        && binding.argument.access == StructuralAccess::Owned
                        && function.block_nodes().any(|node| match &node.operation {
                            O::StructuralLeafCopy {
                                source: backing,
                                path: abstract_operations::StructuralCopyPath::Ordinary(path),
                                result,
                                ..
                            } => {
                                result.place == binding.argument.place
                                    && result.structural_type == parameter.structural_type
                                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                                    && result.qualifications.is_empty()
                                    && result.projected_qualifications.is_empty()
                                    && result.claims.is_empty()
                                    && function.arrival_parameter(*backing).is_some_and(|root| {
                                        byte_leaf_predicates(
                                            types,
                                            domains,
                                            root.structural_type,
                                            path,
                                            parameter,
                                        )
                                    })
                            }
                            _ => false,
                        });
                    let projected = !binding.argument.path.is_empty()
                        && function
                            .arrival_parameter(binding.argument.place)
                            .is_some_and(|root| {
                                byte_leaf_predicates(
                                    types,
                                    domains,
                                    root.structural_type,
                                    &binding.argument.path,
                                    parameter,
                                )
                            });
                    // A state arrival lends completed storage under the same
                    // contract as a call argument. Current predicates, exact
                    // access and exclusive restoration are not properties of
                    // the syntax that introduced the receiving parameter.
                    let owned_result =
                        super::super::operation_contracts::structural_source_contract(
                            function,
                            binding.argument.place,
                        )
                        .is_some_and(|source| {
                            super::super::operation_contracts::owned_bounded_owner_loan(
                                function,
                                &binding.argument,
                                parameter,
                                &source,
                                types,
                                domains,
                            )
                        });
                    forwarded || copied || projected || owned_result
                } else if address_join(parameter, types) {
                    // The root lends its address; it neither changes owner nor
                    // widens past shared access, and the path names the join's
                    // exact referent type. A mutable join may lend the same
                    // carrier over a static projection: the subtree transfers
                    // the exact incoming mutable permission while the rest of
                    // the root stays with the caller.
                    abstract_operations::control_flow::address_joins::is_address_projection(
                        &binding.argument.path,
                    ) && super::super::operation_contracts::structural_source_contract(
                        function, binding.argument.place,
                    ).is_some_and(|source| {
                        let access = source.backing_access(types);
                        (if matches!(
                            parameter.access,terminal_psi::exclusive_borrow_pair_pattern!()) {
                            super::super::operation_contracts::structural_access_can_supply(
                                access,
                                parameter.access,
                            ) && access != StructuralAccess::Owned
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
                        }) && source.multiplicity != StructuralMultiplicity::Linear
                            && source.borrowed_qualifications_match(&binding.argument.path, parameter)
                            && abstract_operations::control_flow::address_joins::borrowed_referent_type(
                                types.values().copied(),
                                source.structural_type,
                                &binding.argument.path,
                                parameter.access,
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
                } else if terminal_semantics::view_descriptor_parameter(
                    parameter,
                    types.values().copied(),
                ) && parameter.qualifications.is_empty()
                    && !binding.argument.path.is_empty()
                {
                    source.is_some_and(|(root_type, access, multiplicity)| {
                        multiplicity != StructuralMultiplicity::Linear
                            && super::super::operation_contracts::structural_access_can_supply(access, parameter.access)
                            && abstract_operations::control_flow::address_joins::borrowed_referent_type(
                                types.values().copied(), root_type, &binding.argument.path, parameter.access,
                            ) == Some(parameter.structural_type)
                    })
                } else if !binding.argument.path.is_empty() {
                    // Geometry names the exact moved child; current ownership
                    // independently transfers its complete claim frontier and
                    // retains the source's siblings until edge cleanup.
                    parameter.access == StructuralAccess::Owned
                        && source.is_some_and(|(root_type, access, multiplicity)| {
                            access == StructuralAccess::Owned
                                && (multiplicity == StructuralMultiplicity::Affine
                                    && matches!(parameter.multiplicity, StructuralMultiplicity::Affine | StructuralMultiplicity::Linear)
                                    || multiplicity == StructuralMultiplicity::Unrestricted
                                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted)
                                &&
                            crate::unit_validation::structural_catalog::resolve_structural_path(
                                types,
                                root_type,
                                &binding.argument.path,
                            ) == Some(parameter.structural_type)
                                && super::super::operation_contracts::structural_source_contract(
                                    function, binding.argument.place,
                                ).is_some_and(|source| {
                                    let (whole, projected) = source.selected_qualifications(&binding.argument.path);
                                    parameter.qualifications.iter().all(|domain| whole.contains(domain))
                                        && parameter.projected_qualifications.iter().all(|row| projected.contains(row))
                                })
                        })
                } else {
                    source.is_some_and(|(source_type, access, multiplicity)| {
                            // A borrowed arrival carries permission over the
                                // source, not ownership of its descriptor. Its
                                // declaration does not erase the affine result's
                                // separate custody and cleanup obligations.
                                match parameter.access {
                                    StructuralAccess::Owned => source_type == parameter.structural_type
                                        && access == StructuralAccess::Owned
                                        && multiplicity == parameter.multiplicity,StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => {
                                        let presentation = terminal_semantics::borrowed_referent_presentation(
                                            types.values().copied(), source_type, parameter.structural_type);
                                        let lends_backing = matches!(presentation,
                                            Some(terminal_semantics::BorrowedReferentPresentation::AddressOfBacking { .. }));
                                        presentation.is_some()
                                            && (access == StructuralAccess::Owned && lends_backing
                                                || access != StructuralAccess::Owned
                                                    && super::super::operation_contracts::structural_access_can_supply(access, parameter.access))
                                            && multiplicity != StructuralMultiplicity::Linear
                                    },
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
                    || (claim_hit && parameter.access != StructuralAccess::Owned)
                {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}

/// Nominal aggregate arrivals keep source-carried qualifications. Byte,
/// element-view, reference and binding carriers retain their distinct
/// content, backing and permission contracts. Catalog validity and required
/// membership correspondence grant no new membership or issuer authority;
/// live custody remains the separate current-ownership transaction.
fn qualified_owned_arrival(
    parameter: &StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    parameter.access == StructuralAccess::Owned
        && !qualified_owned_bytes(parameter, types)
        && types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::Record { .. }
                        | StructuralTypeShape::Sum { .. }
                        | StructuralTypeShape::Mixed { .. }
                        | StructuralTypeShape::FixedArray { .. }
                )
            })
        && (!parameter.qualifications.is_empty() || !parameter.projected_qualifications.is_empty())
        && crate::unit_validation::structural_catalog::structural_qualifications_match(
            parameter.structural_type,
            &parameter.qualifications,
            domains,
        )
        && crate::unit_validation::structural_catalog::structural_projected_qualifications_match(
            parameter.structural_type,
            &parameter.projected_qualifications,
            types,
            domains,
        )
}

/// The declared byte evidence a projected path ends at: its last segment
/// names a field of the resolved parent whose byte carrier is the expected
/// parameter's carrier and whose declared predicates imply the parameter's
/// roster — the same storage facts a forwarded declaration states inline.
fn byte_leaf_predicates(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
    root_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    parameter: &StructuralParameterDeclaration,
) -> bool {
    let Some((terminal_psi::StructuralPathSegment::Field(identity), prefix)) = path.split_last()
    else {
        return false;
    };
    let Some(field) =
        terminal_semantics::runtime_structural_path_tip(types.values().copied(), root_type, prefix)
            .and_then(|parent| match &parent.shape {
                terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                    terminal_psi::live_field_by_identity(fields, *identity)
                }
                _ => None,
            })
    else {
        return false;
    };
    field
        .field_type
        .byte_sequence_carrier()
        .is_some_and(|carrier| {
            types
                .get(&parameter.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(declared) if declared == carrier
                    )
                })
        })
        && terminal_semantics::byte_qualification_predicates_in(
            domains.values().copied(),
            parameter.structural_type,
            &parameter.qualifications,
        )
        .is_some_and(|required| field.field_type.byte_predicates().implies(required))
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
        .chain(function.block_structural_parameters())
        .any(|source| {
            source.place == argument.place
                && source.structural_type == parameter.structural_type
                && source.access == parameter.access
                && source.multiplicity == parameter.multiplicity
                && source.qualifications == parameter.qualifications
                && source.projected_qualifications.is_empty()
        });
    let literal = || {
        function.block_nodes().any(|node| match &node.operation {
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
    (match parameter.access {
        StructuralAccess::SharedBorrow
        | StructuralAccess::MutableBorrow
        | StructuralAccess::WriteOnlyBorrow => {
            parameter.multiplicity == StructuralMultiplicity::Unrestricted
        }
        StructuralAccess::Owned => matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted
                | StructuralMultiplicity::Affine
                | StructuralMultiplicity::Linear
        ),
    }) && parameter.qualifications.is_empty()
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
        .chain(function.block_structural_parameters())
        .find(|parameter| parameter.place == place)
    {
        return (plain(parameter)
            || parameter.qualifications.is_empty()
                && terminal_semantics::view_descriptor_parameter(
                    parameter,
                    types.values().copied(),
                ))
        .then_some((
            parameter.structural_type,
            parameter.access,
            parameter.multiplicity,
        ));
    }
    function
        .block_nodes()
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
            operation => {
                let source =
                    super::super::operation_contracts::structural_operation_result_contract(
                        function, operation,
                    )?;
                let (_, result) = operation.structural_result()?;
                if result.place != place
                    || (source.access != StructuralAccess::Owned
                        && (source.multiplicity == StructuralMultiplicity::Linear
                            || !result.claims.is_empty()))
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                {
                    return None;
                }
                // Every completed producer supplies its declared storage contract.
                // Activation-local scalar homes lend access without owned escape;
                // dominance and live backing remain independently replayed.
                // A returned/copied view presents its descriptor's loan,
                // rather than lending owned payload storage. View establishments
                // already report their exact source-derived access.
                let access = if source.access == StructuralAccess::Owned
                    && types
                        .get(&source.structural_type)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                StructuralTypeShape::ElementView { .. }
                                    | StructuralTypeShape::ByteSequence(
                                        ByteSequenceCarrier::BorrowedView { .. },
                                    )
                            )
                        }) {
                    match source.multiplicity {
                        StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
                        StructuralMultiplicity::Affine => StructuralAccess::MutableBorrow,
                        StructuralMultiplicity::Linear => return None,
                    }
                } else {
                    source.access
                };
                Some((source.structural_type, access, source.multiplicity))
            }
        })
}
