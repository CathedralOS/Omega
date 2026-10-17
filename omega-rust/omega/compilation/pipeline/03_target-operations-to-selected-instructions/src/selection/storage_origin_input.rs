//! Physical backing alternatives for descriptor payloads and thin scalar exports.
//! Descriptor payload homes do not allocate their backing. Reconstruct the
//! completed calls reached by a payload access, then retain an opaque read of
//! those calls' authenticated origin rosters. Constructor fields, copies and
//! incoming bindings transport descriptor coordinates, not backing ownership.
//! Replacement sources are a conservative union, never a live-value replay.
//! Callee-local coordinates cannot bound this reach in the caller; unresolved
//! borrowed backing remains unsupported rather than becoming a local allocation.
//! Calls retain both top-level descriptor origins and the target producer's
//! reference-leaf origins. Their conservative union describes memory reach,
//! never the runtime pointer selected from an aggregate's returned payload.
use crate::SelectedInstructionError;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind as I, LegalizedScalarTerminator as T,
};
use selected_instructions::{
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole as R,
};
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};

/// Successful geometry selections belong to this immutable function's type
/// catalog. Keep full paths (including runtime selector identities); access
/// reachability and producer inventories remain separate for each read.
struct SourceGeometry<'a> {
    function: &'a LegalizedScalarFunction,
    selected_types: BTreeMap<
        (
            semantic_vocabulary::StructuralTypeId,
            Vec<terminal_psi::StructuralPathSegment>,
        ),
        semantic_vocabulary::StructuralTypeId,
    >,
}

/// Construction scratch for the coordinate walk. Recurrent bindings are
/// dependencies, not evidence that a descriptor has real backing. Every
/// discovered coordinate must reach an authenticated leaf after the walk.
#[derive(Default)]
struct OriginInventory {
    coordinates: Vec<terminal_psi::StructuralArgument>,
    dependencies: Vec<(usize, usize)>,
    active: Vec<usize>,
    origins: Vec<usize>,
    /// Terminal backing coordinates, excluding opaque completed-call homes.
    storage_origins: Vec<usize>,
    /// An inexact call origin cannot establish a scalar storage footprint.
    unknown_storage: bool,
}

impl OriginInventory {
    fn enter(&mut self, coordinate: &terminal_psi::StructuralArgument) -> bool {
        let previous = self.coordinates.iter().position(|previous| {
            terminal_semantics::same_descriptor_coordinate(previous, coordinate)
        });
        let index = previous.unwrap_or(self.coordinates.len());
        if let Some(parent) = self.active.last() {
            self.dependencies.push((*parent, index));
        }
        if previous.is_some() {
            return false;
        }
        self.coordinates.push(coordinate.clone());
        self.active.push(index);
        true
    }

    fn origin(&mut self) {
        if let Some(index) = self.active.last() {
            self.origins.push(*index);
            self.storage_origins.push(*index);
        }
    }

    fn returned_origin(&mut self, complete: bool) {
        if let Some(index) = self.active.last() {
            self.origins.push(*index);
        }
        self.unknown_storage |= !complete;
    }

    fn leave(&mut self) {
        self.active.pop();
    }

