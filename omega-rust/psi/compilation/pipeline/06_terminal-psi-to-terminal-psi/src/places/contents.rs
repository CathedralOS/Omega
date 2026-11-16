//! Authenticate logical storage snapshots at their owner's entry or an exact
//! call's normal return. Names alone establish no value or type guarantee.

use terminal_check_tables::values::ValueOrigin;

use super::*;

impl Storage<'_> {
    /// The callee establishes only its result and the exact subtrees borrowed
    /// exclusively. A caller's enclosing record is outside that guarantee.
    pub(super) fn call_content_boundary(
        &self,
        call: OperationId,
        leaf: &Lineage,
        concerns: Construct,
    ) -> Checked<Lineage> {
        let operation = self.operation(call, concerns)?;
        let callee = operation.kind.declared_invocation().ok_or_else(|| {
            self.foreign(concerns, "a storage snapshot is not after a declared call")
        })?;
        if let Some(result) = operation.result.as_structural()
            && result.place == leaf.root
            && matches!(self.root_kind(leaf.root, concerns)?, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == call && structural_type == result.structural_type)
        {
            return Ok(Lineage {
                root: leaf.root,
                segments: Vec::new(),
            });
        }
        let formals = self
            .module
            .callable_structural_parameters(callee.callee)
            .ok_or_else(|| {
                self.foreign(
                    concerns,
                    "a storage snapshot's callee has no structural telescope",
                )
            })?;
        let arguments = operation.kind.structural_arguments();
        if formals.len() != arguments.len() {
            return Err(self.foreign(
                concerns,
                "a storage snapshot's call has another structural arity",
            ));
        }
        let mut found = None;
        for (formal, actual) in formals.iter().zip(arguments) {
            if !matches!(
                actual.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) {
                continue;
            }
            let captured = self.argument(actual, concerns)?;
            if captured.root != leaf.root || !leaf.segments.starts_with(&captured.segments) {
                continue;
            }
            if actual.access != formal.access {
                return Err(self.foreign(
                    concerns,
                    "a returned storage snapshot has another formal access",
                ));
            }
            let matches_type = match self.shape(&captured, concerns)? {
                Shape::Typed(actual_type) => {
                    terminal_semantics::borrowed_referent_presentation(
                        self.types.iter(),
                        actual_type,
                        formal.structural_type,
                    ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                        || self.byte_array_presentation(actual, formal, concerns)?
                }
                Shape::Scalar(actual) => {
                    matches!(self.declaration(formal.structural_type, concerns)?.shape,
                    StructuralTypeShape::PrimitiveScalar(declared) if declared == actual)
                }
                Shape::Atomic(actual) => {
                    self.declaration(formal.structural_type, concerns)?.shape
                        == StructuralTypeShape::AtomicCell(actual)
                }
                Shape::View | Shape::Payload(_) => false,
            };
            if !matches_type {
                return Err(self.foreign(
                    concerns,
                    "a returned storage snapshot has another formal type",
                ));
            }
            if found.replace(captured).is_some() {
                return Err(self.foreign(
                    concerns,
                    "a returned storage snapshot has overlapping call owners",
                ));
            }
        }
        found.ok_or_else(|| {
            self.foreign(
                concerns,
                "a returned storage snapshot is outside the call's established storage",
            )
        })
    }

    pub(super) fn initial_contents_are_declared(&mut self, tables: &MachineTables) -> Checked<()> {
        for (value, row) in tables.values.iter() {
            let ValueOrigin::StorageContent { place, after } = row.origin else {
                continue;
            };
            let concerns = Construct::Value(value);
            let leaf = lineage(self.machine, &tables.places, place)?;
            let (supported, block_entry) = match after {
                None => match self.root_kind(leaf.root, concerns)? {
                    StructuralPlaceKind::Parameter { .. } => (true, None),
                    StructuralPlaceKind::BlockParameter { block, .. } => (
                        terminal_psi::StructuralParameterDeclaration::find_by_place(
                            &self.block(block)?.structural_parameters,
                            leaf.root,
                        )
                        .is_some_and(|formal| {
                            terminal_psi::structural_access_covers(
                                formal.access,
                                terminal_psi::StructuralAccess::SharedBorrow,
                            )
                        }) && leaf.segments.iter().all(|segment| {
                            matches!(
                                segment,
                                Segment::Field(_)
                                    | Segment::Case(_)
                                    | Segment::FixedIndex(_)
                                    | Segment::Referent
                                    | Segment::Extent
                            )
                        }),
                        Some(block),
                    ),
                    _ => (false, None),
                },
                Some(call) => {
                    self.call_content_boundary(call, &leaf, concerns)?;
                    (true, None)
                }
            };
            if !supported
                || !matches!(self.shape(&leaf, concerns)?, Shape::Scalar(declared) if declared == row.scalar_type)
            {
                return Err(self.foreign(concerns,
                    "an initial content value of storage that is no scalar leaf of a formal or declared call result"));
            }
            if let Some(block) = block_entry {
                // A borrowed formal has the same backing as its incoming
                // loan. Use that backing for reads and overlapping writes.
                let backed = self.backing(&leaf, block, 0, concerns)?;
                self.observations[index(block.get())]
                    .entry(backed)
                    .or_default()
                    .push(Observation {
                        point: ProgramPoint::BlockEntry(block),
                        position: 0,
                        value,
                    });
            }
        }
        Ok(())
    }

    /// Identify snapshots that are rebound if a trace crosses their block's
    /// entry. Invocation contents remain fixed across the whole invocation.
    pub(super) fn observation_block(
        &self,
        tables: &MachineTables,
        value: ValueId,
        concerns: Construct,
    ) -> Checked<Option<BlockId>> {
        match tables.values.get(value).map(|row| row.origin) {
            Some(ValueOrigin::Operation(operation))
                if self
                    .read_leaf(self.operation(operation, concerns)?)?
                    .is_some() =>
            {
                let (owner, _) = self.graph.locate(ProgramPoint::AfterOperation(operation))?;
                Ok(Some(owner))
            }
            Some(ValueOrigin::StorageContent { place, after: None }) => {
                let leaf = lineage(self.machine, &tables.places, place)?;
                match self.root_kind(leaf.root, concerns)? {
                    StructuralPlaceKind::BlockParameter { block, .. } => Ok(Some(block)),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    /// Type validity is a call guarantee even without an authored `ensures`.
    /// Substitute only types inside each authenticated returned subtree, and
    /// use its post-call versions, never the caller's earlier observations.
    pub(crate) fn returned_type_premises(
        &self,
        tables: &MachineTables,
        operation: &Operation,
    ) -> Checked<Vec<Proposition>> {
        let Some(values) = self.returned_contents.get(index(operation.id.get())) else {
            return Ok(Vec::new());
        };
        let concerns = Construct::Operation(operation.id);
        let mut owners = Vec::new();
        for (place, _) in values {
            let leaf = lineage(self.machine, &tables.places, *place)?;
            let captured = self.call_content_boundary(operation.id, &leaf, concerns)?;
            for length in captured.segments.len()..leaf.segments.len() {
                owners.push(Lineage {
                    root: leaf.root,
                    segments: leaf.segments[..length].to_vec(),
                });
            }
        }
        owners
            .sort_by(|left, right| (left.root, &left.segments).cmp(&(right.root, &right.segments)));
        owners.dedup();
        let (block, up_to) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let mut premises = Vec::new();
        for owner in owners {
            let Shape::Typed(data) = self.shape(&owner, concerns)? else {
                continue;
            };
            for clause in self.clauses_of(data, concerns)? {
                let mut held = Held {
                    storage: self,
                    tables,
                    place: &owner,
                    block,
                    up_to,
                    concerns,
                };
                match crate::substitute(clause, &mut held) {
                    Ok(Proposition::Truth | Proposition::Falsehood) => {}
                    Ok(premise) => premises.push(premise),
                    Err(crate::Unbound::Unproved(_)) => {}
                    Err(unbound) => return Err(unbound.refusal(self.machine, concerns)),
                }
            }
        }
        Ok(premises)
    }
}

impl Storage<'_> {
    /// What a machine and each block that binds record formals knows at
    /// its entry about the values those formals hold there, returned with
    /// the block it holds at. A formal arrives as a valid value of its
    /// declared type: a machine formal from a call, a loop head's formal (a
    /// nested machine's) from an edge, and each consumes the value under
    /// its domain (dependent_values.md: "no consumption sees the value
    /// outside its required domain"; "Invariant windows": "Transfer ...
    /// supplying a value under that domain"). A write-only formal arrives
    /// holding nothing its caller established, so it learns nothing.
    ///
    /// Two passes state the clauses of the data types a formal's leaves lie
    /// in.
    ///
    /// - Each entry value of a machine formal's leaf gets the clauses a
    ///   read of it would (`Storage::premises`), stated at the machine's
    ///   entry. The body then knows them wherever that value goes: copied
    ///   into the result or passed to a call, not only where a scalar read
    ///   of the formal's field observes it.
    /// - A clause that relates several fields, such as
    ///   `self.i <= 5 && self.j <= 4`, cannot be stated on one leaf's
    ///   value. It is stated over the entry values of every leaf it names,
    ///   for each data type on the path from a formal down to a leaf with
    ///   an entry value, at the machine's entry and at each block whose
    ///   formal takes a snapshot on arrival. That is how it reaches a
    ///   write of one of the fields: the write's goal names the written
    ///   value and, for each unwritten field, the value that field holds
    ///   there. When that is still the arrival value the premise speaks
    ///   for it; when the field was written since, the goal names another
    ///   value and the premise says nothing about it. Nothing assumes a
    ///   field is unchanged. A type reached through a case payload is left
    ///   out of this pass, and a case test leaves its clause unstated:
    ///   which case a formal holds at entry is not established by
    ///   structure.
    ///
    /// The passes can state one clause twice; the copy is dropped.
    pub(crate) fn entry_type_premises(
        &self,
        tables: &MachineTables,
    ) -> Checked<Vec<(BlockId, Proposition)>> {
        let entry = self.graph.entry();
        let mut premises = Vec::new();
        for (value, row) in tables.values.iter() {
            let ValueOrigin::StorageContent { place, after: None } = row.origin else {
                continue;
            };
            let leaf = lineage(self.machine, &tables.places, place)?;
            let readable = terminal_psi::StructuralParameterDeclaration::find_by_place(
                &self.lowered.structural_parameters,
                leaf.root,
            )
            .is_some_and(|formal| {
                terminal_psi::structural_access_covers(
                    formal.access,
                    terminal_psi::StructuralAccess::SharedBorrow,
                )
            });
            if !readable {
                continue;
            }
            for premise in self.premises(
                tables,
                &leaf,
                (value, row.scalar_type),
                (entry, 0),
                Construct::Value(value),
            )? {
                premises.push((entry, premise));
            }
        }
        let concerns = Construct::Machine;
        let mut owners = Vec::new();
        for (slot, held) in self.initial_contents.iter().enumerate() {
            if held.is_none() {
                continue;
            }
            let place = ExactPlaceId::new(slot as u64 + 1)
                .ok_or_else(|| self.foreign(concerns, "a storage content identity overflow"))?;
            let leaf = lineage(self.machine, &tables.places, place)?;
            let (block, formals) = match self.root_kind(leaf.root, concerns)? {
                StructuralPlaceKind::Parameter { .. } => (
                    self.lowered.entry,
                    self.lowered.structural_parameters.as_slice(),
                ),
                StructuralPlaceKind::BlockParameter { block, .. } => {
                    (block, self.block(block)?.structural_parameters.as_slice())
                }
                _ => continue,
            };
            if !terminal_psi::StructuralParameterDeclaration::find_by_place(formals, leaf.root)
                .is_some_and(|formal| {
                    terminal_psi::structural_access_covers(
                        formal.access,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    )
                })
            {
                continue;
            }
            for length in 0..leaf.segments.len() {
                if leaf.segments[..length]
                    .iter()
                    .any(|segment| matches!(segment, Segment::Case(_)))
                {
                    break;
                }
                owners.push((
                    block,
                    Lineage {
                        root: leaf.root,
                        segments: leaf.segments[..length].to_vec(),
                    },
                ));
            }
        }
        owners.sort_by(|(_, left), (_, right)| {
            (left.root, &left.segments).cmp(&(right.root, &right.segments))
        });
        owners.dedup();
        for (block, owner) in owners {
            let Shape::Typed(data) = self.shape(&owner, concerns)? else {
                continue;
            };
            for clause in self.clauses_of(data, concerns)? {
                let mut entry = EntryOwner {
                    storage: self,
                    tables,
                    place: &owner,
                };
                match crate::substitute(clause, &mut entry) {
                    Ok(Proposition::Truth | Proposition::Falsehood) => {}
                    Ok(premise) => premises.push((block, premise)),
                    // A name with no entry value, or a form this binding
                    // does not serve, leaves the clause unstated: no
                    // premise, and nothing assumed in its place.
                    Err(crate::Unbound::Unproved(_) | crate::Unbound::Unsupported(_)) => {}
                    Err(crate::Unbound::Refused(refused)) => return Err(refused),
                }
            }
        }
        premises.sort();
        premises.dedup();
        Ok(premises)
    }
}

/// The names of a data type's `requires` clause for the value the formal
/// place `place` held where its owner was entered: each field its entry
/// content value. A case test is undecided.
struct EntryOwner<'s, 'a> {
    storage: &'s Storage<'a>,
    tables: &'s MachineTables,
    place: &'s Lineage,
}

impl EntryOwner<'_, '_> {
    fn beneath(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Lineage, crate::Unbound> {
        if Some(root) != terminal_check_tables::program::self_place() {
            return Err(crate::Unbound::Unsupported(
                "a data clause names storage other than its own value",
            ));
        }
        extended(self.place.clone(), path)
    }

    fn observation(
        &self,
        leaf: &Lineage,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let storage = self.storage;
        let concerns = Construct::Machine;
        if !matches!(
            storage
                .shape(leaf, concerns)
                .map_err(crate::Unbound::Refused)?,
            Shape::Scalar(declared) if declared == scalar_type
        ) {
            return Err(crate::Unbound::Unsupported(
                "a clause names storage with another scalar type",
            ));
        }
        let held = storage
            .interned(self.tables, leaf, concerns)
            .map_err(crate::Unbound::Refused)?
            .and_then(|place| storage.initial_contents.get(index(place.get())).copied())
            .flatten()
            .ok_or(crate::Unbound::Unproved(
                "no operation names the field, so its entry content has no value identity",
            ))?;
        let held_type =
            terminal_check_tables::row(&self.tables.values, held, storage.machine, concerns)
                .map_err(crate::Unbound::Refused)?
                .scalar_type;
        if held_type != scalar_type {
            return Err(crate::Unbound::Unsupported(
                "a formal's field holds a value of another type than its clause term",
            ));
        }
        Ok(ScalarTerm::value(held, scalar_type))
    }
}

impl crate::Bindings for EntryOwner<'_, '_> {
    type Error = crate::Unbound;
    fn value(&mut self, _: ValueId, _: ScalarType) -> Result<ScalarTerm, crate::Unbound> {
        Err(crate::Unbound::Unsupported(
            "a data clause names a value that is no field of its own value",
        ))
    }

    fn field(
        &mut self,
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if version != semantic_vocabulary::ContentPlaceVersion::Current {
            return Err(crate::Unbound::Unsupported(
                "a data predicate has no invocation-entry contents",
            ));
        }
        self.observation(&self.beneath(root, path)?, scalar_type)
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let mut leaf = self.beneath(root, path)?;
        leaf.segments.push(Segment::Extent);
        self.observation(&leaf, ScalarType::Integer(scalar_type))
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, crate::Unbound> {
        Err(crate::Unbound::Unproved(
            "which case a formal holds at entry is not established by structure",
        ))
    }
}
