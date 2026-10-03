//! Exact block-local parameter declarations and simultaneous edge bindings.
//!
//! Exclusive view names move across edges regardless of whether a signature,
//! a previous arrival, or a checked view operation established them. Rejoin
//! the producer's access lineage before admitting the transfer, and intersect
//! availability over every incoming edge. Dominance alone cannot resurrect
//! the original name after a successor took its exclusive permission.
//! Write-only loans participate in that same exclusive-name fixed point;
//! forwarding them never supplies readable authority. Operation validation
//! independently checks which accesses each current name permits.

use super::{
    BTreeMap, BTreeSet, BlockId, EdgeId, ModuleError, OperationKind, PlaceId, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
    Terminator,
};
pub(super) mod mutable_records;

fn exclusive_access(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    )
}

pub(super) fn parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
        return None;
    };
    machine
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .structural_parameters
        .get(position as usize)
        .filter(|parameter| {
            parameter.place == place && parameter.position == position && !parameter.is_self
        })
}

pub(super) fn validate_declarations(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), ModuleError> {
    for block in &machine.blocks {
        if block.id == machine.entry && !block.structural_parameters.is_empty() {
            return Err(ModuleError::EntryBlockCannotHaveParameters(block.id));
        }
        for (position, declaration) in block.structural_parameters.iter().enumerate() {
            if u32::try_from(position).ok() != Some(declaration.position)
                || parameter(machine, declaration.place) != Some(declaration)
                || !matches!(
                    (declaration.access, declaration.multiplicity),
                    (
                        StructuralAccess::SharedBorrow
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow,
                        StructuralMultiplicity::Unrestricted
                    ) | (
                        StructuralAccess::Owned,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                )
                || (!declaration.qualifications.is_empty()
                    && !bounded_owner_parameter(module, declaration)
                    && !qualified_shared_byte_view(module, declaration))
                || !declaration.projected_qualifications.is_empty()
                || !machine.structural_places.iter().any(|place| {
                    place.id == declaration.place
                        && place.kind
                            == StructuralPlaceKind::BlockParameter {
                                block: block.id,
                                position: declaration.position,
                            }
                })
                || !module.structural_types.iter().any(|row| {
                    row.id == declaration.structural_type
                        && (declaration.access == StructuralAccess::Owned
                            || matches!(
                                row.shape,
                                StructuralTypeShape::ByteSequence(carrier)
                                if carrier.is_borrowed()
                            )
                            || matches!(row.shape, StructuralTypeShape::ElementView { .. })
                            || bounded_owner_parameter(module, declaration)
                            || mutable_records::parameter_type(module, declaration)
                            || (declaration.access == StructuralAccess::SharedBorrow
                                && (matches!(row.shape, StructuralTypeShape::PrimitiveScalar(_))
                                    || super::record::plain_type(module, row.id)
                                    || super::record::shared_view_carrier_type(module, row.id)
                                    || mutable_records::borrowed_carrier_type(module, row.id))))
                })
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == declaration.place)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == declaration.place)
            {
                return Err(ModuleError::InvalidBlockStructuralParameter {
                    block: block.id,
                    place: declaration.place,
                });
            }
        }
    }
    for place in &machine.structural_places {
        if let StructuralPlaceKind::BlockParameter { block, .. } = place.kind
            && parameter(machine, place.id).is_none()
        {
            return Err(ModuleError::InvalidBlockStructuralParameter {
                block,
                place: place.id,
            });
        }
    }
    Ok(())
}

/// A shared loan of a bounded byte owner (`&[u8; N] in Utf8`) whose every
/// qualification is a plain byte predicate of that exact carrier. The loan
/// freezes its referent, so those predicates hold for as long as the block
/// holds the loan; the owner's live length is its own, never the capacity.
pub(super) fn shared_bounded_owner_loan(
    module: &TerminalModule,
    declaration: &StructuralParameterDeclaration,
) -> bool {
    declaration.access == StructuralAccess::SharedBorrow
        && bounded_owner_parameter(module, declaration)
}

