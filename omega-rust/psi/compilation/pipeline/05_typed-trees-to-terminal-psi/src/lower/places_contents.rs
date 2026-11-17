//! Logical storage observations are sparse: a contract on element 4095 names
//! that element, without creating identities for the other 4095 elements.
//! Allocate them after lowering so exact-place order is independent of when a
//! body or a callee's clause first demands the leaf.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerType, OperationId, PlaceId, Proposition, PsiSemanticId,
    ScalarTerm, ScalarType, StructuralCaseId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, Segment};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, ExactPlaceId, ProgramPoint};
use terminal_semantics::substitution::clauses::{ClauseBindings, substitute_clause};

use super::Lowering;

struct Named {
    leaves: Vec<(PlaceId, Vec<Segment>, ScalarType)>,
}

fn path(path: &[CanonicalStructuralPathSegment]) -> Result<Vec<Segment>, &'static str> {
    path.iter()
        .map(|segment| match segment {
            CanonicalStructuralPathSegment::Field(field) => Ok(Segment::Field(*field)),
            CanonicalStructuralPathSegment::Case(case) => Ok(Segment::Case(*case)),
            CanonicalStructuralPathSegment::FixedIndex(index) => Ok(Segment::FixedIndex(*index)),
            CanonicalStructuralPathSegment::Referent => Ok(Segment::Referent),
        })
        .collect()
}

impl ClauseBindings for Named {
    type Error = &'static str;

    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Self::Error> {
        Ok(ScalarTerm::value(id, scalar_type))
    }

    fn field(
        &mut self,
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        selected: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        self.leaves.push((root, path(selected)?, scalar_type));
        match scalar_type {
            ScalarType::Integer(scalar_type) => Ok(ScalarTerm::IntegerField {
                version,
                root,
                path: selected.to_vec(),
                scalar_type,
            }),
            ScalarType::Boolean => Ok(ScalarTerm::BooleanField {
                version,
                root,
                path: selected.to_vec(),
            }),
            ScalarType::IeeeFloat(_) => Err("a floating-point clause observation"),
            ScalarType::Callable(_) => Err("a machine-value clause observation"),
        }
    }

    fn extent(
        &mut self,
        root: PlaceId,
        selected: &[CanonicalStructuralPathSegment],
        scalar_type: IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        let mut steps = path(selected)?;
        steps.push(Segment::Extent);
        self.leaves
            .push((root, steps, ScalarType::Integer(scalar_type)));
        Ok(ScalarTerm::ViewExtent {
            root,
            path: selected.to_vec(),
            scalar_type,
        })
    }

    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        selected: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, Self::Error> {
        Ok(ScalarTerm::byte_sequence_predicate(
            predicate,
            root,
            selected.to_vec(),
        ))
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        // Leave case selection undecided so substitution visits every arm.
        // This inventory neither decides clauses nor supplies proof evidence.
        Err("a case test is recorded, not decided")
    }
}

