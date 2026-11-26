//! The parts of an inline formal that this body exposes. A whole-value use
//! exposes every field. Passing one field to an opaque callee exposes the
//! whole selected field, while its siblings remain in the caller.

use std::collections::{HashMap, HashSet};

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, Proposition, ScalarTerm, StructuralFieldId,
};
use terminal_psi::{
    OperationProjection, StructuralFieldDeclaration, StructuralPathSegment, StructuralStorageUse,
    StructuralTypeDeclaration, TerminalIdentityIndex, TerminalMachine,
};

struct Formal<'a> {
    record: &'a [StructuralFieldDeclaration],
    exposed: HashSet<TerminalIdentityIndex>,
}

#[derive(Default)]
pub(super) struct Fields<'a>(HashMap<PlaceId, Formal<'a>>);

enum Field {
    Name(TerminalIdentityIndex),
    Id(StructuralFieldId),
}

impl Fields<'_> {
    pub(super) fn excludes(&self, root: PlaceId, path: &[StructuralPathSegment]) -> bool {
        let Some(StructuralPathSegment::Field(field)) = path.first() else {
            return false;
        };
        self.0
            .get(&root)
            .is_some_and(|formal| !formal.exposed.contains(field))
    }

    fn expose(&mut self, root: PlaceId, field: Option<Field>) {
        let Some(formal) = self.0.get_mut(&root) else {
            return;
        };
        let name = field.and_then(|field| match field {
            Field::Name(name) => formal
                .record
                .iter()
                .any(|field| field.identity == name)
                .then_some(name),
            Field::Id(id) => formal
                .record
                .iter()
                .find(|field| field.id == id)
                .map(|field| field.identity),
        });
        if let Some(name) = name {
            formal.exposed.insert(name);
        } else {
            self.0.remove(&root);
        }
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
) -> Fields<'a> {
    let mut fields = Fields::default();
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
        return fields;
    }
    let lookup = |id| {
        types
            .binary_search_by_key(&id, |ty| ty.id)
            .ok()
            .map(|i| &types[i])
    };
    for formal in &machine.structural_parameters {
        if !formal.qualifications.is_empty()
            || !formal.projected_qualifications.is_empty()
            || !formal.additional_input_qualifications.is_empty()
        {
            continue;
        }
        let Some(ty) = lookup(formal.structural_type) else {
            continue;
        };
        let Some(record) = ty.shape.record_fields() else {
            continue;
        };
        if terminal_psi::structural_type_has_borrowed_storage_by(lookup, ty.id) {
            continue;
        }
        if let Some(required) = predicate_fields(ty) {
            fields.0.insert(
                formal.place,
                Formal {
                    record,
                    exposed: HashSet::new(),
                },
            );
            for id in required {
                fields.expose(formal.place, Some(Field::Id(id)));
            }
        }
    }
    if fields.0.is_empty() {
        return fields;
    }
    for operation in machine.operations() {
        if let Some(result) = operation.result.structural() {
            fields.0.remove(&result.place);
        }
        for (operand, _) in operation.kind.storage_uses() {
            match operand {
                StructuralStorageUse::Projection(access) => fields.projection(access),
                StructuralStorageUse::Place(root) => {
                    fields.0.remove(&root);
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
    for block in &machine.blocks {
        for access in block.terminator.storage_uses() {
            fields.projection(access);
        }
    }
    fields
}

/// Scalar field bounds expose their declared paths even when only the checker
/// reads them. Keep the complete inventory for other kinds of type predicate.
fn predicate_fields(ty: &StructuralTypeDeclaration) -> Option<HashSet<StructuralFieldId>> {
    let mut fields = HashSet::new();
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
                if let Some(CanonicalStructuralPathSegment::Field(field)) = path.first() {
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
