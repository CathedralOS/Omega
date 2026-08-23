use super::*;

impl<'program> Evaluator<'program> {
    pub(super) fn exec_statement(
        &mut self,
        statement: &StatementNode,
        frame: &Frame,
    ) -> EvalResult<()> {
        self.tick()?;
        match statement {
            // Assembly facts are compile-time assertions and have no runtime
            // evaluation in either interpreter or native execution.
            StatementNode::AssemblyFact(_) => Ok(()),
            StatementNode::Assignment(assignment) => {
                // Atomic RMW source syntax is carried as an opaque expression so
                // native instruction selection can replace the whole assignment
                // with one instruction. The interpreter executes serially, but it
                // must preserve the same observable contract: the result local is
                // the value observed by that RMW, not a separate earlier read.
                // Seed the compiler-authored result place from the target before
                // evaluating the arithmetic-shaped single-threaded model.
                if let ExpressionNode::Atomic(atomic) = self
                    .program
                    .expression_table
                    .expression(assignment.value)
                    .clone()
                    && matches!(
                        atomic.ordering,
                        psi_language_core::AtomicOrderingPlan::ReadModifyWrite(_)
                            | psi_language_core::AtomicOrderingPlan::Swap(_)
                            | psi_language_core::AtomicOrderingPlan::CompareExchange { .. }
                    )
                {
                    if !atomic.result.is_valid() {
                        return Err(Halt::Trap(
                            "atomic RMW carrier lost its result place".to_owned(),
                        ));
                    }
                    let target = self.resolve_place(assignment.target, frame)?;
                    let target = self.deref_cell(target);
                    let prior = target.borrow().clone();
                    let result = self.resolve_place(atomic.result, frame)?;
                    let result = self.deref_cell(result);
                    *result.borrow_mut() = prior;
                }
                // A STRUCT, or a whole owned ARRAY, assignment is a VALUE copy: deep-clone so
                // mutating the destination later does not alias the source (`self.f =
                // self.arr[1]; self.f.x = 50` must not touch arr[1]; `self.b = self.a;
                // self.b[0] = 9` must not touch a). A `Value::Array` is deep-cloned ONLY when the
                // TARGET's declared type is an owned `[T; N]` (FixedArray) -- a slice `&[T]`
                // target is a shared view whose writes MUST alias the backing array, so it stays
                // shared. `Ref` is likewise left shared for `&mut` write-through.
                let value = self.eval_expression(assignment.value, frame)?;
                let copy_array = matches!(value, Value::Array(_))
                    && self
                        .assignment_target_type_reference(assignment.target, frame)
                        .map(|target| self.declared_type_is_fixed_array(target))
                        .unwrap_or(false);
                let value = if matches!(value, Value::Struct { .. }) || copy_array {
                    value.deep_clone()
                } else {
                    value
                };
                // Apply the target field's declared width AND arithmetic domain
                // (decision 17), matching the native store: Exact/Wrapping truncate
                // to the field's low bytes (a u16 field assigned 70000 reads back
                // 4464), Saturating clamps to the type range (a u8 Saturating field
                // assigned a folded 10000 reads back 255, not the wrapped 16), and
                // Trapping halts on overflow. Mirrors the LocalData store below.
                // Coerce the stored SCALAR to the target's declared width +
                // arithmetic domain, matching the native store -- for a FIELD from
                // its type, for an ARRAY ELEMENT `arr[i]` from the element width +
                // the array's domain (`[u8;N]` given `a+b`=300 reads 44,
                // `[u8;N] in Saturating` clamps to 255). Integers truncate/clamp/
                // trap (decision 17); an f32 target rounds to f32 (native keeps f32
                // in the slot). Mirrors the LocalData store below.
                if self.write_mutable_record_recast_target(
                    assignment.target,
                    frame,
                    value.clone(),
                )? {
                    return Ok(());
                }
                let value = if let Some(recast) =
                    self.mutable_scalar_recast_target(assignment.target, frame)
                {
                    // The write is stated in the VIEW type, then lands in the
                    // backing scalar cell or byte region as the identical bit
                    // pattern. Validation proves the complete footprint.
                    let target = recast.target().ok_or_else(|| {
                        Halt::Trap("record recast reached the scalar write seam".to_owned())
                    })?;
                    let value = self.coerce_scalar_with(value, target, ArithmeticDomain::Exact)?;
                    match recast {
                        MutableScalarRecast::Direct { source, .. } => {
                            self.eval_recast(value, Some(source))?
                        }
                        MutableScalarRecast::ByteRegion {
                            cells,
                            offset,
                            target,
                        } => {
                            self.write_scalar_byte_region(&cells, offset, target, value)?;
                            return Ok(());
                        }
                        MutableScalarRecast::AggregateByteRegion { .. }
                        | MutableScalarRecast::AggregateTyped { .. } => {
                            return trap("aggregate recast reached the scalar write seam");
                        }
                    }
                } else {
                    match self.assignment_target_coercion(assignment.target, frame) {
                        Some((primitive, domain)) => {
                            self.coerce_scalar_with(value, primitive, domain)?
                        }
                        None => value,
                    }
                };
                // Carrier byte WRITE: `out[i] = ch` where `out` is text (`Value::Str`, packed
                // BYTES). The byte has no per-element cell, so write it straight into the vec
                // rather than resolving an element place (element_cell only handles Array). The
                // value is the byte (an Int); a range index is not a scalar write.
                if let ExpressionNode::Indexed(indexed) = self
                    .program
                    .expression_table
                    .expression(assignment.target)
                    .clone()
                    && !matches!(
                        self.program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    )
                    && let Ok(collection_cell) = self.resolve_place(indexed.collection, frame)
                {
                    let collection_cell = self.deref_cell(collection_cell);
                    if matches!(&*collection_cell.borrow(), Value::Str(_)) {
                        let index = self.eval_index(indexed.index, frame)?;
                        let byte = value.as_int().ok_or_else(|| {
                            Halt::Trap("carrier byte write value is not an integer".to_owned())
                        })? as u8;
                        if let Value::Str(text) = &*collection_cell.borrow() {
                            let mut bytes = text.borrow_mut();
                            match bytes.get_mut(index) {
                                Some(slot) => *slot = byte,
                                None => {
                                    return Err(Halt::Trap(format!(
                                        "carrier byte write index {index} out of bounds (len {})",
                                        bytes.len()
                                    )));
                                }
                            }
                        }
                        return Ok(());
                    }
                }
                let target = self.resolve_place(assignment.target, frame)?;
                // Assigning to a `&mut` place writes THROUGH the reference into the aliased
                // cell (so assigning through a mutable text-carrier parameter mutates the
                // caller's carrier), rather than rebinding the local to a non-reference value.
                let target = self.deref_cell(target);
                *target.borrow_mut() = value;
                Ok(())
            }
            StatementNode::LocalData(local) => {
                // A mutable scalar recast is an ALIAS, not a snapshot. Preserve
                // the source cell and remember the two scalar interpretations;
                // eval_name performs source -> view reads, while assignment
                // performs view -> source writes before touching the cell.
                if local.initial_value.is_valid() {
                    if let Some((source, recast)) =
                        self.mutable_scalar_recast_initializer(local.initial_value, frame)?
                    {
                        frame.bind(local.name.as_str(), Value::Ref(source).cell());
                        frame.bind_type(local.name.as_str(), local.type_reference);
                        frame
                            .mutable_scalar_recasts
                            .borrow_mut()
                            .insert(local.name.as_str().to_owned(), recast);
                        return Ok(());
                    }
                }
                // A `let v = <struct>` or `let v = <owned array>` is a VALUE copy: deep-clone so
                // a later mutation of `v` does not alias the initializer's source. A
                // `Value::Array` is deep-cloned ONLY when the local's declared type is an owned
                // `[T; N]` (FixedArray); a slice `let s = arr[1..3]` (a `&[T]` local) is a shared
                // view and must keep sharing the array's cells. A `Ref` keeps aliasing the
                // referent.
                let value = if local.initial_value.is_valid() {
                    let value = self.eval_expression(local.initial_value, frame)?;
                    let copy_array = matches!(value, Value::Array(_))
                        && self.declared_type_is_fixed_array(local.type_reference);
                    if matches!(value, Value::Struct { .. }) || copy_array {
                        value.deep_clone()
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
                // A `let` introduces a fresh local cell, bound through the frame's
                // interior-mutable locals map. A scalar local also RECORDS its
                // declared (primitive, domain) so later arithmetic on the name
                // applies the domain at the operation node.
                if let Some(primitive) = self.program.primitive_type_reference(local.type_reference)
                {
                    let domain = self
                        .program
                        .arithmetic_domain_for_type_reference(local.type_reference);
                    frame
                        .scalar_locals
                        .borrow_mut()
                        .insert(local.name.as_str().to_owned(), (primitive, domain));
                }
                frame.bind(local.name.as_str(), value.cell());
                frame.bind_type(local.name.as_str(), local.type_reference);
                Ok(())
            }
            StatementNode::Call(call) => {
                self.eval_call_statement(call, frame)?;
                Ok(())
            }
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

    pub(super) fn eval_transition(
        &mut self,
        transition: &TableTransition,
        frame: &Frame,
    ) -> EvalResult<Option<TransitionDecision>> {
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
        if !holds {
            return Ok(None);
        }

        let target = self
            .program
            .statement_table
            .transition_target(transition.target)
            .clone();
        let decision = self.resolve_transition_target(&target, frame)?;
        Ok(Some(decision))
    }

    fn resolve_transition_target(
        &mut self,
        target: &TransitionTargetNode,
        frame: &Frame,
    ) -> EvalResult<TransitionDecision> {
        match target {
            TransitionTargetNode::Terminal => Ok(TransitionDecision::Terminal),
            TransitionTargetNode::SelfTarget => Ok(TransitionDecision::SelfTarget),
            TransitionTargetNode::Value(expression) => {
                let value = self.eval_expression(*expression, frame)?;
                Ok(TransitionDecision::Value(value))
            }
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let members = self.program.statement_table.name_path_members(path.members);
                let state_name = members
                    .last()
                    .map(|name| name.as_str().to_owned())
                    .ok_or_else(|| Halt::Unsupported("empty named transition".to_owned()))?;

                // Same-machine sibling state on the current `self`, or a FREE
                // machine's self-recursion (`-> count(...)` inside top-level
                // `machine count` names the MACHINE, whose body state is the
                // generated `entry`).
                let (machine, state_name) = match self.machine_of_state_named(&state_name, frame) {
                    Some(machine) => (machine, state_name),
                    None => self
                        .free_machine_self_recursion_target(&state_name, frame)
                        .ok_or_else(|| {
                            Halt::Unsupported(format!(
                                "transition target `{state_name}` not found in current machine"
                            ))
                        })?,
                };

                let mut args = Vec::new();
                for argument in self.program.statement_table.expression_handles(*arguments) {
                    args.push(self.eval_state_argument(*argument, frame)?);
                }

                Ok(TransitionDecision::Named {
                    state_name,
                    machine,
                    instance: Rc::clone(&frame.self_cell),
                    args,
                })
            }
        }
    }

    /// A FREE machine's self-recursive transition target: the named target is the
    /// CURRENT machine's own (leaf) name and the machine has no attached data, so
    /// the recursion re-enters the machine's entry state (the generated `entry`)
    /// with the transition's arguments.
    fn free_machine_self_recursion_target(
        &self,
        state_name: &str,
        frame: &Frame,
    ) -> Option<(Machine, String)> {
        let machine = self.current_machine(frame)?;
        let leaf = machine.name.as_str().rsplit("::").next().unwrap_or("");
        if machine.attached_data.is_some() || leaf != state_name {
            return None;
        }
        let entry = self.machine_entry_state_name(machine)?;
        Some((machine.clone(), entry))
    }

    /// Find the machine that owns a sibling state of `self` by state name. The entry and
    /// its sub-states all live in the same machine group; a named transition stays within
    /// the current machine.
    fn machine_of_state_named(&self, state_name: &str, frame: &Frame) -> Option<Machine> {
        // A named transition target is a SIBLING state of the machine currently executing, so
        // resolve within the CURRENT machine FIRST. Otherwise a state name shared across machines
        // -- e.g. `Picker::pick` and `Main::read_at` BOTH having a `try1` sub-state -- collides on
        // the type/global fallbacks below and runs the WRONG machine's body (the read_at `try1`
        // transition would run pick's `try1`, returning pick's value).
        if let Some(machine) = self.current_machine(frame) {
            if self.find_state(machine, state_name).is_some() {
                return Some(machine.clone());
            }
        }
        let type_symbol = match &*frame.self_cell.borrow() {
            Value::Struct { type_symbol, .. } => *type_symbol,
            _ => SymbolHandle::invalid(),
        };
        // First, the machine whose symbol matches the instance and has the state.
        for machine in self.program.machines() {
            if machine.symbol == type_symbol && self.find_state(machine, state_name).is_some() {
                return Some(machine.clone());
            }
        }
        // Fall back: any machine that defines a state of that name (single-machine
        // programs share one instance shape).
        self.program
            .machines()
            .iter()
            .find(|machine| self.find_state(machine, state_name).is_some())
            .cloned()
    }

    // ---- calls --------------------------------------------------------------

    fn eval_call_statement(&mut self, call: &TableCall, frame: &Frame) -> EvalResult<Value> {
        // Asm intrinsic statement (`asm { hlt }`): the tree-walker cannot model
        // halting the CPU, but `hlt` in an idle loop is observably a no-op step
        // (the loop simply proceeds), so evaluate it as unit. Memory fences
        // are also no-ops in the single-threaded tree walker: its evaluation
        // order is already total. CLI/STI cannot change an interrupt source
        // the interpreter does not model, so they are unit steps as well.
        // Port I/O (`asm#port_out`) has real device effects the interpreter
        // cannot reproduce and stays unsupported.
        if call.target.as_str() == "asm#hlt"
            || call.target.as_str() == "asm#popfq"
            || psi_language_core::inline_assembly::AsmFenceKind::from_intrinsic_name(
                call.target.as_str(),
            )
            .is_some()
            || psi_language_core::inline_assembly::AsmInterruptControlKind::from_intrinsic_name(
                call.target.as_str(),
            )
            .is_some()
        {
            return Ok(Value::Unit);
        }
        // CH10 root grant (GR3): `b.accept_boundary<path>();` desugars to
        // the `accept_boundary#<path>` marker call. Grants are DECLARATIONS
        // harvested statically by the build-config pass; evaluation serves
        // the marker as a no-op so the build machine runs through it.
        if call.target.as_str().starts_with("accept_boundary#")
            || call.target.as_str().starts_with("select_provider#")
            || call.target.as_str().starts_with("wire_compatibility#")
            || call.target.as_str().starts_with("bind_root#")
        {
            return Ok(Value::Unit);
        }

        // Host boundary call? (e.g. self.console.exit_process(70))
        if let Some(value) = self.try_host_call(call, frame)? {
            return Ok(value);
        }

        // The synthesized wire encoder (chapter 20, wire stage 2a)?
        if let Some(value) = self.try_wire_encode_call(call, frame)? {
            return Ok(value);
        }

        // The synthesized wire decoder (chapter 20, wire stage 2b)?
        if let Some(value) = self.try_wire_decode_call(call, frame)? {
            return Ok(value);
        }

        let target = call.target.as_str();
        let (machine, state_name, instance) = if call.receiver.is_empty() {
            self.resolve_entry_state_symbol(call.target_symbol, frame)
                .map_or_else(|| self.resolve_state_call(call.receiver, target, frame), Ok)?
        } else {
            self.resolve_state_call(call.receiver, target, frame)?
        };

        let mut args = Vec::new();
        for argument in self
            .program
            .statement_table
            .expression_handles(call.arguments)
        {
            args.push(self.eval_state_argument(*argument, frame)?);
        }

        self.run_state_collect(&machine, &state_name, instance, args)
            .map(|value| value.unwrap_or(Value::Unit))
    }

    /// Resolve a receiverless direct call through its exact entry-state
    /// symbol. Selected checked adapters and static-machine specialization
    /// retain this identity specifically so attached/plural realizations do
    /// not fall back to ambiguous display-name lookup.
    pub(super) fn resolve_entry_state_symbol(
        &self,
        target_symbol: SymbolHandle,
        frame: &Frame,
    ) -> Option<(Machine, String, Cell)> {
        if !target_symbol.is_valid() {
            return None;
        }
        self.program.machines().iter().find_map(|machine| {
            self.program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == target_symbol)
                .map(|state| {
                    (
                        machine.clone(),
                        state.name.as_str().to_owned(),
                        Rc::clone(&frame.self_cell),
                    )
                })
        })
    }

    /// Resolve a call target -- a state name with an optional receiver path -- to the
    /// (machine, state, instance) it runs against. Priority:
    /// 1. An explicit receiver path naming a CONTAINED sub-machine instance field whose
    ///    type defines the target state (`self.dungeon.foo()`): run on that sub-instance.
    /// 2. A SIBLING state of the current machine (`self.foo()` where `foo` is a state of
    ///    the machine currently executing): run that state on the same `self`.
    /// 3. A free helper machine named `<group>::<target>` or any machine with that state:
    ///    run its entry state on the current `self`.
    fn resolve_state_call(
        &self,
        receiver: psi_arena::HandleSpan<psi_typed_trees::name::Identifier>,
        target: &str,
        frame: &Frame,
    ) -> EvalResult<(Machine, String, Cell)> {
        // (1) Explicit receiver path to a contained sub-machine instance.
        if let Some(resolved) = self.resolve_receiver_state_call(receiver, target, frame)? {
            return Ok(resolved);
        }

        // (2) Sibling state of the current machine.
        if let Some(machine) = self.current_machine(frame) {
            if self.find_state(machine, target).is_some() {
                return Ok((
                    machine.clone(),
                    target.to_owned(),
                    Rc::clone(&frame.self_cell),
                ));
            }
        }

        // (3) A free helper machine.
        let machine = self
            .find_machine_for_call(target, frame)
            .ok_or_else(|| Halt::Unsupported(format!("unknown call target `{target}`")))?;
        let entry_state = self
            .machine_entry_state_name(&machine)
            .ok_or_else(|| Halt::Unsupported(format!("call target `{target}` has no state")))?;
        Ok((machine, entry_state, Rc::clone(&frame.self_cell)))
    }

    /// If the call has a receiver path that resolves (relative to `self`) to a CONTAINED
    /// sub-machine instance whose machine defines the target state, return that instance and
    /// machine. The receiver path's leaf is the field; the head may be `self`.
    fn resolve_receiver_state_call(
        &self,
        receiver: psi_arena::HandleSpan<psi_typed_trees::name::Identifier>,
        target: &str,
        frame: &Frame,
    ) -> EvalResult<Option<(Machine, String, Cell)>> {
        let members: Vec<String> = self
            .program
            .statement_table
            .name_path_members(receiver)
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        if members.is_empty() {
            return Ok(None);
        }

        // Walk the receiver path to a cell, starting at `self` (an implicit-self leaf like
        // `console` is a single-member path; `self.dungeon` is `[self, dungeon]`).
        let mut cell = Rc::clone(&frame.self_cell);
        let mut start = 0;
        if members[0] == "self" {
            start = 1;
        } else if let Some(local) = frame.get(&members[0]) {
            cell = local;
            start = 1;
        }
        for member in &members[start..] {
            cell = self.deref_cell(cell);
            match self.field_cell(&cell, member) {
                Ok(next) => cell = next,
                Err(_) => return Ok(None),
            }
        }
        cell = self.deref_cell(cell);

        // Only treat this as a sub-machine call if the receiver is NOT just `self` (a bare
        // self receiver is handled by the sibling-state path).
        let bare_self = members.len() == 1 && members[0] == "self";
        if bare_self {
            return Ok(None);
        }
        Ok(self
            .machine_for_instance_state(&cell, target)
            .map(|machine| (machine, target.to_owned(), cell)))
    }

    /// Find the machine that operates on `instance` and defines `target` as a state. The
    /// instance is a `Struct` whose `type_name` is the data/machine type (e.g. `Circle`); a
    /// free machine `Circle::code` lives in that type's group. Matches by machine symbol, by
    /// attached-data name, or by the `<type>::<target>` group-qualified machine name.
    pub(super) fn machine_for_instance_state(
        &self,
        instance: &Cell,
        target: &str,
    ) -> Option<Machine> {
        let (type_symbol, type_name) = match &*instance.borrow() {
            Value::Struct {
                type_symbol,
                type_name,
                ..
            } => (*type_symbol, type_name.clone()),
            // An ENUM receiver (`self.s.go_value()` where `s: Signal`): the
            // enum-attached machine group is the declaring data type, whose
            // NAME resolves from the value's type_symbol. Without this arm the
            // method call silently failed to find its machine and returned ZII.
            Value::Enum { type_symbol, .. } => {
                let name = self
                    .program
                    .data_definitions()
                    .iter()
                    .find(|data| type_symbol.is_valid() && data.symbol == *type_symbol)
                    .map(|data| data.name.as_str().to_owned())?;
                (*type_symbol, name)
            }
            _ => return None,
        };
        // The group is the leading segment of the type name (e.g. `Circle` from `Circle`).
        let group = type_name
            .split("::")
            .next()
            .unwrap_or(&type_name)
            .to_owned();
        for machine in self.program.machines() {
            if self.find_state(machine, target).is_none() {
                continue;
            }
            let machine_group = machine
                .name
                .as_str()
                .split("::")
                .next()
                .unwrap_or("")
                .to_owned();
            let by_symbol = type_symbol.is_valid() && machine.symbol == type_symbol;
            let by_attached = machine
                .attached_data
                .as_ref()
                .is_some_and(|data| data.as_str() == group);
            let by_group = machine_group == group;
            if by_symbol || by_attached || by_group {
                return Some(machine.clone());
            }
        }
        None
    }

    pub(super) fn current_machine(&self, frame: &Frame) -> Option<&'program Machine> {
        if !frame.machine_symbol.is_valid() {
            return None;
        }
        self.program
            .machines()
            .iter()
            .find(|machine| machine.symbol == frame.machine_symbol)
    }

    /// Find the machine invoked by a call whose `target` is a state name. A free helper
    /// machine is named `<group>::<target>` (e.g. `Main::bump`); resolve by that name, or
    /// by any machine that contains a state of that name and shares the receiver group.
    pub(super) fn find_machine_for_call(&self, target: &str, frame: &Frame) -> Option<Machine> {
        // The receiver's machine-group prefix (e.g. "Main" from "Main::main").
        let group = {
            let self_name = match &*frame.self_cell.borrow() {
                Value::Struct { type_name, .. } => type_name.clone(),
                _ => String::new(),
            };
            self_name
                .split("::")
                .next()
                .map(|prefix| prefix.to_owned())
                .unwrap_or_default()
        };

        let qualified = format!("{group}::{target}");
        if let Some(machine) = self.find_machine_by_name(&qualified) {
            return Some(machine.clone());
        }
        // A FREE top-level machine named exactly `target` (`machine pick(x: i32)
        // -> i32`): its body state is the generated `entry`, so the state-name
        // scan below would miss it.
        if let Some(machine) = self.find_machine_by_name(target) {
            if machine.attached_data.is_none() {
                return Some(machine.clone());
            }
        }
        // Otherwise a machine that simply has a state named `target` -- but only when that
        // is UNAMBIGUOUS. With several candidates (e.g. two impls of the same trait
        // machine), guessing the first would silently dispatch to the wrong type; decline
        // instead so the caller reports unsupported (dispatch by the RECEIVER's runtime
        // type is handled earlier, in `machine_for_instance_state`).
        let mut candidates = self
            .program
            .machines()
            .iter()
            .filter(|machine| self.find_state(machine, target).is_some());
        let first = candidates.next().cloned();
        if candidates.next().is_some() {
            return None;
        }
        first
    }

    pub(super) fn machine_entry_state_name(&self, machine: &Machine) -> Option<String> {
        // A free helper machine `Main::bump` exposes its body as a state. Prefer a state
        // whose name matches the machine's leaf (`bump`); else the first state.
        let leaf = machine.name.as_str().rsplit("::").next().unwrap_or("");
        if self.find_state(machine, leaf).is_some() {
            return Some(leaf.to_owned());
        }
        self.program
            .machine_states(machine)
            .first()
            .map(|state| state.name.as_str().to_owned())
    }

    /// Evaluate an argument. A `Mutable(place)` or a direct place under a `&mut` param
    /// yields a `Ref` that ALIASES the original cell; a value argument yields a fresh
    /// cell holding a copy.
    pub(super) fn eval_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Cell> {
        match self.program.expression_table.expression(argument) {
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
                    Value::Ref(target) => Rc::clone(target),
                    _ => Rc::clone(&cell),
                };
                Ok(Value::Ref(target).cell())
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                // A bare place argument that is ALREADY a reference (a forwarded `&mut`
                // parameter, e.g. `out_room` of type `&mut Room`) must keep aliasing the
                // same underlying cell -- otherwise a chain of forwarding calls silently
                // detaches the write. If the place resolves and holds a `Ref`, forward that
                // cell; otherwise evaluate the expression normally (handles enum-value name
                // paths, plain values, etc.).
                if let Ok(place) = self.resolve_place(argument, frame) {
                    let forwarded = match &*place.borrow() {
                        Value::Ref(target) => Some(Rc::clone(target)),
                        _ => None,
                    };
                    if let Some(target) = forwarded {
                        // Keep the Ref WRAPPER (not the bare target cell) so reference-ness
                        // survives the NEXT hop too: the callee's param must itself look like
                        // a `&mut` binding when it forwards the bare name onward (e.g. a
                        // transition arm `gate_title(out_line)` two machines deep).
                        return Ok(Value::Ref(target).cell());
                    }
                }
                let value = self.eval_expression(argument, frame)?;
                Ok(value.cell())
            }
            _ => {
                let value = self.eval_expression(argument, frame)?;
                Ok(value.cell())
            }
        }
    }
}
