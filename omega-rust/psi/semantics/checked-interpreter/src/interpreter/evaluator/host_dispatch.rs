use super::{
    DataMember, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, TableCall,
    Value, trap, unsupported,
};
use typed_trees::signature::StateSignature;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
impl<'program> Evaluator<'program> {
    /// The VIRTUAL TimeHost read ops (std::time rung 4, D12). The
    /// interpreter's clock is deterministic: `sleep` advances virtual_ticks
    /// by the slept milliseconds and these reads never advance it, so interp
    /// canaries assert EXACT values. Calibration: 1 tick = 1 ms (frequency
    /// 1000); wall clock = 2026-01-01T00:00:00Z + elapsed, already in Unix
    /// units (epoch offset 0). Native rebinds these to real clocks (rung 5)
    /// and its canaries assert inequalities instead.
    pub(super) fn virtual_time_host_value(&self, target: &str) -> Option<Value> {
        match target {
            "monotonic_ticks" => Some(Value::Int(self.virtual_ticks)),
            "monotonic_ticks_per_second" => Some(Value::Int(1000)),
            "wall_clock_raw" => Some(Value::Int(1767225600000 + self.virtual_ticks)),
            "wall_clock_units_per_second" => Some(Value::Int(1000)),
            "wall_clock_epoch_offset_seconds" => Some(Value::Int(0)),
            _ => None,
        }
    }

