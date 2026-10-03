//! Primitive byte predicates replay from exact values and declared invariants.
//! These queries grant no availability or loan authority: receiving validation
//! separately checks dominance, source access and current length observations.

use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    ByteSequencePredicate, ByteSequencePredicateSet, OperationKind, StructuralAccess,
    StructuralParameterDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
};

/// The current-content proof needed to install a descriptor in a qualified
/// field. Static predicates may discharge clauses, but never rename the source
/// to its backing: a window and its owner need not satisfy the same predicate.
/// Callers independently derive both predicate sets from their current graph.
pub fn byte_predicate_requirement(
    required: ByteSequencePredicateSet,
    established: ByteSequencePredicateSet,
    root: PlaceId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
) -> semantic_vocabulary::Proposition {
    use semantic_vocabulary::{Proposition, ScalarTerm};
    let mut clauses = Vec::new();
    for predicate in required.predicates() {
        let mut singleton = ByteSequencePredicateSet::EMPTY;
        singleton.insert(predicate);
        if !established.implies(singleton) {
            clauses.push(Proposition::Equal(
                ScalarTerm::ByteSequencePredicate {
                    predicate,
                    root,
                    path: path.to_vec(),
                },
                ScalarTerm::Boolean(true),
            ));
        }
    }
    clauses.sort();
    match clauses.len() {
        0 => Proposition::Truth,
        1 => clauses.pop().expect("one predicate clause"),
        _ => Proposition::Conjunction(clauses),
    }
}

/// Recover plain field-root declaration custody, not current availability or
/// permission to mutate it. An owned result must rejoin its exact producer;
/// root qualifications and claims cannot disappear through a field read.
/// Consumers separately check dominance, live ownership, loan conflicts and
/// whether the operation permits this access (in particular write-only reads).
pub fn byte_field_root(
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<(semantic_vocabulary::StructuralTypeId, StructuralAccess)> {
    if machine
        .entry_claims
        .iter()
        .any(|claim| claim.input == source)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == source)
    {
        return None;
    }
    let place = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)?;
    let parameter = match place.kind {
        StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. } => {
            crate::arrival_storage::arrival_parameter(machine, source)?
        }
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let operation = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == producer)?;
            let result = operation.result.structural()?;
            return (result.place == source
                && result.structural_type == structural_type
                && matches!(
                    result.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                )
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty())
            .then_some((structural_type, StructuralAccess::Owned));
        }
        _ => return None,
    };
    (matches!(
        parameter.multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
            | terminal_psi::StructuralMultiplicity::Affine
    ) && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty())
    .then_some((parameter.structural_type, parameter.access))
}

/// The exact plain byte predicates carried by a formal. Routed/content domains
/// need their own evidence and cannot be treated as byte classifications.
pub fn byte_parameter_predicates(
    module: &TerminalModule,
    parameter: &StructuralParameterDeclaration,
) -> Option<ByteSequencePredicateSet> {
    byte_parameter_predicates_from_domains(&module.structural_domains, parameter)
}

pub fn byte_parameter_predicates_from_domains(
    domains: &[terminal_psi::StructuralDomainDeclaration],
    parameter: &StructuralParameterDeclaration,
) -> Option<ByteSequencePredicateSet> {
    byte_parameter_predicates_in(domains.iter(), parameter)
}

pub fn byte_parameter_predicates_in<'domain>(
    domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
    parameter: &StructuralParameterDeclaration,
) -> Option<ByteSequencePredicateSet> {
    if !parameter.projected_qualifications.is_empty() {
        return None;
    }
    byte_qualification_predicates_in(
        domains,
        parameter.structural_type,
        &parameter.qualifications,
    )
}

