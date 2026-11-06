//! Keep each real normal exit only after its writable slots and parent closure
//! are complete. The CFG owner seals this candidate after joins and coverage.
use super::*;

/// A replay owns its candidate until CFG joins, every real normal exit and
/// returned-origin coverage have succeeded. No candidate enters the catalog.
pub(crate) struct ReferenceOutputBuilder {
    machine: MachineId,
    entry: Vec<LiveReference>,
    slots: Vec<ReferenceOutputSlot>,
    slot_order: Vec<usize>,
    incomplete_slots: Option<&'static str>,
    exits: Vec<ReferenceOutputExit>,
}

impl ReferenceOutputBuilder {
    pub(crate) fn new(
        function: &PsiOptimizationFunction,
        types: &[StructuralTypeDeclaration],
        entry: &[LiveReference],
    ) -> Result<Self, &'static str> {
        let mut slots = Vec::new();
        let mut incomplete_slots = None;
        for (position, parameter) in function.structural_parameters.iter().enumerate() {
            let Some(writable) =
                terminal_semantics::descriptor_results::DescriptorSlotKind::References
                    .writable_slots(
                        types,
                        parameter.place,
                        parameter.structural_type,
                        parameter.access,
                    )
            else {
                incomplete_slots = Some("a reference output has no complete typed slot inventory");
                continue;
            };
            for slot in writable {
                let path = terminal_semantics::descriptor_lifetimes::descriptor_path(
                    types,
                    parameter.structural_type,
                    &slot.path,
                )
                .ok_or("a reference output has no typed storage path")?;
                let selected = terminal_semantics::runtime_structural_path_tip(
                    types.iter(),
                    parameter.structural_type,
                    &path,
                )
                .ok_or("a reference output has no current reference type")?;
                slots.push(ReferenceOutputSlot {
                    parameter: position,
                    path,
                    reference_type: selected.id,
                });
            }
        }
        Ok(Self {
            machine: function.machine,
            incomplete_slots,
            entry: entry.to_vec(),
            slot_order: {
                let mut indices = (0..slots.len()).collect::<Vec<_>>();
                indices.sort_by(|left, right| {
                    let key = |index: usize| {
                        let slot = &slots[index];
                        (
                            function.structural_parameters[slot.parameter].place,
                            &slot.path,
                        )
                    };
                    key(*left).cmp(&key(*right))
                });
                indices
            },
            slots,
            exits: Vec::new(),
        })
    }

    pub(crate) fn record(
        &mut self,
        block: BlockId,
        returned: Vec<LiveReference>,
        stored: Vec<LiveReference>,
    ) {
        let exit = ReferenceOutputExit {
            block,
            returned,
            stored,
            graph: Vec::new(),
        };
        if let Some(previous) = self.exits.iter_mut().find(|exit| exit.block == block) {
            *previous = exit;
        } else {
            self.exits.push(exit);
        }
    }

    pub(crate) fn validate_exit(
        &self,
        function: &PsiOptimizationFunction,
        returned: &[LiveReference],
        live: &[LiveReference],
        inactive_case: impl Fn(PlaceId, &[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Result<(), &'static str> {
        if self.incomplete_slots.is_some() {
            // An optional output summary must not reject an unchanged ordinary
            // body merely because its writable pointee inventory is recursive.
            // Keep the previous exact borrowed-ingress restoration judgment;
            // this branch cannot publish a complete output graph at seal().
            let mut actual = live.iter().collect::<Vec<_>>();
            let mut expected = self
                .entry
                .iter()
                .filter(|reference| is_entry_storage(function, reference.carrier))
                .collect::<Vec<_>>();
            actual.sort_by(|left, right| left.identity.cmp(&right.identity));
            expected.sort_by(|left, right| left.identity.cmp(&right.identity));
            return if actual == expected {
                Ok(())
            } else {
                Err("unknown writable reference outputs require exact unchanged ingress custody")
            };
        }
        fn location<'slot>(
            function: &PsiOptimizationFunction,
            slot: &'slot ReferenceOutputSlot,
        ) -> (PlaceId, &'slot [StructuralPathSegment]) {
            (
                function.structural_parameters[slot.parameter].place,
                &slot.path,
            )
        }
        let mut residents = live.iter().collect::<Vec<_>>();
        residents.sort_by(|left, right| {
            (left.carrier, &left.carrier_path).cmp(&(right.carrier, &right.carrier_path))
        });
        for slot in &self.slots {
            let (place, path) = location(function, slot);
            let first = residents.partition_point(|reference| {
                (reference.carrier, reference.carrier_path.as_slice()) < (place, path)
            });
            let after = residents.partition_point(|reference| {
                (reference.carrier, reference.carrier_path.as_slice()) <= (place, path)
            });
            let count = after - first;
            if count > 1 {
                return Err("a stored reference output has ambiguous live occurrences");
            }
            if count == 0
                && !path.iter().enumerate().any(|(position, segment)| {
                    matches!(segment, StructuralPathSegment::Case(case)
                        if inactive_case(place, &path[..position], *case))
                })
            {
                return Err("a stored reference output is neither live nor proved inactive");
            }
        }
        let is_output = |reference: &LiveReference| {
            self.slot_order
                .binary_search_by(|index| {
                    location(function, &self.slots[*index])
                        .cmp(&(reference.carrier, reference.carrier_path.as_slice()))
                })
                .is_ok()
        };
        // Read-only ingress is still the exact borrowed permission. A normal
        // output roster cannot make an unexplained local loan disappear.
        let mut remaining = live
            .iter()
            .filter(|reference| !is_output(reference))
            .collect::<Vec<_>>();
        let mut expected = self
            .entry
            .iter()
            .filter(|reference| {
                is_entry_storage(function, reference.carrier) && !is_output(reference)
            })
            .collect::<Vec<_>>();
        remaining.sort_by(|left, right| left.identity.cmp(&right.identity));
        expected.sort_by(|left, right| left.identity.cmp(&right.identity));
        if remaining != expected {
            return Err("normal completion does not restore borrowed reference custody");
        }
        let mut current = returned.iter().chain(live).collect::<Vec<_>>();
        current.sort_by(|left, right| left.identity.cmp(&right.identity));
        if current
            .windows(2)
            .any(|rows| rows[0].identity == rows[1].identity && rows[0] != rows[1])
        {
            return Err("normal reference outputs disagree about one permission occurrence");
        }
        current.dedup_by(|left, right| left.identity == right.identity);
        let mut entry = self.entry.iter().collect::<Vec<_>>();
        entry.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut entry_roots = entry
            .iter()
            .map(|reference| &reference.root)
            .collect::<Vec<_>>();
        entry_roots.sort();
        entry_roots.dedup();
        let mut complete = vec![false; current.len()];
        let mut active = vec![false; current.len()];
        for index in 0..current.len() {
            self.validate_parent(
                function,
                &current,
                &entry,
                &entry_roots,
                index,
                &mut active,
                &mut complete,
            )?;
        }
        Ok(())
    }

    fn validate_parent(
        &self,
        function: &PsiOptimizationFunction,
        current: &[&LiveReference],
        entry: &[&LiveReference],
        entry_roots: &[&ReferenceOrigin],
        index: usize,
        active: &mut [bool],
        complete: &mut [bool],
    ) -> Result<(), &'static str> {
        if complete[index] {
            return Ok(());
        }
        if active[index] {
            return Err("normal reference outputs contain cyclic permission ancestry");
        }
        active[index] = true;
        let reference = current[index];
        let anchored = entry
            .binary_search_by(|incoming| incoming.identity.cmp(&reference.identity))
            .ok()
            .is_some_and(|position| {
                entry[position].root == reference.root && entry[position].parent == reference.parent
            });
        if !anchored {
            match &reference.parent {
                ReferenceParent::Reference(parent) => {
                    if let Ok(parent_index) =
                        current.binary_search_by(|candidate| candidate.identity.cmp(parent))
                    {
                        self.validate_parent(
                            function,
                            current,
                            entry,
                            entry_roots,
                            parent_index,
                            active,
                            complete,
                        )?;
                    } else if entry
                        .binary_search_by(|candidate| candidate.identity.cmp(parent))
                        .is_err()
                    {
                        return Err("a normal reference output has an unaccounted local parent");
                    }
                }
                ReferenceParent::Root(root) => {
                    if root != &reference.root || !external_root(function, entry_roots, root) {
                        return Err("a normal reference output has no captured external anchor");
                    }
                }
            }
        }
        active[index] = false;
        complete[index] = true;
        Ok(())
    }

    pub(crate) fn seal(
        mut self,
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        compatibility_requirements: Vec<
            terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement,
        >,
    ) -> Result<ReferenceBodyOutputs, &'static str> {
        if let Some(reason) = self.incomplete_slots {
            return Err(reason);
        }
        for exit in &mut self.exits {
            exit.graph = output_graph(function, types, &self.entry, &self.slots, exit)?;
        }
        fn retain_origins(origins: &[ReferenceOutputOrigin], entries: &mut Vec<usize>) {
            entries.extend(origins.iter().filter_map(|origin| match origin {
                ReferenceOutputOrigin::Entry { entry, .. } => Some(*entry),
                ReferenceOutputOrigin::Storage { .. } => None,
            }));
        }
        let mut required_entries = Vec::new();
        for node in self.exits.iter().flat_map(|exit| &exit.graph) {
            match node.supply() {
                ReferenceOutputSupply::Entry(entry) => required_entries.push(*entry),
                ReferenceOutputSupply::Constructed { origins, parent } => {
                    retain_origins(origins, &mut required_entries);
                    match parent {
                        ReferenceOutputParent::Entry(entry) => required_entries.push(*entry),
                        ReferenceOutputParent::Root(origins) => {
                            retain_origins(origins, &mut required_entries)
                        }
                        ReferenceOutputParent::Node(_) => {}
                    }
                }
            }
        }
        required_entries.sort_unstable();
        required_entries.dedup();
        Ok(ReferenceBodyOutputs {
            machine: self.machine,
            entry: self.entry,
            slots: self.slots,
            exits: self.exits,
            required_entries,
            compatibility_requirements,
        })
    }
}
