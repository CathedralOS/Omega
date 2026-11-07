//! Find the actual bytes a captured reference points to.
//!
//! Logical field names do not establish disjoint bytes. The existing selected
//! storage-geometry owner supplies the original backing interval, including
//! authored placements. A reference keeps that interval and its evaluated
//! offset across copies and calls. Its target type cannot recreate the source
//! address or turn a narrow stored representation into a whole HANDLE.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{IdTable, PlaceId, ScalarType, StructuralTypeId};
use terminal_psi::{
    Operation, OperationKind, StructuralTypeShape, TerminalMachine, TerminalModule,
};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPathSegment, descriptor_path,
};
use terminal_semantics::descriptor_results::DescriptorMachine;
use terminal_semantics::{ReferenceStorageGeometry, ReferenceStorageRegion, StorageGeometryError};

use super::windows_handle_aliases::{
    HandleAliases, HandleStorageBacking, HandleStorageOwner, HandleStorageRange, HandleWordAliases,
    RelativeHandleRange,
};
use super::windows_handle_lifetimes::HandleInstance;

/// A current pointer value, not a source qualification or guessed allocation.
/// Entry/call rebasing retains the caller's backing and captured scalar codec.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CapturedHandleReference {
    pub referent: StructuralTypeId,
    pub range: HandleStorageRange,
    pub stored_scalar: Option<ScalarType>,
    pub dependencies: BTreeSet<HandleInstance>,
}

/// A descriptor's data window is separate from the bytes of its carrier.
/// Unknown selectors retain a containing window, never a guessed first byte.
/// Such a family can invalidate/read dependencies but cannot supply an exact
/// word or an exact length until an independently evaluated selection does.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CapturedHandleSequence {
    pub range: HandleStorageRange,
    pub element: Option<StructuralTypeId>,
    pub stride: u64,
    pub length: Option<u64>,
    pub precise: bool,
    pub dependencies: BTreeSet<HandleInstance>,
}

impl CapturedHandleSequence {
    pub(super) fn subslice(
        &self,
        start: Option<u64>,
        end: Option<u64>,
        dependencies: BTreeSet<HandleInstance>,
    ) -> Result<Self, String> {
        let mut result = self.clone();
        result.dependencies.extend(dependencies);
        match (self.precise, self.length, start, end) {
            (true, Some(length), Some(start), Some(end)) => {
                if start > end || end > length {
                    return Err(
                        "handle descriptor window exceeds its actual captured extent".into(),
                    );
                }
                let offset = start
                    .checked_mul(self.stride)
                    .and_then(|offset| self.range.byte_offset().checked_add(offset))
                    .ok_or("handle descriptor window offset overflows")?;
                let bytes = (end - start)
                    .checked_mul(self.stride)
                    .ok_or("handle descriptor window extent overflows")?;
                let range = HandleStorageRange::new(self.range.backing(), offset, bytes)?;
                if !self.range.contains(&range) {
                    return Err("handle descriptor window changes its captured backing".into());
                }
                result.range = range;
                result.length = Some(end - start);
            }
            _ => {
                // The original checked bounds still constrain every runtime
                // selection to this source. No selected offset is invented.
                result.precise = false;
                result.length = None;
            }
        }
        Ok(result)
    }
}

/// The scalar's logical result and physical decoding are independently
/// retained. Reading bits through a noninteger or narrow codec loses exact
/// identity before any later widening of the logical value.
pub(super) struct HandleScalarStorage {
    pub range: HandleStorageRange,
    pub stored_scalar: ScalarType,
    pub logical_scalar: ScalarType,
}

pub(super) enum HandleScalarSelection {
    Exact(HandleScalarStorage),
    Family(HandleStorageRange),
}

pub(super) enum HandlePointerKind {
    Reference(StructuralTypeId),
    Sequence(Option<StructuralTypeId>),
}

