use crate::interpreter::evaluator::{
    Cell, EvalResult, Evaluator, ExpressionHandle, ExpressionNode, Frame, FrameLocal, Halt,
    Machine, Reference, State, StatementNode, SymbolHandle, TableCall, TableTransition,
    TransitionDecision, TransitionGuardNode, TransitionTargetNode, Value,
};
use typed_trees::statement::TransitionExit;

impl<'program> Evaluator<'program> {
    pub(in crate::interpreter::evaluator) fn exec_statement(
        &mut self,
        statement: &StatementNode,
        statement_index: usize,
        frame: &mut Frame,
    ) -> EvalResult<()> {
        self.tick()?;
        match statement {
            StatementNode::Assembly(_) => Ok(()),
            // Assembly facts are compile-time assertions and have no runtime
            // evaluation in either interpreter or native execution.
            StatementNode::AssemblyFact(_) => Ok(()),
            StatementNode::RootBinding(_) => Err(Halt::Trap(
                "root binding requires its executed statement identity".to_owned(),
            )),
            StatementNode::Assignment(assignment) => {
                self.assign(statement_index, assignment.target, assignment.value, frame)
            }
            StatementNode::LocalData(local) => {
                // A `let v = <struct>` or `let v = <owned array>` is a VALUE copy: deep-clone so
                // a later mutation of `v` does not alias the initializer's source. A
                // `Value::Array` is deep-cloned ONLY when the local's declared type is an owned
                // `[T; N]` (FixedArray); a slice `let s = arr[1..3]` (a `&[T]` local) is a shared
                // view and must keep sharing the array's cells. A `Ref` keeps aliasing the
                // referent.
                let value = if local.initial_value.is_valid() {
                    let value = self.eval_expression_at_type(
                        local.initial_value,
                        local.type_reference,
                        frame,
                    )?;
                    let copy_array = matches!(value, Value::Array(_))
                        && self.declared_type_is_fixed_array(local.type_reference);
                    if matches!(value, Value::Struct { .. }) || copy_array {
                        value.deep_clone_with(&|value| self.allocate_cell(value))?
                    } else {
                        value
                    }
                } else {
                    self.default_value_for_type(local.type_reference)?
                };
                // Coerce to the local's declared width + arithmetic domain
                // (decision 17): Wrapping/Exact truncate like the native store,
                // Saturating clamps, Trapping traps, an f32 local rounds to f32.
                let value = self.coerce_scalar_value(value, local.type_reference)?;
                // A `let` introduces a fresh local cell bound by its symbol. A
                // scalar local also RECORDS its declared (primitive, domain) so
                // later arithmetic on the name applies the domain at the
                // operation node.
                let scalar = self
                    .program
                    .primitive_type_reference(local.type_reference)
                    .map(|primitive| {
                        (
                            primitive,
                            self.program
                                .arithmetic_domain_for_type_reference(local.type_reference),
                        )
                    });
                frame.bind(FrameLocal {
                    symbol: local.symbol,
                    cell: self.allocate_cell(value)?,
                    type_reference: local.type_reference,
                    scalar,
                });
                Ok(())
            }
            // Executed in run_state_collect_inner, where the statement handle
            // is known — an executed exclusion call records its coordinate.
            StatementNode::Call(_) => Err(Halt::Trap(
                "call statement requires its executed statement identity".to_owned(),
            )),
            StatementNode::Expression(expression) => {
                let _ = self.eval_expression(*expression, frame)?;
                Ok(())
            }
            StatementNode::Transition(_) => {
                // Handled in run_state_collect.
                Ok(())
            }
        }
    }

    // ---- transitions --------------------------------------------------------

    /// Where an authored crash edge fired: the executing machine and state
    /// by name and the transition's source span, for the crash's message.
    fn crash_site(&self, frame: &Frame, span: source::SourceSpan) -> String {
        format!(
            "machine `{}` state `{}` (source {:?} bytes {}..{})",
            self.program.symbols.name(frame.machine_symbol),
            self.program.symbols.name(frame.state_symbol),
            span.source_id,
            span.span.start,
            span.span.end
        )
    }

