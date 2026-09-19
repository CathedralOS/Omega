//! Compiler-owned required-output obligations (`builder.output.require`,
//! `complete`, `fail`) and sealed-file custody.
//!
//! `require` reserves one canonical output name and returns an opaque
//! `RequiredOutput` marker; the semantic row lives in the evaluator's private
//! `output_obligations` table, so evaluated code can carry the marker but
//! cannot read, convert, or fabricate the obligation. `complete` binds the
//! obligation to the sealed file its `BuildPath` names and issues an
//! `OutputReceipt` marker; when the file is not yet sealed it returns
//! `OutputCompletion::Retry` carrying the obligation and file custody back so
//! a checked error can be followed by an explicit retry. `fail` settles the
//! obligation as sticky `Failed`.
//!
//! Custody is enforced at the table, not the value: `Pending` is the only
//! state `complete`/`fail` accept, a completed obligation cannot complete
//! again, and a failed obligation stays failed for the rest of the
//! activation. An authored `RequiredOutput {}` or `OutputReceipt {}` has the
//! declared type name instead of the private marker name, so it can never
//! satisfy `described_output_obligation` or name a table row.
use super::{
    BTreeMap, Cell, EvalResult, Evaluator, ExpressionHandle, FilesystemGrantRootIdentity,
    FilesystemHostOperation, FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutputSource, FilesystemOperationAttemptOutcome, Frame, Halt,
    SymbolHandle, Value, rooted_build_path_parts, validate_build_relative_path,
};
use crate::MAX_BUILD_OUTPUT_OBLIGATIONS;

/// The marker type name carried by issued `RequiredOutput` values. An
/// authored `RequiredOutput {}` has the declared type name instead and is
/// refused by `described_output_obligation` below.
const REQUIRED_OUTPUT_TYPE: &str = "$OmegaBuildRequiredOutput";

/// The marker type name carried by issued `OutputReceipt` values. An
/// authored `OutputReceipt {}` has the declared type name instead.
const OUTPUT_RECEIPT_TYPE: &str = "$OmegaBuildOutputReceipt";

/// Sealed-file custody for one rooted path within this run. Only completed
/// filesystem attempts move custody: a failed create produces no writer and
/// a failed close retires nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::interpreter) enum OutputSealState {
    /// At least one writer descriptor bound to this path is still live.
    Open {
        /// Filesystem-attempt ordinal that opened the current writer set.
        since: usize,
    },
    /// The path's last writer retired at this filesystem-attempt ordinal.
    Sealed {
        /// The completed filesystem-attempt ordinal that sealed the path.
        at: usize,
    },
}

impl<'program> Evaluator<'program> {
    /// `builder.output.require` / `complete` / `fail` as a value-position
    /// call on the compiler-issued `Build.output` facet.
    pub(super) fn try_build_output_obligation_value_call(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if !call.receiver.is_valid()
            || !matches!(call.target.as_str(), "require" | "complete" | "fail")
        {
            return Ok(None);
        }
        let receiver = self.resolve_place(call.receiver, frame)?;
        let Some(output_root) =
            self.output_facet_root(receiver, call.target.as_str(), call.target_symbol)?
        else {
            return Ok(None);
        };
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        self.dispatch_build_output_obligation(call.target.as_str(), output_root, arguments, frame)
            .map(Some)
    }

    /// The statement-position twin: `builder.output.require(name);` still
    /// issues the obligation even though the marker result is dropped, and
    /// `complete`/`fail` still settle their operand obligation.
    pub(super) fn try_build_output_obligation_statement(
        &mut self,
        call: &typed_trees::statement::TableCall,
        frame: &Frame,
    ) -> EvalResult<bool> {
        if call.receiver.is_empty()
            || !matches!(call.target.as_str(), "require" | "complete" | "fail")
        {
            return Ok(false);
        }
        let Some(receiver) = self.statement_receiver_cell(call.receiver, frame)? else {
            return Ok(false);
        };
        let Some(output_root) =
            self.output_facet_root(receiver, call.target.as_str(), call.target_symbol)?
        else {
            return Ok(false);
        };
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        self.dispatch_build_output_obligation(call.target.as_str(), output_root, arguments, frame)?;
        Ok(true)
    }

