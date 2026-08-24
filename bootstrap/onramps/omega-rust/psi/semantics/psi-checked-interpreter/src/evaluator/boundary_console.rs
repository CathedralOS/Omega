use super::*;

impl<'program> Evaluator<'program> {
    /// Resolve one call target to an exact canonical toolchain
    /// `FilesystemHost` requirement before any provider authority is touched.
    /// The returned readable leaf is handler routing only; the opaque target
    /// symbol and source ownership perform authority selection.
    pub(super) fn exact_filesystem_host_operation(
        &self,
        target_symbol: SymbolHandle,
    ) -> Option<String> {
        if !target_symbol.is_valid() {
            return None;
        }
        let trait_definition = self.program.traits().iter().find(|definition| {
            definition.name.as_str() == "FilesystemHost"
                && self.symbol_has_exact_toolchain_source(definition.symbol, "filesystem_host.omg")
        })?;
        let signature = self
            .program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| {
                signature.symbol == target_symbol
                    && self
                        .symbol_has_exact_toolchain_source(signature.symbol, "filesystem_host.omg")
            })?;
        Some(signature.name.as_str().to_owned())
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
                file.origin == psi_source::SourceOrigin::Toolchain
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(std::path::Path::new(expected_source))
            })
    }

    /// Consume the next line from the remaining stdin (without the line terminator). CRLF
    /// and LF are both handled; returns an empty string at end of input.
    /// One raw stdin byte as a std `ByteRead` value: `Byte { value }` while
    /// input remains, `Eof` after (ordinal 0 -- the ZII zero case; sentinel
    /// spellings vetoed). The declaring type resolves by
    /// name from std/console.omg (invalid + name-global fallback when a
    /// program shadows or lacks it, the WireVerdict precedent).
    pub(super) fn read_stdin_byte_value(&mut self) -> Value {
        let type_symbol = self
            .find_data_by_name("ByteRead")
            .map(|data| data.symbol)
            .unwrap_or_else(SymbolHandle::invalid);
        if self.stdin_cursor < self.stdin.len() {
            let byte = self.stdin[self.stdin_cursor];
            self.stdin_cursor += 1;
            Value::Enum {
                type_symbol,
                variant_name: "Byte".to_owned(),
                payload: vec![("value".to_owned(), Value::Int(i64::from(byte)).cell())],
            }
        } else {
            Value::Enum {
                type_symbol,
                variant_name: "Eof".to_owned(),
                payload: Vec::new(),
            }
        }
    }

    pub(super) fn read_stdin_line(&mut self) -> String {
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

    /// A call is a host-boundary call when its target state is declared on a
    /// `boundary trait` (matched by `target_symbol`, or by the receiver leaf naming a
    /// field whose type is a boundary trait).
    pub(super) fn is_boundary_call(&self, call: &TableCall, frame: &Frame) -> bool {
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
            if let Some(machine) = self.find_machine_by_name(&self_type) {
                if let Some(data_name) = machine.attached_data.as_ref() {
                    if let Some(data) = self.find_data_by_name(data_name.as_str()) {
                        for member in self.program.data_members(data) {
                            if let DataMember::Field(field) = member {
                                if field.name.as_str() == leaf {
                                    let type_symbol =
                                        self.program.type_reference_symbol(field.type_reference);
                                    if self.is_boundary_trait_symbol(type_symbol) {
                                        return true;
                                    }
                                    // Fallback for an imported boundary trait whose
                                    // `is_boundary` flag did not survive resolution (the std
                                    // `console`): a canonical host method on a `Console`-typed
                                    // field is a host call.
                                    let type_name =
                                        self.program.display_type_reference(field.type_reference);
                                    return type_name.contains("Console")
                                        && is_canonical_host_method(call.target.as_str());
                                }
                            }
                        }
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
