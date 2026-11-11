//! Find the actual bytes a captured reference points to.
//!
//! Logical field names do not establish disjoint bytes. The existing selected
//! storage-geometry owner supplies the original backing interval, including
//! authored placements. A reference keeps that interval and its evaluated
//! offset across copies and calls. Its target type cannot recreate the source
//! address or turn a narrow stored representation into a whole HANDLE.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use semantic_vocabulary::{IdTable, PlaceId, ScalarType, StructuralTypeId};
use terminal_psi::{
    Operation, OperationKind, StructuralTypeShape, TerminalMachine, TerminalModule,
};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPathSegment, descriptor_path,
};
use terminal_semantics::descriptor_results::{DescriptorMachine, ResidentValueTransfer};
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
    /// The selected path must denote the complete declared array value.
    /// Explicit byte windows have no element namespace and need a separate
    /// checked interpretation; their presence is not a whole-array proof.
    fn from_complete_array(
        range: HandleStorageRange,
        selected: ReferenceStorageRegion,
        element: StructuralTypeId,
        length: u64,
        stride: u64,
        element_size: u64,
        dependencies: BTreeSet<HandleInstance>,
    ) -> Result<Self, String> {
        let complete_bytes = stride
            .checked_mul(length)
            .ok_or("handle array's complete extent overflows")?;
        if selected.array.is_some()
            || selected.scalar.is_some()
            || stride < element_size
            || selected.byte_length != complete_bytes
            || range.byte_length() != complete_bytes
        {
            return Err(format!(
                "handle array has no complete original element geometry: element {element:?}, count {length}, stride {stride}, element bytes {element_size}, selected {selected:?}, actual {range:?}"
            ));
        }
        Ok(Self {
            range,
            element: Some(element),
            stride,
            length: Some(length),
            precise: true,
            dependencies,
        })
    }

    fn element_range(&self, ordinal: u64, byte_length: u64) -> Result<HandleStorageRange, String> {
        if !self.precise || self.length.is_none_or(|length| ordinal >= length) {
            return Err("handle collection has no exact selected element".into());
        }
        let start = ordinal
            .checked_mul(self.stride)
            .and_then(|offset| self.range.byte_offset().checked_add(offset))
            .ok_or("handle collection pointer offset overflows")?;
        let result = HandleStorageRange::new(self.range.backing(), start, byte_length)?;
        if !self.range.contains(&result) {
            return Err("handle collection pointer exceeds its actual data window".into());
        }
        Ok(result)
    }

    /// Reading a view element observes captured data, not a byte inside its
    /// descriptor. Unknown selectors keep the complete possible data range;
    /// even one selected byte retains every dependency of its captured address.
    pub(super) fn byte_scalar_selection(
        self,
        location: DescriptorLocation,
        expected: ScalarType,
        index: Option<u64>,
    ) -> Result<HandleScalarSelection, String> {
        if !matches!(expected, ScalarType::Integer(integer)
            if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 8)
        {
            return Err("handle byte read changes its original scalar codec".into());
        }
        let range = self.byte_selection_range(index)?;
        // This remains a content observation with no exact numeric value.
        // Byte dependencies cannot reconstruct a complete HANDLE identity.
        Ok(HandleScalarSelection::Family {
            location,
            range,
            dependencies: self.dependencies,
        })
    }

    /// A point read or weak write selects the same captured data bytes.
    /// An unknown selector preserves the full possible data window, including
    /// its original backing; it never selects bytes of the descriptor itself.
    fn byte_selection_range(&self, index: Option<u64>) -> Result<HandleStorageRange, String> {
        if self.element.is_some() || self.stride != 1 {
            return Err("handle byte selection changes its original byte window".into());
        }
        match index.filter(|_| self.precise) {
            Some(index) => self.element_range(index, 1),
            None => Ok(self.range),
        }
    }

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
    pub address_dependencies: BTreeSet<HandleInstance>,
    pub range: HandleStorageRange,
    pub stored_scalar: ScalarType,
    pub logical_scalar: ScalarType,
}

