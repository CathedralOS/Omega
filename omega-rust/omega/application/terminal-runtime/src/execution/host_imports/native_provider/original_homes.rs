//! One physical allocation per authenticated interpreter root. Views only pick
//! ranges inside it; neither repeated loans nor argument snapshots allocate a
//! replacement. Residents keep this sponsor alive through moves and suspension.

use super::super::scalar_codec::{decode, encode, scalar_extent};
use native_provider_runtime::{NativeHomeId, NativeProviderProcess};
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
            result.push(row);
        }
        Ok(result)
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
