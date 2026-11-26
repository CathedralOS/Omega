//! Keep the value a block names separate from the storage it may borrow.
//! A complete list of possible addresses supports overlap checks, but does
//! not prove that two independently selected references choose the same one.

use super::*;
use terminal_semantics::descriptor_results::DescriptorCallOrigin;

#[derive(Debug, Clone)]
struct CapturedRegion {
    place: Lineage,
    /// A captured window's enclosing place bounds overlap. A write through
    /// the window does not initialize that entire place.
    place_is_exact: bool,
    // Keep the complete formation address, rather than only its flattened
    // path. Different capture events are not the same selected reference.
    origin: Option<DescriptorCallOrigin>,
    /// A later execution of this capture can select a different cell.
    index_can_change_on_reentry: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Backing {
    // One address can keep the existing canonical value lookup. Otherwise
    // the block's logical subject stays intact for predecessor substitution.
    pub(super) place: Lineage,
    regions: Vec<CapturedRegion>,
    subject: Option<(BlockId, Lineage)>,
}

#[derive(Clone, Copy)]
enum CaptureComparison {
    Independent,
    /// Descriptor lifetime checking identified the active parent of this reborrow.
    CheckedReborrow,
}

impl Backing {
    pub(super) fn direct(place: Lineage) -> Self {
        Self {
            place: place.clone(),
            regions: vec![CapturedRegion {
                place,
                place_is_exact: true,
                origin: None,
                index_can_change_on_reentry: false,
            }],
            subject: None,
        }
    }

    fn captured(
        subject: &Lineage,
        mut regions: Vec<CapturedRegion>,
        owner: Option<BlockId>,
    ) -> Result<Self, &'static str> {
        regions.sort_by(|left, right| left.place.cmp(&right.place));
        let first = regions
            .first()
            .ok_or("captured storage has no actual address")?;
        let place = if regions.len() == 1 && first.place_is_exact {
            first.place.clone()
        } else {
            subject.clone()
        };
        Ok(Self {
            place,
            regions,
            subject: owner.map(|owner| (owner, subject.clone())),
        })
    }

    /// The same borrowed block input keeps its selected address during this
    /// entry. Crossing that entry or a captured selector retires this shortcut.
    pub(super) fn current_subject_contains(&self, other: &Self, crossed_entries: &[bool]) -> bool {
        let (Some((owner, parent)), Some((other_owner, child))) = (&self.subject, &other.subject)
        else {
            return false;
        };
        owner == other_owner
            && crossed_entries.get(index(owner.get())) == Some(&false)
            && parent.root == child.root
            && child.segments.starts_with(&parent.segments)
            && parent
                .segments
                .iter()
                .chain(&child.segments)
                .all(|segment| !matches!(segment, Segment::RuntimeIndex { .. } | Segment::Referent))
    }

    /// Independently observed references must each name one address. A
    /// recurring instruction can select a new element on its next execution.
    pub(super) fn same_single_region(&self, other: &Self) -> bool {
        match (self.regions.as_slice(), other.regions.as_slice()) {
            ([left], [right]) if left.place_is_exact && right.place_is_exact => {
                left.place == right.place
                    && (left
                        .place
                        .segments
                        .iter()
                        .all(|segment| !matches!(segment, Segment::RuntimeIndex { .. }))
                        || (left.origin == right.origin
                            && Self::region_contains(left, right, CaptureComparison::Independent)))
            }
            _ => false,
        }
    }

    pub(super) fn single_place(&self) -> Option<&Lineage> {
        match self.regions.as_slice() {
            [region] if region.place_is_exact => Some(&region.place),
            _ => None,
        }
    }

    /// Every possible destination must cover every possible selected place.
    /// A captured runtime selector in a family needs a separate correlation
    /// proof; matching selector IDs alone cannot supply that proof.
    pub(super) fn definitely_contains(&self, selected: &Self) -> bool {
        self.contains_regions(selected, CaptureComparison::Independent)
    }

