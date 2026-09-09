use super::*;

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
        frame: &Frame,
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
            let args = self
                .program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec();
            let value = self.try_filesystem_call(filesystem_operation, &args, frame)?;
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
                // Read up to (and including) the next newline from the remaining stdin into
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

    /// Rejoin a concrete Console leaf to its satisfied requirement before
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
                && machine.attached_data.as_ref().map(|name| name.as_str())
                    == Some("ConsoleNativeProvider")
                && self
                    .program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == target_symbol)
        })?;
        let requirement = self
            .program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary && definition.name.as_str() == "Console")
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
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "host_dispatch/byte_input_rejection_tests.rs"]
mod byte_input_rejection_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use tokens_to_syntax_trees::parse_syntax_trees;
    use typed_trees_to_checked_trees::lower_typed_trees;

    pub(super) fn checked(source: &str) -> CheckedTrees {
        let tokens = Lexer::new(source).tokenize().expect("host-call tokens");
        let syntax = parse_syntax_trees(&tokens).expect("host-call syntax");
        let resolved = lower_syntax_trees(&syntax).expect("host-call symbols");
        let typed = lower_symbol_resolved_trees(&resolved).expect("host-call types");
        lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
    }

    #[test]
    fn direct_console_byte_intrinsics_write_bytes_and_retain_host_backstops() {
        for declaration in [
            "boundary machine ConsoleNativeProvider::write_byte(byte: i32)
                satisfies Console::write_byte;",
            "machine ConsoleNativeProvider::write_byte(byte: i32)
                satisfies Console::write_byte via Binding::CompilerIntrinsic;",
        ] {
            let checked = checked(&format!(
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
                machine read_byte() -> ByteRead reaches Console;
                machine write_byte(byte: i32) reaches Console;
            }
            pub data ConsoleNativeProvider {}
            machine ConsoleNativeProvider::read_byte() -> ByteRead
                satisfies Console::read_byte via Binding::CompilerIntrinsic;
            boundary machine ConsoleNativeProvider::write_byte(byte: i32)
                satisfies Console::write_byte;
            machine main() reaches Console {
                let observed: ByteRead = ConsoleNativeProvider::read_byte();
                transition observed {
                    ByteRead::Byte { value } -> emit(value)
                    ByteRead::Eof -> done()
                }
                state emit(value: i32) { ConsoleNativeProvider::write_byte(value); }
                state done() {}
            }";
        let guarded = source.replace(
            "let observed: ByteRead = ConsoleNativeProvider::read_byte();\n                transition observed {\n                    ByteRead::Byte { value } -> emit(value)\n                    ByteRead::Eof -> done()",
            "transition ConsoleNativeProvider::read_byte() {\n                    ByteRead::Eof -> done()\n                    ByteRead::Byte { value } -> emit(value)",
        );
        for source in [source.to_owned(), guarded] {
            let checked = checked(&source);
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
        let checked = checked(
            "pub data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
            pub boundary trait Console {
                machine read_byte() -> ByteRead reaches Console;
                machine write_byte(byte: i32) reaches Console;
            }
            pub data ConsoleNativeProvider {}
            machine ConsoleNativeProvider::read_byte() -> ByteRead
                satisfies Console::read_byte via Binding::CompilerIntrinsic;
            boundary machine ConsoleNativeProvider::write_byte(byte: i32)
                satisfies Console::write_byte;
            machine sample() -> i32 reaches Console {
                transition ConsoleNativeProvider::read_byte() {
                    ByteRead::Eof -> end()
                    ByteRead::Byte { value } -> found(value)
                }
                state end() -> i32 { -1 }
                state found(value: i32) -> i32 { value }
            }
            machine main() reaches Console {
                transition sample() == 0 {
                    true -> emit(sample())
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
            let checked = checked(&format!(
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
                "DllImport",
                "i32",
                "machine ConsoleNativeProvider::write_byte(byte: i32)
                    satisfies Console::write_byte
                    via Binding::DllImport(\"console\", \"write_byte\");",
                "ConsoleNativeProvider::write_byte",
            ),
            (
                "no satisfies edge",
                "i32",
                "boundary machine ConsoleNativeProvider::write_byte(byte: i32);",
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
            let checked = checked(&format!(
                "pub boundary trait Console {{
                    machine write_byte(byte: {primitive}) reaches Console;
                }}
                pub data ConsoleNativeProvider {{}}
                pub data OtherProvider {{}}
                {declaration}
                machine main() reaches Console {{ {target}(70); }}"
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
