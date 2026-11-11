/*
    Build-machine facet dispatch, rooted path provenance, product descriptions,
    and output obligation custody during checked evaluation.

    We service resolved `Build`, `BuildSource`, `BuildOutput`, `BuildLog`, and
    product-schema facet routes against the activation's root `Build` cell.
*/

use crate::interpreter::evaluator::{
    Cell, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, Value,
};
use symbols::SymbolHandle;

impl Evaluator<'_> {
    pub(crate) fn try_component_assumption_acceptance(
        &mut self,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if !target.is_valid()
            || !self.exact_build_facet_method("Build", "accept_component_assumption", target)
        {
            return Ok(None);
        }
        let [builder, digest] = arguments else {
            return Err(Halt::Trap(
                "component assumption acceptance takes an explicit Build argument and one digest spelling".to_owned(),
            ));
        };
        let builder = self.eval_argument(*builder, frame)?;
        self.require_root_build_argument(self.deref_cell(builder), "accept_component_assumption")?;
        self.eval_expression(*digest, frame)?;
        Ok(Some(Value::Unit))
    }

    pub(crate) fn try_build_facet_routes(
        &mut self,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        occurrence: source::SourceSpan,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if let Some(value) = self.try_build_path_call(target, arguments, frame)? {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.try_build_product_entry_call(target, arguments, occurrence, frame)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.try_build_product_schema_call(target, arguments, occurrence, frame)?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.try_build_product_provider_call(target, arguments, occurrence, frame)?
        {
            return Ok(Some(value));
        }
        if let Some(value) = self.try_product_description_path_call(target, arguments, frame)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.try_build_output_obligation_call(target, arguments, frame)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.try_required_output_path_call(target, arguments, frame)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.try_build_log_write_line_call(target, arguments, frame)? {
            return Ok(Some(value));
        }
        self.try_component_assumption_acceptance(target, arguments, frame)
    }

    pub(crate) fn try_build_product_description_call(
        &mut self,
        facet_method: &'static str,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        occurrence: source::SourceSpan,
        frame: &mut Frame,
        arity_message: &'static str,
        eval_operand: impl FnOnce(&mut Self, ExpressionHandle, &mut Frame) -> EvalResult<Vec<u8>>,
        issue: impl FnOnce(&mut Self, &[u8], source::SourceSpan) -> EvalResult<Value>,
    ) -> EvalResult<Option<Value>> {
        if !target.is_valid()
            || !self.exact_build_facet_method("BuildProduct", facet_method, target)
        {
            return Ok(None);
        }
        let [facet, path] = arguments else {
            return Err(Halt::Trap(arity_message.to_owned()));
        };
        self.require_product_facet(*facet, frame)?;
        let path = eval_operand(self, *path, frame)?;
        issue(self, &path, occurrence).map(Some)
    }
    pub(crate) fn require_root_build_argument(
        &self,
        builder: Cell,
        spelling: &str,
    ) -> EvalResult<()> {
        if self
            .root_build
            .as_ref()
            .is_some_and(|root| Cell::ptr_eq(&builder, root))
        {
            return Ok(());
        }
        Err(Halt::Trap(format!(
            "Build operation `{spelling}` requires the current activation's original Build value"
        )))
    }

    pub(crate) fn eval_byte_data_operand(
        &mut self,
        expression: ExpressionHandle,
        frame: &mut Frame,
        subject: &str,
    ) -> EvalResult<Vec<u8>> {
        match self.eval_expression(expression, frame)? {
            Value::Str(bytes) => Ok(bytes.borrow().to_vec()),
            Value::Array(cells) => cells
                .iter()
                .map(|cell| {
                    cell.borrow()
                        .as_int()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .ok_or_else(|| Halt::Trap(format!("{subject} contains a non-byte element")))
                })
                .collect(),
            other => Err(Halt::Trap(format!(
                "{subject} must be byte data, got {other:?}"
            ))),
        }
    }
}

fn required_index(
    fields: &std::collections::BTreeMap<crate::value::ValueName, Cell>,
    key: &str,
    label: &str,
) -> Result<usize, Halt> {
    fields
        .get(key)
        .and_then(|cell| cell.borrow().as_int())
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(|| Halt::Trap(format!("{label} carries no valid {key} index")))
}

fn described_product_index(
    value: &Value,
    expected_type: &str,
    guard_message: &'static str,
    index_label: &'static str,
) -> EvalResult<usize> {
    let Value::Struct {
        type_name, fields, ..
    } = value
    else {
        return Err(Halt::Trap(guard_message.to_owned()));
    };
    if &**type_name != expected_type {
        return Err(Halt::Trap(guard_message.to_owned()));
    }
    required_index(fields, "description", index_label)
}

pub(crate) mod behavior_exclusions {
    use crate::interpreter::evaluator::{
        EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, TableCall, Value,
        expression_table, statement_table, trap,
    };
    use crate::{
        ExecutedBehaviorExclusion, ExecutedBehaviorExclusionKind, ExecutedBehaviorExclusionSite,
    };
    use language_semantics::declaration_selection::BuildOperation;
    use typed_trees::statement::StatementHandle;

    impl<'program> Evaluator<'program> {
        pub(crate) fn try_behavior_exclusion_statement(
            &mut self,
            statement: StatementHandle,
            call: &TableCall,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let kind = if BuildOperation::from_call_target(call.target.as_str())
                == Some(BuildOperation::ServiceExclusion)
                && !call.target_symbol.is_valid()
            {
                self.evaluate_service_exclusion(
                    statement_table(self.program).expression_handles(call.arguments),
                    frame,
                )?;
                ExecutedBehaviorExclusionKind::Service
            } else {
                let arguments = statement_table(self.program).expression_handles(call.arguments);
                let Some(kind) =
                    self.evaluate_enum_exclusion(call.target_symbol, arguments, frame)?
                else {
                    return Ok(None);
                };
                kind
            };
            self.record_executed_exclusion(
                ExecutedBehaviorExclusionSite::Statement(statement),
                kind,
                frame,
            )?;
            Ok(Some(Value::Unit))
        }

        pub(crate) fn try_behavior_exclusion_value_call(
            &mut self,
            handle: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let kind = if BuildOperation::from_call_target(call.target.as_str())
                == Some(BuildOperation::ServiceExclusion)
                && !call.target_symbol.is_valid()
            {
                self.evaluate_service_exclusion(
                    expression_table(self.program).expression_handles(call.arguments),
                    frame,
                )?;
                ExecutedBehaviorExclusionKind::Service
            } else {
                let arguments = expression_table(self.program).expression_handles(call.arguments);
                let Some(kind) =
                    self.evaluate_enum_exclusion(call.target_symbol, arguments, frame)?
                else {
                    return Ok(None);
                };
                kind
            };
            self.record_executed_exclusion(
                ExecutedBehaviorExclusionSite::Expression(handle),
                kind,
                frame,
            )?;
            Ok(Some(Value::Unit))
        }

