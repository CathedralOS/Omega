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
            self.assume_normal_equation(selected, operand);
        }
    }
}
