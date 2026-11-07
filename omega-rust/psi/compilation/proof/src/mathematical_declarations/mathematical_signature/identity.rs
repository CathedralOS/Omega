//! Form the sealed core relation using the kernel's existing Id rule.
//!
//! The type argument supplies the carrier, never a value equation. Each subject
//! keeps its own source-domain admission before the kernel checks both at that
//! carrier. A formed Id has a universe; it is not a witness of itself.

use super::{Elaborator, Term, TermHandle};
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};

impl Elaborator<'_> {
    pub(super) fn elaborate_identity(
        &mut self,
        expression: ExpressionHandle,
        call: &TableCallExpression,
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        let application =
            validation::mathematical_applications::identity_application(self.program, call)
                .map_err(|reason| self.refuse(reason.to_owned()))?;
        let ty = self.elaborate_source_operand_type(application.carrier)?;
        let [left, right] = application.subjects;
        let left_term = self.elaborate_expression(left)?;
        self.retain_source_result(left, ty, left_term, false)?;
        let right_term = self.elaborate_expression(right)?;
        self.retain_source_result(right, ty, right_term, false)?;
        self.span = Some(self.program.expression_table.source_span(expression));
        Ok(self.arena.insert(Term::Id {
            ty,
            left: left_term,
            right: right_term,
        }))
    }
}
