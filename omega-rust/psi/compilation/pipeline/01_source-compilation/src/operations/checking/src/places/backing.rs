//! Keep the value a block names separate from the storage it may borrow.
//! A complete list of possible addresses supports overlap checks, but does
//! not prove that two independently selected references choose the same one.

use super::*;
use terminal_semantics::descriptor_results::DescriptorCallOrigin;

#[derive(Debug, Clone)]
struct CapturedRegion {
    place: Lineage,
    // Keep the complete formation address, rather than only its flattened
    // path. Different capture events are not the same selected reference.
    origin: Option<DescriptorCallOrigin>,
}

#[derive(Debug, Clone)]
pub(crate) struct Backing {
    // One address can keep the existing canonical value lookup. Otherwise
    // the block's logical subject stays intact for predecessor substitution.
    pub(super) place: Lineage,
    regions: Vec<CapturedRegion>,
    subject: Option<(BlockId, Lineage)>,
}

impl Backing {
    pub(super) fn direct(place: Lineage) -> Self {
        Self {
            place: place.clone(),
            regions: vec![CapturedRegion {
                place,
                origin: None,
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
        let place = if regions.len() == 1 {
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

    /// A single original address retains the established comparison rule.
    /// Equal lists of alternatives do not prove a correlated selection.
    pub(super) fn same_single_region(&self, other: &Self) -> bool {
        match (self.regions.as_slice(), other.regions.as_slice()) {
            ([left], [right]) => {
                left.place == right.place
                    && (left
                        .place
                        .segments
                        .iter()
                        .all(|segment| !matches!(segment, Segment::RuntimeIndex { .. }))
                        || matches!((&left.origin, &right.origin), (Some(left), Some(right)) if left == right))
            }
            _ => false,
        }
    }

    pub(super) fn single_place(&self) -> Option<&Lineage> {
        match self.regions.as_slice() {
            [region] => Some(&region.place),
            _ => None,
        }
    }

    /// Every possible destination must cover every possible selected place.
    /// A captured runtime selector in a family needs a separate correlation
    /// proof; matching selector IDs alone cannot supply that proof.
    pub(super) fn definitely_contains(&self, selected: &Self) -> bool {
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

    /// Possible effect footprints only; this iterator proves no selected identity.
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
        let borrowed_arrival =
            matches!(
                self.root_kind(place.root, concerns)?,
                StructuralPlaceKind::BlockParameter { .. }
            ) && terminal_semantics::structural_argument_source(self.lowered, place.root, 0)
                .is_some_and(|parameter| {
                    parameter.access != StructuralAccess::Owned
                        && !terminal_semantics::descriptor_results::is_descriptor(
                            self.types.iter(),
                            parameter.structural_type,
                        )
                });
        if !borrowed_arrival && !place.segments.contains(&Segment::Referent) {
            return Ok(Backing::direct(place.clone()));
        }
        let before = crate::checking::position(
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
            let path = projection
                .captured_address_path()
                .map_err(|detail| self.unsupported(concerns, detail))?;
            let mut resolved = self.stored_at(*source, &path, concerns)?;
            resolved.segments.extend_from_slice(suffix);
            self.shape(&resolved, concerns)?;
            regions.push(CapturedRegion {
                place: resolved,
                origin: Some(origin),
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
                        origin: None,
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
