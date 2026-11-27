//! Logical storage observations are sparse: a contract on element 4095 names
//! that element, without creating identities for the other 4095 elements.
//! Allocate them after lowering so exact-place order is independent of when a
//! body or a callee's clause first demands the leaf.

use std::collections::{BTreeSet, HashMap, HashSet};

use arena::NumericId;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, OperationId, PlaceId, Proposition, ScalarTerm, ScalarType,
    StructuralPlaceKind, StructuralTypeId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, Segment};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, ExactPlaceId, ProgramPoint};

use super::Lowering;

#[path = "places_contents/formal_fields.rs"]
mod formal_fields;

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

impl Observation {
    /// Collect every scalar leaf demanded by the declaration. This inventory
    /// needs neither substitutions nor evidence about the current owner.
    fn in_clause(clause: &Proposition) -> Vec<Self> {
        let mut leaves = Vec::new();
        clause.visit_scalar_terms(|term| {
            let (root, path) = match term {
                ScalarTerm::BooleanField { root, path, .. }
                | ScalarTerm::IntegerField { root, path, .. }
                | ScalarTerm::IeeeFloatField { root, path, .. }
                | ScalarTerm::ViewExtent { root, path, .. } => (*root, path),
                _ => return,
            };
            leaves.push(Self {
                root,
                path: path.clone(),
                extent: matches!(term, ScalarTerm::ViewExtent { .. }),
                scalar_type: term.scalar_type(),
            });
        });
        leaves
    }
}

