//! Replay declared byte invariants against the current optimized operands.
//! A retained capacity fact cannot authorize replacing a qualified destination
//! with a different source of equal length but incompatible byte content.
//! An indexed store into a predicate-bearing field is admitted only with its
//! accepted goal carrying the stored byte's class legs (checked beside the
//! unit's obligation catalog) and, for `valid_utf8`, an all-ASCII carrier on
//! every path of the current graph, checked here.
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as Operation;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ByteSequencePredicate, ByteSequencePredicateSet, StructuralAccess, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// Every indexed store into a field declaring `valid_utf8` without
/// `ascii_only` needs the field all-ASCII on every path of the current graph:
/// an ASCII byte written over part of a multi-byte scalar would break it. The
/// Terminal verifier admitted the store under the same rule; rewrites since
/// then (reordering, removed replacements, merged blocks) must not have
/// weakened it, so the rule is replayed here rather than trusted.
///
/// Each path must reach a whole replacement of the identical field whose
/// source replays to `ascii_only`. Stores into the identical field keep the
/// class, because each proves its own byte ASCII; any other operation that
/// could change the field's extent or contents rejects.
pub(super) fn validate_indexed_byte_stores(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut ascii = ByteSequencePredicateSet::EMPTY;
    ascii.insert(ByteSequencePredicate::AsciiOnly);
    for (block_position, block) in function.blocks.iter().enumerate() {
        for (node_position, node) in block.nodes.iter().enumerate() {
            if let Operation::ByteSequenceWrite { destination, .. } = &node.operation {
                if let Some(predicates) = owned_byte_predicates(unit, function, *destination) {
                    let splits_scalars = predicates
                        .predicates()
                        .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
                        && !predicates.implies(ascii);
                    if splits_scalars
                        && !super::structural_freshness::established_on_every_path(
                            function,
                            predecessors,
                            (block_position, node_position),
                            *destination,
                            |_, operation, root| {
                                matches!(operation,
                            Operation::EstablishByteSequenceConcatenation { result, .. }
                                if result.place == root
                                    && concatenation_predicates(unit, function, operation)
                                        .is_some_and(|initial| initial.implies(ascii)))
                            },
                            |operation, root| {
                                super::structural_freshness::structural_metadata_may_change(
                                    function,
                                    types,
                                    root,
                                    &[],
                                    operation,
                                )
                            },
                        )
                    {
                        return Err(OptimizationUnitValidationError::InvalidByteSequenceWrite {
                            machine: function.machine,
                            block: block.id,
                            node: u32::try_from(node_position).unwrap_or(u32::MAX),
                        });
                    }
                }
                continue;
            }
            let Operation::StructuralByteSequenceFieldByteStore {
                destination,
                path,
                field,
                ..
            } = &node.operation
            else {
                continue;
            };
            let invalid = || OptimizationUnitValidationError::InvalidByteSequenceWrite {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_position).unwrap_or(u32::MAX),
            };
            let predicates = byte_field_predicates(unit, function, *destination, path, *field)
                .ok_or_else(invalid)?;
            let splits_scalars = predicates
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
                && !predicates.implies(ascii);
            if !splits_scalars {
                continue;
            }
            let exact_path = super::byte_field_freshness::field_path(
                function,
                types,
                *destination,
                path,
                *field,
            )
            .ok_or_else(invalid)?;
            let establishes = |_, operation: &Operation, root: PlaceId| {
                matches!(operation,
                    Operation::StructuralByteSequenceFieldStore {
                        destination: written_root, path: written, field: written_field, source, ..
                    } if *written_root == root && written == path && written_field == field
                        && whole_source_predicates(
                            unit.structural_types.iter(),
                            unit.structural_domains.iter(),
                            function,
                            *source,
                        )
                        .is_some_and(|available| available.implies(ascii)))
            };
            let invalidates = |operation: &Operation, root: PlaceId| match operation {
                Operation::StructuralByteSequenceFieldByteStore {
                    destination: written_root,
                    path: written,
                    field: written_field,
                    ..
                } if *written_root == root => {
                    !(written == path && written_field == field)
                        && super::byte_field_freshness::field_path(
                            function,
                            types,
                            root,
                            written,
                            *written_field,
                        )
                        .is_none_or(|written| {
                            super::byte_field_freshness::overlaps(&exact_path, &written)
                        })
                }
                _ => super::structural_freshness::structural_metadata_may_change(
                    function,
                    types,
                    root,
                    &exact_path,
                    operation,
                ),
            };
            if !super::structural_freshness::established_on_every_path(
                function,
                predecessors,
                (block_position, node_position),
                *destination,
                establishes,
                invalidates,
            ) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Declared invariants of the exact completed owner; initial byte properties
/// are separate, since indexed writes can invalidate incidental predicates.
pub(crate) fn owned_byte_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<ByteSequencePredicateSet> {
    let declaration = function
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let result = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            Operation::EstablishByteSequenceConcatenation {
                psi_operation,
                result,
                ..
            } if *psi_operation == producer
                && result.place == place
                && result.structural_type == structural_type =>
            {
                Some(result)
            }
            _ => None,
        })?;
    if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !unit.structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
    {
        return None;
    }
    terminal_semantics::byte_qualification_predicates_in(
        unit.structural_domains.iter(),
        structural_type,
        &result.qualifications,
    )
}

