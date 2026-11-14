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

fn path(path: &[CanonicalStructuralPathSegment]) -> Result<Vec<Segment>, &'static str> {
    path.iter()
        .map(|segment| match segment {
            CanonicalStructuralPathSegment::Field(field) => Ok(Segment::Field(*field)),
            CanonicalStructuralPathSegment::Case(case) => Ok(Segment::Case(*case)),
            CanonicalStructuralPathSegment::FixedIndex(index) => Ok(Segment::FixedIndex(*index)),
            CanonicalStructuralPathSegment::Referent => Err("a contract through a referent"),
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
        root: PlaceId,
        selected: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        self.leaves.push((root, path(selected)?, scalar_type));
        match scalar_type {
            ScalarType::Integer(scalar_type) => Ok(ScalarTerm::IntegerField {
                root,
                path: selected.to_vec(),
                scalar_type,
            }),
            ScalarType::Boolean => Ok(ScalarTerm::BooleanField {
                root,
                path: selected.to_vec(),
            }),
            ScalarType::IeeeFloat(_) => Err("a floating-point clause observation"),
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
    pub(in crate::lower) fn name_clause_contents(
        &mut self,
        clause: &Proposition,
        bindings: &[(PlaceId, ExactPlaceId)],
    ) -> Checked<()> {
        let mut named = Named { leaves: Vec::new() };
        let _ = substitute_clause(clause, &mut named);
        for (root, path, scalar_type) in named.leaves {
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
            let Some(scalar_type) = self.demanded_contents[slot] else {
                continue;
            };
            let leaf = ExactPlaceId::new(slot as u64 + 1)
                .ok_or_else(|| self.internal("a storage observation identity overflow"))?;
            let mut at = leaf;
            let mut calls = Vec::<OperationId>::new();
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
                    {
                        if call_operations.binary_search(&call).is_ok() {
                            calls.push(call);
                        }
                    }
                }
                match row.origin {
                    PlaceOrigin::Root(root) => break root,
                    PlaceOrigin::Projection { parent, .. } => at = parent,
                }
            };
            if matches!(
                self.roots.get(root).map(|(kind, _)| kind),
                Some(StructuralPlaceKind::Parameter { .. })
            ) {
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
