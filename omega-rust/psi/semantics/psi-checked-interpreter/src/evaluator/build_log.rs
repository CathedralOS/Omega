use super::*;

const BUILD_LOG_FACET_TYPE: &str = "$OmegaBuildLogFacet";

impl<'program> Evaluator<'program> {
    pub(super) fn try_build_log_write_line_value_call(
        &mut self,
        call: &psi_typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if call.target.as_str() != "write_line" || !call.receiver.is_valid() {
            return Ok(None);
        }
        let receiver = self.resolve_place(call.receiver, frame)?;
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        self.try_write_build_log_line(receiver, call.target_symbol, arguments, frame)
            .map(|handled| handled.then_some(Value::Unit))
    }

    pub(super) fn try_build_log_write_line_statement(
        &mut self,
        call: &psi_typed_trees::statement::TableCall,
        frame: &Frame,
    ) -> EvalResult<bool> {
        if call.target.as_str() != "write_line" || call.receiver.is_empty() {
            return Ok(false);
        }
        let Some(receiver) = self.statement_receiver_cell(call.receiver, frame)? else {
            return Ok(false);
        };
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        self.try_write_build_log_line(receiver, call.target_symbol, arguments, frame)
    }

    fn try_write_build_log_line(
        &mut self,
        receiver: Cell,
        target_symbol: SymbolHandle,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<bool> {
        let receiver = self.deref_cell(receiver);
        if !matches!(
            &*receiver.borrow(),
            Value::Struct { type_name, .. } if type_name == BUILD_LOG_FACET_TYPE
        ) {
            return Ok(false);
        }
        if !self.exact_build_log_write_line(target_symbol) {
            return Err(Halt::Trap(
                "build logging did not select the exact toolchain machine".to_owned(),
            ));
        }
        let [text] = arguments else {
            return Err(Halt::Trap(
                "build logging requires one byte-string argument".to_owned(),
            ));
        };
        let bytes = match self.eval_expression(*text, frame)? {
            Value::Str(bytes) => bytes.borrow().clone(),
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
        Ok(true)
    }

    fn exact_build_log_write_line(&self, target_symbol: SymbolHandle) -> bool {
        self.program.machines().iter().any(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached.as_str() == "BuildLog")
                && self.symbol_has_build_prelude_source(machine.symbol)
                && self.program.machine_states(machine).iter().any(|state| {
                    state.name.as_str() == "write_line"
                        && self.symbol_has_build_prelude_source(state.symbol)
                        && (!target_symbol.is_valid() || state.symbol == target_symbol)
                })
        })
    }
}
