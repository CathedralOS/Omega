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

pub(crate) fn run(checked: &CheckedTrees, stdin: &[u8]) -> InterpretOutcome {
    let mut evaluator = Evaluator::new(checked, stdin);
    match evaluator.run_entry() {
        Ok(()) => {
            // Reached a terminal transition without an explicit exit_process.
            InterpretOutcome::exited(0, evaluator.stdout)
        }
        Err(Halt::Exit(code)) => InterpretOutcome::exited(code, evaluator.stdout),
        Err(Halt::Unsupported(message)) | Err(Halt::Trap(message)) => {
            InterpretOutcome::error(message, evaluator.stdout)
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
}

struct Evaluator<'program> {
    program: &'program CheckedTrees,
    stdout: Vec<u8>,
    #[allow(dead_code)]
    stdin: &'program [u8],
    steps: u64,
}

impl<'program> Evaluator<'program> {
    fn new(program: &'program CheckedTrees, stdin: &'program [u8]) -> Self {
        Self {
            program,
            stdout: Vec::new(),
            stdin,
            steps: 0,
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
        self.run_state_in_machine(entry_machine, entry_state_name, instance, Vec::new())
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
                };
                self.eval_expression(owned.initial_value, &frame)?
            } else {
                self.default_for_type(owned.type_reference)
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

            // Nested `data` record? Recurse to a sub-Struct holding its own defaults.
            if let Some(nested) = self.field_nested_data(field.type_reference) {
                let mut nested_fields = BTreeMap::new();
                self.populate_data_fields(nested, &mut nested_fields)?;
                fields.insert(
                    name,
                    Value::Struct {
                        type_symbol: nested.symbol,
                        type_name: nested.name.as_str().to_owned(),
                        fields: nested_fields,
                    }
                    .cell(),
                );
                continue;
            }

            let value = if field.initial_value.is_valid() {
                let frame = Frame {
                    locals: RefCell::new(BTreeMap::new()),
                    self_cell: Value::Unit.cell(),
                };
                self.eval_expression(field.initial_value, &frame)?
            } else {
                self.default_for_type(field.type_reference)
            };
            fields.insert(name, value.cell());
        }
        Ok(())
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
                // Only RECORDS (have fields) instantiate as nested structs; enums don't.
                self.program
                    .data_members(data)
                    .iter()
                    .any(|member| matches!(member, DataMember::Field(_)))
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

    /// Run a state by name within `machine` on the given `self` instance. `args` are the
    /// already-evaluated argument cells bound positionally to the state's non-self params.
    /// Returns Ok on a terminal/named-without-target completion; the returned value (for
    /// value transitions) is delivered via the call path, not here.
    fn run_state_in_machine(
        &mut self,
        machine: &Machine,
        state_name: &str,
        instance: Cell,
        args: Vec<Cell>,
    ) -> EvalResult<()> {
        let _ = self.run_state_collect(machine, state_name, instance, args)?;
        Ok(())
    }

    /// Run a state; returns the value produced by a `Value` transition target, if any.
    fn run_state_collect(
        &mut self,
        machine: &Machine,
        state_name: &str,
        instance: Cell,
        args: Vec<Cell>,
    ) -> EvalResult<Option<Value>> {
        let mut current_state = state_name.to_owned();
        let mut current_args = args;

        loop {
            self.tick()?;
            let state = self
                .find_state(machine, &current_state)
                .ok_or_else(|| Halt::Unsupported(format!("unknown state `{current_state}`")))?
                .clone();

            let frame = self.bind_frame(&state, Rc::clone(&instance), &current_args)?;

            // Execute statements, watching for the first satisfied transition.
            let mut next: Option<TransitionDecision> = None;
            for statement in self.program.statement_table.statements(state.statement_nodes) {
                let statement = statement.clone();
                match &statement {
                    StatementNode::Transition(transition) => {
                        if let Some(decision) = self.eval_transition(transition, &frame)? {
                            next = Some(decision);
                            break;
                        }
                    }
                    other => {
                        self.exec_statement(other, &frame)?;
                    }
                }
            }

            match next {
                None => return Ok(None),
                Some(TransitionDecision::Value(value)) => return Ok(Some(value)),
                Some(TransitionDecision::Terminal) => return Ok(None),
                Some(TransitionDecision::SelfTarget) => {
                    // Re-run the same state (rare; guard against infinite loops via budget).
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

    /// Bind a state's parameters (skipping `self`) to the positional argument cells.
    fn bind_frame(
        &self,
        state: &State,
        self_cell: Cell,
        args: &[Cell],
    ) -> EvalResult<Frame> {
        let mut locals = BTreeMap::new();
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
        })
    }

    // ---- statements ---------------------------------------------------------

    fn exec_statement(&mut self, statement: &StatementNode, frame: &Frame) -> EvalResult<()> {
        self.tick()?;
        match statement {
            StatementNode::Assignment(assignment) => {
                let value = self.eval_expression(assignment.value, frame)?;
                let target = self.resolve_place(assignment.target, frame)?;
                *target.borrow_mut() = value;
                Ok(())
            }
            StatementNode::LocalData(local) => {
                let value = if local.initial_value.is_valid() {
                    self.eval_expression(local.initial_value, frame)?
                } else {
                    self.default_for_type(local.type_reference)
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
                let value = self.eval_expression(expression, frame)?;
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

        // A state call on `self`'s machine group: target machine is `Main::<target>`.
        let target = call.target.as_str();
        let machine = self
            .find_machine_for_call(target, frame)
            .ok_or_else(|| Halt::Unsupported(format!("unknown call target `{target}`")))?;

        let mut args = Vec::new();
        for argument in self.program.statement_table.expression_handles(call.arguments) {
            args.push(self.eval_argument(*argument, frame)?);
        }

        let instance = Rc::clone(&frame.self_cell);
        let entry_state = self
            .machine_entry_state_name(&machine)
            .ok_or_else(|| Halt::Unsupported(format!("call target `{target}` has no state")))?;
        self.run_state_collect(&machine, &entry_state, instance, args)
            .map(|value| value.unwrap_or(Value::Unit))
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
        // Otherwise a machine that simply has a state named `target`.
        self.program
            .machines()
            .iter()
            .find(|machine| self.find_state(machine, target).is_some())
            .cloned()
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
                // &mut place -> alias the SAME cell (this is the whole point of the oracle).
                let cell = self.resolve_place(*inner, frame)?;
                Ok(cell)
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
            "write" | "write_line" => {
                if let Some(first) = arguments.first() {
                    let value = self.eval_expression(*first, frame)?;
                    match value {
                        Value::Str(text) => self.stdout.extend_from_slice(text.borrow().as_bytes()),
                        other => {
                            return unsupported(format!(
                                "host write of non-string value {other:?}"
                            ));
                        }
                    }
                }
                if target == "write_line" {
                    self.stdout.push(b'\n');
                }
                Ok(Some(Value::Unit))
            }
            other => unsupported(format!("host boundary call `{other}` not yet supported")),
        }
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
                                    return self.is_boundary_trait_symbol(type_symbol);
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
            ExpressionNode::Cast(_) => unsupported("cast expressions not yet supported"),
            ExpressionNode::Indexed(_) => unsupported("indexed expressions not yet supported"),
            ExpressionNode::ArrayLiteral(_) => unsupported("array literals not yet supported"),
            ExpressionNode::Range(_) => unsupported("range expressions not yet supported"),
            ExpressionNode::StructLiteral(_) => {
                unsupported("struct literals not yet supported")
            }
        }
    }

    fn eval_call_expression(
        &mut self,
        _handle: ExpressionHandle,
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

        // A state/machine value call: target machine `<group>::<target>`, run to its
        // terminal Value transition.
        let machine = self
            .find_machine_for_call(target, frame)
            .ok_or_else(|| Halt::Unsupported(format!("unknown value-call target `{target}`")))?;
        let mut args = Vec::new();
        for argument in self.program.expression_table.expression_handles(call.arguments) {
            args.push(self.eval_call_expression_argument(*argument, frame)?);
        }
        let instance = Rc::clone(&frame.self_cell);
        let entry_state = self
            .machine_entry_state_name(&machine)
            .ok_or_else(|| Halt::Unsupported(format!("value-call `{target}` has no state")))?;
        self.run_state_collect(&machine, &entry_state, instance, args)
            .map(|value| value.unwrap_or(Value::Unit))
    }

    fn eval_call_expression_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Cell> {
        match self.program.expression_table.expression(argument) {
            ExpressionNode::Mutable(inner) => Ok(self.resolve_place(*inner, frame)?),
            _ => Ok(self.eval_expression(argument, frame)?.cell()),
        }
    }

    fn eval_name(&mut self, path: &TableNamePath, frame: &Frame) -> EvalResult<Value> {
        // An enum value reference (`CellId::R02` / `Command::Look`) resolves to an Enum.
        let members = self.program.expression_table.name_path_members(path.members);
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

    fn field_cell(&self, container: &Cell, field: &str) -> EvalResult<Cell> {
        let container = self.deref_cell(Rc::clone(container));
        let borrowed = container.borrow();
        match &*borrowed {
            Value::Struct { fields, type_name, .. } => fields
                .get(field)
                .cloned()
                .ok_or_else(|| Halt::Trap(format!("no field `{field}` on `{type_name}`"))),
            other => trap(format!("cannot read field `{field}` of {other:?}")),
        }
    }

    // ---- operators ----------------------------------------------------------

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
            (Value::Enum { variant_name: a }, Value::Enum { variant_name: b }) => a == b,
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