    fn validate(&self) -> Result<(), SelectedInstructionError> {
        let mut reached = vec![false; self.coordinates.len()];
        for index in &self.origins {
            reached[*index] = true;
        }
        loop {
            let mut changed = false;
            for (parent, child) in &self.dependencies {
                if reached[*child] && !reached[*parent] {
                    reached[*parent] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        if reached.iter().all(|reached| *reached) {
            Ok(())
        } else {
            Err(SelectedInstructionError::custody())
        }
    }
}

/// A thin foreign scalar pointer drops the descriptor codec. Follow the same
/// conservative storage-origin inventory used for memory reach, requiring every
/// terminal backing to have the requested width. Unknown descriptor ingress
/// remains unknown; its logical referent is not its physical storage contract.
pub(super) fn scalar_backing_width_matches(
    function: &LegalizedScalarFunction,
    coordinate: &terminal_psi::StructuralArgument,
    width: u32,
) -> Result<bool, SelectedInstructionError> {
    let mut geometry = SourceGeometry {
        function,
        selected_types: BTreeMap::new(),
    };
    let mut inventory = OriginInventory::default();
    collect_calls(
        &mut geometry,
        coordinate,
        &mut inventory,
        &mut BTreeSet::new(),
    )?;
    inventory.validate()?;
    let signature = function
        .structural
        .as_ref()
        .ok_or(SelectedInstructionError::custody())?;
    if inventory.unknown_storage {
        return Ok(false);
    }
    for index in &inventory.storage_origins {
        let origin = &inventory.coordinates[*index];
        // A crossing in a terminal origin reaches external descriptor backing.
        // Its actual codec was not recovered by this inventory.
        if origin
            .path
            .contains(&terminal_psi::StructuralPathSegment::Referent)
            || super::reference_input::borrowed_scalar_source(function, origin.place).is_some()
        {
            return Ok(false);
        }
        let root = super::reference_input::root_type(function, origin.place)
            .ok_or(SelectedInstructionError::custody())?;
        let codec = crate::structural_inputs::structural_reference_input::projected_scalar_codec(
            root,
            &origin.path,
            &signature.structural_types,
        );
        if codec.map(|codec| u32::from(codec.byte_size())) != Some(width) {
            return Ok(false);
        }
    }
    Ok(!inventory.storage_origins.is_empty())
}

pub(super) fn replay_memory_origins(
    source: &LegalizedScalarFunction,
    accesses: &mut Vec<SelectedMemoryAccess>,
) -> Result<(), SelectedInstructionError> {
    let mut geometry = SourceGeometry {
        function: source,
        selected_types: BTreeMap::new(),
    };
    let mut replayed = Vec::with_capacity(accesses.len());
    for access in accesses.iter() {
        if !matches!(
            access.role,
            R::ReadByteSequence { .. }
                | R::ReadByteSpan { .. }
                | R::ReadElementView { .. }
                | R::ReadIndexedPrimitive { .. }
        ) {
            replayed.push(access.clone());
            continue;
        }
        let mut producers = BTreeSet::new();
        let mut visited = OriginInventory::default();
        for coordinate in access_coordinates(&mut geometry, access)? {
            collect_calls(&mut geometry, &coordinate, &mut visited, &mut producers)?;
        }
        visited.validate()?;
        if producers.is_empty() {
            replayed.push(access.clone());
        } else {
            // The original read's bounds still validate its physical operation.
            // Its access row must not turn descriptor identity into a separate
            // backing allocation or reuse the shortened view extent at root 0.
            for (producer, place) in producers {
                let mut row = access.clone();
                row.place = place;
                row.byte_offset = 0;
                row.byte_count = 0;
                row.role = R::ReadReturnedView { producer };
                replayed.push(row);
            }
        }
    }
    *accesses = replayed;
    Ok(())
}

/// Native footprints sometimes name a backing root rather than the descriptor
/// home. Start from the operation's selected operand where it is retained;
/// other footprint families conservatively retain the named root's reach.
fn access_coordinates(
    geometry: &mut SourceGeometry<'_>,
    access: &SelectedMemoryAccess,
) -> Result<Vec<terminal_psi::StructuralArgument>, SelectedInstructionError> {
    let function = geometry.function;
    use terminal_psi::{StructuralAccess, StructuralArgument};
    let whole = |place| StructuralArgument::whole(place, StructuralAccess::SharedBorrow);
    let SelectedMemoryAccessOrigin::Operation(operation) = access.origin else {
        return Ok(vec![whole(access.place)]);
    };
    let row = function
        .instructions()
        .find(|row| row.operation == operation)
        .ok_or(SelectedInstructionError::custody())?;
    let coordinates = match &row.kind {
        I::ByteSequenceRead { source, .. } => vec![whole(*source)],
        I::PrimitiveScalarRead { source, path, .. } => {
            let signature = function
                .structural
                .as_ref()
                .ok_or(SelectedInstructionError::custody())?;
            let root = super::reference_input::root_type(function, *source)
                .ok_or(SelectedInstructionError::custody())?;
            let geometry =
                crate::structural_inputs::structural_reference_input::primitive_read_geometry(
                    root,
                    path,
                    row.result
                        .ok_or(SelectedInstructionError::custody())?
                        .scalar_type,
                    &signature.structural_types,
                )
                .ok_or(SelectedInstructionError::custody())?;
            // A scalar path staying in the original home reads owned bytes,
            // even if another field carries a returned loan. Only pointer
            // crossings or indexed byte descriptors need backing-origin reach;
            // the read's original footprint still bounds the inline access.
            if !geometry.uses_borrowed_backing {
                Vec::new()
            } else {
                vec![StructuralArgument {
                    place: *source,
                    path: path.clone(),
                    access: StructuralAccess::SharedBorrow,
                }]
            }
        }
        I::ElementViewRead { source, path, .. } => {
            vec![StructuralArgument {
                place: *source,
                path: path.clone(),
                access: StructuralAccess::SharedBorrow,
            }]
        }
        I::StructuralByteSequenceFieldRead { source, field, .. } => {
            vec![field_coordinate(geometry, source, Some(*field))?]
        }
        I::ByteSequenceLength { source, .. } | I::ElementViewLength { source, .. } => {
            vec![source.clone()]
        }
        I::ByteSequencePredicate { operand, .. } => vec![byte_coordinate(geometry, operand)?],
        I::ByteSequenceEqual { left, right }
        | I::EstablishByteSequenceConcatenation { left, right, .. } => {
            vec![
                byte_coordinate(geometry, left)?,
                byte_coordinate(geometry, right)?,
            ]
        }
        _ => vec![whole(access.place)],
    };
    Ok(coordinates)
}

fn byte_coordinate(
    geometry: &mut SourceGeometry<'_>,
    operand: &terminal_psi::control_flow::ByteSequenceOperand,
) -> Result<terminal_psi::StructuralArgument, SelectedInstructionError> {
    match operand {
        terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => Ok(source.clone()),
        terminal_psi::control_flow::ByteSequenceOperand::Field { source, field, .. } => {
            field_coordinate(geometry, source, Some(*field))
        }
    }
}

/// Field IDs are declaration-local. Join the containing path first, including
/// a selected payload namespace, before recovering its retained field identity.
fn field_coordinate(
    geometry: &mut SourceGeometry<'_>,
    source: &terminal_psi::StructuralArgument,
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Result<terminal_psi::StructuralArgument, SelectedInstructionError> {
    let function = geometry.function;
    let Some(field) = field else {
        return Ok(source.clone());
    };
    let signature = function
        .structural
        .as_ref()
        .ok_or(SelectedInstructionError::custody())?;
    let root = super::reference_input::root_type(function, source.place)
        .ok_or(SelectedInstructionError::custody())?;
    let (prefix, case) = match source.path.split_last() {
        Some((terminal_psi::StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
        _ => (source.path.as_slice(), None),
    };
    let key = (root, prefix.to_vec());
    let selected_type = match geometry.selected_types.get(&key) {
        Some(selected) => *selected,
        None => {
            // The first query still performs the complete geometry check.
            // Cache only success; case and field checks below run every time.
            let selected =
                crate::structural_inputs::structural_reference_input::reference_source_geometry(
                    root,
                    prefix,
                    &signature.structural_types,
                )
                .ok_or(SelectedInstructionError::custody())?
                .selected;
            geometry.selected_types.insert(key, selected);
            selected
        }
    };
    let shape = &signature
        .structural_type(selected_type)
        .ok_or(SelectedInstructionError::custody())?
        .shape;
    let fields = match case {
        Some(case) => shape
            .sum_cases()
            .and_then(|cases| cases.iter().find(|row| row.id == case))
            .map(|case| case.fields.as_slice()),
        None => shape.direct_fields(),
    }
    .ok_or(SelectedInstructionError::custody())?;
    let selected =
        terminal_psi::live_field_by_id(fields, field).ok_or(SelectedInstructionError::custody())?;
    let mut coordinate = source.clone();
    coordinate
        .path
        .push(terminal_psi::StructuralPathSegment::Field(
            selected.identity.clone(),
        ));
    Ok(coordinate)
}

/// Retain coordinates through value transport. This inventory is conservative
/// about possible origins and supplies no access, allocation or count bound.
fn collect_calls(
    geometry: &mut SourceGeometry<'_>,
    coordinate: &terminal_psi::StructuralArgument,
    visited: &mut OriginInventory,
    calls: &mut BTreeSet<(OperationId, PlaceId)>,
) -> Result<(), SelectedInstructionError> {
    let function = geometry.function;
    if !visited.enter(coordinate) {
        return Ok(());
    }
    let place = coordinate.place;
    let signature = function
        .structural
        .as_ref()
        .ok_or(SelectedInstructionError::custody())?;
    let declaration = signature
        .structural_place(place)
        .ok_or(SelectedInstructionError::custody())?;
    // Replacements are independently ordered by the live storage owner. Origin
    // accounting retains all overlapping possibilities, including the original
    // constructor value; it never reinterprets a future write as a live fact.
    let stored = stored_sources(geometry, coordinate, visited, calls)?;
    match declaration.kind {
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let row = function
                .instructions()
                .find(|row| row.operation == producer)
                .ok_or(SelectedInstructionError::custody())?;
            match &row.kind {
                I::Call(call) => {
                    let (result, _) = super::aggregate_result_input::call_result(function, call)
                        .ok_or(SelectedInstructionError::custody())?;
                    if result.place != place || result.structural_type != structural_type {
                        return Err(SelectedInstructionError::custody());
                    }
                    if call.descriptor_origins.is_empty() {
                        if contains_borrowed_storage(
                            &signature.structural_types,
                            structural_type,
                            &mut BTreeSet::new(),
                        ) {
                            return Err(SelectedInstructionError::custody());
                        }
                    } else {
                        calls.insert((producer, place));
                        // A selected descendant can contain another descriptor.
                        // Its backing comes from the retained actual-origin
                        // roster, not from the returned carrier's allocation.
                        if !coordinate.path.is_empty() {
                            for origin in &call.descriptor_origins {
                                if let legalized_operations::LegalizedDescriptorOrigin::Argument {
                                    source,
                                    ..
                                } = origin
                                {
                                    collect_calls(geometry, source, visited, calls)?;
                                }
                            }
                        }
                    }
                    visited.returned_origin(
                        !coordinate.path.is_empty()
                            && !call.descriptor_origins.is_empty()
                            && call.descriptor_origins.iter().all(|origin| {
                                matches!(
                                    origin,
                                    legalized_operations::LegalizedDescriptorOrigin::Argument {
                                        exact: true,
                                        ..
                                    }
                                )
                            }),
                    );
                }
                I::ByteSequenceSubslice {
                    source: parent,
                    indices,
                    ..
                }
                | I::ElementViewSubslice {
                    source: parent,
                    indices,
                    ..
                } => {
                    let root = super::reference_input::root_type(function, parent.place)
                        .ok_or(SelectedInstructionError::custody())?;
                    let descriptor_geometry = crate::structural_inputs::structural_reference_input::descriptor_source_geometry(
                        root, &parent.path, &signature.structural_types,
                    ).ok_or(SelectedInstructionError::custody())?;
                    if !descriptor_geometry.readable || !crate::structural_inputs::structural_reference_input::runtime_indices_match(indices, &descriptor_geometry.elements) {
                        return Err(SelectedInstructionError::custody());
                    }
                    // A result index is relative to the captured cut, not to
                    // its parent's backing. Retain the exact descriptor prefix
                    // and union its backing reach instead of reusing that index
                    // as an original collection selector.
                    collect_calls(geometry, parent, visited, calls)?;
                }
                I::StructuralLeafCopy {
                    source: parent,
                    path,
                    ..
                } => {
                    let prefix = match path {
                        abstract_operations::StructuralCopyPath::Ordinary(path) => path.clone(),
                        abstract_operations::StructuralCopyPath::Case(path) => {
                            let root = super::reference_input::root_type(function, *parent)
                                .ok_or(SelectedInstructionError::custody())?;
                            let selected = terminal_semantics::descriptor_lifetimes::canonical_descriptor_location(
                                &signature.structural_types, *parent, root, path,
                            ).ok_or(SelectedInstructionError::custody())?;
                            terminal_semantics::descriptor_lifetimes::descriptor_path(
                                &signature.structural_types,
                                root,
                                &selected.path,
                            )
                            .ok_or(SelectedInstructionError::custody())?
                        }
                    };
                    let parent = terminal_psi::StructuralArgument {
                        place: *parent,
                        path: prefix,
                        access: coordinate.access,
                    };
                    collect_calls(
                        geometry,
                        &rebased(&parent, &coordinate.path),
                        visited,
                        calls,
                    )?;
                }
                I::EstablishElementView { source: parent, .. } => {
                    // A view's selectors are in its backing coordinate. The
                    // source collection is retained conservatively when a cut
                    // cannot map an element back to one exact original index.
                    collect_calls(geometry, parent, visited, calls)?;
                }
                I::EstablishReference { source: parent, .. }
                | I::EstablishPlacedReference { source: parent, .. } => {
                    let suffix = match coordinate.path.split_first() {
                        Some((terminal_psi::StructuralPathSegment::Referent, suffix)) => suffix,
                        None => &[],
                        _ => return Err(SelectedInstructionError::custody()),
                    };
                    collect_calls(geometry, &rebased(parent, suffix), visited, calls)?;
                }
                I::EstablishRecord { fields, .. } => {
                    constructed_fields(
                        geometry,
                        structural_type,
                        None,
                        fields,
                        coordinate,
                        visited,
                        calls,
                    )?;
                }
                I::EstablishCase {
                    result_case,
                    fields,
                    ..
                } => {
                    constructed_fields(
                        geometry,
                        structural_type,
                        Some(*result_case),
                        fields,
                        coordinate,
                        visited,
                        calls,
                    )?;
                }
                I::EstablishStructuralArray { elements, .. } => {
                    if elements.is_empty() {
                        visited.origin();
                    }
                    match coordinate.path.split_first() {
                        None => {
                            for element in elements {
                                collect_calls(geometry, element, visited, calls)?;
                            }
                        }
                        Some((terminal_psi::StructuralPathSegment::FixedIndex(index), suffix)) => {
                            let element = elements
                                .get(
                                    usize::try_from(*index)
                                        .map_err(|_| SelectedInstructionError::custody())?,
                                )
                                .ok_or(SelectedInstructionError::custody())?;
                            collect_calls(geometry, &rebased(element, suffix), visited, calls)?;
                        }
                        Some((
                            terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                            suffix,
                        )) => {
                            for element in elements {
                                collect_calls(geometry, &rebased(element, suffix), visited, calls)?;
                            }
                        }
                        _ => return Err(SelectedInstructionError::custody()),
                    }
                }
                I::EstablishZeroedStructural { .. } => visited.origin(),
                I::EstablishStructuralLocal { .. } => {
                    // Its live members come from the retained stores collected
                    // above, rather than from the uninitialized home allocation.
                    if !stored
                        && contains_borrowed_storage(
                            &signature.structural_types,
                            structural_type,
                            &mut BTreeSet::new(),
                        )
                    {
                        return Err(SelectedInstructionError::custody());
                    }
                    if !contains_borrowed_storage(
                        &signature.structural_types,
                        structural_type,
                        &mut BTreeSet::new(),
                    ) {
                        visited.origin();
                    }
                }
                _ => {
                    if contains_borrowed_storage(
                        &signature.structural_types,
                        structural_type,
                        &mut BTreeSet::new(),
                    ) {
                        // A callable aggregate's descriptor words are not a
                        // backing allocation. Missing selected result origins
                        // remain a recoverable custody refusal.
                        return Err(SelectedInstructionError::custody());
                    }
                    visited.origin();
                }
            }
        }
        StructuralPlaceKind::BlockParameter { block, .. } => {
            let mut arrivals = 0;
            for owner in &function.blocks {
                let mut follow = |successor: &legalized_operations::LegalizedScalarSuccessor| -> Result<(), SelectedInstructionError> {
                    if successor.target == block {
                        let binding = successor.structural_binding(place)
                            .ok_or(SelectedInstructionError::custody())?;
                        arrivals += 1;
                        collect_calls(geometry, &rebased(&binding.argument, &coordinate.path), visited, calls)?;
                    }
                    Ok(())
                };
                match &owner.terminator {
                    T::Jump { successor, .. } => follow(successor)?,
                    T::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        follow(when_true)?;
                        follow(when_false)?;
                    }
                    T::StructuralCase { cases, .. }
                        if cases.iter().any(|case| case.target == block) =>
                    {
                        // Case edges carry scalar payloads, not structural
                        // bindings. They cannot authenticate this home.
                        return Err(SelectedInstructionError::custody());
                    }
                    _ => {}
                }
            }
            if arrivals == 0 {
                return Err(SelectedInstructionError::custody());
            }
        }
        StructuralPlaceKind::Parameter { .. }
        | StructuralPlaceKind::ByteSequenceLiteral { .. }
        | StructuralPlaceKind::TrivialAffineLocal { .. } => visited.origin(),
        _ => return Err(SelectedInstructionError::custody()),
    }
    visited.leave();
    Ok(())
}

fn rebased(
    parent: &terminal_psi::StructuralArgument,
    suffix: &[terminal_psi::StructuralPathSegment],
) -> terminal_psi::StructuralArgument {
    let mut result = parent.clone();
    result.path.extend_from_slice(suffix);
    result
}

fn constructed_fields(
    geometry: &mut SourceGeometry<'_>,
    identity: semantic_vocabulary::StructuralTypeId,
    case: Option<semantic_vocabulary::StructuralCaseId>,
    fields: &[terminal_psi::RecordFieldInitializer],
    coordinate: &terminal_psi::StructuralArgument,
    visited: &mut OriginInventory,
    calls: &mut BTreeSet<(OperationId, PlaceId)>,
) -> Result<(), SelectedInstructionError> {
    let function = geometry.function;
    if coordinate.path.is_empty() {
        if fields.is_empty() {
            visited.origin();
        }
        for field in fields {
            if let Some(child) = field.value.as_structural() {
                collect_calls(geometry, child, visited, calls)?;
            } else {
                visited.origin();
            }
        }
        return Ok(());
    }
    let (selected_case, path) = match coordinate.path.split_first() {
        Some((terminal_psi::StructuralPathSegment::Case(selected), suffix)) => {
            (Some(*selected), suffix)
        }
        _ => (None, coordinate.path.as_slice()),
    };
    if selected_case.is_some() && selected_case != case {
        // This constructor cannot supply the selected case. Case liveness is
        // checked at the read; origin inventory supplies no case proof.
        visited.origin();
        return Ok(());
    }
    let (terminal_psi::StructuralPathSegment::Field(name), suffix) = path
        .split_first()
        .ok_or(SelectedInstructionError::custody())?
    else {
        return Err(SelectedInstructionError::custody());
    };
    let shape = &function
        .structural
        .as_ref()
        .ok_or(SelectedInstructionError::custody())?
        .structural_type(identity)
        .ok_or(SelectedInstructionError::custody())?
        .shape;
    let declared = match selected_case {
        Some(case) => shape
            .sum_cases()
            .and_then(|cases| cases.iter().find(|row| row.id == case))
            .map(|case| case.fields.as_slice()),
        None => shape.direct_fields(),
    }
    .ok_or(SelectedInstructionError::custody())?;
    let field = terminal_psi::live_field_by_identity(declared, *name)
        .ok_or(SelectedInstructionError::custody())?;
    let mut initializers = fields
        .iter()
        .filter(|initializer| initializer.field == field.id);
    let initializer = initializers
        .next()
        .ok_or(SelectedInstructionError::custody())?;
    if initializers.next().is_some() {
        return Err(SelectedInstructionError::custody());
    }
    if let Some(child) = initializer.value.as_structural() {
        collect_calls(geometry, &rebased(child, suffix), visited, calls)?;
    } else {
        visited.origin();
    }
    Ok(())
}

fn contains_borrowed_storage(
    types: &[terminal_psi::StructuralTypeDeclaration],
    identity: semantic_vocabulary::StructuralTypeId,
    visited: &mut BTreeSet<semantic_vocabulary::StructuralTypeId>,
) -> bool {
    if !visited.insert(identity) {
        return false;
    }
    let Some(declaration) = types.iter().find(|declaration| declaration.id == identity) else {
        return true;
    };
    match &declaration.shape {
        terminal_psi::StructuralTypeShape::ElementView { .. }
        | terminal_psi::StructuralTypeShape::Reference { .. } => true,
        terminal_psi::StructuralTypeShape::ByteSequence(carrier) => !matches!(
            carrier,
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
        ),
        terminal_psi::StructuralTypeShape::FixedArray { element, .. } => {
            contains_borrowed_storage(types, *element, visited)
        }
        shape => shape
            .direct_fields()
            .into_iter()
            .flatten()
            .chain(
                shape
                    .sum_cases()
                    .into_iter()
                    .flatten()
                    .flat_map(|case| &case.fields),
            )
            .any(|field| {
                field
                    .field_type
                    .as_structural()
                    .is_some_and(|identity| contains_borrowed_storage(types, identity, visited))
                    || field
                        .field_type
                        .byte_sequence_carrier()
                        .is_some_and(|carrier| {
                            !matches!(
                                carrier,
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        })
            }),
    }
}

fn stored_sources(
    geometry: &mut SourceGeometry<'_>,
    coordinate: &terminal_psi::StructuralArgument,
    visited: &mut OriginInventory,
    calls: &mut BTreeSet<(OperationId, PlaceId)>,
) -> Result<bool, SelectedInstructionError> {
    let function = geometry.function;
    let mut found = false;
    for row in function.instructions() {
        let (destination, field, value) = match &row.kind {
            I::StoreStructuralField {
                destination,
                path,
                field,
                value,
                ..
            } => (
                terminal_psi::StructuralArgument {
                    place: destination.place(),
                    path: path.clone(),
                    access: coordinate.access,
                },
                *field,
                *value,
            ),
            I::StructuralByteViewFieldStore {
                destination,
                field,
                source,
                ..
            } => (destination.clone(), *field, *source),
            _ => continue,
        };
        if destination.place != coordinate.place {
            continue;
        }
        let target = field_coordinate(geometry, &destination, field)?.path;
        if !terminal_semantics::structural_paths_may_overlap(&target, &coordinate.path) {
            continue;
        }
        found = true;
        let suffix = if target.len() <= coordinate.path.len() {
            &coordinate.path[target.len()..]
        } else {
            &[]
        };
        let parent = terminal_psi::StructuralArgument::whole(value, coordinate.access);
        collect_calls(geometry, &rebased(&parent, suffix), visited, calls)?;
    }
    Ok(found)
}