    pub(super) fn try_host_call(
        &mut self,
        call: &TableCall,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let filesystem_operation = self.exact_filesystem_host_operation(call.target_symbol)?;
        let intrinsic_method = if filesystem_operation.is_none() {
            self.exact_console_intrinsic_host_method(call.target_symbol)
        } else {
            None
        };
        if filesystem_operation.is_none()
            && intrinsic_method.is_none()
            && !self.is_boundary_call(call, frame)
        {
            return Ok(None);
        }
        // Any driven host-boundary call marks the run: the build-time
        // evaluation entry uses this as a DYNAMIC purity backstop (the static
        // effect surface does not fold host-authority audit facts in yet).
        self.host_boundary_touched = true;
        let target = intrinsic_method.unwrap_or(call.target.as_str());

        // Filesystem authority is selected only by the exact canonical
        // toolchain requirement symbol. The trusted requirement leaf routes
        // within the provider after that selection; package-controlled names
        // cannot enter this branch.
        if let Some(filesystem_operation) = filesystem_operation {
            let arguments = self
                .program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec();
            let value = self.try_filesystem_call(filesystem_operation, &arguments, frame)?;
            return Ok(Some(value));
        }

        // Everything past the Filesystem branch is a NON-fs host boundary
        // (console, exit, clock, gui) -- the granted-build backstop's line.
        // EXCEPTION (owner answer #5, 2026-07-11k): the CONSOLE WRITE family
        // is served during granted builds. The effect gate already verified
        // statically that the build machine reaches console only through
        // DECLARED stdout_io/stderr_io rows (a row-less boundary surfaces as
        // opaque `host_boundary` and refuses before evaluation starts), and
        // the granted entry flushes the buffered bytes to the compiler's
        // real streams -- "the interpreter should never just catch it".
        // Everything else keeps tripping the backstop (defense in depth
        // beneath the gate). The name family IS the interpreter's console
        // dispatch (the serve below matches the same names).
        let served_console_write = matches!(
            target,
            "write" | "write_line" | "write_error" | "write_error_line"
        );
        if !served_console_write {
            self.non_fs_host_boundary_touched = true;
        }

        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec();

        match target {
            "exit_process" => {
                // A `ProcessExit`-named boundary may only end the simulated
                // domain through the exact canonical toolchain requirement
                // (omega::language::core::process_exit). An ordinary package or
                // user declaration with a matching name is a lookalike and
                // does not acquire terminal meaning. Requirements on any other
                // boundary trait keep the transitional console lane.
                if let Some((trait_symbol, requirement_symbol)) =
                    self.exit_process_call_requirement(call, frame)
                    && self.program.traits().iter().any(|definition| {
                        definition.symbol == trait_symbol
                            && definition.name.as_str() == "ProcessExit"
                    })
                    && !self.is_canonical_process_exit_requirement(trait_symbol, requirement_symbol)
                {
                    return unsupported(
                        "boundary call `exit_process` names a non-canonical `ProcessExit` declaration; only the exact toolchain requirement ends the simulated domain"
                            .to_owned(),
                    );
                }
                let code = if let Some(first) = arguments.first() {
                    self.eval_expression(*first, frame)?
                        .as_int()
                        .ok_or_else(|| Halt::Trap("exit_process arg not integer".to_owned()))?
                } else {
                    0
                };
                Err(Halt::Exit(code as i32))
            }
            "write" | "write_line" | "write_error" | "write_error_line" => {
                let bytes = if let Some(first) = arguments.first() {
                    let value = self.eval_expression(*first, frame)?;
                    match value {
                        Value::Str(text) => text.borrow().to_vec(),
                        other => {
                            return unsupported(format!(
                                "host write of non-string value {other:?}"
                            ));
                        }
                    }
                } else {
                    Vec::new()
                };
                let stream = if target.starts_with("write_error") {
                    &mut self.stderr
                } else {
                    &mut self.stdout
                };
                stream.extend_from_slice(&bytes);
                if target.ends_with("_line") {
                    stream.push(b'\n');
                }
                Ok(Some(Value::Unit))
            }
            "read_byte" => {
                // The next raw stdin byte as `ByteRead::Byte { value }`, or
                // `ByteRead::Eof` at end-of-input (Eof = ordinal 0 = the ZII
                // zero case; sentinel spellings vetoed).
                // No CRLF normalization: byte-level readers see the stream
                // as-is.
                Ok(Some(self.read_stdin_byte_value(call.target_symbol)?))
            }
            "write_byte" => {
                // Append one byte (the argument's low 8 bits) to stdout.
                let byte = arguments
                    .first()
                    .and_then(|argument| self.eval_expression(*argument, frame).ok())
                    .and_then(|value| match value {
                        Value::Int(byte) => Some(byte as u8),
                        _ => None,
                    });
                match byte {
                    Some(byte) => {
                        self.stdout.push(byte);
                        Ok(Some(Value::Unit))
                    }
                    None => unsupported("write_byte expects one integer argument".to_string()),
                }
            }
            "read_line" => {
                // Bounded contracts serve their declared `LineReadResult`
                // surface (wiki/spec/resources/bounded_input.md): the call's
                // own resolved signature selects the contract, so bundled std
                // and fixture requirements cannot pick up the legacy path's
                // whole-owner/Boolean meaning. Calls without that exact result
                // shape are legacy local boundary fixtures; bundled std
                // read_line itself is an ordinary selected checked body over
                // its exact read_byte leaf.
                if let Some(type_symbol) = self.read_line_result_type(call, frame) {
                    let value = self.read_stdin_bounded_line(
                        arguments.first().copied(),
                        frame,
                        type_symbol,
                    )?;
                    return Ok(Some(value));
                }
                // Read up to the next newline from the remaining stdin into
                // the mutable text-carrier out-parameter. CRLF is normalized (a trailing `\r` is
                // dropped). Returns whether a line was available (some programs ignore it).
                let line = self.read_stdin_line();
                if let Some(first) = arguments.first()
                    && let Ok(cell) = self.resolve_place(*first, frame)
                {
                    let cell = self.deref_cell(cell);
                    if let Value::Str(text) = &*cell.borrow() {
                        text.replace(line.clone().into_bytes())
                            .map_err(Halt::Resource)?;
                    } else {
                        *cell.borrow_mut() = self.allocate_text(line.clone().into_bytes())?;
                    }
                }
                Ok(Some(Value::Bool(!line.is_empty())))
            }
            // TimeHost read ops (std::time rung 4): one shared helper for both
            // statement- and value-position dispatch.
            "monotonic_ticks"
            | "monotonic_ticks_per_second"
            | "wall_clock_raw"
            | "wall_clock_units_per_second"
            | "wall_clock_epoch_offset_seconds" => Ok(self.virtual_time_host_value(target)),
            "sleep" => {
                // Frame pacing: no REAL delay in the interpreter (real time has no
                // effect on the deterministic state the differential oracle
                // compares), but the VIRTUAL clock advances by the slept
                // milliseconds -- so tick-paced programs observe the same elapsed
                // arithmetic natively (where GetTickCount64 advances across a real
                // Sleep) and virtually.
                let slept = arguments
                    .first()
                    .and_then(|argument| self.eval_expression(*argument, frame).ok())
                    .and_then(|value| match value {
                        Value::Int(ms) => Some(ms.max(1)),
                        _ => None,
                    })
                    .unwrap_or(1);
                self.virtual_ticks += slept;
                Ok(Some(Value::Unit))
            }
            "tick_count" => {
                // A VIRTUAL monotonic millisecond counter: deterministic (the
                // differential oracle compares exit codes, and tick-based
                // programs must assert MONOTONICITY, not values), advancing on
                // every read and every sleep.
                self.virtual_ticks += 1;
                Ok(Some(Value::Int(self.virtual_ticks)))
            }
            other => unsupported(format!("host boundary call `{other}` not yet supported")),
        }
    }

