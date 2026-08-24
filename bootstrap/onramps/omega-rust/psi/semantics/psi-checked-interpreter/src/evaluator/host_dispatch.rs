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
        let filesystem_operation = self.exact_filesystem_host_operation(call.target_symbol);
        if filesystem_operation.is_none() && !self.is_boundary_call(call, frame) {
            return Ok(None);
        }
        // Any driven host-boundary call marks the run: the build-time
        // evaluation entry uses this as a DYNAMIC purity backstop (the static
        // effect surface does not fold host-authority audit facts in yet).
        self.host_boundary_touched = true;
        let target = call.target.as_str();

        // Filesystem authority is selected only by the exact canonical
        // toolchain requirement symbol. The trusted requirement leaf routes
        // within the provider after that selection; package-controlled names
        // cannot enter this branch.
        if let Some(filesystem_operation) = filesystem_operation {
            self.filesystem_host_observed = true;
            let args = self
                .program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec();
            if let Some(value) = self.try_filesystem_call(&filesystem_operation, &args, frame)? {
                return Ok(Some(value));
            }
            return unsupported(format!(
                "canonical filesystem host call `{filesystem_operation}` not yet supported"
            ));
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
                        Value::Str(text) => text.borrow().clone(),
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
                Ok(Some(self.read_stdin_byte_value()))
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
                if let Some(first) = arguments.first() {
                    if let Ok(cell) = self.resolve_place(*first, frame) {
                        let cell = self.deref_cell(cell);
                        if let Value::Str(text) = &*cell.borrow() {
                            *text.borrow_mut() = line.clone().into_bytes();
                        } else {
                            *cell.borrow_mut() = Value::str(line.clone());
                        }
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
}
