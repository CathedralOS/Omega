use crate::value::{Cell, Value};
use crate::InterpretOutcome;
use omega_checked_trees::CheckedTrees;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::data::{DataDefinition, DataMember};
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use omega_typed_trees::types::PrimitiveType;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

const STEP_BUDGET: u64 = 10_000_000;
/// Max native recursion depth (call / cross-machine transition nesting) before we decline
/// rather than overflow the host stack. Deep recursive programs are skipped (reported as
/// unsupported), never crash the differential harness.
const CALL_DEPTH_BUDGET: u32 = 512;

pub(crate) fn run(checked: &CheckedTrees, stdin: &[u8]) -> InterpretOutcome {
    // Run on a worker thread with a generous stack: the tree-walker recurses with the
    // program's call/expression nesting, which can exceed the default test-thread stack on
    // deep programs even with the call-depth budget. A scoped thread lets us keep the
    // borrow of `checked`/`stdin`.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || run_on_current_thread(checked, stdin))
            .expect("spawn interpreter worker thread")
            .join()
            .unwrap_or_else(|_| {
                InterpretOutcome::error("interpreter thread panicked", Vec::new(), Vec::new())
            })
    })
}

fn run_on_current_thread(checked: &CheckedTrees, stdin: &[u8]) -> InterpretOutcome {
    let mut evaluator = Evaluator::new(checked, stdin);
    match evaluator.run_entry() {
        Ok(()) => {
            // Reached a terminal transition without an explicit exit_process.
            InterpretOutcome::exited(0, evaluator.stdout, evaluator.stderr)
        }
        Err(Halt::Exit(code)) => InterpretOutcome::exited(code, evaluator.stdout, evaluator.stderr),
        Err(Halt::Unsupported(message)) | Err(Halt::Trap(message)) => {
            InterpretOutcome::error(message, evaluator.stdout, evaluator.stderr)
        }
    }
}

/// A non-local control-flow signal. `Exit` halts cleanly with a code; the others abort
/// the run and surface as `InterpretOutcome.error` (so a harness skips rather than
/// reports a false mismatch).
enum Halt {
    Exit(i32),
    Unsupported(String),
    Trap(String),
}

type EvalResult<T> = Result<T, Halt>;

fn unsupported<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Unsupported(message.into()))
}

fn trap<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Trap(message.into()))
}

/// A lexical scope: parameter / local bindings by name, plus the receiver (`self`) cell.
/// `locals` is behind a `RefCell` so `let` bindings can be added while the frame is
/// shared by `&` during statement execution.
struct Frame {
    locals: RefCell<BTreeMap<String, Cell>>,
    self_cell: Cell,
    /// The machine whose state is currently executing. Lets a call/transition that names a
    /// SIBLING state resolve it within this machine (rather than re-entering the machine's
    /// entry state, which would recurse forever).
    machine_symbol: SymbolHandle,
    /// Value-call results computed while evaluating THIS state pass's transition guards,
    /// keyed by call-expression handle. A transition subject is evaluated ONCE per
    /// transition evaluation: the parser lowers `transition self.f(x) { true -> a
    /// false -> b }` into one guard per arm, each holding a COPY of the subject call, so
    /// a later arm must reuse the first arm's result (matching the native lowering)
    /// instead of re-running the callee's side effects. Copies have distinct handles, so
    /// lookups compare structurally. The frame is rebuilt for every state (re)entry, so
    /// loops re-evaluate naturally.
    guard_call_results: RefCell<Vec<(ExpressionHandle, Value)>>,
}

struct Evaluator<'program> {
    program: &'program CheckedTrees,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdin: &'program [u8],
    stdin_cursor: usize,
    steps: u64,
    call_depth: u32,
    /// Non-zero while evaluating a transition GUARD expression. Value-calls evaluated
    /// under a guard memoize into the frame's `guard_call_results` so the per-arm
    /// copies of one transition subject evaluate the callee once (see `Frame`).
    guard_depth: u32,
}

impl<'program> Evaluator<'program> {
    fn new(program: &'program CheckedTrees, stdin: &'program [u8]) -> Self {
        Self {
            program,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdin,
            stdin_cursor: 0,
            steps: 0,
            call_depth: 0,
            guard_depth: 0,
        }
    }

    fn tick(&mut self) -> EvalResult<()> {
        self.steps += 1;
        if self.steps > STEP_BUDGET {
            return trap("step budget exceeded");
        }
        Ok(())
    }

    // ---- entry --------------------------------------------------------------

    fn run_entry(&mut self) -> EvalResult<()> {
        let entry_machine = self
            .find_machine_by_name("Main::main")
            .or_else(|| self.find_machine_by_name("main"))
            .ok_or_else(|| Halt::Unsupported("no entry machine `Main::main`".to_owned()))?;
        let entry_state_name = if self.find_state(entry_machine, "main").is_some() {
            "main"
        } else {
            "entry"
        };

        let instance = self.instantiate_machine(entry_machine)?;
        // The entry machine's value (its terminal `Value` transition / final expression)
        // becomes the process exit code when it has no explicit `exit_process`. Mirrors the
        // backend: `machine Main::main(...) -> i32` returns the exit status.
        let returned =
            self.run_state_collect(entry_machine, entry_state_name, instance, Vec::new())?;
        if let Some(value) = returned {
            if let Some(code) = value.as_int() {
                return Err(Halt::Exit(code as i32));
            }
        }
        Ok(())
    }

    // ---- machine / data instantiation --------------------------------------