/// A shared borrowed byte view (`&[u8] in Utf8`) whose every qualification is
/// a plain byte predicate of that exact carrier. The shared loan freezes its
/// referent, so the predicates hold for as long as the block holds the view.
/// Every arrival must carry the identical qualifications: a whole forwarded
/// parameter declared the same way, or a dominating literal establishment
/// whose octets literal validation already checked against exactly them.
pub(super) fn qualified_shared_byte_view(
    module: &TerminalModule,
    declaration: &StructuralParameterDeclaration,
) -> bool {
    declaration.access == StructuralAccess::SharedBorrow
        && declaration.multiplicity == StructuralMultiplicity::Unrestricted
        && !declaration.is_self
        && !declaration.qualifications.is_empty()
        && declaration.projected_qualifications.is_empty()
        && module.structural_types.iter().any(|row| {
            row.id == declaration.structural_type
                && matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(carrier) if carrier.is_borrowed()
                )
        })
        && terminal_semantics::byte_parameter_predicates(module, declaration).is_some()
}

/// The exact qualification roster of the one available literal establishment
/// that names `place`, with the literal's declared carrier.
fn established_literal_qualifications<'machine>(
    machine: &'machine TerminalMachine,
    place: PlaceId,
    available: &BTreeSet<PlaceId>,
) -> Option<(
    StructuralTypeId,
    &'machine [semantic_vocabulary::StructuralDomainId],
)> {
    if !available.contains(&place) {
        return None;
    }
    let structural_type =
        machine
            .structural_places
            .iter()
            .find_map(|declaration| match declaration.kind {
                StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                } if declaration.id == place => Some(structural_type),
                _ => None,
            })?;
    let mut establishments = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishByteSequenceLiteral {
                destination,
                qualifications,
                ..
            } if *destination == place
                && operation.result == terminal_psi::OperationResult::Unit =>
            {
                Some(qualifications.as_slice())
            }
            _ => None,
        });
    let qualifications = establishments.next()?;
    establishments
        .next()
        .is_none()
        .then_some((structural_type, qualifications))
}

