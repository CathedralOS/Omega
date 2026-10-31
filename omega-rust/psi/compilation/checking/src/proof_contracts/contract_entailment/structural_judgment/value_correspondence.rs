//! A completed construction or exact store establishes value correspondence,
//! not observation of a referenced backing. Both producers retain the same
//! typed metadata equations and independently proved case tags without relaxing
//! the scalar descriptor fence. A general value rewrite never proves a tag.

use super::{CaseGuarantee, StructuralJudge, StructuralJudgment, StructuralTerm};
use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::types::TypeReferenceHandle;

impl StructuralJudge<'_> {
    pub(super) fn assume_completed_value_equation(
        &mut self,
        declared: TypeReferenceHandle,
        selected: StructuralTerm,
        operand: StructuralTerm,
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
        } else {
            self.intake_completed_case_correspondence(declared, &selected, &operand);
            self.assume_normal_equation(selected, operand);
        }
    }

    /// Both callers supply an exact completed-value coordinate: an element of
    /// a constructed array or a field snapshot of a current stored value.
    /// Publish its classifier from the completed operand, not from an equation
    /// normalizer or the shape of the expression that produced it.
    fn intake_completed_case_correspondence(
        &mut self,
        declared: TypeReferenceHandle,
        selected: &StructuralTerm,
        operand: &StructuralTerm,
    ) {
        let Some(data) =
            crate::value_custody::places::data_definition_for_type(self.program, declared)
        else {
            return;
        };
        let proven = self.program.data_members(data).iter().find_map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            matches!(
                self.judge_case(operand.clone(), data.symbol, variant.symbol),
                StructuralJudgment::Proven
            )
            .then_some(variant.symbol)
        });
        if let Some(case) = proven {
            self.intake_case_guarantee(&CaseGuarantee {
                subject: selected.clone(),
                data: data.symbol,
                case,
            });
        }
    }
}
