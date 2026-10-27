//! A completed construction or exact store establishes value correspondence,
//! not observation of a referenced backing. Both producers retain the same
//! typed metadata equations without relaxing the scalar descriptor fence.

use super::{StructuralJudge, StructuralTerm};
use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::types::TypeReferenceHandle;

impl StructuralJudge<'_> {
    pub(super) fn assume_completed_value_equation(
        &mut self,
        declared: TypeReferenceHandle,
        selected: StructuralTerm,
        operand: StructuralTerm,
    ) {
        self.assume_completed_value_fields(declared, selected, operand, &mut Vec::new());
    }

    fn assume_completed_value_fields(
        &mut self,
        declared: TypeReferenceHandle,
        selected: StructuralTerm,
        operand: StructuralTerm,
        active_carriers: &mut Vec<TypeReferenceHandle>,
    ) {
        if crate::reference_shell(self.program, declared).is_some() {
            let length = |descriptor: StructuralTerm| {
                super::super::structural_terms::observed_collection_measure(
                    self.program,
                    declared,
                    CollectionMeasure::Length,
                    || Some(descriptor),
                )
            };
            if let (Some(selected), Some(operand)) = (length(selected), length(operand)) {
                self.assume_normal_equation(selected, operand);
            }
            return;
        }
        // A record containing a returned descriptor cannot become a scalar
        // equality: that would also equate unobserved referent contents. Its
        // completed inline fields nevertheless retain their own correspondence.
        // Project both exact completed operands through their declared carrier;
        // a reference child stops above at descriptor metadata, never its backing.
        let has_descriptor =
            super::super::structural_terms::term_is_unobserved_descriptor(self.program, &selected)
                || super::super::structural_terms::term_is_unobserved_descriptor(
                    self.program,
                    &operand,
                );
        self.assume_normal_equation(selected.clone(), operand.clone());
        if has_descriptor {
            let Some(carrier) =
                crate::value_custody::places::unwrapped_type_reference(self.program, declared)
            else {
                return;
            };
            // Checked inline carriers form a finite graph. References stopped
            // above; a repeated inline carrier cannot justify another projection.
            if active_carriers.contains(&carrier) {
                return;
            }
            let Some(data) = self.program.closed_data_definition(carrier) else {
                return;
            };
            active_carriers.push(carrier);
            let fields = self
                .program
                .data_members(data)
                .iter()
                .filter_map(|member| {
                    let typed_trees::data::DataMember::Field(field) = member else {
                        // A variant payload requires its separately established case.
                        return None;
                    };
                    let field_type = self
                        .program
                        .closed_data_member_field(carrier, field.symbol, field.name.as_str())?
                        .type_reference;
                    Some((field.symbol, field_type))
                })
                .collect::<Vec<_>>();
            for (symbol, field_type) in fields {
                let path = [facts::PlaceSegment::Field { symbol }];
                let Some(selected) = super::super::observed_projections::project_observed_value(
                    self.program,
                    declared,
                    selected.clone(),
                    &path,
                ) else {
                    continue;
                };
                let Some(operand) = super::super::observed_projections::project_observed_value(
                    self.program,
                    declared,
                    operand.clone(),
                    &path,
                ) else {
                    continue;
                };
                self.assume_completed_value_fields(field_type, selected, operand, active_carriers);
            }
            active_carriers.pop();
        }
    }
}