/// Interpret an exact primitive qualification roster without manufacturing a
/// parameter. The caller retains its own result or parameter custody checks.
pub fn byte_qualification_predicates_in<'domain>(
    domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
    carrier: semantic_vocabulary::StructuralTypeId,
    qualifications: &[semantic_vocabulary::StructuralDomainId],
) -> Option<ByteSequencePredicateSet> {
    if qualifications.windows(2).any(|pair| pair[0] >= pair[1]) {
        return None;
    }
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    for identity in qualifications {
        let domain = domains.clone().find(|domain| domain.id == *identity)?;
        if domain.carrier != carrier
            || domain.content_projection.is_some()
            || !domain.establishment_routes.is_empty()
        {
            return None;
        }
        predicates.insert(domain.byte_predicate?);
    }
    Some(predicates)
}

/// Authenticate the owned bytes imported by one ordinary normal completion.
/// This query imports only the exact callee signature, never incidental facts
/// about its initializer. Receiving validation separately checks the callee
/// body, call obligations and producer dominance.
pub fn byte_owned_call_result<'machine>(
    module: &TerminalModule,
    machine: &'machine TerminalMachine,
    source: PlaceId,
) -> Option<&'machine terminal_psi::StructuralOperationResult> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    let callee = match operation.kind {
        OperationKind::CallStructural { callee, .. }
        | OperationKind::CallStructuralWithScalarArguments { callee, .. } => callee,
        _ => return None,
    };
    let result = operation.result.structural()?;
    let signature = module
        .machines
        .iter()
        .find(|machine| machine.id == callee)?
        .result
        .structural()?;
    (result.place == source
        && result.structural_type == structural_type
        && signature.structural_type == structural_type
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && signature.multiplicity == result.multiplicity
        && signature.qualifications == result.qualifications
        && signature.projected_qualifications.is_empty()
        && signature.reference_sources.is_empty()
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && module.structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        }))
    .then_some(result)
}

/// The declared primitive predicates of an exact established owned byte result.
/// Initializer octets are not persistent declarations: mutation may discard
/// incidental ASCII or no-NUL facts while preserving these qualifications.
/// This decodes a result contract, not a producer-family permission. Each
/// operation independently proves its result's establishment; calls additionally
/// rejoin the exact callee contract here. Availability remains a use-site check.
pub fn byte_owned_result_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<ByteSequencePredicateSet> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)?;
    let StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = declaration.kind
    else {
        return None;
    };
    if !module.structural_types.iter().any(|declaration| {
        declaration.id == structural_type
            && matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                )
            )
    }) {
        return None;
    }
    byte_result_predicates(module, machine, source)
}

// Result predicates describe the exact completed byte value. Shared descriptor
// custody is not owned backing authority; write consumers still use the bounded
// owner query above and independently validate source permission.
fn byte_result_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<ByteSequencePredicateSet> {
    let place = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = place.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    let result = operation.result.structural()?;
    if result.place != source
        || result.structural_type != structural_type
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            | terminal_psi::ByteSequenceCarrier::BorrowedView {
                                access: None | Some(StructuralAccess::SharedBorrow)
                            }
                    )
                )
        })
    {
        return None;
    }
    if let OperationKind::CallStructural { callee, .. }
    | OperationKind::CallStructuralWithScalarArguments { callee, .. } = operation.kind
    {
        let signature = module
            .machines
            .iter()
            .find(|machine| machine.id == callee)?
            .result
            .structural()?;
        if signature.structural_type != structural_type
            || signature.multiplicity != result.multiplicity
            || signature.qualifications != result.qualifications
            || !signature.projected_qualifications.is_empty()
            || !signature.reference_sources.is_empty()
        {
            return None;
        }
    }
    byte_qualification_predicates_in(
        module.structural_domains.iter(),
        structural_type,
        &result.qualifications,
    )
}

/// Declared predicates captured by copying a shared descriptor field. This
/// resolves exact field identity only; the copy's readable source authority and
/// the copied descriptor's continuing backing loan are receiving obligations.
/// Replacing the original slot does not change this copied descriptor's bytes.
pub fn shared_byte_field_copy_predicates<'a>(
    types: impl Iterator<Item = &'a terminal_psi::StructuralTypeDeclaration> + Clone,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<ByteSequencePredicateSet> {
    let (terminal_psi::StructuralPathSegment::Field(identity), parent_path) = path.split_last()?
    else {
        return None;
    };
    let parent = crate::runtime_structural_path_tip(types.clone(), root, parent_path)?;
    let StructuralTypeShape::Record { fields } = &parent.shape else {
        return None;
    };
    let field = fields
        .iter()
        .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
    if !matches!(
        field.field_type.byte_sequence_carrier(),
        Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: None | Some(StructuralAccess::SharedBorrow)
        })
    ) {
        return None;
    }
    Some(field.field_type.byte_predicates())
}

