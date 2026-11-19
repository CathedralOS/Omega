/*
We manage one physical shared-memory allocation per authenticated interpreter
root in the native provider sponsor process.

Views only select subranges inside an established original home; neither
repeated loans nor argument snapshots allocate a replacement. Resident scalar
backings keep this sponsor alive across moves and suspension, while pointer
slots are linked and verified inside the worker process so parent mapping
addresses never cross IPC.
*/

use super::super::scalar_codec::{decode, encode, scalar_extent};
use native_provider_runtime::{
    NativeHomeId, NativePointerSlot, NativePointerTarget, NativeProviderError,
    NativeProviderProcess,
};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, ScalarType, StructuralTypeId};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
use terminal_interpreter::{
    TerminalEffectRejection, TerminalOriginalScalarBackings, TerminalOriginalScalarValue,
    TerminalOriginalStorageLoan, TerminalOriginalStorageRequest, TerminalOriginalStorageRoot,
    TerminalScalarBacking, TerminalScalarValue,
};

pub(in super::super) fn rejection(reason: impl Into<String>) -> TerminalEffectRejection {
    TerminalEffectRejection::new(reason)
}

pub(super) struct Sponsor {
    command: Option<native_provider_runtime::NativeProviderCommand>,
    process: Option<NativeProviderProcess>,
    failure: Option<String>,
    references: Rc<super::provider_results::LeaseRegistry>,
}

impl Sponsor {
    pub fn with<T>(
        &mut self,
        f: impl FnOnce(&mut NativeProviderProcess) -> Result<T, NativeProviderError>,
    ) -> Result<T, TerminalEffectRejection> {
        if let Some(reason) = &self.failure {
            return Err(rejection(reason.clone()));
        }
        if self.process.is_none() {
            let command = self
                .command
                .take()
                .ok_or_else(|| rejection("no explicit native provider sponsor"))?;
            match command.start(Duration::from_secs(30)) {
                Ok(process) => self.process = Some(process),
                Err(error) => {
                    self.failure = Some(error.to_string());
                    return Err(rejection(error.to_string()));
                }
            }
        }
        self.drain_released().map_err(|error| {
            self.failure = Some(error.to_string());
            rejection(error.to_string())
        })?;
        f(self.process.as_mut().unwrap()).map_err(|error| {
            self.failure = Some(error.to_string());
            rejection(error.to_string())
        })
    }

    fn existing(&mut self) -> Result<&mut NativeProviderProcess, NativeProviderError> {
        if let Some(reason) = &self.failure {
            return Err(NativeProviderError {
                reason: reason.clone(),
                status: None,
            });
        }
        self.process.as_mut().ok_or_else(|| NativeProviderError {
            reason: "native provider incarnation is not live".into(),
            status: None,
        })
    }

    fn drain_released(&mut self) -> Result<(), NativeProviderError> {
        for reference in self.references.take_released() {
            self.existing()?.release_provider_reference(reference)?;
        }
        Ok(())
    }

    fn completed<T>(
        &mut self,
        result: Result<T, NativeProviderError>,
    ) -> Result<T, NativeProviderError> {
        if self.failure.is_none()
            && let Err(error) = &result
        {
            self.failure = Some(error.to_string());
        }
        result
    }