/// Whole bounded owners and their loans carry only primitive byte invariants
/// of their exact carrier. An owned arrival moves the complete `{length,
/// bytes}` value; it does not acquire loan authority. Mutable transport moves
/// the usable loan name, while every permitted store independently preserves
/// the predicates before another edge can forward it.
fn bounded_owner_parameter(
    module: &TerminalModule,
    declaration: &StructuralParameterDeclaration,
) -> bool {
    matches!(
        declaration.access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && declaration.multiplicity == StructuralMultiplicity::Unrestricted
        && !declaration.is_self
        && declaration.projected_qualifications.is_empty()
        && module.structural_types.iter().any(|row| {
            row.id == declaration.structural_type
                && matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
        && terminal_semantics::byte_parameter_predicates(module, declaration).is_some()
}

/// Return permission follows the owned binding, not the syntax of its producer.
/// Control-flow validation must establish dominance and the frontier pass must
/// consume any affine obligation before using this claim-free classification.
pub(super) fn plain_owned_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    parameter(machine, source).is_some_and(|parameter| {
        parameter.access == StructuralAccess::Owned
            && matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            )
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty()
            && super::structural::result_contracts::has_plain_owned_shape(
                module,
                parameter.structural_type,
            )
    })
}

pub(super) fn validate_successor(
    module: &TerminalModule,
    machine: &TerminalMachine,
    edge: EdgeId,
    target: &terminal_psi::Block,
    arguments: &[StructuralArgument],
    available: &BTreeSet<PlaceId>,
    dominators: &crate::control_graph::DominatorTree,
    source_block: BlockId,
    allow_projected: bool,
) -> Result<(), ModuleError> {
    if arguments.len() != target.structural_parameters.len() {
        return Err(ModuleError::StructuralJumpArityMismatch {
            edge,
            expected: target.structural_parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut exclusive = BTreeSet::new();
    for (argument, expected) in arguments.iter().zip(&target.structural_parameters) {
        // A projected loan's machine-parameter root dominates every block, so
        // it needs no `available` row — the same exemption the projected owned
        // argument below records for its root. A whole-view argument transfers
        // the name itself: once an edge has moved it, `available` no longer
        // holds it, and a machine parameter gets no exemption to move it again.
        if exclusive_access(expected.access)
            && ((!available.contains(&argument.place)
                && (argument.path.is_empty()
                    || !machine
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == argument.place)))
                || !exclusive.insert(argument.place))
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
        }
        if bounded_owner_parameter(module, expected) {
            // A bounded owner or its loan is forwarded whole from a signature
            // or dominating block parameter with the identical declaration.
            // Owned transfer moves its complete value and qualification roster;
            // shared loans freeze their referent, while mutable stores preserve
            // its primitive predicates. Exclusive availability above still
            // consumes a mutable name instead of duplicating authority.
            let forwarded = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == argument.place)
                .or_else(|| {
                    parameter(machine, argument.place)
                        .filter(|_| available.contains(&argument.place))
                })
                .is_some_and(|source| {
                    source.structural_type == expected.structural_type
                        && source.access == expected.access
                        && source.multiplicity == StructuralMultiplicity::Unrestricted
                        && source.qualifications == expected.qualifications
                        && source.projected_qualifications.is_empty()
                });
            if !argument.path.is_empty() || argument.access != expected.access || !forwarded {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
            continue;
        }
        if qualified_shared_byte_view(module, expected) {
            // A qualified shared byte view arrives whole, either forwarded
            // from an identically declared signature or available block
            // parameter, or from a literal whose establishment carries
            // exactly the declared qualifications (checked against its octets
            // by literal validation). No other source proves the predicates.
            let forwarded = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == argument.place)
                .or_else(|| {
                    parameter(machine, argument.place)
                        .filter(|_| available.contains(&argument.place))
                })
                .is_some_and(|source| {
                    source.structural_type == expected.structural_type
                        && source.access == StructuralAccess::SharedBorrow
                        && source.multiplicity == StructuralMultiplicity::Unrestricted
                        && source.qualifications == expected.qualifications
                        && source.projected_qualifications.is_empty()
                });
            let literal = established_literal_qualifications(machine, argument.place, available)
                .is_some_and(|(structural_type, qualifications)| {
                    structural_type == expected.structural_type
                        && qualifications == expected.qualifications.as_slice()
                });
            if !argument.path.is_empty()
                || argument.access != StructuralAccess::SharedBorrow
                || !(forwarded || literal)
            {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
            continue;
        }
        if expected.access == StructuralAccess::SharedBorrow {
            // A shared successor loan joins the exact referent the authored
            // borrow observed: the argument names the same root and the same
            // projected field/fixed-index path, presented as `SharedBorrow`,
            // and the path resolves to the parameter's declared referent
            // type. No custody moves on this edge — the joined parameter can
            // only read — and the frontier walk separately proves the root is
            // still held when the edge completes. A borrowed byte view is a
            // whole-view loan; record and primitive-scalar joins carry the
            // projection that resolves to their declared referent type.
            let byte_view_parameter = module.structural_types.iter().any(|row| {
                row.id == expected.structural_type
                    && matches!(
                        row.shape,
                        StructuralTypeShape::ByteSequence(carrier)
                        if carrier.is_borrowed()
                    )
            });
            if expected.multiplicity != StructuralMultiplicity::Unrestricted
                || !expected.qualifications.is_empty()
                || !expected.projected_qualifications.is_empty()
                || argument.access != StructuralAccess::SharedBorrow
                || (byte_view_parameter && !argument.path.is_empty())
                || (!byte_view_parameter
                    && !argument.path.is_empty()
                    && !super::structural::operations::is_static_borrow_path(&argument.path))
                || shared_loan_root(module, machine, argument, available)
                    .and_then(|root| {
                        super::foundation::resolve_structural_path(module, root, &argument.path)
                    })
                    .is_none_or(|referent| {
                        // A fixed byte view observes the array's exact backing;
                        // different carrier IDs do not imply different storage.
                        // The source check above and frontier retain loan authority.
                        !terminal_semantics::borrowed_referent_matches(
                            module.structural_types.iter(),
                            referent,
                            expected.structural_type,
                        )
                    })
            {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
            continue;
        }
        if exclusive_access(expected.access) && !argument.path.is_empty() {
            // An exclusive projected loan joins the rooted place and its
            // projection path as the continuation argument: the subtree
            // under the path stays exclusively bound to the parameter for
            // the target block's duration. The root keeps its custody — a
            // mutable loan moves nothing itself — and the path resolves
            // exactly to the declared referent type.
            if expected.multiplicity != StructuralMultiplicity::Unrestricted
                || !expected.qualifications.is_empty()
                || !expected.projected_qualifications.is_empty()
                || argument.access != expected.access
                || !super::structural::operations::is_static_borrow_path(&argument.path)
                || mutable_loan_root(module, machine, argument, available).and_then(|root| {
                    super::foundation::resolve_structural_path(module, root, &argument.path)
                }) != Some(expected.structural_type)
            {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
            continue;
        }
        if !argument.path.is_empty() {
            // A projected owned argument moves one affine child out of a live
            // root into the target's plain affine parameter. Only Jump edges
            // carry the residual evidence that closes the root; the path must
            // resolve exactly to the declared child type. A machine parameter
            // root is established at entry and dominates every block, so it
            // needs no `available` row — exactly as the whole-owned branch
            // below already treats signature places.
            if !(allow_projected
                && expected.access == StructuralAccess::Owned
                && expected.multiplicity == StructuralMultiplicity::Affine
                && expected.qualifications.is_empty()
                && expected.projected_qualifications.is_empty()
                && argument.access == StructuralAccess::Owned
                && (available.contains(&argument.place)
                    || machine
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == argument.place))
                && super::partial_affine::partial_affine_root_type(machine, argument.place)
                    .is_some_and(|root_type| {
                        super::foundation::resolve_structural_path(
                            module,
                            root_type,
                            &argument.path,
                        ) == Some(expected.structural_type)
                    }))
            {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
            continue;
        }
        let source = machine
            .structural_parameters
            .iter()
            .find(|row| row.place == argument.place)
            .or_else(|| {
                parameter(machine, argument.place).filter(|_| available.contains(&argument.place))
            });
        let exact_source = if let Some(source) = source {
            source.structural_type == expected.structural_type
                && (source.access == expected.access
                    || exclusive_access(expected.access)
                        && super::structural::operations::structural_access_can_supply(
                            source.access,
                            expected.access,
                        ))
                && source.multiplicity == expected.multiplicity
                && source.qualifications == expected.qualifications
                && source.projected_qualifications == expected.projected_qualifications
        } else if exclusive_access(expected.access) {
            expected.multiplicity == StructuralMultiplicity::Unrestricted
                && expected.qualifications.is_empty()
                && expected.projected_qualifications.is_empty()
                && mutable_loan_root(module, machine, argument, available)
                    == Some(expected.structural_type)
        } else if expected.access == StructuralAccess::Owned {
            // The operation independently establishes its exact result. A whole
            // owned edge transfers that value regardless of aggregate syntax.
            // A verified copy may also own descriptor storage while retaining
            // its source loan lineage; it does not own the borrowed referent.
            // An aggregate may likewise own explicitly shared byte descriptor
            // slots without owning their backing. Edge loan replay retains that
            // backing inside this machine; external escape is checked at return.
            // The frontier separately consumes any affine obligation.
            available.contains(&argument.place)
                && (super::structural::result_contracts::has_plain_owned_shape(
                    module,
                    expected.structural_type,
                ) || (terminal_semantics::descriptor_results::aggregate_descriptor_slots(
                    &module.structural_types,
                    argument.place,
                    expected.structural_type,
                )
                .is_some_and(|slots| !slots.is_empty())
                    && terminal_semantics::descriptor_results::owned_shared_byte_descriptor_shape(
                        module.structural_types.iter(),
                        expected.structural_type,
                    ))
                    || super::structural::leaf_copy::copied_result_source(machine, argument.place)
                        .is_some())
                && machine.blocks.iter().any(|block| {
                    dominators.dominates(block.id, source_block)
                        && block.operations.iter().any(|operation| {
                            operation.result.structural().is_some_and(|result| {
                                result.place == argument.place
                                    && result.structural_type == expected.structural_type
                                    && result.multiplicity == expected.multiplicity
                                    && result.qualifications == expected.qualifications
                                    && result.projected_qualifications
                                        == expected.projected_qualifications
                                    && result.claims.is_empty()
                            })
                        })
                })
                || expected.multiplicity == StructuralMultiplicity::Affine
                    && expected.qualifications.is_empty()
                    && expected.projected_qualifications.is_empty()
                    && machine.structural_places.iter().any(|declaration| {
                        declaration.id == argument.place
                            && matches!(
                                declaration.kind,
                                StructuralPlaceKind::TrivialAffineLocal {
                                    structural_type,
                                    ..
                                } if structural_type == expected.structural_type
                            )
                    })
                    && machine.blocks.iter().any(|block| {
                        dominators.dominates(block.id, source_block)
                            && block.operations.iter().any(|operation| {
                                matches!(
                                    operation.kind,
                                    OperationKind::EstablishTrivialAffineLocal { destination }
                                        if destination == argument.place
                                )
                            })
                    })
        } else {
            // Shared-borrow parameters were handled by their own lane above.
            false
        };
        if argument.access != expected.access || !exact_source {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
        }
    }
    // A shared read loan may not coexist with an exclusive loan over an
    // overlapping subtree of the same root on one edge: the exclusive
    // writer would invalidate the reader's observed storage.
    for (shared_index, shared) in target.structural_parameters.iter().enumerate() {
        if shared.access != StructuralAccess::SharedBorrow {
            continue;
        }
        for (mutable_index, mutable) in target.structural_parameters.iter().enumerate() {
            if !exclusive_access(mutable.access)
                || arguments[shared_index].place != arguments[mutable_index].place
            {
                continue;
            }
            if terminal_semantics::structural_paths_may_overlap(
                &arguments[shared_index].path,
                &arguments[mutable_index].path,
            ) {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: arguments[mutable_index].place,
                });
            }
        }
    }
    Ok(())
}

/// The declared type at a shared successor loan's root. A signature or block
/// parameter presents its own custody — anything the access lattice allows
/// to read shared — while a completed record result or an established
/// borrowed byte view carries its producer's exact type. Linear custody
/// cannot present a shared read at all. Non-signature roots must still
/// dominate the edge through `available`; machine parameters are
/// established at entry and dominate every block.
fn shared_loan_root(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
    available: &BTreeSet<PlaceId>,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .or_else(|| {
            parameter(machine, argument.place).filter(|_| available.contains(&argument.place))
        })
    {
        return (super::structural::operations::structural_access_can_supply(
            parameter.access,
            StructuralAccess::SharedBorrow,
        ) && parameter.multiplicity != StructuralMultiplicity::Linear
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty())
        .then_some(parameter.structural_type);
    }
    if let Some(result) = super::record::completed_source(module, machine, argument.place)
        .filter(|_| available.contains(&argument.place))
    {
        return Some(result.structural_type);
    }
    // A literal establishes immutable borrowed-view storage with a Unit
    // result. Its declaration and dominating establishment supply the same
    // whole shared successor view without inventing owned-result custody.
    if argument.path.is_empty()
        && available.contains(&argument.place)
        && let Some(structural_type) =
            machine
                .structural_places
                .iter()
                .find_map(|place| match place.kind {
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } if place.id == argument.place => Some(structural_type),
                    _ => None,
                })
    {
        let mut establishments = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(operation.kind,
                OperationKind::EstablishByteSequenceLiteral { destination, .. }
                    if destination == argument.place)
            });
        let establishment = establishments.next()?;
        if establishments.next().is_some()
            || establishment.result != terminal_psi::OperationResult::Unit
            || !matches!(&establishment.kind,
                OperationKind::EstablishByteSequenceLiteral { qualifications, .. }
                    if qualifications.is_empty())
        {
            return None;
        }
        return module
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == structural_type
                    && matches!(declaration.shape,
                StructuralTypeShape::ByteSequence(carrier) if carrier.is_borrowed())
            })
            .then_some(structural_type);
    }
    if argument.path.is_empty() {
        return super::byte_sequence::subslice::borrowed_result_in_module(
            module,
            machine,
            argument.place,
        )
        .or_else(|| {
            super::element_view::subslice::borrowed_result_in_module(
                module,
                machine,
                argument.place,
            )
        })
        .or_else(|| {
            super::structural::leaf_copy::copied_return_source(machine, argument.place)
                .then_some(argument.place)
                .and_then(|place| {
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .find_map(|operation| {
                            operation.result.structural().filter(|result| {
                                result.place == place
                                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                            })
                        })
                })
        })
        .filter(|_| available.contains(&argument.place))
        .map(|result| result.structural_type);
    }
    // An element view's elements are structural storage: a projected shared
    // loan (`&cells[i]`) roots at the view place and resolves its path into
    // the view's element type, exactly as a record field loan roots at the
    // record. Byte views keep the whole-view rule above — their elements
    // carry no structural projection.
    super::element_view::subslice::borrowed_result_in_module(module, machine, argument.place)
        .filter(|_| available.contains(&argument.place))
        .map(|result| result.structural_type)
}