/// Predicates at one concatenation's establishment, before subsequent writes
/// to its fresh result. Operand observations remain independently validated.
pub fn byte_sequence_concatenation_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<ByteSequencePredicateSet> {
    let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &operation.kind
    else {
        return None;
    };
    let mut resolve = |source| byte_sequence_source_predicates(module, machine, source);
    Some(
        concatenation_operand_predicates(module, machine, left, &mut resolve)?.concatenation(
            concatenation_operand_predicates(module, machine, right, &mut resolve)?,
        ),
    )
}

/// Declared byte invariants captured by a field copy at one owned call.
/// The snapshot observes the field's live bytes, not its spare capacity. A
/// later legal write to the backing field preserves its declared invariants
/// and cannot change this fresh copy. In contrast, any other possible write
/// or exclusive alias of the copy makes this conservative query unavailable.
/// This is not the timeless source query: only this exact consuming call is
/// excluded from the clobber scan. Dominance and loan authority remain the
/// receiving verifier's independent obligations.
pub fn byte_copied_field_predicates_at_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
    consumer: semantic_vocabulary::OperationId,
) -> Option<ByteSequencePredicateSet> {
    let place = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = place.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    let result = operation.result.structural()?;
    let OperationKind::StructuralLeafCopy {
        source: backing,
        path,
    } = &operation.kind
    else {
        return None;
    };
    if result.place != source
        || result.structural_type != structural_type
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
    {
        return None;
    }
    let carrier =
        module
            .structural_types
            .iter()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::ByteSequence(
                    carrier @ terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) if declaration.id == structural_type => Some(carrier),
                _ => None,
            })?;
    let parameter = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == *backing)?;
    if !matches!(
        parameter.access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) {
        return None;
    }
    let (terminal_psi::StructuralPathSegment::Field(identity), parent_path) = path.split_last()?
    else {
        return None;
    };
    // Crossing a reference requires its own referent access proof. This
    // declaration-only query follows owned record/array storage, not pointers.
    if parent_path.iter().any(|segment| {
        !matches!(
            segment,
            terminal_psi::StructuralPathSegment::Field(_)
                | terminal_psi::StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return None;
    }
    let parent = crate::runtime_structural_path_tip(
        module.structural_types.iter(),
        parameter.structural_type,
        parent_path,
    )?;
    let StructuralTypeShape::Record { fields } = &parent.shape else {
        return None;
    };
    let field = fields
        .iter()
        .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
    if field.field_type.byte_sequence_carrier()? != carrier {
        return None;
    }
    let consuming = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == consumer)?;
    let arguments = match &consuming.kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => return None,
    };
    if !arguments.iter().any(|argument| {
        argument.place == source
            && argument.path.is_empty()
            && argument.access == StructuralAccess::Owned
    }) || arguments.iter().any(|argument| {
        argument.place == source
            && (!argument.path.is_empty() || argument.access != StructuralAccess::Owned)
    }) {
        return None;
    }
    if machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|candidate| {
            candidate.id != consumer
                && (may_change_owned_source(candidate, source)
                    || matches!(&candidate.kind,
                OperationKind::EstablishReference { source: argument }
                | OperationKind::EstablishElementView { source: argument, .. }
                | OperationKind::EstablishMutableElementView { source: argument, .. }
                if argument.place == source)
                    || matches!(&candidate.kind, OperationKind::ByteSequenceSubslice { source: viewed, .. }
                        if *viewed == source))
        })
    {
        return None;
    }
    // This bounded query does not chase renamed snapshots through block
    // formals. Otherwise an exclusive use of the new name could evade the
    // direct-place clobber scan while the old name still appeared pristine.
    let carries = |arguments: &[terminal_psi::StructuralArgument]| {
        arguments.iter().any(|argument| argument.place == source)
    };
    if machine.blocks.iter().any(|block| match &block.terminator {
        terminal_psi::Terminator::Jump {
            structural_arguments,
            ..
        } => carries(structural_arguments),
        terminal_psi::Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => carries(&when_true.structural_arguments) || carries(&when_false.structural_arguments),
        terminal_psi::Terminator::ReturnStructural {
            source: returned, ..
        } => *returned == source,
        _ => false,
    }) {
        return None;
    }
    Some(field.field_type.byte_predicates())
}