pub(super) enum HandleScalarSelection {
    Exact {
        location: DescriptorLocation,
        storage: HandleScalarStorage,
    },
    Family {
        location: DescriptorLocation,
        range: HandleStorageRange,
        dependencies: BTreeSet<HandleInstance>,
    },
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
    pub address_dependencies: BTreeSet<HandleInstance>,
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
/// Execution snapshots share unchanged bindings. Only an actual formal bind
/// copies the table; equality and ordering still compare every original slot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct HandleStorageBindings {
    rows: Arc<IdTable<PlaceId, Option<BorrowedStorageSlot>>>,
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
            rows: Arc::new(
                IdTable::from_rows_with_gaps(rows, 65_536).map_err(|reason| {
                    format!("borrowed handle storage namespace is invalid: {reason:?}")
                })?,
            ),
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
            .get(place)
            .and_then(Option::as_ref)
            .ok_or("handle storage binding has no original borrowed formal")?;
        if slot.access != access {
            return Err("handle storage binding changes its original formal access".into());
        }
        let slot = Arc::make_mut(&mut self.rows)
            .get_mut(place)
            .and_then(Option::as_mut)
            .ok_or("handle storage binding lost its original borrowed formal")?;
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
        let word = aliases
            .stored_word(&self.range, self.stored_scalar)
            .represented_as(self.logical_scalar);
        if self.address_dependencies.is_empty() {
            word
        } else {
            HandleWordAliases::derived_from_dependencies(
                self.logical_scalar,
                word.dependencies()
                    .iter()
                    .chain(&self.address_dependencies)
                    .copied(),
            )
        }
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

/// A borrowed descriptor may keep its pointer and extent under another type
/// identity. Reuse Terminal's admitted presentation rule, preserving only the
/// unchanged representation here. Constructing a pointer to inline backing
/// needs its own producer. Original call/edge custody still owns access and
/// qualified-type admission; owned values retain their exact carrier.
fn unchanged_storage_presentation(
    types: &[terminal_psi::StructuralTypeDeclaration],
    actual: Option<StructuralTypeId>,
    expected: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
) -> bool {
    actual.is_some_and(|actual| {
        actual == expected
            || (access != terminal_psi::StructuralAccess::Owned
                && terminal_semantics::borrowed_referent_presentation(
                    types.iter(),
                    actual,
                    expected,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged))
    })
}

/// A leaf copy can publish the same shared descriptor under its canonical
/// value type. Reuse the copy rule, not geometry alone: the original operation
/// must select this source and result, and both descriptor values must retain
/// shared access. This does not admit reference formation or another writer.
fn shared_leaf_copy_presentation(
    types: &[terminal_psi::StructuralTypeDeclaration],
    operation: &Operation,
    transfer: &ResidentValueTransfer,
    source_root_type: StructuralTypeId,
    carriers: (Option<StructuralTypeId>, Option<StructuralTypeId>),
    root_access: Option<terminal_psi::StructuralAccess>,
) -> bool {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    let Some(result) = operation.result.structural() else {
        return false;
    };
    let selected = match &operation.kind {
        OperationKind::StructuralLeafCopy { source, path } => {
            terminal_semantics::descriptor_lifetimes::descriptor_location(
                types,
                *source,
                source_root_type,
                path,
                None,
            )
        }
        OperationKind::StructuralCaseLeafCopy { source, path } => {
            terminal_semantics::descriptor_lifetimes::canonical_descriptor_location(
                types,
                *source,
                source_root_type,
                path,
            )
        }
        _ => return false,
    };
    let (Some(source_type), Some(destination_type)) = carriers else {
        return false;
    };
    selected.as_ref() == Some(&transfer.source)
        && transfer.destination == DescriptorLocation::whole(result.place)
        && destination_type == result.structural_type
        && root_access != Some(StructuralAccess::WriteOnlyBorrow)
        && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && (result.multiplicity == StructuralMultiplicity::Unrestricted
            || (result.multiplicity == StructuralMultiplicity::Affine
                && terminal_semantics::is_copyable_carrier(types.iter(), result.structural_type)))
        && terminal_semantics::shared_descriptor_copy_matches(
            types.iter(),
            source_type,
            transfer
                .source
                .path
                .is_empty()
                .then_some(root_access)
                .flatten(),
            result.structural_type,
            result.multiplicity,
        )
}

impl<'a> HandleStorageGeometry<'a> {
    /// Select only the actual discriminator bytes. A neighboring payload may
    /// retain an old HANDLE without making a tag observation read that value.
    pub(super) fn case_tag(
        &self,
        location: &DescriptorLocation,
        references: &HandleReferences,
    ) -> Result<(HandleStorageRange, BTreeSet<HandleInstance>), String> {
        let (range, _, codec, address_dependencies) = self.resolve(location, references)?;
        let carrier = self
            .carrier(location)?
            .ok_or("handle case observation has no original sum carrier")?;
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
        let sum = geometry.sum_layout(carrier).map_err(|reason| {
            format!("handle case tag has no closed selected geometry: {reason:?}")
        })?;
        if codec.is_some() || range.byte_length() != sum.storage.byte_size {
            return Err("handle case tag changes its complete selected sum backing".into());
        }
        let start = range
            .byte_offset()
            .checked_add(sum.tag_byte_offset)
            .ok_or("handle case tag offset overflows its actual backing")?;
        let tag = HandleStorageRange::new(range.backing(), start, sum.tag.byte_size)?;
        if sum.tag.byte_size == 0 || !range.contains(&tag) {
            return Err("handle case tag exceeds its complete selected sum backing".into());
        }
        Ok((tag, address_dependencies))
    }

    pub(super) fn source_context(&self) -> (semantic_vocabulary::MachineId, usize) {
        (self.body.id, self.activation)
    }