    fn contains_regions(&self, selected: &Self, comparison: CaptureComparison) -> bool {
        if self
            .regions
            .iter()
            .chain(&selected.regions)
            .any(|region| !region.place_is_exact)
        {
            return matches!(comparison, CaptureComparison::CheckedReborrow)
                && self.regions.iter().all(|parent| {
                    selected
                        .regions
                        .iter()
                        .all(|child| Self::region_contains(parent, child, comparison))
                });
        }
        let Some(deepest) = self
            .regions
            .iter()
            .max_by_key(|region| region.place.segments.len())
        else {
            return false;
        };
        let families = self.regions.len() > 1 || selected.regions.len() > 1;
        if !self.regions.iter().all(|parent| {
            parent.place.root == deepest.place.root
                && deepest.place.segments.starts_with(&parent.place.segments)
                && (!families
                    || parent
                        .place
                        .segments
                        .iter()
                        .all(|segment| !matches!(segment, Segment::RuntimeIndex { .. })))
        }) || !selected.regions.iter().all(|child| {
            deepest.place.root == child.place.root
                && child.place.segments.starts_with(&deepest.place.segments)
        }) {
            return false;
        }
        if !deepest
            .place
            .segments
            .iter()
            .any(|step| matches!(step, Segment::RuntimeIndex { .. }))
        {
            return true;
        }
        let ([parent], [child]) = (self.regions.as_slice(), selected.regions.as_slice()) else {
            return false;
        };
        Self::region_contains(parent, child, comparison)
    }

    /// Check the physical projections after the descriptor flow has proved
    /// which lenders the value can carry. Every lender and every projected
    /// region must occur in that correspondence.
    pub(super) fn projections_from<'a>(
        &self,
        owners: impl Clone + Iterator<Item = &'a Self>,
    ) -> bool {
        let projects = |owner: &Self, child: &CapturedRegion| {
            owner.regions.iter().any(|parent| {
                Self::region_contains(parent, child, CaptureComparison::CheckedReborrow)
            })
        };
        self.regions
            .iter()
            .all(|child| owners.clone().any(|owner| projects(owner, child)))
            && owners
                .clone()
                .all(|owner| self.regions.iter().any(|child| projects(owner, child)))
    }

    pub(super) fn contains_reborrow(&self, child: &Self) -> bool {
        self.contains_regions(child, CaptureComparison::CheckedReborrow)
    }

    fn region_contains(
        parent: &CapturedRegion,
        child: &CapturedRegion,
        comparison: CaptureComparison,
    ) -> bool {
        if !parent.place_is_exact
            || (!child.place_is_exact && matches!(comparison, CaptureComparison::Independent))
            || child.place.root != parent.place.root
            || !child.place.segments.starts_with(&parent.place.segments)
        {
            return false;
        }
        if !parent
            .place
            .segments
            .iter()
            .any(|step| matches!(step, Segment::RuntimeIndex { .. }))
        {
            return true;
        }
        if parent.index_can_change_on_reentry
            && matches!(comparison, CaptureComparison::Independent)
        {
            return false;
        }
        let (
            Some(DescriptorCallOrigin::Argument {
                source: a,
                projection: left,
            }),
            Some(DescriptorCallOrigin::Argument {
                source: b,
                projection: right,
            }),
        ) = (&parent.origin, &child.origin)
        else {
            return false;
        };
        let Some(address) = left.captured_address_path().ok() else {
            return false;
        };
        // A suffix added after the replay query has no capture account yet.
        // It cannot borrow authority from the prefix's different capture.
        let retained = address
            .iter()
            .filter_map(|step| step.runtime_index().map(|(index, _)| index));
        let physical = parent.place.segments.iter().filter_map(|step| {
            if let Segment::RuntimeIndex { selector } = step {
                Some(*selector)
            } else {
                None
            }
        });
        a == b && retained.eq(physical) && left.captured_address_is_prefix_of(right)
    }

    pub(super) fn strictly_contains(&self, selected: &Self) -> bool {
        self.definitely_contains(selected)
            && self
                .regions
                .iter()
                .map(|region| region.place.segments.len())
                .max()
                < selected
                    .regions
                    .iter()
                    .map(|region| region.place.segments.len())
                    .min()
    }

    pub(super) fn current_subject_strictly_contains(
        &self,
        selected: &Self,
        crossed: &[bool],
    ) -> bool {
        self.current_subject_contains(selected, crossed)
            && matches!((&self.subject, &selected.subject), (Some((_, a)), Some((_, b)))
                if a.segments.len() < b.segments.len())
    }

    pub(super) fn relative_suffix<'s>(
        &'s self,
        selected: &'s Self,
        crossed: &[bool],
    ) -> Option<&'s [Segment]> {
        if self.current_subject_contains(selected, crossed) {
            let (Some((_, parent)), Some((_, child))) = (&self.subject, &selected.subject) else {
                return None;
            };
            return child.segments.strip_prefix(parent.segments.as_slice());
        }
        if !self.definitely_contains(selected) {
            return None;
        }
        let ([parent], [child]) = (self.regions.as_slice(), selected.regions.as_slice()) else {
            return None;
        };
        child
            .place
            .segments
            .strip_prefix(parent.place.segments.as_slice())
    }

    /// Split window alternatives can share one logical name and different
    /// captured addresses. Keep their reaching equations distinct.
    pub(super) fn single_window(&self) -> Option<&DescriptorCallOrigin> {
        match self.regions.as_slice() {
            [region] if !region.place_is_exact => region.origin.as_ref(),
            _ => None,
        }
    }

    pub(super) fn has_alternatives(&self) -> bool {
        self.regions.len() > 1
    }

    /// Possible effect footprints only; this iterator proves no selected identity.
    /// Each address keeps its capture while an earlier storage version is
    /// checked. The joined descriptor may not yet exist at that point.
    pub(super) fn captured_regions(&self) -> impl Iterator<Item = Self> + '_ {
        self.regions.iter().map(|region| Self {
            place: if region.place_is_exact {
                region.place.clone()
            } else {
                self.place.clone()
            },
            regions: vec![region.clone()],
            subject: None,
        })
    }

    pub(super) fn possible_regions(&self) -> impl Iterator<Item = &Lineage> {
        self.regions.iter().map(|region| &region.place)
    }

    pub(super) fn all_roots(&self, mut predicate: impl FnMut(PlaceId) -> bool) -> bool {
        self.regions
            .iter()
            .all(|region| predicate(region.place.root))
    }
}

