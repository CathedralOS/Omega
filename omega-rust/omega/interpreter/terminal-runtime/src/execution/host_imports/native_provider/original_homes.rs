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
    process::Command,
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
    command: Option<Command>,
    process: Option<NativeProviderProcess>,
    failure: Option<String>,
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
            match NativeProviderProcess::launch(command, Duration::from_secs(30)) {
                Ok(process) => self.process = Some(process),
                Err(error) => {
                    self.failure = Some(error.to_string());
                    return Err(rejection(error.to_string()));
                }
            }
        }
        f(self.process.as_mut().unwrap()).map_err(|error| {
            self.failure = Some(error.to_string());
            rejection(error.to_string())
        })
    }

    pub fn call(
        &mut self,
        adapter: native_provider_runtime::NativeAdapterId,
        arguments: &[native_provider_runtime::NativeArgument<'_>],
    ) -> Result<Vec<u8>, NativeProviderError> {
        let result = self
            .process
            .as_mut()
            .expect("installed native process")
            .call(adapter, arguments);
        if let Err(error) = &result {
            self.failure = Some(error.to_string());
        }
        result
    }

    pub fn finish(&mut self) -> Result<(), TerminalEffectRejection> {
        if self.failure.is_some() {
            return Ok(());
        }
        if let Some(process) = self.process.take() {
            process
                .close()
                .map_err(|error| rejection(error.to_string()))?;
        }
        self.failure = Some("native provider incarnation has retired".into());
        Ok(())
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
        command: Option<Command>,
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
        Ok(TerminalScalarValue::Integer {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(address)),
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
                    || !matches!(logical, ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 8)
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
            if !matches!(cell.logical, ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 8)
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

fn host_address_value(value: TerminalScalarValue) -> Result<u64, TerminalEffectRejection> {
    match value {
        TerminalScalarValue::Integer {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(address),
        } if super::super::scalar_codec::is_host_address_carrier(ScalarType::Integer(
            scalar_type,
        )) =>
        {
            u64::try_from(address).map_err(|_| rejection("host address exceeds its carrier"))
        }
        _ => Err(rejection(
            "native address argument lost its address carrier",
        )),
    }
}