fn concatenation_operand_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operand: &terminal_psi::control_flow::ByteSequenceOperand,
    resolve_whole: &mut impl FnMut(PlaceId) -> Option<ByteSequencePredicateSet>,
) -> Option<ByteSequencePredicateSet> {
    match operand {
        terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
            resolve_whole(*source)
        }
        terminal_psi::control_flow::ByteSequenceOperand::Field { source, field, .. } => {
            let (structural_type, access) = byte_field_root(machine, source.place)?;
            if source.access != StructuralAccess::SharedBorrow
                || !matches!(
                    access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                )
            {
                return None;
            }
            let declaration = crate::runtime_structural_path_tip(
                module.structural_types.iter(),
                structural_type,
                &source.path,
            )?;
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                return None;
            };
            let field = fields
                .iter()
                .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())?;
            field.field_type.byte_sequence_carrier()?;
            Some(field.field_type.byte_predicates())
        }
    }
}

/// A timeless query cannot retain initializer-specific byte classes after a
/// possible mutation. Call access matters even when another machine performs
/// the write; this classification grants no call or mutation authority.
fn may_change_owned_source(operation: &terminal_psi::Operation, source: PlaceId) -> bool {
    match &operation.kind {
        OperationKind::ByteSequenceWrite { destination, .. }
        | OperationKind::ByteSequenceStore { destination, .. }
        | OperationKind::WriteOnlyPrimitiveStore { destination, .. }
        | OperationKind::StructuralByteSequenceFieldStore { destination, .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { destination, .. }
        | OperationKind::StoreStructuralField { destination, .. } => *destination == source,
        OperationKind::MoveStructuralField { source: moved, .. } => *moved == source,
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.iter().any(|argument| {
            argument.place == source
                && matches!(
                    argument.access,
                    StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                        | StructuralAccess::Owned
                )
        }),
        _ => false,
    }
}

/// Reconstruct predicates valid at every use of a whole source. An untouched
/// concatenation retains its inputs' establishment predicates; a potentially
/// mutated result retains only its declared primitive qualifications.
pub fn byte_sequence_source_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<ByteSequencePredicateSet> {
    fn source_predicates(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: PlaceId,
        fuel: usize,
        memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
    ) -> Option<ByteSequencePredicateSet> {
        if let Some((_, result)) = memo.iter().find(|(place, _)| *place == source) {
            return *result;
        }
        let fuel = fuel.checked_sub(1)?;
        let place = machine
            .structural_places
            .iter()
            .find(|place| place.id == source)?;
        // Mark an in-progress replay before following children. A malformed
        // cycle contributes no predicates, and shared subgraphs replay once.
        let index = memo.len();
        memo.push((source, None));
        let result = (|| match place.kind {
            StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. } => {
                let parameter = crate::arrival_storage::arrival_parameter(machine, source)?;
                // A by-value bounded formal carries its declared invariants
                // just as its borrowed counterpart does. Owning a view does
                // not confer authority over the view's backing storage.
                let owned_buffer = parameter.access == StructuralAccess::Owned
                    && module.structural_types.iter().any(|row| {
                        row.id == parameter.structural_type
                            && matches!(
                                row.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                            )
                    });
                if !owned_buffer
                    && !matches!(
                        parameter.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                {
                    return None;
                }
                byte_parameter_predicates(module, parameter)
            }
            StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                let mut producers = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match &operation.kind {
                        OperationKind::EstablishByteSequenceLiteral {
                            destination, bytes, ..
                        } if *destination == source => Some(bytes),
                        _ => None,
                    });
                let bytes = producers.next()?;
                if producers.next().is_some() {
                    return None;
                }
                let mut predicates = ByteSequencePredicateSet::EMPTY;
                for predicate in ByteSequencePredicate::ALL {
                    if predicate.holds_for(bytes) {
                        predicates.insert(predicate);
                    }
                }
                Some(predicates)
            }
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                let operation = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == producer)?;
                let result = operation.result.structural()?;
                if result.place != source || result.structural_type != structural_type {
                    return None;
                }
                if let OperationKind::ByteSequenceSubslice {
                    source: backing, ..
                } = operation.kind
                {
                    if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                        || !result.qualifications.is_empty()
                        || !result.projected_qualifications.is_empty()
                        || !result.qualification_establishments.is_empty()
                        || !result.claims.is_empty()
                        || !module.structural_types.iter().any(|declaration| {
                            declaration.id == structural_type
                                && matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                                            access: None | Some(StructuralAccess::SharedBorrow)
                                        }
                                    )
                                )
                        })
                    {
                        return None;
                    }
                    // Only the exact backing's invariant survives. Bounds,
                    // source access and dominance remain use-site obligations.
                    return Some(
                        source_predicates(module, machine, backing, fuel, memo)?.subslice(),
                    );
                }
                let declared = byte_result_predicates(module, machine, source)?;
                if let OperationKind::StructuralLeafCopy {
                    source: backing,
                    path,
                } = &operation.kind
                    && module.structural_types.iter().any(|declaration| {
                        declaration.id == structural_type
                            && declaration.shape.is_borrowed_byte_view()
                    })
                {
                    let captured = if path.is_empty() {
                        source_predicates(module, machine, *backing, fuel, memo)?
                    } else {
                        let (root, access) = byte_field_root(machine, *backing)?;
                        if access == StructuralAccess::WriteOnlyBorrow {
                            return None;
                        }
                        shared_byte_field_copy_predicates(
                            module.structural_types.iter(),
                            root,
                            path,
                        )?
                    };
                    let mut predicates = declared;
                    for predicate in captured.predicates() {
                        predicates.insert(predicate);
                    }
                    return Some(predicates);
                }
                let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } =
                    &operation.kind
                else {
                    return Some(declared);
                };
                if machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|candidate| may_change_owned_source(candidate, source))
                {
                    return byte_owned_result_predicates(module, machine, source);
                }
                let mut resolve = |source| source_predicates(module, machine, source, fuel, memo);
                Some(
                    concatenation_operand_predicates(module, machine, left, &mut resolve)?
                        .concatenation(concatenation_operand_predicates(
                            module,
                            machine,
                            right,
                            &mut resolve,
                        )?),
                )
            }
            _ => None,
        })();
        memo[index].1 = result;
        result
    }
    source_predicates(
        module,
        machine,
        source,
        machine.structural_places.len().saturating_add(1),
        &mut Vec::new(),
    )
}