/// The declared type at an exclusive successor loan's root. A signature or
/// block parameter presents writable custody only through the access
/// lattice: owned or mutable storage can lend either exclusive permission;
/// a write-only source can only supply another write-only name.
/// A produced home or view supplies its independently checked type and access.
/// Linear custody cannot lend at all. Non-signature roots must still dominate the
/// edge through `available`; machine parameters are established at entry
/// and dominate every block.
fn mutable_loan_root(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
    available: &BTreeSet<PlaceId>,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .or_else(|| {
            parameter(machine, argument.place).filter(|_| available.contains(&argument.place))
        })
    {
        return (super::structural::operations::structural_access_can_supply(
            parameter.access,
            argument.access,
        ) && parameter.multiplicity != StructuralMultiplicity::Linear
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty())
        .then_some(parameter.structural_type);
    }
    mutable_records::owned_home(module, machine, argument.place)
        .or_else(|| {
            super::element_view::view_loan_result_in_module(module, machine, argument.place)
                .filter(|(_, access)| {
                    super::structural::operations::structural_access_can_supply(
                        *access,
                        argument.access,
                    )
                })
                .map(|(result, _)| result)
        })
        .filter(|_| available.contains(&argument.place))
        .map(|result| result.structural_type)
}

/// A view producer's checked access lineage supplies authority, not its affine
/// multiplicity alone. This also covers narrowed views without granting a
/// shared source exclusive access or treating descriptor ownership as ownership
/// of the referent. Formation validates the exact result type and custody.
fn mutable_view_result<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    super::element_view::view_loan_result_in_module(module, machine, place)
        .filter(|(_, access)| exclusive_access(*access))
        .map(|(result, _)| result)
}

