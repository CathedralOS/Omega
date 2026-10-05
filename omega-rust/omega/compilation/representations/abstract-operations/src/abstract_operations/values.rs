//! Scalar parameter, result and successor binding identities.

use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::StructuralResultDeclaration;

/// Refine runtime selectors whose current SSA producer is an exact integer
/// constant. This reads the current operation graph, never its Terminal
/// snapshot. Bounds, dominance and available storage remain consumer duties.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractResult {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractFunctionResult {
    Unit,
    Scalar(AbstractResult),
    Structural(StructuralResultDeclaration),
}

impl AbstractFunctionResult {
    pub const fn scalar(&self) -> Option<AbstractResult> {
        match self {
            Self::Unit => None,
            Self::Scalar(result) => Some(*result),
            Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&StructuralResultDeclaration> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueBinding {
    pub parameter: ValueId,
    pub argument: ValueId,
    pub scalar_type: ScalarType,
}