    /// The `LineReadResult` data symbol when the boundary signature behind a
    /// `read_line` call returns that exact settled shape
    /// (wiki/spec/resources/bounded_input.md). Result-shape identity, not name
    /// spelling: `Unit`- or Boolean-returning local declarations keep the
    /// legacy fallback in `try_host_call`.
    fn read_line_result_type(&self, call: &TableCall, frame: &Frame) -> Option<SymbolHandle> {
        let signature = self.read_line_boundary_signature(call, frame)?;
        self.line_read_result_shape(signature.return_type)
    }

    /// Resolve the boundary signature a `read_line` call resolved against,
    /// through the same channels `is_boundary_call` admits: the call's target
    /// symbol naming a boundary-trait requirement, a compiler-intrinsic
    /// realization through its satisfied requirement, or the receiver field's
    /// declared boundary-trait type by method name.
    fn read_line_boundary_signature(
        &self,
        call: &TableCall,
        frame: &Frame,
    ) -> Option<&'program StateSignature> {
        if call.target_symbol.is_valid() {
            for definition in self.program.traits() {
                if !definition.is_boundary {
                    continue;
                }
                if let Some(signature) = self
                    .program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == call.target_symbol)
                {
                    return Some(signature);
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
                    if let Some(signature) = self
                        .program
                        .trait_machine_signatures(definition)
                        .iter()
                        .find(|signature| signature.symbol == requirement)
                    {
                        return Some(signature);
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
            let definition = self
                .program
                .traits()
                .iter()
                .find(|definition| definition.is_boundary && definition.symbol == type_symbol)
                .or_else(|| {
                    // `Service<Console>` carriers: the boundary trait is the
                    // carrier's type argument.
                    if let TypeReferenceNode::Generic { arguments, .. } = self
                        .program
                        .type_reference_table
                        .type_reference(field.type_reference)
                    {
                        let argument = self
                            .program
                            .type_reference_table
                            .type_reference_handles(*arguments)
                            .first()?;
                        let argument_symbol = self.program.type_reference_symbol(*argument);
                        return self.program.traits().iter().find(|definition| {
                            definition.is_boundary && definition.symbol == argument_symbol
                        });
                    }
                    None
                })?;
            return self
                .program
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| signature.name.as_str() == "read_line");
        }
        None
    }

    /// The bounded line-input result contract by declaration identity: data
    /// named `LineReadResult` with exactly `Invalid`, `LineComplete(count)`,
    /// `EndOfInput(count)`, and `Full(count)` variants whose payloads are one
    /// `u64` field named `count`. Mirrors the `ByteRead` shape check the
    /// validation crate applies to `read_byte`.
    fn line_read_result_shape(&self, return_type: TypeReferenceHandle) -> Option<SymbolHandle> {
        let TypeReferenceNode::Named { symbol, .. } = self
            .program
            .type_reference_table
            .type_reference(return_type)
        else {
            return None;
        };
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        if !symbol.is_valid()
            || data.name.as_str() != "LineReadResult"
            || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || !data.lifetime_parameters.is_empty()
            || !self.program.data_type_parameters(data).is_empty()
            || data.generic_instance.is_some()
            || data.quotient.is_some()
            || !data.where_facts.is_empty()
            || data.zero_gated
            || data.properties.carry.is_some()
            || data.properties.multiplicity == language_semantics::Multiplicity::Linear
        {
            return None;
        }
        let [
            DataMember::Variant(invalid),
            DataMember::Variant(line_complete),
            DataMember::Variant(end_of_input),
            DataMember::Variant(full),
        ] = self.program.data_members(data)
        else {
            return None;
        };
        if invalid.name.as_str() != "Invalid"
            || !invalid.payload.is_empty()
            || line_complete.name.as_str() != "LineComplete"
            || end_of_input.name.as_str() != "EndOfInput"
            || full.name.as_str() != "Full"
        {
            return None;
        }
        for variant in [line_complete, end_of_input, full] {
            let [field] = self.program.data_payload_fields(variant) else {
                return None;
            };
            if field.name.as_str() != "count"
                || field.relevance.is_erased()
                || self.program.primitive_type_reference(field.type_reference)
                    != Some(PrimitiveType::U64)
            {
                return None;
            }
        }
        Some(*symbol)
    }

