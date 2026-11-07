//! Current scalar and storage copies of native resource values in one activation.
//!
//! A whole HANDLE word and bits computed from that word are different things.
//! Both remain uses for retirement and escape checks, but only an exact whole
//! word can be supplied to a native handle operation. The canonical replay owns
//! initialization, access, address resolution, active cases and all-path custody;
//! these maps do not infer any of those facts from an absent entry.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{ScalarType, ValueId};
use terminal_semantics::descriptors::descriptor_lifetimes::DescriptorLocation;

use super::windows_handle_lifetimes::HandleInstance;

#[derive(Clone, Debug, PartialEq, Eq)]
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
        Self {
            scalar_type,
            exact: None,
            dependencies: operands
                .into_iter()
                .flat_map(|operand| operand.dependencies)
                .collect(),
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

    pub(super) fn dependencies(&self) -> &BTreeSet<HandleInstance> {
        &self.dependencies
    }
}

fn handle_word(scalar_type: ScalarType) -> bool {
    matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address() && integer.bits() == 64)
}

/// Sparse resident contents. Exact cells retain complete word identities;
/// unresolved but bounded regions retain dependencies without guessing a cell.
/// Every query uses already resolved current canonical storage coordinates in
/// this activation. Caller/formal substitution belongs to the canonical owner.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct HandleAliases {
    scalars: BTreeMap<ValueId, HandleWordAliases>,
    cells: BTreeMap<DescriptorLocation, HandleWordAliases>,
    regions: BTreeMap<DescriptorLocation, BTreeSet<HandleInstance>>,
}

impl HandleAliases {
    pub(super) fn scalar(&self, value: ValueId) -> Result<&HandleWordAliases, String> {
        self.scalars
            .get(&value)
            .ok_or_else(|| format!("handle alias census has no current scalar capture {value:?}"))
    }