    pub fn call(
        &mut self,
        adapter: native_provider_runtime::NativeAdapterId,
        arguments: &[native_provider_runtime::NativeArgument<'_>],
    ) -> Result<Vec<u8>, NativeProviderError> {
        let result = self
            .drain_released()
            .and_then(|()| self.existing()?.call(adapter, arguments));
        self.completed(result)
    }

    fn call_provider_reference(
        &mut self,
        adapter: native_provider_runtime::NativeAdapterId,
        arguments: &[native_provider_runtime::NativeArgument<'_>],
    ) -> Result<native_provider_runtime::NativeProviderReference, NativeProviderError> {
        let result = self
            .drain_released()
            .and_then(|()| self.existing()?.call_provider_reference(adapter, arguments));
        self.completed(result)
    }

    pub(super) fn read_provider_reference(
        &mut self,
        reference: native_provider_runtime::NativeProviderReference,
        offset: usize,
        output: &mut [u8],
    ) -> Result<(), TerminalEffectRejection> {
        // A backing read may only use its installed incarnation. It must not
        // launch a replacement worker when an old reference becomes stale.
        let result = self.drain_released().and_then(|()| {
            self.existing()?
                .read_provider_reference(reference, offset, output)
        });
        self.completed(result)
            .map_err(|error| rejection(error.to_string()))
    }

    pub fn finish(&mut self) -> Result<(), TerminalEffectRejection> {
        // This check also precedes failure teardown: an error cannot erase a
        // still-observable reference's lease accounting.
        if self.references.active() != 0 {
            return Err(rejection(
                "native provider incarnation still has live result references",
            ));
        }
        if self.failure.is_some() {
            self.references.take_released();
            self.process.take();
            return Ok(());
        }
        let result = self
            .drain_released()
            .map_err(|error| rejection(error.to_string()))
            .and_then(|()| match self.process.take() {
                Some(process) => process
                    .close()
                    .map_err(|error| rejection(error.to_string())),
                None => Ok(()),
            });
        self.failure = Some("native provider incarnation has retired".into());
        result
    }
}

#[derive(Clone)]
pub(in super::super) struct NativeContext {
    pub(super) sponsor: Rc<RefCell<Sponsor>>,
    homes: Rc<RefCell<BTreeMap<(u64, u64), OriginalHome>>>,
    available: bool,
    services: crate::execution::TerminalNativeServiceDelegation,
    // Shared with the emulated allocator so host and emulated addresses stay disjoint.
    addresses: Arc<Mutex<super::super::allocation::AllocationValues>>,
}

struct OriginalHome {
    root: TerminalOriginalStorageRoot,
    structural_type: Option<StructuralTypeId>, // None for raw byte allocations; Some for typed roots.
    home: NativeHomeId,
    extent: usize,
    alignment: usize,
    cells: BTreeMap<Vec<Path>, Rc<Resident>>,
    references: BTreeMap<Vec<Path>, (usize, Option<NativePointerTarget>)>,
}

struct Resident {
    sponsor: Rc<RefCell<Sponsor>>,
    home: NativeHomeId,
    offset: usize,
    stored: ScalarType,
    logical: ScalarType,
}

impl std::fmt::Debug for Resident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeOriginalScalar")
            .field("home", &self.home)
            .field("offset", &self.offset)
            .finish_non_exhaustive()
    }
}

impl TerminalScalarBacking for Resident {
    fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        let size = scalar_extent(self.stored)?;
        let mut bytes = [0; 16];
        self.sponsor
            .borrow_mut()
            .with(|process| process.read_home(self.home, self.offset, &mut bytes[..size]))?;
        decode(&bytes[..size], self.stored, self.logical)
    }

    fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
        if value.scalar_type() != self.logical {
            return Err(rejection("native resident logical carrier changed"));
        }
        let bytes = encode(value, self.stored)?;
        let size = scalar_extent(self.stored)?;
        self.sponsor
            .borrow_mut()
            .with(|process| process.write_home(self.home, self.offset, &bytes[..size]))
    }
}

