//! The structural output contract of an already selected classification builtin.
//! Selection supplies the execution identity. This shape check never selects
//! an implementation from a source name or from a similarly shaped sum.
use calling_conventions::{ConventionalSumLayout, ValueShape};
use semantic_vocabulary::ScalarType;
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

pub fn conventional_float_class_layout(
    declaration: &StructuralTypeDeclaration,
) -> Option<ConventionalSumLayout> {
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    let [nan, infinity, normal, subnormal, zero] = cases.as_slice() else {
        return None;
    };
    if !nan.fields.is_empty()
        || [infinity, normal, subnormal, zero].iter().any(|case| {
            !matches!(case.fields.as_slice(), [field] if !field.relevance.is_erased()
            && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean))
        })
    {
        return None;
    }
    let layout = calling_conventions::evaluate_conventional_sum_layout(
        &[],
        &[
            vec![],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
        ],
    )
    .ok()?;
    // The selected scalar form computes these complete bytes. Admission derives
    // the layout first; a future different sum layout must change the form.
    if layout.shape != ValueShape::integer(8, 4)
        || layout.tag_byte_offset != 0 || layout.tag_shape != ValueShape::integer(4, 4)
        || layout.cases[1..].iter().any(|case| !matches!(case.fields.as_slice(), [field] if field.byte_offset == 4 && field.shape == ValueShape::integer(1, 1))) {
        return None;
    }
    Some(layout)
}
