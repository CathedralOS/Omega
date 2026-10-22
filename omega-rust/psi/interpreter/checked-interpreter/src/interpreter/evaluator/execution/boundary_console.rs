use crate::interpreter::evaluator::{EvalResult, Evaluator, Halt, SymbolHandle, Value};
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;
impl<'program> Evaluator<'program> {
    /// Validate the exact requirement's byte result before advancing input.
    /// A same-spelled declaration or invalid result symbol supplies no identity.
    pub(in crate::interpreter::evaluator) fn read_stdin_byte_value(
        &mut self,
        target: SymbolHandle,
    ) -> EvalResult<Value> {
        let requirement =
            validation::exact_compiler_intrinsic_boundary_requirement(self.program, target)
                .map_or(target, |(requirement, _)| requirement);
        let type_symbol = self
            .program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary && definition.name.as_str() == "Console")
            .flat_map(|definition| self.program.trait_machine_signatures(definition))
            .find(|signature| {
                signature.symbol == requirement
                    && signature.name.as_str() == "read_byte"
                    && self
                        .program
                        .state_signature_parameters(signature)
                        .is_empty()
            })
            .and_then(|signature| {
                validation::exact_byte_read_result_type(self.program, signature.return_type)
            })
            .ok_or_else(|| {
                Halt::Unsupported("byte input has no exact supported result declaration".to_owned())
            })?;
        if self.stdin_cursor < self.stdin.len() {
            let byte = self.stdin[self.stdin_cursor];
            self.stdin_cursor += 1;
            Ok(Value::Enum {
                type_symbol,
                variant_name: "Byte".into(),
                payload: vec![(
                    "value".into(),
                    self.allocate_cell(Value::Int(i64::from(byte)))?,
                )],
            })
        } else {
            Ok(Value::Enum {
                type_symbol,
                variant_name: "Eof".into(),
                payload: Vec::new(),
            })
        }
    }

    pub(in crate::interpreter::evaluator) fn read_stdin_line(&mut self) -> String {
        let mut line = String::new();
        while self.stdin_cursor < self.stdin.len() {
            let byte = self.stdin[self.stdin_cursor];
            self.stdin_cursor += 1;
            if byte == b'\n' {
                break;
            }
            if byte == b'\r' {
                // Drop a CRLF terminator; a lone CR also ends the line.
                if self.stdin_cursor < self.stdin.len() && self.stdin[self.stdin_cursor] == b'\n' {
                    self.stdin_cursor += 1;
                }
                break;
            }
            line.push(byte as char);
        }
        line
    }

    /// The boundary-trait requirement a call's resolved target selects: the
    /// requirement signature itself, or an exact compiler-intrinsic provider
    /// realization through the requirement it satisfies. Resolution identity
    /// is the only authority. A qualifier (`Console::read_line`) names a
    /// declaration and a service field names storage; neither the receiver
    /// spelling nor the current instance's field types select a requirement.
    pub(in crate::interpreter::evaluator) fn boundary_call_requirement(
        &self,
        target_symbol: SymbolHandle,
    ) -> Option<(&'program TraitDefinition, &'program StateSignature)> {
        self.boundary_trait_requirement(target_symbol).or_else(|| {
            let (requirement, _provider) =
                validation::exact_compiler_intrinsic_boundary_requirement(
                    self.program,
                    target_symbol,
                )?;
            self.boundary_trait_requirement(requirement)
        })
    }

    /// The `boundary trait` requirement whose signature symbol is exactly
    /// `symbol`.
    pub(in crate::interpreter::evaluator) fn boundary_trait_requirement(
        &self,
        symbol: SymbolHandle,
    ) -> Option<(&'program TraitDefinition, &'program StateSignature)> {
        if !symbol.is_valid() {
            return None;
        }
        self.program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .find_map(|definition| {
                self.program
                    .trait_machine_signature_by_symbol(definition, symbol)
                    .map(|signature| (definition, signature))
            })
    }

    /// The exact canonical `ProcessExit::exit_process` requirement:
    /// `omega::language::core::process_exit` toolchain custody owns one closed
    /// public boundary trait with one `i32 -> Unit` signature. A same-named
    /// declaration elsewhere is an ordinary lookalike, not the terminal-event
    /// identity.
    pub(in crate::interpreter::evaluator) fn is_canonical_process_exit_requirement(
        &self,
        trait_symbol: SymbolHandle,
        requirement_symbol: SymbolHandle,
    ) -> bool {
        validation::is_canonical_process_exit_requirement(
            self.program,
            trait_symbol,
            requirement_symbol,
        )
    }
}