/// The complete goal of an indexed byte store into a field declaring
/// `predicates`: the operand-only `bound` (`index < length`) followed by the
/// stored byte's class legs. `ascii_only` and `valid_utf8` require
/// `value <= 127`; `no_nul` requires `1 <= value`; `non_empty` requires
/// nothing, because a store keeps the extent. A plain carrier's goal is the
/// bound alone. The Terminal verifier reconstructs this goal and the Omega
/// unit validator requires the accepted obligation to be exactly it, so the
/// two cannot disagree about what a qualified store proved.
///
/// The byte legs are necessary, not sufficient, for `valid_utf8`: an ASCII
/// byte can still split a multi-byte scalar. That half is a custody rule on
/// the carrier's prior contents, which each consumer checks on its own graph.
pub fn indexed_byte_store_goal(
    bound: semantic_vocabulary::Proposition,
    value: semantic_vocabulary::ValueId,
    predicates: ByteSequencePredicateSet,
) -> Result<semantic_vocabulary::Proposition, semantic_vocabulary::PropositionError> {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
    };
    let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid");
    let literal =
        |byte: u8| ScalarTerm::integer(byte_type, IntegerValue::Unsigned(u128::from(byte)));
    let stored = ScalarTerm::value(value, ScalarType::Integer(byte_type));
    let declares = |predicate| {
        predicates
            .predicates()
            .any(|declared| declared == predicate)
    };
    let mut legs = vec![bound];
    if declares(ByteSequencePredicate::AsciiOnly) || declares(ByteSequencePredicate::ValidUtf8) {
        legs.push(Proposition::LessOrEqual(stored.clone(), literal(0x7f)?));
    }
    if declares(ByteSequencePredicate::NoNul) {
        legs.push(Proposition::LessOrEqual(literal(1)?, stored));
    }
    Ok(match legs.len() {
        1 => legs.pop().expect("one leg"),
        _ => Proposition::Conjunction(legs),
    })
}

