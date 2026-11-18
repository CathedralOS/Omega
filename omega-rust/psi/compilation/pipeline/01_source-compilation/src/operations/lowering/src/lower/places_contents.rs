//! Logical storage observations are sparse: a contract on element 4095 names
//! that element, without creating identities for the other 4095 elements.
//! Allocate them after lowering so exact-place order is independent of when a
//! body or a callee's clause first demands the leaf.

use std::collections::HashSet;

use arena::NumericId;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerType, OperationId, PlaceId, Proposition, ScalarTerm,
    ScalarType, StructuralCaseId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, Segment};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, ExactPlaceId, ProgramPoint};
use terminal_semantics::substitution::clauses::{ClauseBindings, substitute_clause};

use super::Lowering;

struct Named {
    leaves: Vec<Observation>,
}

struct Observation {
    root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
    extent: bool,
    scalar_type: ScalarType,
}

#[derive(Default)]
struct ElementDemands {
    seen: HashSet<(StructuralTypeId, Vec<Segment>, ScalarType)>,
    ordered: Vec<(StructuralTypeId, PlaceId, Vec<Segment>, ScalarType)>,
}

impl ElementDemands {
    fn retain(
        &mut self,
        record: StructuralTypeId,
        root: PlaceId,
        segments: Vec<Segment>,
        scalar_type: ScalarType,
    ) {
        let key = (record, segments, scalar_type);
        if !self.seen.contains(&key) {
            self.ordered
                .push((record, root, key.1.clone(), scalar_type));
            self.seen.insert(key);
        }
    }

    fn into_ordered(self) -> Vec<(StructuralTypeId, PlaceId, Vec<Segment>, ScalarType)> {
        self.ordered
    }
}

