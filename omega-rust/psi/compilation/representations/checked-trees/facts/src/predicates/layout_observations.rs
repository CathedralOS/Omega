//! Traverse retained scalar terms without interpreting geometry or source syntax.
use super::{CheckedBooleanExpression, CheckedIntegerMathTerm, CheckedScalarExpression};

impl CheckedScalarExpression {
    pub fn visit_layout_observations(&self, visit: &mut impl FnMut(&Self)) {
        match self {
            Self::LayoutObservation { .. } => visit(self),
            Self::IntegerBinary { left, right, .. } => {
                left.visit_layout_observations(visit);
                right.visit_layout_observations(visit);
            }
            Self::IntegerBitwiseNot { operand, .. }
            | Self::IntegerWiden { operand, .. }
            | Self::IntegerExactCast { operand, .. }
            | Self::RepresentationView { operand, .. } => operand.visit_layout_observations(visit),
            Self::Boolean(predicate) => predicate.visit_layout_observations(visit),
            Self::StructuralSequenceLength { path, .. } => visit_path(path, visit),
            Self::StructuralParameterIndexedRead {
                index,
                element_path,
                ..
            } => {
                index.expression.visit_layout_observations(visit);
                visit_path(element_path, visit);
            }
            Self::StorageRead { .. }
            | Self::Parameter { .. }
            | Self::ErasedParameter { .. }
            | Self::Local { .. }
            | Self::StructuralParameterField { .. }
            | Self::IntegerLiteral { .. }
            | Self::IeeeFloatLiteral { .. }
            | Self::ImplementationSelection { .. } => {}
        }
    }
}

fn visit_path(
    path: &[super::CheckedScalarReadProjection],
    visit: &mut impl FnMut(&CheckedScalarExpression),
) {
    for step in path {
        if let super::CheckedScalarReadProjection::RuntimeIndex(selector) = step {
            selector.expression.visit_layout_observations(visit);
        }
    }
}

impl CheckedBooleanExpression {
    pub fn visit_layout_observations(&self, visit: &mut impl FnMut(&CheckedScalarExpression)) {
        match self {
            Self::Not(operand) => operand.visit_layout_observations(visit),
            Self::Equal { left, right } | Self::And { left, right } | Self::Or { left, right } => {
                left.visit_layout_observations(visit);
                right.visit_layout_observations(visit);
            }
            Self::IntegerComparison { left, right, .. }
            | Self::ScalarIeeeFloatComparison { left, right, .. } => {
                left.visit_layout_observations(visit);
                right.visit_layout_observations(visit);
            }
            Self::IntegerMathComparison { left, right, .. } => {
                for expression in [left, right] {
                    let mut pending = vec![expression.root];
                    let mut visited = std::collections::HashSet::new();
                    while let Some(handle) = pending.pop() {
                        if !expression.terms.is_valid(handle) || !visited.insert(handle) {
                            continue;
                        }
                        match expression.terms.get(handle) {
                            CheckedIntegerMathTerm::Embed { operand, .. } => {
                                operand.visit_layout_observations(visit)
                            }
                            CheckedIntegerMathTerm::Add { left, right }
                            | CheckedIntegerMathTerm::Subtract { left, right }
                            | CheckedIntegerMathTerm::Multiply { left, right } => {
                                pending.extend([*right, *left])
                            }
                            CheckedIntegerMathTerm::Literal(_)
                            | CheckedIntegerMathTerm::BuiltinTypeMember(_) => {}
                        }
                    }
                }
            }
            Self::StorageRead { .. }
            | Self::Constant(_)
            | Self::Parameter { .. }
            | Self::ErasedParameter { .. }
            | Self::Local { .. }
            | Self::StructuralParameterField { .. }
            | Self::IeeeFloatComparison { .. }
            | Self::StructuralIdentity { .. }
            | Self::ByteSequenceEqual { .. }
            | Self::ByteSequencePredicate { .. }
            | Self::StructuralCaseMembership { .. } => {}
        }
    }
}
