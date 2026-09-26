use crate::interpreter::evaluator::{
    DataMember, EvalResult, Evaluator, FilesystemHostOperation, Frame, Halt, SymbolHandle,
    TableCall, Value,
};
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

    /// Byte-exact custody for `omega::language::core::process_exit`: the
    /// toolchain origin and relative path alone cannot distinguish the core
    /// trait file from another toolchain module of the same name.
    fn symbol_has_exact_core_process_exit_source(&self, symbol: SymbolHandle) -> bool {
        const CORE_PROCESS_EXIT: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../source/library/core/process_exit.omg"
        ));
        self.program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| self.program.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new("process_exit.omg"))
                    && file.source.as_bytes() == CORE_PROCESS_EXIT
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
                variant_name: "Byte".to_owned(),
                payload: vec![(
                    "value".to_owned(),
                    self.allocate_cell(Value::Int(i64::from(byte)))?,
                )],
            })
        } else {
            Ok(Value::Enum {
                type_symbol,
                variant_name: "Eof".to_owned(),
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

    /// Resolve the boundary trait and requirement that own an `exit_process`
    /// call target: the requirement signature directly, an exact provider
    /// realization through its satisfied requirement, or the receiver field's
    /// declared boundary-trait type by method name.
    pub(in crate::interpreter::evaluator) fn exit_process_call_requirement(
        &self,
        call: &TableCall,
        frame: &Frame,
    ) -> Option<(SymbolHandle, SymbolHandle)> {
        if call.target_symbol.is_valid() {
            for definition in self.program.traits() {
                if !definition.is_boundary {
                    continue;
                }
                for signature in self.program.trait_machine_signatures(definition) {
                    if signature.symbol == call.target_symbol {
                        return Some((definition.symbol, signature.symbol));
                    }
                }
            }
            if let Some((requirement, _provider)) =
                validation::exact_compiler_intrinsic_boundary_requirement(
                    self.program,
                    call.target_symbol,
                )
            {
                for definition in self.program.traits() {
                    if !definition.is_boundary {
                        continue;
                    }
                    for signature in self.program.trait_machine_signatures(definition) {
                        if signature.symbol == requirement {
                            return Some((definition.symbol, signature.symbol));
                        }
                    }
                }
            }
        }

        let receiver_leaf = self
            .program
            .statement_table
            .name_path_members(call.receiver)
            .last()
            .map(|name| name.as_str().to_owned())?;
        let self_type = match &*frame.self_cell.borrow() {
            Value::Struct { type_name, .. } => type_name.clone(),
            _ => String::new(),
        };
        let machine = self.find_machine_by_name(&self_type)?;
        let data = self.find_data_by_name(machine.attached_data.as_ref()?.as_str())?;
        for member in self.program.data_members(data) {
            let DataMember::Field(field) = member else {
                continue;
            };
            if field.name.as_str() != receiver_leaf {
                continue;
            }
            let type_symbol = self.program.type_reference_symbol(field.type_reference);
            let definition =
                self.program.traits().iter().find(|definition| {
                    definition.is_boundary && definition.symbol == type_symbol
                })?;
            let signature = self
                .program
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| signature.name.as_str() == "exit_process")?;
            return Some((definition.symbol, signature.symbol));
        }
        None
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
        let Some(definition) = self
            .program
            .traits()
            .iter()
            .find(|definition| definition.symbol == trait_symbol)
        else {
            return false;
        };
        if !definition.is_boundary
            || definition.name.as_str() != "ProcessExit"
            || !definition.is_public
            || !definition.lifetime_parameters.is_empty()
            || !self.program.trait_type_parameters(definition).is_empty()
            || !self.symbol_has_exact_core_process_exit_source(definition.symbol)
        {
            return false;
        }
        let signatures = self.program.trait_machine_signatures(definition);
        let [signature] = signatures else {
            return false;
        };
        if signature.symbol != requirement_symbol
            || signature.name.as_str() != "exit_process"
            || !self.symbol_has_exact_core_process_exit_source(signature.symbol)
        {
            return false;
        }
        let parameters = self.program.state_signature_parameters(signature);
        let [parameter] = parameters else {
            return false;
        };
        !parameter.is_self
            && !parameter.is_const
            && !parameter.is_mutable
            && self
                .program
                .primitive_type_reference(parameter.type_reference)
                == Some(typed_trees::types::PrimitiveType::I32)
            && matches!(
                self.program
                    .type_reference_table
                    .type_reference(signature.return_type),
                typed_trees::types::TypeReferenceNode::Unit
            )
    }

    /// A call is a host-boundary call when its target state is declared on a
    /// `boundary trait` (matched by `target_symbol`, or by the receiver leaf naming a
    /// field whose type is a boundary trait).
    pub(in crate::interpreter::evaluator) fn is_boundary_call(
        &self,
        call: &TableCall,
        frame: &Frame,
    ) -> bool {
        // By target symbol: any boundary trait machine signature with this symbol.
        if call.target_symbol.is_valid() {
            for trait_definition in self.program.traits() {
                if !trait_definition.is_boundary {
                    continue;
                }
                for signature in self.program.trait_machine_signatures(trait_definition) {
                    if signature.symbol == call.target_symbol {
                        return true;
                    }
                }
            }
        }

        // By the receiver field's declared type being a boundary trait. The receiver leaf
        // (e.g. "console") names a field whose type symbol is a boundary trait.
        let receiver_leaf = self
            .program
            .statement_table
            .name_path_members(call.receiver)
            .last()
            .map(|name| name.as_str().to_owned());
        if let Some(leaf) = receiver_leaf {
            // The receiver field exists on `self`; look up its declared type via the
            // attached data definition.
            let self_type = match &*frame.self_cell.borrow() {
                Value::Struct { type_name, .. } => type_name.clone(),
                _ => String::new(),
            };
            if let Some(machine) = self.find_machine_by_name(&self_type)
                && let Some(data_name) = machine.attached_data.as_ref()
                && let Some(data) = self.find_data_by_name(data_name.as_str())
            {
                for member in self.program.data_members(data) {
                    if let DataMember::Field(field) = member
                        && field.name.as_str() == leaf
                    {
                        let type_symbol = self.program.type_reference_symbol(field.type_reference);
                        if self.is_boundary_trait_symbol(type_symbol) {
                            return true;
                        }
                        // Fallback for an imported boundary trait whose
                        // `is_boundary` flag did not survive resolution (the std
                        // `console`): a canonical host method on a `Console`-typed
                        // field is a host call.
                        let type_name = self.program.display_type_reference(field.type_reference);
                        return type_name.contains("Console")
                            && is_canonical_host_method(call.target.as_str());
                    }
                }
            }
        }

        false
    }

    fn is_boundary_trait_symbol(&self, symbol: SymbolHandle) -> bool {
        symbol.is_valid()
            && self.program.traits().iter().any(|trait_definition| {
                trait_definition.is_boundary && trait_definition.symbol == symbol
            })
    }
}

/// The canonical Console host-boundary method names the interpreter drives directly.
fn is_canonical_host_method(name: &str) -> bool {
    matches!(
        name,
        "write"
            | "write_line"
            | "write_error"
            | "write_error_line"
            | "read_line"
            | "read_byte"
            | "write_byte"
            | "exit_process"
            | "sleep"
            | "tick_count"
            | "key_state"
            | "dc_create"
            | "get_dc"
            | "window_create"
            | "blit"
            | "msg_peek"
            | "msg_translate"
            | "msg_dispatch"
            | "is_window"
            | "window_destroy"
            | "foreground_window"
    )
}