fn projection(segment: CanonicalStructuralPathSegment) -> Segment {
    match segment {
        CanonicalStructuralPathSegment::Field(field) => Segment::Field(field),
        CanonicalStructuralPathSegment::Case(case) => Segment::Case(case),
        CanonicalStructuralPathSegment::FixedIndex(index) => Segment::FixedIndex(index),
        CanonicalStructuralPathSegment::Referent => Segment::Referent,
    }
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
        self.leaves.push(Observation {
            root,
            path: selected.to_vec(),
            extent: false,
            scalar_type,
        });
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
        self.leaves.push(Observation {
            root,
            path: selected.to_vec(),
            extent: true,
            scalar_type: ScalarType::Integer(scalar_type),
        });
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
    pub(in crate::lowering::lower) fn name_clause_contents(
        &mut self,
        clause: &Proposition,
        bindings: &[(PlaceId, ExactPlaceId)],
        result: Option<PlaceId>,
    ) -> Checked<()> {
        self.name_call_clause_contents(clause, bindings, result, &[])
    }

    pub(in crate::lowering::lower) fn name_call_clause_contents(
        &mut self,
        clause: &Proposition,
        bindings: &[(PlaceId, ExactPlaceId)],
        result: Option<PlaceId>,
        argument_types: &[Option<(StructuralTypeId, StructuralTypeId)>],
    ) -> Checked<()> {
        let clause = self.qualifications.content_proposition(self.id, clause)?;
        let mut named = Named { leaves: Vec::new() };
        let mut clauses = vec![&clause];
        while let Some(clause) = clauses.pop() {
            match clause {
                Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
                    clauses.extend(parts)
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    clauses.push(premise);
                    clauses.push(conclusion);
                }
                Proposition::ContentConservation(equation) => {
                    self.name_content_endpoints(
                        equation,
                        bindings,
                        result,
                        argument_types,
                        &mut named,
                    )?;
                }
                _ => {
                    let _ = substitute_clause(clause, &mut named);
                }
            }
        }
        for Observation {
            root,
            mut path,
            extent,
            scalar_type,
        } in named.leaves
        {
            // The declared output is not an invocation input. Its actual
            // returned storage is observed at the exit, not named as an
            // entry snapshot of an unavailable result.
            if Some(root) == result {
                continue;
            }
            let Some((position, &(_, mut leaf))) = bindings
                .iter()
                .enumerate()
                .find(|(_, (formal, _))| *formal == root)
            else {
                return Err(self.internal("a contract observation has no structural subject"));
            };
            if let Some(Some((formal_type, actual_type))) = argument_types.get(position)
                && formal_type != actual_type
            {
                path = crate::call_projection::translate(
                    self.structural_types,
                    *formal_type,
                    *actual_type,
                    &path,
                )
                .ok_or_else(|| {
                    self.internal(
                        "a call contract projection is absent from the actual storage type",
                    )
                })?;
            }
            for segment in path {
                leaf = self.project(leaf, projection(segment))?;
            }
            if extent {
                leaf = self.project(leaf, Segment::Extent)?;
            }
            self.initial_content(leaf, scalar_type)?;
        }
        Ok(())
    }

    fn name_content_endpoints(
        &self,
        equation: &semantic_vocabulary::ContentConservation,
        bindings: &[(PlaceId, ExactPlaceId)],
        result: Option<PlaceId>,
        argument_types: &[Option<(StructuralTypeId, StructuralTypeId)>],
        named: &mut Named,
    ) -> Checked<()> {
        let mut terms = vec![equation.left(), equation.right()];
        while let Some(term) = terms.pop() {
            let semantic_vocabulary::ContentTerm::Projection {
                projection,
                subject,
            } = term
            else {
                if let semantic_vocabulary::ContentTerm::Separate(parts) = term {
                    terms.extend(parts);
                }
                continue;
            };
            // Returned storage is named at its exit. Capturing it here would
            // invent an entry snapshot before the call has produced it.
            if Some(subject.root) == result {
                continue;
            }
            let (position, &(_, exact)) = bindings
                .iter()
                .enumerate()
                .find(|(_, (root, _))| *root == subject.root)
                .ok_or_else(|| self.internal("content observation has no exact input subject"))?;
            let root_type = match argument_types.get(position).copied().flatten() {
                Some((formal, _)) => formal,
                None => self.content_subject_type(exact)?,
            };
            let selected = terminal_semantics::content_projections::resolve_projection(
                self.structural_types,
                &self.qualifications.structural,
                self.qualifications.identities.as_table(),
                root_type,
                equation.algebra(),
                *projection,
                subject,
            )
            .map_err(|detail| self.internal(detail))?;
            let mut scalars = Vec::new();
            match &selected.definition.expression {
                semantic_vocabulary::ContentProjectionExpression::IntervalSet(intervals) => {
                    for (start, end) in intervals {
                        scalars.extend([start, end]);
                    }
                }
                semantic_vocabulary::ContentProjectionExpression::CountedQuantity(value) => {
                    scalars.push(value)
                }
            }
            for question in &selected.definition.formation {
                match question {
                    semantic_vocabulary::ContentProjectionFormation::Natural(value) => {
                        scalars.push(value)
                    }
                    semantic_vocabulary::ContentProjectionFormation::OrderedInterval {
                        start,
                        end,
                    } => scalars.extend([start, end]),
                }
            }
            for scalar in scalars {
                let term = terminal_semantics::content_projections::scalar(
                    self.structural_types.iter(),
                    self.qualifications.identities.as_table(),
                    selected.carrier,
                    subject.root,
                    &selected.prefix,
                    subject.version,
                    scalar,
                )
                .map_err(|detail| self.internal(detail))?;
                terminal_semantics::substitution::clauses::substitute_clause_math_term(
                    &term, named,
                )
                .map_err(|detail| self.internal(detail))?;
            }
        }
        Ok(())
    }

    fn content_subject_type(&self, mut exact: ExactPlaceId) -> Checked<StructuralTypeId> {
        let mut path = Vec::new();
        let root = loop {
            let row = self
                .tables
                .places
                .get(exact)
                .ok_or_else(|| self.internal("content subject has no exact storage row"))?;
            match row.origin {
                PlaceOrigin::Root(root) => break root,
                PlaceOrigin::Projection { parent, segment } => {
                    path.push(match segment {
                        Segment::Field(field) => CanonicalStructuralPathSegment::Field(field),
                        Segment::Case(case) => CanonicalStructuralPathSegment::Case(case),
                        Segment::FixedIndex(index) => {
                            CanonicalStructuralPathSegment::FixedIndex(index)
                        }
                        Segment::Referent => CanonicalStructuralPathSegment::Referent,
                        _ => {
                            return Err(self.internal(
                                "content contract subject has no static declared carrier",
                            ));
                        }
                    });
                    exact = parent;
                }
            }
        };
        path.reverse();
        let (_, root_type) = self
            .roots
            .get(root)
            .ok_or_else(|| self.internal("content subject lost its storage type"))?;
        terminal_semantics::canonical_structural_path_tip(
            self.structural_types.iter(),
            *root_type,
            &path,
        )
        .map(|carrier| carrier.id)
        .ok_or_else(|| self.internal("content subject path has no exact declared type"))
    }

    /// Name the declared predicates before sealing their field snapshots.
    /// Custody and restoration remain separate checks.
    pub(in crate::lowering::lower) fn name_declared_predicates(
        &mut self,
        machine: &terminal_psi::TerminalMachine,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Checked<()> {
        use terminal_semantics::structural_qualifications::DefaultRequirements;

        let inventory = {
            let requirements =
                DefaultRequirements::new(types, &self.qualifications.structural, machine)
                    .map_err(|error| self.internal(error.detail))?;
            let mut inventory = Vec::new();
            for required in requirements
                .declared_predicates()
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
                inventory.push((required.root, selected, clause));
            }
            inventory
        };
        let subject = terminal_check_tables::program::self_place()
            .ok_or_else(|| self.internal("default clause subject identity"))?;
        for (root, selected, clause) in inventory {
            let mut exact = self.exact_root(root)?;
            for segment in selected {
                exact = self.project(exact, projection(segment))?;
            }
            self.name_clause_contents(&clause, &[(subject, exact)], None)?;
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
        let mut elements = ElementDemands::default();
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
            // The first demand already names its own root; propagating it
            // below names every other formal of this type. Later roots with
            // the same path and scalar type only repeat that work. Retain
            // first-encounter order so exact-place identities stay unchanged,
            // and keep differing scalar types for initial_content to refuse.
            elements.retain(*record, root, segments, scalar_type);
        }
        for (record, root, segments, scalar_type) in elements.into_ordered() {
            let first = formals.partition_point(|(held, _)| *held < record);
            for &(held, other) in &formals[first..] {
                if held != record {
                    break;
                }
                if other == root {
                    continue;
                }
                let mut leaf = self.exact_root(other)?;
                for segment in &segments {
                    leaf = self.project(leaf, *segment)?;
                }
                self.initial_content(leaf, scalar_type)?;
            }
        }
        Ok(())
    }

    pub(in crate::lowering::lower) fn name_storage_contents(
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::IntegerSign;

    fn integer(bits: u16) -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
    }

    #[test]
    fn repeated_roots_keep_the_first_demand_and_path_order() {
        let record = StructuralTypeId::new(1).unwrap();
        let first_root = PlaceId::new(7).unwrap();
        let other_root = PlaceId::new(2).unwrap();
        let first_path = vec![Segment::FixedIndex(4)];
        let second_path = vec![Segment::FixedIndex(1)];
        let mut demands = ElementDemands::default();
        demands.retain(record, first_root, first_path.clone(), integer(32));
        demands.retain(record, other_root, second_path.clone(), integer(32));
        demands.retain(record, other_root, first_path.clone(), integer(32));
        demands.retain(record, first_root, second_path.clone(), integer(32));
        assert_eq!(
            demands.ordered,
            vec![
                (record, first_root, first_path, integer(32)),
                (record, other_root, second_path, integer(32)),
            ]
        );
    }

    #[test]
    fn repeated_element_requests_do_not_repeat_formal_propagation() {
        let record = StructuralTypeId::new(1).unwrap();
        let mut demands = ElementDemands::default();
        let mut requests = 0;
        for root in 1..=128 {
            for index in [3, 1, 5, 0] {
                requests += 1;
                demands.retain(
                    record,
                    PlaceId::new(root).unwrap(),
                    vec![Segment::Referent, Segment::FixedIndex(index)],
                    integer(32),
                );
            }
        }
        assert_eq!(requests, 512);
        assert_eq!(demands.ordered.len(), 4);
        assert!(
            demands
                .ordered
                .iter()
                .all(|(_, root, _, _)| *root == PlaceId::new(1).unwrap())
        );
        eprintln!(
            "element demands: {requests} requests, {} propagation paths",
            demands.ordered.len()
        );
    }

    #[test]
    fn distinct_carriers_and_conflicting_scalar_types_remain_separate() {
        let record = StructuralTypeId::new(1).unwrap();
        let other_record = StructuralTypeId::new(2).unwrap();
        let root = PlaceId::new(1).unwrap();
        let other_root = PlaceId::new(2).unwrap();
        let path = vec![Segment::FixedIndex(4)];
        let mut demands = ElementDemands::default();
        demands.retain(record, root, path.clone(), integer(32));
        demands.retain(record, other_root, path.clone(), integer(64));
        demands.retain(other_record, other_root, path.clone(), integer(32));
        assert_eq!(
            demands.ordered,
            vec![
                (record, root, path.clone(), integer(32)),
                (record, other_root, path.clone(), integer(64)),
                (other_record, other_root, path, integer(32)),
            ]
        );
    }
}