impl NativeContext {
    pub fn new(
        command: Option<native_provider_runtime::NativeProviderCommand>,
        services: crate::execution::TerminalNativeServiceDelegation,
        addresses: Arc<Mutex<super::super::allocation::AllocationValues>>,
    ) -> Self {
        Self {
            available: command.is_some(),
            services,
            addresses,
            sponsor: Rc::new(RefCell::new(Sponsor {
                command,
                process: None,
                failure: None,
                references: Rc::new(super::provider_results::LeaseRegistry::default()),
            })),
            homes: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }

    pub fn available(&self) -> bool {
        self.available
    }

    pub fn services(&self) -> crate::execution::TerminalNativeServiceDelegation {
        self.services
    }

    pub fn finish(&self) -> Result<(), TerminalEffectRejection> {
        self.sponsor.borrow_mut().finish()
    }

    pub(super) fn call_provider_reference(
        &self,
        adapter: native_provider_runtime::NativeAdapterId,
        arguments: &[native_provider_runtime::NativeArgument<'_>],
    ) -> Result<Rc<super::provider_results::Lease>, NativeProviderError> {
        let mut sponsor = self.sponsor.borrow_mut();
        let result = sponsor
            .call_provider_reference(adapter, arguments)
            .and_then(|reference| {
                sponsor
                    .references
                    .retain(reference, Rc::clone(&self.sponsor))
            });
        sponsor.completed(result)
    }

    // A native import may receive only an address the host itself returned.
    pub(super) fn host_address_argument(
        &self,
        value: TerminalScalarValue,
    ) -> Result<Vec<u8>, TerminalEffectRejection> {
        let address = host_address_value(value)?;
        let addresses = self.addresses.lock().map_err(|_| poisoned_addresses())?;
        if !addresses.is_host_address(address) {
            return Err(rejection(
                "native import argument is not an address the host returned",
            ));
        }
        Ok(address.to_le_bytes().to_vec())
    }

    pub(super) fn host_address_result(
        &self,
        bytes: &[u8],
        scalar: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        let ScalarType::Integer(scalar_type) = scalar else {
            return Err(rejection("host address result lost its address carrier"));
        };
        let address = u64::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| rejection("host address result has the wrong width"))?,
        );
        self.addresses
            .lock()
            .map_err(|_| poisoned_addresses())?
            .admit_host_address(address)?;
        // Null names no storage and carries no provenance; every other value
        // keeps its host provenance so the interpreter refuses to mix it with
        // its own addresses.
        Ok(if address == 0 {
            TerminalScalarValue::Integer {
                scalar_type,
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
            }
        } else {
            TerminalScalarValue::HostAddress {
                scalar_type,
                address,
            }
        })
    }

    pub(super) fn prepare(
        &self,
        requests: &[TerminalOriginalStorageRequest],
        layouts: &[NativeBorrowedStorageLayout],
        codecs: &BTreeMap<StructuralTypeId, super::values::StorageCodec>,
    ) -> Result<TerminalOriginalScalarBackings, TerminalEffectRejection> {
        let mut result = Vec::with_capacity(requests.len());
        let mut homes = self.homes.borrow_mut();
        let mut established_here = BTreeSet::new();
        for request in requests {
            let layout = if request.byte_extent().is_some() {
                None
            } else {
                Some(exact_layout(layouts, request.structural_type())?)
            };
            let (extent, alignment) = match (request.byte_extent(), layout) {
                (Some(extent), None) if request.references().is_empty() => (extent, 1),
                (None, Some(layout)) => (layout.byte_size as usize, usize::from(layout.alignment)),
                _ => {
                    return Err(rejection(
                        "original byte backing contains unsupported reference slots",
                    ));
                }
            };
            let structural_type = layout.map(|_| request.structural_type());
            let root = request.root();
            let key = (root.namespace(), root.allocation());
            if let std::collections::btree_map::Entry::Vacant(entry) = homes.entry(key) {
                let home = self
                    .sponsor
                    .borrow_mut()
                    .with(|process| process.create_home(extent.max(1), alignment))?;
                entry.insert(OriginalHome {
                    root,
                    structural_type,
                    home,
                    extent,
                    alignment,
                    cells: BTreeMap::new(),
                    references: BTreeMap::new(),
                });
            }
            let original = homes.get_mut(&key).unwrap();
            if original.root != root
                || original.structural_type != structural_type
                || original.extent != extent
                || original.alignment != alignment
            {
                return Err(rejection(
                    "original native root changed artifact, type or geometry",
                ));
            }
            for value in request.allocation_contents() {
                self.prepare_scalar(
                    original,
                    layout,
                    extent,
                    &mut established_here,
                    value,
                    false,
                )?;
            }
            let mut row = Vec::with_capacity(request.values().len());
            for value in request.values() {
                let backing = self.prepare_scalar(
                    original,
                    layout,
                    extent,
                    &mut established_here,
                    value,
                    true,
                )?;
                row.push(backing.ok_or_else(|| {
                    rejection("original scalar lost its returned resident backing")
                })?);
            }
            for reference in request.references() {
                let offset = reference_location(
                    layout.ok_or_else(|| {
                        rejection("byte owner cannot materialize reference slots")
                    })?,
                    reference.path(),
                )?;
                let entry = original
                    .references
                    .entry(reference.path().to_vec())
                    .or_insert((offset, None));
                if entry.0 != offset {
                    return Err(rejection(
                        "native reference slot changed original coordinates",
                    ));
                }
            }
            result.push(row);
        }
        drop(homes);
        for request in requests {
            for reference in request.references() {
                let codec = codecs.get(&reference.referent_type()).ok_or_else(|| {
                    rejection("native reference target has no exact produced codec")
                })?;
                let (home, offset) = self.loan(reference.target(), codec)?;
                let target = NativePointerTarget {
                    home,
                    offset,
                    byte_size: codec.extent,
                    alignment: codec.alignment,
                };
                let slot = self.reference_slot(
                    &request.loan(),
                    &reference.path()[request.referent().len()..],
                )?;
                self.sponsor
                    .borrow_mut()
                    .with(|process| process.link_pointer(slot, target))?;
                self.record_reference(slot, target)?;
            }
        }
        Ok(result)
    }

