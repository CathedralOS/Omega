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
    StructuralTypeShape, live_field_by_id,
};

/// Indexed writes preserve the same declared invariants through a caller loan
/// as through owned local backing. Do not use incidental initializer predicates:
/// a previous write can have invalidated those without changing the contract.
pub(crate) fn byte_write_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<ByteSequencePredicateSet> {
    if let Some(parameter) = function.arrival_parameter(place) {
        terminal_semantics::byte_parameter_predicates_in(unit.structural_domains.iter(), parameter)
    } else {
        owned_byte_predicates(unit, function, place)
    }
}

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
            if let Operation::WriteOnlyPrimitiveStore {
                destination, path, ..
            } = &node.operation
            {
                if super::fixed_byte_freshness::requires_ascii(unit, destination, path)
                    == Some(true)
                    && !super::fixed_byte_freshness::holds(
                        unit,
                        function,
                        types,
                        predecessors,
                        (block_position, node_position),
                        destination.place(),
                        &path[..path.len() - 1],
                        function.structural_places.len() + 1,
                    )
                {
                    return Err(OptimizationUnitValidationError::InvalidByteSequenceWrite {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_position).unwrap_or(u32::MAX),
                    });
                }
                continue;
            }
            if let Operation::ByteSequenceWrite { destination, .. } = &node.operation {
                if let Some(predicates) = byte_write_predicates(unit, function, *destination) {
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
    let declaration = function.structural_place(place)?;
    let (producer, structural_type) = declaration.kind.as_operation_result()?;
    let result = function
        .block_nodes()
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
            declaration.id == structural_type && declaration.is_bounded_owned_byte_sequence()
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
            descriptor_source_predicates(
                &Catalogs {
                    types: unit.structural_types.iter(),
                    domains: unit.structural_domains.iter(),
                },
                function,
                source,
                function.structural_places.len().saturating_add(1),
                &mut Vec::new(),
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

/// Declaration predicates captured by one byte-field snapshot at the owned
/// transfer that consumes it: an ordinary call argument or a constructor
/// operand. Only that consumer is excluded from the conservative write scan;
/// this is not a timeless initializer fact or a permission to borrow the field.
/// Current-graph custody independently checks availability and dominance.
pub(crate) fn copied_field_predicates_at_transfer<'t>(
    types: impl Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
    function: &PsiOptimizationFunction,
    source: PlaceId,
    consumer: semantic_vocabulary::OperationId,
) -> Option<ByteSequencePredicateSet> {
    let declaration = function.structural_place(source)?;
    let (producer, structural_type) = declaration.kind.as_operation_result()?;
    let node = function
        .block_nodes()
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
        .chain(function.block_structural_parameters())
        .find(|parameter| parameter.place == *backing)?;
    if !matches!(
        parameter.access,
        StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
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
    let Some(fields) = parent.shape.record_fields() else {
        return None;
    };
    let field = terminal_psi::live_field_by_identity(fields, *identity)?;
    if field.field_type.byte_sequence_carrier()? != carrier {
        return None;
    }
    let consuming = function
        .block_nodes()
        .find(|node| node.operation.psi_operation() == Some(consumer))?;
    // Whole constructors and ordinary calls consume the same completed value.
    // The source occurrence, not the destination's requested predicates,
    // selects which pristine snapshot may supply its declared field invariants.
    let call_arguments = match &consuming.operation {
        Operation::CallStructural {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        _ => &[],
    };
    let mut occurrences = call_arguments
        .iter()
        .chain(crate::current_ownership::constructor_arguments(
            &consuming.operation,
        ))
        .filter(|argument| argument.place == source)
        .peekable();
    if occurrences.peek().is_none()
        || occurrences
            .any(|argument| !argument.path.is_empty() || argument.access != StructuralAccess::Owned)
    {
        return None;
    }
    let carries = |bindings: &[abstract_operations::AbstractStructuralBinding]| {
        bindings
            .iter()
            .any(|binding| binding.argument.place == source)
    };
    if function.block_nodes().any(|node| {
        let operation = &node.operation;
        // An edge can rename the snapshot; a place-only clobber scan must not
        // infer that such an alias stays pristine. Backing-field writes are
        // independent because this operation already captured its own bytes.
        let escapes = match operation {
            Operation::Jump { structural_bindings, .. } => carries(structural_bindings),
            Operation::Conditional { when_true, when_false, .. } => carries(&when_true.structural_bindings) || carries(&when_false.structural_bindings),
            Operation::ReturnStructural { source: returned, .. } => *returned == source,
            _ => false
        };
        escapes || (operation.psi_operation() != Some(consumer)
            && (may_change_owned_source(operation, source)
                || matches!(operation, Operation::EstablishReference { source: argument, .. } | Operation::EstablishElementView { source: argument, .. } if argument.place == source)
                || matches!(operation, Operation::ByteSequenceSubslice { source: viewed, .. } if viewed.place == source)))
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
        super::super::references::record_field_destination(
            function,
            &catalog,
            root,
            path,
            terminal_psi::StructuralAccess::MutableBorrow,
        )?
    } else {
        (structural_type, path)
    };
    let declaration =
        terminal_semantics::runtime_structural_path_tip(types, structural_type, path)?;
    let Some(fields) = declaration.shape.record_fields() else {
        return None;
    };
    let field = live_field_by_id(fields, field)?;
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

/// A projected whole operand names the selected descriptor, never its root's
/// unrelated predicate roster. Reference crossings retain only the predicates
/// declared on the selected field; a plain referent contributes no refinement.
fn descriptor_source_predicates<'t, 'd, Types, Domains>(
    catalogs: &Catalogs<Types, Domains>,
    function: &PsiOptimizationFunction,
    source: &terminal_psi::StructuralArgument,
    remaining: usize,
    memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
) -> Option<ByteSequencePredicateSet>
where
    Types: Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
    Domains: Iterator<Item = &'d terminal_psi::StructuralDomainDeclaration> + Clone,
{
    if source.path.is_empty() {
        return source_predicates(catalogs, function, source.place, remaining, memo);
    }
    let (root, access) = super::byte_field_freshness::byte_field_root(function, source.place)?;
    if access == StructuralAccess::WriteOnlyBorrow {
        return None;
    }
    let selected = terminal_semantics::runtime_structural_path_tip(
        catalogs.types.clone(),
        root,
        &source.path,
    )?;
    if !matches!(
        selected.shape,
        terminal_psi::StructuralTypeShape::ByteSequence(_)
    ) {
        return None;
    }
    Some(
        terminal_semantics::runtime_structural_field(catalogs.types.clone(), root, &source.path)
            .map_or(ByteSequencePredicateSet::EMPTY, |field| {
                field.field_type.byte_predicates()
            }),
    )
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
    let declaration = function.structural_place(source)?;
    match declaration.kind {
        StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. } => {
            let parameter = function.arrival_parameter(source)?;
            if !matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
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
                .block_nodes()
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
            Some(ByteSequencePredicateSet::holding(bytes))
        }
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let node = function
                .block_nodes()
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
                            && declaration.shape.is_shared_readable_byte_view()
                    })
                {
                    return None;
                }
                let captured = if backing.path.is_empty() {
                    source_predicates(catalogs, function, backing.place, remaining, memo)?
                } else {
                    let (root, access) =
                        super::byte_field_freshness::byte_field_root(function, backing.place)?;
                    if access == StructuralAccess::WriteOnlyBorrow {
                        return None;
                    }
                    terminal_semantics::shared_byte_field_copy_predicates(
                        catalogs.types.clone(),
                        root,
                        &backing.path,
                    )?
                };
                return Some(captured.subslice());
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
                        && declaration.shape.is_owned_or_shared_byte_carrier()
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
                .block_nodes()
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
                        descriptor_source_predicates(catalogs, function, source, remaining, memo)
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
