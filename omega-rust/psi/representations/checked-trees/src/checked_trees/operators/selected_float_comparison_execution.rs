//! A selected realization is distinct from the source comparison it implements.
//! Omega supplies this child only after validating the exact provider plan.
//! Psi retains semantic coordinates, never target or provider implementation
//! types. The snapshot lets execution reject stale arms, operands and selection
//! without turning a Match into a fabricated binary expression.

use super::{CheckedOperatorFacts, CheckedOperatorUseFact, CheckedOperatorUseHandle};
use semantic_vocabulary::IeeeFloatComparisonOperation;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedSelectedFloatComparisonExecution {
    operator_use: CheckedOperatorUseHandle,
    selected: CheckedOperatorUseFact,
    operands: [ExpressionHandle; 2],
    comparison: IeeeFloatComparisonOperation,
    primitive: PrimitiveType,
}

impl Default for CheckedSelectedFloatComparisonExecution {
    fn default() -> Self {
        Self {
            operator_use: CheckedOperatorUseHandle::invalid(),
            selected: CheckedOperatorUseFact::default(),
            operands: [ExpressionHandle::invalid(); 2],
            comparison: IeeeFloatComparisonOperation::Equal,
            primitive: PrimitiveType::F32,
        }
    }
}

impl CheckedSelectedFloatComparisonExecution {
    /// Retain a compiler-resolved execution after the caller has checked its
    /// full selected plan and exact intrinsic identity. This constructor checks
    /// source/application custody; it does not select or validate a provider.
    pub fn from_selected_provider(
        program: &TypedTrees,
        facts: &CheckedOperatorFacts,
        operator_use: CheckedOperatorUseHandle,
        comparison: IeeeFloatComparisonOperation,
        primitive: PrimitiveType,
    ) -> Option<Self> {
        let selected = *facts.uses.get(operator_use);
        let operands = selected.operands(program)?.try_into().ok()?;
        let execution = Self {
            operator_use,
            selected,
            operands,
            comparison,
            primitive,
        };
        execution.validated_primitive(program, facts)?;
        Some(execution)
    }

    pub fn operator_use(&self) -> CheckedOperatorUseHandle {
        self.operator_use
    }

    /// Rejoin the complete occurrence before using already-evaluated operands.
    /// Duplicate roster children are rejected by the executing consumer.
    pub fn validated_primitive(
        &self,
        program: &TypedTrees,
        facts: &CheckedOperatorFacts,
    ) -> Option<PrimitiveType> {
        if !facts.uses.is_valid(self.operator_use)
            || facts.uses.get(self.operator_use) != &self.selected
            || self.selected.provider_plan_commitment.is_empty()
            || self.selected.provider_plan_report_fingerprint == 0
            || self.selected.operands(program)?.as_slice() != self.operands
            || facts.selected_float_comparison(program, self.operator_use)
                != Some((self.comparison, self.primitive))
        {
            return None;
        }
        let site = self.selected.application_site();
        let mut applications = facts
            .boundary_applications
            .iter()
            .filter(|application| application.site == site);
        let application = applications.next()?;
        if applications.next().is_some()
            || application.requirement_symbol != self.selected.selected_operator_symbol
            || !application.arguments.is_empty()
            || facts
                .symbolic_boundary_applications
                .iter()
                .any(|application| application.site == site)
        {
            return None;
        }
        Some(self.primitive)
    }
}