pub(super) struct HandlePointerSlot {
    pub range: HandleStorageRange,
    pub kind: HandlePointerKind,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CapturedHandleStorage {
    pub range: HandleStorageRange,
    pub stored_scalar: Option<ScalarType>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BorrowedStorageSlot {
    access: terminal_psi::StructuralAccess,
    value: Option<CapturedHandleStorage>,
}

/// Borrowed signature/block roots already name their captured pointee. They
/// do not require a Referent suffix, and must not acquire a callee-owned home.
/// The slots come from the actual declarations, independently of input values.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct HandleStorageBindings {
    rows: IdTable<PlaceId, Option<BorrowedStorageSlot>>,
}

impl HandleStorageBindings {
    pub(super) fn new(body: &TerminalMachine) -> Result<Self, String> {
        let mut rows = body
            .structural_parameters
            .iter()
            .chain(
                body.blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .filter(|parameter| parameter.access != terminal_psi::StructuralAccess::Owned)
            .map(|parameter| {
                (
                    parameter.place,
                    Some(BorrowedStorageSlot {
                        access: parameter.access,
                        value: None,
                    }),
                )
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|(place, _)| *place);
        Ok(Self {
            rows: IdTable::from_rows_with_gaps(rows, 65_536).map_err(|reason| {
                format!("borrowed handle storage namespace is invalid: {reason:?}")
            })?,
        })
    }

    pub(super) fn bind(
        &mut self,
        place: PlaceId,
        access: terminal_psi::StructuralAccess,
        value: CapturedHandleStorage,
    ) -> Result<(), String> {
        let slot = self
            .rows
            .get_mut(place)
            .and_then(Option::as_mut)
            .ok_or("handle storage binding has no original borrowed formal")?;
        if slot.access != access {
            return Err("handle storage binding changes its original formal access".into());
        }
        slot.value = Some(value);
        Ok(())
    }

    fn get(&self, place: PlaceId) -> Result<Option<&CapturedHandleStorage>, String> {
        let Some(slot) = self.rows.get(place).and_then(Option::as_ref) else {
            return Ok(None);
        };
        slot.value.as_ref().map(Some).ok_or_else(|| {
            format!("borrowed handle storage formal {place:?} has no captured actual address")
        })
    }
}

impl HandleScalarStorage {
    pub(super) fn read(&self, aliases: &HandleAliases) -> HandleWordAliases {
        aliases
            .stored_word(&self.range, self.stored_scalar)
            .represented_as(self.logical_scalar)
    }
}

/// Reference values are copied with their resident descriptor slots. A write
/// to a slot removes its old captured view before a new value is installed.
/// Borrowed pointee writes are resolved separately; replacing a reference
/// carrier does not replace the caller's pointee.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct HandleReferences {
    values: BTreeMap<HandleStorageRange, CapturedHandleReference>,
    sequences: BTreeMap<HandleStorageRange, CapturedHandleSequence>,
}

impl HandleReferences {
    pub(super) fn retire_activation(&mut self, activation: usize) {
        let owner = HandleStorageOwner::Activation(activation);
        self.values.retain(|slot, _| slot.backing().owner != owner);
        self.sequences
            .retain(|slot, _| slot.backing().owner != owner);
    }

    pub(super) fn get(&self, location: &HandleStorageRange) -> Option<&CapturedHandleReference> {
        self.values.get(location)
    }

    pub(super) fn replace(&mut self, location: HandleStorageRange, value: CapturedHandleReference) {
        self.retire(&location);
        self.values.insert(location, value);
    }

    pub(super) fn sequence(
        &self,
        location: &HandleStorageRange,
    ) -> Option<&CapturedHandleSequence> {
        self.sequences.get(location)
    }

    pub(super) fn replace_sequence(
        &mut self,
        location: HandleStorageRange,
        value: CapturedHandleSequence,
    ) {
        self.retire(&location);
        self.sequences.insert(location, value);
    }

    pub(super) fn retire(&mut self, location: &HandleStorageRange) {
        self.values.retain(|slot, _| !location.overlaps(slot));
        self.sequences.retain(|slot, _| !location.overlaps(slot));
    }

    pub(super) fn capture(&self, source: &HandleStorageRange) -> CapturedHandleReferences {
        CapturedHandleReferences {
            byte_length: source.byte_length(),
            values: self
                .values
                .iter()
                .filter_map(|(slot, value)| {
                    slot.relative_to(source)
                        .map(|relative| (relative, value.clone()))
                })
                .collect(),
            sequences: self
                .sequences
                .iter()
                .filter_map(|(slot, value)| {
                    slot.relative_to(source)
                        .map(|relative| (relative, value.clone()))
                })
                .collect(),
        }
    }