    /// Build a machine instance as a `Struct` whose fields are the attached data's
    /// fields (with their defaults) plus the machine's contained sub-objects.
    fn instantiate_machine(&mut self, machine: &Machine) -> EvalResult<Cell> {
        let mut fields: BTreeMap<String, Cell> = BTreeMap::new();

        if let Some(data_name) = machine.attached_data.as_ref() {
            if let Some(data) = self.find_data_by_name(data_name.as_str()) {
                self.populate_data_fields(data, &mut fields)?;
            }
        }

        // Machine-owned data (the `owned_data` span) are additional named cells.
        for owned in self.program.machine_owned_data(machine) {
            let value = if owned.initial_value.is_valid() {
                let frame = Frame {
                    locals: RefCell::new(BTreeMap::new()),
                    self_cell: Value::Unit.cell(),
                    machine_symbol: SymbolHandle::invalid(),
                    guard_call_results: RefCell::new(Vec::new()),
                };
                self.eval_expression(owned.initial_value, &frame)?
            } else {
                self.default_value_for_type(owned.type_reference)?
            };
            fields.insert(owned.name.as_str().to_owned(), value.cell());
        }

        Ok(Value::Struct {
            type_symbol: machine.symbol,
            type_name: machine.name.as_str().to_owned(),
            fields,
        }
        .cell())
    }

    /// Insert a `data` definition's fields (with defaults) into `fields`. Nested `data`
    /// members recurse so their own defaults are populated.
    fn populate_data_fields(
        &mut self,
        data: &DataDefinition,
        fields: &mut BTreeMap<String, Cell>,
    ) -> EvalResult<()> {
        for member in self.program.data_members(data) {
            let DataMember::Field(field) = member else {
                continue;
            };
            let name = field.name.as_str().to_owned();

            let value = if field.initial_value.is_valid() {
                let frame = Frame {
                    locals: RefCell::new(BTreeMap::new()),
                    self_cell: Value::Unit.cell(),
                    machine_symbol: SymbolHandle::invalid(),
                    guard_call_results: RefCell::new(Vec::new()),
                };
                self.eval_expression(field.initial_value, &frame)?
            } else {
                self.default_value_for_type(field.type_reference)?
            };
            fields.insert(name, value.cell());
        }
        Ok(())
    }

