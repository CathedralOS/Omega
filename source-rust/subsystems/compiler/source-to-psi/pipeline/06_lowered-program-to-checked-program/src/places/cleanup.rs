//! Cleanup actions use real edge epochs in the existing storage proof walk.
use super::*;
use terminal_check_tables::bindings::Content;

pub(crate) struct AtCleanupCall<'s, 'a> {
    inputs: AtCall<'s, 'a>,
    edge: EdgeId,
    action_ordinal: u32,
}

impl<'a> Storage<'a> {
    pub(crate) fn at_cleanup_call<'s>(
        &'s self,
        references: &'s mut DescriptorAnalysis<'a>,
        tables: &'s MachineTables,
        actuals: &[Actual],
        (formals, extent_owner): (
            &'s [terminal_psi::StructuralParameterDeclaration],
            Option<MachineId>,
        ),
        edge: EdgeId,
        action_ordinal: u32,
        concerns: Construct,
    ) -> Checked<AtCleanupCall<'s, 'a>> {
        let (block, up_to) = self.graph.locate(ProgramPoint::BeforeCleanup {
            edge,
            action_ordinal,
        })?;
        let places = actuals
            .iter()
            .filter_map(|actual| match actual {
                Actual::Place(place) => Some(lineage(self.machine, &tables.places, *place)),
                _ => None,
            })
            .collect::<Checked<Vec<_>>>()?;
        Ok(AtCleanupCall {
            inputs: AtCall {
                storage: self,
                references,
                tables,
                scalars: crate::scalar_actuals(actuals),
                places,
                callee_formals: formals,
                extent_owner,
                block,
                up_to,
                concerns,
            },
            edge,
            action_ordinal,
        })
    }

    pub(super) fn cleanup_backing(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        place: &Lineage,
        edge: EdgeId,
        ordinal: u32,
        concerns: Construct,
    ) -> Checked<Backing> {
        let (block, before) = self.graph.locate(ProgramPoint::BeforeCleanup {
            edge,
            action_ordinal: ordinal,
        })?;
        let borrowed_arrival = self.is_borrowed_arrival(place.root, concerns)?;
        if !borrowed_arrival && !place.segments.contains(&Segment::Referent) {
            return Ok(Backing::direct(place.clone()));
        }
        let split = place
            .segments
            .iter()
            .position(|segment| matches!(segment, Segment::RuntimeIndex { .. } | Segment::Extent))
            .unwrap_or(place.segments.len());
        let (prefix, suffix) = place.segments.split_at(split);
        if suffix.contains(&Segment::Referent) {
            return Err(self.unsupported(
                concerns,
                "cleanup backing crosses a permission beneath an unresolved selector",
            ));
        }
        let descriptor = Lineage {
            root: place.root,
            segments: prefix.to_vec(),
        }
        .descriptor_location()
        .ok_or_else(|| {
            self.unsupported(
                concerns,
                "cleanup observation needs an exact captured storage path",
            )
        })?;
        let addresses = references
            .captured_storage_family_at(
                self.machine,
                (block, crate::position(before, self.machine)?),
                terminal_psi::InvocationOwner::Cleanup {
                    edge,
                    action_ordinal: ordinal,
                },
                &descriptor,
            )
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

    fn cleanup_untouched(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        place: &Lineage,
        edge: EdgeId,
        ordinal: u32,
        concerns: Construct,
    ) -> Checked<bool> {
        let place = self.cleanup_backing(references, place, edge, ordinal, concerns)?;
        Ok(self
            .cleanup_writes
            .iter()
            .filter(|write| write.edge == edge && write.action_ordinal < ordinal)
            .all(|write| {
                !overlap(
                    &place,
                    &Backing::direct(Lineage {
                        root: write.backing.root,
                        segments: write.backing.path.clone(),
                    }),
                )
            }))
    }

    pub(super) fn cleanup_at_point(
        &self,
        asked: Asked<'_>,
        watched: &Backing,
        block: BlockId,
        point: ProgramPoint,
    ) -> Checked<Option<Reached>> {
        let (edge, limit) = match point {
            ProgramPoint::BeforeCleanup {
                edge,
                action_ordinal,
            } => (edge, Some((action_ordinal, false))),
            ProgramPoint::AfterCleanup {
                edge,
                action_ordinal,
            } => (edge, Some((action_ordinal, true))),
            ProgramPoint::Edge(edge) => (edge, None),
            _ => return Ok(None),
        };
        if self.graph.locate(point)?.0 != block {
            return Ok(None);
        }
        let mut changed = false;
        for write in self.cleanup_writes.iter().filter(|write| {
            write.edge == edge
                && limit.is_none_or(|(ordinal, after)| {
                    write.action_ordinal < ordinal || after && write.action_ordinal == ordinal
                })
        }) {
            let written = Backing::direct(Lineage {
                root: write.backing.root,
                segments: write.backing.path.clone(),
            });
            if !overlap(watched, &written) {
                continue;
            }
            let ends = cleanup_write_ends(asked, watched, &written);
            if write.content == Content::Vacated {
                // A payload vacancy does not change its enclosing sum's tag.
                if !matches!(asked, Asked::Case(..)) || ends {
                    return Ok(Some(Reached::Unestablished));
                }
            }
            changed |= ends;
        }
        Ok(changed.then_some(match asked {
            Asked::Window { .. } => Reached::Closed,
            _ => Reached::Unknown,
        }))
    }

    pub(super) fn cleanup_log_is_stated(&self, tables: &MachineTables) -> Checked<()> {
        let mut stated =
            std::collections::BTreeMap::<(EdgeId, u32), Vec<(Lineage, Content)>>::new();
        for row in tables.writes.values() {
            if let ProgramPoint::AfterCleanup {
                edge,
                action_ordinal,
            } = row.at
            {
                self.graph.locate(row.at)?;
                stated.entry((edge, action_ordinal)).or_default().push((
                    lineage(self.machine, &tables.places, row.place)?,
                    row.content,
                ));
            }
        }
        let mut expected =
            std::collections::BTreeMap::<(EdgeId, u32), Vec<(Lineage, Content)>>::new();
        for write in &self.cleanup_writes {
            expected
                .entry((write.edge, write.action_ordinal))
                .or_default()
                .push((
                    Lineage {
                        root: write.captured.root,
                        segments: write.captured.path.clone(),
                    },
                    write.content,
                ));
        }
        if stated != expected {
            return Err(self.foreign(
                Construct::Machine,
                "cleanup write rows differ from their exact action effects",
            ));
        }
        Ok(())
    }

    /// A write on any selected arrival prevents old operation contents from
    /// answering a later read. The normal callee establishes valid storage,
    /// but a new scalar identity requires an actual read or postcondition.
    pub(super) fn cleanup_arrival(
        &self,
        asked: Asked<'_>,
        watched: &Backing,
        source: BlockId,
        target: BlockId,
    ) -> Checked<Option<Reached>> {
        let predecessor = self.block(source)?;
        if self.cleanup_writes.is_empty() {
            return Ok(None);
        }
        let mut edges = predecessor.terminator.successor_edges();
        edges.retain(|(_, into)| *into == target);
        edges.sort_unstable_by_key(|(edge, _)| *edge);
        let mut changed = false;
        let mut restored_edges = Vec::new();
        for write in self.cleanup_writes.iter().filter(|write| {
            edges
                .binary_search_by_key(&write.edge, |(edge, _)| *edge)
                .is_ok()
        }) {
            let written = Backing::direct(Lineage {
                root: write.backing.root,
                segments: write.backing.path.clone(),
            });
            if !overlap(watched, &written) {
                continue;
            }
            let ends = cleanup_write_ends(asked, watched, &written);
            if write.content == Content::Vacated {
                // A payload vacancy does not change its enclosing sum's tag.
                if !matches!(asked, Asked::Case(..)) || ends {
                    return Ok(Some(Reached::Unestablished));
                }
            }
            changed |= ends;
            if ends
                && matches!(asked, Asked::Window { .. })
                && !restored_edges.contains(&write.edge)
            {
                restored_edges.push(write.edge);
            }
        }
        if matches!(asked, Asked::Window { .. }) {
            // Every actual incoming edge must restore this exact captured owner.
            // One alternative's write cannot close another alternative's window.
            return Ok((!edges.is_empty()
                && edges.iter().all(|(edge, _)| restored_edges.contains(edge)))
            .then_some(Reached::Closed));
        }
        Ok(changed.then_some(Reached::Unknown))
    }
}