/// Literal qualification rosters are claims about these exact octets, never
/// trusted annotations. Unsupported nominal qualifications cannot be minted.
pub fn byte_literal_qualifications_hold(
    domains: &[terminal_psi::StructuralDomainDeclaration],
    carrier: semantic_vocabulary::StructuralTypeId,
    qualifications: &[semantic_vocabulary::StructuralDomainId],
    bytes: &[u8],
) -> bool {
    byte_literal_qualifications_hold_in(domains.iter(), carrier, qualifications, bytes)
}

pub fn byte_literal_qualifications_hold_in<'domain>(
    domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
    carrier: semantic_vocabulary::StructuralTypeId,
    qualifications: &[semantic_vocabulary::StructuralDomainId],
    bytes: &[u8],
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|identity| {
            domains
                .clone()
                .find(|domain| domain.id == *identity)
                .is_some_and(|domain| {
                    domain.carrier == carrier
                        && domain.content_projection.is_none()
                        && domain.establishment_routes.is_empty()
                        && domain
                            .byte_predicate
                            .is_some_and(|predicate| predicate.holds_for(bytes))
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{DomainSemanticId, StructuralDomainId, StructuralTypeId};
    use terminal_psi::{StructuralDomainDeclaration, StructuralEstablishmentRoute};

    #[test]
    fn owned_source_predicate_invalidation_tracks_call_access_and_exact_root() {
        let source = PlaceId::new(1).unwrap();
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::Owned,
        ] {
            let operation = terminal_psi::Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: semantic_vocabulary::OperationId::new(1).unwrap(),
                result: terminal_psi::OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: semantic_vocabulary::MachineId::new(1).unwrap(),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: vec![terminal_psi::StructuralArgument {
                        place: source,
                        path: Vec::new(),
                        access,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            };
            assert_eq!(
                may_change_owned_source(&operation, source),
                access != StructuralAccess::SharedBorrow
            );
            assert!(!may_change_owned_source(
                &operation,
                PlaceId::new(2).unwrap()
            ));
        }
    }

    #[test]
    fn literal_membership_replays_octets_carrier_and_nonrouted_definition() {
        let carrier = StructuralTypeId::new(1).unwrap();
        let identity = StructuralDomainId::new(1).unwrap();
        let mut domains = [StructuralDomainDeclaration {
            id: identity,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: "bytes".into(),
            carrier,
            content_projection: None,
            byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
            establishment_routes: Vec::new(),
        }];
        assert!(byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity],
            "é".as_bytes()
        ));
        assert!(!byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity],
            &[0xff]
        ));
        assert!(!byte_literal_qualifications_hold(
            &domains,
            StructuralTypeId::new(2).unwrap(),
            &[identity],
            b"ok"
        ));
        assert!(!byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity, identity],
            b"ok"
        ));
        domains[0]
            .establishment_routes
            .push(StructuralEstablishmentRoute::ExactMachine {
                machine: "issuer".into(),
            });
        assert!(!byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity],
            b"ok"
        ));
        domains[0].establishment_routes.clear();
        domains[0].byte_predicate = None;
        assert!(!byte_literal_qualifications_hold(
            &domains,
            carrier,
            &[identity],
            b"ok"
        ));
    }
}