    fn prepare_scalar(
        &self,
        original: &mut OriginalHome,
        layout: Option<&NativeBorrowedStorageLayout>,
        extent: usize,
        established_here: &mut BTreeSet<(u64, u64, Vec<Path>)>,
        value: &TerminalOriginalScalarValue,
        resident: bool,
    ) -> Result<Option<Rc<dyn TerminalScalarBacking>>, TerminalEffectRejection> {
        let (offset, logical, stored) = match layout {
            Some(layout) => scalar_location(layout, value.path())?,
            None => {
                let [Path::FixedIndex(index)] = value.path() else {
                    return Err(rejection("original byte owner lost its exact byte index"));
                };
                let offset = usize::try_from(*index)
                    .map_err(|_| rejection("original byte index exceeds the host address space"))?;
                let logical = value.value().scalar_type();
                if offset >= extent
                    || !matches!(logical, ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == Some(8))
                {
                    return Err(rejection(
                        "original byte owner has an invalid byte coordinate or carrier",
                    ));
                }
                (offset, logical, logical)
            }
        };
        if value.value().scalar_type() != logical {
            return Err(rejection("original scalar differs from native recipe"));
        }
        if let Some(existing) = original.cells.get(value.path()) {
            let backing: Rc<dyn TerminalScalarBacking> = existing.clone();
            let key = (
                original.root.namespace(),
                original.root.allocation(),
                value.path().to_vec(),
            );
            if existing.offset != offset
                || existing.logical != logical
                || existing.stored != stored
                || match value.backing() {
                    Some(actual) => !Rc::ptr_eq(actual, &backing),
                    None => !established_here.contains(&key),
                }
                || backing.read()? != value.value()
            {
                return Err(rejection(
                    "original scalar no longer names the same resident home",
                ));
            }
            return Ok(Some(backing));
        }
        if value.backing().is_some() {
            return Err(rejection(
                "resident scalar belongs to a different original allocation",
            ));
        }
        if !resident {
            let bytes = encode(value.value(), stored)?;
            let size = scalar_extent(stored)?;
            self.sponsor
                .borrow_mut()
                .with(|process| process.write_home(original.home, offset, &bytes[..size]))?;
            return Ok(None);
        }
        let resident_cell = Rc::new(Resident {
            sponsor: self.sponsor.clone(),
            home: original.home,
            offset,
            logical,
            stored,
        });
        resident_cell.write(value.value())?;
        original
            .cells
            .insert(value.path().to_vec(), resident_cell.clone());
        established_here.insert((
            original.root.namespace(),
            original.root.allocation(),
            value.path().to_vec(),
        ));
        Ok(Some(resident_cell as Rc<dyn TerminalScalarBacking>))
    }