    pub(in crate::interpreter::evaluator) fn eval_transition(
        &mut self,
        transition: &TableTransition,
        frame: &mut Frame,
    ) -> EvalResult<
        Option<(
            TransitionDecision<'program>,
            checked_trees::FlowClaimJoinExitKind,
        )>,
    > {
        let holds = match transition.guard {
            TransitionGuardNode::Always => true,
            TransitionGuardNode::When(expression) => {
                self.guard_depth += 1;
                let value = self.eval_expression(expression, frame);
                self.guard_depth -= 1;
                let value = value?;
                value
                    .as_bool()
                    .ok_or_else(|| Halt::Trap("transition guard is not boolean".to_owned()))?
            }
        };
        let (target_handle, kind) = if holds {
            (
                transition.target,
                checked_trees::FlowClaimJoinExitKind::TransitionTarget,
            )
        } else if transition.continuation.is_valid() {
            (
                transition.continuation,
                checked_trees::FlowClaimJoinExitKind::TransitionContinuation,
            )
        } else {
            return Ok(None);
        };

        // A crash and an ordinary return may both have a terminal target.
        // Preserve the exit kind before dispatching that target; otherwise a
        // failed runtime assertion silently becomes successful completion.
        if holds {
            match transition.exit {
                TransitionExit::Ordinary => {}
                TransitionExit::Crash(cause) => {
                    return Err(Halt::Crash(
                        cause,
                        self.crash_site(frame, transition.source_span),
                    ));
                }
            }
        }

        let target = self
            .program
            .statement_table
            .transition_target(target_handle)
            .clone();
        let decision = self.resolve_transition_target(&target, frame)?;
        Ok(Some((decision, kind)))
    }

