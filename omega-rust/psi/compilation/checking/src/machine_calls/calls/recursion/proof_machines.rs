//! Proof-machine recursive-call decrease and citation validation.
//!
//! Runtime tail-position legality remains with the parent. This child owns the
//! structural decrease judgment, recursive-call collection, guard provenance,
//! and sub-state descent closure. Computed proof-data predecessors use the same
//! structural judge as contract entailment; a separate spelling-based citation
//! matcher could borrow guarantees from a different declaration and bypass the
//! judge's premise checks.

use super::is_self_entry_call;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

mod slices;

/// The arithmetic predecessor bridge for proof recursion over an integer
/// measure.  A call nested in the TAKEN value of `transition n > 0` may pass
/// `n - 1`: the guard proves subtraction cannot underflow and the result is
/// strictly below `n`.  Keep this association syntactic and local -- an
/// unrelated positive guard elsewhere in the state must not license the
/// call.
fn guarded_integer_predecessor_call(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    entry_name: &str,
    measure_position: usize,
    argument: ExpressionHandle,
    measure_symbol: symbols::SymbolHandle,
    measure_name: Option<&typed_trees::name::Identifier>,
) -> bool {
    let ExpressionNode::Binary(predecessor) = program.expression_table.expression(argument) else {
        return false;
    };
    if predecessor.operator != typed_trees::expression::BinaryOperator::Subtract
        || !expression_names_measure(program, predecessor.left, measure_symbol, measure_name)
        || !matches!(
            program.expression_table.expression(predecessor.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
        )
    {
        return false;
    }

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return false;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                return false;
            };
            let guard = match program.expression_table.expression(guard) {
                ExpressionNode::Binary(wrapper)
                    if wrapper.operator == typed_trees::expression::BinaryOperator::Equal
                        && matches!(
                            program.expression_table.expression(wrapper.right),
                            ExpressionNode::Boolean(true)
                        ) =>
                {
                    wrapper.left
                }
                _ => guard,
            };
            let ExpressionNode::Binary(positive) = program.expression_table.expression(guard)
            else {
                return false;
            };
            if positive.operator != typed_trees::expression::BinaryOperator::Greater
                || !expression_names_measure(program, positive.left, measure_symbol, measure_name)
                || !matches!(
                    program.expression_table.expression(positive.right),
                    ExpressionNode::Integer(literal) if literal.value_i64() == Some(0)
                )
            {
                return false;
            }

            let TransitionTargetNode::Value(value) =
                program.statement_table.transition_target(transition.target)
            else {
                return false;
            };
            let mut calls = Vec::new();
            collect_self_entry_call_arguments(program, entry_name, *value, &mut calls);
            calls.into_iter().any(|arguments| {
                arguments
                    .get(measure_position)
                    .is_some_and(|candidate| *candidate == argument)
            })
        })
}

fn expression_names_measure(
    program: &TypedTrees,
    expression: ExpressionHandle,
    measure_symbol: symbols::SymbolHandle,
    measure_name: Option<&typed_trees::name::Identifier>,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    path.symbol == measure_symbol
        || measure_name.is_some_and(|name| {
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|member| member.as_str() == name.as_str())
        })
}

/// A direct self-application whose ranked operand is a strict constructor
/// subterm on every incoming state edge. This query deliberately excludes
/// cited decrease and contract normalization: an earlier call may use the
/// resulting IH to establish its own premises, so its guarantees cannot also
/// be the reason that induction is licensed.
pub(crate) fn proof_call_has_structural_descent(
    program: &TypedTrees,
    machine: &Machine,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    if call.target_symbol != entry.symbol || call.receiver.is_valid() {
        return false;
    }
    let Some(subjects) = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
    else {
        return false;
    };
    let [subject] = subjects.as_slice() else {
        return false;
    };
    let ExpressionNode::Name(measure) = program.expression_table.expression(*subject) else {
        return false;
    };
    if !measure.symbol.is_valid() {
        return false;
    }
    let parameters = program.state_parameters(entry);
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.symbol == measure.symbol)
    else {
        return false;
    };
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != parameters.len() {
        return false;
    }
    arguments.get(position).is_some_and(|argument| {
        strict_subterm_of_measure(program, *argument, measure.symbol, None)
            || matches!(program.expression_table.expression(*argument), ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && program.machine_states(machine)[1..].iter().any(|state| {
                        program.state_parameters(state).iter()
                            .any(|parameter| parameter.symbol == path.symbol)
                    })
                    && substate_parameter_descends(program, machine, *argument, measure.symbol, None))
    })
}