    /// Inline byte fields have their own original carrier contract even when
    /// the catalog has no standalone structural type for that leaf.
    fn inline_byte_carrier(
        &self,
        location: &DescriptorLocation,
    ) -> Result<terminal_psi::ByteSequenceCarrier, String> {
        let Some((DescriptorPathSegment::Field(field), prefix)) = location.path.split_last() else {
            return Err(
                "untyped handle content source is not an original inline byte field".into(),
            );
        };
        let (prefix, selected_case) = match prefix.split_last() {
            Some((DescriptorPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
            _ => (prefix, None),
        };
        let owner = DescriptorLocation {
            root: location.root,
            path: prefix.to_vec(),
        };
        let carrier = self
            .carrier(&owner)?
            .and_then(|carrier| {
                terminal_psi::find_structural_type(&self.module.structural_types, carrier)
            })
            .ok_or("inline byte field has no original parent carrier")?;
        let fields = match selected_case {
            Some(case) => {
                let cases = carrier
                    .shape
                    .sum_cases()
                    .ok_or("inline byte payload has no original sum carrier")?;
                let mut matching = cases.iter().filter(|candidate| candidate.id == case);
                let case = matching
                    .next()
                    .ok_or("inline byte payload has no original case")?;
                if matching.next().is_some() {
                    return Err("inline byte payload case is ambiguous".into());
                }
                case.fields.as_slice()
            }
            None => carrier
                .shape
                .record_fields()
                .ok_or("inline byte field has no original field roster")?,
        };
        let mut matching = fields
            .iter()
            .filter(|candidate| candidate.id == *field && !candidate.relevance.is_erased());
        let field = matching
            .next()
            .ok_or("inline byte field is absent from its original parent")?;
        if matching.next().is_some() {
            return Err("inline byte field identity is ambiguous".into());
        }
        field
            .field_type
            .byte_sequence_carrier()
            .ok_or("untyped handle content source is not a byte carrier".into())
    }
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
        let byte_length = match sequence.element {
            Some(element) => self.region(element, &[])?.byte_length,
            None if sequence.stride == 1 => 1,
            None => return Err("handle byte collection changes its original unit stride".into()),
        };
        sequence.element_range(ordinal, byte_length)
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
                        if self.is_byte_view(&source)? {
                            if !path[ordinal + 1..].is_empty() {
                                return Err("handle byte endpoint has another projection".into());
                            }
                            return self
                                .sequence(&source, references)?
                                .byte_scalar_selection(source, expected, None);
                        }
                        let captured = self.sequence(&source, references)?;
                        return Ok(HandleScalarSelection::Family {
                            range: captured.range,
                            dependencies: captured.dependencies,
                            location: source,
                        });
                    }
                }
            } else {
                selected.push(part.clone());
            }
        }
        if let Some((terminal_psi::StructuralPathSegment::FixedIndex(index), prefix)) =
            selected.split_last()
        {
            let source = self.location(place, prefix)?;
            if self.is_byte_view(&source)? {
                return self.sequence(&source, references)?.byte_scalar_selection(
                    source,
                    expected,
                    Some(*index),
                );
            }
        }
        let location = self.location(place, &selected)?;
        let storage = self.scalar(&location, references, expected)?;
        Ok(HandleScalarSelection::Exact { location, storage })
    }

    fn is_byte_view(&self, location: &DescriptorLocation) -> Result<bool, String> {
        let carrier = match self.carrier(location)? {
            Some(carrier) => {
                match terminal_psi::find_structural_type(&self.module.structural_types, carrier)
                    .map(|declaration| &declaration.shape)
                {
                    Some(StructuralTypeShape::ByteSequence(carrier)) => *carrier,
                    _ => return Ok(false),
                }
            }
            None => self.inline_byte_carrier(location)?,
        };
        Ok(matches!(
            carrier,
            terminal_psi::byte_sequence_view_carrier_pattern!()
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
            if !source.range.byte_length().is_multiple_of(stride) {
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
        let (range, selected, _, address_dependencies) = self.resolve(location, references)?;
        if self.carrier(location)?.is_none() {
            if !matches!(
                self.inline_byte_carrier(location)?,
                terminal_psi::byte_sequence_view_carrier_pattern!()
            ) {
                return Err("inline owned byte contents have no retained live data window".into());
            }
            let captured = references
                .sequence(&range)
                .ok_or("inline byte descriptor has no captured data window")?;
            if captured.element.is_some() || captured.stride != 1 {
                return Err("inline byte descriptor changes its original byte carrier".into());
            }
            let mut captured = captured.clone();
            captured.dependencies.extend(address_dependencies);
            return Ok(captured);
        }
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
                    let (first, _, _, _) = self.resolve(&zero, references)?;
                    let mut second = location.clone();
                    second.path.push(DescriptorPathSegment::FixedIndex(1));
                    let (second, _, _, _) = self.resolve(&second, references)?;
                    second
                        .byte_offset()
                        .checked_sub(first.byte_offset())
                        .ok_or("handle collection stride changes its original element ordering")?
                } else {
                    self.element_stride(*element)?
                };
                CapturedHandleSequence::from_complete_array(
                    range,
                    selected,
                    *element,
                    length,
                    stride,
                    first.byte_length,
                    address_dependencies,
                )
            }
            StructuralTypeShape::ByteSequence(_) | StructuralTypeShape::ElementView { .. } => {
                let mut captured = references
                    .sequence(&range)
                    .cloned()
                    .ok_or("handle descriptor has no captured data window")?;
                captured.dependencies.extend(address_dependencies);
                Ok(captured)
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
            let shape = terminal_psi::find_structural_type(&self.module.structural_types, root)
                .map(|declaration| match &declaration.shape {
                    shape @ (StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::ByteSequence(_)
                    | StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::Reference { .. }
                    | StructuralTypeShape::PrimitiveScalar(_)) => format!("{shape:?}"),
                    _ => "aggregate (retain its original type identity)".into(),
                });
            format!(
                "handle argument {:?}/{place:?}/{path:?} has no exact storage coordinate: root {root:?} ({:?}), shape {shape:?}",
                self.body.id,
                self.carrier_identity(root),
            )
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

    pub(super) fn carrier_identity(&self, carrier: StructuralTypeId) -> Option<&str> {
        let declaration =
            terminal_psi::find_structural_type(&self.module.structural_types, carrier)?;
        self.module.identities.text(declaration.identity)
    }

    pub(super) fn stored_carrier_access(
        &self,
        carrier: StructuralTypeId,
    ) -> Option<terminal_psi::StructuralAccess> {
        let declaration =
            terminal_psi::find_structural_type(&self.module.structural_types, carrier)?;
        terminal_semantics::stored_reference_access(&declaration.shape)
    }

    pub(super) fn borrowed_root_access(
        &self,
        place: PlaceId,
    ) -> Option<terminal_psi::StructuralAccess> {
        self.bindings
            .rows
            .get(place)
            .and_then(Option::as_ref)
            .map(|slot| slot.access)
    }

    pub(super) fn matches_carrier(
        &self,
        actual: Option<StructuralTypeId>,
        expected: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    ) -> bool {
        unchanged_storage_presentation(&self.module.structural_types, actual, expected, access)
    }

    pub(super) fn matches_resident_copy(
        &self,
        operation: &Operation,
        transfer: &ResidentValueTransfer,
        carriers: (Option<StructuralTypeId>, Option<StructuralTypeId>),
    ) -> Result<bool, String> {
        if carriers.0.is_some() && carriers.0 == carriers.1 {
            return Ok(true);
        }
        // Borrowed roots retain their actual declaration access in the same
        // bounded binding table used by physical resolution. No access is
        // guessed for an owned canonical descriptor with no stored ceiling.
        let root_access = self.borrowed_root_access(transfer.source.root);
        Ok(shared_leaf_copy_presentation(
            &self.module.structural_types,
            operation,
            transfer,
            self.place_type(&transfer.source)?,
            carriers,
            root_access,
        ))
    }

    /// A checked byte write changes data behind its actual mutable view.
    /// The canonical operation still owns access and bounds admission. This
    /// query supplies only the possible physical bytes and captured address
    /// dependencies; callers must use a weak update even for an exact point.
    pub(super) fn byte_write(
        &self,
        location: &DescriptorLocation,
        references: &HandleReferences,
        index: Option<u64>,
    ) -> Result<(HandleStorageRange, BTreeSet<HandleInstance>), String> {
        if !self.is_byte_view(location)? {
            return Err("handle byte write has no original byte-view carrier".into());
        }
        let captured = self.sequence(location, references)?;
        Ok((captured.byte_selection_range(index)?, captured.dependencies))
    }

    /// A runtime-selected write can touch any member of its first collection.
    /// Return that containing interval, not a guessed element address. The
    /// caller must retain old dependencies and invalidate exact overlapping
    /// words, because this is a possible write rather than a full replacement.
    pub(super) fn possible_write(
        &self,
        query: &terminal_semantics::descriptor_lifetimes::DescriptorContentsQuery,
        references: &HandleReferences,
    ) -> Result<(HandleStorageRange, BTreeSet<HandleInstance>), String> {
        use terminal_semantics::descriptor_lifetimes::DescriptorContentsStep;
        let mut location = DescriptorLocation::whole(query.root);
        for (ordinal, step) in query.steps.iter().enumerate() {
            match step {
                DescriptorContentsStep::Address(segment) => location.path.push(segment.clone()),
                DescriptorContentsStep::AnyElement { .. } => {
                    if query.steps[ordinal + 1..].iter().any(|step| {
                        matches!(
                            step,
                            DescriptorContentsStep::Address(DescriptorPathSegment::Referent)
                        )
                    }) {
                        return Err(
                            "unknown handle write cannot select a resident reference's pointee"
                                .into(),
                        );
                    }
                    let captured = self.sequence(&location, references)?;
                    return Ok((captured.range, captured.dependencies));
                }
            }
        }
        self.resolve(&location, references)
            .map(|(range, _, _, dependencies)| (range, dependencies))
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
            BTreeSet<HandleInstance>,
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
            return Ok((range, selected, None, captured.dependencies));
        }
        if let Some(referent) = last_referent {
            let slot = DescriptorLocation {
                root: location.root,
                path: location.path[..referent].to_vec(),
            };
            let (slot_range, _, _, mut address_dependencies) = self.resolve(&slot, references)?;
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
            address_dependencies.extend(&captured.dependencies);
            return Ok((
                range,
                selected,
                captured.stored_scalar,
                address_dependencies,
            ));
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
            return Ok((
                range,
                selected,
                captured.stored_scalar,
                captured.address_dependencies.clone(),
            ));
        }
        let backing = HandleStorageBacking {
            owner: HandleStorageOwner::Activation(self.activation),
            place: location.root,
        };
        Ok((
            HandleStorageRange::new(backing, selected.byte_offset, selected.byte_length)?,
            selected,
            None,
            BTreeSet::new(),
        ))
    }

    pub(super) fn scalar(
        &self,
        location: &DescriptorLocation,
        references: &HandleReferences,
        expected: ScalarType,
    ) -> Result<HandleScalarStorage, String> {
        let (range, selected, codec, address_dependencies) = self.resolve(location, references)?;
        let scalar = selected
            .scalar
            .ok_or("handle scalar observation selects an aggregate region")?;
        if scalar.logical_scalar != expected {
            return Err("handle scalar observation changes its original logical carrier".into());
        }
        Ok(HandleScalarStorage {
            address_dependencies,
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
        mut dependencies: BTreeSet<HandleInstance>,
        mut require_active: impl FnMut(&DescriptorLocation) -> Result<(), String>,
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
        require_active(&source)?;
        let (backing, _, inherited_codec, address_dependencies) =
            self.resolve(&source, references)?;
        dependencies.extend(address_dependencies);
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
        let (carrier, _, _, _) =
            self.resolve(&DescriptorLocation::whole(result.place), references)?;
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
    fn windows_handle_replay_borrowed_bindings_share_unchanged_slots_and_isolate_mutations() {
        use terminal_psi::StructuralAccess;
        let first = PlaceId::new(u64::MAX - 128).unwrap();
        let last = PlaceId::new(u64::MAX).unwrap();
        let hole = PlaceId::new(first.get() + 1).unwrap();
        let rows = IdTable::from_rows_with_gaps(
            [
                (
                    first,
                    Some(BorrowedStorageSlot {
                        access: StructuralAccess::SharedBorrow,
                        value: None,
                    }),
                ),
                (
                    last,
                    Some(BorrowedStorageSlot {
                        access: StructuralAccess::MutableBorrow,
                        value: None,
                    }),
                ),
            ],
            129,
        )
        .unwrap();
        let mut original = HandleStorageBindings {
            rows: Arc::new(rows),
        };
        let source = CapturedHandleStorage {
            address_dependencies: BTreeSet::from([HandleInstance(7)]),
            range: range(9, 8, 16),
            stored_scalar: None,
        };
        assert!(original.get(first).is_err());
        assert_eq!(original.get(hole).unwrap(), None);
        original
            .bind(first, StructuralAccess::SharedBorrow, source.clone())
            .unwrap();
        let mut sibling = original.clone();
        assert!(Arc::ptr_eq(&original.rows, &sibling.rows));
        assert_eq!(original, sibling);
        assert!(
            sibling
                .bind(first, StructuralAccess::MutableBorrow, source.clone())
                .is_err()
        );
        assert!(
            sibling
                .bind(hole, StructuralAccess::SharedBorrow, source.clone())
                .is_err()
        );
        assert!(Arc::ptr_eq(&original.rows, &sibling.rows));
        assert_eq!(
            original, sibling,
            "refusals preserve the complete binding state"
        );
        let other = CapturedHandleStorage {
            range: range(10, 4, 8),
            ..source.clone()
        };
        sibling
            .bind(last, StructuralAccess::MutableBorrow, other.clone())
            .unwrap();
        assert!(!Arc::ptr_eq(&original.rows, &sibling.rows));
        assert_ne!(original, sibling);
        assert_eq!(sibling.get(first).unwrap(), Some(&source));
        assert_eq!(sibling.get(last).unwrap(), Some(&other));
        assert!(original.get(last).is_err());
        assert_eq!(sibling.get(hole).unwrap(), None);
        let independent = HandleStorageBindings {
            rows: Arc::new((*original.rows).clone()),
        };
        assert!(!Arc::ptr_eq(&original.rows, &independent.rows));
        assert_eq!(
            original, independent,
            "allocation identity is not semantic identity"
        );
        assert_eq!(original.cmp(&independent), std::cmp::Ordering::Equal);
    }

    #[test]
    fn borrowed_descriptor_identity_does_not_authorize_repacking_or_owned_transfer() {
        use semantic_vocabulary::{IntegerSign, IntegerType};
        use terminal_psi::{ByteSequenceCarrier, StructuralAccess, StructuralTypeDeclaration};
        let id = |value| StructuralTypeId::new(value).unwrap();
        let declaration = |value: u32, shape| StructuralTypeDeclaration {
            requires: Vec::new(),
            id: id(u64::from(value)),
            identity: terminal_psi::TerminalIdentityIndex(value),
            shape,
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
        };
        let types = [
            declaration(
                1,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                }),
            ),
            declaration(
                2,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                }),
            ),
            declaration(
                3,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    access: None,
                    capacity: 12,
                }),
            ),
            declaration(
                4,
                StructuralTypeShape::FixedArray {
                    element: id(5),
                    length: terminal_psi::ArrayExtent::Literal(12),
                },
            ),
            declaration(
                5,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                )),
            ),
            declaration(
                6,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                }),
            ),
            declaration(
                7,
                StructuralTypeShape::FixedArray {
                    element: id(5),
                    length: terminal_psi::ArrayExtent::Literal(12),
                },
            ),
        ];
        assert!(unchanged_storage_presentation(
            &types,
            Some(id(1)),
            id(2),
            StructuralAccess::SharedBorrow,
        ));
        assert!(!unchanged_storage_presentation(
            &types,
            Some(id(1)),
            id(2),
            StructuralAccess::Owned,
        ));
        assert!(!unchanged_storage_presentation(
            &types,
            None,
            id(2),
            StructuralAccess::SharedBorrow,
        ));
        // Terminal permits a separately produced pointer to these inline
        // bytes. Passing their first word through unchanged would lose that
        // backing, so this replay cannot accept the presentation as a copy.
        assert_eq!(
            terminal_semantics::borrowed_referent_presentation(types.iter(), id(4), id(3),),
            Some(
                terminal_semantics::BorrowedReferentPresentation::AddressOfBacking {
                    byte_count: 12,
                }
            )
        );
        assert!(!unchanged_storage_presentation(
            &types,
            Some(id(4)),
            id(3),
            StructuralAccess::SharedBorrow,
        ));

        // Copying an already shared descriptor is a separate admitted
        // operation from presenting an owned argument under another type.
        // Bind the actual source, result and occurrence multiplicity here.
        use semantic_vocabulary::OperationId;
        use terminal_psi::{OperationResult, StructuralMultiplicity, StructuralOperationResult};
        let source = PlaceId::new(1).unwrap();
        let destination = PlaceId::new(2).unwrap();
        let mut operation = Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Structural(StructuralOperationResult {
                place: destination,
                structural_type: id(2),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::StructuralLeafCopy {
                source,
                path: Vec::new(),
            },
        );
        let mut transfer = ResidentValueTransfer {
            source: DescriptorLocation::whole(source),
            destination: DescriptorLocation::whole(destination),
        };
        let permits =
            |operation: &Operation, transfer: &ResidentValueTransfer, source_type, root_access| {
                let destination_type = operation.result.structural().unwrap().structural_type;
                shared_leaf_copy_presentation(
                    &types,
                    operation,
                    transfer,
                    source_type,
                    (Some(source_type), Some(destination_type)),
                    root_access,
                )
            };
        assert!(permits(&operation, &transfer, id(1), None));
        let mut other_writer = operation.clone();
        other_writer.kind = OperationKind::MoveStructuralValue {
            source: terminal_psi::StructuralArgument::whole(source, StructuralAccess::Owned),
        };
        assert!(!permits(&other_writer, &transfer, id(1), None));
        assert!(permits(
            &operation,
            &transfer,
            id(2),
            Some(StructuralAccess::SharedBorrow),
        ));
        // Shape alone cannot copy an exclusive stored descriptor, infer a
        // canonical source's access or open write-only source storage.
        assert!(!permits(&operation, &transfer, id(6), None));
        assert!(!permits(&operation, &transfer, id(2), None));
        assert!(!permits(
            &operation,
            &transfer,
            id(1),
            Some(StructuralAccess::WriteOnlyBorrow),
        ));
        transfer.destination = DescriptorLocation::whole(PlaceId::new(3).unwrap());
        assert!(!permits(&operation, &transfer, id(1), None));
        transfer.destination = DescriptorLocation::whole(destination);
        transfer.source = DescriptorLocation::whole(PlaceId::new(3).unwrap());
        assert!(!permits(&operation, &transfer, id(1), None));
        transfer.source = DescriptorLocation::whole(source);
        transfer.source.path.push(DescriptorPathSegment::Field(
            semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
        ));
        assert!(!permits(&operation, &transfer, id(1), None));
        transfer.source = DescriptorLocation::whole(source);
        // The canonical result's affine occurrence denotes exclusive access,
        // so a shared input cannot silently change that backing permission.
        if let OperationResult::Structural(result) = &mut operation.result {
            result.multiplicity = StructuralMultiplicity::Affine;
        }
        assert!(!permits(&operation, &transfer, id(1), None));
        if let OperationResult::Structural(result) = &mut operation.result {
            result.multiplicity = StructuralMultiplicity::Unrestricted;
            result.structural_type = id(3);
        }
        assert!(!permits(&operation, &transfer, id(4), None));
        if let OperationResult::Structural(result) = &mut operation.result {
            result.structural_type = id(7);
        }
        assert!(!permits(&operation, &transfer, id(4), None));
        // No standalone source identity is reconstructed for an unknown
        // inline endpoint, even when the expected descriptor shape is known.
        assert!(!shared_leaf_copy_presentation(
            &types,
            &operation,
            &transfer,
            id(4),
            (None, Some(id(7))),
            None,
        ));
    }

    #[test]
    fn scalar_read_through_a_retired_handle_derived_address_keeps_its_use() {
        use super::super::windows_handle_lifetimes::{
            HandleInstance, HandleLifetimes, NativeOccurrence, ScalarOccurrence, ScalarUseSite,
        };
        use semantic_vocabulary::{
            BoundaryMachineId, IntegerSign, IntegerType, InvocationSite, MachineId, OperationId,
        };
        let native = |operation| NativeOccurrence {
            machine: MachineId::new(1).unwrap(),
            site: InvocationSite::Operation(OperationId::new(operation).unwrap()),
            context: [1; 32],
            boundary: BoundaryMachineId::new(1).unwrap(),
        };
        let handle = HandleInstance(0);
        let mut ledger = HandleLifetimes::default();
        ledger.acquire_file(handle, native(1), 0).unwrap();
        ledger.release_file(handle, native(2)).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let selected = HandleScalarStorage {
            range: range(3, 0, 8),
            stored_scalar: scalar_type,
            logical_scalar: scalar_type,
            address_dependencies: BTreeSet::from([handle]),
        };
        // The pointee contains no HANDLE. Its captured address still carries
        // the retired instance, independently of current pointee contents.
        let word = selected.read(&HandleAliases::default());
        assert_eq!(word.dependencies(), &BTreeSet::from([handle]));
        assert!(word.exact_handle().is_err());
        assert!(
            ledger
                .observe_scalars(
                    word.dependencies(),
                    ScalarOccurrence {
                        machine: MachineId::new(1).unwrap(),
                        site: ScalarUseSite::Operation(OperationId::new(3).unwrap()),
                        context: [1; 32],
                    }
                )
                .is_err()
        );
    }

    #[test]
    fn captured_windows_do_not_recover_an_unknown_selected_address() {
        use super::super::windows_handle_lifetimes::{
            HandleLifetimes, NativeOccurrence, ScalarOccurrence, ScalarUseSite,
        };
        use semantic_vocabulary::{
            BoundaryMachineId, IntegerSign, IntegerType, InvocationSite, MachineId, OperationId,
        };
        let byte = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let handle = HandleInstance(0);
        let id = |value| StructuralTypeId::new(value).unwrap();
        let wide = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
        let types = [
            (1, StructuralTypeShape::PrimitiveScalar(wide)),
            (
                2,
                StructuralTypeShape::FixedArray {
                    element: id(1),
                    length: terminal_psi::ArrayExtent::Literal(3),
                },
            ),
        ]
        .map(|(value, shape)| terminal_psi::StructuralTypeDeclaration {
            requires: Vec::new(),
            id: id(value),
            identity: terminal_psi::TerminalIdentityIndex(value as u32),
            shape,
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
        });
        let mut geometry = ReferenceStorageGeometry::new(types.iter(), |application| {
            Err(StorageGeometryError::UnresolvedExtent(application))
        });
        let whole = geometry.region(id(2), &[]).unwrap();
        assert_eq!(whole.array, None, "whole array is not an explicit window");
        let array = CapturedHandleSequence::from_complete_array(
            range(7, 40, 6),
            whole,
            id(1),
            3,
            2,
            2,
            BTreeSet::from([handle]),
        )
        .unwrap();
        assert_eq!(array.element_range(2, 2).unwrap(), range(7, 44, 2));
        assert_eq!(array.length, Some(3));
        assert_eq!(array.dependencies, BTreeSet::from([handle]));
        assert!(array.element_range(3, 2).is_err());
        let window = geometry
            .region(
                id(2),
                &[terminal_psi::StructuralPathSegment::FixedRange { start: 1, end: 3 }],
            )
            .unwrap();
        assert!(window.array.is_some());
        // Even an integral byte extent does not give a region an element
        // namespace. A separately checked representation must establish it.
        assert!(
            CapturedHandleSequence::from_complete_array(
                range(7, 42, 4),
                window,
                id(1),
                2,
                2,
                2,
                BTreeSet::new(),
            )
            .is_err()
        );
        for (actual, count, stride, element_size) in [
            (range(7, 40, 8), 3, 2, 2),
            (range(7, 40, 6), 3, 1, 2),
            (range(7, 40, 6), 3, 2, 3),
            (range(7, 40, 6), u64::MAX, 2, 2),
        ] {
            assert!(
                CapturedHandleSequence::from_complete_array(
                    actual,
                    whole,
                    id(1),
                    count,
                    stride,
                    element_size,
                    BTreeSet::new(),
                )
                .is_err()
            );
        }
        let scalar = geometry.region(id(1), &[]).unwrap();
        assert!(
            CapturedHandleSequence::from_complete_array(
                range(7, 40, 2),
                scalar,
                id(1),
                1,
                2,
                2,
                BTreeSet::new(),
            )
            .is_err()
        );
        let source = CapturedHandleSequence {
            range: range(1, 32, 24),
            element: None,
            stride: 1,
            length: Some(24),
            precise: true,
            dependencies: BTreeSet::from([handle]),
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

        let carrier = DescriptorLocation::whole(PlaceId::new(3).unwrap());
        let HandleScalarSelection::Family {
            location,
            range: byte_range,
            dependencies,
        } = source
            .clone()
            .byte_scalar_selection(carrier.clone(), byte, Some(5))
            .unwrap()
        else {
            panic!("byte data selection cannot claim an exact whole HANDLE");
        };
        assert_eq!(location, carrier);
        assert_eq!(byte_range, range(1, 37, 1));
        assert_eq!(dependencies, BTreeSet::from([handle]));
        for (window, index) in [(source.clone(), None), (unknown.clone(), Some(0))] {
            let HandleScalarSelection::Family { range, .. } = window
                .byte_scalar_selection(carrier.clone(), byte, index)
                .unwrap()
            else {
                panic!("unknown byte address must keep its possible data family");
            };
            assert_eq!(range, source.range);
        }
        assert!(
            source
                .clone()
                .byte_scalar_selection(carrier.clone(), byte, Some(24))
                .is_err()
        );
        let mut missing_length = source.clone();
        missing_length.length = None;
        assert!(
            missing_length
                .clone()
                .byte_scalar_selection(carrier.clone(), byte, Some(0))
                .is_err()
        );
        let wide = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        assert!(
            source
                .clone()
                .byte_scalar_selection(carrier.clone(), wide, Some(0))
                .is_err()
        );
        let mut wrong_stride = source.clone();
        wrong_stride.stride = 2;
        assert!(
            wrong_stride
                .clone()
                .byte_scalar_selection(carrier.clone(), byte, Some(0))
                .is_err()
        );
        let mut other_elements = source.clone();
        other_elements.element = Some(StructuralTypeId::new(1).unwrap());
        assert!(
            other_elements
                .clone()
                .byte_scalar_selection(carrier, byte, Some(0))
                .is_err()
        );

        // A point write and point read use the same actual data window.
        // Weak writes keep old dependencies even when the incoming byte is
        // untracked. An unknown point cannot erase any possible old alias.
        assert_eq!(source.byte_selection_range(Some(5)).unwrap(), byte_range);
        assert_eq!(source.byte_selection_range(None).unwrap(), source.range);
        assert_eq!(unknown.byte_selection_range(Some(5)).unwrap(), source.range);
        assert!(source.byte_selection_range(Some(24)).is_err());
        assert!(missing_length.byte_selection_range(Some(0)).is_err());
        assert!(wrong_stride.byte_selection_range(None).is_err());
        assert!(other_elements.byte_selection_range(None).is_err());
        let neighbor = range(1, 64, 8);
        let old_word = range(1, 32, 8);
        let mut aliases = HandleAliases::with_scalar_namespace([]).unwrap();
        for home in [old_word, neighbor] {
            aliases
                .write_complete_word(
                    home,
                    wide,
                    HandleWordAliases::acquired(wide, handle).unwrap(),
                )
                .unwrap();
        }
        aliases.write_region(
            source.byte_selection_range(Some(5)).unwrap(),
            BTreeSet::new(),
        );
        let partial = aliases.stored_word(&old_word, wide);
        assert!(partial.exact_handle().is_err());
        assert_eq!(partial.dependencies(), &BTreeSet::from([handle]));
        assert_eq!(
            aliases.stored_word(&neighbor, wide).exact_handle().unwrap(),
            handle
        );
        aliases.write_region(source.byte_selection_range(None).unwrap(), BTreeSet::new());
        assert_eq!(
            aliases.stored_dependencies(&old_word),
            BTreeSet::from([handle])
        );
        assert_eq!(
            aliases.stored_word(&neighbor, wide).exact_handle().unwrap(),
            handle
        );

        // A byte that carries an address dependency still observes that
        // resource. Selecting it cannot discard the dependency after close.
        let native = |ordinal| NativeOccurrence {
            machine: MachineId::new(1).unwrap(),
            site: InvocationSite::Operation(OperationId::new(ordinal).unwrap()),
            context: [1; 32],
            boundary: BoundaryMachineId::new(1).unwrap(),
        };
        let occurrence = ScalarOccurrence {
            machine: MachineId::new(1).unwrap(),
            site: ScalarUseSite::Operation(OperationId::new(3).unwrap()),
            context: [1; 32],
        };
        let word = HandleWordAliases::derived_from_dependencies(byte, dependencies);
        assert!(word.exact_handle().is_err());
        let mut ledger = HandleLifetimes::default();
        ledger.acquire_file(handle, native(1), 0).unwrap();
        assert!(
            ledger
                .observe_scalars(word.dependencies(), occurrence)
                .is_ok()
        );
        ledger.release_file(handle, native(2)).unwrap();
        assert!(
            ledger
                .observe_scalars(word.dependencies(), occurrence)
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
