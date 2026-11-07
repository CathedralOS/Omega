//! Current scalar and storage copies of native resource values in one activation.
//!
//! A whole HANDLE word and bits computed from that word are different things.
//! Both remain uses for retirement and escape checks, but only an exact whole
//! word can be supplied to a native handle operation. The canonical replay owns
//! initialization, access, address resolution, active cases and all-path custody;
//! these maps do not infer any of those facts from an absent entry.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{IdTable, MachineId, PlaceId, ScalarType, ValueId};

use super::windows_handle_lifetimes::HandleInstance;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct HandleWordAliases {
    scalar_type: ScalarType,
    exact: Option<HandleInstance>,
    dependencies: BTreeSet<HandleInstance>,
}

impl HandleWordAliases {
    /// No tracked acquired handle flows from this independently checked input
    /// or native output. This supplies neither a numeric value nor permission
    /// to observe uninitialized or inaccessible storage.
    pub(super) fn untracked(scalar_type: ScalarType) -> Self {
        Self {
            scalar_type,
            exact: None,
            dependencies: BTreeSet::new(),
        }
    }

    pub(super) fn acquired(
        scalar_type: ScalarType,
        instance: HandleInstance,
    ) -> Result<Self, String> {
        if !handle_word(scalar_type) {
            return Err(
                "native HANDLE acquisition does not have its complete 64-bit integer carrier"
                    .into(),
            );
        }
        Ok(Self {
            scalar_type,
            exact: Some(instance),
            dependencies: BTreeSet::from([instance]),
        })
    }

    /// The storage owner must independently establish an exact, same-address
    /// represented read. Matching widths alone never supplies that premise.
    /// Equal-width signed/unsigned integer views preserve every HANDLE bit;
    /// a narrow read loses exact identity permanently, even if later widened.
    pub(super) fn represented_as(&self, target: ScalarType) -> Self {
        Self {
            scalar_type: target,
            exact: (handle_word(self.scalar_type) && handle_word(target))
                .then_some(self.exact)
                .flatten(),
            dependencies: self.dependencies.clone(),
        }
    }

    pub(super) fn derived(
        scalar_type: ScalarType,
        operands: impl IntoIterator<Item = Self>,
    ) -> Self {
        Self::derived_from_dependencies(
            scalar_type,
            operands
                .into_iter()
                .flat_map(|operand| operand.dependencies),
        )
    }

    pub(super) fn derived_from_dependencies(
        scalar_type: ScalarType,
        dependencies: impl IntoIterator<Item = HandleInstance>,
    ) -> Self {
        Self {
            scalar_type,
            exact: None,
            dependencies: dependencies.into_iter().collect(),
        }
    }

    pub(super) fn exact_handle(&self) -> Result<HandleInstance, String> {
        let instance = self
            .exact
            .ok_or("native handle operand is not an exact acquired whole word")?;
        if !handle_word(self.scalar_type) || self.dependencies != BTreeSet::from([instance]) {
            return Err(
                "native handle operand mixes another tracked value or incomplete representation"
                    .into(),
            );
        }
        Ok(instance)
    }

    pub(super) fn scalar_type(&self) -> ScalarType {
        self.scalar_type
    }

    pub(super) fn dependencies(&self) -> &BTreeSet<HandleInstance> {
        &self.dependencies
    }
}

fn handle_word(scalar_type: ScalarType) -> bool {
    matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address() && integer.bits() == 64)
}

/// Analysis-owned activation coordinate plus the original owned storage
/// declaration. Borrowed arguments carry this same backing across a call;
/// they never gain a fresh physical home from the callee's formal PlaceId.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct HandleStorageBacking {
    pub owner: HandleStorageOwner,
    pub place: PlaceId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum HandleStorageOwner {
    Activation(usize),
    /// The actual literal producer in this same Terminal artifact. An image
    /// value can leave its producing activation without borrowing its stack.
    Literal(MachineId),
}

/// A checked byte interval in one actual backing. The canonical storage owner
/// derives its geometry and loan relationship from the original operation.
/// The interval itself supplies neither access nor initialization permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct HandleStorageRange {
    backing: HandleStorageBacking,
    start: u64,
    end: u64,
}

impl HandleStorageRange {
    pub(super) fn new(
        backing: HandleStorageBacking,
        start: u64,
        byte_length: u64,
    ) -> Result<Self, String> {
        let end = start
            .checked_add(byte_length)
            .ok_or("handle storage range overflows its backing coordinate")?;
        Ok(Self {
            backing,
            start,
            end,
        })
    }