    /// Greedy bounded line read per the `LineReadResult` contract: stop at the
    /// destination's own extent (`Full`), LF stored (`LineComplete`), or input
    /// end (`EndOfInput`). A zero-length destination is `Full(0)` without
    /// consuming input; a full destination is `Full` even when EOF is next;
    /// bytes are preserved exactly, no CRLF normalization.
    fn read_stdin_bounded_line(
        &mut self,
        destination: Option<ExpressionHandle>,
        frame: &mut Frame,
        type_symbol: SymbolHandle,
    ) -> EvalResult<Value> {
        let cell = match destination {
            Some(argument) => {
                let place = self.resolve_place(argument, frame)?;
                Some(self.deref_cell(place))
            }
            None => None,
        };
        let capacity = match &cell {
            Some(cell) => match &*cell.borrow() {
                Value::Str(text) => text.borrow().len(),
                Value::Array(elements) => elements.len(),
                _ => return trap("read_line destination is not a bounded byte carrier"),
            },
            None => 0,
        };
        let mut count: u64 = 0;
        let variant = loop {
            if count as usize >= capacity {
                break "Full";
            }
            let Some(byte) = self.stdin.get(self.stdin_cursor).copied() else {
                break "EndOfInput";
            };
            self.stdin_cursor += 1;
            if let Some(cell) = &cell {
                match &*cell.borrow() {
                    Value::Str(text) => text.write_byte(count as usize, byte).map_err(|_| {
                        Halt::Trap("read_line destination shrank during input".to_owned())
                    })?,
                    Value::Array(elements) => {
                        let Some(element) = elements.get(count as usize) else {
                            return trap("read_line destination shrank during input");
                        };
                        *element.borrow_mut() = Value::Int(i64::from(byte));
                    }
                    _ => return trap("read_line destination is not a bounded byte carrier"),
                }
            }
            count += 1;
            if byte == b'\n' {
                break "LineComplete";
            }
        };
        let count = i64::try_from(count)
            .map_err(|_| Halt::Resource("read_line byte count overflowed".to_owned()))?;
        Ok(Value::Enum {
            type_symbol,
            variant_name: variant.to_owned(),
            payload: vec![("count".to_owned(), self.allocate_cell(Value::Int(count))?)],
        })
    }

