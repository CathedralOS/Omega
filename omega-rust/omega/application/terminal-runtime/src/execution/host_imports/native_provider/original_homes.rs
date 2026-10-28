//! One physical allocation per authenticated interpreter root. Views only pick
//! ranges inside it; neither repeated loans nor argument snapshots allocate a
//! replacement. Residents keep this sponsor alive through moves and suspension.

use super::super::scalar_codec::{decode, encode, scalar_extent};
use native_provider_runtime::{
    NativeHomeId, NativePointerSlot, NativePointerTarget, NativeProviderProcess,
};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, ScalarType, StructuralTypeId};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    process::Command,
    rc::Rc,
    time::Duration,
};
use terminal_interpreter::{
    TerminalEffectRejection, TerminalOriginalScalarBackings, TerminalOriginalStorageLoan,
    TerminalOriginalStorageRequest, TerminalOriginalStorageRoot, TerminalScalarBacking,
    TerminalScalarValue,
};

pub(super) fn rejection(reason: impl Into<String>) -> TerminalEffectRejection {
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
        f: impl FnOnce(
            &mut NativeProviderProcess,
        ) -> Result<T, native_provider_runtime::NativeProviderError>,
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
    ) -> Result<Vec<u8>, native_provider_runtime::NativeProviderError> {
        // Installation already established the process. Preserve the actual
        // exit status for the call owner to distinguish a declared trap.
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
}
struct OriginalHome {
    root: TerminalOriginalStorageRoot,
    structural_type: StructuralTypeId,
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
    pub fn new(command: Option<Command>) -> Self {
        Self {
            available: command.is_some(),
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
    pub fn finish(&self) -> Result<(), TerminalEffectRejection> {
        self.sponsor.borrow_mut().finish()
    }

    pub(super) fn prepare(
        &self,
        requests: &[TerminalOriginalStorageRequest],
        layouts: &[NativeBorrowedStorageLayout],
        codecs: &BTreeMap<StructuralTypeId, super::values::StorageCodec>,
    ) -> Result<TerminalOriginalScalarBackings, TerminalEffectRejection> {
        let mut result = Vec::with_capacity(requests.len());
        let mut homes = self.homes.borrow_mut();
        // All requests captured their entry values before any engine cell was
        // attached. Fresh shared aliases in this batch therefore have no Rc
        // yet. Only exact endpoints created here may use that exception.
        let mut established_here = BTreeSet::new();
        for request in requests {
            let layout = exact_layout(layouts, request.structural_type())?;
            let root = request.root();
            let key = (root.namespace(), root.allocation());
            if let std::collections::btree_map::Entry::Vacant(entry) = homes.entry(key) {
                let home = self.sponsor.borrow_mut().with(|process| {
                    process.create_home(layout.byte_size as usize, usize::from(layout.alignment))
                })?;
                entry.insert(OriginalHome {
                    root,
                    structural_type: request.structural_type(),
                    home,
                    extent: layout.byte_size as usize,
                    alignment: usize::from(layout.alignment),
                    cells: BTreeMap::new(),
                    references: BTreeMap::new(),
                });
            }
            let original = homes.get_mut(&key).unwrap();
            if original.root != root
                || original.structural_type != request.structural_type()
                || original.extent != layout.byte_size as usize
                || original.alignment != usize::from(layout.alignment)
            {
                return Err(rejection(
                    "original native root changed artifact, type or geometry",
                ));
            }
            let mut row = Vec::with_capacity(request.values().len());
            for value in request.values() {
                let (offset, logical, stored) = scalar_location(layout, value.path())?;
                if value.value().scalar_type() != logical {
                    return Err(rejection("original scalar differs from native recipe"));
                }
                if let Some(existing) = original.cells.get(value.path()) {
                    let backing: Rc<dyn TerminalScalarBacking> = existing.clone();
                    if existing.offset != offset
                        || existing.logical != logical
                        || existing.stored != stored
                        || match value.backing() {
                            Some(actual) => !Rc::ptr_eq(actual, &backing),
                            None => !established_here.contains(&(key, value.path().to_vec())),
                        }
                        || backing.read()? != value.value()
                    {
                        return Err(rejection(
                            "original scalar no longer names the same resident home",
                        ));
                    }
                    row.push(backing);
                } else {
                    if value.backing().is_some() {
                        return Err(rejection(
                            "resident scalar belongs to a different original allocation",
                        ));
                    }
                    let resident = Rc::new(Resident {
                        sponsor: self.sponsor.clone(),
                        home: original.home,
                        offset,
                        logical,
                        stored,
                    });
                    resident.write(value.value())?;
                    original
                        .cells
                        .insert(value.path().to_vec(), resident.clone());
                    established_here.insert((key, value.path().to_vec()));
                    row.push(resident as Rc<dyn TerminalScalarBacking>);
                }
            }
            for reference in request.references() {
                let offset = reference_location(layout, reference.path())?;
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
        // Allocate every captured home before installing edges. The worker
        // computes its own pointers; parent mapping addresses never cross IPC.
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
        // A retained provider can write through a descriptor even when that
        // descriptor is not an explicit completion argument. Never let an
        // unmodeled pointer write leave Core using a stale referent.
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

    /// Join every exposed scalar's view offset to its original-root offset.
    /// Agreement on one base proves a subrange, never a new allocation. Tags
    /// and opaque regions need their own resident codecs before admission.
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
            let mut path = loan.referent().to_vec();
            path.extend_from_slice(&field.path);
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
