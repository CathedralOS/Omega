//! Path exploration of one checked predicate body over refinable values.
//!
//! Every integer the explorer can learn about is an *atom*: one element of an
//! argument array, or one scalar argument. A path carries one [`IntegerSet`]
//! per atom. A comparison whose outcome the current sets do not decide splits
//! the path on exactly one atom, so each fork refines that atom and makes the
//! comparison decided. Paths that arrive at the same state with the same
//! bindings are joined atom by atom. Joining over-approximates the reachable
//! values, so both answers the explorer gives stay sound: "every path
//! returns true" can only become unprovable, and the values joined over
//! `true` returns can only grow.
//!
//! The body fragment is deliberately small: state statements are transitions
//! and immutable locals (checking hoists a computed element read into one),
//! guards and results are builtin comparisons, conjunctions, disjunctions and
//! negations over literals, parameters and in-bounds element reads, and index
//! arithmetic is Exact addition or subtraction of known values. Named targets
//! move to another state of the same machine; a result that calls another
//! admitted predicate tail-calls it. Anything else stands down; a complete
//! exploration inside the work budget is itself the termination witness for
//! these exact arguments.

use super::integer_sets::IntegerSet;
use crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning;
use std::collections::VecDeque;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};
use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Visits plus splits one exploration may spend before it stands down.
const WORK_BUDGET: usize = 1 << 14;
/// Arrays wider than this are not unfolded element by element.
const MAXIMUM_EXTENT: usize = 4096;
/// A set with more ranges than this stops being worth refining.
const MAXIMUM_RANGES: usize = 64;

/// What one exploration concluded, as atom sets joined over every path that
/// returned the given result. `None` means no explored path returned it.
pub(crate) struct Outcomes {
    pub(crate) when_true: Option<Vec<IntegerSet>>,
    pub(crate) when_false: Option<Vec<IntegerSet>>,
}

/// How one parameter of a frame reads its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Binding {
    /// A fixed array whose elements are the atoms `first..first + length`.
    Array {
        first: usize,
        length: usize,
        element_type: TypeReferenceHandle,
    },
    /// A refinable integer.
    Atom {
        atom: usize,
        carrier: TypeReferenceHandle,
    },
    /// An integer already known exactly.
    Constant {
        value: i128,
        carrier: TypeReferenceHandle,
    },
    Boolean(bool),
    /// A parameter outside the fragment; reading it stands down.
    Unreadable,
}

