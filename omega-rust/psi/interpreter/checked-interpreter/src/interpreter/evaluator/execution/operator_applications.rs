//! Execute the requirement realization retained by checked operator facts.
//! Shared realization/semantic classification belongs to
//! `checked_trees::CheckedOperatorFacts::requirement_realization`;
//! this owner evaluates operands once and performs the selected operation.

use crate::interpreter::evaluator::{
    EvalResult, Evaluator, ExpressionHandle, Frame, SymbolHandle, Value, trap,
};
use checked_trees::RequirementRealization;

impl<'program> Evaluator<'program> {
    /// Run a call whose resolved target is a boundary requirement through its
    /// selected realization. `application` is the call expression in value
    /// position and the invalid handle in statement position. `Ok(None)`
    /// leaves the call to the remaining routes: the target is not such a
    /// requirement, or checked Psi does not select one realization this route
    /// executes.
    pub(in crate::interpreter::evaluator) fn try_operator_application(
        &mut self,
        target_symbol: SymbolHandle,
        application: ExpressionHandle,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let Some(facts) = self.operator_facts else {
            return Ok(None);
        };
        let realization =
            match facts.requirement_realization(self.program, target_symbol, application) {
                Ok(realization) => realization,
                Err(reason) => return trap(reason),
            };
        let Some(realization) = realization else {
            return Ok(None);
        };
        match realization {
            RequirementRealization::CheckedBody { machine, state } => {
                let arguments = self.eval_state_arguments(state, arguments, frame)?;
                // The realization's body is a separate activation: guard-subject
                // memoization belongs to the caller's transition, not to calls
                // inside the realization.
                let entered_guard_depth = self.guard_depth;
                self.guard_depth = 0;
                let value = self.run_state_collect(machine, state, arguments);
                self.guard_depth = entered_guard_depth;
                Ok(Some(value?.unwrap_or(Value::Unit)))
            }
            RequirementRealization::Conversion(conversion) => {
                let [argument] = arguments else {
                    return trap("a destination-owned conversion takes exactly one argument");
                };
                let value = self.eval_expression_at_type(*argument, conversion.parameter, frame)?;
                self.eval_named_conversion(
                    value,
                    Some(conversion.source),
                    Some(conversion.destination),
                    conversion.domain,
                )
                .map(Some)
            }
            RequirementRealization::PrimitiveFloatBinary {
                operation,
                primitive,
                domain,
                parameters,
            } => {
                let [left, right] = arguments else {
                    return trap("a primitive float operation takes exactly two operands");
                };
                let left = self.eval_expression_at_type(*left, parameters[0], frame)?;
                let right = self.eval_expression_at_type(*right, parameters[1], frame)?;
                self.eval_float_binary(operation, &left, &right, Some((primitive, domain)), None)
                    .map(Some)
            }
        }
    }
}
