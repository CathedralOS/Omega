//! Discover source contracts beneath original data fields and their actual
//! telescopes. Prepared fields are storage views: their copied region spelling
//! cannot supply semantic custody. This walk reads the original declarations,
//! retaining every simultaneous outer application beside each occurrence.
//!
//! Storage recursion is finite: a repeated declaration can continue only when
//! resolving a parameter removes an enclosing telescope, as in Wrapper<Wrapper<T>>.
//! A growing authored recursive field keeps that telescope and stops here.
//! Reference projections explicitly present in source are separate live roots;
//! this inventory never expands every possible future referent.

use super::{AppliedTraitScope, enclosing, project_type_reference};
use crate::TypedTrees;
use crate::data::DataMember;
use crate::references::TypedReferences;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub(super) fn service_application_roots(program: &TypedTrees) -> Vec<AppliedTraitScope<'_>> {
    let mut discovery = SourceFields {
        program,
        roots: Vec::new(),
        seen: Vec::new(),
        active: Vec::new(),
    };
    for occurrence in crate::references::collect_program_type_occurrences(program) {
        discovery.visit(
            occurrence.lexical_owner,
            occurrence.reference,
            &[],
            &[],
            &[],
        );
    }
    discovery.roots
}

struct SourceFields<'program> {
    program: &'program TypedTrees,
    roots: Vec<AppliedTraitScope<'program>>,
    seen: Vec<Vec<(SymbolHandle, TypeReferenceHandle, Vec<TypeReferenceHandle>)>>,
    active: Vec<(SymbolHandle, usize)>,
}

impl SourceFields<'_> {
    fn visit(
        &mut self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
        enclosing_applications: &[TypeReferenceHandle],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
    ) {
        let applied = project_type_reference(self.program, reference, substitutions, scope_starts);
        let reference = applied.reference;
        if !reference.is_valid() {
            return;
        }
        let enclosing_applications = &enclosing_applications[..applied.scope_starts.len()];
        let slot = reference.arena_index() as usize;
        if self.seen.len() <= slot {
            self.seen.resize_with(slot + 1, Vec::new);
        }
        // A field at u64 and bool under the same owner is two occurrences.
        // Generations, supplying owner and full frame path all remain in the key.
        // Only this membership bucket is ordered, not the traversal or frame
        // sequence. Root publication retains the authored discovery order.
        let seen = &mut self.seen[slot];
        let insertion =
            match seen.binary_search_by(|(existing_owner, existing_reference, frames)| {
                (
                    existing_owner.arena_index(),
                    existing_owner.generation(),
                    existing_reference.arena_index(),
                    existing_reference.generation(),
                )
                    .cmp(&(
                        owner.arena_index(),
                        owner.generation(),
                        reference.arena_index(),
                        reference.generation(),
                    ))
                    .then_with(|| {
                        frames
                            .iter()
                            .map(|frame| (frame.arena_index(), frame.generation()))
                            .cmp(
                                enclosing_applications
                                    .iter()
                                    .map(|frame| (frame.arena_index(), frame.generation())),
                            )
                    })
            }) {
                Ok(_) => return,
                Err(insertion) => insertion,
            };
        seen.insert(
            insertion,
            (owner, reference, enclosing_applications.to_vec()),
        );

        if let Ok(Some(contract)) =
            crate::service::inspect_service_carrier_contract(self.program, reference)
        {
            if let Some(scope) = contract.application(self.program, owner, enclosing_applications) {
                self.roots.push(scope);
            }
            return;
        }
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => return,
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.visit(
                    owner,
                    *base_type,
                    enclosing_applications,
                    applied.substitutions,
                    applied.scope_starts,
                );
                return;
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                self.visit(
                    owner,
                    *element_type,
                    enclosing_applications,
                    applied.substitutions,
                    applied.scope_starts,
                );
                return;
            }
            _ => {}
        }
        let Some(application) = enclosing::data_application(self.program, reference) else {
            return;
        };
        if self
            .active
            .iter()
            .rev()
            .find(|(ancestor, _)| *ancestor == application.declaration.symbol)
            .is_some_and(|(_, end)| applied.scope_starts.len() >= *end)
        {
            return;
        }
        let mut frames = enclosing_applications.to_vec();
        frames.push(reference);
        let mut starts = applied.scope_starts.to_vec();
        starts.push(applied.substitutions.len());
        let mut substitutions = applied.substitutions.to_vec();
        substitutions.extend(
            self.program
                .data_type_parameters(application.declaration)
                .iter()
                .zip(application.arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument)),
        );

        let mut fields = FieldTypes {
            program: self.program,
            references: Vec::new(),
        };
        for member in self.program.data_members(application.declaration) {
            match member {
                DataMember::Field(field) => crate::references::visit_type_reference(
                    self.program,
                    &mut fields,
                    field.type_reference,
                ),
                DataMember::Variant(variant) => {
                    for field in self.program.data_payload_fields(variant) {
                        crate::references::visit_type_reference(
                            self.program,
                            &mut fields,
                            field.type_reference,
                        );
                    }
                }
            }
        }
        self.active
            .push((application.declaration.symbol, starts.len()));
        for field in fields.references {
            self.visit(owner, field, &frames, &substitutions, &starts);
        }
        self.active.pop();
    }
}

struct FieldTypes<'program> {
    program: &'program TypedTrees,
    references: Vec<TypeReferenceHandle>,
}

impl Extend<SymbolHandle> for FieldTypes<'_> {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
}

impl TypedReferences for FieldTypes<'_> {
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if self.references.contains(&reference) {
            return false;
        }
        self.references.push(reference);
        // A borrow is not a by-value field-expansion edge. Its reached referent
        // projections already occur in the source-owned outer inventory.
        !matches!(
            self.program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. }
        )
    }

    fn expression(&mut self, _: crate::expression::ExpressionHandle) -> bool {
        false
    }

    fn mathematical_type(&mut self, _: crate::mathematical::MathematicalTypeHandle) -> bool {
        false
    }
}