/// The integer carrier of a type, when it is a builtin fixed integer.
pub(crate) fn integer_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(i128, i128)> {
    let bounds = match program.primitive_type_reference(type_reference)? {
        PrimitiveType::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
        PrimitiveType::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
        PrimitiveType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveType::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        PrimitiveType::U8 => (0, i128::from(u8::MAX)),
        PrimitiveType::U16 => (0, i128::from(u16::MAX)),
        PrimitiveType::U32 => (0, i128::from(u32::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => (0, i128::from(u64::MAX)),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    Some(bounds)
}

/// The shape of a parameter or field the explorer can bind: a fixed integer
/// array (owned or behind a shared reference) or an integer scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueShape {
    Array {
        length: usize,
        element_type: TypeReferenceHandle,
    },
    Integer(TypeReferenceHandle),
    Boolean,
}

pub(crate) fn value_shape(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<ValueShape> {
    let mut reference = type_reference;
    for _ in 0..8 {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } if *access == language_core::ReferenceAccess::Shared => {
                reference = *referee;
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                return (*length <= MAXIMUM_EXTENT
                    && integer_carrier(program, *element_type).is_some())
                .then_some(ValueShape::Array {
                    length: *length,
                    element_type: *element_type,
                });
            }
            _ => {
                return match program.primitive_type_reference(reference)? {
                    PrimitiveType::Bool => Some(ValueShape::Boolean),
                    _ => {
                        integer_carrier(program, reference).map(|_| ValueShape::Integer(reference))
                    }
                };
            }
        }
    }
    None
}

/// The index of a free predicate machine the explorer may enter, selected by
/// its machine symbol or its entry state symbol. `entered_by_call` admits
/// arrival requirements, which the caller's checked body already proved.
pub(crate) fn admitted_predicate_machine(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    entered_by_call: bool,
) -> Option<usize> {
    if !symbol.is_valid() {
        return None;
    }
    let (index, machine) = program.machines().iter().enumerate().find(|(_, machine)| {
        machine.symbol == symbol
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == symbol)
    })?;
    let entry = program.machine_states(machine).first()?;
    let pure = machine.attached_data.is_none()
        && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        && machine.body_is_present
        && !machine.structural_type_equations_pending
        && !machine.suspends
        && !machine.blocks
        && machine.type_parameters.is_empty()
        && program.machine_owned_data(machine).is_empty()
        && program
            .service_reach_rows
            .services(machine.service_reach_row)
            .is_empty();
    let returns_boolean =
        program.primitive_type_reference(entry.return_type) == Some(PrimitiveType::Bool);
    let unconditional = entered_by_call
        || program.machine_contracts(machine).iter().all(|contract| {
            !matches!(
                contract.kind,
                typed_trees::signature::SignatureContractKind::Requires
            )
        });
    let parameters_readable = program.machine_states(machine).iter().all(|state| {
        program
            .state_parameters(state)
            .iter()
            .all(|parameter| !parameter.is_mutable && !parameter.is_self)
    });
    (pure && returns_boolean && unconditional && parameters_readable).then_some(index)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrameKey {
    machine: usize,
    state: usize,
    bindings: Vec<Binding>,
}

enum Exit {
    Result(bool, Vec<IntegerSet>),
    Enter(FrameKey, Vec<IntegerSet>),
}

/// Why evaluation stopped before producing a value.
pub(crate) enum Stop {
    /// The value depends on `atom`: refine it to each part and retry.
    Split {
        atom: usize,
        parts: [IntegerSet; 2],
    },
    Unsupported,
}

#[derive(Debug, Clone)]
pub(crate) enum Value {
    Boolean(bool),
    Integer {
        set: IntegerSet,
        atom: Option<usize>,
        carrier: Option<TypeReferenceHandle>,
    },
    Array {
        first: usize,
        length: usize,
        element_type: TypeReferenceHandle,
    },
}

/// One activation's scope: the state whose parameters `bindings` bind, in
/// declaration order, plus the immutable locals declared so far on this path.
pub(crate) struct Frame<'scope> {
    pub(crate) machine: &'scope Machine,
    pub(crate) state: &'scope State,
    pub(crate) bindings: &'scope [Binding],
    pub(crate) locals: &'scope [(symbols::SymbolHandle, Binding)],
}

