//! Resolve handle-bearing bytes through the actual captured reference view.
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

/// The scalar's logical result and physical decoding are independently
/// retained. Reading bits through a noninteger or narrow codec loses exact
/// identity before any later widening of the logical value.
pub(super) struct HandleScalarStorage {
    pub range: HandleStorageRange,
    pub stored_scalar: ScalarType,
    pub logical_scalar: ScalarType,
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
}

impl HandleReferences {
    pub(super) fn get(&self, location: &HandleStorageRange) -> Option<&CapturedHandleReference> {
        self.values.get(location)
    }

    pub(super) fn replace(&mut self, location: HandleStorageRange, value: CapturedHandleReference) {
        self.retire(&location);
        self.values.insert(location, value);
    }

    pub(super) fn retire(&mut self, location: &HandleStorageRange) {
        self.values.retain(|slot, _| !location.overlaps(slot));
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
        self.retire(destination);
        self.values.extend(values);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CapturedHandleReferences {
    byte_length: u64,
    values: Vec<(RelativeHandleRange, CapturedHandleReference)>,
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
        if let Some(referent) = location
            .path
            .iter()
            .rposition(|part| *part == DescriptorPathSegment::Referent)
        {
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
            Some(scalar) => u64::from(
                terminal_semantics::reference_storage_byte_width(scalar)
                    .ok_or("handle reference's selected scalar codec has no closed width")?,
            ),
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