    pub(super) fn reference_slot(
        &self,
        loan: &TerminalOriginalStorageLoan,
        relative: &[Path],
    ) -> Result<NativePointerSlot, TerminalEffectRejection> {
        let root = loan.root();
        let homes = self.homes.borrow();
        let original = homes
            .get(&(root.namespace(), root.allocation()))
            .filter(|home| home.root == root)
            .ok_or_else(|| rejection("native reference slot lost its original root"))?;
        let mut path = loan.referent().to_vec();
        path.extend_from_slice(relative);
        let (offset, _) = original
            .references
            .get(&path)
            .ok_or_else(|| rejection("native reference slot was not captured"))?;
        Ok(NativePointerSlot {
            home: original.home,
            offset: *offset,
        })
    }

    pub(super) fn record_reference(
        &self,
        slot: NativePointerSlot,
        target: NativePointerTarget,
    ) -> Result<(), TerminalEffectRejection> {
        let mut homes = self.homes.borrow_mut();
        let home = homes
            .values_mut()
            .find(|home| home.home == slot.home)
            .ok_or_else(|| rejection("native reference lost its original home"))?;
        let mut found = false;
        for (offset, current) in home.references.values_mut() {
            if *offset == slot.offset {
                *current = Some(target);
                found = true;
            }
        }
        if !found {
            return Err(rejection("native reference has no captured slot"));
        }
        Ok(())
    }