/// Explore `machine` entered at its first state with `bindings` over the
/// initial atom sets.
pub(crate) fn explore(
    program: &TypedTrees,
    machine: usize,
    bindings: Vec<Binding>,
    atoms: Vec<IntegerSet>,
) -> Option<Outcomes> {
    let mut remaining = WORK_BUDGET;
    let mut frames: Vec<(FrameKey, Vec<IntegerSet>)> = Vec::new();
    let mut pending: VecDeque<usize> = VecDeque::new();
    let mut outcomes = Outcomes {
        when_true: None,
        when_false: None,
    };
    frames.push((
        FrameKey {
            machine,
            state: 0,
            bindings,
        },
        atoms,
    ));
    pending.push_back(0);
    while let Some(frame_index) = pending.pop_front() {
        let (key, atoms) = frames[frame_index].clone();
        for exit in visit(program, &key, atoms, &mut remaining)? {
            match exit {
                Exit::Result(result, atoms) => {
                    let joined = if result {
                        &mut outcomes.when_true
                    } else {
                        &mut outcomes.when_false
                    };
                    *joined = Some(match joined.take() {
                        None => atoms,
                        Some(existing) => join(&existing, &atoms)?,
                    });
                }
                Exit::Enter(target, atoms) => {
                    remaining = remaining.checked_sub(1)?;
                    match frames.iter().position(|(existing, _)| *existing == target) {
                        None => {
                            frames.push((target, atoms));
                            pending.push_back(frames.len() - 1);
                        }
                        Some(position) => {
                            let joined = join(&frames[position].1, &atoms)?;
                            if joined != frames[position].1 {
                                frames[position].1 = joined;
                                if !pending.contains(&position) {
                                    pending.push_back(position);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Some(outcomes)
}

fn join(left: &[IntegerSet], right: &[IntegerSet]) -> Option<Vec<IntegerSet>> {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let joined = left.union(right);
            (joined.range_count() <= MAXIMUM_RANGES).then_some(joined)
        })
        .collect()
}

/// Run one state's statements under one joined entry. Splits fork inside the
/// visit; only results and state entries leave it.
fn visit(
    program: &TypedTrees,
    key: &FrameKey,
    atoms: Vec<IntegerSet>,
    remaining: &mut usize,
) -> Option<Vec<Exit>> {
    let machine = program.machines().get(key.machine)?;
    let state = program.machine_states(machine).get(key.state)?;
    if program.state_parameters(state).len() != key.bindings.len() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut exits = Vec::new();
    let mut stack = vec![(0usize, atoms, Vec::new())];
    while let Some((index, atoms, locals)) = stack.pop() {
        *remaining = remaining.checked_sub(1)?;
        let frame = Frame {
            machine,
            state,
            bindings: &key.bindings,
            locals: &locals,
        };
        let step = match statements.get(index)? {
            // Checking hoists computed operands such as a symbolic element
            // read into an immutable local; it binds like an argument.
            StatementNode::LocalData(local)
                if !local.is_mutable
                    && local.relevance == language_core::BindingRelevance::Relevant =>
            {
                match argument_binding(
                    program,
                    &frame,
                    &atoms,
                    local.initial_value,
                    local.type_reference,
                ) {
                    Ok(binding) => {
                        let mut declared = locals.clone();
                        declared.push((local.symbol, binding));
                        stack.push((index + 1, atoms, declared));
                        continue;
                    }
                    Err(stop) => Err(stop),
                }
            }
            StatementNode::Transition(transition)
                if transition.exit == TransitionExit::Ordinary
                    && !transition.continuation.is_valid() =>
            {
                let taken = match transition.guard {
                    TransitionGuardNode::Always => Ok(true),
                    TransitionGuardNode::When(guard) => {
                        evaluate_boolean(program, &frame, &atoms, guard)
                    }
                };
                match taken {
                    Ok(false) => {
                        stack.push((index + 1, atoms, locals));
                        continue;
                    }
                    Ok(true) => target_exit(program, &frame, key, &atoms, transition.target),
                    Err(stop) => Err(stop),
                }
            }
            _ => return None,
        };
        match step {
            Ok(exit) => exits.push(exit),
            Err(Stop::Split { atom, parts }) => {
                for part in parts {
                    let mut refined = atoms.clone();
                    refined[atom] = part;
                    stack.push((index, refined, locals.clone()));
                }
            }
            Err(Stop::Unsupported) => return None,
        }
    }
    Some(exits)
}

fn target_exit(
    program: &TypedTrees,
    frame: &Frame<'_>,
    key: &FrameKey,
    atoms: &[IntegerSet],
    target: TransitionTargetHandle,
) -> Result<Exit, Stop> {
    if !target.is_valid() {
        return Err(Stop::Unsupported);
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Value(result) => {
            if let ExpressionNode::Call(call) = program.expression_table.expression(*result) {
                return tail_call_exit(program, frame, atoms, call);
            }
            let result = evaluate_boolean(program, frame, atoms, *result)?;
            Ok(Exit::Result(result, atoms.to_vec()))
        }
        TransitionTargetNode::Named {
            path,
            arguments,
            static_machine_parameter,
            evidence_arguments,
            ..
        } => {
            if static_machine_parameter.is_valid() || !evidence_arguments.is_empty() {
                return Err(Stop::Unsupported);
            }
            let (machine, state) =
                named_target(program, frame, key.machine, path.symbol).ok_or(Stop::Unsupported)?;
            let arguments = program.statement_table.expression_handles(*arguments);
            enter(program, frame, atoms, machine, state, arguments)
        }
        TransitionTargetNode::Terminal => Err(Stop::Unsupported),
    }
}

/// A result position that calls a predicate returns that call's result: the
/// call is a tail transition into the callee's entry.
fn tail_call_exit(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: &[IntegerSet],
    call: &typed_trees::expression::TableCallExpression,
) -> Result<Exit, Stop> {
    if call.receiver.is_valid()
        || call.static_machine_parameter.is_valid()
        || call.static_requirement_dispatch.is_some()
        || !call.machine_arguments.is_empty()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
        || call.layout_query.is_some()
        || !call.evidence_arguments.is_empty()
    {
        return Err(Stop::Unsupported);
    }
    let machine =
        admitted_predicate_machine(program, call.target_symbol, true).ok_or(Stop::Unsupported)?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    enter(program, frame, atoms, machine, 0, arguments)
}

/// Bind `arguments`, evaluated in the current frame, to the parameters of
/// the target state.
fn enter(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: &[IntegerSet],
    machine: usize,
    state: usize,
    arguments: &[ExpressionHandle],
) -> Result<Exit, Stop> {
    let target_machine = program.machines().get(machine).ok_or(Stop::Unsupported)?;
    let target_state = program
        .machine_states(target_machine)
        .get(state)
        .ok_or(Stop::Unsupported)?;
    let parameters = program.state_parameters(target_state);
    if parameters.len() != arguments.len() {
        return Err(Stop::Unsupported);
    }
    let mut bindings = Vec::with_capacity(arguments.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        bindings.push(argument_binding(
            program,
            frame,
            atoms,
            *argument,
            parameter.type_reference,
        )?);
    }
    Ok(Exit::Enter(
        FrameKey {
            machine,
            state,
            bindings,
        },
        atoms.to_vec(),
    ))
}

/// A sibling state of the current machine, or the entry of another admitted
/// predicate reached by a tail call.
fn named_target(
    program: &TypedTrees,
    frame: &Frame<'_>,
    current_machine: usize,
    symbol: symbols::SymbolHandle,
) -> Option<(usize, usize)> {
    if let Some(state) = program
        .machine_states(frame.machine)
        .iter()
        .position(|state| state.symbol == symbol)
        .filter(|position| *position != 0)
    {
        return Some((current_machine, state));
    }
    admitted_predicate_machine(program, symbol, true).map(|machine| (machine, 0))
}

fn argument_binding(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: &[IntegerSet],
    argument: ExpressionHandle,
    parameter_type: TypeReferenceHandle,
) -> Result<Binding, Stop> {
    let shape = value_shape(program, parameter_type).ok_or(Stop::Unsupported)?;
    match (evaluate(program, frame, atoms, argument)?, shape) {
        (
            Value::Array {
                first,
                length,
                element_type,
            },
            ValueShape::Array {
                length: expected, ..
            },
        ) if length == expected => Ok(Binding::Array {
            first,
            length,
            element_type,
        }),
        (Value::Integer { set, atom, .. }, ValueShape::Integer(carrier)) => {
            let (low, high) = integer_carrier(program, carrier).ok_or(Stop::Unsupported)?;
            if !set.is_subset_of(&IntegerSet::range(low, high)) {
                return Err(Stop::Unsupported);
            }
            match (set.single_value(), atom) {
                (Some(value), _) => Ok(Binding::Constant { value, carrier }),
                (None, Some(atom)) => Ok(Binding::Atom { atom, carrier }),
                (None, None) => Err(Stop::Unsupported),
            }
        }
        (Value::Boolean(value), ValueShape::Boolean) => Ok(Binding::Boolean(value)),
        _ => Err(Stop::Unsupported),
    }
}

pub(crate) fn evaluate_boolean(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: &[IntegerSet],
    expression: ExpressionHandle,
) -> Result<bool, Stop> {
    match evaluate(program, frame, atoms, expression)? {
        Value::Boolean(value) => Ok(value),
        _ => Err(Stop::Unsupported),
    }
}

pub(crate) fn evaluate(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: &[IntegerSet],
    expression: ExpressionHandle,
) -> Result<Value, Stop> {
    if !program.expression_table.expression_is_valid(expression) {
        return Err(Stop::Unsupported);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Ok(Value::Boolean(*value)),
        ExpressionNode::Integer(literal) => Ok(Value::Integer {
            set: IntegerSet::singleton(i128::from(literal.value_i64().ok_or(Stop::Unsupported)?)),
            atom: None,
            carrier: None,
        }),
        ExpressionNode::Name(path) => {
            if program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
                || !path.symbol.is_valid()
            {
                return Err(Stop::Unsupported);
            }
            let binding = match frame
                .locals
                .iter()
                .rev()
                .find(|(symbol, _)| *symbol == path.symbol)
            {
                Some((_, binding)) => binding,
                None => {
                    let position = program
                        .state_parameters(frame.state)
                        .iter()
                        .position(|parameter| parameter.symbol == path.symbol)
                        .ok_or(Stop::Unsupported)?;
                    frame.bindings.get(position).ok_or(Stop::Unsupported)?
                }
            };
            Ok(match binding {
                Binding::Array {
                    first,
                    length,
                    element_type,
                } => Value::Array {
                    first: *first,
                    length: *length,
                    element_type: *element_type,
                },
                Binding::Atom { atom, carrier } => Value::Integer {
                    set: atoms.get(*atom).ok_or(Stop::Unsupported)?.clone(),
                    atom: Some(*atom),
                    carrier: Some(*carrier),
                },
                Binding::Constant { value, carrier } => Value::Integer {
                    set: IntegerSet::singleton(*value),
                    atom: None,
                    carrier: Some(*carrier),
                },
                Binding::Boolean(value) => Value::Boolean(*value),
                Binding::Unreadable => return Err(Stop::Unsupported),
            })
        }
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Shared =>
        {
            match evaluate(program, frame, atoms, borrow.target)? {
                array @ Value::Array { .. } => Ok(array),
                _ => Err(Stop::Unsupported),
            }
        }
        ExpressionNode::Indexed(indexed) => {
            let Value::Array {
                first,
                length,
                element_type,
            } = evaluate(program, frame, atoms, indexed.collection)?
            else {
                return Err(Stop::Unsupported);
            };
            let Value::Integer { set, atom, .. } = evaluate(program, frame, atoms, indexed.index)?
            else {
                return Err(Stop::Unsupported);
            };
            let Some(position) = set.single_value() else {
                // A symbolic index is a case split over its possible values;
                // any value outside the extent leaves a fork unsupported.
                let (Some(atom), Some((low, _))) = (atom, set.bounds()) else {
                    return Err(Stop::Unsupported);
                };
                let low_part = IntegerSet::singleton(low);
                return Err(Stop::Split {
                    atom,
                    parts: [set.difference(&low_part), low_part],
                });
            };
            let element = usize::try_from(position)
                .ok()
                .filter(|element| *element < length)
                .ok_or(Stop::Unsupported)?;
            Ok(Value::Integer {
                set: atoms.get(first + element).ok_or(Stop::Unsupported)?.clone(),
                atom: Some(first + element),
                carrier: Some(element_type),
            })
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => Ok(
            Value::Boolean(!evaluate_boolean(program, frame, atoms, unary.operand)?),
        ),
        ExpressionNode::Binary(binary) => {
            evaluate_binary(program, frame, atoms, expression, binary)
        }
        _ => Err(Stop::Unsupported),
    }
}

fn evaluate_binary(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: &[IntegerSet],
    expression: ExpressionHandle,
    binary: &typed_trees::expression::TableBinaryExpression,
) -> Result<Value, Stop> {
    match binary.operator {
        // Neither logical connective has an overloadable spelling; both keep
        // their short-circuit order so a guarded operand is never evaluated
        // on the path its guard excludes.
        BinaryOperator::And => Ok(Value::Boolean(
            evaluate_boolean(program, frame, atoms, binary.left)?
                && evaluate_boolean(program, frame, atoms, binary.right)?,
        )),
        BinaryOperator::Or => Ok(Value::Boolean(
            evaluate_boolean(program, frame, atoms, binary.left)?
                || evaluate_boolean(program, frame, atoms, binary.right)?,
        )),
        _ => {
            if !has_builtin_binary_expression_meaning(
                program,
                frame.machine,
                Some(frame.state),
                expression,
            ) {
                return Err(Stop::Unsupported);
            }
            let left = evaluate(program, frame, atoms, binary.left)?;
            let right = evaluate(program, frame, atoms, binary.right)?;
            match binary.operator {
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual => compare(binary.operator, left, right),
                BinaryOperator::Add | BinaryOperator::Subtract => {
                    exact_index_arithmetic(program, binary.operator, left, right)
                }
                _ => Err(Stop::Unsupported),
            }
        }
    }
}

/// Builtin integer comparison is the relation between the operands' exact
/// mathematical values, whatever their carriers, so comparing sets of
/// mathematical integers is its meaning.
fn compare(operator: BinaryOperator, left: Value, right: Value) -> Result<Value, Stop> {
    match (left, right) {
        (Value::Boolean(left), Value::Boolean(right)) => match operator {
            BinaryOperator::Equal => Ok(Value::Boolean(left == right)),
            BinaryOperator::NotEqual => Ok(Value::Boolean(left != right)),
            _ => Err(Stop::Unsupported),
        },
        (
            Value::Integer {
                set: left,
                atom: left_atom,
                ..
            },
            Value::Integer {
                set: right,
                atom: right_atom,
                ..
            },
        ) => {
            if let Some(bound) = right.single_value() {
                return refine(operator, left, left_atom, bound);
            }
            if let Some(bound) = left.single_value() {
                return refine(mirrored(operator), right, right_atom, bound);
            }
            decided_by_bounds(operator, &left, &right)
                .map(Value::Boolean)
                .ok_or(Stop::Unsupported)
        }
        _ => Err(Stop::Unsupported),
    }
}

fn refine(
    operator: BinaryOperator,
    subject: IntegerSet,
    atom: Option<usize>,
    bound: i128,
) -> Result<Value, Stop> {
    let satisfying = IntegerSet::satisfying(operator, bound).ok_or(Stop::Unsupported)?;
    let holds = subject.intersect(&satisfying);
    let fails = subject.difference(&satisfying);
    if fails.is_empty() {
        return Ok(Value::Boolean(true));
    }
    if holds.is_empty() {
        return Ok(Value::Boolean(false));
    }
    let atom = atom.ok_or(Stop::Unsupported)?;
    if holds.range_count() > MAXIMUM_RANGES || fails.range_count() > MAXIMUM_RANGES {
        return Err(Stop::Unsupported);
    }
    Err(Stop::Split {
        atom,
        parts: [fails, holds],
    })
}

fn mirrored(operator: BinaryOperator) -> BinaryOperator {
    match operator {
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        other => other,
    }
}

fn decided_by_bounds(
    operator: BinaryOperator,
    left: &IntegerSet,
    right: &IntegerSet,
) -> Option<bool> {
    let (left_low, left_high) = left.bounds()?;
    let (right_low, right_high) = right.bounds()?;
    match operator {
        BinaryOperator::Less if left_high < right_low => Some(true),
        BinaryOperator::Less if left_low >= right_high => Some(false),
        BinaryOperator::LessOrEqual if left_high <= right_low => Some(true),
        BinaryOperator::LessOrEqual if left_low > right_high => Some(false),
        BinaryOperator::Greater => decided_by_bounds(BinaryOperator::Less, right, left),
        BinaryOperator::GreaterOrEqual => {
            decided_by_bounds(BinaryOperator::LessOrEqual, right, left)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual
            if left_high < right_low || right_high < left_low =>
        {
            Some(operator == BinaryOperator::NotEqual)
        }
        _ => None,
    }
}

/// Exact addition or subtraction of two known values, as used to step an
/// index. The result must fit the typed operand's Exact carrier; any other
/// policy, or an unknown operand, stands down.
fn exact_index_arithmetic(
    program: &TypedTrees,
    operator: BinaryOperator,
    left: Value,
    right: Value,
) -> Result<Value, Stop> {
    let (
        Value::Integer {
            set: left,
            carrier: left_carrier,
            ..
        },
        Value::Integer {
            set: right,
            carrier: right_carrier,
            ..
        },
    ) = (left, right)
    else {
        return Err(Stop::Unsupported);
    };
    let carrier = left_carrier.or(right_carrier).ok_or(Stop::Unsupported)?;
    if [left_carrier, right_carrier]
        .into_iter()
        .flatten()
        .any(|typed| {
            program.arithmetic_domain_for_type_reference(typed)
                != numerics::arithmetic::ArithmeticDomain::Exact
                || program.primitive_type_reference(typed)
                    != program.primitive_type_reference(carrier)
        })
    {
        return Err(Stop::Unsupported);
    }
    let (Some(left), Some(right)) = (left.single_value(), right.single_value()) else {
        return Err(Stop::Unsupported);
    };
    let value = match operator {
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        _ => None,
    }
    .ok_or(Stop::Unsupported)?;
    let (low, high) = integer_carrier(program, carrier).ok_or(Stop::Unsupported)?;
    if value < low || value > high {
        return Err(Stop::Unsupported);
    }
    Ok(Value::Integer {
        set: IntegerSet::singleton(value),
        atom: None,
        carrier: Some(carrier),
    })
}