    /// `required.path()` as a value-position call on an issued obligation
    /// marker: returns the obligation's declared canonical name.
    pub(super) fn try_required_output_path_value_call(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if call.target.as_str() != "path" || !call.receiver.is_valid() {
            return Ok(None);
        }
        let receiver = self.resolve_place(call.receiver, frame)?;
        let receiver = self.deref_cell(receiver);
        let is_marker = matches!(
            &*receiver.borrow(),
            Value::Struct { type_name, .. } if type_name == REQUIRED_OUTPUT_TYPE
        );
        if !is_marker {
            if call.target_symbol.is_valid()
                && self.exact_build_facet_method("RequiredOutput", "path", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "required-output path access requires a compiler-issued RequiredOutput"
                        .to_owned(),
                ));
            }
            return Ok(None);
        }
        if !self.exact_build_facet_method("RequiredOutput", "path", call.target_symbol) {
            return Err(Halt::Trap(
                "required-output path access did not select the exact toolchain machine".to_owned(),
            ));
        }
        let index = self.described_output_obligation(&receiver.borrow())?;
        let path = self.output_obligations[index].relative_path.clone();
        Ok(Some(self.allocate_text(path)?))
    }

    /// The statement-position twin of `required.path()`; the declared name is
    /// evaluated and dropped.
    pub(super) fn try_required_output_path_statement(
        &mut self,
        call: &typed_trees::statement::TableCall,
        frame: &Frame,
    ) -> EvalResult<bool> {
        if call.target.as_str() != "path" || call.receiver.is_empty() {
            return Ok(false);
        }
        let Some(receiver) = self.statement_receiver_cell(call.receiver, frame)? else {
            return Ok(false);
        };
        let is_marker = matches!(
            &*receiver.borrow(),
            Value::Struct { type_name, .. } if type_name == REQUIRED_OUTPUT_TYPE
        );
        if !is_marker {
            if call.target_symbol.is_valid()
                && self.exact_build_facet_method("RequiredOutput", "path", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "required-output path access requires a compiler-issued RequiredOutput"
                        .to_owned(),
                ));
            }
            return Ok(false);
        }
        if !self.exact_build_facet_method("RequiredOutput", "path", call.target_symbol) {
            return Err(Halt::Trap(
                "required-output path access did not select the exact toolchain machine".to_owned(),
            ));
        }
        self.described_output_obligation(&receiver.borrow())?;
        Ok(true)
    }

    /// The receiver must be the compiler-issued `BuildOutput` facet marker;
    /// an authored `BuildOutput` value can select the toolchain machine
    /// through member resolution but carries no output-root authority.
    /// Returns the facet's grant-root identity.
    fn output_facet_root(
        &self,
        receiver: Cell,
        target: &str,
        target_symbol: SymbolHandle,
    ) -> EvalResult<Option<FilesystemGrantRootIdentity>> {
        let receiver = self.deref_cell(receiver);
        let root = match &*receiver.borrow() {
            Value::Struct {
                type_name, fields, ..
            } if type_name == super::build_paths::OUTPUT_ROOT_FACET_TYPE => fields
                .get("root")
                .and_then(|root| root.borrow().as_int())
                .and_then(|root| u32::try_from(root).ok())
                .and_then(FilesystemGrantRootIdentity::new)
                .ok_or_else(|| {
                    Halt::Trap(
                        "build-output activation facet has no valid root identity".to_owned(),
                    )
                })?,
            _ => {
                if target_symbol.is_valid()
                    && self.exact_build_facet_method("BuildOutput", target, target_symbol)
                {
                    return Err(Halt::Trap(format!(
                        "build output `{target}` requires the compiler-issued Build.output facet"
                    )));
                }
                return Ok(None);
            }
        };
        if !self.exact_build_facet_method("BuildOutput", target, target_symbol) {
            return Err(Halt::Trap(format!(
                "build output `{target}` did not select the exact toolchain machine"
            )));
        }
        Ok(Some(root))
    }

    fn dispatch_build_output_obligation(
        &mut self,
        target: &str,
        output_root: FilesystemGrantRootIdentity,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Value> {
        match target {
            "require" => {
                let [relative] = arguments else {
                    return Err(Halt::Trap(
                        "required output declaration requires one relative path".to_owned(),
                    ));
                };
                let relative = self.eval_byte_operand(*relative, frame, "required output name")?;
                self.issue_output_obligation(output_root, relative)
            }
            "complete" => {
                let [obligation, file] = arguments else {
                    return Err(Halt::Trap(
                        "required output completion requires an obligation and a sealed file"
                            .to_owned(),
                    ));
                };
                let obligation = self.eval_expression(*obligation, frame)?;
                let file = self.eval_expression(*file, frame)?;
                self.complete_output_obligation(output_root, obligation, file)
            }
            "fail" => {
                let [obligation, diagnostic] = arguments else {
                    return Err(Halt::Trap(
                        "required output failure requires an obligation and a diagnostic"
                            .to_owned(),
                    ));
                };
                let obligation = self.eval_expression(*obligation, frame)?;
                let diagnostic =
                    self.eval_byte_operand(*diagnostic, frame, "required output diagnostic")?;
                self.fail_output_obligation(obligation, diagnostic)?;
                Ok(Value::Unit)
            }
            _ => unreachable!("dispatch is gated on require/complete/fail"),
        }
    }

    /// `require(name)`: reserve one canonical output name and issue its
    /// `Pending` obligation marker. Duplicate names and file/directory prefix
    /// collisions reject; registration is permanent for the activation, so a
    /// settled name still collides.
    fn issue_output_obligation(
        &mut self,
        output_root: FilesystemGrantRootIdentity,
        relative: Vec<u8>,
    ) -> EvalResult<Value> {
        validate_build_relative_path(&relative)?;
        for existing in &self.output_obligations {
            if existing.root != output_root {
                continue;
            }
            let mut existing_prefixed = existing.relative_path.clone();
            existing_prefixed.push(b'/');
            let mut name_prefixed = relative.clone();
            name_prefixed.push(b'/');
            if existing.relative_path == relative
                || relative.starts_with(&existing_prefixed)
                || existing.relative_path.starts_with(&name_prefixed)
            {
                return Err(Halt::Trap(format!(
                    "required build output `{}` duplicates or collides with `{}`",
                    String::from_utf8_lossy(&relative),
                    String::from_utf8_lossy(&existing.relative_path)
                )));
            }
        }
        if self.output_obligations.len() == MAX_BUILD_OUTPUT_OBLIGATIONS {
            return Err(Halt::Resource(format!(
                "required build outputs exceed the {MAX_BUILD_OUTPUT_OBLIGATIONS}-entry ceiling"
            )));
        }
        let index = self.output_obligations.len();
        self.output_obligations.try_reserve(1).map_err(|_| {
            Halt::Resource("required-output obligation allocation was refused".to_owned())
        })?;
        self.output_obligations.push(crate::BuildOutputObligation {
            root: output_root,
            relative_path: relative.clone(),
            issued_at: self.filesystem_operation_attempts.len(),
            state: crate::BuildOutputObligationState::Pending,
            post_completion_mutations: Vec::new(),
        });
        self.output_obligation_paths
            .insert((output_root, relative), index);
        self.required_output_marker(index)
    }

    /// `complete(obligation, file)`: bind the obligation to the sealed file
    /// `file` names. A pending obligation whose file is not yet sealed returns
    /// `Retry` with the obligation and file custody so the activation can
    /// finish the file and retry; every other custody violation is a hard
    /// rejection.
    fn complete_output_obligation(
        &mut self,
        output_root: FilesystemGrantRootIdentity,
        obligation: Value,
        file: Value,
    ) -> EvalResult<Value> {
        let index = self.described_output_obligation(&obligation)?;
        let declared = self.output_obligations[index].relative_path.clone();
        match self.output_obligations[index].state {
            crate::BuildOutputObligationState::Pending => {}
            crate::BuildOutputObligationState::Completed { .. } => {
                return Err(Halt::Trap(
                    "required output obligation is already completed".to_owned(),
                ));
            }
            crate::BuildOutputObligationState::Failed { .. } => {
                return Err(Halt::Trap(
                    "required output obligation is already failed".to_owned(),
                ));
            }
        }
        let Some((file_root, file_relative)) = rooted_build_path_parts(&file)? else {
            return Err(Halt::Trap(
                "required output completion requires a rooted build path".to_owned(),
            ));
        };
        if file_root != output_root {
            return Err(Halt::Trap(
                "required output completion names a file outside this activation's Output root"
                    .to_owned(),
            ));
        }
        if file_relative != declared {
            return Err(Halt::Trap(format!(
                "required output completion must name the obligation's reserved output `{}`",
                String::from_utf8_lossy(&declared)
            )));
        }
        let key = (output_root, file_relative);
        let Some(OutputSealState::Sealed { at: sealed_at }) =
            self.output_seal_states.get(&key).copied()
        else {
            // Completion error: the file is not yet sealed. Custody returns to
            // the caller; the obligation stays Pending so an explicit retry is
            // possible.
            let obligation_marker = self.required_output_marker(index)?;
            let type_symbol = self.output_completion_type_symbol();
            return Ok(Value::Enum {
                type_symbol,
                variant_name: "Retry".to_owned(),
                payload: vec![
                    (
                        "obligation".to_owned(),
                        self.allocate_cell(obligation_marker)?,
                    ),
                    ("file".to_owned(), self.allocate_cell(file)?),
                ],
            });
        };
        let completed_at = self.filesystem_operation_attempts.len();
        let receipt_index = self.output_receipts.len();
        self.output_receipts
            .try_reserve(1)
            .map_err(|_| Halt::Resource("output receipt allocation was refused".to_owned()))?;
        self.output_receipts.push(crate::BuildOutputReceipt {
            obligation: index,
            root: output_root,
            relative_path: key.1.clone(),
            sealed_at,
            completed_at,
        });
        self.output_obligations[index].state = crate::BuildOutputObligationState::Completed {
            completed_at,
            receipt: receipt_index,
        };
        let receipt = self.output_receipt_marker(receipt_index)?;
        let type_symbol = self.output_completion_type_symbol();
        Ok(Value::Enum {
            type_symbol,
            variant_name: "Sealed".to_owned(),
            payload: vec![("receipt".to_owned(), self.allocate_cell(receipt)?)],
        })
    }

    /// `fail(obligation, diagnostic)`: consume the obligation and record a
    /// sticky `Failed` state; the activation can never publish a successful
    /// product set.
    fn fail_output_obligation(&mut self, obligation: Value, diagnostic: Vec<u8>) -> EvalResult<()> {
        let index = self.described_output_obligation(&obligation)?;
        match self.output_obligations[index].state {
            crate::BuildOutputObligationState::Pending => {}
            crate::BuildOutputObligationState::Completed { .. } => {
                return Err(Halt::Trap(
                    "required output obligation is already completed".to_owned(),
                ));
            }
            crate::BuildOutputObligationState::Failed { .. } => {
                return Err(Halt::Trap(
                    "required output obligation is already failed".to_owned(),
                ));
            }
        }
        self.output_obligations[index].state = crate::BuildOutputObligationState::Failed {
            failed_at: self.filesystem_operation_attempts.len(),
            diagnostic,
        };
        Ok(())
    }

    /// Rejoin a `RequiredOutput` runtime value to the obligation the compiler
    /// issued for it. Only the evaluator's own marker shape is admitted: an
    /// authored `RequiredOutput {}`, a foreign struct, or an index outside the
    /// issued table cannot satisfy this check.
    fn described_output_obligation(&self, value: &Value) -> EvalResult<usize> {
        let Value::Struct {
            type_name, fields, ..
        } = value
        else {
            return Err(Halt::Trap(
                "output operation operand is not a compiler-issued required output obligation"
                    .to_owned(),
            ));
        };
        if type_name != REQUIRED_OUTPUT_TYPE {
            return Err(Halt::Trap(
                "output operation operand is not a compiler-issued required output obligation"
                    .to_owned(),
            ));
        }
        let index = fields
            .get("obligation")
            .and_then(|cell| cell.borrow().as_int())
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| {
                Halt::Trap(
                    "required output obligation carries no valid obligation index".to_owned(),
                )
            })?;
        if index >= self.output_obligations.len() {
            return Err(Halt::Trap(
                "required output obligation names an unknown obligation".to_owned(),
            ));
        }
        Ok(index)
    }

    fn required_output_marker(&mut self, index: usize) -> EvalResult<Value> {
        let index = i64::try_from(index).map_err(|_| {
            Halt::Resource("required-output obligation index overflowed".to_owned())
        })?;
        let obligation = self.allocate_cell(Value::Int(index))?;
        Ok(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: REQUIRED_OUTPUT_TYPE.to_owned(),
            fields: BTreeMap::from([("obligation".to_owned(), obligation)]),
        })
    }

    fn output_receipt_marker(&mut self, index: usize) -> EvalResult<Value> {
        let index = i64::try_from(index)
            .map_err(|_| Halt::Resource("output receipt index overflowed".to_owned()))?;
        let receipt = self.allocate_cell(Value::Int(index))?;
        Ok(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: OUTPUT_RECEIPT_TYPE.to_owned(),
            fields: BTreeMap::from([("receipt".to_owned(), receipt)]),
        })
    }

    /// The exact toolchain `OutputCompletion` data symbol so case values carry
    /// type-local tag resolution rather than a name-global scan.
    fn output_completion_type_symbol(&self) -> SymbolHandle {
        self.program
            .data_definitions()
            .iter()
            .find(|definition| {
                definition.name.as_str() == "OutputCompletion"
                    && self.symbol_has_build_prelude_source(definition.symbol)
            })
            .map(|definition| definition.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    /// Byte-data operand shared by `require` and `fail`: a `&[u8]`/`[u8; N]`
    /// value, never a rooted path or a carrier with another meaning.
    fn eval_byte_operand(
        &mut self,
        expression: ExpressionHandle,
        frame: &Frame,
        context: &str,
    ) -> EvalResult<Vec<u8>> {
        match self.eval_expression(expression, frame)? {
            Value::Str(bytes) => Ok(bytes.borrow().to_vec()),
            Value::Array(cells) => cells
                .iter()
                .map(|cell| {
                    let value = cell.borrow();
                    value
                        .as_int()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .ok_or_else(|| Halt::Trap(format!("{context} contains a non-byte element")))
                })
                .collect(),
            other => Err(Halt::Trap(format!(
                "{context} must be byte data, got {other:?}"
            ))),
        }
    }

    /// Update sealed-output custody from one completed filesystem attempt.
    /// Called after the attempt's outcome and observations are final, so only
    /// completed operations drive custody.
    ///
    /// - A successful `create`/`open`-class call binds its new descriptor to
    ///   the rooted path it resolved and marks the path `Open`.
    /// - A `write` through a live writer mutates its bound path.
    /// - A `close` retires its writer; the path becomes `Sealed` once its
    ///   last live writer is gone.
    /// - Any mutation of a completed obligation's path is recorded on the
    ///   obligation; final settlement rejects the activation.
    pub(super) fn note_completed_build_output_attempt(&mut self, attempt_index: usize) {
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        if !matches!(
            attempt.outcome(),
            Some(FilesystemOperationAttemptOutcome::Returned { .. })
        ) {
            return;
        }
        let mut mutated: Vec<(FilesystemGrantRootIdentity, Vec<u8>)> = Vec::new();
        if let Some(output) = attempt.logical_handle_output()
            && output.kind() == FilesystemLogicalHandleKind::Descriptor
        {
            match output.source() {
                FilesystemLogicalHandleOutputSource::Created => {
                    // The build facet's path-taking creator ops carry exactly
                    // one rooted path operand (operand 0).
                    if let Some(resolution) = attempt.rooted_path_operand_resolutions().first() {
                        let key = (resolution.root(), resolution.relative_path().to_vec());
                        self.output_open_writers
                            .insert(output.identity(), key.clone());
                        self.output_seal_states.insert(
                            key.clone(),
                            OutputSealState::Open {
                                since: attempt_index,
                            },
                        );
                        mutated.push(key);
                    }
                }
                FilesystemLogicalHandleOutputSource::Duplicated(source)
                | FilesystemLogicalHandleOutputSource::Borrowed(source) => {
                    if let Some(key) = self.output_open_writers.get(&source).cloned() {
                        self.output_open_writers.insert(output.identity(), key);
                    }
                }
            }
        }
        if attempt.operation_tag() == FilesystemHostOperation::Write.operation_tag() {
            for input in attempt.logical_handle_inputs() {
                if let FilesystemLogicalHandleInputResolution::Resolved(identity) =
                    input.resolution()
                    && let Some(key) = self.output_open_writers.get(&identity)
                {
                    mutated.push(key.clone());
                }
            }
        }
        for retired in attempt.retired_logical_handles() {
            if let Some(key) = self.output_open_writers.remove(retired)
                && !self.output_open_writers.values().any(|live| *live == key)
            {
                self.output_seal_states
                    .insert(key, OutputSealState::Sealed { at: attempt_index });
            }
        }
        for key in mutated {
            if let Some(index) = self.output_obligation_paths.get(&key).copied()
                && matches!(
                    self.output_obligations[index].state,
                    crate::BuildOutputObligationState::Completed { .. }
                )
            {
                self.output_obligations[index]
                    .post_completion_mutations
                    .push(attempt_index);
            }
        }
    }
}
