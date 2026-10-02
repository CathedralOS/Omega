//! All-path operation-point custody, including length-changing backedges.

use super::{
    ModuleError, OperationKind, PlaceId, StructuralAccess, StructuralFieldId,
    StructuralPathSegment, StructuralTypeShape, TerminalMachine, TerminalModule, byte_count_type,
    field_path,
};
use crate::validation::{BTreeSet, OperationId, Proposition, ScalarTerm};
fn operation_position(machine: &TerminalMachine, identity: OperationId) -> Option<(usize, usize)> {
    machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, declaration)| {
            declaration
                .operations
                .iter()
                .position(|operation| operation.id == identity)
                .map(|position| (block, position))
        })
}

/// Every operation of the machine, with the name its own block gives the
/// storage that `root` names at `use_site`. An operation in a block the
/// storage never reaches is absent: no place there denotes it.
fn operations_naming<'machine>(
    machine: &'machine TerminalMachine,
    use_site: OperationId,
    root: PlaceId,
) -> impl Iterator<Item = (PlaceId, &'machine terminal_psi::Operation)> {
    let names = operation_position(machine, use_site)
        .map(|(block, _)| machine.blocks[block].id)
        .map(|block| terminal_semantics::storage_names(machine, block, root));
    machine.blocks.iter().flat_map(move |block| {
        let block_names: Vec<_> = names
            .as_ref()
            .map(|names| names.in_block(block.id).collect())
            .unwrap_or_default();
        block.operations.iter().flat_map(move |operation| {
            block_names
                .clone()
                .into_iter()
                .map(move |name| (name, operation))
        })
    })
}

/// Every finite execution reaching `use_site` must have passed `producer`
/// without an invalidating operation. Visiting a graph point twice adds no
/// additional paths; mutations on cyclic paths are still examined in full.
/// Reaching the operation is insufficient when another arrival supplies
/// different storage: its translated root must match the producer too.
fn unchanged_since(
    machine: &TerminalMachine,
    producer: OperationId,
    producer_root: PlaceId,
    use_site: OperationId,
    root: PlaceId,
    invalidates: impl Fn(&terminal_psi::Operation, PlaceId) -> bool,
) -> bool {
    operation_position(machine, producer).is_some()
        && established_on_every_path(
            machine,
            use_site,
            root,
            |candidate, root| candidate.id == producer && root == producer_root,
            invalidates,
        )
}

/// Every finite execution reaching `use_site` must have passed some
/// operation that `establishes` the fact, with no `invalidates` operation
/// after the last such establishment. The walk runs backward from the use
/// and stops each path at its nearest establishment, so a fact may come from
/// different establishments on different arrivals. Reaching the machine
/// entry, or a point with no predecessor, first fails: a machine parameter's
/// entry state establishes nothing here.
fn established_on_every_path(
    machine: &TerminalMachine,
    use_site: OperationId,
    root: PlaceId,
    establishes: impl Fn(&terminal_psi::Operation, PlaceId) -> bool,
    invalidates: impl Fn(&terminal_psi::Operation, PlaceId) -> bool,
) -> bool {
    let Some((use_block, use_position)) = operation_position(machine, use_site) else {
        return false;
    };
    let outgoing = crate::control_graph::successors(machine);
    let mut pending = vec![(use_block, use_position, root)];
    let mut visited = BTreeSet::new();
    let mut established = false;
    while let Some((block_index, position, root)) = pending.pop() {
        if !visited.insert((block_index, position, root)) {
            continue;
        }
        let block = &machine.blocks[block_index];
        if position != 0 {
            let previous = &block.operations[position - 1];
            if establishes(previous, root) {
                established = true;
            } else {
                if invalidates(previous, root) {
                    return false;
                }
                pending.push((block_index, position - 1, root));
            }
        } else {
            if block.id == machine.entry {
                return false;
            }
            let mut has_predecessor = false;
            for (predecessor_index, predecessor) in machine.blocks.iter().enumerate() {
                if outgoing[&predecessor.id]
                    .iter()
                    .any(|(_, target)| *target == block.id)
                {
                    has_predecessor = true;
                    // The predecessor names this storage by whatever it bound
                    // on the edge. An edge that forwards only part of it, or
                    // binds payload fields instead, leaves the fact unproven.
                    let Some(before) =
                        terminal_semantics::root_before_edge(machine, block.id, predecessor, root)
                    else {
                        return false;
                    };
                    pending.push((predecessor_index, predecessor.operations.len(), before));
                }
            }
            if !has_predecessor {
                return false;
            }
        }
    }
    established
}