fn concatenation_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    operation: &Operation,
) -> Option<ByteSequencePredicateSet> {
    let Operation::EstablishByteSequenceConcatenation { left, right, .. } = operation else {
        return None;
    };
    let operand = |operand: &terminal_psi::control_flow::ByteSequenceOperand| match operand {
        terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
            whole_source_predicates(
                unit.structural_types.iter(),
                unit.structural_domains.iter(),
                function,
                *source,
            )
        }
        terminal_psi::control_flow::ByteSequenceOperand::Field { source, field, .. }
            if source.access == StructuralAccess::SharedBorrow =>
        {
            byte_field_predicates(unit, function, source.place, &source.path, *field)
        }
        _ => None,
    };
    Some(operand(left)?.concatenation(operand(right)?))
}

pub(crate) fn concatenation_preserves_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    operation: &Operation,
) -> bool {
    let Operation::EstablishByteSequenceConcatenation { result, .. } = operation else {
        return false;
    };
    owned_byte_predicates(unit, function, result.place).is_some_and(|required| {
        required.is_empty()
            || concatenation_predicates(unit, function, operation)
                .is_some_and(|initial| initial.implies(required))
    })
}

pub(crate) fn replacement_preserves_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    operation: &Operation,
) -> bool {
    let (source, required) = match operation {
        Operation::ByteSequenceStore {
            destination,
            source,
            ..
        } => {
            let Some(parameter) = function.arrival_parameter(*destination) else {
                return false;
            };
            (
                *source,
                terminal_semantics::byte_parameter_predicates_from_domains(
                    &unit.structural_domains,
                    parameter,
                ),
            )
        }
        Operation::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => (
            *source,
            byte_field_predicates(unit, function, *destination, path, *field),
        ),
        _ => return false,
    };
    let Some(required) = required else {
        return false;
    };
    required.is_empty()
        || whole_source_predicates(
            unit.structural_types.iter(),
            unit.structural_domains.iter(),
            function,
            source,
        )
        .is_some_and(|available| available.implies(required))
}

