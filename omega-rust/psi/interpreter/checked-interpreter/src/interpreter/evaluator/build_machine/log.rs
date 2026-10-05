use crate::interpreter::evaluator::{
    EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
};
use language_semantics::declaration_selection::BuildOperation;
const BUILD_LOG_FACET_TYPE: &str = "$OmegaBuildLogFacet";

impl<'program> Evaluator<'program> {
    /// `BuildLog::write_line(&mut builder.log, text)` carries the activation's
    /// log facet as its first explicit operand, the shape every other Build
    /// facet family already uses. Selection is the resolved prelude
    /// `(attached data, state)` pair, so an ordinary user machine spelled
    /// `write_line` never acquires build logging, and no spelling depends on a
    /// receiver place.
    pub(in crate::interpreter::evaluator) fn try_build_log_write_line_call(
        &mut self,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if self.build_prelude_facet_method(target)
            != Some(("BuildLog", BuildOperation::LogWriteLine.authored_spelling()))
        {
            return Ok(None);
        }
        let Some((facet, operands)) = arguments.split_first() else {
            return Err(Halt::Trap(
                "build logging has no explicit activation facet".to_owned(),
            ));
        };
        // Validate the compiler-issued facet before evaluating the text: only
        // the activation's own log slot grants retained build logging, and an
        // authored `BuildLog {}` has the same static shape with no slot.
        let facet = self.eval_argument(*facet, frame)?;
        let facet = self.deref_cell(facet);
        if !matches!(
            &*facet.borrow(),
            Value::Struct { type_name, .. } if **type_name == *BUILD_LOG_FACET_TYPE
        ) {
            return Err(Halt::Trap(
                "build logging requires the activation's compiler-issued BuildLog facet".to_owned(),
            ));
        }
        let [text] = operands else {
            return Err(Halt::Trap(
                "build logging requires one byte-string argument".to_owned(),
            ));
        };
        let bytes = match self.eval_expression(*text, frame)? {
            Value::Str(bytes) => bytes.borrow().to_vec(),
            Value::Array(cells) => cells
                .iter()
                .map(|cell| {
                    cell.borrow()
                        .as_int()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .ok_or_else(|| {
                            Halt::Trap("build log text contains a non-byte element".to_owned())
                        })
                })
                .collect::<EvalResult<Vec<_>>>()?,
            other => {
                return Err(Halt::Trap(format!(
                    "build log text must be byte data, got {other:?}"
                )));
            }
        };
        let appended = bytes
            .len()
            .checked_add(1)
            .ok_or_else(|| Halt::Resource("build log byte count overflowed".to_owned()))?;
        self.charge_build_log_bytes(appended)?;
        self.build_log
            .try_reserve(appended)
            .map_err(|_| Halt::Resource("build log allocation was refused".to_owned()))?;
        self.build_log.extend_from_slice(&bytes);
        self.build_log.push(b'\n');
        Ok(Some(Value::Unit))
    }
}
