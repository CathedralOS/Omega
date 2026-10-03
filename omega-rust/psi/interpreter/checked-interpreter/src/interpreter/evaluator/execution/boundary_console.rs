use crate::interpreter::evaluator::{
    EvalResult, Evaluator, FilesystemHostOperation, Halt, SymbolHandle, Value,
};
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;
impl<'program> Evaluator<'program> {
    /// Resolve one call target through an exact compiler-selected filesystem
    /// boundary before any provider authority is touched. Package-aware
    /// execution consumes Omega's accepted declaration symbol. Standalone
    /// execution retains the exact bundled-std source fallback.
    pub(in crate::interpreter::evaluator) fn exact_filesystem_host_operation(
        &self,
        target_symbol: SymbolHandle,
    ) -> EvalResult<Option<FilesystemHostOperation>> {
        if !target_symbol.is_valid() {
            return Ok(None);
        }
        let package_binding = self.filesystem_service_symbol;
        let Some(trait_definition) = self.program.traits().iter().find(|definition| {
            definition.is_boundary
                && match package_binding {
                    Some(symbol) => definition.symbol == symbol,
                    None => {
                        definition.name.as_str() == "FilesystemHost"
                            && self.symbol_has_exact_toolchain_source(
                                definition.symbol,
                                "filesystem_host.omg",
                            )
                    }
                }
        }) else {
            return Ok(None);
        };
        let Some(signature) = self
            .program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| {
                signature.symbol == target_symbol
                    && (package_binding.is_some()
                        || self.symbol_has_exact_toolchain_source(
                            signature.symbol,
                            "filesystem_host.omg",
                        ))
            })
        else {
            return Ok(None);
        };
        let operation = FilesystemHostOperation::from_canonical_name(signature.name.as_str())
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "selected filesystem host operation `{}` has no compiler-owned identity",
                    signature.name.as_str()
                ))
            })?;
        Ok(Some(operation))
    }

    fn symbol_has_exact_toolchain_source(
        &self,
        symbol: SymbolHandle,
        expected_source: &str,
    ) -> bool {
        self.program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| self.program.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new(expected_source))
            })
    }

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
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == symbol)
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