impl Lowering<'_> {
    pub(in crate::lower) fn name_clause_contents(
        &mut self,
        clause: &Proposition,
        bindings: &[(PlaceId, ExactPlaceId)],
        result: Option<PlaceId>,
    ) -> Checked<()> {
        let mut named = Named { leaves: Vec::new() };
        let _ = substitute_clause(clause, &mut named);
        for (root, path, scalar_type) in named.leaves {
            // The declared output is not an invocation input. Its actual
            // returned storage is observed at the exit, not named as an
            // entry snapshot of an unavailable result.
            if Some(root) == result {
                continue;
            }
            let Some((_, mut leaf)) = bindings.iter().find(|(formal, _)| *formal == root).copied()
            else {
                return Err(self.internal("a contract observation has no structural subject"));
            };
            for segment in path {
                leaf = self.project(leaf, segment)?;
            }
            self.initial_content(leaf, scalar_type)?;
        }
        Ok(())
    }

    /// Named qualifications and type defaults name the same current storage
    /// that restoration checks inspect. Inventory those clauses before sealing
    /// snapshot identities; preparing a question supplies no proof of it.
    pub(in crate::lower) fn name_default_contents(
        &mut self,
        machine: &terminal_psi::TerminalMachine,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Checked<()> {
        use terminal_semantics::structural_qualifications::DefaultRequirements;

        let (inventory, questions) = {
            let requirements =
                DefaultRequirements::new(types, &self.qualifications.structural, machine)
                    .map_err(|error| self.internal(error.detail))?;
            let mut inventory = Vec::new();
            for required in requirements
                .declared()
                .map_err(|error| self.internal(error.detail))?
            {
                let (_, root_type) = self
                    .roots
                    .get(required.root)
                    .ok_or_else(|| self.internal("a default clause has no actual storage root"))?;
                let (selected, carrier) = terminal_semantics::canonical_static_projection(
                    types.iter(),
                    *root_type,
                    &required.path,
                )
                .ok_or_else(|| {
                    self.internal("a default clause has no exact canonical storage path")
                })?;
                if carrier.id != required.carrier {
                    return Err(self.internal("a default clause changed its actual carrier"));
                }
                let clause = requirements
                    .clause(&required)
                    .map_err(|error| self.internal(error.detail))?;
                inventory.push((
                    required.root,
                    path(&selected).map_err(|detail| self.internal(detail))?,
                    clause,
                ));
            }
            let mut questions = Vec::new();
            for block in &machine.blocks {
                let mut before = ProgramPoint::BlockEntry(block.id);
                for operation in &block.operations {
                    let count = requirements
                        .operation(operation)
                        .map_err(|error| self.internal(error.detail))?
                        .len();
                    let source = *self.sources.operations.get(operation.id).ok_or_else(|| {
                        self.internal("a default question has no operation source")
                    })?;
                    for clause in 0..count {
                        questions.push((operation.id, self.position(clause)?, before, source));
                    }
                    before = ProgramPoint::AfterOperation(operation.id);
                }
            }
            (inventory, questions)
        };
        let subject = terminal_check_tables::program::self_place()
            .ok_or_else(|| self.internal("default clause subject identity"))?;
        for (root, selected, clause) in inventory {
            let mut exact = self.exact_root(root)?;
            for segment in selected {
                exact = self.project(exact, segment)?;
            }
            self.name_clause_contents(&clause, &[(subject, exact)], None)?;
        }
        for (operation, clause, before, source) in questions {
            self.obligation(
                terminal_check_tables::obligations::ObligationKind::DataDomain {
                    operation,
                    clause,
                },
                before,
                source,
            );
        }
        Ok(())
    }

    /// A record formal names every scalar field at entry (`name_contents`),
    /// but array elements only where something reads them. Inference states
    /// a helper's condition after lowering, and a cycle copies that helper's
    /// head into each member: `sc_verdict` reads `main.wires[4]`, and the
    /// copied head of `sc_stop` must name what its own formal holds there.
    /// Demand each element path read beneath one structural formal beneath
    /// every other formal of the same type.
    fn name_elements_beneath_formals(&mut self) -> Checked<()> {
        let mut formals: Vec<(StructuralTypeId, PlaceId)> = self
            .roots
            .iter()
            .filter(|(_, (kind, _))| {
                matches!(
                    kind,
                    StructuralPlaceKind::Parameter { .. }
                        | StructuralPlaceKind::BlockParameter { .. }
                )
            })
            .map(|(root, (_, record))| (*record, root))
            .collect();
        formals.sort_unstable();
        let mut elements = Vec::new();
        for slot in 0..self.demanded_contents.len() {
            let Some(scalar_type) = self.demanded_contents[slot] else {
                continue;
            };
            let leaf = ExactPlaceId::new(slot as u64 + 1)
                .ok_or_else(|| self.internal("a storage observation identity overflow"))?;
            let (root, segments) = self.lineage(leaf)?;
            if !segments
                .iter()
                .any(|segment| matches!(segment, Segment::FixedIndex(_)))
                || !segments.iter().all(|segment| {
                    matches!(
                        segment,
                        Segment::Field(_) | Segment::FixedIndex(_) | Segment::Referent
                    )
                })
            {
                continue;
            }
            let Some((_, record)) = self.roots.get(root) else {
                continue;
            };
            elements.push((*record, root, segments, scalar_type));
        }
        for (record, root, segments, scalar_type) in elements {
            let first = formals.partition_point(|(held, _)| *held < record);
            let others: Vec<PlaceId> = formals[first..]
                .iter()
                .take_while(|(held, _)| *held == record)
                .map(|(_, other)| *other)
                .filter(|other| *other != root)
                .collect();
            for other in others {
                let mut leaf = self.exact_root(other)?;
                for segment in &segments {
                    leaf = self.project(leaf, *segment)?;
                }
                self.initial_content(leaf, scalar_type)?;
            }
        }
        Ok(())
    }

    pub(in crate::lower) fn name_storage_contents(
        &mut self,
        machine: &terminal_psi::TerminalMachine,
    ) -> Checked<()> {
        self.name_elements_beneath_formals()?;
        let mut call_operations: Vec<_> = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.kind.declared_invocation().is_some())
            .map(|operation| operation.id)
            .collect();
        call_operations.sort_unstable();
        for slot in 0..self.demanded_contents.len() {
            let Some(scalar_type) = self.demanded_contents[slot] else {
                continue;
            };
            let leaf = ExactPlaceId::new(slot as u64 + 1)
                .ok_or_else(|| self.internal("a storage observation identity overflow"))?;
            let mut at = leaf;
            let mut calls = Vec::<OperationId>::new();
            let mut block_observation = true;
            let root = loop {
                let row = self
                    .tables
                    .places
                    .get(at)
                    .ok_or_else(|| self.internal("a demanded storage leaf has no place row"))?;
                for write in &row.writes {
                    let write = self
                        .tables
                        .writes
                        .get(*write)
                        .ok_or_else(|| self.internal("a storage write has no row"))?;
                    if let (Content::Overwritten, ProgramPoint::AfterOperation(call)) =
                        (write.content, write.at)
                        && call_operations.binary_search(&call).is_ok()
                    {
                        calls.push(call);
                    }
                }
                match row.origin {
                    PlaceOrigin::Root(root) => break root,
                    PlaceOrigin::Projection { parent, segment } => {
                        block_observation &= matches!(
                            segment,
                            Segment::Field(_)
                                | Segment::Case(_)
                                | Segment::FixedIndex(_)
                                | Segment::Referent
                                | Segment::Extent
                        );
                        at = parent;
                    }
                }
            };
            // Inference discovers a helper's condition after lowering. Every
            // demanded, readable field therefore needs its entry name, even
            // when no authored header clause named it. Runtime selectors do
            // not have a binding at that entry and cannot name a snapshot.
            let named_entry = match self.roots.get(root).map(|(kind, _)| kind) {
                Some(StructuralPlaceKind::Parameter { .. }) => true,
                Some(StructuralPlaceKind::BlockParameter { block, position })
                    if block_observation =>
                {
                    let formal = machine
                        .blocks
                        .get(block.get() as usize - 1)
                        .filter(|body| body.id == *block)
                        .and_then(|body| body.structural_parameters.get(*position as usize))
                        .filter(|formal| formal.place == root)
                        .ok_or_else(|| self.internal("a block content has no structural formal"))?;
                    terminal_psi::structural_access_covers(
                        formal.access,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    )
                }
                _ => false,
            };
            if named_entry {
                self.value(
                    scalar_type,
                    ValueOrigin::StorageContent {
                        place: leaf,
                        after: None,
                    },
                );
            }
            calls.sort_unstable();
            for owners in calls.chunk_by(|left, right| left == right) {
                // Overlapping exclusive arguments have no unique snapshot
                // owner. Keep them unnamed so the loan checker reports the
                // source conflict; a forged name is independently refused.
                let [call] = owners else { continue };
                self.value(
                    scalar_type,
                    ValueOrigin::StorageContent {
                        place: leaf,
                        after: Some(*call),
                    },
                );
            }
        }
        Ok(())
    }
}