fn overlaps(left: &[StructuralPathSegment], right: &[StructuralPathSegment]) -> bool {
    terminal_semantics::structural_paths_may_overlap(left, right)
}

/// A descriptor is a spelling of backing storage, not an independent owner.
/// Retain its exact establishment prefix when checking a call's mutation;
/// a narrowed view with an unknown offset conservatively names its parent's
/// whole backing. Unresolved aliases cannot establish that a snapshot survived.
fn call_argument_changes(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    observed: &[StructuralPathSegment],
    argument: &terminal_psi::StructuralArgument,
) -> bool {
    let mut place = argument.place;
    let mut region = argument.path.clone();
    let mut visited = BTreeSet::new();
    loop {
        if region
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::Referent))
        {
            return true;
        }
        if place == root {
            return terminal_semantics::structural_paths_may_overlap(observed, &region);
        }
        if !visited.insert(place) {
            return true;
        }
        // A different borrowed block name may denote this same storage. Its
        // declaration alone cannot prove disjointness from the observation.
        if crate::validation::block_views::parameter(machine, place).is_some()
            || crate::validation::block_views::parameter(machine, root).is_some()
        {
            return true;
        }
        let Some(declaration) = machine.structural_places.iter().find(|row| row.id == place) else {
            return true;
        };
        let semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = declaration.kind
        else {
            // Distinct entry parameters and literal homes have independently
            // checked separation; they cannot be aliases of a fresh local.
            return false;
        };
        let Some(operation) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)
        else {
            return true;
        };
        match &operation.kind {
            OperationKind::EstablishElementView { source, .. }
            | OperationKind::EstablishMutableElementView { source, .. } => {
                if crate::validation::element_view::view_loan_result(machine, place).is_none() {
                    return true;
                }
                let mut backing = source.path.clone();
                if let Some(range) = backing.iter().position(|segment| {
                    matches!(segment, StructuralPathSegment::FixedByteRange { .. })
                }) {
                    // View-relative indices do not share a range's origin.
                    // Without an exact offset join, keep the containing array.
                    backing.truncate(range);
                } else {
                    backing.extend(region);
                }
                region = backing;
                place = source.place;
            }
            OperationKind::ElementViewSubslice { source, .. }
            | OperationKind::ByteSequenceSubslice { source, .. } => {
                region.clear();
                place = *source;
            }
            _ => {
                return module
                    .structural_types
                    .iter()
                    .find(|row| row.id == structural_type)
                    .is_none_or(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::Reference { .. }
                                | StructuralTypeShape::ElementView { .. }
                                | StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                )
                        )
                    });
            }
        }
    }
}

fn changes_length(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[StructuralPathSegment],
    operation: &terminal_psi::Operation,
) -> bool {
    match &operation.kind {
        OperationKind::EstablishByteSequenceConcatenation { .. } => operation
            .result
            .structural()
            .is_some_and(|result| result.place == root),
        OperationKind::ByteSequenceStore { destination, .. } => *destination == root,
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path: written,
            field,
            ..
        } => {
            *destination == root
                && field_path(module, machine, *destination, written, *field, true, false)
                    .is_none_or(|written| overlaps(path, &written))
        }
        // A descriptor replacement installs another view, whose extent is the
        // source's: no earlier observation of the replaced field stays current.
        OperationKind::StructuralByteViewFieldStore {
            destination,
            path: written,
            field,
            ..
        } => {
            *destination == root
                && stored_field_identity(module, machine, root, written, field).is_none_or(
                    |identity| {
                        let mut written = written.clone();
                        written.push(StructuralPathSegment::Field(identity));
                        overlaps(path, &written)
                    },
                )
        }
        // A whole-field move or reseat replaces the stored view descriptor an
        // element-view extent observation measured, so the observation is not
        // current across it. A root window replaces the whole referent, which
        // contains every observed field.
        OperationKind::MoveStructuralField {
            source: window,
            path: windowed,
            field,
        }
        | OperationKind::StoreStructuralField {
            destination: window,
            path: windowed,
            field,
            ..
        } => {
            *window == root
                && match field {
                    None => true,
                    Some(field) => stored_field_identity(module, machine, root, windowed, field)
                        .is_none_or(|identity| {
                            let mut windowed = windowed.clone();
                            windowed.push(StructuralPathSegment::Field(identity));
                            overlaps(path, &windowed)
                        }),
                }
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            path: written,
            ..
        } => {
            // A primitive-array sibling write cannot change this field's
            // extent. Prefixes and possibly aliasing runtime indices still
            // overlap under the shared projection rule.
            *destination == root && terminal_semantics::structural_paths_may_overlap(path, written)
        }
        OperationKind::AtomicAccess { place, event, .. } => {
            event.modifies_resident() && *place == root
        }
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
            matches!(
                argument.access,
                StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
                    | StructuralAccess::Owned
            ) && call_argument_changes(module, machine, root, path, argument)
        }),
        // Dynamic dispatch retains its receiver in a separate catalog. Until
        // that exact footprint is resolved here it cannot preserve this fact.
        OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::StoreDynamicDescriptor { .. } => true,
        _ => false,
    }
}