impl Lowering<'_> {
    pub(in crate::lower) fn name_clause_contents(
        &mut self,
        clause: &Proposition,
        bindings: &[(PlaceId, ExactPlaceId)],
        result: Option<PlaceId>,
    ) -> Checked<()> {
        self.name_call_clause_contents(clause, bindings, result, &[])
    }

    pub(in crate::lower) fn name_call_clause_contents(
        &mut self,
        clause: &Proposition,
        bindings: &[(PlaceId, ExactPlaceId)],
        result: Option<PlaceId>,
        argument_types: &[Option<(StructuralTypeId, StructuralTypeId)>],
    ) -> Checked<()> {
        self.name_observations(
            &Observation::in_clause(clause),
            bindings,
            result,
            argument_types,
        )?;
        self.name_tested_cases(clause, bindings, result)
    }

    /// Name the case of each whole structural formal a clause tests, as an
    /// input's case is named where the body reads it. A test beneath `||`
    /// or `ensures` then reads that entry name, even when the body never
    /// touches the formal. A test of a field's case is left to the readers
    /// that name it, and refused by name where none does.
    fn name_tested_cases(
        &mut self,
        clause: &Proposition,
        bindings: &[(PlaceId, ExactPlaceId)],
        result: Option<PlaceId>,
    ) -> Checked<()> {
        let mut pending = vec![clause];
        while let Some(clause) = pending.pop() {
            match clause {
                Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
                    pending.extend(parts.iter().rev())
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    pending.push(conclusion);
                    pending.push(premise);
                }
                Proposition::IsMere(inner) | Proposition::Universal { body: inner, .. } => {
                    pending.push(inner)
                }
                Proposition::StructuralCaseMembership { subject, .. }
                    if subject.path().is_empty() && Some(subject.root()) != result =>
                {
                    let Some(&(_, leaf)) = bindings
                        .iter()
                        .find(|(formal, _)| *formal == subject.root())
                    else {
                        continue;
                    };
                    let (root, segments) = self.lineage(leaf)?;
                    if !segments.is_empty() {
                        continue;
                    }
                    if let Some(&(_, sum)) = self.roots.get(root) {
                        self.initial_case(leaf, sum)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn name_observations(
        &mut self,
        observations: &[Observation],
        bindings: &[(PlaceId, ExactPlaceId)],
        result: Option<PlaceId>,
        argument_types: &[Option<(StructuralTypeId, StructuralTypeId)>],
    ) -> Checked<()> {
        for observation in observations {
            let root = observation.root;
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
            let translated;
            let path = if let Some(Some((formal_type, actual_type))) = argument_types.get(position)
                && formal_type != actual_type
            {
                translated = terminal_semantics::call_projection::translate(
                    self.structural_types,
                    *formal_type,
                    *actual_type,
                    &observation.path,
                )
                .ok_or_else(|| {
                    self.internal(
                        "a call contract projection is absent from the actual storage type",
                    )
                })?;
                &translated
            } else {
                &observation.path
            };
            for &segment in path {
                leaf = self.project(leaf, projection(segment))?;
            }
            if observation.extent {
                leaf = self.project(leaf, Segment::Extent)?;
            }
            self.initial_content(leaf, observation.scalar_type)?;
        }
        Ok(())
    }

    /// A scalar-returning, call-free reader only observes the elements selected
    /// by its body and by the structural predicates on those values. Preserve
    /// the full inventory when a runtime selector, structural handoff, or
    /// formation can concern other elements.
    fn read_only_default_indices(
        &self,
        machine: &terminal_psi::TerminalMachine,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<BTreeSet<u64>> {
        if machine.result.structural().is_some()
            || !machine.contract.requires.is_empty()
            || !machine.contract.ensures.is_empty()
            || !self.invariants.is_empty()
            || machine.operations().any(|op| {
                terminal_semantics::structural_qualifications::operation_writes_default_storage(
                    &op.kind,
                ) || op.kind.declared_invocation().is_some()
                    || matches!(
                        op.kind,
                        terminal_psi::OperationKind::EstablishZeroedStructural {}
                    )
            })
        {
            return None;
        }
        let mut indices = BTreeSet::new();
        for place in self.tables.places.values() {
            let PlaceOrigin::Projection { segment, .. } = place.origin else {
                continue;
            };
            match segment {
                Segment::FixedIndex(index) => {
                    indices.insert(index);
                }
                Segment::Field(_) | Segment::Case(_) | Segment::Extent | Segment::Referent => {}
                _ => return None,
            }
        }
        for clause in types.iter().flat_map(|ty| &ty.requires).chain(
            self.qualifications
                .structural
                .iter()
                .filter_map(|d| d.predicate.as_ref().map(|p| &p.proposition)),
        ) {
            for observation in Observation::in_clause(clause) {
                for segment in observation.path {
                    if let CanonicalStructuralPathSegment::FixedIndex(index) = segment {
                        indices.insert(index);
                    }
                }
            }
        }
        Some(indices)
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

        let (inventory, clauses) = {
            let requirements =
                DefaultRequirements::new(types, &self.qualifications.structural, machine)
                    .map_err(|error| self.internal(error.detail))?;
            let observations = std::cell::OnceCell::new();
            let declared = requirements
                .declared(|root, path, length| {
                    let (indices, fields) = observations.get_or_init(|| {
                        // A state entry can carry an explicit true invariant,
                        // which has no storage operands.
                        let unconditional = self
                            .invariants
                            .iter()
                            .all(|invariant| invariant.predicate == Proposition::Truth);
                        (
                            self.read_only_default_indices(machine, types),
                            formal_fields::observed(machine, types, unconditional, self.signatures),
                        )
                    });
                    if fields.excludes(root, path) {
                        return Some(Vec::new());
                    }
                    indices
                        .as_ref()
                        .map(|set| set.range(..length).copied().collect())
                })
                .map_err(|error| self.internal(error.detail))?;
            let mut inventory = Vec::with_capacity(declared.len());
            let mut clauses = Vec::new();
            let mut by_clause = HashMap::new();
            for required in declared {
                // Equal predicates over the same carrier read the same fields.
                // Each occurrence still names its own actual storage below.
                let clause = requirements
                    .clause(&required)
                    .map_err(|error| self.internal(error.detail))?;
                let index = *by_clause
                    .entry((required.carrier, clause))
                    .or_insert_with_key(|(_, clause)| {
                        let index = clauses.len();
                        clauses.push(Observation::in_clause(clause));
                        index
                    });
                inventory.push((required, index));
            }
            (inventory, clauses)
        };
        let subject = terminal_check_tables::program::self_place()
            .ok_or_else(|| self.internal("default clause subject identity"))?;
        for group in inventory.chunk_by(|(left, _), (right, _)| {
            left.root == right.root && left.path == right.path && left.carrier == right.carrier
        }) {
            let required = &group[0].0;
            let (_, root_type) = self
                .roots
                .get(required.root)
                .ok_or_else(|| self.internal("a default clause has no actual storage root"))?;
            let (selected, carrier) = terminal_semantics::canonical_static_projection(
                types.iter(),
                *root_type,
                &required.path,
            )
            .ok_or_else(|| self.internal("a default clause has no exact canonical storage path"))?;
            if carrier.id != required.carrier {
                return Err(self.internal("a default clause changed its actual carrier"));
            }
            let mut exact = self.exact_root(required.root)?;
            for segment in selected {
                exact = self.project(exact, projection(segment))?;
            }
            for &(_, index) in group {
                self.name_observations(&clauses[index], &[(subject, exact)], None, &[])?;
            }
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

    pub(in crate::lower) fn name_storage_contents(
        &mut self,
        machine: &terminal_psi::TerminalMachine,
    ) -> Checked<()> {
        // Each predicate is already in its owner's scope. A call-site clause
        // still uses the caller's substitutions and is never imported here.
        let mut indexed = Vec::new();
        for clause in machine
            .contract
            .requires
            .iter()
            .chain(self.invariants.iter().map(|row| &row.predicate))
        {
            clause.visit_scalar_terms(|term| {
                let ScalarTerm::StructuralRead {
                    root,
                    path,
                    scalar_type,
                } = term
                else {
                    return;
                };
                use semantic_vocabulary::ScalarReadProjection as Read;
                let segments: Option<Vec<_>> = path
                    .iter()
                    .map(|step| {
                        Some(match step {
                            Read::Referent => Segment::Referent,
                            Read::Extent => Segment::Extent,
                            Read::Static(step) => projection(*step),
                            Read::Index(term) => match term.as_ref() {
                                ScalarTerm::Value { id, .. } => {
                                    Segment::RuntimeIndex { selector: *id }
                                }
                                _ => return None,
                            },
                        })
                    })
                    .collect();
                if let Some(segments) = segments {
                    indexed.push((*root, segments, *scalar_type));
                }
            });
        }
        for (root, segments, scalar_type) in indexed {
            let mut leaf = self.exact_root(root)?;
            for step in segments {
                leaf = self.project(leaf, step)?;
            }
            self.initial_content(leaf, scalar_type)?;
        }
        self.name_elements_beneath_formals()?;
        let mut call_operations: Vec<_> = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.kind.declared_invocation().is_some())
            .map(|operation| operation.id)
            .collect();
        call_operations.sort_unstable();
        let mut overwrites: Vec<_> = self
            .writes
            .iter()
            .filter_map(|write| {
                let (Content::Overwritten, ProgramPoint::AfterOperation(call)) =
                    (write.content, write.at)
                else {
                    return None;
                };
                call_operations
                    .binary_search(&call)
                    .is_ok()
                    .then_some((write.place, call))
            })
            .collect();
        overwrites.sort_unstable();
        for slot in 0..self.demanded_contents.len() {
            let Some(scalar_type) = self.demanded_contents[slot] else {
                continue;
            };
            let leaf = ExactPlaceId::new(slot as u64 + 1)
                .ok_or_else(|| self.internal("a storage observation identity overflow"))?;
            let (named_entry, calls) = self.snapshot_boundaries(machine, leaf, &overwrites)?;
            if named_entry {
                self.value(
                    scalar_type,
                    ValueOrigin::StorageContent {
                        place: leaf,
                        after: None,
                    },
                );
            }
            for call in calls {
                self.value(
                    scalar_type,
                    ValueOrigin::StorageContent {
                        place: leaf,
                        after: Some(call),
                    },
                );
            }
        }
        // A sum's case is named at the same boundaries as a leaf's contents,
        // one Boolean per declared case, in place, boundary and case order.
        for slot in 0..self.demanded_cases.len() {
            let Some(sum_type) = self.demanded_cases[slot] else {
                continue;
            };
            let sum = ExactPlaceId::new(slot as u64 + 1)
                .ok_or_else(|| self.internal("a storage observation identity overflow"))?;
            let (named_entry, calls) = self.snapshot_boundaries(machine, sum, &overwrites)?;
            let mut cases: Vec<_> = self
                .record_type(sum_type)?
                .cases()
                .iter()
                .map(|case| case.id)
                .collect();
            cases.sort_unstable();
            // A block formal is bound anew on each arrival, and the checker
            // names no case of it yet: only an invocation formal's entry.
            let (root, _) = self.lineage(sum)?;
            let invocation_formal = matches!(
                self.roots.get(root),
                Some((StructuralPlaceKind::Parameter { .. }, _))
            );
            let entry = (named_entry && invocation_formal).then_some(None);
            for after in entry.into_iter().chain(calls.into_iter().map(Some)) {
                for &case in &cases {
                    self.value(
                        ScalarType::Boolean,
                        ValueOrigin::StorageCase {
                            place: sum,
                            case,
                            after,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    /// Where the storage at `place` has a snapshot a logical name can stand
    /// for: at entry, when its root is a readable formal, and at the normal
    /// return of each call that alone overwrites it or a place above it, in
    /// call order.
    fn snapshot_boundaries(
        &self,
        machine: &terminal_psi::TerminalMachine,
        place: ExactPlaceId,
        overwrites: &[(ExactPlaceId, OperationId)],
    ) -> Checked<(bool, Vec<OperationId>)> {
        let mut at = place;
        let mut calls = Vec::<OperationId>::new();
        let mut segments = Vec::new();
        let root = loop {
            let row = self
                .tables
                .places
                .get(at)
                .ok_or_else(|| self.internal("a demanded storage leaf has no place row"))?;
            let start = overwrites.partition_point(|(place, _)| *place < at);
            calls.extend(
                overwrites[start..]
                    .iter()
                    .take_while(|(place, _)| *place == at)
                    .map(|(_, call)| *call),
            );
            match row.origin {
                PlaceOrigin::Root(root) => break root,
                PlaceOrigin::Projection { parent, segment } => {
                    segments.push(segment);
                    at = parent;
                }
            }
        };
        // Inference discovers a helper's condition after lowering. Every
        // demanded, readable field therefore needs its entry name, even
        // when no authored header clause named it. A selector must already
        // be available when the formal receives its incoming storage.
        let owner = self.roots.get(root).map(|(kind, _)| kind);
        let named_entry = match owner.filter(|kind| {
            segments.iter().all(|&step| {
                terminal_check_tables::bindings::entry_snapshot_segment(
                    step,
                    **kind,
                    &self.tables.values,
                )
            })
        }) {
            Some(StructuralPlaceKind::Parameter { .. }) => true,
            Some(StructuralPlaceKind::BlockParameter { block, position }) => {
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
        calls.sort_unstable();
        // Overlapping exclusive arguments have no unique snapshot owner. Keep
        // them unnamed so the loan checker reports the source conflict; a
        // forged name is independently refused.
        let unique = calls
            .chunk_by(|left, right| left == right)
            .filter_map(|owners| match owners {
                [call] => Some(*call),
                _ => None,
            })
            .collect();
        Ok((named_entry, unique))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType};

    fn integer(bits: u16) -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
    }

    #[test]
    fn a_meaning_equation_names_both_of_its_structural_sources() {
        use numerics::float_projection::FloatProjectionOperation;
        use semantic_vocabulary::{ClauseIdentity, FloatMeaningClauseValue, IeeeFloatFormat};

        let roots = [PlaceId::new(2).unwrap(), PlaceId::new(9).unwrap()];
        let format = IeeeFloatFormat::Binary32;
        let values = roots
            .into_iter()
            .map(|root| FloatMeaningClauseValue::Projection {
                source: ScalarTerm::ieee_float_field_path(root, Vec::new(), format),
                contract: FloatProjectionOperation::Meaning32.contract_identity(),
            })
            .collect();
        let clause = Proposition::DeclarationIdentity(ClauseIdentity::float_meaning(values, 0, 1));
        let found: Vec<_> = Observation::in_clause(&clause)
            .into_iter()
            .map(|field| (field.root, field.extent, field.scalar_type))
            .collect();
        assert_eq!(
            found,
            roots.map(|root| (root, false, ScalarType::IeeeFloat(format)))
        );
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