    pub(super) fn install(
        &mut self,
        destination: &HandleStorageRange,
        captured: &CapturedHandleReferences,
    ) -> Result<(), String> {
        if destination.byte_length() != captured.byte_length {
            return Err("captured reference slots change their complete physical extent".into());
        }
        let values = captured
            .values
            .iter()
            .map(|(relative, value)| Ok((relative.at(destination)?, value.clone())))
            .collect::<Result<Vec<_>, String>>()?;
        let sequences = captured
            .sequences
            .iter()
            .map(|(relative, value)| Ok((relative.at(destination)?, value.clone())))
            .collect::<Result<Vec<_>, String>>()?;
        self.retire(destination);
        self.values.extend(values);
        self.sequences.extend(sequences);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CapturedHandleReferences {
    byte_length: u64,
    values: Vec<(RelativeHandleRange, CapturedHandleReference)>,
    sequences: Vec<(RelativeHandleRange, CapturedHandleSequence)>,
}

pub(super) struct HandleStorageGeometry<'a> {
    module: &'a TerminalModule,
    body: &'a TerminalMachine,
    graph: &'a DescriptorMachine,
    profile: target::TargetProfile,
    activation: usize,
    bindings: &'a HandleStorageBindings,
}

impl<'a> HandleStorageGeometry<'a> {
    pub(super) fn primitive_scalar(&self, carrier: StructuralTypeId) -> Option<ScalarType> {
        match terminal_psi::find_structural_type(&self.module.structural_types, carrier)?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
            _ => None,
        }
    }

    pub(super) fn has_pointer_slots(
        &self,
        carrier: StructuralTypeId,
        place: PlaceId,
    ) -> Result<bool, String> {
        terminal_semantics::descriptor_results::DescriptorSlotKind::BorrowedStorage
            .owned_slots(&self.module.structural_types, place, carrier)
            .map(|slots| !slots.is_empty())
            .ok_or("handle collection element has no complete resident pointer inventory".into())
    }

    pub(super) fn sequence_element(
        &self,
        sequence: &CapturedHandleSequence,
        ordinal: u64,
    ) -> Result<HandleStorageRange, String> {
        if !sequence.precise || sequence.length.is_none_or(|length| ordinal >= length) {
            return Err("handle collection pointer census has no exact selected element".into());
        }
        let element = sequence
            .element
            .ok_or("handle collection has no declared pointer element")?;
        let selected = self.region(element, &[])?;
        let start = ordinal
            .checked_mul(sequence.stride)
            .and_then(|offset| sequence.range.byte_offset().checked_add(offset))
            .ok_or("handle collection pointer offset overflows")?;
        let result =
            HandleStorageRange::new(sequence.range.backing(), start, selected.byte_length)?;
        if !sequence.range.contains(&result) {
            return Err("handle collection pointer exceeds its actual data window".into());
        }
        Ok(result)
    }

    /// Enumerate only pointer carriers resident in this original typed value.
    /// Referents are separate census work; inactive payloads own no slot.
    pub(super) fn pointer_slots(
        &self,
        carrier: StructuralTypeId,
        range: &HandleStorageRange,
        mut active: impl FnMut(
            HandleStorageRange,
            StructuralTypeId,
            semantic_vocabulary::StructuralCaseId,
        ) -> Result<bool, String>,
    ) -> Result<Vec<HandlePointerSlot>, String> {
        let slots = terminal_semantics::descriptor_results::DescriptorSlotKind::BorrowedStorage
            .owned_slots(
                &self.module.structural_types,
                range.backing().place,
                carrier,
            )
            .ok_or("handle output has no complete resident pointer inventory")?;
        let at = |selected: ReferenceStorageRegion| -> Result<HandleStorageRange, String> {
            let offset = range
                .byte_offset()
                .checked_add(selected.byte_offset)
                .ok_or("handle output pointer offset overflows")?;
            let slot = HandleStorageRange::new(range.backing(), offset, selected.byte_length)?;
            if !range.contains(&slot) {
                return Err("handle output pointer exceeds its captured typed bytes".into());
            }
            Ok(slot)
        };
        let mut result = Vec::new();
        'slot: for slot in slots {
            for (ordinal, part) in slot.path.iter().enumerate() {
                if let DescriptorPathSegment::Case(case) = part {
                    let prefix = &slot.path[..ordinal];
                    let sum = if prefix.is_empty() {
                        carrier
                    } else {
                        terminal_semantics::descriptor_lifetimes::descriptor_location_type(
                            &self.module.structural_types,
                            carrier,
                            prefix,
                        )
                        .ok_or("handle output case guard has no exact sum carrier")?
                    };
                    if !active(at(self.region(carrier, prefix)?)?, sum, *case)? {
                        continue 'slot;
                    }
                }
            }
            let selected = self.region(carrier, &slot.path)?;
            let kind = if slot.path.is_empty() {
                Some(carrier)
            } else {
                terminal_semantics::descriptor_lifetimes::descriptor_location_type(
                    &self.module.structural_types,
                    carrier,
                    &slot.path,
                )
            }
            .and_then(|selected| {
                terminal_psi::find_structural_type(&self.module.structural_types, selected)
            });
            let kind = match kind.map(|declaration| &declaration.shape) {
                Some(StructuralTypeShape::Reference { referent, .. }) => {
                    HandlePointerKind::Reference(*referent)
                }
                Some(StructuralTypeShape::ElementView { element, .. }) => {
                    HandlePointerKind::Sequence(Some(*element))
                }
                Some(StructuralTypeShape::ByteSequence(_)) | None => {
                    HandlePointerKind::Sequence(None)
                }
                _ => {
                    return Err(
                        "handle output pointer inventory changes its declared slot kind".into(),
                    );
                }
            };
            result.push(HandlePointerSlot {
                range: at(selected)?,
                kind,
            });
        }
        Ok(result)
    }