impl StorageAddresses for Backing {
    fn address_count(&self) -> usize {
        self.regions.len()
    }

    fn address(&self, index: usize) -> &Lineage {
        &self.regions[index].place
    }
}

impl<'a> Storage<'a> {
    /// Retain only ancestors below the same typed borrowed arrival, using
    /// the current copy operand and a complete single original address.
    /// This caps solved depth; it gives no new value or selection identity.
    pub(super) fn borrowed_transfer_depth(
        &self,
        tables: &MachineTables,
        original: &Lineage,
        backing: &Backing,
        copied: &Lineage,
        destination: &Lineage,
        (block, before): (BlockId, usize),
        concerns: Construct,
    ) -> Checked<usize> {
        let (Some((owner, subject)), [region]) = (&backing.subject, backing.regions.as_slice())
        else {
            return Ok(0);
        };
        let readable =
            terminal_semantics::structural_argument_source(self.lowered, original.root, 0)
                .is_some_and(|parameter| {
                    matches!(
                        parameter.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                });
        if !region.place_is_exact
            || *owner != block
            || subject != original
            || !readable
            || original == &backing.place
            || !matches!(
                &region.origin,
                Some(DescriptorCallOrigin::Argument { source, projection })
                    if *source == region.place.root && !projection.indirect && projection.window.is_none()
            )
        {
            return Ok(0);
        }
        if !matches!(
            (self.shape(copied, concerns)?, self.shape(destination, concerns)?),
            (Shape::Typed(source), Shape::Typed(result)) if source == result
        ) {
            return Ok(0);
        }
        let Some(prefix) = region
            .place
            .segments
            .strip_suffix(original.segments.as_slice())
        else {
            return Ok(0);
        };
        if !prefix.iter().all(|step| {
            matches!(
                step,
                Segment::Field(_) | Segment::Case(_) | Segment::FixedIndex(_)
            )
        }) {
            return Ok(0);
        }
        let physical_owner = Lineage {
            root: region.place.root,
            segments: prefix.to_vec(),
        };
        let logical_owner = Lineage {
            root: original.root,
            segments: Vec::new(),
        };
        if !matches!(
            (self.shape(&logical_owner, concerns)?, self.shape(&physical_owner, concerns)?),
            (Shape::Typed(logical), Shape::Typed(physical)) if logical == physical
        ) {
            return Ok(0);
        }

        let leaf = self.shape_with(
            original.root,
            &original.segments,
            concerns,
            |shape, step| match step {
                Segment::Field(_) | Segment::Case(_) => Ok(true),
                Segment::FixedIndex(_) | Segment::RuntimeIndex { .. } => {
                    let Shape::Typed(ty) = shape else {
                        return Ok(false);
                    };
                    Ok(matches!(
                        self.declaration(*ty, concerns)?.shape,
                        StructuralTypeShape::FixedArray { .. }
                    ))
                }
                _ => Ok(false),
            },
        )?;
        if !matches!(leaf, Some(Shape::Scalar(_))) {
            return Ok(0);
        }
        // These selectors are evaluated for this transfer, not compared
        // with selectors at another point in the reaching walk.
        for step in &original.segments {
            let Segment::RuntimeIndex { selector } = step else {
                continue;
            };
            let current = match tables
                .values
                .checked(*selector, self.machine, concerns)?
                .origin
            {
                ValueOrigin::MachineParameter { .. } => true,
                ValueOrigin::BlockParameter { block: defined, .. } => defined == block,
                ValueOrigin::Operation(operation) => {
                    let (defined, position) =
                        self.graph.locate(ProgramPoint::AfterOperation(operation))?;
                    defined == block && position <= before
                }
                ValueOrigin::MachineResult
                | ValueOrigin::StorageContent { .. }
                | ValueOrigin::StorageCase { .. } => false,
            };
            if !current {
                return Ok(0);
            }
        }
        Ok(original.segments.len())
    }

    /// A whole same-typed sum replacement can only hit this fixed-array
    /// payload's cell or miss it. Different selectors need no equality proof
    /// here: this relation supplies neither an address nor a scalar identity.
    pub(super) fn selected_payload_cell_family(
        &self,
        write: &Backing,
        selected: &Backing,
        case: StructuralCaseId,
        concerns: Construct,
    ) -> Checked<Option<StructuralTypeId>> {
        let ([parent], [child]) = (write.regions.as_slice(), selected.regions.as_slice()) else {
            return Ok(None);
        };
        if !parent.place_is_exact || !child.place_is_exact {
            return Ok(None);
        }
        let Some((Segment::Field(field), payload)) = child.place.segments.split_last() else {
            return Ok(None);
        };
        let Some((Segment::Case(selected_case), cell)) = payload.split_last() else {
            return Ok(None);
        };
        if *selected_case != case
            || parent.place.root != child.place.root
            || parent.place.segments.len() != cell.len()
        {
            return Ok(None);
        }
        match (&parent.origin, &child.origin) {
            (None, None) => {}
            (
                Some(DescriptorCallOrigin::Argument {
                    source: a,
                    projection: left,
                }),
                Some(DescriptorCallOrigin::Argument {
                    source: b,
                    projection: right,
                }),
            ) if a == b
                && *a == parent.place.root
                && !left.indirect
                && !right.indirect
                && left.window.is_none()
                && right.window.is_none() =>
            {
                let (Ok(left), Ok(right)) =
                    (left.captured_address_path(), right.captured_address_path())
                else {
                    return Ok(None);
                };
                if left != right
                    || !left.iter().all(|step| {
                        matches!(
                            step,
                            StructuralPathSegment::Field(_)
                                | StructuralPathSegment::Case(_)
                                | StructuralPathSegment::FixedIndex(_)
                        )
                    })
                {
                    return Ok(None);
                }
            }
            _ => return Ok(None),
        }
        let mut indexed = false;
        for (written, observed) in parent.place.segments.iter().zip(cell) {
            match (written, observed) {
                (Segment::Field(left), Segment::Field(right)) if left == right => {}
                (Segment::Case(left), Segment::Case(right)) if left == right => {}
                (
                    Segment::FixedIndex(_) | Segment::RuntimeIndex { .. },
                    Segment::FixedIndex(_) | Segment::RuntimeIndex { .. },
                ) => indexed = true,
                _ => return Ok(None),
            }
        }
        if !indexed {
            return Ok(None);
        }
        let shape_at = |root, path: &[Segment]| {
            self.shape_with(root, path, concerns, |shape, segment| match segment {
                Segment::Field(_) | Segment::Case(_) => Ok(true),
                Segment::FixedIndex(_) | Segment::RuntimeIndex { .. } => {
                    let Shape::Typed(ty) = shape else {
                        return Ok(false);
                    };
                    Ok(matches!(
                        self.declaration(*ty, concerns)?.shape,
                        StructuralTypeShape::FixedArray { .. }
                    ))
                }
                _ => Ok(false),
            })
        };
        let (Some(Shape::Typed(sum)), Some(Shape::Typed(other))) = (
            shape_at(parent.place.root, &parent.place.segments)?,
            shape_at(child.place.root, cell)?,
        ) else {
            return Ok(None);
        };
        if sum != other {
            return Ok(None);
        }
        let StructuralTypeShape::Sum { cases } = &self.declaration(sum, concerns)?.shape else {
            return Ok(None);
        };
        Ok(cases
            .iter()
            .any(|selected| {
                selected.id == case
                    && selected.fields.iter().any(|member| {
                        member.id == *field && member.field_type.as_scalar().is_some()
                    })
            })
            .then_some(sum))
    }

    /// A store and read with the same current index value reach the same
    /// scalar. This supplies its value without proving physical containment,
    /// case membership, establishment or loan authority.
    pub(super) fn stored_scalar_suffix<'b>(
        &self,
        tables: &MachineTables,
        row: &'b Logged,
        selected: &'b Backing,
        trace: &mut Reaching,
        concerns: Construct,
    ) -> Checked<Option<&'b [Segment]>> {
        if !matches!(row.stored, Stored::Value(_)) {
            return Ok(None);
        }
        let ([parent], [child]) = (row.backing.regions.as_slice(), selected.regions.as_slice())
        else {
            return Ok(None);
        };
        if !parent.place_is_exact
            || !child.place_is_exact
            || parent.place.root != child.place.root
            || !child.place.segments.starts_with(&parent.place.segments)
            || !parent
                .place
                .segments
                .iter()
                .any(|step| matches!(step, Segment::RuntimeIndex { .. }))
        {
            return Ok(None);
        }
        match (&parent.origin, &child.origin) {
            (None, None)
                if !parent.place.segments.contains(&Segment::Referent)
                    && !child.place.segments.contains(&Segment::Referent) => {}
            (
                Some(DescriptorCallOrigin::Argument {
                    source: a,
                    projection: left,
                }),
                Some(DescriptorCallOrigin::Argument {
                    source: b,
                    projection: right,
                }),
            ) if a == b
                && !parent.index_can_change_on_reentry
                && left.captured_address_is_prefix_of(right) => {}
            (Some(DescriptorCallOrigin::Argument { source, projection }), None)
            | (None, Some(DescriptorCallOrigin::Argument { source, projection }))
                if *source == parent.place.root
                    && !projection.indirect
                    && projection.window.is_none()
                    && child.place.segments.iter().all(|step| {
                        matches!(
                            step,
                            Segment::Field(_)
                                | Segment::Case(_)
                                | Segment::FixedIndex(_)
                                | Segment::RuntimeIndex { .. }
                        )
                    }) => {}
            _ => return Ok(None),
        }
        // Initialization and unscoped queries do not identify a current
        // selector evaluation. Keep their existing conservative supply.
        let Some(used) = trace.used else {
            return Ok(None);
        };
        let before = row.position.checked_sub(1).ok_or_else(|| {
            self.foreign(
                concerns,
                "a current storage selection has no store prestate",
            )
        })?;
        let stored = if let Some(previous) = before.checked_sub(1) {
            let operation = self
                .block(row.block)?
                .operations
                .get(previous)
                .ok_or_else(|| {
                    self.foreign(concerns, "a current storage selection is past its block")
                })?;
            ProgramPoint::AfterOperation(operation.id)
        } else {
            ProgramPoint::BlockEntry(row.block)
        };
        let mut entries = Vec::new();
        // A block's structural actual, or an operation's fresh storage, is
        // rebound as well as its scalar selectors when its entry is crossed.
        let root_definition = match self.root_kind(parent.place.root, concerns)? {
            StructuralPlaceKind::BlockParameter { block, .. } => {
                Some(ProgramPoint::BlockEntry(block))
            }
            StructuralPlaceKind::OperationResult { producer, .. } => {
                Some(ProgramPoint::AfterOperation(producer))
            }
            _ => None,
        };
        if let Some(definition) = root_definition {
            if !self.graph.dominates(definition, stored)?
                || !self.graph.dominates(definition, used)?
            {
                return Ok(None);
            }
            entries.push(self.graph.locate(definition)?.0);
        }
        for step in parent.place.segments.iter().chain(&child.place.segments) {
            let Segment::RuntimeIndex { selector } = step else {
                continue;
            };
            let origin = tables
                .values
                .checked(*selector, self.machine, concerns)?
                .origin;
            let definition = match origin {
                ValueOrigin::MachineParameter { .. } => continue,
                ValueOrigin::BlockParameter { block, .. } => ProgramPoint::BlockEntry(block),
                ValueOrigin::Operation(operation) => ProgramPoint::AfterOperation(operation),
                ValueOrigin::MachineResult
                | ValueOrigin::StorageContent { .. }
                | ValueOrigin::StorageCase { .. } => return Ok(None),
            };
            if !self.graph.dominates(definition, stored)?
                || !self.graph.dominates(definition, used)?
            {
                return Ok(None);
            }
            entries.push(self.graph.locate(definition)?.0);
        }
        entries.sort_unstable();
        entries.dedup();
        // A capture may predate the store or read at the ends of this
        // walk. Comparing it to a fresh selection is sound when the root
        // and selectors each have one evaluation per invocation.
        let compares_capture_to_direct = parent.origin.is_some() != child.origin.is_some();
        for owner in &entries {
            if trace.crossed_entries.get(index(owner.get())) != Some(&false)
                || (compares_capture_to_direct && self.graph.in_cycle(*owner)?)
            {
                return Ok(None);
            }
        }
        trace.selector_entries.extend(entries);
        trace.selector_entries.sort_unstable();
        trace.selector_entries.dedup();
        Ok(child
            .place
            .segments
            .strip_prefix(parent.place.segments.as_slice()))
    }

    /// Capture every complete storage address at the actual use. Unknown
    /// correspondence remains an error; it never becomes a new local root.
    pub(super) fn backing(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        place: &Lineage,
        block: BlockId,
        before: usize,
        concerns: Construct,
    ) -> Checked<Backing> {
        let borrowed_arrival = self.is_borrowed_arrival(place.root, concerns)?;
        if !borrowed_arrival && !place.segments.contains(&Segment::Referent) {
            return Ok(Backing::direct(place.clone()));
        }
        let before = crate::position(
            before.min(self.block(block)?.operations.len()),
            self.machine,
        )?;
        let split = place
            .segments
            .iter()
            .position(|segment| matches!(segment, Segment::RuntimeIndex { .. } | Segment::Extent))
            .unwrap_or(place.segments.len());
        let (prefix, suffix) = place.segments.split_at(split);
        if suffix.contains(&Segment::Referent) {
            return Err(self.unsupported(
                concerns,
                "reference backing crosses a permission beneath an unresolved selector",
            ));
        }
        let location = Lineage {
            root: place.root,
            segments: prefix.to_vec(),
        }
        .descriptor_location()
        .ok_or_else(|| {
            self.unsupported(
                concerns,
                "reference backing has no complete descriptor coordinate",
            )
        })?;
        let addresses = references
            .captured_storage_family(self.machine, (block, before), &location)
            .map_err(|detail| self.unsupported(concerns, detail))?;
        let owner = if borrowed_arrival && !place.segments.contains(&Segment::Referent) {
            match self.root_kind(place.root, concerns)? {
                StructuralPlaceKind::BlockParameter { block, .. } => Some(block),
                _ => None,
            }
        } else {
            None
        };
        self.captured_backing(place, suffix, addresses, owner, concerns)
    }

    /// Ordinary uses and cleanup actions interpret the same authenticated
    /// addresses; only the replay point that supplies them differs.
    pub(super) fn captured_backing(
        &self,
        subject: &Lineage,
        suffix: &[Segment],
        addresses: Vec<DescriptorCallOrigin>,
        owner: Option<BlockId>,
        concerns: Construct,
    ) -> Checked<Backing> {
        let mut regions = Vec::with_capacity(addresses.len());
        for origin in addresses {
            let DescriptorCallOrigin::Argument { source, projection } = &origin else {
                return Err(self.unsupported(
                    concerns,
                    "reference backing is not an original storage address",
                ));
            };
            let (resolved, place_is_exact) = match projection
                .validate_captured_storage_address(|_| true)
                .map_err(|detail| self.unsupported(concerns, detail))?
            {
                Some(path) => (self.stored_beneath(*source, &path, suffix, concerns)?, true),
                None => {
                    let owner = projection.enclosing_storage().ok_or_else(|| {
                        self.unsupported(concerns, "a captured window has no enclosing storage")
                    })?;
                    (self.stored_beneath(*source, &owner.path, &[], concerns)?, false)
                }
            };
            let mut index_can_change_on_reentry = false;
            for (selector, (block, _)) in projection.index_captures() {
                if self.graph.in_cycle(block)?
                    && !self
                        .lowered
                        .parameters
                        .iter()
                        .any(|param| param.id == selector)
                {
                    index_can_change_on_reentry = true;
                    break;
                }
            }
            regions.push(CapturedRegion {
                place: resolved,
                place_is_exact,
                origin: Some(origin),
                index_can_change_on_reentry,
            });
        }
        Backing::captured(subject, regions, owner)
            .map_err(|detail| self.unsupported(concerns, detail))
    }
}

