//! One selected comparison emitter for authored binary uses and saved Match operands.
//!
//! Both callers supply completed operands in the scalar prefix. Match keeps its
//! subject across failed arms; neither path re-evaluates source expressions here.
//! Exact selected requirement/application custody remains separate from Omega's
//! provider authority. Proof-only float equality never supplies executable meaning.
//! A selected IEEE float or authored-order integer comparison is one binding
//! whose emission records the occurrence row that joins it back to its use.
use super::{Expansion, LoweringError, Site, unsupported};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::source_custody::comparisons::occurrence;

#[cfg(test)]
mod tests;

impl Expansion<'_> {
    pub(super) fn comparison_binding(
        &self,
        operator_use: checked_trees::CheckedOperatorUseHandle,
        left: LoweredDirectExpression,
        right: LoweredDirectExpression,
        site: &Site<'_>,
    ) -> Result<LoweredScalarBinding, LoweringError> {
        let occurrence = occurrence(
            self.checked,
            operator_use,
            self.machine,
            site.state,
            site.statement,
        )?;
        let expected = occurrence.operand_type();
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return unsupported("selected comparison operand format changed");
        }
        Ok(LoweredScalarBinding::SelectedComparison {
            occurrence,
            source_machine: self.machine,
            left,
            right,
        })
    }
}