/// Resolve the name of a stored record field so a whole-field move or store
/// can be compared against an observation's exact path.
fn stored_field_identity(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[StructuralPathSegment],
    field: &StructuralFieldId,
) -> Option<String> {
    // Owned homes and state-local loans can also open and repair a nested
    // window. Closing its debt does not revive an earlier length observation.
    let (structural_type, _) = terminal_semantics::byte_field_root(machine, root)?;
    let carrier =
        super::super::runtime_indexes::checked_projection_type(module, structural_type, path)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    fields
        .iter()
        .find(|candidate| candidate.id == *field)
        .map(|candidate| candidate.identity.clone())
}

pub(in crate::validation) fn exact_length_is_current(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> bool {
    let (root, path, field, length, writing) = match &operation.kind {
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            length,
            ..
        } => (*destination, path, *field, *length, true),
        OperationKind::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            length,
            ..
        } => (*source, path, *field, *length, false),
        _ => return false,
    };
    let Some(exact_path) = field_path(module, machine, root, path, field, writing, false) else {
        return false;
    };
    let Some((producer_root, producer)) = operations_naming(machine, operation.id, root).find(|(name, candidate)| {
        candidate.result.scalar().is_some_and(|result| result.id == length)
            && matches!(&candidate.kind, OperationKind::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field }
                if source == name && terminal_semantics::structural_paths_select_same_location(measured_path, path) && *measured_field == field)
    }) else { return false; };
    unchanged_since(
        machine,
        producer.id,
        producer_root,
        operation.id,
        root,
        |candidate, root| changes_length(module, machine, root, &exact_path, candidate),
    )
}

/// Whether an indexed store's exact destination field holds only ASCII bytes
/// on every path into the store. Each path must reach a whole replacement of
/// the identical root, path and field whose source predicates -- replayed
/// from literal octets, qualified formals and concatenation inputs, never a
/// producer's roster -- include `ascii_only`. Between that replacement and
/// the store, byte stores into the identical field keep the class: this
/// check runs only for a predicate-bearing field, and every store into it
/// proves its stored byte ASCII in its own goal. Any other operation that
/// could change the field's extent or contents rejects, including a
/// replacement from a source not proved ASCII, a projected move, an
/// overlapping mutable or write-only call argument, and dynamic dispatch.
pub(crate) fn field_content_is_ascii(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> bool {
    let OperationKind::StructuralByteSequenceFieldByteStore {
        destination: root,
        path,
        field,
        ..
    } = &operation.kind
    else {
        return false;
    };
    let Some(exact_path) = field_path(module, machine, *root, path, *field, true, false) else {
        return false;
    };
    let mut ascii = terminal_psi::ByteSequencePredicateSet::EMPTY;
    ascii.insert(terminal_psi::ByteSequencePredicate::AsciiOnly);
    established_on_every_path(
        machine,
        operation.id,
        *root,
        |candidate, root| {
            matches!(&candidate.kind,
                OperationKind::StructuralByteSequenceFieldStore {
                    destination, path: written, field: written_field, source, ..
                } if *destination == root && written == path && written_field == field
                    && terminal_semantics::byte_sequence_source_predicates(module, machine, *source)
                        .is_some_and(|predicates| predicates.implies(ascii)))
        },
        |candidate, root| match &candidate.kind {
            OperationKind::StructuralByteSequenceFieldByteStore {
                destination,
                path: written,
                field: written_field,
                ..
            } if *destination == root => {
                // A store into the identical field is class-preserving; a
                // store into any other field is disjoint unless its path
                // cannot be resolved.
                !(written == path && written_field == field)
                    && field_path(module, machine, root, written, *written_field, true, false)
                        .is_none_or(|written| overlaps(&exact_path, &written))
            }
            _ => changes_length(module, machine, root, &exact_path, candidate),
        },
    )
}

/// A mutable owned result remains the exact activation-local establishment.
/// A byte write preserves extent; an overlapping replacement or call does not
/// supply authority to keep using a former establishment's metadata.
pub(crate) fn owned_content_is_current(
    module: &TerminalModule,
    machine: &TerminalMachine,
    use_site: OperationId,
    root: PlaceId,
) -> bool {
    established_on_every_path(
        machine,
        use_site,
        root,
        |candidate, root| {
            matches!(
                candidate.kind,
                OperationKind::EstablishByteSequenceConcatenation { .. }
            ) && candidate
                .result
                .structural()
                .is_some_and(|result| result.place == root)
        },
        |candidate, root| changes_length(module, machine, root, &[], candidate),
    )
}

/// UTF-8 mutation needs an all-ASCII backing at this occurrence, not merely an
/// ASCII replacement byte. The exact concatenation supplies that fact; every
/// intervening write to this UTF-8 owner proves its own ASCII value separately.
pub(crate) fn owned_content_is_ascii(
    module: &TerminalModule,
    machine: &TerminalMachine,
    use_site: OperationId,
    root: PlaceId,
) -> bool {
    let mut ascii = terminal_psi::ByteSequencePredicateSet::EMPTY;
    ascii.insert(terminal_psi::ByteSequencePredicate::AsciiOnly);
    established_on_every_path(
        machine,
        use_site,
        root,
        |candidate, root| {
            candidate
                .result
                .structural()
                .is_some_and(|result| result.place == root)
                && terminal_semantics::byte_sequence_concatenation_predicates(
                    module, machine, candidate,
                )
                .is_some_and(|predicates| predicates.implies(ascii))
        },
        |candidate, root| changes_length(module, machine, root, &[], candidate),
    )
}

/// A concatenation captures content as well as extent. A byte write preserves
/// length but changes the value being read, so it also invalidates this interval.
pub(crate) fn field_content_length_is_current(
    module: &TerminalModule,
    machine: &TerminalMachine,
    use_site: OperationId,
    root: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    length: semantic_vocabulary::ValueId,
) -> bool {
    let Some(exact_path) = field_path(module, machine, root, path, field, false, false) else {
        return false;
    };
    let Some((producer_root, producer)) = operations_naming(machine, use_site, root).find(|(name, candidate)| {
        candidate.result.scalar().is_some_and(|result| result.id == length)
            && matches!(&candidate.kind, OperationKind::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field }
                if source == name && terminal_semantics::structural_paths_select_same_location(measured_path, path) && *measured_field == field)
    }) else { return false; };
    unchanged_since(
        machine,
        producer.id,
        producer_root,
        use_site,
        root,
        |candidate, root| changes_content(module, machine, root, &exact_path, candidate),
    )
}

