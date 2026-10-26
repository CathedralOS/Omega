//! Completed array construction relates each stored element to its actual
//! evaluated operand. Descriptor metadata follows that exact element, while
//! reference-content authority stays with the separate observation owner.

use super::{ExpressionHandle, NormalOperandResult, StructuralJudge, StructuralTerm};

impl StructuralJudge<'_> {
    pub(super) fn intake_completed_array_elements(
        &mut self,
        value: &StructuralTerm,
        element_count: u32,
        inputs: &[(ExpressionHandle, NormalOperandResult)],
    ) {
        let Ok(element_count) = usize::try_from(element_count) else {
            return;
        };
        let StructuralTerm::CapturedValue { result_type, .. } = value else {
            return;
        };
        let Some(extent) =
            crate::value_custody::places::fixed_array_type_length(self.program, *result_type)
        else {
            return;
        };
        if extent != element_count || inputs.len() != element_count {
            return;
        }
        let Some(element_type) = crate::value_custody::places::collection_element_type_reference(
            self.program,
            *result_type,
        ) else {
            return;
        };
        for (element_index, (_, operand)) in inputs.iter().enumerate() {
            let Ok(element_index) = u64::try_from(element_index) else {
                return;
            };
            let Some(selected) = super::super::super::observed_projections::project_index(
                self.program,
                *result_type,
                value.clone(),
                StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(element_index)),
            ) else {
                return;
            };
            let Some(operand) = operand.observed_value() else {
                return;
            };
            self.assume_completed_value_equation(element_type, selected, operand.clone());
        }
    }
}
