//! The parts of an inline formal that this body exposes. A whole-value use
//! exposes every field. Passing one field to an opaque callee exposes the
//! whole selected field, while its siblings remain in the caller.

use arena::{IdSet, IdTable};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, Proposition, ScalarTerm, StructuralFieldId,
    StructuralTypeId,
};
use terminal_psi::{
    OperationProjection, StructuralArgument, StructuralFieldDeclaration,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralStorageUse,
    StructuralTypeDeclaration, TerminalIdentityIndex, TerminalMachine,
};

struct Formal<'a> {
    carrier: StructuralTypeId,
    record: &'a [StructuralFieldDeclaration],
    exposed: IdSet<StructuralFieldId>,
}

#[derive(Default)]
pub(super) struct Fields<'a>(IdTable<PlaceId, Option<Formal<'a>>>);

enum Field {
    Name(TerminalIdentityIndex),
    Id(StructuralFieldId),
}

impl Formal<'_> {
    fn field_id(&self, field: Field) -> Option<StructuralFieldId> {
        self.record
            .iter()
            .find(|candidate| match field {
                Field::Name(name) => candidate.identity == name,
                Field::Id(id) => candidate.id == id,
            })
            .map(|candidate| candidate.id)
    }
}

impl Fields<'_> {
    pub(super) fn excludes(&self, root: PlaceId, path: &[StructuralPathSegment]) -> bool {
        let Some(StructuralPathSegment::Field(field)) = path.first() else {
            return false;
        };
        self.0
            .get(root)
            .and_then(Option::as_ref)
            .is_some_and(|formal| {
                formal
                    .field_id(Field::Name(*field))
                    .is_some_and(|id| !formal.exposed.contains(id))
            })
    }

    fn expose(&mut self, root: PlaceId, field: Option<Field>) {
        let Some(slot) = self.0.get_mut(root) else {
            return;
        };
        let Some(formal) = slot.as_mut() else { return };
        if let Some(id) = field.and_then(|field| formal.field_id(field)) {
            formal.exposed.insert(id);
        } else {
            *slot = None;
        }
    }

    fn carried_record(
        &self,
        argument: &StructuralArgument,
        formal: &StructuralParameterDeclaration,
    ) -> bool {
        argument.path.is_empty()
            && matches!(
                argument.access,
                terminal_psi::reading_borrow_pair_pattern!()
            )
            && argument.access == formal.access
            && [argument.place, formal.place].into_iter().all(|root| {
                self.0
                    .get(root)
                    .and_then(Option::as_ref)
                    .is_some_and(|source| source.carrier == formal.structural_type)
            })
    }

    /// A transferred borrow may reach any use of its block parameter, including
    /// after a backedge. Union the observed fields across each connected set.
    /// This shares the demand for names; every place still has its own snapshots.
    fn include_transferred_fields(&mut self, transfers: &[(PlaceId, PlaceId)]) {
        if transfers.is_empty() {
            return;
        }
        let mut adjacent = self.0.map(|_, _| Vec::new());
        for &(source, destination) in transfers {
            adjacent
                .get_mut(source)
                .expect("tracked source")
                .push(destination);
            adjacent
                .get_mut(destination)
                .expect("tracked destination")
                .push(source);
        }
        let mut seen = IdSet::new();
        let mut group = Vec::new();
        for &(start, _) in transfers {
            if !seen.insert(start) {
                continue;
            }
            group.clear();
            group.push(start);
            let mut next = 0;
            while let Some(&root) = group.get(next) {
                next += 1;
                for &other in adjacent.get(root).expect("tracked formal") {
                    if seen.insert(other) {
                        group.push(other);
                    }
                }
            }
            let exposed = group.iter().try_fold(IdSet::new(), |mut combined, &root| {
                let formal = self.0.get(root)?.as_ref()?;
                combined.union_with(&formal.exposed);
                Some(combined)
            });
            for &root in &group {
                let slot = self.0.get_mut(root).expect("tracked formal");
                if let (Some(formal), Some(exposed)) = (slot.as_mut(), &exposed) {
                    formal.exposed.clone_from(exposed);
                } else {
                    *slot = None;
                }
            }
        }
    }

    /// A direct callee restores the domains of a whole borrowed formal.
    /// The caller checks its own later field reads and each projected loan's
    /// containing owners. A private inferred contract can name input fields
    /// beyond the published clauses, so it still observes the whole argument.
    fn returns_whole_borrows(
        &self,
        operation: &terminal_psi::OperationKind,
        signatures: crate::body::Signatures<'_>,
    ) -> bool {
        use crate::body::{BorrowedReferent, Qualification, ValueType};

        let Some(call) = operation.module_invocation() else {
            return false;
        };
        if call.structural_arguments.is_empty()
            || call.required_contract.is_some()
            || !call.erased_arguments.is_empty()
            || !call.erased_proof_arguments.is_empty()
            || !call.claim_transfers.is_empty()
            || !call.completion_receipts.is_empty()
            || !call.crash_continuations.is_empty()
        {
            return false;
        }
        let Some(private) = signatures.inferred_inputs else {
            return false;
        };
        let Some(callee) = signatures.machines.get(call.callee) else {
            return false;
        };
        if private.binary_search(&call.callee).is_ok()
            || callee.has_structural_guarantees()
            || callee.structural_result_place().is_some()
            || callee.observing_clauses().any(|p| p != &Proposition::Truth)
        {
            return false;
        }
        let mut actuals = call.structural_arguments.iter();
        for (position, &ty) in callee.parameters.iter().enumerate() {
            if !callee.is_place(position) {
                continue;
            }
            let (Some(actual), Some(borrowed), ValueType::Record(carrier, Qualification::NONE, _)) =
                (actuals.next(), callee.borrowed(position), ty)
            else {
                return false;
            };
            if !actual.path.is_empty()
                || borrowed.referent != BorrowedReferent::Value
                || crate::lower::places::borrowed_access(borrowed.access) != Some(actual.access)
                || self
                    .0
                    .get(actual.place)
                    .and_then(Option::as_ref)
                    .is_none_or(|formal| formal.carrier != carrier)
            {
                return false;
            }
        }
        actuals.next().is_none()
    }

    fn projection(&mut self, access: OperationProjection<'_>) {
        let field = match access.path.first() {
            Some(StructuralPathSegment::Field(name)) => Some(Field::Name(*name)),
            None => access.field.map(Field::Id),
            _ => None,
        };
        self.expose(access.root, field);
    }

    fn canonical(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        final_field: Option<StructuralFieldId>,
    ) {
        let id = match path.first() {
            Some(CanonicalStructuralPathSegment::Field(id)) => Some(*id),
            None => final_field,
            _ => None,
        };
        self.expose(root, id.map(Field::Id));
    }
}

