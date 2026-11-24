//! Reconcile a structural boundary value with its reviewed data declaration.
//! That declaration carries ownership and the domains written on its fields.
use arena::IdSet;
use semantic_vocabulary::{PackageKeyIdentity, StructuralTypeId};
use source_compiler::publication::AcceptedTerminalSource;
use source_compiler::representations::source_state::data::DataDefinition;
use source_compiler::representations::source_state::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::BTreeSet;
use terminal_psi::{
    BoundaryStructuralResultDeclaration, StructuralMultiplicity, StructuralPathQualification,
    StructuralPathSegment, StructuralTypeShape,
};

fn declared_data(
    accepted: &AcceptedTerminalSource,
    root: StructuralTypeId,
    reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    let records = accepted.source_records();
    let owner = &records[records.binary_search_by_key(&root, |row| row.record).ok()?];
    let source = accepted.source();
    let data = source.closed_data_definition(reference)?;
    if data.symbol != owner.declaration
        || owner.application.as_ref().is_some_and(|application| {
            if application.type_identity(source) == source.normalized_type_identity(reference) {
                return false;
            }
            // A lifetime-only application shares the declaration's storage across
            // its regions. Both source occurrences must name that exact carrier.
            if data.lifetime_parameters.is_empty() || data.generic_instance.is_some() {
                return true;
            }
            let retained = application
                .scope
                .declared_carrier_identity(source, application.reference);
            let declared =
                source_compiler::representations::source_state::SourceTypeScope::default()
                    .declared_carrier_identity(source, reference);
            retained.is_none() || retained != declared
        })
    {
        return None;
    }
    Some(data)
}

pub(super) fn result_multiplicity_retained(
    accepted: &AcceptedTerminalSource,
    result: &BoundaryStructuralResultDeclaration,
    reference: TypeReferenceHandle,
) -> bool {
    if result.multiplicity == StructuralMultiplicity::Unrestricted {
        return true;
    }
    let table = &accepted.source().type_reference_table;
    if !table.contains_type_reference(reference) {
        return false;
    }
    // A borrow's referent does not declare the borrow's ownership.
    let reference = table.unconstrained_type_reference(reference);
    let Some(data) = declared_data(accepted, result.structural_type, reference) else {
        return false;
    };
    data.is_public
        && data.supply_mode == language_core::DataSupplyMode::CheckedShape
        && accepted
            .module()
            .structural_type(result.structural_type)
            .is_some_and(|declaration| declaration.multiplicity == result.multiplicity)
        && matches!(
            (data.properties.multiplicity, result.multiplicity),
            (
                language_core::Multiplicity::Affine,
                StructuralMultiplicity::Affine
            ) | (
                language_core::Multiplicity::Linear,
                StructuralMultiplicity::Linear
            )
        )
}

pub(super) fn field_domains_retained(
    accepted: &AcceptedTerminalSource,
    root: StructuralTypeId,
    published: &[StructuralPathQualification],
    reference: TypeReferenceHandle,
    private_owner: Option<PackageKeyIdentity>,
) -> bool {
    if !published.is_empty() {
        if !reference.is_valid() {
            return false;
        }
        let table = &accepted.source().type_reference_table;
        let mut reference = table.unconstrained_type_reference(reference);
        if let TypeReferenceNode::Reference { referee, .. } = table.type_reference(reference) {
            reference = table.unconstrained_type_reference(*referee);
        }
        if declared_data(accepted, root, reference).is_none() {
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
    let mut fields = FieldDomains {
        accepted,
        private_owner,
        pending,
        path: Vec::new(),
        active: IdSet::new(),
        empty: IdSet::new(),
    };
    fields.check_record(root).is_some() && fields.pending.is_empty()
}

struct FieldDomains<'a> {
    accepted: &'a AcceptedTerminalSource,
    private_owner: Option<PackageKeyIdentity>,
    pending: BTreeSet<(Vec<StructuralPathSegment>, (u32, u32, u32))>,
    path: Vec<StructuralPathSegment>,
    active: IdSet<StructuralTypeId>,
    empty: IdSet<StructuralTypeId>,
}

impl FieldDomains<'_> {
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
        if !data.is_public && !self.can_read_private(data.symbol) {
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
            self.check_field(child, authored.type_reference)?;
            self.path.truncate(depth);
        }
        self.active.remove(record);
        if self.pending.len() == remaining {
            self.empty.insert(record);
        }
        Some(())
    }

    fn can_read_private(&self, symbol: symbols::SymbolHandle) -> bool {
        self.private_owner.is_some()
            && self
                .accepted
                .source()
                .symbols
                .symbol_package_identity(symbol)
                == self.private_owner
    }

    fn check_field(
        &mut self,
        mut storage: StructuralTypeId,
        mut reference: TypeReferenceHandle,
    ) -> Option<()> {
        let source = self.accepted.source();
        let table = &source.type_reference_table;
        let module = self.accepted.module();
        let mut memberships = Vec::new();
        loop {
            match table.type_reference(reference) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in table.constraints(*constraints) {
                        if let TypeConstraintNode::Domain(domain) = constraint
                            && domain.subject == DomainConstraintSubject::Declared
                        {
                            memberships.push(domain);
                        }
                    }
                    reference = *base_type;
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    match module.structural_type(storage)?.shape {
                        StructuralTypeShape::Reference { referent, .. } => {
                            self.path.push(StructuralPathSegment::Referent);
                            storage = referent;
                        }
                        // A borrowed slice carries its backing and length as one value.
                        StructuralTypeShape::ElementView { .. }
                        | StructuralTypeShape::ByteSequence(_)
                            if matches!(
                                table.type_reference(table.unconstrained_type_reference(*referee)),
                                TypeReferenceNode::Slice { .. }
                            ) => {}
                        _ => return None,
                    }
                    reference = *referee;
                }
                _ => break,
            }
        }
        let shape = &module.structural_type(storage)?.shape;
        if matches!(shape, StructuralTypeShape::PrimitiveScalar(_)) {
            // Scalar predicates are accounted for with the primitive storage.
            return Some(());
        }
        for domain in memberships {
            let declaration = source.domain_definition(domain.symbol)?;
            if !declaration.is_public && !self.can_read_private(domain.symbol) {
                return None;
            }
            let coordinate = (
                domain.symbol.arena_index(),
                domain.symbol.generation(),
                domain.semantic_id.0,
            );
            self.pending
                .remove(&(self.path.clone(), coordinate))
                .then_some(())?;
        }
        match (shape, table.type_reference(reference)) {
            (
                StructuralTypeShape::FixedArray { element, .. },
                TypeReferenceNode::FixedArray { element_type, .. },
            )
            | (
                StructuralTypeShape::ElementView { element, .. },
                TypeReferenceNode::Slice { element_type },
            ) => {
                self.path.push(StructuralPathSegment::AllElements);
                self.check_field(*element, *element_type)
            }
            _ => self.check_record(storage),
        }
    }
}