    /// Rejoin a concrete hosted leaf to its satisfied requirement before
    /// selecting host behavior. Other external realizations retain their own
    /// execution path, even when their method spelling matches a host method.
    pub(super) fn exact_console_intrinsic_host_method(
        &self,
        target_symbol: SymbolHandle,
    ) -> Option<&'static str> {
        let (requirement_symbol, provider_symbol) =
            validation::exact_compiler_intrinsic_boundary_requirement(self.program, target_symbol)?;
        let realization = self.program.machines().iter().find(|machine| {
            machine.attached_data_symbol == provider_symbol
                && self
                    .program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == target_symbol)
        })?;
        let (provider_name, trait_name) = match realization.name.as_str() {
            "ConsoleNativeProvider::read_byte"
            | "ConsoleNativeProvider::write_byte"
            | "ConsoleNativeProvider::exit_process" => ("ConsoleNativeProvider", "Console"),
            "ProcessExitNativeProvider::exit_process" => {
                ("ProcessExitNativeProvider", "ProcessExit")
            }
            _ => return None,
        };
        if realization.attached_data.as_ref().map(|name| name.as_str()) != Some(provider_name) {
            return None;
        }
        let requirement = self
            .program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary && definition.name.as_str() == trait_name)
            .flat_map(|definition| self.program.trait_machine_signatures(definition))
            .find(|requirement| requirement.symbol == requirement_symbol)?;
        match (realization.name.as_str(), requirement.name.as_str()) {
            ("ConsoleNativeProvider::read_byte", "read_byte")
                if self
                    .program
                    .state_signature_parameters(requirement)
                    .is_empty()
                    && validation::exact_byte_read_result_type(
                        self.program,
                        requirement.return_type,
                    )
                    .is_some() =>
            {
                Some("read_byte")
            }
            ("ConsoleNativeProvider::write_byte", "write_byte") => Some("write_byte"),
            ("ConsoleNativeProvider::exit_process", "exit_process") => Some("exit_process"),
            ("ProcessExitNativeProvider::exit_process", "exit_process") => Some("exit_process"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod byte_input_rejection_tests;

#[cfg(test)]
mod tests {
    use super::super::{CheckedTrees, Frame};
    use super::{Evaluator, Halt, Value};
    use crate::value::Cell;
    use std::collections::BTreeMap;
    use typed_trees::statement::{StatementNode, TableCall};

    /// Compile `Helper::take(&mut self.line...)` — an ordinary static call the
    /// single-source harness resolves — then retarget the statement's leaf and
    /// target symbol to the `read_line` boundary requirement. Service-carrier
    /// dispatch (`self.console.read_line`) is selected by build-time machinery
    /// this harness does not run; the requirement symbol is what
    /// `is_boundary_call` and the arm's contract check actually inspect.
    fn read_line_call(checked: &CheckedTrees, boundary_trait: &str) -> TableCall {
        let take = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .and_then(|machine| checked.machine_states(machine).first())
            .and_then(|state| {
                checked
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .find_map(|node| match node {
                        StatementNode::Call(call) if call.target.as_str() == "take" => {
                            Some(call.clone())
                        }
                        _ => None,
                    })
            })
            .expect("a Helper::take call statement in main");
        let requirement = checked
            .traits()
            .iter()
            .find(|definition| definition.is_boundary && definition.name.as_str() == boundary_trait)
            .and_then(|definition| {
                checked
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.name.as_str() == "read_line")
            })
            .expect("a read_line requirement on the boundary trait");
        TableCall {
            target: typed_trees::name::Identifier::generated_static("read_line"),
            target_symbol: requirement.symbol,
            ..take
        }
    }

    /// A `Main` instance with a `line` carrier of 165s (the marker byte whose
    /// survival proves the destination was not resized or rewritten past the
    /// bounded prefix).
    fn main_self(evaluator: &mut Evaluator<'_>, checked: &CheckedTrees, len: usize) -> Cell {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("Main::main machine");
        let line = evaluator
            .allocate_cell(
                evaluator
                    .allocate_text(vec![165; len])
                    .unwrap_or_else(|_| panic!("line text")),
            )
            .unwrap_or_else(|_| panic!("line cell"));
        let mut fields = BTreeMap::new();
        fields.insert("line".to_owned(), line);
        evaluator
            .allocate_cell(Value::Struct {
                type_symbol: machine.symbol,
                type_name: "Main".to_owned(),
                fields,
            })
            .unwrap_or_else(|_| panic!("self cell"))
    }

    fn line_bytes(self_cell: &Cell) -> Vec<u8> {
        let guard = self_cell.borrow();
        let Value::Struct { fields, .. } = &*guard else {
            panic!("self is a struct")
        };
        let line = fields.get("line").expect("line field");
        match &*line.borrow() {
            Value::Str(text) => text.borrow().to_vec(),
            other => panic!("line carrier is a text buffer, not {other:?}"),
        }
    }

    const BOUNDED_CONSOLE_SOURCE: &str = "pub data LineReadResult {
            case Invalid;
            case LineComplete(count: u64);
            case EndOfInput(count: u64);
            case Full(count: u64);
        }
        pub boundary trait Console {
            machine read_line(out_line: &mut [u8]) -> LineReadResult reaches Console;
        }
        data Main {
            line: [u8; 4];
        }
        pub data Helper {}
        machine Helper::take(out: &mut [u8]) {}
        machine Main::main(&mut self) {
            Helper::take(&mut self.line);
        }";

    #[test]
    fn bounded_read_line_serves_line_read_result_by_signature() {
        let checked = crate::front_end::checked_program(BOUNDED_CONSOLE_SOURCE);
        let call = read_line_call(&checked, "Console");
        let machine_symbol = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("Main machine")
            .symbol;
        // (stdin, expected variant, count, destination bytes, consumed)
        for (input, variant, count, bytes, consumed) in [
            (&b"ok\n"[..], "LineComplete", 3, &b"ok\n\xA5"[..], 3),
            (&b"abc\n"[..], "LineComplete", 4, &b"abc\n"[..], 4),
            // Capacity is reached before EOF or LF: Full, not EndOfInput.
            (&b"pqrs\nt"[..], "Full", 4, &b"pqrs"[..], 4),
            (&b"pqrs"[..], "Full", 4, &b"pqrs"[..], 4),
            (&b"xy"[..], "EndOfInput", 2, &b"xy\xA5\xA5"[..], 2),
            // Bytes are preserved exactly: the \r lands in the destination and
            // LF still terminates the line. The legacy path normalized CRLF.
            (&b"a\r\n"[..], "LineComplete", 3, &b"a\r\n\xA5"[..], 3),
            (&b""[..], "EndOfInput", 0, &b"\xA5\xA5\xA5\xA5"[..], 0),
        ] {
            let mut evaluator = Evaluator::new_checked(&checked, input);
            let self_cell = main_self(&mut evaluator, &checked, 4);
            let mut frame = Frame::bare(self_cell.clone(), machine_symbol);
            let value = evaluator
                .try_host_call(&call, &mut frame)
                .unwrap_or_else(|_| panic!("{input:?}: bounded serve"))
                .expect("boundary call admitted");
            let Value::Enum {
                variant_name,
                payload,
                ..
            } = value
            else {
                panic!("{input:?}: read_line returned {value:?}")
            };
            assert_eq!(variant_name, variant, "{input:?}");
            let [(name, count_cell)] = payload.as_slice() else {
                panic!("{input:?}: payload {payload:?}")
            };
            assert_eq!(name, "count", "{input:?}");
            assert!(
                matches!(&*count_cell.borrow(), Value::Int(value) if *value == count),
                "{input:?}: count payload"
            );
            assert_eq!(line_bytes(&frame.self_cell), bytes, "{input:?}");
            assert_eq!(evaluator.stdin_cursor, consumed, "{input:?}");
            assert!(evaluator.host_boundary_touched, "{input:?}");
            assert!(evaluator.non_fs_host_boundary_touched, "{input:?}");
        }
    }

    #[test]
    fn bounded_read_line_respects_bounded_views() {
        for (destination, input, variant, count, bytes, consumed) in [
            // A zero-length destination is Full(0) without consuming input;
            // the legacy fallback's owner replace could not serve a view.
            (
                "[0..0]",
                &b"ok\n"[..],
                "Full",
                0,
                &b"\xA5\xA5\xA5\xA5"[..],
                0,
            ),
            // The window's own extent bounds the read; bytes land inside it.
            (
                "[1..3]",
                &b"q\n"[..],
                "LineComplete",
                2,
                &b"\xA5q\n\xA5"[..],
                2,
            ),
            ("[1..3]", &b"wxyz"[..], "Full", 2, &b"\xA5wx\xA5"[..], 2),
        ] {
            let source = BOUNDED_CONSOLE_SOURCE.replace(
                "Helper::take(&mut self.line);",
                &format!("Helper::take(&mut self.line{destination});"),
            );
            let checked = crate::front_end::checked_program(&source);
            let call = read_line_call(&checked, "Console");
            let machine_symbol = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Main::main")
                .expect("Main machine")
                .symbol;
            let mut evaluator = Evaluator::new_checked(&checked, input);
            let self_cell = main_self(&mut evaluator, &checked, 4);
            let mut frame = Frame::bare(self_cell.clone(), machine_symbol);
            let value = evaluator
                .try_host_call(&call, &mut frame)
                .unwrap_or_else(|_| panic!("{input:?} {destination}: bounded serve"))
                .expect("boundary call admitted");
            let Value::Enum {
                variant_name,
                payload,
                ..
            } = value
            else {
                panic!("{input:?} {destination}: read_line returned {value:?}")
            };
            assert_eq!(variant_name, variant, "{input:?} {destination}");
            let [(_, count_cell)] = payload.as_slice() else {
                panic!("{input:?} {destination}: payload {payload:?}")
            };
            assert!(
                matches!(&*count_cell.borrow(), Value::Int(value) if *value == count),
                "{input:?} {destination}: count payload"
            );
            assert_eq!(
                line_bytes(&frame.self_cell),
                bytes,
                "{input:?} {destination}"
            );
            assert_eq!(evaluator.stdin_cursor, consumed, "{input:?} {destination}");
        }
    }

    #[test]
    fn unit_returning_read_line_keeps_the_legacy_fallback() {
        let checked = crate::front_end::checked_program(
            "pub domain [u8]::LineUtf8
            requires
                valid_utf8(self);
            pub domain [u8; 16]::LineUtf8
            requires
                valid_utf8(self);
            pub boundary trait Console {
                machine read_line(out_line: &mut [u8; 16] in LineUtf8);
            }
            data Main {
                line: [u8; 16] in LineUtf8;
            }
            pub data Helper {}
            machine Helper::take(out: &mut [u8; 16] in LineUtf8) {}
            machine Main::main(&mut self) {
                Helper::take(&mut self.line);
            }",
        );
        let call = read_line_call(&checked, "Console");
        let machine_symbol = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("Main machine")
            .symbol;
        let mut evaluator = Evaluator::new_checked(&checked, b"hi\n");
        let self_cell = main_self(&mut evaluator, &checked, 16);
        let mut frame = Frame::bare(self_cell.clone(), machine_symbol);
        let value = evaluator
            .try_host_call(&call, &mut frame)
            .unwrap_or_else(|_| panic!("legacy serve"))
            .expect("boundary call admitted");
        // The Unit-returning shape keeps the whole-owner Boolean fallback: the
        // carrier is replaced by the (CRLF-normalized) line and the result
        // reports whether a line was available.
        assert!(matches!(value, Value::Bool(true)));
        assert_eq!(line_bytes(&frame.self_cell), b"hi");
        assert_eq!(evaluator.stdin_cursor, 3);
        assert!(evaluator.host_boundary_touched);
        assert!(evaluator.non_fs_host_boundary_touched);
    }

    #[test]
    fn direct_console_byte_intrinsics_write_bytes_and_retain_host_backstops() {
        for declaration in [
            "boundary machine ConsoleNativeProvider::write_byte(byte: i32)
                satisfies Console::write_byte;",
            "machine ConsoleNativeProvider::write_byte(byte: i32)
                satisfies Console::write_byte via Binding::CompilerIntrinsic;",
        ] {
            let checked = crate::front_end::checked_program(&format!(
                "pub boundary trait Console {{
                    machine write_byte(byte: i32) reaches Console;
                }}
                pub data ConsoleNativeProvider {{}}
                {declaration}
                machine main() reaches Console {{
                    ConsoleNativeProvider::write_byte(70);
                    ConsoleNativeProvider::write_byte(128);
                    ConsoleNativeProvider::write_byte(10);
                }}"
            ));
            let mut evaluator = Evaluator::new_checked(&checked, &[]);
            assert!(evaluator.run_entry("main").is_ok(), "{declaration}");
            assert_eq!(evaluator.stdout, [b'F', 0x80, b'\n'], "{declaration}");
            assert!(evaluator.host_boundary_touched, "{declaration}");
            assert!(evaluator.non_fs_host_boundary_touched, "{declaration}");
        }
    }

    #[test]
    fn direct_console_byte_input_preserves_octets_eof_and_host_backstops() {
        let source = "pub data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
            pub boundary trait Console {
                machine read_byte() -> ByteRead reaches Console blocks; crashes Trap;
                machine write_byte(byte: i32) reaches Console;
            }
            pub data ConsoleNativeProvider {}
            machine ConsoleNativeProvider::read_byte() -> ByteRead
                satisfies Console::read_byte via Binding::CompilerIntrinsic
                crashes Trap blocks;
            boundary machine ConsoleNativeProvider::write_byte(byte: i32)
                satisfies Console::write_byte;
            machine main() reaches Console {
                let observed: ByteRead = block ConsoleNativeProvider::read_byte();
                transition observed {
                    ByteRead::Byte { value } -> emit(value)
                    ByteRead::Eof -> done()
                }
                state emit(value: i32) { ConsoleNativeProvider::write_byte(value); }
                state done() {}
            }";
        let guarded = source.replace(
            "let observed: ByteRead = block ConsoleNativeProvider::read_byte();\n                transition observed {\n                    ByteRead::Byte { value } -> emit(value)\n                    ByteRead::Eof -> done()",
            "transition block ConsoleNativeProvider::read_byte() {\n                    ByteRead::Eof -> done()\n                    ByteRead::Byte { value } -> emit(value)",
        );
        for source in [source.to_owned(), guarded] {
            let checked = crate::front_end::checked_program(&source);
            let input = (0..=255).collect::<Vec<u8>>();
            let mut evaluator = Evaluator::new_checked(&checked, &input);
            for consumed in 1..=input.len() {
                let result = evaluator.run_entry("main");
                assert!(result.is_ok(), "byte {consumed}");
                assert_eq!(evaluator.stdin_cursor, consumed);
                assert_eq!(evaluator.stdout, input[..consumed]);
                assert!(evaluator.host_boundary_touched);
                assert!(evaluator.non_fs_host_boundary_touched);
            }
            for _ in 0..2 {
                evaluator.host_boundary_touched = false;
                evaluator.non_fs_host_boundary_touched = false;
                assert!(evaluator.run_entry("main").is_ok());
                assert_eq!(evaluator.stdin_cursor, input.len());
                assert_eq!(evaluator.stdout, input);
                assert!(
                    evaluator.host_boundary_touched,
                    "EOF is an input observation"
                );
                assert!(evaluator.non_fs_host_boundary_touched);
            }
        }
    }

    #[test]
    fn byte_input_guard_does_not_memoize_a_separately_authored_successor_call() {
        let checked = crate::front_end::checked_program(
            "pub data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
            pub boundary trait Console {
                machine read_byte() -> ByteRead reaches Console blocks; crashes Trap;
                machine write_byte(byte: i32) reaches Console;
            }
            pub data ConsoleNativeProvider {}
            machine ConsoleNativeProvider::read_byte() -> ByteRead
                satisfies Console::read_byte via Binding::CompilerIntrinsic
                crashes Trap blocks;
            boundary machine ConsoleNativeProvider::write_byte(byte: i32)
                satisfies Console::write_byte;
            machine sample() -> i32 reaches Console {
                transition block ConsoleNativeProvider::read_byte() {
                    ByteRead::Eof -> end()
                    ByteRead::Byte { value } -> found(value)
                }
                state end() -> i32 { -1 }
                state found(value: i32) -> i32 { value }
            }
            machine main() reaches Console {
                transition block sample() == 0 {
                    true -> emit(block sample())
                    false -> emit(99)
                }
                state emit(value: i32) { ConsoleNativeProvider::write_byte(value); }
            }",
        );
        let mut evaluator = Evaluator::new_checked(&checked, &[0, 42]);
        assert!(evaluator.run_entry("main").is_ok());
        assert_eq!(evaluator.stdin_cursor, 2);
        assert_eq!(evaluator.stdout, [42]);
    }

    #[test]
    fn direct_console_exit_intrinsic_stops_before_the_next_byte() {
        for supply in ["boundary", "external"] {
            let declaration = if supply == "boundary" {
                "boundary machine ConsoleNativeProvider::exit_process(code: i32)
                    satisfies Console::exit_process;"
            } else {
                "machine ConsoleNativeProvider::exit_process(code: i32)
                    satisfies Console::exit_process via Binding::CompilerIntrinsic;"
            };
            let checked = crate::front_end::checked_program(&format!(
                "pub boundary trait Console {{
                    machine exit_process(code: i32) reaches Console;
                    machine write_byte(byte: i32) reaches Console;
                }}
                pub data ConsoleNativeProvider {{}}
                {declaration}
                boundary machine ConsoleNativeProvider::write_byte(byte: i32)
                    satisfies Console::write_byte;
                machine main() reaches Console {{
                    ConsoleNativeProvider::exit_process(70);
                    ConsoleNativeProvider::write_byte(88);
                }}"
            ));
            let mut evaluator = Evaluator::new_checked(&checked, &[]);
            assert!(
                matches!(evaluator.run_entry("main"), Err(Halt::Exit(70))),
                "{supply}",
            );
            assert!(evaluator.stdout.is_empty(), "{supply}");
            assert!(evaluator.host_boundary_touched, "{supply}");
            assert!(evaluator.non_fs_host_boundary_touched, "{supply}");
        }
    }

    #[test]
    fn concrete_lookalikes_do_not_gain_console_host_authority() {
        for (label, primitive, declaration, target) in [
            (
                "wrong signature",
                "i64",
                "boundary machine ConsoleNativeProvider::write_byte(byte: i64)
                    satisfies Console::write_byte;",
                "ConsoleNativeProvider::write_byte",
            ),
            (
                "wrong provider name",
                "i32",
                "boundary machine OtherProvider::write_byte(byte: i32)
                    satisfies Console::write_byte;",
                "OtherProvider::write_byte",
            ),
            (
                "custom intrinsic provider",
                "i32",
                "machine OtherProvider::write_byte(byte: i32)
                    satisfies Console::write_byte via Binding::CompilerIntrinsic;",
                "OtherProvider::write_byte",
            ),
            (
                "custom intrinsic method",
                "i32",
                "machine ConsoleNativeProvider::custom_write(byte: i32)
                    satisfies Console::write_byte via Binding::CompilerIntrinsic;",
                "ConsoleNativeProvider::custom_write",
            ),
            (
                "Syscall",
                "i32",
                "machine ConsoleNativeProvider::write_byte(byte: i32)
                    satisfies Console::write_byte
                    via Binding::Syscall(60);",
                "ConsoleNativeProvider::write_byte",
            ),
            (
                "satisfies another boundary trait",
                "i32",
                "boundary machine ConsoleNativeProvider::write_byte(byte: i32)
                    satisfies Other::write_byte;",
                "ConsoleNativeProvider::write_byte",
            ),
            (
                "ordinary authored body",
                "i32",
                "machine ConsoleNativeProvider::write_byte(byte: i32)
                    satisfies Console::write_byte {}",
                "ConsoleNativeProvider::write_byte",
            ),
        ] {
            let checked = crate::front_end::checked_program(&format!(
                "pub boundary trait Console {{
                    machine write_byte(byte: {primitive}) reaches Console;
                }}
                pub boundary trait Other {{
                    machine write_byte(byte: i32) reaches Other;
                }}
                pub data ConsoleNativeProvider {{}}
                pub data OtherProvider {{}}
                {declaration}
                machine main() reaches Console + Other {{ {target}(70); }}"
            ));
            let mut evaluator = Evaluator::new_checked(&checked, &[]);
            let realization = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == target)
                .expect("concrete test realization");
            let state = checked
                .machine_states(realization)
                .first()
                .expect("concrete test realization state");
            assert_eq!(
                evaluator.exact_console_intrinsic_host_method(state.symbol),
                None,
                "{label}",
            );
            // External fallback behavior is separate from host admission.
            let result = evaluator.run_entry("main");
            if label == "ordinary authored body" {
                assert!(result.is_ok(), "ordinary checked body must still execute");
            }
            assert!(evaluator.stdout.is_empty(), "{label}");
            assert!(!evaluator.host_boundary_touched, "{label}");
            assert!(!evaluator.non_fs_host_boundary_touched, "{label}");
        }
    }
}