        fn evaluate_service_exclusion(
            &mut self,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<()> {
            let [builder] = arguments else {
                return trap("service exclusion requires one explicit mutable Build argument");
            };
            let builder = self.eval_argument(*builder, frame)?;
            self.require_root_build_argument(self.deref_cell(builder), "exclude_service")
        }

        fn evaluate_enum_exclusion(
            &mut self,
            target_symbol: SymbolHandle,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<ExecutedBehaviorExclusionKind>> {
            if !target_symbol.is_valid() || !self.symbol_has_build_prelude_source(target_symbol) {
                return Ok(None);
            }
            let (method_name, type_name) =
                if self.exact_build_facet_method("Build", "exclude_crash", target_symbol) {
                    ("exclude_crash", "CrashCause")
                } else if self.exact_build_facet_method(
                    "Build",
                    "exclude_physical_authority",
                    target_symbol,
                ) {
                    ("exclude_physical_authority", "PhysicalAuthorityClass")
                } else {
                    return Ok(None);
                };
            let [builder, argument] = arguments else {
                return trap(format!(
                    "behavior exclusion `{method_name}` takes a Build reference and one {type_name} value"
                ));
            };
            let builder = self.eval_argument(*builder, frame)?;
            self.require_root_build_argument(self.deref_cell(builder), method_name)?;
            let case_symbol =
                self.evaluated_exclusion_case(*argument, frame, method_name, type_name)?;
            Ok(Some(match type_name {
                "CrashCause" => ExecutedBehaviorExclusionKind::CrashCause { case_symbol },
                _ => ExecutedBehaviorExclusionKind::PhysicalAuthorityClass { case_symbol },
            }))
        }

        fn evaluated_exclusion_case(
            &mut self,
            argument: ExpressionHandle,
            frame: &mut Frame,
            method_name: &str,
            type_name: &str,
        ) -> EvalResult<SymbolHandle> {
            let Value::Enum {
                type_symbol,
                variant_name,
                payload,
            } = self.eval_expression(argument, frame)?
            else {
                return trap(format!(
                    "behavior exclusion `{method_name}` argument did not evaluate to a compiler-owned {type_name} case"
                ));
            };
            let is_toolchain_case = type_symbol != SymbolHandle::invalid()
                && self.symbol_has_build_prelude_source(type_symbol)
                && self.program.data_definitions().iter().any(|definition| {
                    definition.symbol == type_symbol && definition.name.as_str() == type_name
                });
            if !is_toolchain_case {
                return trap(format!(
                    "behavior exclusion `{method_name}` argument did not evaluate to a compiler-owned {type_name} case"
                ));
            }
            if !payload.is_empty() {
                return trap(format!(
                    "compiler-owned {type_name} case unexpectedly carries a payload"
                ));
            }
            let variant = self
                .program
                .data_definition_by_symbol(type_symbol)
                .and_then(|data| {
                    self.program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Variant(variant)
                                if *variant.name.as_str() == *variant_name =>
                            {
                                Some(variant.symbol)
                            }
                            _ => None,
                        })
                });
            variant.ok_or_else(|| {
                Halt::Trap(format!(
                    "evaluated {type_name} value names no declared compiler-owned case"
                ))
            })
        }

        fn record_executed_exclusion(
            &mut self,
            site: ExecutedBehaviorExclusionSite,
            kind: ExecutedBehaviorExclusionKind,
            frame: &Frame,
        ) -> EvalResult<()> {
            self.executed_behavior_exclusions
                .try_reserve(1)
                .map_err(|_| {
                    Halt::Resource("behavior-exclusion result allocation was refused".to_owned())
                })?;
            self.executed_behavior_exclusions
                .push(ExecutedBehaviorExclusion {
                    machine: frame.machine_symbol,
                    site,
                    kind,
                });
            Ok(())
        }
    }
}

pub(crate) mod log {
    use crate::interpreter::evaluator::{
        EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
    };
    use language_semantics::declaration_selection::BuildOperation;
    const BUILD_LOG_FACET_TYPE: &str = "$OmegaBuildLogFacet";

    impl<'program> Evaluator<'program> {
        pub(crate) fn try_build_log_write_line_call(
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
            let facet = self.eval_argument(*facet, frame)?;
            let facet = self.deref_cell(facet);
            if !matches!(
                &*facet.borrow(),
                Value::Struct { type_name, .. } if **type_name == *BUILD_LOG_FACET_TYPE
            ) {
                return Err(Halt::Trap(
                    "build logging requires the activation's compiler-issued BuildLog facet"
                        .to_owned(),
                ));
            }
            let [text] = operands else {
                return Err(Halt::Trap(
                    "build logging requires one byte-string argument".to_owned(),
                ));
            };
            let bytes = self.eval_byte_data_operand(*text, frame, "build log text")?;
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
}

pub(crate) mod output_obligations {
    use super::paths::OUTPUT_ROOT_FACET_TYPE;
    use crate::BuildOutputObligationState::Completed;
    use crate::MAX_BUILD_OUTPUT_OBLIGATIONS;
    use crate::interpreter::evaluator::{
        BTreeMap, EvalResult, Evaluator, ExpressionHandle, FilesystemGrantRootIdentity,
        FilesystemHostOperation, FilesystemLogicalHandleInputResolution,
        FilesystemLogicalHandleKind, FilesystemLogicalHandleOutputSource,
        FilesystemOperationAttemptOutcome, Frame, Halt, SymbolHandle, Value,
        rooted_build_path_parts, validate_build_relative_path,
    };

    const REQUIRED_OUTPUT_TYPE: &str = "$OmegaBuildRequiredOutput";

    const OUTPUT_RECEIPT_TYPE: &str = "$OmegaBuildOutputReceipt";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum OutputSealState {
        Open { since: usize },
        Sealed { at: usize },
    }

    impl<'program> Evaluator<'program> {
        pub(crate) fn try_build_output_obligation_call(
            &mut self,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some(("BuildOutput", operation @ ("require" | "complete" | "fail"))) =
                self.build_prelude_facet_method(target)
            else {
                return Ok(None);
            };
            let Some((facet, operands)) = arguments.split_first() else {
                return Err(Halt::Trap(format!(
                    "build output `{operation}` has no explicit activation facet"
                )));
            };
            let output_root = self.build_root_argument(*facet, OUTPUT_ROOT_FACET_TYPE, frame)?;
            self.dispatch_build_output_obligation(operation, output_root, operands, frame)
                .map(Some)
        }