fn changes_content(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[StructuralPathSegment],
    operation: &terminal_psi::Operation,
) -> bool {
    changes_length(module, machine, root, path, operation)
        || matches!(&operation.kind,
            OperationKind::StructuralByteSequenceFieldByteStore {
                destination, path: written, field: written_field, ..
            } if *destination == root
                && field_path(module, machine, root, written, *written_field, true, false)
                    .is_none_or(|written| overlaps(path, &written)))
}

/// Capture the range of a runtime-selected byte from still-current literal
/// contents. The index's independent obligation proves that the selected byte
/// exists; these facts only bound its value. Unlike the ASCII carrier rule,
/// even an ASCII-preserving byte write invalidates an exact literal range.
/// The facts name the immutable read result, not mutable field storage.
pub(crate) fn literal_field_read_bounds(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<[Proposition; 2]> {
    let OperationKind::StructuralByteSequenceFieldRead {
        source: root,
        path,
        field,
        ..
    } = &operation.kind
    else {
        return None;
    };
    let result = operation.result.scalar_ref()?;
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .ok()?;
    if result.scalar_type != semantic_vocabulary::ScalarType::Integer(integer) {
        return None;
    }
    let exact_path = field_path(module, machine, *root, path, *field, false, false)?;
    for (name, replacement) in operations_naming(machine, operation.id, *root) {
        let OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path: written,
            field: written_field,
            source,
            ..
        } = &replacement.kind
        else {
            continue;
        };
        if *destination != name
            || written != path
            || written_field != field
            || !unchanged_since(
                machine,
                replacement.id,
                name,
                operation.id,
                *root,
                |candidate, root| changes_content(module, machine, root, &exact_path, candidate),
            )
            || !machine.structural_places.iter().any(|place| {
                place.id == *source
                    && matches!(
                        place.kind,
                        semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { .. }
                    )
            })
        {
            continue;
        }
        let mut literals = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|candidate| match &candidate.kind {
                OperationKind::EstablishByteSequenceLiteral {
                    destination, bytes, ..
                } if destination == source => Some((candidate.id, bytes)),
                _ => None,
            });
        let Some((producer, bytes)) = literals.next() else {
            continue;
        };
        if literals.next().is_some()
            || bytes.is_empty()
            || !unchanged_since(
                machine,
                producer,
                *source,
                replacement.id,
                *source,
                |_, _| false,
            )
        {
            continue;
        }
        let minimum = *bytes.iter().min()?;
        let maximum = *bytes.iter().max()?;
        let value = ScalarTerm::value(result.id, result.scalar_type);
        return Some([
            Proposition::LessOrEqual(
                ScalarTerm::integer(
                    integer,
                    semantic_vocabulary::IntegerValue::Unsigned(minimum.into()),
                )
                .ok()?,
                value.clone(),
            ),
            Proposition::LessOrEqual(
                value,
                ScalarTerm::integer(
                    integer,
                    semantic_vocabulary::IntegerValue::Unsigned(maximum.into()),
                )
                .ok()?,
            ),
        ]);
    }
    None
}