pub(super) fn observed<'a>(
    machine: &TerminalMachine,
    types: &'a [StructuralTypeDeclaration],
    unconditional: bool,
    signatures: crate::body::Signatures<'_>,
) -> Fields<'a> {
    // Claims, ranked transitions, and content proofs can mention storage
    // outside the body's executable operands.
    if machine.ranked_scc.is_some()
        || !machine.block_entry_claims.is_empty()
        || !machine.loan_completions.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
        || !machine.contract.content_entry_claims.is_empty()
        || !unconditional
        || !machine.contract.requires.is_empty()
        || !machine.contract.ensures.is_empty()
        || !machine.contract.source_checked_requires.is_empty()
        || !machine.contract.membership_ensures.is_empty()
        || !machine.contract.outcome_specific_ensures.is_empty()
    {
        return Fields::default();
    }
    let lookup = |id| {
        types
            .binary_search_by_key(&id, |ty| ty.id)
            .ok()
            .map(|i| &types[i])
    };
    let mut formals: Vec<_> = machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
        .filter_map(|formal| {
            if !formal.qualifications.is_empty()
                || !formal.projected_qualifications.is_empty()
                || !formal.additional_input_qualifications.is_empty()
            {
                return None;
            }
            let ty = lookup(formal.structural_type)?;
            let record = ty.shape.record_fields()?;
            if terminal_psi::structural_type_has_borrowed_storage_by(lookup, ty.id) {
                return None;
            }
            let exposed = predicate_fields(ty, record)?;
            Some((
                formal.place,
                Some(Formal {
                    carrier: ty.id,
                    record,
                    exposed,
                }),
            ))
        })
        .collect();
    formals.sort_unstable_by_key(|(place, _)| *place);
    let mut fields = Fields(
        IdTable::from_rows_with_gaps(formals, machine.structural_places.len()).unwrap_or_default(),
    );
    if fields.0.is_empty() {
        return fields;
    }
    for operation in machine.operations() {
        if let Some(result) = operation.result.structural() {
            fields.expose(result.place, None);
        }
        let handback = fields.returns_whole_borrows(&operation.kind, signatures);
        for (operand, _) in operation.kind.storage_uses_with_arguments(|_| !handback) {
            match operand {
                StructuralStorageUse::Projection(access) => fields.projection(access),
                StructuralStorageUse::Place(root) => {
                    fields.expose(root, None);
                }
                StructuralStorageUse::Canonical { root, path } => {
                    fields.canonical(root, path, None)
                }
                StructuralStorageUse::ObservedField { root, path, field } => {
                    fields.canonical(root, path, Some(field))
                }
                // Indirect backing or type-directed access may name a different
                // root from the descriptor written in the body.
                StructuralStorageUse::Element { .. } | StructuralStorageUse::Type(_) => {
                    return Fields::default();
                }
            }
        }
    }
    let mut transfers = Vec::new();
    for block in &machine.blocks {
        let edges = block.terminator.successor_edges();
        let external_arguments = edges.is_empty();
        for access in block
            .terminator
            .storage_uses_with_arguments(|_| external_arguments)
        {
            fields.projection(access);
        }
        for ((_, target), arguments) in edges
            .into_iter()
            .zip(block.terminator.successor_block_arguments())
        {
            let Some(destination) = machine
                .blocks
                .get((target.get() - 1) as usize)
                .filter(|block| block.id == target)
            else {
                return Fields::default();
            };
            if arguments.len() != destination.structural_parameters.len() {
                return Fields::default();
            }
            for (source, formal) in arguments.iter().zip(&destination.structural_parameters) {
                if fields.carried_record(source, formal) {
                    transfers.push((source.place, formal.place));
                } else {
                    fields.projection(OperationProjection {
                        root: source.place,
                        path: &source.path,
                        field: None,
                    });
                    fields.expose(formal.place, None);
                }
            }
        }
    }
    fields.include_transferred_fields(&transfers);
    fields
}

/// Scalar field bounds expose their declared paths even when only the checker
/// reads them. Keep the complete inventory for other kinds of type predicate.
fn predicate_fields(
    ty: &StructuralTypeDeclaration,
    record: &[StructuralFieldDeclaration],
) -> Option<IdSet<StructuralFieldId>> {
    let mut fields = IdSet::new();
    for clause in &ty.requires {
        if !matches!(
            clause,
            Proposition::Truth
                | Proposition::Falsehood
                | Proposition::Equal(..)
                | Proposition::LessThan(..)
                | Proposition::LessOrEqual(..)
        ) {
            return None;
        }
        let mut closed = true;
        clause.visit_scalar_terms(|term| match term {
            ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => {}
            ScalarTerm::BooleanField { path, .. }
            | ScalarTerm::IntegerField { path, .. }
            | ScalarTerm::IeeeFloatField { path, .. } => {
                if let Some(CanonicalStructuralPathSegment::Field(field)) = path.first()
                    && record.iter().any(|candidate| candidate.id == *field)
                {
                    fields.insert(*field);
                } else {
                    closed = false;
                }
            }
            _ => closed = false,
        });
        if !closed {
            return None;
        }
    }
    Some(fields)
}