    /// Keep evaluated selectors at this operation's actual read/write cut.
    /// An unknown index names its entire collection, without choosing an
    /// element or following an unknown element's resident reference.
    pub(super) fn scalar_selection(
        &self,
        place: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        references: &HandleReferences,
        expected: ScalarType,
        mut index: impl FnMut(semantic_vocabulary::ValueId) -> Result<Option<u64>, String>,
    ) -> Result<HandleScalarSelection, String> {
        let mut selected = Vec::with_capacity(path.len());
        for (ordinal, part) in path.iter().enumerate() {
            if let terminal_psi::StructuralPathSegment::RuntimeIndex { index: actual, .. } = part {
                match index(*actual)? {
                    Some(index) => {
                        selected.push(terminal_psi::StructuralPathSegment::FixedIndex(index))
                    }
                    None => {
                        if path[ordinal + 1..].iter().any(|part| {
                            matches!(part, terminal_psi::StructuralPathSegment::Referent)
                        }) {
                            return Err("unknown handle element cannot select a resident reference's pointee".into());
                        }
                        let source = self.location(place, &selected)?;
                        return Ok(HandleScalarSelection::Family(
                            self.sequence(&source, references)?.range,
                        ));
                    }
                }
            } else {
                selected.push(part.clone());
            }
        }
        let location = self.location(place, &selected)?;
        Ok(HandleScalarSelection::Exact(
            self.scalar(&location, references, expected)?,
        ))
    }

    pub(super) fn element_stride(&self, element: StructuralTypeId) -> Result<u64, String> {
        let (size, alignment) =
            ReferenceStorageGeometry::new(self.module.structural_types.iter(), |application| {
                Err(StorageGeometryError::UnresolvedExtent(application))
            })
            .with_target_carriers(Some(runtime_abi::target_carrier_layout(
                self.profile.native_target(),
            )))
            .with_record_storage(layout::TerminalRecordStorage {
                module: self.module,
                target: self.profile,
            })
            .layout(element)
            .map_err(|reason| format!("handle view element has no selected layout: {reason}"))?;
        size.checked_add(
            alignment
                .checked_sub(1)
                .ok_or("handle element has zero alignment")?,
        )
        .map(|rounded| rounded / alignment * alignment)
        .ok_or("handle element stride overflows".into())
    }

    pub(super) fn as_elements(
        &self,
        mut source: CapturedHandleSequence,
        element: StructuralTypeId,
        representation: bool,
    ) -> Result<CapturedHandleSequence, String> {
        let stride = self.element_stride(element)?;
        if !representation {
            let byte_element = terminal_psi::find_structural_type(&self.module.structural_types, element)
                .is_some_and(|declaration| matches!(declaration.shape,
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 8));
            if source.element != Some(element) && !(source.element.is_none() && byte_element) {
                return Err("handle view changes its original collection element without a representation operation".into());
            }
            if source.stride != stride {
                return Err("handle view changes the original selected element stride".into());
            }
        } else if source.precise {
            if stride == 0 {
                return Err("handle representation view has no nonzero element width".into());
            }
            if source.range.byte_length() % stride != 0 {
                return Err(
                    "handle representation view does not tile its original captured bytes".into(),
                );
            }
            source.length = Some(source.range.byte_length() / stride);
        }
        source.element = Some(element);
        source.stride = stride;
        Ok(source)
    }