pub(crate) fn validate_proof_machine_recursion(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str());
    let mut self_calls: Vec<Vec<ExpressionHandle>> = Vec::new();
    if let StatementNode::Call(call) = statement {
        let receiver = program.statement_table.name_path_members(call.receiver);
        let entry_symbol = program
            .machine_states(machine)
            .first()
            .map(|entry| entry.symbol)
            .unwrap_or_else(symbols::SymbolHandle::invalid);
        let selects_self_entry = (call.target_symbol.is_valid()
            && call.target_symbol == entry_symbol)
            || (!call.target_symbol.is_valid() && call.target.as_str() == entry_name);
        if selects_self_entry
            && (receiver.is_empty() || matches!(receiver, [only] if only.is_self_receiver()))
        {
            // A resultless citation and an explicitly discarded value call
            // are both StatementNode::Call. The call itself is the induction
            // edge; looking only through its argument expressions would let
            // `theorem(n);` cite its own ensures without proving descent.
            self_calls.push(
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .to_vec(),
            );
        }
    }
    // Runtime tail normalization changes an exact self call into an entry
    // backedge. Proof-only classification happens later, so retain that edge
    // in this same induction check: constant-stack execution is not descent.
    if let StatementNode::Transition(transition) = statement {
        let entry_symbol = program
            .machine_states(machine)
            .first()
            .map(|entry| entry.symbol);
        for target in [transition.target, transition.continuation] {
            if !target.is_valid() {
                continue;
            }
            if let TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(target)
                && path.symbol.is_valid()
                && (path.symbol == machine.symbol || Some(path.symbol) == entry_symbol)
            {
                self_calls.push(
                    program
                        .statement_table
                        .expression_handles(*arguments)
                        .to_vec(),
                );
            }
        }
    }
    for root in statement_expression_roots(program, statement) {
        collect_self_entry_call_arguments(program, entry_name, root, &mut self_calls);
    }
    if self_calls.is_empty() {
        return;
    }

    let Some(subjects) = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}` needs a single structural measure: declare \
             `terminates by <param>;` naming one proof-data parameter -- a \
             cycle without a measure is an unproven termination claim (measured \
             recursion, both strata)",
            machine.name,
        )));
        return;
    };
    let [subject] = subjects.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}` needs a single structural measure: declare \
             `terminates by <param>;` naming one proof-data parameter -- a \
             cycle without a measure is an unproven termination claim (measured \
             recursion, both strata)",
            machine.name,
        )));
        return;
    };
    let ExpressionNode::Name(measure_path) = program.expression_table.expression(*subject) else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}`: the structural measure must be a bare \
             parameter name (rung 1); compound measures over proof data are not \
             proven yet",
            machine.name,
        )));
        return;
    };
    let measure_symbol = measure_path.symbol;
    let measure_name = program
        .expression_table
        .name_path_members(measure_path.members)
        .first()
        .cloned();
    // The measure names an ENTRY parameter; its POSITION is where every
    // self-call's argument must descend.
    let Some(measure_position) = program.machine_states(machine).first().and_then(|entry| {
        program
            .state_parameters(entry)
            .iter()
            .position(|parameter| {
                (parameter.symbol.is_valid() && parameter.symbol == measure_symbol)
                    || measure_name
                        .as_ref()
                        .is_some_and(|name| parameter.name.as_str() == name.as_str())
            })
    }) else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}`: the measure must name an entry parameter",
            machine.name,
        )));
        return;
    };

    // The slice-tail decrease probe scans the whole program once for its
    // bound index; share that scan across every self-call argument here.
    let mut bound_lookup = None;
    for arguments in self_calls {
        let argument = arguments.get(measure_position).copied();
        let descends = argument.is_some_and(|argument| {
            strict_subterm_of_measure(program, argument, measure_symbol, measure_name.as_ref())
                || substate_parameter_descends(
                    program,
                    machine,
                    argument,
                    measure_symbol,
                    measure_name.as_ref(),
                )
                || program.machine_states(machine).first().is_some_and(|entry| {
                    crate::proof_contracts::contract_entailment::proof_edge_strict_decrease_judged(
                        program,
                        machine,
                        state,
                        argument,
                        &program.state_parameters(entry)[measure_position],
                    )
                })
                || guarded_integer_predecessor_call(
                    program,
                    state,
                    entry_name,
                    measure_position,
                    argument,
                    measure_symbol,
                    measure_name.as_ref(),
                )
                || slices::guarded_slice_tail_call(
                    program,
                    machine,
                    state,
                    statement,
                    argument,
                    measure_position,
                    &mut bound_lookup,
                )
        });
        if !descends {
            diagnostics.push(Diagnostic::error(format!(
                "`{entry_name}(..)` cannot prove the measure `{}` structurally \
                 decreases at this self-call: the call does not prove a strict \
                 predecessor of ranking subject `{}`. Pass a case-payload subterm \
                 (`Nat::Succ {{ prev }} -> .. {entry_name}(prev)`) or, for an integer \
                 measure, `n - 1` in the taken value of its dominating `n > 0` arm; \
                 a slice-length measure admits `items[1..]` under its own nonempty guard",
                measure_name
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("<measure>"),
                measure_name
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("<measure>"),
            )));
        }
    }
}