    pub(super) fn define_scalar(&mut self, value: ValueId, aliases: HandleWordAliases) {
        self.scalars.insert(value, aliases);
    }

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
        let unique = captured
            .iter()
            .map(|(formal, _)| *formal)
            .collect::<BTreeSet<_>>();
        if unique.len() != captured.len() {
            return Err("handle alias arrival repeats a target scalar".into());
        }
        for formal in retired {
            self.scalars.remove(formal);
        }
        self.scalars.extend(captured);
        Ok(())
    }

    /// Return current aliases, not an initialized-value or access verdict. The
    /// complete scalar value must be read legally at the same cut separately.
    pub(super) fn stored_word(
        &self,
        home: &DescriptorLocation,
        target: ScalarType,
    ) -> HandleWordAliases {
        let mut result = self
            .cells
            .get(home)
            .map(|word| word.represented_as(target))
            .unwrap_or_else(|| HandleWordAliases::untracked(target));
        for (region, dependencies) in &self.regions {
            if overlaps(region, home) {
                result.exact = None;
                result.dependencies.extend(dependencies);
            }
        }
        // A broader observation includes every resident scalar beneath it.
        // Such a region is dependency evidence, never one exact scalar word.
        for (cell, word) in &self.cells {
            if cell != home && overlaps(cell, home) {
                result.exact = None;
                result.dependencies.extend(&word.dependencies);
            }
        }
        result
    }

    /// Replace an entire scalar home only after the canonical owner proves
    /// exact physical coverage: zero offset and the full backing scalar extent,
    /// not merely the apparent type of a reinterpreted reference. The retained
    /// backing type is an additional check; it does not establish that extent.
    pub(super) fn write_complete_word(
        &mut self,
        home: DescriptorLocation,
        backing_type: ScalarType,
        word: HandleWordAliases,
    ) -> Result<(), String> {
        let same_extent = |left, right| match (left, right) {
            (ScalarType::Integer(left), ScalarType::Integer(right)) => {
                !left.is_address() && !right.is_address() && left.bits() == right.bits()
            }
            _ => left == right,
        };
        if !same_extent(backing_type, word.scalar_type)
            || self
                .cells
                .get(&home)
                .is_some_and(|old| !same_extent(old.scalar_type, backing_type))
        {
            return Err("complete scalar write does not cover its original backing extent".into());
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
        home: DescriptorLocation,
        word: &HandleWordAliases,
    ) {
        self.write_region(home, word.dependencies.clone());
    }

    /// Replace the complete selected owned region. This does not promise to
    /// replace a containing scalar when the selected region is only a child.
    /// Any such overlapping parent loses exact identity but retains its bits'
    /// dependencies. The caller separately proves the physical coverage of the
    /// selected region; represented partial writes use write_partial_word.
    pub(super) fn replace_complete_region(&mut self, replaced: &DescriptorLocation) {
        self.invalidate_overlapping(replaced);
        self.cells.retain(|cell, _| !replaced.contains(cell));
        self.regions.retain(|region, _| !replaced.contains(region));
    }

    fn invalidate_overlapping(&mut self, written: &DescriptorLocation) {
        for (cell, word) in &mut self.cells {
            if overlaps(cell, written) {
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
        region: DescriptorLocation,
        dependencies: BTreeSet<HandleInstance>,
    ) {
        self.invalidate_overlapping(&region);
        if !dependencies.is_empty() {
            self.regions.entry(region).or_default().extend(dependencies);
        }
    }

    pub(super) fn copy_region(
        &mut self,
        source: &DescriptorLocation,
        destination: &DescriptorLocation,
    ) {
        // Snapshot before any replacement, including overlapping/self copies.
        let cells = self
            .cells
            .iter()
            .filter_map(|(cell, word)| {
                cell.transported(source, destination)
                    .map(|target| (target, word.clone()))
            })
            .collect::<Vec<_>>();
        let mut regions = Vec::new();
        for (region, dependencies) in &self.regions {
            if let Some(target) = region.transported(source, destination) {
                regions.push((target, dependencies.clone()));
            } else if region.contains(source) {
                regions.push((destination.clone(), dependencies.clone()));
            }
        }
        // A partial selection of an imprecisely retained scalar can only carry
        // dependencies, not the exact full word of its containing storage.
        for (cell, word) in &self.cells {
            if cell != source && cell.contains(source) {
                regions.push((destination.clone(), word.dependencies.clone()));
            }
        }
        self.replace_complete_region(destination);
        self.cells.extend(cells);
        for (region, dependencies) in regions {
            self.regions.entry(region).or_default().extend(dependencies);
        }
    }

    pub(super) fn stored_dependencies(
        &self,
        observed: &DescriptorLocation,
    ) -> BTreeSet<HandleInstance> {
        self.cells
            .iter()
            .filter(|(cell, _)| overlaps(cell, observed))
            .flat_map(|(_, word)| word.dependencies.iter())
            .chain(
                self.regions
                    .iter()
                    .filter(|(region, _)| overlaps(region, observed))
                    .flat_map(|(_, dependencies)| dependencies.iter()),
            )
            .copied()
            .collect()
    }
}

fn overlaps(left: &DescriptorLocation, right: &DescriptorLocation) -> bool {
    left.contains(right) || right.contains(left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId};
    use terminal_semantics::descriptors::descriptor_lifetimes::DescriptorPathSegment;

    fn integer(sign: IntegerSign, bits: u16) -> ScalarType {
        ScalarType::Integer(IntegerType::new(sign, bits).unwrap())
    }

    #[test]
    fn copied_outputs_keep_old_handle_dependencies_after_source_replacement_and_narrowing() {
        let word = integer(IntegerSign::Unsigned, 64);
        let acquired = HandleInstance(0);
        let source = DescriptorLocation::whole(PlaceId::new(1).unwrap());
        let destination = DescriptorLocation::whole(PlaceId::new(2).unwrap());
        let mut original = source.clone();
        original.path.push(DescriptorPathSegment::FixedIndex(1));
        let mut copied = destination.clone();
        copied.path = original.path.clone();
        let mut aliases = HandleAliases::default();
        aliases
            .write_complete_word(
                original.clone(),
                word,
                HandleWordAliases::acquired(word, acquired).unwrap(),
            )
            .unwrap();
        aliases.copy_region(&source, &destination);
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
        let home = DescriptorLocation::whole(PlaceId::new(1).unwrap());
        let source = DescriptorLocation::whole(PlaceId::new(2).unwrap());
        let mut child = home.clone();
        child.path.push(DescriptorPathSegment::FixedIndex(0));
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
        copied.copy_region(&source, &child);
        assert!(copied.stored_word(&home, word).exact_handle().is_err());
        assert_eq!(
            copied.stored_dependencies(&home),
            BTreeSet::from([HandleInstance(0)])
        );
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
    fn scalar_arrivals_capture_simultaneously_and_refuse_atomically() {
        let word = integer(IntegerSign::Unsigned, 64);
        let first = ValueId::new(1).unwrap();
        let second = ValueId::new(2).unwrap();
        let mut aliases = HandleAliases::default();
        aliases.define_scalar(
            first,
            HandleWordAliases::acquired(word, HandleInstance(0)).unwrap(),
        );
        aliases.define_scalar(
            second,
            HandleWordAliases::acquired(word, HandleInstance(1)).unwrap(),
        );
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