/// The byte predicates one whole source replays to: a parameter's declared
/// plain byte domains, a literal's own octets, or an owned concatenation's
/// operands. Catalogs are passed as iterators so a caller holding only
/// identity maps shares the one derivation.
pub(crate) fn whole_source_predicates<'t, 'd>(
    types: impl Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'d terminal_psi::StructuralDomainDeclaration> + Clone,
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Option<ByteSequencePredicateSet> {
    source_predicates(
        &Catalogs { types, domains },
        function,
        source,
        function.structural_places.len().saturating_add(1),
        &mut Vec::new(),
    )
}

/// Declaration predicates captured by one byte-field snapshot at an owned
/// ordinary call. Only that consumer is excluded from the conservative write
/// scan; this is not a timeless initializer fact or a permission to borrow the
/// field. Current-graph custody independently checks availability and dominance.
pub(crate) fn copied_field_predicates_at_call<'t>(
    types: impl Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
    function: &PsiOptimizationFunction,
    source: PlaceId,
    consumer: semantic_vocabulary::OperationId,
) -> Option<ByteSequencePredicateSet> {
    let declaration = function
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
    let node = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| node.operation.psi_operation() == Some(producer))?;
    let Operation::StructuralLeafCopy {
        source: backing,
        path: abstract_operations::StructuralCopyPath::Ordinary(path),
        result,
        ..
    } = &node.operation
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
    let carrier = types
        .clone()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::ByteSequence(
                carrier @ terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) if declaration.id == structural_type => Some(carrier),
            _ => None,
        })?;
    let parameter = function
        .structural_parameters
        .iter()
        .chain(
            function
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
    if parent_path.iter().any(|segment| {
        !matches!(
            segment,
            terminal_psi::StructuralPathSegment::Field(_)
                | terminal_psi::StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return None;
    }
    let parent = terminal_semantics::runtime_structural_path_tip(
        types,
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
    let consuming = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| node.operation.psi_operation() == Some(consumer))?;
    let Operation::CallStructural {
        structural_arguments,
        ..
    } = &consuming.operation
    else {
        return None;
    };
    if !structural_arguments.iter().any(|argument| {
        argument.place == source
            && argument.path.is_empty()
            && argument.access == StructuralAccess::Owned
    }) || structural_arguments.iter().any(|argument| {
        argument.place == source
            && (!argument.path.is_empty() || argument.access != StructuralAccess::Owned)
    }) {
        return None;
    }
    let carries = |bindings: &[abstract_operations::AbstractStructuralBinding]| {
        bindings
            .iter()
            .any(|binding| binding.argument.place == source)
    };
    if function.blocks.iter().flat_map(|block| &block.nodes).any(|node| {
        let operation = &node.operation;
        // An edge can rename the snapshot; a place-only clobber scan must not
        // infer that such an alias stays pristine. Backing-field writes are
        // independent because this operation already captured its own bytes.
        let escapes = match operation {
            Operation::Jump { structural_bindings, .. } => carries(structural_bindings),
            Operation::Conditional { when_true, when_false, .. } => carries(&when_true.structural_bindings) || carries(&when_false.structural_bindings),
            Operation::ReturnStructural { source: returned, .. } => *returned == source,
            _ => false,
        };
        escapes || (operation.psi_operation() != Some(consumer)
            && (may_change_owned_source(operation, source)
                || matches!(operation, Operation::EstablishReference { source: argument, .. } | Operation::EstablishElementView { source: argument, .. } if argument.place == source)
                || matches!(operation, Operation::ByteSequenceSubslice { source: viewed, .. } if *viewed == source)
                || matches!(operation, Operation::CallDynamic { .. })))
    }) { return None; }
    Some(field.field_type.byte_predicates())
}

/// The type and domain catalogs a predicate replay reads.
struct Catalogs<Types, Domains> {
    types: Types,
    domains: Domains,
}

/// The declared byte predicates of one exact relevant byte field below a
/// structural root; empty for a plain carrier, `None` when the path does
/// not resolve to a byte field.
pub(crate) fn byte_field_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    root: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<ByteSequencePredicateSet> {
    byte_field_predicates_in(unit.structural_types.iter(), function, root, path, field)
}

fn byte_field_predicates_in<'t>(
    types: impl Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
    function: &PsiOptimizationFunction,
    root: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<ByteSequencePredicateSet> {
    let (structural_type, _) = super::byte_field_freshness::byte_field_root(function, root)?;
    // The carrier's affine permission and referent shape are reconstructed
    // before traversing storage. Keep the explicit path on the operation.
    let (structural_type, path) = if matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        let catalog = types
            .clone()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        super::super::references::mutable_record_field_destination(function, &catalog, root, path)?
    } else {
        (structural_type, path)
    };
    let declaration =
        terminal_semantics::runtime_structural_path_tip(types, structural_type, path)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let field = fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?;
    field.field_type.byte_sequence_carrier()?;
    Some(field.field_type.byte_predicates())
}

fn source_predicates<'t, 'd, Types, Domains>(
    catalogs: &Catalogs<Types, Domains>,
    function: &PsiOptimizationFunction,
    source: PlaceId,
    remaining: usize,
    memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
) -> Option<ByteSequencePredicateSet>
where
    Types: Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
    Domains: Iterator<Item = &'d terminal_psi::StructuralDomainDeclaration> + Clone,
{
    if let Some((_, predicates)) = memo.iter().find(|(place, _)| *place == source) {
        return *predicates;
    }
    let remaining = remaining.checked_sub(1)?;
    // A completed source may be reused by both operands of many later concats.
    // Reserve its unknown result before walking children: each source is replayed
    // once, and an in-progress revisit rejects a cycle without recursive expansion.
    let index = memo.len();
    memo.push((source, None));
    let result = derive_source_predicates(catalogs, function, source, remaining, memo);
    memo[index].1 = result;
    result
}

fn derive_source_predicates<'t, 'd, Types, Domains>(
    catalogs: &Catalogs<Types, Domains>,
    function: &PsiOptimizationFunction,
    source: PlaceId,
    remaining: usize,
    memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
) -> Option<ByteSequencePredicateSet>
where
    Types: Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
    Domains: Iterator<Item = &'d terminal_psi::StructuralDomainDeclaration> + Clone,
{
    let declaration = function
        .structural_places
        .iter()
        .find(|place| place.id == source)?;
    match declaration.kind {
        StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. } => {
            let parameter = function.arrival_parameter(source)?;
            if !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ) && !abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter(
                parameter,
                |identity| {
                    catalogs
                        .types
                        .clone()
                        .find(|declaration| declaration.id == identity)
                },
            ) {
                return None;
            }
            terminal_semantics::byte_parameter_predicates_in(catalogs.domains.clone(), parameter)
        }
        StructuralPlaceKind::ByteSequenceLiteral { .. } => {
            let mut producers = function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter_map(|node| match &node.operation {
                    Operation::EstablishByteSequenceLiteral { place, bytes, .. }
                        if place.id == source =>
                    {
                        Some(bytes)
                    }
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
            let node = function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find(|node| node.operation.psi_operation() == Some(producer))?;
            if let Operation::ByteSequenceSubslice {
                source: backing,
                result,
                ..
            } = &node.operation
            {
                if result.place != source
                    || result.structural_type != structural_type
                    || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                    || !catalogs.types.clone().any(|declaration| {
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
                return Some(
                    source_predicates(catalogs, function, *backing, remaining, memo)?.subslice(),
                );
            }
            let (_, result) = node.operation.structural_result()?;
            if result.place != source
                || result.structural_type != structural_type
                || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
                || !catalogs.types.clone().any(|declaration| {
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
            let declared = terminal_semantics::byte_qualification_predicates_in(
                catalogs.domains.clone(),
                structural_type,
                &result.qualifications,
            )?;
            // Signature/custody validation independently rejoins ordinary call
            // results. Here only their declared value predicates are imported.
            if let Operation::StructuralLeafCopy {
                source: backing,
                path: abstract_operations::StructuralCopyPath::Ordinary(path),
                ..
            } = &node.operation
                && catalogs.types.clone().any(|declaration| {
                    declaration.id == structural_type && declaration.shape.is_borrowed_byte_view()
                })
            {
                let captured = if path.is_empty() {
                    source_predicates(catalogs, function, *backing, remaining, memo)?
                } else {
                    let (root, access) =
                        super::byte_field_freshness::byte_field_root(function, *backing)?;
                    if access == StructuralAccess::WriteOnlyBorrow {
                        return None;
                    }
                    terminal_semantics::shared_byte_field_copy_predicates(
                        catalogs.types.clone(),
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
            let operands = match &node.operation {
                Operation::EstablishByteSequenceConcatenation { left, right, .. } => {
                    Some((left, right))
                }
                _ => None,
            };
            if function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| may_change_owned_source(&node.operation, source))
            {
                return Some(declared);
            }
            let Some((left, right)) = operands else {
                // Ordinary call completion carries the callee's verified
                // qualification roster on this exact result. Unlike a local
                // concatenation it has no caller-side initializer operands to
                // re-derive; later writes are checked against the same roster.
                return Some(declared);
            };
            let mut operand =
                |operand: &terminal_psi::control_flow::ByteSequenceOperand| match operand {
                    terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                        source_predicates(catalogs, function, *source, remaining, memo)
                    }
                    terminal_psi::control_flow::ByteSequenceOperand::Field {
                        source,
                        field,
                        ..
                    } => {
                        if source.access != StructuralAccess::SharedBorrow {
                            return None;
                        }
                        byte_field_predicates_in(
                            catalogs.types.clone(),
                            function,
                            source.place,
                            &source.path,
                            *field,
                        )
                    }
                };
            Some(operand(left)?.concatenation(operand(right)?))
        }
        _ => None,
    }
}

/// Initializer predicates are not invariants across calls or local writes.
/// Match Terminal's timeless source query; this grants no mutation authority.
fn may_change_owned_source(operation: &Operation, source: PlaceId) -> bool {
    match operation {
        Operation::ByteSequenceWrite { destination, .. }
        | Operation::ByteSequenceStore { destination, .. }
        | Operation::StructuralByteSequenceFieldStore { destination, .. }
        | Operation::StructuralByteSequenceFieldByteStore { destination, .. } => {
            *destination == source
        }
        Operation::WriteOnlyPrimitiveStore { destination, .. } => destination.place() == source,
        Operation::StructuralScalarFieldStore { destination, .. } => destination.place() == source,
        Operation::StoreStructuralField { destination, .. } => destination.place() == source,
        Operation::MoveStructuralField { source: moved, .. } => moved.place() == source,
        Operation::CallStructural {
            dynamic_arguments, ..
        } if !dynamic_arguments.is_empty() => true,
        Operation::CallStructural {
            structural_arguments,
            ..
        }
        | Operation::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.iter().any(|argument| {
            argument.place == source && argument.access != StructuralAccess::SharedBorrow
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use abstract_operations::AbstractStructuralCallResult;
    use semantic_vocabulary::{ObligationId, OperationId, StructuralTypeId, ValueId};
    use terminal_psi::control_flow::ByteSequenceOperand;

    // This raw graph exercises predicate traversal only. Forty doubling nodes
    // describe exponentially many paths while retaining forty-one source places.
    // Each concatenation result is an owned byte result, so the unit's catalog
    // declares its bounded owner type.
    fn shared_concatenations() -> PsiOptimizationUnit {
        let mut unit = crate::tests::fixtures::scalar_units::unit();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let declaration = terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "bytes".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 1 << 40 },
            ),
        };
        assert!(unit.structural_types.is_empty());
        unit.structural_types = vec![declaration.clone()].into();
        let function = &mut unit.functions[0];
        let template = function.blocks[0].nodes[0].clone();
        function.blocks[0].nodes.clear();
        let literal = terminal_psi::StructuralPlaceDeclaration {
            id: PlaceId::new(1).unwrap(),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type,
            },
        };
        function.structural_places.push(literal);
        let mut node = template.clone();
        node.operation = Operation::EstablishByteSequenceLiteral {
            psi_operation: OperationId::new(1).unwrap(),
            place: literal,
            structural_type: declaration,
            bytes: b"A".to_vec(),
            qualifications: Vec::new(),
        };
        function.blocks[0].nodes.push(node);
        for identity in 2..=41 {
            let place = PlaceId::new(identity).unwrap();
            let producer = OperationId::new(identity).unwrap();
            function
                .structural_places
                .push(terminal_psi::StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    },
                });
            let operand = ByteSequenceOperand::Whole {
                source: PlaceId::new(identity - 1).unwrap(),
                length: ValueId::new(1).unwrap(),
            };
            let mut node = template.clone();
            node.operation = Operation::EstablishByteSequenceConcatenation {
                psi_operation: producer,
                left: operand.clone(),
                right: operand,
                capacity_obligation: ObligationId::new(1).unwrap(),
                result: terminal_psi::StructuralOperationResult {
                    place,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                },
            };
            function.blocks[0].nodes.push(node);
        }
        unit
    }

    fn catalogs(
        unit: &PsiOptimizationUnit,
    ) -> Catalogs<
        std::slice::Iter<'_, StructuralTypeDeclaration>,
        std::slice::Iter<'_, terminal_psi::StructuralDomainDeclaration>,
    > {
        Catalogs {
            types: unit.structural_types.iter(),
            domains: unit.structural_domains.iter(),
        }
    }

    // Predicate replay consumes producer identity, not endpoints or stored
    // result qualifications. Separate operation validation owns the bounds.
    fn nested_subslices() -> PsiOptimizationUnit {
        let mut unit = shared_concatenations();
        let structural_type = unit.structural_types[0].id;
        unit.structural_types.make_mut()[0].shape =
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: None,
            });
        let function = &mut unit.functions[0];
        for node in &mut function.blocks[0].nodes {
            if let Operation::EstablishByteSequenceLiteral {
                structural_type: declaration,
                ..
            } = &mut node.operation
            {
                declaration.shape = StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { access: None },
                );
            }
            if let Operation::EstablishByteSequenceConcatenation {
                psi_operation,
                result,
                left,
                ..
            } = &node.operation
            {
                assert_eq!(result.structural_type, structural_type);
                node.operation = Operation::ByteSequenceSubslice {
                    psi_operation: *psi_operation,
                    result: result.clone(),
                    source: left.source(),
                    start: ValueId::new(1).unwrap(),
                    end: ValueId::new(1).unwrap(),
                    length: ValueId::new(1).unwrap(),
                    obligation: ObligationId::new(1).unwrap(),
                };
            }
        }
        unit
    }

    #[test]
    fn subslice_predicates_replay_exact_backing_and_drop_sequence_properties() {
        let unit = nested_subslices();
        let replay = |unit: &PsiOptimizationUnit| {
            whole_source_predicates(
                unit.structural_types.iter(),
                unit.structural_domains.iter(),
                &unit.functions[0],
                PlaceId::new(41).unwrap(),
            )
        };
        let predicates = replay(&unit).unwrap();
        assert_eq!(
            predicates.predicates().collect::<Vec<_>>(),
            vec![
                ByteSequencePredicate::NoNul,
                ByteSequencePredicate::AsciiOnly
            ]
        );
        assert!(predicates.holds_for(b""));
        assert!(!predicates.holds_for(&[0]));

        // Changing the root literal changes all descendants' actual facts.
        let mut changed = unit.clone();
        let Operation::EstablishByteSequenceLiteral { bytes, .. } =
            &mut changed.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("literal");
        };
        *bytes = "é".as_bytes().to_vec();
        assert_eq!(
            replay(&changed).unwrap().predicates().collect::<Vec<_>>(),
            vec![ByteSequencePredicate::NoNul]
        );

        for forged_source in [2, 1000] {
            let mut changed = unit.clone();
            let Operation::ByteSequenceSubslice { source, .. } =
                &mut changed.functions[0].blocks[0].nodes[1].operation
            else {
                panic!("subslice");
            };
            *source = PlaceId::new(forged_source).unwrap();
            assert_eq!(replay(&changed), None, "cycle or unknown source");
        }
        for corruption in 0..4 {
            let mut changed = unit.clone();
            let Operation::ByteSequenceSubslice { result, .. } =
                &mut changed.functions[0].blocks[0].nodes[1].operation
            else {
                panic!("subslice");
            };
            match corruption {
                0 => result.place = PlaceId::new(1).unwrap(),
                1 => result.structural_type = StructuralTypeId::new(99).unwrap(),
                2 => result
                    .qualifications
                    .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap()),
                3 => result.multiplicity = terminal_psi::StructuralMultiplicity::Affine,
                _ => unreachable!(),
            }
            assert_eq!(
                replay(&changed),
                None,
                "forged subslice result {corruption}"
            );
        }
    }

    #[test]
    fn shared_concatenation_predicates_visit_each_source_once() {
        let unit = shared_concatenations();
        let function = &unit.functions[0];
        let mut memo = Vec::new();
        let predicates = source_predicates(
            &catalogs(&unit),
            function,
            PlaceId::new(41).unwrap(),
            42,
            &mut memo,
        )
        .unwrap();
        assert!(predicates.holds_for(b"AAAA"));
        assert!(
            predicates
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
        );
        assert_eq!(memo.len(), 41);
        let cached = memo.clone();
        assert_eq!(
            source_predicates(
                &catalogs(&unit),
                function,
                PlaceId::new(41).unwrap(),
                42,
                &mut memo
            ),
            Some(predicates)
        );
        assert_eq!(memo, cached);
    }

    #[test]
    fn shared_concatenation_predicates_reject_cycles_and_unknown_sources() {
        for corrupted_source in [2, 1000] {
            let mut unit = shared_concatenations();
            let Operation::EstablishByteSequenceConcatenation { left, .. } =
                &mut unit.functions[0].blocks[0].nodes[1].operation
            else {
                panic!("concat");
            };
            *left = ByteSequenceOperand::Whole {
                source: PlaceId::new(corrupted_source).unwrap(),
                length: ValueId::new(1).unwrap(),
            };
            let mut memo = Vec::new();
            assert_eq!(
                source_predicates(
                    &catalogs(&unit),
                    &unit.functions[0],
                    PlaceId::new(41).unwrap(),
                    42,
                    &mut memo
                ),
                None
            );
            assert!(memo.len() <= 41);
        }
    }

    /// The current graph, not the admitted Terminal one, must still show an
    /// all-ASCII carrier before an indexed store into a UTF-8 field.
    #[test]
    fn indexed_utf8_store_needs_an_ascii_carrier_on_every_current_path() {
        let unit = crate::tests::indexed_byte_fields::indexed_field_unit();
        let function = unit
            .functions
            .iter()
            .find(|function| function.machine == unit.entry)
            .unwrap();
        let types = unit
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<BTreeMap<_, _>>();
        let predecessors = BTreeMap::new();
        validate_indexed_byte_stores(&unit, function, &types, &predecessors)
            .expect("an ASCII literal replacement dominates the store");
        let (block, store) = function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block, owner)| {
                owner
                    .nodes
                    .iter()
                    .position(|node| {
                        matches!(
                            node.operation,
                            Operation::StructuralByteSequenceFieldByteStore { .. }
                        )
                    })
                    .map(|store| (block, store))
            })
            .unwrap();
        let Operation::StructuralByteSequenceFieldByteStore {
            destination, field, ..
        } = function.blocks[block].nodes[store].operation.clone()
        else {
            unreachable!()
        };
        let replacement = function.blocks[block]
            .nodes
            .iter()
            .position(|node| {
                matches!(&node.operation,
                    Operation::StructuralByteSequenceFieldStore { field: written, .. }
                        if *written == field)
            })
            .unwrap();
        let Operation::StructuralByteSequenceFieldStore { source, .. } =
            function.blocks[block].nodes[replacement].operation
        else {
            unreachable!()
        };
        let accepted = |changed: &PsiOptimizationFunction| {
            validate_indexed_byte_stores(&unit, changed, &types, &predecessors).is_ok()
        };

        // Valid UTF-8 of the same extent that is not ASCII.
        let mut non_ascii = function.clone();
        for node in &mut non_ascii.blocks[block].nodes {
            if let Operation::EstablishByteSequenceLiteral { place, bytes, .. } =
                &mut node.operation
                && place.id == source
            {
                *bytes = "\u{e9}X".as_bytes().to_vec();
            }
        }
        assert!(!accepted(&non_ascii));

        // Without the replacement, only the declaration holds at entry.
        let mut unestablished = function.clone();
        unestablished.blocks[block].nodes.remove(replacement);
        assert!(!accepted(&unestablished));

        // Another store into the identical field keeps the class.
        let mut chained = function.clone();
        let repeated = chained.blocks[block].nodes[store].clone();
        chained.blocks[block].nodes.insert(store, repeated);
        assert!(accepted(&chained));

        // A call that may write the root reopens the question; a shared
        // loan does not.
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ] {
            let mut called = function.clone();
            let mut call = called.blocks[block].nodes[store].clone();
            call.operation = Operation::CallStructural {
                psi_operation: OperationId::new(991).unwrap(),
                callee: function.machine,
                arguments: Vec::new(),
                structural_arguments: vec![terminal_psi::StructuralArgument {
                    place: destination,
                    access,
                    path: Vec::new(),
                }],
                dynamic_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                result: AbstractStructuralCallResult::Unit,
            };
            called.blocks[block].nodes.insert(store, call);
            assert_eq!(
                accepted(&called),
                access == StructuralAccess::SharedBorrow,
                "{access:?}"
            );
        }
    }
}