/// Possible writes retire old observations. Only definite coverage can
/// restore a window; equal lists of possible addresses do not prove that.
fn cleanup_write_ends(asked: Asked<'_>, watched: &Backing, written: &Backing) -> bool {
    match asked {
        Asked::Contents(_) => true,
        // A write proved strictly below the sum changes its payload, not its tag.
        Asked::Case(..) => {
            !watched.definitely_contains(written) || written.definitely_contains(watched)
        }
        Asked::Window { .. } => written.definitely_contains(watched),
    }
}

impl crate::Bindings for AtCleanupCall<'_, '_> {
    type Error = crate::Unbound;
    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Self::Error> {
        self.inputs.value(id, scalar_type)
    }
    fn field(
        &mut self,
        _version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        let actual = self.inputs.actual(root, path)?;
        if !self
            .inputs
            .storage
            .cleanup_untouched(
                self.inputs.references,
                &actual,
                self.edge,
                self.action_ordinal,
                self.inputs.concerns,
            )
            .map_err(crate::Unbound::Refused)?
        {
            return Err(crate::Unbound::Unproved(
                "cleanup requirement needs a fresh value after a preceding action's write",
            ));
        }
        let actual = self
            .inputs
            .storage
            .cleanup_backing(
                self.inputs.references,
                &actual,
                self.edge,
                self.action_ordinal,
                self.inputs.concerns,
            )
            .map_err(crate::Unbound::Refused)?;
        let mut held = self.inputs.held_at(&actual.place);
        held.point = Some(ProgramPoint::BeforeCleanup {
            edge: self.edge,
            action_ordinal: self.action_ordinal,
        });
        held.observation(&actual.place, scalar_type)
    }
    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        let actual = self.inputs.actual(root, path)?;
        if !self
            .inputs
            .storage
            .cleanup_untouched(
                self.inputs.references,
                &actual,
                self.edge,
                self.action_ordinal,
                self.inputs.concerns,
            )
            .map_err(crate::Unbound::Refused)?
        {
            return Err(crate::Unbound::Unproved(
                "cleanup extent changed in a preceding action",
            ));
        }
        let actual = self
            .inputs
            .storage
            .cleanup_backing(
                self.inputs.references,
                &actual,
                self.edge,
                self.action_ordinal,
                self.inputs.concerns,
            )
            .map_err(crate::Unbound::Refused)?;
        let mut held = self.inputs.held_at(&actual.place);
        held.point = Some(ProgramPoint::BeforeCleanup {
            edge: self.edge,
            action_ordinal: self.action_ordinal,
        });
        held.extent_of(&actual.place, scalar_type)
    }
    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        self.inputs.case(root, path, case)
    }
}