    /// Build a default-initialized value for a declared type, recursing into nested `data`
    /// records (a sub-Struct with its own defaults) and fixed arrays (an `Array` of
    /// per-element default cells). Falls back to the primitive/unit default.
    fn default_value_for_type(
        &mut self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> EvalResult<Value> {
        if type_reference.is_valid() {
            // Fixed array `[T; N]` -> N default-initialized element cells.
            if let omega_typed_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length,
            } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                let element_type = *element_type;
                if let omega_typed_trees::types::FixedArrayLength::Literal(count) = length {
                    let count = *count;
                    let mut elements = Vec::with_capacity(count);
                    for _ in 0..count {
                        elements.push(self.default_value_for_type(element_type)?.cell());
                    }
                    return Ok(Value::Array(elements));
                }
            }

            // Nested `data` record -> a sub-Struct of its own defaults.
            if let Some(nested) = self.field_nested_data(type_reference) {
                let mut nested_fields = BTreeMap::new();
                self.populate_data_fields(nested, &mut nested_fields)?;
                return Ok(Value::Struct {
                    type_symbol: nested.symbol,
                    type_name: nested.name.as_str().to_owned(),
                    fields: nested_fields,
                });
            }
        }
        Ok(self.default_for_type(type_reference))
    }

    /// If a field's declared type is a (non-primitive) `data` record, return it.
    fn field_nested_data(
        &self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> Option<&'program DataDefinition> {
        if !type_reference.is_valid() {
            return None;
        }
        if self.program.primitive_type_reference(type_reference).is_some() {
            return None;
        }
        let symbol = self.program.type_reference_symbol(type_reference);
        if !symbol.is_valid() {
            return None;
        }
        self.program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)
            .filter(|data| {
                // Records instantiate as nested structs; enums don't. EMPTY records (e.g.
                // `data Circle {}`) must still instantiate as a typed struct -- the type
                // identity is what a `dyn Trait` receiver dispatches on at runtime.
                !matches!(
                    DataDefinition::shape_kind_from_members(self.program.data_members(data)),
                    omega_typed_trees::data::DataShapeKind::Enum
                )
            })
    }

    fn default_for_type(
        &self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> Value {
        match self.program.primitive_type_reference(type_reference) {
            Some(PrimitiveType::Bool) => Value::Bool(false),
            Some(PrimitiveType::F32) | Some(PrimitiveType::F64) => Value::Float(0.0),
            Some(PrimitiveType::String) => Value::str(String::new()),
            Some(_) => Value::Int(0),
            None => Value::Unit,
        }
    }

    // ---- state execution ----------------------------------------------------

    /// Run a state; returns the value produced by a `Value` transition target, if any.
    /// Guards native recursion depth so a deeply recursive program is declined (skipped)
    /// instead of overflowing the host stack.
    fn run_state_collect(
        &mut self,
        machine: &Machine,
        state_name: &str,
        instance: Cell,
        args: Vec<Cell>,
    ) -> EvalResult<Option<Value>> {
        self.call_depth += 1;
        if self.call_depth > CALL_DEPTH_BUDGET {
            self.call_depth -= 1;
            return unsupported("recursion depth budget exceeded");
        }
        let result = self.run_state_collect_inner(machine, state_name, instance, args);
        self.call_depth -= 1;
        result
    }

    fn run_state_collect_inner(
        &mut self,
        machine: &Machine,
        state_name: &str,
        instance: Cell,
        args: Vec<Cell>,
    ) -> EvalResult<Option<Value>> {
        let mut current_state = state_name.to_owned();
        let mut current_args = args;
        // Locals accumulated across SAME-machine sibling transitions: the backend models a
        // machine as one frame whose slots persist, so an inlined sub-state still sees the
        // enclosing state's params/`let`s (e.g. `mark_current_room` reading `enter_room`'s
        // `room_index`). New args bind on top; carried-over names stay visible.
        let mut carried: BTreeMap<String, Cell> = BTreeMap::new();

        loop {
            self.tick()?;
            let state = self
                .find_state(machine, &current_state)
                .ok_or_else(|| Halt::Unsupported(format!("unknown state `{current_state}`")))?
                .clone();

            let frame = self.bind_frame(
                &state,
                Rc::clone(&instance),
                &current_args,
                machine.symbol,
                &carried,
            )?;

            // Execute statements, watching for the first satisfied transition. A state
            // whose body ends in a bare expression (`{ 22 }`) returns that expression's
            // value as its result (the backend's value-state form).
            let mut next: Option<TransitionDecision> = None;
            let mut tail_value: Option<Value> = None;
            for statement in self.program.statement_table.statements(state.statement_nodes) {
                let statement = statement.clone();
                match &statement {
                    StatementNode::Transition(transition) => {
                        if let Some(decision) = self.eval_transition(transition, &frame)? {
                            next = Some(decision);
                            break;
                        }
                    }
                    StatementNode::Expression(expression) => {
                        tail_value = Some(self.eval_expression(*expression, &frame)?);
                    }
                    other => {
                        self.exec_statement(other, &frame)?;
                    }
                }
            }

            match next {
                None => return Ok(tail_value),
                Some(TransitionDecision::Value(value)) => return Ok(Some(value)),
                Some(TransitionDecision::Terminal) => return Ok(None),
                Some(TransitionDecision::SelfTarget) => {
                    // Re-run the same state (rare; guard against infinite loops via budget),
                    // carrying its bindings forward.
                    carried = frame.locals.into_inner();
                    continue;
                }
                Some(TransitionDecision::Named {
                    state_name,
                    machine: target_machine,
                    instance: target_instance,
                    args,
                }) => {
                    if target_machine.symbol == machine.symbol
                        && Rc::ptr_eq(&target_instance, &instance)
                    {
                        // Carry this state's bindings forward to the sibling state.
                        carried = frame.locals.into_inner();
                        current_state = state_name;
                        current_args = args;
                        continue;
                    }
                    // Cross-machine named transition: run it to completion, then we are
                    // done (the entry sub-state machine model is single-threaded).
                    return self.run_state_collect(
                        &target_machine,
                        &state_name,
                        target_instance,
                        args,
                    );
                }
            }
        }
    }

    /// Bind a state's parameters (skipping `self`) to the positional argument cells. Seeds
    /// from `carried` (the enclosing same-machine state's bindings) so an inlined sub-state
    /// still sees outer params/locals; the state's own params override on top.
    fn bind_frame(
        &self,
        state: &State,
        self_cell: Cell,
        args: &[Cell],
        machine_symbol: SymbolHandle,
        carried: &BTreeMap<String, Cell>,
    ) -> EvalResult<Frame> {
        let mut locals = carried.clone();
        let mut arg_index = 0;
        for parameter in self.program.state_parameters(state) {
            if parameter.is_self {
                continue;
            }
            let cell = args
                .get(arg_index)
                .cloned()
                .unwrap_or_else(|| Value::Unit.cell());
            locals.insert(parameter.name.as_str().to_owned(), cell);
            arg_index += 1;
        }
        Ok(Frame {
            locals: RefCell::new(locals),
            self_cell,
            machine_symbol,
            guard_call_results: RefCell::new(Vec::new()),
        })
    }

    // ---- statements ---------------------------------------------------------

    fn exec_statement(&mut self, statement: &StatementNode, frame: &Frame) -> EvalResult<()> {
        self.tick()?;
        match statement {
            StatementNode::Assignment(assignment) => {
                let value = self.eval_expression(assignment.value, frame)?;
                let target = self.resolve_place(assignment.target, frame)?;
                // Assigning to a `&mut` place writes THROUGH the reference into the aliased
                // cell (so `out_line = ...` on an `out_line: &mut String` param mutates the
                // caller's String), rather than rebinding the local to a non-reference value.
                let target = self.deref_cell(target);
                *target.borrow_mut() = value;
                Ok(())
            }
            StatementNode::LocalData(local) => {
                let value = if local.initial_value.is_valid() {
                    self.eval_expression(local.initial_value, frame)?
                } else {
                    self.default_value_for_type(local.type_reference)?
                };
                // A `let` introduces a fresh local cell, bound through the frame's
                // interior-mutable locals map.
                frame.bind(local.name.as_str(), value.cell());
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

    fn eval_transition(
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
            TransitionTargetNode::Named { path, arguments } => {
                let members = self.program.statement_table.name_path_members(path.members);
                let state_name = members
                    .last()
                    .map(|name| name.as_str().to_owned())
                    .ok_or_else(|| Halt::Unsupported("empty named transition".to_owned()))?;

                // Same-machine sibling state on the current `self`.
                let machine = self
                    .machine_of_state_named(&state_name, frame)
                    .ok_or_else(|| {
                        Halt::Unsupported(format!(
                            "transition target `{state_name}` not found in current machine"
                        ))
                    })?;

                let mut args = Vec::new();
                for argument in self.program.statement_table.expression_handles(*arguments) {
                    args.push(self.eval_argument(*argument, frame)?);
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

    /// Find the machine that owns a sibling state of `self` by state name. The entry and
    /// its sub-states all live in the same machine group; a named transition stays within
    /// the current machine.
    fn machine_of_state_named(&self, state_name: &str, frame: &Frame) -> Option<Machine> {
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
        // Host boundary call? (e.g. self.console.exit_process(70))
        if let Some(value) = self.try_host_call(call, frame)? {
            return Ok(value);
        }

        let target = call.target.as_str();
        let (machine, state_name, instance) = self.resolve_state_call(call.receiver, target, frame)?;

        let mut args = Vec::new();
        for argument in self.program.statement_table.expression_handles(call.arguments) {
            args.push(self.eval_argument(*argument, frame)?);
        }

        self.run_state_collect(&machine, &state_name, instance, args)
            .map(|value| value.unwrap_or(Value::Unit))
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
        receiver: omega_core::arena::HandleSpan<omega_typed_trees::name::Identifier>,
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
                return Ok((machine.clone(), target.to_owned(), Rc::clone(&frame.self_cell)));
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
        receiver: omega_core::arena::HandleSpan<omega_typed_trees::name::Identifier>,
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
        let bare_self = members.len() == start && start == 1;
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
    fn machine_for_instance_state(&self, instance: &Cell, target: &str) -> Option<Machine> {
        let (type_symbol, type_name) = match &*instance.borrow() {
            Value::Struct {
                type_symbol,
                type_name,
                ..
            } => (*type_symbol, type_name.clone()),
            _ => return None,
        };
        // The group is the leading segment of the type name (e.g. `Circle` from `Circle`).
        let group = type_name.split("::").next().unwrap_or(&type_name).to_owned();
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

    fn current_machine(&self, frame: &Frame) -> Option<&'program Machine> {
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
    fn find_machine_for_call(&self, target: &str, frame: &Frame) -> Option<Machine> {
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

    fn machine_entry_state_name(&self, machine: &Machine) -> Option<String> {
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
    fn eval_argument(&mut self, argument: ExpressionHandle, frame: &Frame) -> EvalResult<Cell> {
        match self.program.expression_table.expression(argument) {
            ExpressionNode::Mutable(inner) => {
                // &mut place -> a Ref to the SAME cell (the whole point of the oracle). The
                // param binding holds a `Ref`, so a later forward of that param (as a bare
                // name) can detect it is a reference and keep aliasing -- otherwise a
                // `&mut String` field passed down a call chain detaches after the first hop.
                let cell = self.resolve_place(*inner, frame)?;
                Ok(Value::Ref(cell).cell())
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

    fn try_host_call(&mut self, call: &TableCall, frame: &Frame) -> EvalResult<Option<Value>> {
        if !self.is_boundary_call(call, frame) {
            return Ok(None);
        }
        let target = call.target.as_str();
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
                        Value::Str(text) => text.borrow().as_bytes().to_vec(),
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
            "read_line" => {
                // Read up to (and including) the next newline from the remaining stdin into
                // the `&mut String` out-parameter. CRLF is normalized (a trailing `\r` is
                // dropped). Returns whether a line was available (some programs ignore it).
                let line = self.read_stdin_line();
                if let Some(first) = arguments.first() {
                    if let Ok(cell) = self.resolve_place(*first, frame) {
                        let cell = self.deref_cell(cell);
                        if let Value::Str(text) = &*cell.borrow() {
                            *text.borrow_mut() = line.clone();
                        } else {
                            *cell.borrow_mut() = Value::str(line.clone());
                        }
                    }
                }
                Ok(Some(Value::Bool(!line.is_empty())))
            }
            other => unsupported(format!("host boundary call `{other}` not yet supported")),
        }
    }

    /// Consume the next line from the remaining stdin (without the line terminator). CRLF
    /// and LF are both handled; returns an empty string at end of input.
    fn read_stdin_line(&mut self) -> String {
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
    fn is_boundary_call(&self, call: &TableCall, frame: &Frame) -> bool {
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
            && self
                .program
                .traits()
                .iter()
                .any(|trait_definition| {
                    trait_definition.is_boundary && trait_definition.symbol == symbol
                })
    }

    // ---- expressions --------------------------------------------------------

    fn eval_expression(&mut self, handle: ExpressionHandle, frame: &Frame) -> EvalResult<Value> {
        self.tick()?;
        let node = self.program.expression_table.expression(handle).clone();
        match node {
            ExpressionNode::Integer(value) => Ok(Value::Int(value)),
            ExpressionNode::Boolean(value) => Ok(Value::Bool(value)),
            ExpressionNode::Float(value) => Ok(Value::Float(value.value())),
            ExpressionNode::String(value) => Ok(Value::str(value.to_string())),
            ExpressionNode::Name(path) => self.eval_name(&path, frame),
            ExpressionNode::Member(_) => {
                let cell = self.resolve_place(handle, frame)?;
                let value = cell.borrow().clone();
                Ok(value)
            }
            ExpressionNode::Mutable(inner) => {
                let cell = self.resolve_place(inner, frame)?;
                Ok(Value::Ref(cell))
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.eval_expression(unary.operand, frame)?;
                self.eval_unary(unary.operator, operand)
            }
            ExpressionNode::Binary(binary) => {
                let left = self.eval_expression(binary.left, frame)?;
                let right = self.eval_expression(binary.right, frame)?;
                self.eval_binary(binary.operator, left, right)
            }
            ExpressionNode::Call(call) => self.eval_call_expression(handle, &call, frame),
            ExpressionNode::Cast(cast) => {
                let value = self.eval_expression(cast.value, frame)?;
                let target = self.cast_target_primitive(cast.target_type);
                self.eval_cast(value, target)
            }
            ExpressionNode::Indexed(indexed) => {
                // A range index `arr[start..end]` produces a SUBSLICE view sharing the
                // collection's element cells; a scalar index reads one element.
                if let ExpressionNode::Range(range) =
                    self.program.expression_table.expression(indexed.index).clone()
                {
                    return self.eval_subslice(indexed.collection, &range, frame);
                }
                let cell = self.resolve_place(handle, frame)?;
                let value = self.deref_cell(cell).borrow().clone();
                Ok(value)
            }
            ExpressionNode::ArrayLiteral(values) => {
                let mut elements = Vec::new();
                for value in self.program.expression_table.expression_handles(values) {
                    elements.push(self.eval_expression(*value, frame)?.cell());
                }
                Ok(Value::Array(elements))
            }
            // The frontend only produces a Range under an index expression (handled in the
            // Indexed arm above as a subslice); general/open ranges in value or argument
            // position are parse errors (probed in tests/coverage.rs). Decline defensively
            // in case a future frontend starts emitting them elsewhere.
            ExpressionNode::Range(_) => unsupported("range expression outside index position"),
            ExpressionNode::StructLiteral(literal) => self.eval_struct_literal(&literal, frame),
        }
    }

    fn eval_call_expression(
        &mut self,
        handle: ExpressionHandle,
        call: &omega_typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Value> {
        // Builtins: max / min over two integer/float operands.
        let target = call.target.as_str();
        if matches!(target, "max" | "min") {
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 2 {
                let left = self.eval_expression(args[0], frame)?;
                let right = self.eval_expression(args[1], frame)?;
                return self.eval_min_max(target, left, right);
            }
        }

        // Slice/array view builtins on an array-valued receiver. `.as_slice()` /
        // `.as_mut_slice()` produce a slice that SHARES the array's element cells (so a
        // write through the slice aliases the array); `.len()` returns the element count.
        if matches!(target, "as_slice" | "as_mut_slice" | "len") && call.receiver.is_valid() {
            if let Ok(cell) = self.resolve_place(call.receiver, frame) {
                let cell = self.deref_cell(cell);
                let elements = match &*cell.borrow() {
                    Value::Array(elements) => Some(elements.clone()),
                    _ => None,
                };
                if let Some(elements) = elements {
                    return Ok(match target {
                        "len" => Value::Int(elements.len() as i64),
                        // A slice view shares the same element `Rc`s.
                        _ => Value::Array(elements),
                    });
                }
            }
        }

        // A transition's guard subject evaluates ONCE per transition evaluation: the
        // parser lowers `transition self.f(x) { true -> a false -> b }` into one guard
        // per arm, each holding a COPY of the subject call (distinct handles, identical
        // structure). A later arm reuses the earlier arm's result instead of re-running
        // the callee's side effects -- matching the native lowering's shared prelude.
        if self.guard_depth > 0 {
            let memo = frame.guard_call_results.borrow();
            for (seen, value) in memo.iter() {
                if self
                    .program
                    .expression_table
                    .expressions_structurally_equal(*seen, handle)
                {
                    return Ok(value.clone());
                }
            }
        }

        // Resolve the value-call. A bare-self receiver naming a SIBLING state of the
        // current machine runs that state; a receiver expression resolving to a contained
        // sub-machine instance runs on that instance; otherwise a free helper machine.
        let (machine, entry_state, instance) =
            self.resolve_value_call_target(call, target, frame)?;
        let mut args = Vec::new();
        for argument in self.program.expression_table.expression_handles(call.arguments) {
            args.push(self.eval_call_expression_argument(*argument, frame)?);
        }
        // Suspend the guard flag while the callee RUNS: distinct same-shaped calls
        // inside its body are genuine repeat calls, not copies of one source
        // expression, and must not memoize against each other.
        let entered_guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let value = self
            .run_state_collect(&machine, &entry_state, instance, args)
            .map(|value| value.unwrap_or(Value::Unit));
        self.guard_depth = entered_guard_depth;
        let value = value?;
        if entered_guard_depth > 0 {
            frame
                .guard_call_results
                .borrow_mut()
                .push((handle, value.clone()));
        }
        Ok(value)
    }

    fn resolve_value_call_target(
        &mut self,
        call: &omega_typed_trees::expression::TableCallExpression,
        target: &str,
        frame: &Frame,
    ) -> EvalResult<(Machine, String, Cell)> {
        // (1) Receiver expression resolving to a contained sub-machine / data instance
        // (e.g. `s.code()` where `s: &mut Circle`): run on that instance's machine.
        if call.receiver.is_valid() {
            if let Ok(cell) = self.resolve_place(call.receiver, frame) {
                let cell = self.deref_cell(cell);
                let is_self = Rc::ptr_eq(&cell, &frame.self_cell);
                if !is_self {
                    if let Some(machine) = self.machine_for_instance_state(&cell, target) {
                        return Ok((machine, target.to_owned(), cell));
                    }
                }
            }
        }

        // (2) Sibling state of the current machine.
        if let Some(machine) = self.current_machine(frame) {
            if self.find_state(machine, target).is_some() {
                return Ok((machine.clone(), target.to_owned(), Rc::clone(&frame.self_cell)));
            }
        }

        // (3) A free helper machine.
        let machine = self
            .find_machine_for_call(target, frame)
            .ok_or_else(|| Halt::Unsupported(format!("unknown value-call target `{target}`")))?;
        let entry_state = self
            .machine_entry_state_name(&machine)
            .ok_or_else(|| Halt::Unsupported(format!("value-call `{target}` has no state")))?;
        Ok((machine, entry_state, Rc::clone(&frame.self_cell)))
    }

    fn eval_call_expression_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Cell> {
        // Share the same argument-evaluation rules as state calls (incl. reference
        // forwarding for bare-place args that already hold a `&mut`).
        self.eval_argument(argument, frame)
    }

    fn eval_name(&mut self, path: &TableNamePath, frame: &Frame) -> EvalResult<Value> {
        // The boolean keywords `true`/`false` can arrive as single-member name paths in
        // value/transition position (the parser does not always fold them to a literal).
        let members = self.program.expression_table.name_path_members(path.members);
        if members.len() == 1 {
            match members[0].as_str() {
                "true" => return Ok(Value::Bool(true)),
                "false" => return Ok(Value::Bool(false)),
                _ => {}
            }
        }
        // An enum value reference (`CellId::R02` / `Command::Look`) resolves to an Enum.
        if let Some(enum_value) = self.enum_value_from_path(members) {
            return Ok(enum_value);
        }
        let cell = self.resolve_name_place(path, frame)?;
        let value = cell.borrow().clone();
        // A `Ref` read through a name dereferences transparently (param of `&mut T`).
        if let Value::Ref(inner) = value {
            return Ok(inner.borrow().clone());
        }
        Ok(value)
    }

    /// `Type::Variant` paths whose head is an enum/data symbol with a matching variant.
    fn enum_value_from_path(
        &self,
        members: &[omega_typed_trees::name::Identifier],
    ) -> Option<Value> {
        if members.len() != 2 {
            return None;
        }
        let type_name = members[0].as_str();
        let variant_name = members[1].as_str();
        let data = self.find_data_by_name(type_name)?;
        let is_variant = self.program.data_members(data).iter().any(|member| {
            matches!(member, DataMember::Variant(variant) if variant.name.as_str() == variant_name)
        });
        is_variant.then(|| Value::Enum {
            variant_name: variant_name.to_owned(),
            payload: Vec::new(),
        })
    }

    // ---- place resolution ---------------------------------------------------

    /// Resolve an lvalue expression to its storage cell (for assignment / `&mut`).
    fn resolve_place(&mut self, handle: ExpressionHandle, frame: &Frame) -> EvalResult<Cell> {
        match self.program.expression_table.expression(handle).clone() {
            ExpressionNode::Name(path) => self.resolve_name_place(&path, frame),
            ExpressionNode::Member(member) => {
                let receiver = self.resolve_place(member.receiver, frame)?;
                self.field_cell(&receiver, member.member.as_str())
            }
            ExpressionNode::Indexed(indexed) => {
                let collection = self.resolve_place(indexed.collection, frame)?;
                let index = self.eval_index(indexed.index, frame)?;
                self.element_cell(&collection, index)
            }
            ExpressionNode::Mutable(inner) => self.resolve_place(inner, frame),
            other => unsupported(format!("place expression not supported: {other:?}")),
        }
    }

    fn resolve_name_place(&mut self, path: &TableNamePath, frame: &Frame) -> EvalResult<Cell> {
        let members = self
            .program
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect::<Vec<_>>();
        if members.is_empty() {
            return trap("empty name path");
        }

        // Head: `self`, a local, or a self-field (implicit self).
        let head = members[0].as_str();
        let mut cell = if head == "self" {
            Rc::clone(&frame.self_cell)
        } else if let Some(local) = frame.get(head) {
            local
        } else {
            // Implicit self-field: `n` means `self.n`.
            self.field_cell(&frame.self_cell, head)?
        };

        // Walk the remaining members as field accesses, dereferencing refs along the way.
        for member in &members[1..] {
            cell = self.deref_cell(cell);
            cell = self.field_cell(&cell, member)?;
        }
        Ok(cell)
    }

    /// If a cell holds a `Ref`, return the referenced cell (so field access on a `&mut`
    /// parameter reaches the aliased place). Otherwise the cell itself.
    fn deref_cell(&self, cell: Cell) -> Cell {
        let inner = match &*cell.borrow() {
            Value::Ref(target) => Some(Rc::clone(target)),
            _ => None,
        };
        inner.unwrap_or(cell)
    }

    /// Evaluate an index expression to a `usize` element index.
    fn eval_index(&mut self, index: ExpressionHandle, frame: &Frame) -> EvalResult<usize> {
        let value = self.eval_expression(index, frame)?;
        let raw = value
            .as_int()
            .ok_or_else(|| Halt::Trap("array index is not an integer".to_owned()))?;
        usize::try_from(raw).map_err(|_| Halt::Trap(format!("array index {raw} out of range")))
    }

    /// Resolve one element CELL of an `Array` place (sharing the same `Rc`, so a write
    /// through the returned cell aliases the array element).
    fn element_cell(&self, container: &Cell, index: usize) -> EvalResult<Cell> {
        let container = self.deref_cell(Rc::clone(container));
        let borrowed = container.borrow();
        match &*borrowed {
            Value::Array(elements) => elements
                .get(index)
                .cloned()
                .ok_or_else(|| Halt::Trap(format!("array index {index} out of bounds"))),
            other => trap(format!("cannot index {other:?}")),
        }
    }

    /// Evaluate a subslice `collection[start..end]` into an `Array` view that SHARES the
    /// collection's element cells (so writes through the subslice alias the original). A
    /// missing start defaults to 0; a missing end to the length; `end_inclusive` extends by
    /// one.
    fn eval_subslice(
        &mut self,
        collection: ExpressionHandle,
        range: &omega_typed_trees::expression::TableRangeExpression,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let collection_cell = self.resolve_place(collection, frame)?;
        let elements = match &*self.deref_cell(collection_cell).borrow() {
            Value::Array(elements) => elements.clone(),
            other => return trap(format!("cannot subslice {other:?}")),
        };
        let len = elements.len();
        let start = if range.start.is_valid() {
            self.eval_index(range.start, frame)?
        } else {
            0
        };
        let mut end = if range.end.is_valid() {
            self.eval_index(range.end, frame)?
        } else {
            len
        };
        if range.end_inclusive {
            end = end.saturating_add(1);
        }
        let end = end.min(len);
        let start = start.min(end);
        Ok(Value::Array(elements[start..end].to_vec()))
    }

    /// Construct a `data` value from a struct literal `Type { field: value, .. }`. Fields
    /// not named take the type's default; named fields override.
    fn eval_struct_literal(
        &mut self,
        literal: &omega_typed_trees::expression::TableStructLiteral,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let type_name = literal.type_name.as_str().to_owned();
        let data = self.find_data_by_name(&type_name);
        let (type_symbol, mut fields) = if let Some(data) = data {
            let mut fields = BTreeMap::new();
            self.populate_data_fields(data, &mut fields)?;
            (data.symbol, fields)
        } else {
            (SymbolHandle::invalid(), BTreeMap::new())
        };
        for field in self.program.expression_table.struct_fields(literal.fields) {
            let value = self.eval_expression(field.value, frame)?;
            fields.insert(field.name.as_str().to_owned(), value.cell());
        }
        Ok(Value::Struct {
            type_symbol,
            type_name,
            fields,
        })
    }

    fn field_cell(&self, container: &Cell, field: &str) -> EvalResult<Cell> {
        let container = self.deref_cell(Rc::clone(container));
        let borrowed = container.borrow();
        match &*borrowed {
            Value::Struct { fields, type_name, .. } => fields
                .get(field)
                .cloned()
                .ok_or_else(|| Halt::Trap(format!("no field `{field}` on `{type_name}`"))),
            // `slice.len` / `array.len` (member form, no parens) -> a fresh length cell.
            Value::Array(elements) if field == "len" => Ok(Value::Int(elements.len() as i64).cell()),
            other => trap(format!("cannot read field `{field}` of {other:?}")),
        }
    }

    // ---- operators ----------------------------------------------------------

    /// The target `PrimitiveType` of a cast's `target_type` name-path (its leaf member).
    fn cast_target_primitive(
        &self,
        target_type: omega_core::arena::HandleSpan<omega_typed_trees::name::Identifier>,
    ) -> Option<PrimitiveType> {
        self.program
            .expression_table
            .name_path_members(target_type)
            .last()
            .and_then(|name| PrimitiveType::from_name(name.as_str()))
    }

    /// Apply an `as` cast with width/signedness semantics: int<->float conversions and
    /// integer narrowing/widening (wrapping to the target width, sign- or zero-extending on
    /// read per the SOURCE signedness, which the value carries as its width tag).
    fn eval_cast(&self, value: Value, target: Option<PrimitiveType>) -> EvalResult<Value> {
        let Some(target) = target else {
            // A cast to a non-primitive (e.g. a trait object) is a no-op identity here.
            return Ok(value);
        };
        match target {
            PrimitiveType::F32 => {
                let source = value
                    .as_float()
                    .ok_or_else(|| Halt::Trap("cast to f32 of non-numeric".to_owned()))?;
                Ok(Value::Float(source as f32 as f64))
            }
            PrimitiveType::F64 => {
                let source = value
                    .as_float()
                    .ok_or_else(|| Halt::Trap("cast to f64 of non-numeric".to_owned()))?;
                Ok(Value::Float(source))
            }
            PrimitiveType::Bool => Ok(Value::Bool(value.as_bool().unwrap_or(false))),
            PrimitiveType::String => Ok(value),
            integer => {
                // Float -> int truncates toward zero; int -> int reinterprets at the target
                // width. The result keeps the target's width tag so later ops/casts wrap.
                let raw = match value {
                    Value::Float(f) => f.trunc() as i64,
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("cast to integer of non-numeric".to_owned()))?,
                };
                Ok(Value::Int(wrap_to_width(raw, integer)))
            }
        }
    }

    fn eval_unary(&self, operator: UnaryOperator, operand: Value) -> EvalResult<Value> {
        match operator {
            UnaryOperator::LogicalNot => operand
                .as_bool()
                .map(|value| Value::Bool(!value))
                .ok_or_else(|| Halt::Trap("logical-not of non-boolean".to_owned())),
        }
    }

    fn eval_binary(
        &self,
        operator: BinaryOperator,
        left: Value,
        right: Value,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;

        // Logical short-circuit-style operators (already fully evaluated here).
        if matches!(operator, And | Or) {
            let l = left
                .as_bool()
                .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
            let r = right
                .as_bool()
                .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
            return Ok(Value::Bool(match operator {
                And => l && r,
                Or => l || r,
                _ => unreachable!(),
            }));
        }

        // Equality / inequality across scalar kinds (incl. enums).
        if matches!(operator, Equal | NotEqual) {
            let equal = self.values_equal(&left, &right)?;
            return Ok(Value::Bool(if operator == Equal { equal } else { !equal }));
        }

        // String concatenation: `a + b` over two strings yields a fresh string.
        if let (Value::Str(a), Value::Str(b)) = (&left, &right) {
            if operator == Add {
                let mut joined = a.borrow().clone();
                joined.push_str(&b.borrow());
                return Ok(Value::str(joined));
            }
        }

        // Float arithmetic / comparison if either operand is float.
        if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
            let l = left
                .as_float()
                .ok_or_else(|| Halt::Trap("non-numeric float operand".to_owned()))?;
            let r = right
                .as_float()
                .ok_or_else(|| Halt::Trap("non-numeric float operand".to_owned()))?;
            return self.eval_float_binary(operator, l, r);
        }

        // Integer arithmetic / comparison.
        let l = left
            .as_int()
            .ok_or_else(|| Halt::Trap("non-integer operand".to_owned()))?;
        let r = right
            .as_int()
            .ok_or_else(|| Halt::Trap("non-integer operand".to_owned()))?;
        self.eval_int_binary(operator, l, r)
    }

    fn eval_int_binary(
        &self,
        operator: BinaryOperator,
        l: i64,
        r: i64,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;
        Ok(match operator {
            Add => Value::Int(l.wrapping_add(r)),
            Subtract => Value::Int(l.wrapping_sub(r)),
            Multiply => Value::Int(l.wrapping_mul(r)),
            Divide => {
                if r == 0 {
                    return trap("integer division by zero");
                }
                Value::Int(l.wrapping_div(r))
            }
            Modulo => {
                if r == 0 {
                    return trap("integer modulo by zero");
                }
                Value::Int(l.wrapping_rem(r))
            }
            ShiftLeft => Value::Int(l.wrapping_shl(r as u32)),
            ShiftRight => Value::Int(l.wrapping_shr(r as u32)),
            Less => Value::Bool(l < r),
            LessOrEqual => Value::Bool(l <= r),
            Greater => Value::Bool(l > r),
            GreaterOrEqual => Value::Bool(l >= r),
            Equal | NotEqual | And | Or => unreachable!("handled earlier"),
        })
    }

    fn eval_float_binary(
        &self,
        operator: BinaryOperator,
        l: f64,
        r: f64,
    ) -> EvalResult<Value> {
        use BinaryOperator::*;
        Ok(match operator {
            Add => Value::Float(l + r),
            Subtract => Value::Float(l - r),
            Multiply => Value::Float(l * r),
            Divide => Value::Float(l / r),
            Less => Value::Bool(l < r),
            LessOrEqual => Value::Bool(l <= r),
            Greater => Value::Bool(l > r),
            GreaterOrEqual => Value::Bool(l >= r),
            Modulo | ShiftLeft | ShiftRight => {
                return unsupported("float modulo/shift not supported")
            }
            Equal | NotEqual | And | Or => unreachable!("handled earlier"),
        })
    }

    fn eval_min_max(&self, name: &str, left: Value, right: Value) -> EvalResult<Value> {
        if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
            let l = left.as_float().ok_or_else(|| Halt::Trap("min/max float".to_owned()))?;
            let r = right.as_float().ok_or_else(|| Halt::Trap("min/max float".to_owned()))?;
            return Ok(Value::Float(if name == "max" { l.max(r) } else { l.min(r) }));
        }
        let l = left.as_int().ok_or_else(|| Halt::Trap("min/max int".to_owned()))?;
        let r = right.as_int().ok_or_else(|| Halt::Trap("min/max int".to_owned()))?;
        Ok(Value::Int(if name == "max" { l.max(r) } else { l.min(r) }))
    }

    fn values_equal(&self, left: &Value, right: &Value) -> EvalResult<bool> {
        Ok(match (left, right) {
            (
                Value::Enum {
                    variant_name: a,
                    payload: pa,
                },
                Value::Enum {
                    variant_name: b,
                    payload: pb,
                },
            ) => {
                a == b
                    && pa.len() == pb.len()
                    && pa.iter().zip(pb.iter()).all(|(x, y)| {
                        self.values_equal(&x.borrow().clone(), &y.borrow().clone())
                            .unwrap_or(false)
                    })
            }
            (Value::Str(a), Value::Str(b)) => *a.borrow() == *b.borrow(),
            (Value::Bool(a), Value::Bool(b)) => a == b,
            _ => {
                if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
                    left.as_float() == right.as_float()
                } else {
                    left.as_int() == right.as_int()
                }
            }
        })
    }

    // ---- lookups ------------------------------------------------------------

    fn find_machine_by_name(&self, name: &str) -> Option<&'program Machine> {
        self.program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
    }

    fn find_state(&self, machine: &Machine, name: &str) -> Option<&State> {
        self.program
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == name)
    }

    fn find_data_by_name(&self, name: &str) -> Option<&'program DataDefinition> {
        self.program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)
    }
}

/// The canonical Console host-boundary method names the interpreter drives directly.
fn is_canonical_host_method(name: &str) -> bool {
    matches!(
        name,
        "write" | "write_line" | "write_error" | "write_error_line" | "read_line" | "exit_process"
    )
}

/// Reinterpret an i64 at an integer primitive's width, sign- or zero-extending back to i64
/// so the value carries the same numeric meaning the target type would observe. `u8` 250
/// stays 250; `i8` 250 wraps to -6; `u32` of a negative becomes its 32-bit unsigned value.
fn wrap_to_width(raw: i64, ty: PrimitiveType) -> i64 {
    match ty {
        PrimitiveType::I8 => raw as i8 as i64,
        PrimitiveType::U8 => raw as u8 as i64,
        PrimitiveType::I16 => raw as i16 as i64,
        PrimitiveType::U16 => raw as u16 as i64,
        PrimitiveType::I32 => raw as i32 as i64,
        PrimitiveType::U32 => raw as u32 as i64,
        // 64-bit and pointer-width types keep the full value (unsigned reinterpretation of a
        // u64 is still represented by the same bit pattern in i64).
        PrimitiveType::I64
        | PrimitiveType::U64
        | PrimitiveType::Isize
        | PrimitiveType::Usize => raw,
        // Non-integer primitives do not reach this path.
        PrimitiveType::Bool
        | PrimitiveType::F32
        | PrimitiveType::F64
        | PrimitiveType::String => raw,
    }
}

/// What a satisfied transition decided to do next.
enum TransitionDecision {
    Terminal,
    SelfTarget,
    Value(Value),
    Named {
        state_name: String,
        machine: Machine,
        instance: Cell,
        args: Vec<Cell>,
    },
}

// `Frame::locals` needs interior mutability so `let` bindings can be added while the
// frame is shared by `&`. Wrap the map in a RefCell.
impl Frame {
    fn get(&self, name: &str) -> Option<Cell> {
        self.locals_ref().borrow().get(name).cloned()
    }

    fn bind(&self, name: &str, cell: Cell) {
        self.locals_ref().borrow_mut().insert(name.to_owned(), cell);
    }

    fn locals_ref(&self) -> &RefCell<BTreeMap<String, Cell>> {
        &self.locals
    }
}