    /// Byte views and element views use the same formation operation, but
    /// retain their declared carrier. The u8 specialization does not introduce
    /// a new element declaration or change the captured pointer/stride.
    pub(super) fn present_sequence(
        &self,
        mut source: CapturedHandleSequence,
        destination: &DescriptorLocation,
    ) -> Result<CapturedHandleSequence, String> {
        let carrier = self
            .carrier(destination)?
            .and_then(|carrier| {
                terminal_psi::find_structural_type(&self.module.structural_types, carrier)
            })
            .ok_or("handle descriptor result has no original declared carrier")?;
        match &carrier.shape {
            StructuralTypeShape::ByteSequence(carrier) => {
                if source.stride != 1 || source.element.is_some_and(|element|
                    !matches!(self.primitive_scalar(element), Some(ScalarType::Integer(integer))
                        if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 8)) {
                    return Err("handle byte descriptor changes its captured element representation".into());
                }
                if let terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } = carrier {
                    if source.precise && source.length != Some(*capacity) {
                        return Err(
                            "handle bounded descriptor changes its declared complete extent".into(),
                        );
                    }
                    source.length = Some(*capacity);
                } else if !matches!(
                    carrier,
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                ) {
                    return Err("handle descriptor formation substitutes owned byte storage".into());
                }
                source.element = None;
            }
            StructuralTypeShape::ElementView { element, .. }
                if source.element == Some(*element) => {}
            _ => return Err("handle descriptor formation changes its original result kind".into()),
        }
        Ok(source)
    }