    fn resolve_transition_target(
        &mut self,
        target: &TransitionTargetNode,
        frame: &mut Frame,
    ) -> EvalResult<TransitionDecision<'program>> {
        match target {
            TransitionTargetNode::Terminal => Ok(TransitionDecision::Terminal),
            TransitionTargetNode::Value(expression) => {
                let value = self.eval_expression_at_type(*expression, frame.return_type, frame)?;
                Ok(TransitionDecision::Value(value))
            }
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let members = self.program.statement_table.name_path_members(path.members);
                let (name, receiver_members) = members
                    .split_last()
                    .ok_or_else(|| Halt::Unsupported("empty named transition".to_owned()))?;
                let state_name = name.as_str();
                // This runs after guard selection. Provider dispatch owns operand
                // evaluation and returns the same value an ordinary call produces;
                // the enclosing activation then performs its normal completion.
                // The head is the receiver identity only for a one-component
                // receiver path. Nested paths retain the root and callable, but
                // no receiver leaf symbol; they cannot select an adapter by root.
                let receiver_symbol = if receiver_members.len() == 1 {
                    path.head_symbol
                } else {
                    SymbolHandle::invalid()
                };
                if let Some(value) = self.try_selected_named_call(
                    receiver_symbol,
                    path.symbol,
                    &[],
                    self.program.statement_table.expression_handles(*arguments),
                    frame,
                )? {
                    return Ok(TransitionDecision::Value(value));
                }

                if let Some(value) = self.try_host_invocation(
                    path.symbol,
                    state_name,
                    self.program.statement_table.expression_handles(*arguments),
                    frame,
                )? {
                    return Ok(TransitionDecision::Value(value));
                }

                // Resolution already distinguishes an internal state from a
                // machine invocation. Follow that identity: a machine names
                // its entry, including self-reentry and foreign tail calls.
                // Name fallback could execute a colliding sibling instead.
                let (machine, state) =
                    self.resolve_entry_state_symbol(path.symbol)
                        .ok_or_else(|| {
                            Halt::Unsupported(format!(
                                "transition target `{state_name}` has no resolved machine or state"
                            ))
                        })?;

                let arguments = self.eval_state_arguments(
                    state,
                    self.program.statement_table.expression_handles(*arguments),
                    frame,
                )?;

                Ok(TransitionDecision::Named {
                    state,
                    machine,
                    arguments,
                })
            }
        }
    }

    // ---- calls --------------------------------------------------------------

    pub(in crate::interpreter::evaluator) fn eval_call_statement(
        &mut self,
        statement: typed_trees::statement::StatementHandle,
        call: &TableCall,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        if let Some(value) = self.try_selected_named_call(
            call.receiver_symbol,
            call.target_symbol,
            &call.machine_arguments,
            self.program
                .statement_table
                .expression_handles(call.arguments),
            frame,
        )? {
            return Ok(value);
        }

        // Asm intrinsic statement (`asm { hlt }`): the tree-walker cannot model
        // halting the CPU, but `hlt` in an idle loop is observably a no-op step
        // (the loop simply proceeds), so evaluate it as unit. Memory fences,
        // serialization barriers and scheduling hints are also no-ops in the
        // single-threaded tree walker: its evaluation order is already total.
        // CLI/STI cannot change an interrupt source
        // the interpreter does not model, so they are unit steps as well;
        // `wbinvd`/`invd`/`wbnoinvd` act on caches the tree-walker's
        // single-threaded memory model does not distinguish, so they are unit
        // steps too.
        // Port I/O (`asm#port_out`) has real device effects the interpreter
        // cannot reproduce and stays unsupported.
        if call.target.as_str() == "asm#hlt"
            || call.target.as_str() == "asm#popfq"
            || language_core::inline_assembly::AsmFenceKind::from_intrinsic_name(
                call.target.as_str(),
            )
            .is_some()
            || language_core::inline_assembly::AsmInterruptControlKind::from_intrinsic_name(
                call.target.as_str(),
            )
            .is_some()
            || language_core::inline_assembly::AsmInstructionSerializationKind::from_intrinsic_name(
                call.target.as_str(),
            )
            .is_some()
            || language_core::inline_assembly::AsmSchedulingHintKind::from_intrinsic_name(
                call.target.as_str(),
            )
            .is_some()
            || language_core::inline_assembly::AsmCacheOperationKind::from_intrinsic_name(
                call.target.as_str(),
            )
            .is_some()
        {
            return Ok(Value::Unit);
        }
        if let Some(value) = self.try_component_assumption_acceptance(
            call.target_symbol,
            self.program
                .statement_table
                .expression_handles(call.arguments),
            frame,
        )? {
            return Ok(value);
        }
        // Exclusions include exact static Build calls: their authority is an
        // explicit argument, so namespace classification cannot bypass them.
        if let Some(value) = self.try_behavior_exclusion_statement(statement, call, frame)? {
            return Ok(value);
        }
        // Discarding an entry description still performs the exact query;
        // its explicit facet cannot be bypassed by namespace classification.
        if self
            .try_build_product_entry_call(
                call.target_symbol,
                self.program
                    .statement_table
                    .expression_handles(call.arguments),
                call.source_span,
                frame,
            )?
            .is_some()
        {
            return Ok(Value::Unit);
        }
        if self
            .try_build_product_schema_call(
                call.target_symbol,
                self.program
                    .statement_table
                    .expression_handles(call.arguments),
                call.source_span,
                frame,
            )?
            .is_some()
        {
            return Ok(Value::Unit);
        }
        if self
            .try_build_product_provider_call(
                call.target_symbol,
                self.program
                    .statement_table
                    .expression_handles(call.arguments),
                call.source_span,
                frame,
            )?
            .is_some()
        {
            return Ok(Value::Unit);
        }
        if self
            .try_product_description_path_call(
                call.target_symbol,
                self.program
                    .statement_table
                    .expression_handles(call.arguments),
                frame,
            )?
            .is_some()
        {
            return Ok(Value::Unit);
        }
        if let Some(value) = self.try_build_path_call(
            call.target_symbol,
            self.program
                .statement_table
                .expression_handles(call.arguments),
            frame,
        )? {
            return Ok(value);
        }
        // `BuildOutput::require(&mut builder.output, name);` still issues the
        // obligation when the marker result is dropped, and `complete`/`fail`
        // still settle their operand obligation. Both take the facet as an
        // explicit operand, so neither depends on a receiver place.
        if self
            .try_build_output_obligation_call(
                call.target_symbol,
                self.program
                    .statement_table
                    .expression_handles(call.arguments),
                frame,
            )?
            .is_some()
        {
            return Ok(Value::Unit);
        }
        if self
            .try_required_output_path_call(
                call.target_symbol,
                self.program
                    .statement_table
                    .expression_handles(call.arguments),
                frame,
            )?
            .is_some()
        {
            return Ok(Value::Unit);
        }
        // Build logging takes the activation's log facet as an explicit
        // operand, so it is selected here by resolved toolchain identity
        // beside the other facet families rather than by receiver shape.
        if self
            .try_build_log_write_line_call(
                call.target_symbol,
                self.program
                    .statement_table
                    .expression_handles(call.arguments),
                frame,
            )?
            .is_some()
        {
            return Ok(Value::Unit);
        }
        // Host boundary call? (e.g. self.console.exit_process(70))
        if let Some(value) = self.try_host_call(call, frame)? {
            return Ok(value);
        }

        let (machine, state) =
            self.resolve_static_call(call.target_symbol, call.target.as_str())?;

        let arguments = self.eval_state_arguments(
            state,
            self.program
                .statement_table
                .expression_handles(call.arguments),
            frame,
        )?;

        self.run_state_collect(machine, state, arguments)
            .map(|value| value.unwrap_or(Value::Unit))
    }

    /// Checked target identity selects the declaration or state. Qualification
    /// contributes no storage; all runtime operands are explicit arguments.
    pub(in crate::interpreter::evaluator) fn resolve_entry_state_symbol(
        &self,
        target_symbol: SymbolHandle,
    ) -> Option<(&'program Machine, &'program State)> {
        if !target_symbol.is_valid() {
            return None;
        }
        let (machine, state) = if let Some(machine) = self.program.machine_by_symbol(target_symbol)
        {
            (machine, self.machine_entry_state(machine)?)
        } else {
            let machine = self.program.machine_holding_state(target_symbol)?;
            (machine, self.find_state_by_symbol(machine, target_symbol)?)
        };
        Some((machine, state))
    }

    /// Missing checked selection is malformed input, not a request to search
    /// receiver values, sibling spellings, or unrelated machine declarations.
    pub(in crate::interpreter::evaluator) fn resolve_static_call(
        &self,
        target_symbol: SymbolHandle,
        target: &str,
    ) -> EvalResult<(&'program Machine, &'program State)> {
        self.resolve_entry_state_symbol(target_symbol)
            .ok_or_else(|| {
                Halt::Unsupported(format!("call target `{target}` has no resolved state"))
            })
    }

    /// Declaration namespaces are resolved symbols, not runtime places or
    /// strings to look up again. Both statement and value calls use this
    /// classification before attempting storage lookup.
    pub(in crate::interpreter::evaluator) fn is_declaration_namespace(
        &self,
        symbol: SymbolHandle,
    ) -> bool {
        symbol.is_valid()
            && matches!(
                self.program.symbols.get(symbol).kind,
                symbols::SymbolKind::BuiltinType
                    | symbols::SymbolKind::Data
                    | symbols::SymbolKind::Domain
                    | symbols::SymbolKind::Machine
                    | symbols::SymbolKind::Module
                    | symbols::SymbolKind::Trait
                    | symbols::SymbolKind::ConformanceParameter
            )
    }

    pub(in crate::interpreter::evaluator) fn current_machine(
        &self,
        frame: &Frame,
    ) -> Option<&'program Machine> {
        if !frame.machine_symbol.is_valid() {
            return None;
        }
        self.program.machine(frame.machine_symbol)
    }

    pub(in crate::interpreter::evaluator) fn machine_entry_state(
        &self,
        machine: &Machine,
    ) -> Option<&'program State> {
        // A free helper machine `Main::bump` exposes its body as a state. Prefer a state
        // whose name matches the machine's leaf (`bump`); else the first state.
        let leaf = machine.name.as_str().rsplit("::").next().unwrap_or("");
        if let Some(state) = self.find_state(machine, leaf) {
            return Some(state);
        }
        self.program.machine_states(machine).first()
    }

    /// Evaluate an argument. A `Mutable(place)` or a direct place under a `&mut` param
    /// yields a `Ref` that ALIASES the original cell; a value argument yields a fresh
    /// cell holding a copy.
    pub(in crate::interpreter::evaluator) fn eval_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Cell> {
        match self.program.expression_table.expression(argument) {
            ExpressionNode::Indexed(indexed)
                if matches!(
                    self.program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) =>
            {
                // A view is already the argument value. Probing its place and
                // then evaluating it again would replay effectful selectors.
                let value = self.eval_expression(argument, frame)?;
                self.allocate_cell(value)
            }
            ExpressionNode::Borrow(inner) => {
                // &mut place -> a Ref to the SAME cell (the whole point of the oracle). The
                // param binding holds a `Ref`, so a later forward of that param (as a bare
                // name) can detect it is a reference and keep aliasing -- otherwise a
                // mutable text carrier passed down a call chain detaches after the first hop.
                let cell = self.resolve_place(inner.target, frame)?;
                // A RE-BORROW (`&mut t` where `t` is itself a `&mut` param)
                // aliases the SAME target: forward the inner Ref instead of
                // nesting Ref-to-Ref, which downstream single-level derefs
                // (receiver method resolution) cannot see through -- the
                // param-forwarding chain declined with "unknown value-call
                // target" while the native build served it (2026-07-11l).
                let target = match &*cell.borrow() {
                    Value::Ref(target) => target.clone(),
                    _ => Reference::new(cell.clone()),
                };
                self.allocate_cell(Value::Ref(target))
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                // Select once: a speculative place lookup followed by value
                // evaluation would replay effectful indices. Retain any Ref
                // wrapper so forwarding aliases the original referent.
                let cell = self.eval_read_cell(argument, frame)?;
                let value = cell.borrow().clone();
                self.allocate_cell(value)
            }
            _ => {
                let value = self.eval_expression(argument, frame)?;
                self.allocate_cell(value)
            }
        }
    }
}