    pub(super) fn backing(&self) -> HandleStorageBacking {
        self.backing
    }
    pub(super) fn byte_offset(&self) -> u64 {
        self.start
    }
    pub(super) fn byte_length(&self) -> u64 {
        self.end - self.start
    }

    pub(super) fn contains(&self, other: &Self) -> bool {
        self.backing == other.backing && self.start <= other.start && self.end >= other.end
    }

    pub(super) fn overlaps(&self, other: &Self) -> bool {
        self.start < self.end
            && other.start < other.end
            && self.backing == other.backing
            && self.start < other.end
            && other.start < self.end
    }

    pub(super) fn relative_to(&self, container: &Self) -> Option<RelativeHandleRange> {
        container.contains(self).then(|| RelativeHandleRange {
            offset: self.start - container.start,
            byte_length: self.byte_length(),
        })
    }

    fn intersection(&self, other: &Self) -> Option<Self> {
        self.overlaps(other).then(|| Self {
            backing: self.backing,
            start: self.start.max(other.start),
            end: self.end.min(other.end),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RelativeHandleRange {
    offset: u64,
    byte_length: u64,
}

impl RelativeHandleRange {
    pub(super) fn at(self, destination: &HandleStorageRange) -> Result<HandleStorageRange, String> {
        let start = destination
            .byte_offset()
            .checked_add(self.offset)
            .ok_or("captured storage offset overflows its destination")?;
        let range = HandleStorageRange::new(destination.backing(), start, self.byte_length)?;
        if !destination.contains(&range) {
            return Err("captured storage extends outside its destination interval".into());
        }
        Ok(range)
    }
}

/// All outgoing bytes are captured before cleanup or caller replacement.
/// Partial scalar selections retain dependencies, never an exact whole word.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CapturedHandleRegion {
    byte_length: u64,
    cells: Vec<(RelativeHandleRange, HandleWordAliases)>,
    regions: Vec<(RelativeHandleRange, BTreeSet<HandleInstance>)>,
}

impl CapturedHandleRegion {
    pub(super) fn dependencies(&self) -> BTreeSet<HandleInstance> {
        self.cells
            .iter()
            .flat_map(|(_, word)| word.dependencies.iter())
            .chain(
                self.regions
                    .iter()
                    .flat_map(|(_, dependencies)| dependencies.iter()),
            )
            .copied()
            .collect()
    }
}

/// Sparse resident contents. Exact cells retain complete word identities;
/// unresolved but bounded regions retain dependencies without guessing a cell.
/// Every query uses already resolved current canonical storage coordinates in
/// this activation. Caller/formal substitution belongs to the canonical owner.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct HandleAliases {
    // Outer Some is an original declared binder; inner Some is its current capture.
    scalars: IdTable<ValueId, Option<Option<HandleWordAliases>>>,
    cells: BTreeMap<HandleStorageRange, HandleWordAliases>,
    regions: BTreeMap<HandleStorageRange, BTreeSet<HandleInstance>>,
}

impl HandleAliases {
    pub(super) fn enter_activation(
        &self,
        values: impl IntoIterator<Item = ValueId>,
    ) -> Result<Self, String> {
        let mut activation = Self::with_scalar_namespace(values)?;
        activation.cells.clone_from(&self.cells);
        activation.regions.clone_from(&self.regions);
        Ok(activation)
    }

    /// Restore the exact caller scalar snapshots while retaining all memory
    /// changes from the completed callee. Return/output census must run before
    /// the separate retirement of callee-owned backing.
    pub(super) fn resume_caller(mut self, caller: &Self) -> Self {
        self.scalars.clone_from(&caller.scalars);
        self
    }

    pub(super) fn retire_activation(&mut self, activation: usize) {
        let retired = HandleStorageOwner::Activation(activation);
        self.cells
            .retain(|range, _| range.backing().owner != retired);
        self.regions
            .retain(|range, _| range.backing().owner != retired);
    }

    pub(super) fn scalar(&self, value: ValueId) -> Result<&HandleWordAliases, String> {
        self.scalars
            .get(value)
            .and_then(Option::as_ref)
            .and_then(Option::as_ref)
            .ok_or_else(|| format!("handle alias census has no current scalar capture {value:?}"))
    }

    pub(super) fn with_scalar_namespace(
        values: impl IntoIterator<Item = ValueId>,
    ) -> Result<Self, String> {
        let mut rows = values
            .into_iter()
            .map(|value| (value, Some(None)))
            .collect::<Vec<_>>();
        rows.sort_by_key(|(value, _)| *value);
        let scalars = IdTable::from_rows_with_gaps(rows, 65_536)
            .map_err(|reason| format!("handle alias scalar inventory is invalid: {reason:?}"))?;
        Ok(Self {
            scalars,
            cells: BTreeMap::new(),
            regions: BTreeMap::new(),
        })
    }

    pub(super) fn define_scalar(
        &mut self,
        value: ValueId,
        aliases: HandleWordAliases,
    ) -> Result<(), String> {
        let target = self
            .scalars
            .get_mut(value)
            .and_then(Option::as_mut)
            .ok_or("handle alias definition has no original scalar binder")?;
        *target = Some(aliases);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn assign_scalars(
        &mut self,
        assignments: &[(ValueId, ValueId)],
        retired: &[ValueId],
    ) -> Result<(), String> {
        // Read every actual before installing any new block formal. Refusal
        // cannot partially rename a sibling or erase a retained old capture.
        let captured = assignments
            .iter()
            .map(|(formal, actual)| self.scalar(*actual).cloned().map(|word| (*formal, word)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut targets = captured
            .iter()
            .map(|(formal, _)| *formal)
            .collect::<Vec<_>>();
        targets.sort_unstable();
        if targets.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("handle alias arrival repeats a target scalar".into());
        }
        if captured
            .iter()
            .any(|(formal, _)| self.scalars.get(*formal).is_none_or(Option::is_none))
        {
            return Err("handle alias arrival has no original target binder".into());
        }
        for formal in retired {
            if let Some(Some(target)) = self.scalars.get_mut(*formal) {
                *target = None;
            }
        }
        for (formal, word) in captured {
            *self
                .scalars
                .get_mut(formal)
                .and_then(Option::as_mut)
                .expect("arrival target was checked before commit") = Some(word);
        }
        Ok(())
    }

    /// Return current aliases, not an initialized-value or access verdict. The
    /// complete scalar value must be read legally at the same cut separately.
    pub(super) fn stored_word(
        &self,
        home: &HandleStorageRange,
        target: ScalarType,
    ) -> HandleWordAliases {
        let mut result = self
            .cells
            .get(home)
            .map(|word| word.represented_as(target))
            .unwrap_or_else(|| HandleWordAliases::untracked(target));
        for (region, dependencies) in &self.regions {
            if region.overlaps(home) {
                result.exact = None;
                result.dependencies.extend(dependencies);
            }
        }
        // A broader observation includes every resident scalar beneath it.
        // Such a region is dependency evidence, never one exact scalar word.
        for (cell, word) in &self.cells {
            if cell != home && cell.overlaps(home) {
                result.exact = None;
                result.dependencies.extend(&word.dependencies);
            }
        }
        result
    }

    /// Replace exactly these scalar bytes. The selected physical codec and
    /// incoming word must cover the entire checked interval. An older value's
    /// different logical interpretation does not keep its overwritten bits live.
    pub(super) fn write_complete_word(
        &mut self,
        home: HandleStorageRange,
        stored_type: ScalarType,
        word: HandleWordAliases,
    ) -> Result<(), String> {
        let stored_width =
            terminal_semantics::reference_storage_byte_width(stored_type).map(u64::from);
        let value_width =
            terminal_semantics::reference_storage_byte_width(word.scalar_type).map(u64::from);
        if word.scalar_type != stored_type
            || stored_width != Some(home.byte_length())
            || value_width != stored_width
        {
            return Err(
                "complete scalar write does not cover its selected physical interval".into(),
            );
        }
        self.replace_complete_region(&home);
        if !word.dependencies.is_empty() {
            self.cells.insert(home, word);
        }
        Ok(())
    }

    /// The canonical owner uses this for a smaller represented store, a
    /// nonzero represented offset or a footprint that does not cover the whole
    /// original scalar. Old untouched bits remain dependencies. No partial
    /// write can retain or establish one exact HANDLE, including a constant.
    pub(super) fn write_partial_word(
        &mut self,
        home: HandleStorageRange,
        word: &HandleWordAliases,
    ) {
        self.write_region(home, word.dependencies.clone());
    }

    /// Replace the complete selected owned region. This does not promise to
    /// replace a containing scalar when the selected region is only a child.
    /// Any such overlapping parent loses exact identity but retains its bits'
    /// dependencies. The caller separately proves the physical coverage of the
    /// selected region; represented partial writes use write_partial_word.
    pub(super) fn replace_complete_region(&mut self, replaced: &HandleStorageRange) {
        if replaced.byte_length() == 0 {
            return;
        }
        self.invalidate_overlapping(replaced);
        self.cells.retain(|cell, _| !replaced.contains(cell));
        self.regions.retain(|region, _| !replaced.contains(region));
    }

    fn invalidate_overlapping(&mut self, written: &HandleStorageRange) {
        for (cell, word) in &mut self.cells {
            if cell.overlaps(written) {
                word.exact = None;
            }
        }
    }

    /// A may-write to a known family cannot erase other possible members.
    /// Even an untracked constant may change an existing handle's bits: every
    /// overlapping exact identity is retired before considering new inputs.
    /// Dependencies remain until a full physical replacement clears them.
    /// Unknown unbounded addresses require a refusal upstream.
    pub(super) fn write_region(
        &mut self,
        region: HandleStorageRange,
        dependencies: BTreeSet<HandleInstance>,
    ) {
        if region.byte_length() == 0 {
            return;
        }
        self.invalidate_overlapping(&region);
        if !dependencies.is_empty() {
            self.regions.entry(region).or_default().extend(dependencies);
        }
    }

    pub(super) fn capture_region(&self, source: &HandleStorageRange) -> CapturedHandleRegion {
        let mut captured = CapturedHandleRegion {
            byte_length: source.byte_length(),
            ..Default::default()
        };
        for (cell, word) in &self.cells {
            if let Some(relative) = cell.relative_to(source) {
                captured.cells.push((relative, word.clone()));
            } else if let Some(overlap) = cell.intersection(source) {
                captured.regions.push((
                    overlap
                        .relative_to(source)
                        .expect("intersection is contained"),
                    word.dependencies.clone(),
                ));
            }
        }
        for (region, dependencies) in &self.regions {
            if let Some(overlap) = region.intersection(source) {
                captured.regions.push((
                    overlap
                        .relative_to(source)
                        .expect("intersection is contained"),
                    dependencies.clone(),
                ));
            }
        }
        captured
    }

    pub(super) fn install_region(
        &mut self,
        destination: &HandleStorageRange,
        captured: &CapturedHandleRegion,
    ) -> Result<(), String> {
        if captured.byte_length != destination.byte_length() {
            return Err("captured handle region changes its complete physical extent".into());
        }
        // Prepare every destination before modifying anything. A refused copy
        // must not erase an older live alias or a sibling's dependencies.
        let cells = captured
            .cells
            .iter()
            .map(|(relative, word)| Ok((relative.at(destination)?, word.clone())))
            .collect::<Result<Vec<_>, String>>()?;
        let regions = captured
            .regions
            .iter()
            .map(|(relative, dependencies)| Ok((relative.at(destination)?, dependencies.clone())))
            .collect::<Result<Vec<_>, String>>()?;
        self.replace_complete_region(destination);
        self.cells.extend(cells);
        for (region, dependencies) in regions {
            self.regions.entry(region).or_default().extend(dependencies);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn copy_region(
        &mut self,
        source: &HandleStorageRange,
        destination: &HandleStorageRange,
    ) -> Result<(), String> {
        let captured = self.capture_region(source);
        self.install_region(destination, &captured)
    }

    pub(super) fn stored_dependencies(
        &self,
        observed: &HandleStorageRange,
    ) -> BTreeSet<HandleInstance> {
        self.cells
            .iter()
            .filter(|(cell, _)| cell.overlaps(observed))
            .flat_map(|(_, word)| word.dependencies.iter())
            .chain(
                self.regions
                    .iter()
                    .filter(|(region, _)| region.overlaps(observed))
                    .flat_map(|(_, dependencies)| dependencies.iter()),
            )
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId};

    fn integer(sign: IntegerSign, bits: u16) -> ScalarType {
        ScalarType::Integer(IntegerType::new(sign, bits).unwrap())
    }

    fn range(place: u64, offset: u64, length: u64) -> HandleStorageRange {
        HandleStorageRange::new(
            HandleStorageBacking {
                owner: HandleStorageOwner::Activation(0),
                place: PlaceId::new(place).unwrap(),
            },
            offset,
            length,
        )
        .unwrap()
    }

    #[test]
    fn copied_outputs_keep_old_handle_dependencies_after_source_replacement_and_narrowing() {
        let word = integer(IntegerSign::Unsigned, 64);
        let acquired = HandleInstance(0);
        let source = range(1, 0, 16);
        let destination = range(2, 0, 16);
        let original = range(1, 8, 8);
        let copied = range(2, 8, 8);
        let mut aliases = HandleAliases::default();
        aliases
            .write_complete_word(
                original.clone(),
                word,
                HandleWordAliases::acquired(word, acquired).unwrap(),
            )
            .unwrap();
        aliases.copy_region(&source, &destination).unwrap();
        aliases.replace_complete_region(&source);
        assert!(aliases.stored_dependencies(&source).is_empty());
        assert_eq!(
            aliases
                .stored_word(&copied, integer(IntegerSign::Signed, 64))
                .exact_handle()
                .unwrap(),
            acquired
        );
        let narrow = aliases.stored_word(&copied, integer(IntegerSign::Unsigned, 32));
        assert!(narrow.represented_as(word).exact_handle().is_err());
        assert_eq!(narrow.dependencies(), &BTreeSet::from([acquired]));
        aliases.write_region(destination.clone(), BTreeSet::from([HandleInstance(1)]));
        assert!(aliases.stored_word(&copied, word).exact_handle().is_err());
        assert_eq!(
            aliases.stored_dependencies(&destination),
            BTreeSet::from([acquired, HandleInstance(1)])
        );
        aliases.replace_complete_region(&destination);
        assert!(aliases.stored_dependencies(&destination).is_empty());
    }

    #[test]
    fn untracked_partial_and_possible_writes_retire_exact_identity_without_erasing_old_bits() {
        let word = integer(IntegerSign::Unsigned, 64);
        let narrow = integer(IntegerSign::Unsigned, 32);
        let home = range(1, 0, 8);
        let source = range(2, 0, 4);
        let child = range(1, 0, 4);
        let mut initial = HandleAliases::default();
        initial
            .write_complete_word(
                home.clone(),
                word,
                HandleWordAliases::acquired(word, HandleInstance(0)).unwrap(),
            )
            .unwrap();
        let mut partial = initial.clone();
        // The backing extent, not the presented U32 store type, distinguishes
        // full replacement from writing only part of this captured U64 home.
        assert!(
            partial
                .write_complete_word(home.clone(), word, HandleWordAliases::untracked(narrow))
                .is_err()
        );
        assert_eq!(partial, initial);
        partial.write_partial_word(home.clone(), &HandleWordAliases::untracked(narrow));
        assert!(partial.stored_word(&home, word).exact_handle().is_err());
        assert_eq!(
            partial.stored_dependencies(&home),
            BTreeSet::from([HandleInstance(0)])
        );
        let mut child_write = initial.clone();
        child_write
            .write_complete_word(child.clone(), narrow, HandleWordAliases::untracked(narrow))
            .unwrap();
        assert!(child_write.stored_word(&home, word).exact_handle().is_err());
        assert_eq!(
            child_write.stored_dependencies(&home),
            BTreeSet::from([HandleInstance(0)])
        );
        let mut copied = initial.clone();
        copied.copy_region(&source, &child).unwrap();
        assert!(copied.stored_word(&home, word).exact_handle().is_err());
        assert_eq!(
            copied.stored_dependencies(&home),
            BTreeSet::from([HandleInstance(0)])
        );
        // Neither interval contains the other. Authored/reinterpreted field
        // names cannot keep the first word exact after this byte overlap.
        let mut overlap = initial.clone();
        let neighbour = range(1, 16, 8);
        overlap
            .write_complete_word(
                neighbour,
                word,
                HandleWordAliases::acquired(word, HandleInstance(1)).unwrap(),
            )
            .unwrap();
        overlap.write_region(range(1, 4, 8), BTreeSet::new());
        assert!(overlap.stored_word(&home, word).exact_handle().is_err());
        assert_eq!(
            overlap
                .stored_word(&neighbour, word)
                .exact_handle()
                .unwrap(),
            HandleInstance(1)
        );
        let before = overlap.clone();
        assert!(overlap.copy_region(&range(2, 0, 4), &home).is_err());
        assert_eq!(overlap, before);
        assert!(HandleStorageRange::new(home.backing(), u64::MAX, 1).is_err());

        let mut possible = initial.clone();
        possible.write_region(home.clone(), BTreeSet::new());
        assert!(possible.stored_word(&home, word).exact_handle().is_err());
        assert_eq!(
            possible.stored_dependencies(&home),
            BTreeSet::from([HandleInstance(0)])
        );
        possible
            .write_complete_word(home.clone(), word, HandleWordAliases::untracked(word))
            .unwrap();
        assert!(possible.stored_dependencies(&home).is_empty());
    }

    #[test]
    fn zero_byte_ranges_do_not_capture_or_overwrite_a_handle() {
        let word = integer(IntegerSign::Unsigned, 64);
        let home = range(1, 0, 8);
        let empty = range(1, 4, 0);
        let mut aliases = HandleAliases::default();
        aliases
            .write_complete_word(
                home,
                word,
                HandleWordAliases::acquired(word, HandleInstance(0)).unwrap(),
            )
            .unwrap();
        let before = aliases.clone();
        assert!(!empty.overlaps(&home));
        assert!(!home.overlaps(&empty));
        assert!(aliases.capture_region(&empty).dependencies().is_empty());
        assert!(aliases.stored_dependencies(&empty).is_empty());
        aliases.copy_region(&empty, &range(1, 6, 0)).unwrap();
        aliases.write_region(empty, BTreeSet::from([HandleInstance(1)]));
        aliases.write_partial_word(
            empty,
            &HandleWordAliases::acquired(word, HandleInstance(1)).unwrap(),
        );
        aliases.replace_complete_region(&empty);
        assert_eq!(aliases, before);
        assert_eq!(
            aliases.stored_word(&home, word).exact_handle().unwrap(),
            HandleInstance(0)
        );
    }

    #[test]
    fn high_base_scalar_namespace_keeps_ids_and_refuses_unknown_arrivals_atomically() {
        let high = ValueId::new(u64::MAX).unwrap();
        let absent = ValueId::new(u64::MAX - 1).unwrap();
        let word = integer(IntegerSign::Unsigned, 64);
        let mut aliases = HandleAliases::with_scalar_namespace([high]).unwrap();
        aliases
            .define_scalar(
                high,
                HandleWordAliases::acquired(word, HandleInstance(0)).unwrap(),
            )
            .unwrap();
        assert_eq!(
            aliases.scalar(high).unwrap().exact_handle().unwrap(),
            HandleInstance(0)
        );
        let before = aliases.clone();
        assert!(aliases.assign_scalars(&[(absent, high)], &[high]).is_err());
        assert_eq!(aliases, before);
        assert!(
            aliases
                .define_scalar(absent, HandleWordAliases::untracked(word))
                .is_err()
        );
        assert_eq!(aliases, before);
        assert!(HandleAliases::with_scalar_namespace([high, high]).is_err());
        assert!(HandleAliases::with_scalar_namespace([ValueId::new(1).unwrap(), high]).is_err());
    }

    #[test]
    fn scalar_arrivals_capture_simultaneously_and_refuse_atomically() {
        let word = integer(IntegerSign::Unsigned, 64);
        let first = ValueId::new(1).unwrap();
        let second = ValueId::new(2).unwrap();
        let mut aliases = HandleAliases::with_scalar_namespace([first, second]).unwrap();
        aliases
            .define_scalar(
                first,
                HandleWordAliases::acquired(word, HandleInstance(0)).unwrap(),
            )
            .unwrap();
        aliases
            .define_scalar(
                second,
                HandleWordAliases::acquired(word, HandleInstance(1)).unwrap(),
            )
            .unwrap();
        aliases
            .assign_scalars(&[(first, second), (second, first)], &[first, second])
            .unwrap();
        assert_eq!(
            aliases.scalar(first).unwrap().exact_handle().unwrap(),
            HandleInstance(1)
        );
        assert_eq!(
            aliases.scalar(second).unwrap().exact_handle().unwrap(),
            HandleInstance(0)
        );
        let before = aliases.clone();
        assert!(
            aliases
                .assign_scalars(
                    &[(first, second), (second, ValueId::new(99).unwrap())],
                    &[first, second]
                )
                .is_err()
        );
        assert_eq!(aliases, before);
    }
}
