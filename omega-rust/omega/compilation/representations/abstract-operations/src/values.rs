// We define scalar parameter, result, and successor binding identities, and
// refine runtime index selectors whose current SSA producer is an exact integer
// constant.

pub use native_vocabulary::values::{
    AbstractFunctionResult, AbstractParameter, AbstractResult, ValueBinding, binding_arguments,
};

pub fn resolve_constant_runtime_indexes<'operation>(
    operations: impl Iterator<Item = &'operation crate::AbstractOperation> + Clone,
    path: &[terminal_psi::StructuralPathSegment],
) -> Vec<terminal_psi::StructuralPathSegment> {
    use semantic_vocabulary::IntegerValue;
    use terminal_psi::StructuralPathSegment;

    path.iter()
        .map(|segment| {
            let StructuralPathSegment::RuntimeIndex { index, .. } = segment else {
                return segment.clone();
            };
            let mut constants = operations.clone().filter_map(|operation| match operation {
                crate::AbstractOperation::IntegerConstant { result, value, .. }
                    if result == index =>
                {
                    Some(*value)
                }
                _ => None,
            });
            let selected = match constants.next() {
                Some(IntegerValue::Signed(value)) => u64::try_from(value).ok(),
                Some(IntegerValue::Unsigned(value)) => u64::try_from(value).ok(),
                None => None,
            };
            if constants.next().is_some() {
                return segment.clone();
            }
            selected
                .map(StructuralPathSegment::FixedIndex)
                .unwrap_or_else(|| segment.clone())
        })
        .collect()
}