        pub(crate) fn try_required_output_path_call(
            &mut self,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some(("RequiredOutput", "path")) = self.build_prelude_facet_method(target) else {
                return Ok(None);
            };
            let [obligation] = arguments else {
                return Err(Halt::Trap(
                    "required-output path access requires one obligation operand".to_owned(),
                ));
            };
            let obligation = self.eval_argument(*obligation, frame)?;
            let obligation = self.deref_cell(obligation);
            let index = self.described_output_obligation(&obligation.borrow())?;
            let path = self.output_obligations[index].relative_path.clone();
            Ok(Some(self.allocate_text(path)?))
        }

        fn dispatch_build_output_obligation(
            &mut self,
            target: &str,
            output_root: FilesystemGrantRootIdentity,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            match target {
                "require" => {
                    let [relative] = arguments else {
                        return Err(Halt::Trap(
                            "required output declaration requires one relative path".to_owned(),
                        ));
                    };
                    let relative =
                        self.eval_byte_operand(*relative, frame, "required output name")?;
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
                Completed { .. } => {
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
                let obligation_marker = self.required_output_marker(index)?;
                let type_symbol = self.output_completion_type_symbol();
                return Ok(Value::Enum {
                    type_symbol,
                    variant_name: "Retry".into(),
                    payload: vec![
                        ("obligation".into(), self.allocate_cell(obligation_marker)?),
                        ("file".into(), self.allocate_cell(file)?),
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
            self.output_obligations[index].state = Completed {
                completed_at,
                receipt: receipt_index,
            };
            let receipt = self.output_receipt_marker(receipt_index)?;
            let type_symbol = self.output_completion_type_symbol();
            Ok(Value::Enum {
                type_symbol,
                variant_name: "Sealed".into(),
                payload: vec![("receipt".into(), self.allocate_cell(receipt)?)],
            })
        }

        fn fail_output_obligation(
            &mut self,
            obligation: Value,
            diagnostic: Vec<u8>,
        ) -> EvalResult<()> {
            let index = self.described_output_obligation(&obligation)?;
            match self.output_obligations[index].state {
                crate::BuildOutputObligationState::Pending => {}
                Completed { .. } => {
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
            if **type_name != *REQUIRED_OUTPUT_TYPE {
                return Err(Halt::Trap(
                    "output operation operand is not a compiler-issued required output obligation"
                        .to_owned(),
                ));
            }
            let index = super::required_index(fields, "obligation", "required output obligation")?;
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
                type_name: REQUIRED_OUTPUT_TYPE.into(),
                fields: BTreeMap::from([("obligation".into(), obligation)]),
            })
        }

        fn output_receipt_marker(&mut self, index: usize) -> EvalResult<Value> {
            let index = i64::try_from(index)
                .map_err(|_| Halt::Resource("output receipt index overflowed".to_owned()))?;
            let receipt = self.allocate_cell(Value::Int(index))?;
            Ok(Value::Struct {
                type_symbol: SymbolHandle::invalid(),
                type_name: OUTPUT_RECEIPT_TYPE.into(),
                fields: BTreeMap::from([("receipt".into(), receipt)]),
            })
        }

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

        fn eval_byte_operand(
            &mut self,
            expression: ExpressionHandle,
            frame: &mut Frame,
            context: &str,
        ) -> EvalResult<Vec<u8>> {
            self.eval_byte_data_operand(expression, frame, context)
        }

        pub(crate) fn note_completed_build_output_attempt(&mut self, attempt_index: usize) {
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
                        if let Some(resolution) = attempt.rooted_path_operand_resolutions().first()
                        {
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
                    && matches!(self.output_obligations[index].state, Completed { .. })
                {
                    self.output_obligations[index]
                        .post_completion_mutations
                        .push(attempt_index);
                }
            }
        }
    }
}

pub(crate) mod paths {
    use crate::interpreter::evaluator::filesystem::real::real_fs;
    use crate::interpreter::evaluator::{
        BTreeMap, Cell, EvalResult, Evaluator, ExpressionHandle, FilesystemHostOperation, Frame,
        Halt, SymbolHandle, Value, trap,
    };
    use crate::{
        FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT, FilesystemGrantRootIdentity,
        MAX_INCLUDED_BUILD_SOURCES,
    };

    pub(crate) const ROOTED_BUILD_PATH_TYPE: &str = "$OmegaBuildRootedPath";
    const SOURCE_ROOT_FACET_TYPE: &str = "$OmegaBuildSourceRoot";
    pub(crate) const OUTPUT_ROOT_FACET_TYPE: &str = "$OmegaBuildOutputRoot";
    const INPUT_SLOTS_FACET_TYPE: &str = "$OmegaBuildInputSlots";

    enum BuildPathOperation {
        Resolve(&'static str),
        NamedInput,
        Filesystem {
            facet: &'static str,
            operation: FilesystemHostOperation,
        },
        IncludeSource,
    }

    impl<'program> Evaluator<'program> {
        pub(crate) fn build_prelude_facet_method(
            &self,
            target: SymbolHandle,
        ) -> Option<(&'program str, &'program str)> {
            if !target.is_valid() || !self.symbol_has_build_prelude_source(target) {
                return None;
            }
            let (machine, state) = if let Some(machine) = self.program.machine_by_symbol(target) {
                (machine, self.machine_entry_state(machine)?)
            } else {
                let machine = self.program.machine_holding_state(target)?;
                (machine, self.find_state_by_symbol(machine, target)?)
            };
            if !self.symbol_has_build_prelude_source(machine.symbol)
                || !self.symbol_has_build_prelude_source(state.symbol)
            {
                return None;
            }
            Some((
                machine.attached_data.as_ref()?.as_str(),
                state.name.as_str(),
            ))
        }

        fn build_path_operation(&self, target: SymbolHandle) -> Option<BuildPathOperation> {
            match self.build_prelude_facet_method(target)? {
                ("BuildSource", "resolve") => {
                    Some(BuildPathOperation::Resolve(SOURCE_ROOT_FACET_TYPE))
                }
                ("BuildOutput", "resolve") => {
                    Some(BuildPathOperation::Resolve(OUTPUT_ROOT_FACET_TYPE))
                }
                ("BuildInputs", "get") => Some(BuildPathOperation::NamedInput),
                ("BuildOutput", "include_source") => Some(BuildPathOperation::IncludeSource),
                ("BuildSource", method) => {
                    let operation = match method {
                        "open" => FilesystemHostOperation::Open,
                        "read" => FilesystemHostOperation::Read,
                        "close" => FilesystemHostOperation::Close,
                        _ => return None,
                    };
                    Some(BuildPathOperation::Filesystem {
                        facet: SOURCE_ROOT_FACET_TYPE,
                        operation,
                    })
                }
                ("BuildOutput", method) => {
                    let operation = match method {
                        "create" => FilesystemHostOperation::Create,
                        "write" => FilesystemHostOperation::Write,
                        "close" => FilesystemHostOperation::Close,
                        _ => return None,
                    };
                    Some(BuildPathOperation::Filesystem {
                        facet: OUTPUT_ROOT_FACET_TYPE,
                        operation,
                    })
                }
                _ => None,
            }
        }

        pub(crate) fn try_build_path_call(
            &mut self,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some(operation) = self.build_path_operation(target) else {
                return Ok(None);
            };
            let Some((facet, operands)) = arguments.split_first() else {
                return trap("Build path operation has no explicit activation facet");
            };
            let value = match operation {
                BuildPathOperation::Resolve(expected) => {
                    let root = self.build_root_argument(*facet, expected, frame)?;
                    let [relative] = operands else {
                        return trap("build-root resolution requires one relative path");
                    };
                    let relative = self.build_relative_bytes(*relative, frame)?;
                    validate_build_relative_path(&relative)?;
                    Value::Struct {
                        type_symbol: SymbolHandle::invalid(),
                        type_name: ROOTED_BUILD_PATH_TYPE.into(),
                        fields: BTreeMap::from([
                            (
                                "root".into(),
                                self.allocate_cell(Value::Int(i64::from(root.get())))?,
                            ),
                            (
                                "relative".into(),
                                self.allocate_cell(self.allocate_text(relative)?)?,
                            ),
                        ]),
                    }
                }
                BuildPathOperation::NamedInput => {
                    let facet = self.eval_argument(*facet, frame)?;
                    let facet = self.deref_cell(facet);
                    self.build_named_input(&facet, operands, frame)?
                }
                BuildPathOperation::Filesystem {
                    facet: expected,
                    operation,
                } => self.evaluate_filesystem_operation(operation, |evaluator| {
                    let root = evaluator.build_root_argument(*facet, expected, frame)?;
                    let call = evaluator.prepare_filesystem_call(operation, operands, frame)?;
                    match &call {
                        crate::interpreter::evaluator::PreparedFilesystemCall::Open { .. }
                        | crate::interpreter::evaluator::PreparedFilesystemCall::Create {
                            ..
                        } => {
                            let issued = evaluator
                                .filesystem_operation_attempt_stack
                                .last()
                                .and_then(|index| {
                                    evaluator.filesystem_operation_attempts[*index]
                                        .rooted_path_operand_resolutions
                                        .first()
                                })
                                .map(|resolution| resolution.root());
                            if issued != Some(root) {
                                return trap("Build filesystem path belongs to another facet root");
                            }
                        }
                        crate::interpreter::evaluator::PreparedFilesystemCall::Read {
                            fd, ..
                        }
                        | crate::interpreter::evaluator::PreparedFilesystemCall::Write {
                            fd, ..
                        }
                        | crate::interpreter::evaluator::PreparedFilesystemCall::Close { fd } => {
                            if let Some(issued) = real_fs(evaluator)
                                .and_then(|filesystem| filesystem.descriptor_root(*fd))
                                && issued != root
                            {
                                return trap("Build descriptor belongs to another facet root");
                            }
                        }
                        _ => {}
                    }
                    Ok(call)
                })?,
                BuildPathOperation::IncludeSource => {
                    let root = self.build_root_argument(*facet, OUTPUT_ROOT_FACET_TYPE, frame)?;
                    self.include_build_source(root, operands, frame)?
                }
            };
            Ok(Some(value))
        }

        pub(crate) fn build_root_argument(
            &mut self,
            expression: ExpressionHandle,
            expected: &str,
            frame: &mut Frame,
        ) -> EvalResult<FilesystemGrantRootIdentity> {
            let facet = self.eval_argument(expression, frame)?;
            let facet = self.deref_cell(facet);
            build_root_facet_identity(&facet.borrow(), expected)
        }

        fn build_named_input(
            &mut self,
            facet: &Cell,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            let [name] = arguments else {
                return trap("named build input lookup requires one slot name");
            };
            if !matches!(&*facet.borrow(), Value::Struct { type_name, .. } if &**type_name == INPUT_SLOTS_FACET_TYPE)
            {
                return trap("named build inputs require a compiler-issued facet");
            }
            let name = self.build_relative_bytes(*name, frame)?;
            validate_build_relative_path(&name)?;
            let name = std::str::from_utf8(&name)
                .map_err(|_| Halt::Trap("named build input slot is not UTF-8".into()))?;
            let facet = facet.borrow();
            let Value::Struct { fields, .. } = &*facet else {
                return trap("named build inputs lost their compiler-issued facet");
            };
            let input = fields.get(name).ok_or_else(|| {
                Halt::Trap(format!(
                    "named build input `{name}` is not assigned to this activation"
                ))
            })?;
            let value = input.borrow().clone();
            build_root_facet_identity(&value, SOURCE_ROOT_FACET_TYPE)?;
            Ok(value)
        }

        fn build_relative_bytes(
            &mut self,
            expression: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Vec<u8>> {
            self.eval_byte_data_operand(expression, frame, "build-root path")
        }

        fn include_build_source(
            &mut self,
            output_root: FilesystemGrantRootIdentity,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            let [included] = arguments else {
                return trap("generated-source handoff requires one Output-rooted path");
            };
            let included = self.eval_expression(*included, frame)?;
            let Some((included_root, relative_path)) = rooted_build_path_parts(&included)? else {
                return trap("generated-source handoff requires an Output-rooted path");
            };
            if included_root != output_root {
                return trap("generated-source handoff path belongs to a different build root");
            }
            validate_build_relative_path(&relative_path)?;
            self.real_fs
                .as_ref()
                .ok_or_else(|| {
                    Halt::Trap("generated source requires scoped output custody".into())
                })?
                .require_sealed_rooted_file(output_root, &relative_path)?;
            let scoped_real_output = real_fs(self)
                .is_some_and(crate::interpreter::evaluator::filesystem::real::RealFs::is_scoped);
            if !scoped_real_output {
                return trap("generated-source handoff requires a scoped build-output grant");
            }
            if self.build_included_sources.len() == MAX_INCLUDED_BUILD_SOURCES {
                return Err(Halt::Resource(format!(
                    "generated-source handoff exceeds its {MAX_INCLUDED_BUILD_SOURCES}-source ceiling"
                )));
            }
            if self.build_included_sources.iter().any(|source| {
                source.root() == included_root && source.relative_path() == relative_path
            }) {
                return trap("generated-source handoff names the same path more than once");
            }
            self.build_included_sources
                .push(crate::BuildIncludedSource::new(
                    included_root,
                    relative_path,
                    self.filesystem_operation_attempts.len(),
                ));
            Ok(Value::Unit)
        }

        pub(crate) fn exact_build_facet_method(
            &self,
            expected_attached: &str,
            expected_state: &str,
            target_symbol: SymbolHandle,
        ) -> bool {
            self.program.machines().iter().any(|machine| {
                machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|attached| attached.as_str() == expected_attached)
                    && self.symbol_has_build_prelude_source(machine.symbol)
                    && self.program.machine_states(machine).iter().any(|state| {
                        state.name.as_str() == expected_state
                            && self.symbol_has_build_prelude_source(state.symbol)
                            && (!target_symbol.is_valid() || state.symbol == target_symbol)
                    })
            })
        }

        pub(crate) fn symbol_has_build_prelude_source(&self, symbol: SymbolHandle) -> bool {
            self.program.symbols.is_build_prelude_symbol(symbol)
        }
    }

    fn build_root_facet_identity(
        value: &Value,
        expected: &str,
    ) -> EvalResult<FilesystemGrantRootIdentity> {
        let Value::Struct {
            type_name, fields, ..
        } = value
        else {
            return trap("Build path operation requires a compiler-issued facet");
        };
        if &**type_name != expected {
            return trap("Build path operation received a different activation facet");
        }
        fields
            .get("root")
            .and_then(|root| root.borrow().as_int())
            .and_then(|root| u32::try_from(root).ok())
            .and_then(FilesystemGrantRootIdentity::new)
            .ok_or_else(|| {
                Halt::Trap("build-root activation facet has no valid root identity".into())
            })
    }

    pub(crate) fn rooted_build_path_parts(
        value: &Value,
    ) -> EvalResult<Option<(FilesystemGrantRootIdentity, Vec<u8>)>> {
        let Value::Struct {
            type_name, fields, ..
        } = value
        else {
            return Ok(None);
        };
        if **type_name != *ROOTED_BUILD_PATH_TYPE {
            return Ok(None);
        }
        let root = fields
            .get("root")
            .and_then(|root| root.borrow().as_int())
            .and_then(|root| u32::try_from(root).ok())
            .and_then(FilesystemGrantRootIdentity::new)
            .ok_or_else(|| Halt::Trap("rooted build path has no valid root identity".to_owned()))?;
        let relative = fields
            .get("relative")
            .and_then(|relative| match &*relative.borrow() {
                Value::Str(bytes) => Some(bytes.borrow().to_vec()),
                _ => None,
            })
            .ok_or_else(|| Halt::Trap("rooted build path has no relative bytes".to_owned()))?;
        Ok(Some((root, relative)))
    }

    pub(crate) fn validate_build_relative_path(relative: &[u8]) -> EvalResult<()> {
        if relative.is_empty() {
            return trap("build-root path is empty");
        }
        if relative.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT {
            return Err(Halt::Resource(format!(
                "build-root path exceeds its {FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT}-byte ceiling"
            )));
        }
        if relative.contains(&0) {
            return trap("build-root path contains NUL");
        }
        if std::str::from_utf8(relative).is_err() {
            return trap("build-root path must use canonical UTF-8 components");
        }
        if relative[0] == b'/'
            || relative.contains(&b'\\')
            || (relative.len() >= 2 && relative[1] == b':')
        {
            return trap("build-root path must not use an absolute or host-specific spelling");
        }
        if relative
            .split(|byte| *byte == b'/')
            .any(|component| component.is_empty() || component == b"." || component == b"..")
        {
            return trap("build-root path must use canonical relative components");
        }
        Ok(())
    }
}

pub(crate) mod product_entries {
    use crate::interpreter::evaluator::{
        BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
    };
    use language_semantics::declaration_selection::DescribedProductEntry;
    pub(crate) const PRODUCT_FACET_TYPE: &str = "$OmegaBuildProductFacet";

    const PRODUCT_ENTRY_DESCRIPTION_TYPE: &str = "$OmegaBuildProductEntry";

    impl<'program> Evaluator<'program> {
        pub(crate) fn try_build_product_entry_call(
            &mut self,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
            occurrence: source::SourceSpan,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            if !target.is_valid() || !self.exact_build_facet_method("BuildProduct", "entry", target)
            {
                return Ok(None);
            }
            let [facet, path, slot] = arguments else {
                return Err(Halt::Trap(
                    "product entry selection requires an explicit facet and path and slot"
                        .to_owned(),
                ));
            };
            self.require_product_facet(*facet, frame)?;
            let path = self.eval_byte_data_operand(*path, frame, "product entry operand")?;
            let slot = self.eval_byte_data_operand(*slot, frame, "product entry operand")?;
            self.issue_product_entry_description(&path, &slot, occurrence)
                .map(Some)
        }

        pub(crate) fn require_product_facet(
            &mut self,
            expression: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<()> {
            let facet = self.eval_argument(expression, frame)?;
            let facet = self.deref_cell(facet);
            if !matches!(&*facet.borrow(), Value::Struct { type_name, .. } if **type_name == *PRODUCT_FACET_TYPE)
            {
                return Err(Halt::Trap(
                    "product selection requires the compiler-issued Build.product facet".to_owned(),
                ));
            }
            Ok(())
        }

        pub(crate) fn described_product_entry(
            &self,
            value: &Value,
        ) -> EvalResult<DescribedProductEntry> {
            let index = super::described_product_index(
                value,
                PRODUCT_ENTRY_DESCRIPTION_TYPE,
                "root binding operand is not a compiler-issued product entry description",
                "product entry description",
            )?;
            self.product_entry_descriptions
                .get(index)
                .cloned()
                .ok_or_else(|| {
                    Halt::Trap("product entry description names an unknown description".to_owned())
                })
        }

        fn issue_product_entry_description(
            &mut self,
            path: &[u8],
            slot: &[u8],
            occurrence: source::SourceSpan,
        ) -> EvalResult<Value> {
            let compatibility = self.product_entry_slots.ok_or_else(|| {
                Halt::Trap(
                    "product entry selection requires the build owner's slot compatibility check"
                        .to_owned(),
                )
            })?;
            let selected = validation::product_queries::describe_entry(
                self.product_declarations,
                occurrence,
                path,
                slot,
                compatibility,
            )
            .map_err(Halt::Trap)?;
            let index = self.product_entry_descriptions.len();
            self.product_entry_descriptions
                .try_reserve(1)
                .map_err(|_| {
                    Halt::Resource("product-entry description allocation was refused".to_owned())
                })?;
            self.product_entry_descriptions.push(selected);
            let index = i64::try_from(index).map_err(|_| {
                Halt::Resource("product-entry description index overflowed".to_owned())
            })?;
            let description = self.allocate_cell(Value::Int(index))?;
            Ok(Value::Struct {
                type_symbol: SymbolHandle::invalid(),
                type_name: PRODUCT_ENTRY_DESCRIPTION_TYPE.into(),
                fields: BTreeMap::from([("description".into(), description)]),
            })
        }
    }
}

pub(crate) mod product_providers {
    use crate::interpreter::evaluator::{
        BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
    };
    use validation::product_queries::describe_provider;

    pub(crate) const PRODUCT_PROVIDER_DESCRIPTION_TYPE: &str = "$OmegaBuildProductProvider";

    impl<'program> Evaluator<'program> {
        pub(crate) fn try_build_product_provider_call(
            &mut self,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
            occurrence: source::SourceSpan,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            self.try_build_product_description_call(
                "provider",
                target,
                arguments,
                occurrence,
                frame,
                "product provider selection requires an explicit facet and path",
                |engine, path, frame| {
                    engine.eval_byte_data_operand(path, frame, "product provider operand")
                },
                |engine, path, occurrence| {
                    engine.issue_product_provider_description(path, occurrence)
                },
            )
        }

        pub(crate) fn described_product_provider(&self, value: &Value) -> EvalResult<usize> {
            let index = super::described_product_index(
                value,
                PRODUCT_PROVIDER_DESCRIPTION_TYPE,
                "product-provider inspection is not a compiler-issued product provider description",
                "product provider description",
            )?;
            if index >= self.product_provider_descriptions.len() {
                return Err(Halt::Trap(
                    "product provider description names an unknown description".to_owned(),
                ));
            }
            Ok(index)
        }

        fn issue_product_provider_description(
            &mut self,
            path: &[u8],
            occurrence: source::SourceSpan,
        ) -> EvalResult<Value> {
            let selected = describe_provider(self.product_declarations, occurrence, path)
                .map_err(Halt::Trap)?;
            let index = self.product_provider_descriptions.len();
            self.product_provider_descriptions
                .try_reserve(1)
                .map_err(|_| {
                    Halt::Resource("product-provider description allocation was refused".to_owned())
                })?;
            self.product_provider_descriptions.push(selected);
            let index = i64::try_from(index).map_err(|_| {
                Halt::Resource("product-provider description index overflowed".to_owned())
            })?;
            let description = self.allocate_cell(Value::Int(index))?;
            Ok(Value::Struct {
                type_symbol: SymbolHandle::invalid(),
                type_name: PRODUCT_PROVIDER_DESCRIPTION_TYPE.into(),
                fields: BTreeMap::from([("description".into(), description)]),
            })
        }
    }
}

pub(crate) mod product_schemas {
    use super::product_providers::PRODUCT_PROVIDER_DESCRIPTION_TYPE;
    use crate::interpreter::evaluator::{
        BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
    };
    use validation::product_queries::describe_schema;

    const PRODUCT_SCHEMA_DESCRIPTION_TYPE: &str = "$OmegaBuildProductTypeSchema";

    enum ProductDescriptionKind {
        Schema,
        Provider,
    }

    impl<'program> Evaluator<'program> {
        pub(crate) fn try_product_description_path_call(
            &mut self,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            if !target.is_valid() {
                return Ok(None);
            }
            let kind = if self.exact_build_facet_method("ProductTypeSchema", "path", target) {
                ProductDescriptionKind::Schema
            } else if self.exact_build_facet_method("ProductProviderRef", "path", target) {
                ProductDescriptionKind::Provider
            } else {
                return Ok(None);
            };
            let [description] = arguments else {
                return Err(Halt::Trap(
                    "product-description path access requires one explicit description".to_owned(),
                ));
            };
            let description = self.eval_argument(*description, frame)?;
            let description = self.deref_cell(description);
            let (marker, rejection) = match kind {
                ProductDescriptionKind::Schema => (
                    PRODUCT_SCHEMA_DESCRIPTION_TYPE,
                    "product-schema path access requires a compiler-issued ProductTypeSchema",
                ),
                ProductDescriptionKind::Provider => (
                    PRODUCT_PROVIDER_DESCRIPTION_TYPE,
                    "product-provider path access requires a compiler-issued ProductProviderRef",
                ),
            };
            if !matches!(&*description.borrow(), Value::Struct { type_name, .. } if **type_name == *marker)
            {
                return Err(Halt::Trap(rejection.to_owned()));
            }
            let path = match kind {
                ProductDescriptionKind::Schema => {
                    let index = self.described_product_schema(&description.borrow())?;
                    self.product_schema_descriptions[index]
                        .canonical_path
                        .clone()
                }
                ProductDescriptionKind::Provider => {
                    let index = self.described_product_provider(&description.borrow())?;
                    self.product_provider_descriptions[index]
                        .canonical_path
                        .clone()
                }
            };
            Ok(Some(self.allocate_text(path.into_bytes())?))
        }

        pub(crate) fn try_build_product_schema_call(
            &mut self,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
            occurrence: source::SourceSpan,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            self.try_build_product_description_call(
                "schema",
                target,
                arguments,
                occurrence,
                frame,
                "product schema selection requires an explicit facet and path",
                |engine, path, frame| {
                    engine.eval_byte_data_operand(path, frame, "product schema operand")
                },
                |engine, path, occurrence| {
                    engine.issue_product_schema_description(path, occurrence)
                },
            )
        }

        pub(crate) fn described_product_schema(&self, value: &Value) -> EvalResult<usize> {
            let index = super::described_product_index(
                value,
                PRODUCT_SCHEMA_DESCRIPTION_TYPE,
                "product-schema inspection is not a compiler-issued product schema description",
                "product schema description",
            )?;
            if index >= self.product_schema_descriptions.len() {
                return Err(Halt::Trap(
                    "product schema description names an unknown description".to_owned(),
                ));
            }
            Ok(index)
        }

        fn issue_product_schema_description(
            &mut self,
            path: &[u8],
            occurrence: source::SourceSpan,
        ) -> EvalResult<Value> {
            let selected =
                describe_schema(self.product_declarations, occurrence, path).map_err(Halt::Trap)?;
            let index = self.product_schema_descriptions.len();
            self.product_schema_descriptions
                .try_reserve(1)
                .map_err(|_| {
                    Halt::Resource("product-schema description allocation was refused".to_owned())
                })?;
            self.product_schema_descriptions.push(selected);
            let index = i64::try_from(index).map_err(|_| {
                Halt::Resource("product-schema description index overflowed".to_owned())
            })?;
            let description = self.allocate_cell(Value::Int(index))?;
            Ok(Value::Struct {
                type_symbol: SymbolHandle::invalid(),
                type_name: PRODUCT_SCHEMA_DESCRIPTION_TYPE.into(),
                fields: BTreeMap::from([("description".into(), description)]),
            })
        }
    }
}

pub(crate) mod provider_selections {
    use crate::ExecutedBuildSelection;
    use crate::interpreter::evaluator::{
        Cell, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
        expression_table, trap,
    };
    use language_semantics::declaration_selection::BuildOperation;

    impl<'program> Evaluator<'program> {
        pub(crate) fn try_build_selection_value_call(
            &mut self,
            expression: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some(
                operation @ (BuildOperation::ProviderSelection
                | BuildOperation::AssumedBindingContract
                | BuildOperation::ServiceAudit
                | BuildOperation::TestGroupRegistration),
            ) = BuildOperation::from_call_target(call.target.as_str())
            else {
                return Ok(None);
            };
            if call.target_symbol.is_valid() {
                return Ok(None);
            }
            let arguments = expression_table(self.program).expression_handles(call.arguments);
            let composition_case = match operation {
                BuildOperation::ProviderSelection => {
                    self.provider_composition_case(arguments, frame)?
                }
                BuildOperation::AssumedBindingContract
                | BuildOperation::ServiceAudit
                | BuildOperation::TestGroupRegistration => {
                    let [builder] = arguments else {
                        return trap(format!(
                            "{} registration takes exactly the mutable Build argument",
                            operation.authored_spelling()
                        ));
                    };
                    let builder = self.eval_read_cell(*builder, frame)?;
                    self.require_provider_selection_activation(self.deref_cell(builder))?;
                    SymbolHandle::invalid()
                }
                _ => unreachable!(),
            };
            self.record_build_selection(expression, operation, composition_case, frame)?;
            Ok(Some(Value::Unit))
        }

        fn require_provider_selection_activation(&self, builder: Cell) -> EvalResult<()> {
            if self
                .root_build
                .as_ref()
                .is_some_and(|root| Cell::ptr_eq(&builder, root))
            {
                Ok(())
            } else {
                trap("provider selection requires the current activation's original Build value")
            }
        }

        fn provider_composition_case(
            &mut self,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<SymbolHandle> {
            let Some((builder, arguments)) = arguments.split_first() else {
                return trap("provider selection requires an explicit mutable Build argument");
            };
            let builder = self.eval_read_cell(*builder, frame)?;
            self.require_provider_selection_activation(self.deref_cell(builder))?;
            if arguments.is_empty() {
                return Ok(SymbolHandle::invalid());
            }
            let [argument] = arguments else {
                return trap("provider selection takes at most one CompositionMode value");
            };
            let Value::Enum {
                type_symbol,
                variant_name,
                payload,
            } = self.eval_expression(*argument, frame)?
            else {
                return trap("provider selection requires a compiler-owned CompositionMode case");
            };
            let definition = self.program.data_definitions().iter().find(|definition| {
                definition.symbol == type_symbol
                    && definition.name.as_str() == "CompositionMode"
                    && self.symbol_has_build_prelude_source(type_symbol)
            });
            let case = definition.and_then(|definition| {
                self.program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Variant(variant)
                            if *variant.name.as_str() == *variant_name && payload.is_empty() =>
                        {
                            Some(variant.symbol)
                        }
                        _ => None,
                    })
            });
            case.ok_or_else(|| Halt::Trap("provider selection requires an exact payload-free compiler-owned CompositionMode case".into()))
        }

        fn record_build_selection(
            &mut self,
            expression: ExpressionHandle,
            operation: BuildOperation,
            composition_case: SymbolHandle,
            frame: &Frame,
        ) -> EvalResult<()> {
            self.executed_build_selections.try_reserve(1).map_err(|_| {
                Halt::Resource("provider-selection result allocation was refused".into())
            })?;
            self.executed_build_selections.push(ExecutedBuildSelection {
                operation,
                machine: frame.machine_symbol,
                expression,
                composition_case,
            });
            Ok(())
        }
    }
}

pub(crate) mod root_bindings {
    use crate::interpreter::evaluator::{
        Cell, EvalResult, Evaluator, Frame, Halt, Value, expression_table, trap,
    };
    use declaration_selection::{
        BuildOperation, BuildSelectionRequest, CompositionMode,
        ExecutedBuildSelection as SourceBuildSelection, ExecutedProductSelection,
        RootBindingOperand,
    };
    use declaration_selection::{ExecutedRootBinding, RootBindingSelection};
    use language_semantics::declaration_selection;
    use typed_trees::expression::TableCallExpression;
    use typed_trees::product_operand::capture_static_environment;

    impl<'program> Evaluator<'program> {
        pub(crate) fn source_selection_protocol(
            &self,
            target: symbols::SymbolHandle,
        ) -> Option<&str> {
            if !self.symbol_has_build_prelude_source(target) {
                return None;
            }
            self.program
                .traits()
                .iter()
                .filter(|definition| {
                    definition.is_boundary && definition.name.as_str() == "BuildBindingOperations"
                })
                .flat_map(|definition| self.program.trait_machine_signatures(definition))
                .find(|signature| signature.symbol == target)
                .map(|signature| signature.name.as_str())
        }

        pub(crate) fn try_source_selection_call(
            &mut self,
            call: &TableCallExpression,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let protocol = self.source_selection_protocol(call.target_symbol);
            if !call.build_selection.is_valid() {
                if matches!(
                    protocol,
                    Some(
                        "prepare"
                            | "prepare_tests"
                            | "bind"
                            | "bind_description"
                            | "select"
                            | "select_provider"
                            | "exclude"
                    )
                ) {
                    return trap(
                        "Build binding protocol requires source-issued designated product operands",
                    );
                }
                return Ok(None);
            }
            let binding = expression_table(self.program)
                .build_selection_request(call.build_selection)
                .ok_or_else(|| {
                    Halt::Trap("root binding lost its admitted selection request".to_owned())
                })?
                .clone();
            let operation = match &binding {
                BuildSelectionRequest::ProviderReadiness(_) => {
                    return trap(
                        "provider readiness requires a retained Terminal Build activation",
                    );
                }
                BuildSelectionRequest::Exclusion(_) => "exclude",
                BuildSelectionRequest::Entry(binding) => match binding.implementation {
                    RootBindingOperand::AuthoredPath(_) => "bind",
                    RootBindingOperand::Description => "bind_description",
                },
                BuildSelectionRequest::Declaration(_) => match protocol {
                    Some("select_provider") => "select_provider",
                    _ => "select",
                },
            };
            let requires_selecting_machine = binding.requires_lexical_subject();
            let preparation = protocol.and_then(|name| match name {
                "prepare" | "prepare_tests" => Some(true),
                name if name == operation => Some(false),
                _ => None,
            });
            let Some(preparation) =
                preparation.filter(|_| self.symbol_has_build_prelude_source(call.target_symbol))
            else {
                return trap("root binding lost its toolchain boundary requirement");
            };
            let arguments = expression_table(self.program).expression_handles(call.arguments);
            if preparation {
                let [receiver_expression] = arguments else {
                    return trap("root binding requires one explicit Build argument");
                };
                let facet = protocol == Some("prepare_tests");
                let receiver = self.eval_read_cell(*receiver_expression, frame)?;
                let receiver = self.deref_cell(receiver);
                // Test-group registration is prepared through the original
                // Build's own `tests` facet cell, not a copy of its value.
                let original = match self.root_build.as_ref() {
                    Some(root) if facet => Some(self.field_cell(root, "tests")?),
                    root => root.cloned(),
                };
                if !original.is_some_and(|root| Cell::ptr_eq(&root, &receiver)) {
                    return trap(
                        "root binding requires the current activation's original Build value",
                    );
                }
                let ticket = i64::try_from(self.pending_build_selections.len())
                    .map_err(|_| Halt::Resource("root-binding ticket space exhausted".into()))?;
                self.pending_build_selections.try_reserve(1).map_err(|_| {
                    Halt::Resource("root-binding ticket allocation was refused".into())
                })?;
                self.pending_build_selections.push(call.build_selection);
                return Ok(Some(Value::Int(ticket)));
            }
            let Some(ticket_expression) = arguments.first().copied() else {
                return trap("root binding requires an authenticated activation ticket");
            };
            let Value::Int(ticket) = self.eval_expression(ticket_expression, frame)? else {
                return trap("root binding activation ticket is not an integer");
            };
            let ticket = usize::try_from(ticket)
                .map_err(|_| Halt::Trap("root binding activation ticket is invalid".into()))?;
            let issued = self
                .pending_build_selections
                .get(ticket)
                .and_then(|request| {
                    expression_table(self.program).build_selection_request(*request)
                });
            if issued != Some(&binding) {
                return trap(
                    "root binding activation ticket is missing, consumed, or belongs to another selection",
                );
            }
            let author_coordinate = self
                .program
                .symbols
                .symbol_source_span(self.program.symbols.authored_origin(frame.machine_symbol));
            let selection = match binding {
                BuildSelectionRequest::ProviderReadiness(_) => {
                    return trap(
                        "provider readiness requires a retained Terminal Build activation",
                    );
                }
                BuildSelectionRequest::Exclusion(request) => {
                    let [_, value] = arguments else {
                        return trap("behavior exclusion requires its evaluated code");
                    };
                    let Value::Int(code) = self.eval_expression(*value, frame)? else {
                        return trap("behavior exclusion code is not an integer");
                    };
                    let code = u64::try_from(code)
                        .map_err(|_| Halt::Trap("invalid behavior exclusion code".into()))?;
                    if code >= request.vocabulary.cases().len() as u64 {
                        return trap("behavior exclusion code is outside its vocabulary");
                    }
                    SourceBuildSelection::Exclusion(declaration_selection::ExecutedEnumExclusion {
                        request,
                        code,
                        selecting_machine: author_coordinate.ok_or_else(|| {
                            Halt::Trap(
                                "behavior exclusion lost its authored machine coordinate".into(),
                            )
                        })?,
                    })
                }
                BuildSelectionRequest::Entry(binding) => {
                    let implementation = match binding.implementation {
                        RootBindingOperand::Description => {
                            let [_, description] = arguments else {
                                return trap(
                                    "delegated root binding requires one issued description",
                                );
                            };
                            let operand = self.eval_expression(*description, frame)?;
                            RootBindingSelection::Described(self.described_product_entry(&operand)?)
                        }
                        RootBindingOperand::AuthoredPath(path) => {
                            if arguments.len() != 1 {
                                return trap(
                                    "static root binding has unexpected runtime arguments",
                                );
                            }
                            RootBindingSelection::AuthoredPath(path)
                        }
                    };
                    SourceBuildSelection::Entry(ExecutedRootBinding {
                        source_span: binding.source_span,
                        slot: binding.slot,
                        implementation,
                    })
                }
                BuildSelectionRequest::Declaration(request) => {
                    let (target, independent) = match (operation, arguments) {
                        ("select", [_]) => (None, None),
                        ("select_provider", [_, target, mode]) => {
                            let Value::Int(code) = self.eval_expression(*target, frame)? else {
                                return trap("Product selection requires its checked target value");
                            };
                            let target = u64::try_from(code)
                                .ok()
                                .and_then(declaration_selection::ProductTarget::from_code)
                                .ok_or_else(|| {
                                    Halt::Trap(
                                        "Product selection names an unknown catalog target".into(),
                                    )
                                })?;
                            let Value::Bool(independent) = self.eval_expression(*mode, frame)?
                            else {
                                return trap(
                                    "Product selection requires its checked composition value",
                                );
                            };
                            (Some(target), Some(independent))
                        }
                        _ => return trap("Product selection lost its ordinary arguments"),
                    };
                    let composition = match request.operation {
                        BuildOperation::ProviderSelection => {
                            Some(if independent.unwrap_or(false) {
                                CompositionMode::Independent
                            } else {
                                CompositionMode::Fused
                            })
                        }
                        operation
                            if operation.selects_product_declarations()
                                && operation != BuildOperation::ProviderSelection
                                && independent.is_none() =>
                        {
                            None
                        }
                        _ => {
                            return trap(
                                "Product selection operation disagrees with its ordinary arguments",
                            );
                        }
                    };
                    let selecting_machine = if requires_selecting_machine {
                        Some(author_coordinate.ok_or_else(|| {
                            Halt::Trap(
                                "Product selection lost its authored machine coordinate".into(),
                            )
                        })?)
                    } else {
                        None
                    };
                    SourceBuildSelection::Declaration(ExecutedProductSelection {
                        static_environment: if request
                            .operands
                            .iter()
                            .any(|operand| operand.application.is_some())
                        {
                            capture_static_environment(self.program, frame.machine_symbol)
                                .map_err(Halt::Trap)?
                        } else {
                            None
                        },
                        request,
                        target,
                        composition,
                        selecting_machine,
                    })
                }
            };
            self.executed_source_selections
                .try_reserve(1)
                .map_err(|_| {
                    Halt::Resource("root-binding result allocation was refused".to_owned())
                })?;
            self.pending_build_selections[ticket] = Default::default();
            self.executed_source_selections.push(selection);
            Ok(Some(Value::Unit))
        }
    }
}