#[cfg(test)]
mod address_comparison_tests {
    use super::*;

    #[test]
    fn a_window_overlaps_its_owner_without_becoming_the_whole_place() {
        use terminal_semantics::borrowed_storage::Projection;

        let root = PlaceId::new(1).unwrap();
        let bounds = Lineage {
            root,
            segments: Vec::new(),
        };
        let subject = Lineage {
            root: PlaceId::new(4).unwrap(),
            segments: Vec::new(),
        };
        let capture = DescriptorCallOrigin::Argument {
            source: root,
            projection: Projection::window(
                ValueId::new(1).unwrap(),
                ValueId::new(2).unwrap(),
                (BlockId::new(1).unwrap(), 0),
            ),
        };
        let window = Backing::captured(
            &subject,
            vec![CapturedRegion {
                place: bounds.clone(),
                place_is_exact: false,
                origin: Some(capture.clone()),
                index_can_change_on_reentry: true,
            }],
            None,
        )
        .unwrap();
        let whole = Backing::direct(bounds);
        assert!(overlap(&window, &whole));
        assert_eq!(window.place, subject);
        assert_eq!(window.single_place(), None);
        assert_eq!(window.single_window(), Some(&capture));
        assert!(!window.has_alternatives());
        assert!(!window.same_single_region(&whole));
        assert!(!window.definitely_contains(&whole));
        assert!(!whole.definitely_contains(&window));
        assert!(whole.contains_reborrow(&window));
        assert_eq!(window.relative_suffix(&whole, &[]), None);
    }