    pub(super) fn sequence(
        &self,
        location: &DescriptorLocation,
        references: &HandleReferences,
    ) -> Result<CapturedHandleSequence, String> {
        let (range, selected, _) = self.resolve(location, references)?;
        let carrier = self
            .carrier(location)?
            .and_then(|carrier| {
                terminal_psi::find_structural_type(&self.module.structural_types, carrier)
            })
            .ok_or("handle collection has no original declared carrier")?;
        match &carrier.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                let length = length
                    .literal()
                    .ok_or("handle collection has an unresolved original extent")?;
                let first = self.region(*element, &[])?;
                let mut zero = location.clone();
                zero.path.push(DescriptorPathSegment::FixedIndex(0));
                let stride = if length >= 2 {
                    let (first, _, _) = self.resolve(&zero, references)?;
                    let mut second = location.clone();
                    second.path.push(DescriptorPathSegment::FixedIndex(1));
                    let (second, _, _) = self.resolve(&second, references)?;
                    second
                        .byte_offset()
                        .checked_sub(first.byte_offset())
                        .ok_or("handle collection stride changes its original element ordering")?
                } else {
                    self.element_stride(*element)?
                };
                if selected.array.is_none() || stride < first.byte_length {
                    return Err(
                        "handle collection has no complete original element geometry".into(),
                    );
                }
                Ok(CapturedHandleSequence {
                    range,
                    element: Some(*element),
                    stride,
                    length: Some(length),
                    precise: true,
                    dependencies: BTreeSet::new(),
                })
            }
            StructuralTypeShape::ByteSequence(_) | StructuralTypeShape::ElementView { .. } => {
                references
                    .sequence(&range)
                    .cloned()
                    .ok_or("handle descriptor has no captured data window".into())
            }
            _ => Err("handle data window is not an original collection or descriptor".into()),
        }
    }

    pub(super) fn location(
        &self,
        place: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<DescriptorLocation, String> {
        let root = self.place_type(&DescriptorLocation::whole(place))?;
        terminal_semantics::descriptor_lifetimes::descriptor_location(
            &self.module.structural_types,
            place,
            root,
            path,
            None,
        )
        .ok_or_else(|| {
            format!("handle argument {place:?}/{path:?} has no exact storage coordinate")
        })
    }

    pub(super) fn carrier(
        &self,
        location: &DescriptorLocation,
    ) -> Result<Option<StructuralTypeId>, String> {
        let root = self.place_type(location)?;
        if location.path.is_empty() {
            return Ok(Some(root));
        }
        Ok(
            terminal_semantics::descriptor_lifetimes::descriptor_location_type(
                &self.module.structural_types,
                root,
                &location.path,
            ),
        )
    }

    /// A runtime-selected write can touch any member of its first collection.
    /// Return that containing interval, not a guessed element address. The
    /// caller must retain old dependencies and invalidate exact overlapping
    /// words, because this is a possible write rather than a full replacement.
    pub(super) fn possible_write(
        &self,
        query: &terminal_semantics::descriptor_lifetimes::DescriptorContentsQuery,
        references: &HandleReferences,
    ) -> Result<HandleStorageRange, String> {
        use terminal_semantics::descriptor_lifetimes::DescriptorContentsStep;
        let mut location = DescriptorLocation::whole(query.root);
        for step in &query.steps {
            match step {
                DescriptorContentsStep::Address(segment) => location.path.push(segment.clone()),
                DescriptorContentsStep::AnyElement { .. } => {
                    let carrier = self.carrier(&location)?.and_then(|carrier| {
                        terminal_psi::find_structural_type(&self.module.structural_types, carrier)
                    });
                    if !matches!(
                        carrier.map(|carrier| &carrier.shape),
                        Some(StructuralTypeShape::FixedArray { .. })
                    ) {
                        return Err(
                            "handle may-write collection has no captured contiguous backing".into(),
                        );
                    }
                    break;
                }
            }
        }
        self.resolve(&location, references)
            .map(|(range, _, _)| range)
    }

    pub(super) fn new(
        module: &'a TerminalModule,
        body: &'a TerminalMachine,
        graph: &'a DescriptorMachine,
        profile: target::TargetProfile,
        activation: usize,
        bindings: &'a HandleStorageBindings,
    ) -> Result<Self, String> {
        if body.id != graph.id || graph.entry != (body.entry, 0) {
            return Err("handle storage geometry belongs to another canonical body".into());
        }
        if profile != target::TargetProfile::WindowsX64 {
            return Err("Windows handle storage has another selected target profile".into());
        }
        Ok(Self {
            module,
            body,
            graph,
            profile,
            activation,
            bindings,
        })
    }

    fn region(
        &self,
        root: StructuralTypeId,
        path: &[DescriptorPathSegment],
    ) -> Result<ReferenceStorageRegion, String> {
        let path = descriptor_path(&self.module.structural_types, root, path)
            .ok_or("handle storage path has no complete original typed projection")?;
        let mut geometry =
            ReferenceStorageGeometry::new(self.module.structural_types.iter(), |application| {
                Err(StorageGeometryError::UnresolvedExtent(application))
            })
            .with_target_carriers(Some(runtime_abi::target_carrier_layout(
                self.profile.native_target(),
            )))
            .with_record_storage(layout::TerminalRecordStorage {
                module: self.module,
                target: self.profile,
            });
        geometry.region(root, &path).ok_or_else(|| {
            format!("handle storage {root:?}/{path:?} has no closed selected byte geometry")
        })
    }

    fn place_type(&self, location: &DescriptorLocation) -> Result<StructuralTypeId, String> {
        self.graph
            .place_types
            .get(&location.root)
            .copied()
            .ok_or_else(|| {
                format!(
                    "handle storage {:?}/{location:?} has no original declared carrier",
                    self.body.id
                )
            })
    }

    /// Resolve a complete current reference before selecting its pointee bytes.
    /// Missing/nested slot captures remain refusals, never an empty backing.
    pub(super) fn resolve(
        &self,
        location: &DescriptorLocation,
        references: &HandleReferences,
    ) -> Result<
        (
            HandleStorageRange,
            ReferenceStorageRegion,
            Option<ScalarType>,
        ),
        String,
    > {
        if location.path.len() > 256 {
            return Err("handle storage path exceeds its bounded projection depth".into());
        }
        let last_referent = location
            .path
            .iter()
            .rposition(|part| *part == DescriptorPathSegment::Referent);
        for (ordinal, part) in location.path.iter().enumerate().rev() {
            if last_referent.is_some_and(|referent| referent > ordinal) {
                continue;
            }
            let DescriptorPathSegment::FixedIndex(index) = part else {
                continue;
            };
            let prefix = DescriptorLocation {
                root: location.root,
                path: location.path[..ordinal].to_vec(),
            };
            let Some(carrier) = self.carrier(&prefix)? else {
                continue;
            };
            if !terminal_psi::find_structural_type(&self.module.structural_types, carrier)
                .is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                })
            {
                continue;
            }
            let captured = self.sequence(&prefix, references)?;
            if !captured.precise || captured.length.is_none_or(|length| *index >= length) {
                return Err(
                    "handle element address has no exact captured descriptor window".into(),
                );
            }
            let element = captured
                .element
                .ok_or("handle element view lost its original element declaration")?;
            let selected = self.region(element, &location.path[ordinal + 1..])?;
            let offset = index
                .checked_mul(captured.stride)
                .and_then(|offset| offset.checked_add(selected.byte_offset))
                .and_then(|offset| captured.range.byte_offset().checked_add(offset))
                .ok_or("captured handle element projection overflows")?;
            let range =
                HandleStorageRange::new(captured.range.backing(), offset, selected.byte_length)?;
            if !captured.range.contains(&range) {
                return Err("handle element projection exceeds its actual data window".into());
            }
            return Ok((range, selected, None));
        }
        if let Some(referent) = last_referent {
            let slot = DescriptorLocation {
                root: location.root,
                path: location.path[..referent].to_vec(),
            };
            let (slot_range, _, _) = self.resolve(&slot, references)?;
            let captured = references.get(&slot_range).ok_or_else(|| {
                format!(
                    "handle storage reference {:?}/{slot:?} has no current captured backing",
                    self.body.id
                )
            })?;
            let selected = self.region(captured.referent, &location.path[referent + 1..])?;
            let length = if let Some(stored) = captured.stored_scalar {
                if !location.path[referent + 1..].is_empty() || selected.scalar.is_none() {
                    return Err(
                        "scalar reference codec is projected as another storage region".into(),
                    );
                }
                u64::from(
                    terminal_semantics::reference_storage_byte_width(stored)
                        .ok_or("handle scalar reference codec has no closed physical width")?,
                )
            } else {
                selected.byte_length
            };
            let offset = captured
                .range
                .byte_offset()
                .checked_add(selected.byte_offset)
                .ok_or("handle reference projection offset overflows")?;
            let range = HandleStorageRange::new(captured.range.backing(), offset, length)?;
            if !captured.range.contains(&range) {
                return Err(
                    "handle reference projection exceeds its actual captured backing".into(),
                );
            }
            return Ok((range, selected, captured.stored_scalar));
        }
        let selected = self.region(self.place_type(location)?, &location.path)?;
        if let Some(captured) = self.bindings.get(location.root)? {
            let length = match captured.stored_scalar {
                Some(stored) => {
                    if !location.path.is_empty() || selected.scalar.is_none() {
                        return Err(
                            "borrowed scalar codec is projected as another storage region".into(),
                        );
                    }
                    u64::from(
                        terminal_semantics::reference_storage_byte_width(stored)
                            .ok_or("borrowed handle scalar codec has no closed physical width")?,
                    )
                }
                None => selected.byte_length,
            };
            let offset = captured
                .range
                .byte_offset()
                .checked_add(selected.byte_offset)
                .ok_or("borrowed handle storage projection offset overflows")?;
            let range = HandleStorageRange::new(captured.range.backing(), offset, length)?;
            if !captured.range.contains(&range) {
                return Err(
                    "borrowed handle storage projection exceeds its captured actual bytes".into(),
                );
            }
            return Ok((range, selected, captured.stored_scalar));
        }
        let backing = HandleStorageBacking {
            owner: HandleStorageOwner::Activation(self.activation),
            place: location.root,
        };
        Ok((
            HandleStorageRange::new(backing, selected.byte_offset, selected.byte_length)?,
            selected,
            None,
        ))
    }

    pub(super) fn scalar(
        &self,
        location: &DescriptorLocation,
        references: &HandleReferences,
        expected: ScalarType,
    ) -> Result<HandleScalarStorage, String> {
        let (range, selected, codec) = self.resolve(location, references)?;
        let scalar = selected
            .scalar
            .ok_or("handle scalar observation selects an aggregate region")?;
        if scalar.logical_scalar != expected {
            return Err("handle scalar observation changes its original logical carrier".into());
        }
        Ok(HandleScalarStorage {
            range,
            logical_scalar: scalar.logical_scalar,
            stored_scalar: codec.unwrap_or(scalar.stored_scalar),
        })
    }

    /// The execution owner supplies the evaluated offset of this operation's
    /// original byte_offset operand. An unknown offset cannot authorize a view.
    /// Source selection occurs before any result/slot replacement.
    pub(super) fn capture_reference(
        &self,
        operation: &Operation,
        references: &HandleReferences,
        evaluated_offset: Option<u64>,
        dependencies: BTreeSet<HandleInstance>,
    ) -> Result<(HandleStorageRange, CapturedHandleReference), String> {
        let OperationKind::EstablishReference { source, storage } = &operation.kind else {
            return Err("handle reference capture is not its actual reference formation".into());
        };
        let result = operation
            .result
            .structural()
            .ok_or("handle reference capture has no original structural result")?;
        let StructuralTypeShape::Reference { referent, access } =
            terminal_psi::find_structural_type(
                &self.module.structural_types,
                result.structural_type,
            )
            .ok_or("handle reference result has no declared reference carrier")?
            .shape
        else {
            return Err("handle reference result is not a reference permission".into());
        };
        if access != source.access {
            return Err("handle reference formation changes its original access mode".into());
        }
        let root = *self
            .graph
            .place_types
            .get(&source.place)
            .ok_or("handle reference source has no declared carrier")?;
        let source_path = terminal_semantics::reference_source_storage_path(
            self.module.structural_types.iter(),
            root,
            &source.path,
        );
        let source = terminal_semantics::descriptor_lifetimes::descriptor_location(
            &self.module.structural_types,
            source.place,
            root,
            source_path,
            None,
        )
        .ok_or("handle reference source has no exact captured coordinate")?;
        let (backing, _, inherited_codec) = self.resolve(&source, references)?;
        let referred = self.region(referent, &[])?;
        let (offset, stored_scalar) = match storage {
            Some(storage) => (
                evaluated_offset
                    .ok_or("handle reference offset has no evaluated original operand")?,
                storage.stored_scalar,
            ),
            None => (0, inherited_codec),
        };
        let length = match stored_scalar {
            Some(scalar) => {
                if referred.scalar.is_none() {
                    return Err(
                        "handle reference scalar codec substitutes an aggregate referent".into(),
                    );
                }
                u64::from(
                    terminal_semantics::reference_storage_byte_width(scalar)
                        .ok_or("handle reference's selected scalar codec has no closed width")?,
                )
            }
            None => referred.byte_length,
        };
        let start = backing
            .byte_offset()
            .checked_add(offset)
            .ok_or("captured handle reference offset overflows")?;
        let range = HandleStorageRange::new(backing.backing(), start, length)?;
        if !backing.contains(&range) {
            return Err(
                "captured handle reference exceeds its original source byte interval".into(),
            );
        }
        let (carrier, _, _) = self.resolve(&DescriptorLocation::whole(result.place), references)?;
        Ok((
            carrier,
            CapturedHandleReference {
                referent,
                range,
                stored_scalar,
                dependencies,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(place: u64, start: u64, length: u64) -> HandleStorageRange {
        HandleStorageRange::new(
            HandleStorageBacking {
                owner: HandleStorageOwner::Activation(0),
                place: PlaceId::new(place).unwrap(),
            },
            start,
            length,
        )
        .unwrap()
    }

    #[test]
    fn captured_windows_do_not_recover_an_unknown_selected_address() {
        let source = CapturedHandleSequence {
            range: range(1, 32, 24),
            element: None,
            stride: 1,
            length: Some(24),
            precise: true,
            dependencies: BTreeSet::new(),
        };
        let selected = source.subslice(Some(5), Some(9), BTreeSet::new()).unwrap();
        assert_eq!(selected.range, range(1, 37, 4));
        assert_eq!(selected.length, Some(4));
        let unknown = source.subslice(None, Some(9), BTreeSet::new()).unwrap();
        assert!(!unknown.precise);
        assert_eq!(unknown.length, None);
        assert_eq!(unknown.range, source.range);
        let narrowed = unknown.subslice(Some(0), Some(4), BTreeSet::new()).unwrap();
        assert!(!narrowed.precise);
        assert_eq!(narrowed.range, source.range);
        assert!(
            source
                .subslice(Some(25), Some(26), BTreeSet::new())
                .is_err()
        );
    }

    #[test]
    fn partial_descriptor_clobber_discards_its_pointer_and_failed_copy_is_atomic() {
        let slot = range(1, 0, 16);
        let pointee = range(2, 80, 12);
        let mut references = HandleReferences::default();
        references.replace_sequence(
            slot,
            CapturedHandleSequence {
                range: pointee,
                element: None,
                stride: 1,
                length: Some(12),
                precise: true,
                dependencies: BTreeSet::new(),
            },
        );
        let captured = references.capture(&slot);
        let old = references.clone();
        assert!(references.install(&range(3, 0, 8), &captured).is_err());
        assert_eq!(references, old);
        references.install(&range(3, 0, 16), &captured).unwrap();
        assert_eq!(
            references.sequence(&range(3, 0, 16)).unwrap().range,
            pointee
        );
        references.retire(&range(1, 7, 1));
        assert!(references.sequence(&slot).is_none());
        assert_eq!(
            references.sequence(&range(3, 0, 16)).unwrap().range,
            pointee
        );
    }
}