/// Mutable view and plain-record names transfer at an edge; ordinary calls only reborrow them.
/// Intersect all actual arrivals, including backedges, so an old source name
/// cannot supply a second exclusive alias after a differently named transfer.
pub(super) fn mutable_availability(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> BTreeMap<BlockId, BTreeSet<PlaceId>> {
    let mutable_parameter = |parameter: &StructuralParameterDeclaration| {
        mutable_records::parameter_type(module, parameter)
            || (exclusive_access(parameter.access)
                && module.structural_types.iter().any(|declaration| {
                    declaration.id == parameter.structural_type
                        && (matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(carrier)
                            if carrier.is_borrowed()
                                || matches!(carrier, terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
                        ) || matches!(
                            declaration.shape,
                            StructuralTypeShape::ElementView { .. }
                        ))
                }))
    };
    let record_loans = machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| mutable_records::parameter_type(module, parameter));
    let produced_homes = machine
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.structural())
                    .filter(|result| {
                        (record_loans
                            && mutable_records::owned_home(module, machine, result.place).is_some())
                            || mutable_view_result(module, machine, result.place).is_some()
                    })
                    .map(|result| result.place)
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let universe = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| mutable_parameter(parameter))
        .map(|parameter| parameter.place)
        .chain(produced_homes.values().flatten().copied())
        .collect::<BTreeSet<_>>();
    if universe.is_empty() {
        return BTreeMap::new();
    }
    let record_aliases = mutable_records::aliases(module, machine);
    let initial = machine
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .filter(|place| universe.contains(place))
        .collect::<BTreeSet<_>>();
    let edges = machine
        .blocks
        .iter()
        .flat_map(|block| {
            let successors: Vec<(BlockId, &[StructuralArgument], Vec<PlaceId>)> =
                match &block.terminator {
                    Terminator::Jump {
                        target,
                        structural_arguments,
                        trivial_affine_discards,
                        residual_affine_discards,
                        ..
                    } => vec![(
                        *target,
                        structural_arguments,
                        trivial_affine_discards
                            .iter()
                            .copied()
                            .chain(residual_affine_discards.iter().map(|discard| discard.place))
                            .collect(),
                    )],
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => vec![
                        (
                            when_true.target,
                            &when_true.structural_arguments,
                            when_true.trivial_affine_discards.clone(),
                        ),
                        (
                            when_false.target,
                            &when_false.structural_arguments,
                            when_false.trivial_affine_discards.clone(),
                        ),
                    ],
                    Terminator::StructuralCase { cases, .. } => cases
                        .iter()
                        .map(|case| (case.target, &[][..], case.trivial_affine_discards.clone()))
                        .collect(),
                    _ => Vec::new(),
                };
            successors
                .into_iter()
                .map(move |(target, arguments, discards)| (block.id, target, arguments, discards))
        })
        .collect::<Vec<_>>();
    let mut available = machine
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                if block.id == machine.entry {
                    initial.clone()
                } else {
                    universe.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in &machine.blocks {
            let mut common = if block.id == machine.entry {
                initial.clone()
            } else {
                universe.clone()
            };
            for (source, _, arguments, discards) in
                edges.iter().filter(|(_, target, _, _)| *target == block.id)
            {
                let mut source_available = available[source].clone();
                source_available.extend(produced_homes[source].iter().copied());
                let mut incoming = source_available.clone();
                let transfers = arguments
                    .iter()
                    .zip(&block.structural_parameters)
                    .filter(|(_, parameter)| universe.contains(&parameter.place))
                    .map(|(argument, parameter)| (argument, parameter.place))
                    .collect::<Vec<_>>();
                // Explicit whole-name attenuation retires the exact mutable
                // source too. A shared destination is outside the exclusive
                // namespace, but keeping its old mutable spelling alive would
                // invent overlapping authority beside that shared arrival.
                // The owned backing remains independently live; this is no
                // restoration or permission to use the retired source again.
                // Invocation parameters retain their invocation resource; this
                // retirement concerns edge-scoped block names only.
                for (argument, parameter) in arguments.iter().zip(&block.structural_parameters) {
                    if argument.path.is_empty()
                        && (universe.contains(&parameter.place)
                            || (source_available.contains(&argument.place)
                                && parameter.access == StructuralAccess::SharedBorrow
                                && (self::parameter(machine, argument.place).is_some_and(
                                    |source| source.access == StructuralAccess::MutableBorrow,
                                ) || mutable_view_result(module, machine, argument.place)
                                    .is_some())))
                    {
                        incoming.remove(&argument.place);
                    }
                }
                for (argument, destination) in &transfers {
                    // A projected loan binds its subtree under the target
                    // parameter's own name: the subtree custody was already
                    // proven at the edge, so the destination joins the
                    // mutable namespace even when the root's whole name is
                    // an ambient machine parameter no arrival re-binds.
                    if source_available.contains(&argument.place) || !argument.path.is_empty() {
                        incoming.insert(*destination);
                    }
                }
                // A mutable record parameter's name lives only while some
                // edge still carries it onward: a loan no successor names
                // ends its exclusivity at the boundary, so its storage
                // rejoins the root for the target's own uses. Parameter
                // places that arm continuations forward by name must stay.
                if let Some(source_block) = machine.blocks.iter().find(|block| block.id == *source)
                {
                    for parameter in &source_block.structural_parameters {
                        if mutable_records::parameter_type(module, parameter)
                            && !transfers
                                .iter()
                                .any(|(argument, _)| argument.place == parameter.place)
                            && !edges.iter().any(|(_, _, arguments, _)| {
                                arguments
                                    .iter()
                                    .any(|argument| argument.place == parameter.place)
                            })
                        {
                            incoming.remove(&parameter.place);
                        }
                    }
                }
                // Disposal retires every borrowed name for the home, including
                // dominating names an arm might otherwise reuse without a new
                // parameter. A fresh constructor can establish storage again;
                // an old alias cannot resurrect it at a later join.
                incoming.retain(|place| {
                    !discards.iter().any(|discard| {
                        *place == *discard
                            || record_aliases.as_ref().is_some_and(|aliases| {
                                mutable_records::same_storage(aliases, *place, *discard)
                            })
                    })
                });
                common.retain(|place| incoming.contains(place));
            }
            if available[&block.id] != common {
                available.insert(block.id, common);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    available
}

pub(super) fn is_mutable_name(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
) -> bool {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .or_else(|| parameter(machine, place))
        .is_some_and(|parameter| exclusive_access(parameter.access))
        || mutable_view_result(module, machine, place).is_some()
}