    #[test]
    fn complete_address_families_share_the_single_place_overlap_policy() {
        let root = |id| PlaceId::new(id).unwrap();
        let field = |id| Segment::Field(semantic_vocabulary::StructuralFieldId::new(id).unwrap());
        let place = |root, segments| Lineage { root, segments };
        let family = |addresses: Vec<Lineage>| {
            let subject = addresses[0].clone();
            Backing::captured(
                &subject,
                addresses
                    .into_iter()
                    .map(|place| CapturedRegion {
                        place,
                        place_is_exact: true,
                        origin: None,
                        index_can_change_on_reentry: false,
                    })
                    .collect(),
                None,
            )
            .unwrap()
        };
        let left = family(vec![
            place(root(1), vec![field(1)]),
            place(root(2), vec![field(1)]),
        ]);
        let disjoint = family(vec![
            place(root(1), vec![field(2)]),
            place(root(3), vec![field(1)]),
        ]);
        assert!(!overlap(&left, &disjoint));
        assert!(!overlap(&disjoint, &left));
        let descendant = family(vec![
            place(root(1), vec![field(1), field(2)]),
            place(root(3), vec![field(2)]),
        ]);
        assert!(overlap(&left, &descendant));
        let runtime = family(vec![
            place(
                root(1),
                vec![Segment::RuntimeIndex {
                    selector: ValueId::new(1).unwrap(),
                }],
            ),
            place(root(2), vec![field(1)]),
        ]);
        let fixed = family(vec![
            place(root(1), vec![Segment::FixedIndex(0)]),
            place(root(3), vec![field(2)]),
        ]);
        assert!(overlap(&runtime, &fixed));
        let singleton = place(root(1), vec![field(1), field(3)]);
        assert!(overlap(&singleton, &left));
        assert!(overlap(&left, &singleton));
        let other_root = place(root(4), vec![field(1)]);
        assert!(!overlap(&other_root, &left));
        assert!(!overlap(&left, &other_root));
    }
}