pub(crate) fn replacement_length_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<Option<Proposition>, ModuleError> {
    let OperationKind::StructuralByteSequenceFieldLength {
        source: root,
        path,
        field,
    } = &operation.kind
    else {
        return Ok(None);
    };
    let Some(exact_path) = field_path(module, machine, *root, path, *field, false, true) else {
        return Ok(None);
    };
    let result = operation.result.expect_scalar().id;
    for (name, replacement) in operations_naming(machine, operation.id, *root) {
        let OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path: written_path,
            field: written_field,
            source,
            length,
            ..
        } = &replacement.kind
        else {
            continue;
        };
        if *destination != name
            || written_path != path
            || written_field != field
            || !unchanged_since(
                machine,
                replacement.id,
                name,
                operation.id,
                *root,
                |candidate, root| changes_length(module, machine, root, &exact_path, candidate),
            )
        {
            continue;
        }
        let scalar_type = byte_count_type();
        let value = ScalarTerm::value(result, scalar_type);
        // Literal identity never changes on reentry. Derive the field count
        // directly, without importing first-iteration scalar assumptions.
        if let Some(literal) = machine.blocks.iter().flat_map(|block| &block.operations).find(|candidate| matches!(&candidate.kind,
            OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == source)) {
            let observation = terminal_semantics::StructuralEffectObservation::ByteSequenceLengthRead { source: *source, result };
            return terminal_semantics::literal_length_equation(&observation, literal)
                .map_err(ModuleError::OperationSemanticSchema);
        }
        // A saved source length is a valid snapshot only while that SSA slot
        // has not been rebound along any path since the replacement.
        if unchanged_since(
            machine,
            replacement.id,
            name,
            operation.id,
            *root,
            |candidate, _| {
                candidate
                    .result
                    .scalar()
                    .is_some_and(|result| result.id == *length)
            },
        ) {
            return Ok(Some(Proposition::Equal(
                value,
                ScalarTerm::value(*length, scalar_type),
            )));
        }
    }
    // A fresh observation of a field whose extent an earlier observation
    // already measured carries the same value while every path between them
    // preserves the extent — `unchanged_since` enforces dominance including
    // backedges, so a guard's `i < self.view.len` discharges a later store's
    // index obligation bound against the successor's own observation.
    for (name, observation) in operations_naming(machine, operation.id, *root) {
        let OperationKind::StructuralByteSequenceFieldLength {
            source,
            path: observed_path,
            field: observed_field,
        } = &observation.kind
        else {
            continue;
        };
        if observation.id == operation.id
            || *source != name
            || observed_path != path
            || observed_field != field
            || !unchanged_since(
                machine,
                observation.id,
                name,
                operation.id,
                *root,
                |candidate, root| changes_length(module, machine, root, &exact_path, candidate),
            )
        {
            continue;
        }
        let Some(observed) = observation.result.scalar() else {
            continue;
        };
        return Ok(Some(Proposition::Equal(
            ScalarTerm::value(result, byte_count_type()),
            ScalarTerm::value(observed.id, byte_count_type()),
        )));
    }
    Ok(None)
}

/// Whole mutable-view writes retain their exact observation across every path.
/// Calls that may change the original referent invalidate an earlier extent.
pub(crate) fn view_length_is_current(
    module: &TerminalModule,
    machine: &TerminalMachine,
    producer: OperationId,
    operation: OperationId,
    destination: PlaceId,
) -> bool {
    // Callers establish descriptor identity separately, sometimes checking
    // every intermediate alias against the same producer. This is only their
    // ordering and mutation fence; it grants no equality between storage names.
    established_on_every_path(
        machine,
        operation,
        destination,
        |candidate, _| candidate.id == producer,
        |candidate, root| changes_length(module, machine, root, &[], candidate),
    )
}
