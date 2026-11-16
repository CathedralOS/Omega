//! Logical storage observations are sparse: a contract on element 4095 names
//! that element, without creating identities for the other 4095 elements.
//! Allocate them after lowering so exact-place order is independent of when a
//! body or a callee's clause first demands the leaf.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerType, OperationId, PlaceId, Proposition, PsiSemanticId,
    ScalarTerm, ScalarType, StructuralCaseId, StructuralPlaceKind, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, Segment};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, ExactPlaceId, ProgramPoint};
use terminal_semantics::substitution::clauses::{ClauseBindings, substitute_clause};

use super::Lowering;

struct Named {
    leaves: Vec<(PlaceId, Vec<Segment>, ScalarType)>,
}

#[derive(Clone, Copy)]
pub(in crate::lower) struct ContentDemand {
    scalar_type: ScalarType,
    /// A clause observes this leaf. On a block formal, the clause needs the
    /// value the formal holds at each entry, whether the clause is the
    /// block's own predicate or a callee requirement stated within it: the
    /// checker traces an arrival's actual back to that snapshot.
    clause: bool,
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
    pub(super) fn demand_content(
        &mut self,
        leaf: ExactPlaceId,
        scalar_type: ScalarType,
        clause: bool,
    ) -> Checked<()> {
        let index = usize::try_from(leaf.get() - 1)
            .map_err(|_| self.internal("place identity exceeds the address space"))?;
        if self.demanded_contents.len() <= index {
            self.demanded_contents.resize(index + 1, None);
        }
        let demand = self.demanded_contents[index].get_or_insert(ContentDemand {
            scalar_type,
            clause,
        });
        if demand.scalar_type != scalar_type {
            return Err(self.internal("a demanded storage leaf has two scalar types"));
        }
        demand.clause |= clause;
        Ok(())
    }

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
            self.demand_content(leaf, scalar_type, true)?;
        }
        Ok(())
    }

    pub(in crate::lower) fn name_storage_contents(&mut self) -> Checked<()> {
        let mut call_operations: Vec<_> = self
            .blocks
            .iter()
            .flat_map(|(_, block)| &block.operations)
            .filter(|operation| operation.kind.declared_invocation().is_some())
            .map(|operation| operation.id)
            .collect();
        call_operations.sort_unstable();
        for slot in 0..self.demanded_contents.len() {
            let Some(ContentDemand {
                scalar_type,
                clause,
            }) = self.demanded_contents[slot]
            else {
                continue;
            };
            let leaf = ExactPlaceId::new(slot as u64 + 1)
                .ok_or_else(|| self.internal("a storage observation identity overflow"))?;
            let mut at = leaf;
            let mut calls = Vec::<OperationId>::new();
            // A block-entry snapshot names one fixed leaf: no runtime index
            // or range selects it.
            let mut fixed = true;
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
                        fixed &= matches!(
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
            if match self.roots.get(root).map(|(kind, _)| *kind) {
                Some(StructuralPlaceKind::Parameter { .. }) => true,
                // A block formal binds a new snapshot on each arrival, which
                // only a readable formal can supply.
                Some(StructuralPlaceKind::BlockParameter { block, .. }) => {
                    clause
                        && fixed
                        && self.blocks.get(block).is_some_and(|draft| {
                            terminal_psi::StructuralParameterDeclaration::find_by_place(
                                &draft.structural_parameters,
                                root,
                            )
                            .is_some_and(|formal| {
                                terminal_psi::structural_access_covers(
                                    formal.access,
                                    terminal_psi::StructuralAccess::SharedBorrow,
                                )
                            })
                        })
                }
                _ => false,
            } {
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