    pub(super) fn validate_references(&self) -> Result<(), TerminalEffectRejection> {
        for home in self.homes.borrow().values() {
            for (offset, target) in home.references.values() {
                let target =
                    target.ok_or_else(|| rejection("native reference link was not initialized"))?;
                if !self.sponsor.borrow_mut().with(|process| {
                    process.pointer_matches(
                        NativePointerSlot {
                            home: home.home,
                            offset: *offset,
                        },
                        target,
                    )
                })? {
                    return Err(rejection(
                        "native provider changed a reference outside its captured completion selection",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn loan(
        &self,
        loan: &TerminalOriginalStorageLoan,
        view: &super::values::StorageCodec,
    ) -> Result<(NativeHomeId, usize), TerminalEffectRejection> {
        let root = loan.root();
        let homes = self.homes.borrow();
        let original = homes
            .get(&(root.namespace(), root.allocation()))
            .filter(|home| home.root == root)
            .ok_or_else(|| rejection("original native allocation is absent"))?;
        let mut base = None;
        for field in &view.fields {
            let path = loan.scalar_path(&field.path).ok_or_else(|| {
                rejection("native view scalar has no authenticated original path")
            })?;
            let cell = original
                .cells
                .get(&path)
                .ok_or_else(|| rejection("native view includes an unestablished scalar"))?;
            if cell.logical != field.logical || cell.stored != field.stored {
                return Err(rejection("native view changes the original scalar codec"));
            }
            let offset = cell
                .offset
                .checked_sub(field.offset)
                .ok_or_else(|| rejection("native view precedes its original allocation"))?;
            if base.is_some_and(|base| base != offset) {
                return Err(rejection(
                    "native view fields disagree on their original range",
                ));
            }
            base = Some(offset);
        }
        for field in &view.references {
            let mut path = loan.referent().to_vec();
            path.extend_from_slice(&field.path);
            let (actual, _) = original
                .references
                .get(&path)
                .ok_or_else(|| rejection("native view includes an unestablished reference slot"))?;
            let offset = actual.checked_sub(field.offset).ok_or_else(|| {
                rejection("native reference view precedes its original allocation")
            })?;
            if base.is_some_and(|base| base != offset) {
                return Err(rejection(
                    "native reference and scalar fields disagree on their original range",
                ));
            }
            base = Some(offset);
        }
        let base =
            base.ok_or_else(|| rejection("native view has no represented initialized contents"))?;
        if base % view.alignment != 0
            || original.alignment < view.alignment
            || base
                .checked_add(view.extent)
                .is_none_or(|end| end > original.extent)
        {
            return Err(rejection(
                "native view exceeds the original allocation geometry",
            ));
        }
        Ok((original.home, base))
    }

    pub(super) fn byte_loan(
        &self,
        loan: &TerminalOriginalStorageLoan,
    ) -> Result<(NativeHomeId, usize, usize), TerminalEffectRejection> {
        let (captured_offset, length) = loan
            .byte_window()
            .ok_or_else(|| rejection("native byte view lost its sealed original window"))?;
        let root = loan.root();
        let homes = self.homes.borrow();
        let original = homes
            .get(&(root.namespace(), root.allocation()))
            .filter(|home| home.root == root)
            .ok_or_else(|| rejection("native byte view has no original allocation"))?;
        let mut base = None;
        for index in 0..length {
            let path = loan
                .scalar_path(&[Path::FixedIndex(index as u64)])
                .ok_or_else(|| rejection("native byte view includes an uncaptured byte"))?;
            let cell = original
                .cells
                .get(&path)
                .ok_or_else(|| rejection("native byte view includes an unestablished byte"))?;
            if !matches!(cell.logical, ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == Some(8))
                || cell.stored != cell.logical
            {
                return Err(rejection(
                    "native byte view changes its original byte representation",
                ));
            }
            let start = cell
                .offset
                .checked_sub(index)
                .ok_or_else(|| rejection("native byte view precedes its original home"))?;
            if base.is_some_and(|base| base != start) {
                return Err(rejection(
                    "native byte view is not one contiguous original window",
                ));
            }
            base = Some(start);
        }
        let base = match base {
            Some(base) => base,
            None if original.structural_type.is_none() => captured_offset,
            None => {
                return Err(rejection(
                    "empty typed byte view requires its retained target anchor",
                ));
            }
        };
        if base
            .checked_add(length)
            .is_none_or(|end| end > original.extent)
        {
            return Err(rejection(
                "native byte view exceeds its original allocation",
            ));
        }
        Ok((original.home, base, length))
    }
}

pub(super) fn exact_layout(
    layouts: &[NativeBorrowedStorageLayout],
    structural_type: StructuralTypeId,
) -> Result<&NativeBorrowedStorageLayout, TerminalEffectRejection> {
    let mut matches = layouts
        .iter()
        .filter(|layout| layout.structural_type == structural_type);
    let layout = matches
        .next()
        .ok_or_else(|| rejection("native original allocation has no retained layout recipe"))?;
    if matches.next().is_some() {
        return Err(rejection("native original allocation layout is ambiguous"));
    }
    Ok(layout)
}

fn scalar_location(
    layout: &NativeBorrowedStorageLayout,
    actual: &[Path],
) -> Result<(usize, ScalarType, ScalarType), TerminalEffectRejection> {
    let mut selected = None;
    for field in layout.fields() {
        let path = layout.field_path(field).collect::<Vec<_>>();
        if path.len() != actual.len() {
            continue;
        }
        let mut offset = field.byte_offset as usize;
        let mut matched = true;
        for (position, (expected, actual)) in path.iter().zip(actual).enumerate() {
            if let Some(dimension) = layout
                .field_repetitions(field)
                .iter()
                .find(|dimension| dimension.path_position == position)
            {
                let Path::FixedIndex(index) = actual else {
                    matched = false;
                    break;
                };
                if *expected != Path::FixedIndex(0) || *index >= dimension.length {
                    matched = false;
                    break;
                }
                offset = offset
                    .checked_add(
                        usize::try_from(
                            index
                                .checked_mul(u64::from(dimension.byte_stride))
                                .ok_or_else(|| rejection("native array displacement overflow"))?,
                        )
                        .map_err(|_| rejection("native array displacement exceeds host range"))?,
                    )
                    .ok_or_else(|| rejection("native array displacement overflow"))?;
            } else if expected != actual {
                matched = false;
                break;
            }
        }
        if !matched {
            continue;
        }
        if selected.is_some()
            || offset
                .checked_add(scalar_extent(field.stored_scalar)?)
                .is_none_or(|end| end > layout.byte_size as usize)
        {
            return Err(rejection(
                "native scalar recipe overlaps or exceeds its original allocation",
            ));
        }
        selected = Some((offset, field.logical_scalar, field.stored_scalar));
    }
    selected.ok_or_else(|| rejection("original scalar has no exact native path/codec"))
}

fn reference_location(
    layout: &NativeBorrowedStorageLayout,
    actual: &[Path],
) -> Result<usize, TerminalEffectRejection> {
    let mut selected = None;
    for recipe in layout.opaque_regions() {
        super::super::borrowed_storage::geometry::expand_coordinates(
            layout.opaque_region_path(recipe).collect(),
            recipe.byte_offset as usize,
            layout.opaque_region_repetitions(recipe),
            &mut |path, offset| {
                if path == actual {
                    if selected.is_some()
                        || recipe.byte_size != 8
                        || offset % 8 != 0
                        || offset
                            .checked_add(8)
                            .is_none_or(|end| end > layout.byte_size as usize)
                    {
                        return Err(rejection(
                            "native reference slot is ambiguous or outside its original allocation",
                        ));
                    }
                    selected = Some(offset);
                }
                Ok(())
            },
        )?;
    }
    selected
        .ok_or_else(|| rejection("original reference has no exact produced pointer-slot recipe"))
}

fn poisoned_addresses() -> TerminalEffectRejection {
    rejection("host address ledger was poisoned")
}

// Only a host-provenance address or null may reach a native import: an
// interpreter address names interpreter storage the host cannot see.
fn host_address_value(value: TerminalScalarValue) -> Result<u64, TerminalEffectRejection> {
    let carrier = |scalar_type| {
        super::super::scalar_codec::is_host_address_carrier(ScalarType::Integer(scalar_type))
    };
    match value {
        TerminalScalarValue::HostAddress {
            scalar_type,
            address,
        } if carrier(scalar_type) => Ok(address),
        TerminalScalarValue::Integer {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
        } if carrier(scalar_type) => Ok(0),
        TerminalScalarValue::Integer { scalar_type, .. } if carrier(scalar_type) => Err(rejection(
            "native import argument is not an address the host returned",
        )),
        _ => Err(rejection(
            "native address argument lost its address carrier",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerType, IntegerValue};

    #[test]
    fn only_host_addresses_and_null_return_to_a_native_import() {
        let address = IntegerType::address(64).unwrap();
        let host = TerminalScalarValue::HostAddress {
            scalar_type: address,
            address: 0x7000,
        };
        assert_eq!(host_address_value(host).unwrap(), 0x7000);
        let null = TerminalScalarValue::Integer {
            scalar_type: address,
            value: IntegerValue::Unsigned(0),
        };
        assert_eq!(host_address_value(null).unwrap(), 0);
        // An interpreter address with the same number is not the host's.
        let interpreter = TerminalScalarValue::Integer {
            scalar_type: address,
            value: IntegerValue::Unsigned(0x7000),
        };
        assert!(host_address_value(interpreter).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn internal_native_provider_worker() {
        crate::run_native_provider_worker_if_requested();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn provider_result_leases_preserve_live_leaf_custody_and_retirement() {
        use super::super::values::{ScalarField, StorageCodec};
        use platform_custody::temporary_directory::TemporaryDirectory;
        use semantic_vocabulary::{IntegerSign, StructuralFieldId};
        use std::process::Command;
        use terminal_interpreter::{
            TerminalEffectResult, TerminalReferencePayload, TerminalStorageContents,
        };
        let directory = TemporaryDirectory::create("provider-result-leases", false).unwrap();
        let library = directory.path().join("libomega_pair.so");
        let provider = directory.path().join("provider.c");
        let version = directory.path().join("provider.map");
        std::fs::write(&provider,
            "#include <stdint.h>\nstruct Pair { int32_t left; int32_t right; };\nstatic const struct Pair cell = { 37, 91 };\nconst struct Pair *omega_reference_pair(void) { return &cell; }\n").unwrap();
        std::fs::write(
            &version,
            "PAIR_1.0 { global: omega_reference_pair; local: *; };\n",
        )
        .unwrap();
        let output = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args([
                "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            ])
            .arg(format!("-Wl,--version-script={}", version.display()))
            .arg(&provider)
            .arg("-o")
            .arg(&library)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let codec = StorageCodec {
            structural_type: StructuralTypeId::new(1).unwrap(),
            extent: 8,
            alignment: 4,
            fields: [0, 4]
                .into_iter()
                .enumerate()
                .map(|(index, offset)| ScalarField {
                    path: vec![Path::Field(
                        StructuralFieldId::new(index as u64 + 1).unwrap(),
                    )],
                    offset,
                    logical: scalar,
                    stored: scalar,
                })
                .collect(),
            references: vec![],
        };
        for failed in [false, true] {
            let mut worker = Command::new(std::env::current_exe().unwrap());
            worker.args(["internal_native_provider_worker", "--nocapture"]);
            let (worker, product) = native_provider_sponsor::test_support::pointer_result_worker(
                worker,
                &library,
                b"omega_reference_pair",
                b"PAIR_1.0",
            );
            let context = NativeContext::new(
                Some(worker),
                crate::execution::TerminalNativeServiceDelegation::MatchingHost,
                Arc::new(Mutex::new(
                    super::super::super::allocation::AllocationValues::default(),
                )),
            );
            let adapter = context
                .sponsor
                .borrow_mut()
                .with(|process| {
                    process.install_published_with_result(
                        &product,
                        &[],
                        native_provider_runtime::NativeResultStorage::SharedProviderReference {
                            byte_size: codec.extent,
                            alignment: codec.alignment,
                        },
                    )
                })
                .unwrap();
            let lease = context.call_provider_reference(adapter, &[]).unwrap();
            let reference = lease.reference;
            // The C provider returns the same object every time. Occurrence
            // identities must never imply disjoint external regions.
            let other = context.call_provider_reference(adapter, &[]).unwrap();
            assert_ne!(reference, other.reference);
            assert_eq!(context.sponsor.borrow().references.active(), 2);
            drop(other);
            let TerminalEffectResult::Reference(value) =
                super::super::provider_results::complete(lease, &codec, &[0, 1]).unwrap()
            else {
                panic!("live provider result")
            };
            let TerminalReferencePayload::Resident {
                aggregate,
                mut scalars,
            } = value.referent
            else {
                panic!("resident payload")
            };
            assert_eq!(context.sponsor.borrow().references.active(), 1);
            assert_eq!(scalars.len(), 2);
            let mut originals = aggregate.fields;
            originals.sort_unstable_by(|left, right| left.path.cmp(&right.path));
            for (scalar, original) in scalars.iter().zip(originals) {
                assert_eq!(scalar.path, original.path);
                let TerminalStorageContents::Scalar { value, .. } = original.contents else {
                    panic!("plain scalar field")
                };
                assert_eq!(scalar.backing.read().unwrap(), value);
                assert!(scalar.backing.write(value).is_err());
            }
            let cloned = scalars[0].clone();
            assert_eq!(scalars[0], cloned);
            drop(scalars.remove(0));
            assert!(
                context
                    .finish()
                    .unwrap_err()
                    .reason
                    .contains("live result references")
            );
            drop(scalars);
            assert_eq!(context.sponsor.borrow().references.active(), 1);
            if failed {
                context.sponsor.borrow_mut().failure = Some("forced test sponsor failure".into());
                assert!(cloned.backing.read().is_err());
                assert!(
                    context
                        .finish()
                        .unwrap_err()
                        .reason
                        .contains("live result references")
                );
            }
            let retained_sponsor = Rc::downgrade(&context.sponsor);
            drop(context);
            // Only the one result lease now pins this worker, across either
            // backing's clones and after the importing context is gone.
            assert_eq!(retained_sponsor.strong_count(), 1);
            if !failed {
                assert!(cloned.backing.read().is_ok());
            }
            let retained_sponsor = retained_sponsor.upgrade().unwrap();
            let mut sponsor = retained_sponsor.borrow_mut();
            // Last drop must only queue its release, even under this borrow.
            drop(cloned);
            assert_eq!(sponsor.references.active(), 0);
            sponsor.finish().unwrap();
            assert!(sponsor.process.is_none());
            assert!(
                sponsor
                    .read_provider_reference(reference, 0, &mut [0; 4])
                    .is_err()
            );
            assert!(sponsor.process.is_none());
        }
    }
}
