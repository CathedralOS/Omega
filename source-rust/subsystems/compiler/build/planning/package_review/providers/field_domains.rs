//! A public data field's exact type already discloses its declared domains.
//! Reconcile those source fields with the checked membership roster before
//! using the data declaration as the review evidence for a boundary.
use arena::IdSet;
use semantic_vocabulary::StructuralTypeId;
use source_compiler::publication::AcceptedTerminalSource;
use source_compiler::representations::source_state::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::BTreeSet;
use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

pub(super) fn retained(
    accepted: &AcceptedTerminalSource,
    root: StructuralTypeId,
    published: &[StructuralPathQualification],
    reference: TypeReferenceHandle,
) -> bool {
    if !published.is_empty() {
        if !reference.is_valid() {
            return false;
        }
        let records = accepted.source_records();
        let Ok(index) = records.binary_search_by_key(&root, |row| row.record) else {
            return false;
        };
        let owner = &records[index];
        let source = accepted.source();
        let table = &source.type_reference_table;
        let mut reference = table.unconstrained_type_reference(reference);
        if let TypeReferenceNode::Reference { referee, .. } = table.type_reference(reference) {
            reference = table.unconstrained_type_reference(*referee);
        }
        if source
            .closed_data_definition(reference)
            .is_none_or(|data| data.symbol != owner.declaration)
            || owner.application.as_ref().is_some_and(|application| {
                application.type_identity(source) != source.normalized_type_identity(reference)
            })
        {
            return false;
        }
    }
    let origins = &accepted.closure().elaboration().domain_sources;
    let mut pending = BTreeSet::new();
    for row in published {
        let Some(origin) = accepted
            .module()
            .structural_domain(row.domain)
            .and_then(|decl| origins.get(decl.semantic_domain))
        else {
            return false;
        };
        if !pending.insert((row.path.clone(), origin.application.coordinates())) {
            return false;
        }
    }
    let mut fields = PublicFieldDomains {
        accepted,
        pending,
        path: Vec::new(),
        active: IdSet::new(),
        empty: IdSet::new(),
    };
    fields.check_record(root).is_some() && fields.pending.is_empty()
}

struct PublicFieldDomains<'a> {
    accepted: &'a AcceptedTerminalSource,
    pending: BTreeSet<(Vec<StructuralPathSegment>, (u32, u32, u32))>,
    path: Vec<StructuralPathSegment>,
    active: IdSet<StructuralTypeId>,
    empty: IdSet<StructuralTypeId>,
}

impl PublicFieldDomains<'_> {
    fn check_record(&mut self, record: StructuralTypeId) -> Option<()> {
        if self.empty.contains(record) {
            return Some(());
        }
        let program = self.accepted;
        let source = program.source();
        let records = program.source_records();
        let Ok(index) = records.binary_search_by_key(&record, |row| row.record) else {
            return Some(());
        };
        let owner = &records[index];
        let Some(data) = source.unique_data_definition_by_symbol(owner.declaration) else {
            return Some(());
        };
        if !data.is_public {
            return Some(());
        }
        if !self.active.insert(record) {
            return None;
        }
        let remaining = self.pending.len();
        let module = program.module();
        let declaration = module.structural_type(record)?;
        for (case, field) in declaration.shape.fields_with_case() {
            let Some(child) = field.field_type.as_structural() else {
                continue;
            };
            let coordinate = owner.fields.iter().find(|row| row.field == field.id)?;
            let authored = source
                .data_member_fields(source.data_members(data))
                .find(|row| row.symbol == coordinate.declaration)?;
            let depth = self.path.len();
            if let Some(case) = case {
                self.path.push(StructuralPathSegment::Case(case.id));
            }
            self.path.push(StructuralPathSegment::Field(field.identity));
            let mut reference = authored.type_reference;
            let table = &source.type_reference_table;
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = table.type_reference(reference)
            {
                for constraint in table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint
                        && domain.subject == DomainConstraintSubject::Declared
                    {
                        let coordinate = (
                            domain.symbol.arena_index(),
                            domain.symbol.generation(),
                            domain.semantic_id.0,
                        );
                        self.pending
                            .remove(&(self.path.clone(), coordinate))
                            .then_some(())?;
                    }
                }
                reference = *base_type;
            }
            self.check_record(child)?;
            self.path.truncate(depth);
        }
        self.active.remove(record);
        if self.pending.len() == remaining {
            self.empty.insert(record);
        }
        Some(())
    }
}