/// The root expression handles a statement can carry (guard subjects, arm
/// arguments, terminal values, initializers, call arguments).
fn statement_expression_roots(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    match statement {
        StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => Vec::new(),
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec(),
        StatementNode::Assignment(assignment) => vec![assignment.target, assignment.value],
        StatementNode::LocalData(local_data) => vec![local_data.initial_value],
        StatementNode::Expression(expression) => vec![*expression],
        StatementNode::Transition(transition) => {
            let mut roots = Vec::new();
            if let TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        roots.extend(
                            program
                                .statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .copied(),
                        );
                    }
                    TransitionTargetNode::Value(expression) => roots.push(*expression),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
            roots
        }
    }
}

/// Collect the ARGUMENT spans of every self-entry call in this tree.
fn collect_self_entry_call_arguments(
    program: &TypedTrees,
    entry_name: &str,
    expression: ExpressionHandle,
    found: &mut Vec<Vec<ExpressionHandle>>,
) {
    if !expression.is_valid() {
        return;
    }
    let recurse = |handle: ExpressionHandle, found: &mut Vec<Vec<ExpressionHandle>>| {
        collect_self_entry_call_arguments(program, entry_name, handle, found);
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            recurse(dispatch.subject, found);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    recurse(pattern, found);
                }
                recurse(arm.value, found);
            }
        }
        ExpressionNode::Atomic(atomic) => recurse(atomic.value, found),
        ExpressionNode::Call(call) => {
            if is_self_entry_call(program, entry_name, call) {
                found.push(
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec(),
                );
            }
            recurse(call.receiver, found);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, found);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left, found);
            recurse(binary.right, found);
        }
        ExpressionNode::Cast(cast) => recurse(cast.value, found),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection, found);
            recurse(indexed.index, found);
        }
        ExpressionNode::Member(member) => recurse(member.receiver, found),
        ExpressionNode::Borrow(inner) => recurse(inner.target, found),
        ExpressionNode::Range(range) => {
            recurse(range.start, found);
            recurse(range.end, found);
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand, found),
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                recurse(*item, found);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                recurse(field.value, found);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// A STRICT subterm of the measure: a Member chain of one or more hops whose
/// root Name is the measure parameter (symbol match, name fallback). One hop
/// = one constructor consumed (`n.prev`); depth composes for free.
/// N3 rung 2 -- descent THROUGH a sub-state parameter: a self-call inside a
/// per-arm sub-proof passes the sub-state's own parameter (`step_case(prev,
/// b)` bound it at the entry arm from the measure's case payload, so inside
/// `step_case` the recursion argument is the bare name `prev`). The
/// parameter counts as strictly descending iff EVERY Named transition into
/// its state passes either a strict-subterm Member read of the measure or a
/// previously proven descending sub-state parameter. The latter closure is
/// what preserves provenance through proof choreography (`prev` forwarded
/// through two case-analysis states before the recursive call). Provenance is
/// still over ALL binding sites, so a single non-descending entry poisons the
/// parameter and cycles without a direct strict-subterm seed prove nothing.
///
/// Matching is symbol-first (precise); the name fallback additionally
/// refuses when any local or assignment anywhere in the machine shares the
/// name, so a shadowing binding cannot launder a non-descending value
/// through a descending parameter's name.
fn substate_parameter_descends(
    program: &TypedTrees,
    machine: &Machine,
    argument: ExpressionHandle,
    measure_symbol: symbols::SymbolHandle,
    measure_name: Option<&Identifier>,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(argument) else {
        return false;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return false;
    };
    let states = program.machine_states(machine);
    if states.len() < 2 {
        return false;
    }
    // Every sub-state parameter this name could denote (symbol-first).
    let mut candidates: Vec<(&typed_trees::state::State, usize)> = Vec::new();
    let mut symbol_matched = false;
    for state in &states[1..] {
        for (position, parameter) in program.state_parameters(state).iter().enumerate() {
            let by_symbol = path.symbol.is_valid()
                && parameter.symbol.is_valid()
                && parameter.symbol == path.symbol;
            let by_name = parameter.name.as_str() == name.as_str();
            if by_symbol {
                symbol_matched = true;
                candidates.push((state, position));
            } else if by_name {
                candidates.push((state, position));
            }
        }
    }
    if candidates.is_empty() {
        return false;
    }
    if !symbol_matched {
        // Name-only matching: refuse if anything else in the machine binds
        // this name.
        for state in states {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::LocalData(local_data)
                        if local_data.name.as_str() == name.as_str() =>
                    {
                        return false;
                    }
                    StatementNode::Assignment(_) => return false,
                    _ => {}
                }
            }
        }
    }
    let parameters: Vec<(&typed_trees::state::State, usize, symbols::SymbolHandle)> = states[1..]
        .iter()
        .flat_map(|state| {
            program
                .state_parameters(state)
                .iter()
                .enumerate()
                .map(move |(position, parameter)| (state, position, parameter.symbol))
        })
        .collect();
    let mut descending: Vec<symbols::SymbolHandle> = Vec::new();
    loop {
        let mut gained = Vec::new();
        for (sub_state, position, parameter_symbol) in &parameters {
            if !parameter_symbol.is_valid()
                || descending.iter().any(|known| known == parameter_symbol)
            {
                continue;
            }
            let mut binding_sites = 0usize;
            let mut all_descend = true;
            for source in states {
                for statement in program.statement_table.statements(source.statement_nodes) {
                    let StatementNode::Transition(transition) = statement else {
                        continue;
                    };
                    for target_handle in [transition.target, transition.continuation] {
                        if !target_handle.is_valid() {
                            continue;
                        }
                        let TransitionTargetNode::Named {
                            path, arguments, ..
                        } = program.statement_table.transition_target(target_handle)
                        else {
                            continue;
                        };
                        let [target_name] = program.statement_table.name_path_members(path.members)
                        else {
                            all_descend = false;
                            continue;
                        };
                        if target_name.as_str() != sub_state.name.as_str() {
                            continue;
                        }
                        binding_sites += 1;
                        let handles = program.statement_table.expression_handles(*arguments);
                        let Some(bound) = handles.get(*position) else {
                            all_descend = false;
                            continue;
                        };
                        let forwarded = match program.expression_table.expression(*bound) {
                            ExpressionNode::Name(path) if path.symbol.is_valid() => {
                                descending.contains(&path.symbol)
                            }
                            _ => false,
                        };
                        if !forwarded
                            && !strict_subterm_of_measure(
                                program,
                                *bound,
                                measure_symbol,
                                measure_name,
                            )
                        {
                            all_descend = false;
                        }
                    }
                }
            }
            if binding_sites > 0 && all_descend {
                gained.push(*parameter_symbol);
            }
        }
        if gained.is_empty() {
            break;
        }
        descending.extend(gained);
    }
    candidates.iter().all(|(state, position)| {
        program
            .state_parameters(state)
            .get(*position)
            .is_some_and(|parameter| {
                parameter.symbol.is_valid() && descending.contains(&parameter.symbol)
            })
    })
}

fn strict_subterm_of_measure(
    program: &TypedTrees,
    expression: ExpressionHandle,
    measure_symbol: symbols::SymbolHandle,
    measure_name: Option<&Identifier>,
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return false;
    };
    let mut root = member.receiver;
    loop {
        match program.expression_table.expression(root) {
            ExpressionNode::Member(inner) => root = inner.receiver,
            ExpressionNode::Name(path) => {
                return (path.symbol.is_valid() && path.symbol == measure_symbol)
                    || measure_name.is_some_and(|name| {
                        matches!(
                            program.expression_table.name_path_members(path.members),
                            [only] if only.as_str() == name.as_str()
                        )
                    });
            }
            _ => return false,
        }
    }
}
