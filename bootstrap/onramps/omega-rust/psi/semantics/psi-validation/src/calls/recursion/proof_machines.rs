//! Proof-machine recursive-call decrease and citation validation.
//!
//! Runtime tail-position legality remains with the parent. This child owns the
//! structural/cited decrease judgment, recursive-call collection, substitution
//! matching, guard provenance, and sub-state descent closure.

use super::is_self_entry_call;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// PROOF-MACHINE recursion legality (math roster N2d gateway). A free
/// machine over proof-only data emits no runtime code, so the tail-only
/// rule does not apply (no frame survives anything) -- structural recursion
/// in ANY position is the induction the measure licenses. What DOES apply,
/// both strata: a cycle without a measure is an unproven termination claim.
/// Every self-call must be measured, and rung 1 proves the decrease
/// STRUCTURALLY: the argument in the measure's parameter position is a
/// case-payload SUBTERM of the measure -- a pattern binding like
/// `transition n { Nat::Succ { prev } -> .. double(prev) .. }` lowers
/// `prev` to the case-tagged member read `n.prev`, so the test is "a Member
/// chain (>= 1 hop) rooted at the measure parameter". Anything else refuses
/// with the shape named; the arithmetic bridge (n > 0 => n == Succ(n - 1))
/// is the recorded follow-on.
/// COMPUTED-SUBJECT strict decrease by CITATION (N4 order rung, slice a2,
/// design-ruled 2026-07-17): a recursive proof machine whose measure
/// argument is an application (`mod(saturating_sub(a, b), b)` at measure `a`) proves
/// the strict edge by citing a lemma in the SAME state whose instantiated
/// ensures is EXACTLY the monus-order strict fact
/// `saturating_sub(Succ(ARG), MEASURE) == Zero` (`ARG < MEASURE`). The cited lemma's
/// REQUIRES discharge syntactically at the site against (i) the citing
/// machine's own requires and (ii) the incoming-arm case equations (every
/// transition arm targeting this state whose guard cases subject S into
/// constructor C contributes the fact `S == C` -- the mod shape's Zero arm
/// over `saturating_sub(b, a)` contributes exactly
/// `saturating_sub(b, a) == Zero`, the `b <= a`
/// premise). Everything is structural expression equality -- no arithmetic
/// is re-derived here; the lemma carries the mathematics.
fn cited_strict_decrease(
    program: &TypedTrees,
    machine: &Machine,
    state: &psi_typed_trees::state::State,
    argument: ExpressionHandle,
    measure_name: Option<&psi_typed_trees::name::Identifier>,
) -> bool {
    let Some(measure_name) = measure_name else {
        return false;
    };
    // A let-bound edge argument (`let next = saturating_sub(a, b); .. mod(next, b)` --
    // the value-call face forces the hoist) resolves through its
    // initializer before matching.
    let argument = resolve_state_local(program, state, argument);
    // Site facts: the citing machine's requires + incoming-arm equations.
    let mut site_facts: Vec<SiteFact> = Vec::new();
    for contract in program.machine_contracts(machine) {
        if !matches!(
            contract.kind,
            psi_typed_trees::signature::SignatureContractKind::Requires
        ) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let psi_typed_trees::domain::ProofFact::Expression(expression) = fact
                && let ExpressionNode::Binary(binary) =
                    program.expression_table.expression(*expression)
                && binary.operator == psi_typed_trees::expression::BinaryOperator::Equal
            {
                site_facts.push(SiteFact {
                    left: binary.left,
                    right: binary.right,
                });
            }
        }
    }
    for other in program.machine_states(machine) {
        for statement in program.statement_table.statements(other.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            let psi_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard
            else {
                continue;
            };
            if !transition.target.is_valid() {
                continue;
            }
            let TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target(transition.target)
            else {
                continue;
            };
            let targets_state = program
                .statement_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|name| name.as_str() == state.name.as_str());
            if !targets_state {
                continue;
            }
            if let ExpressionNode::Binary(binary) = program.expression_table.expression(guard)
                && binary.operator == psi_typed_trees::expression::BinaryOperator::Equal
            {
                site_facts.push(SiteFact {
                    left: binary.left,
                    right: binary.right,
                });
            }
        }
    }

    // Each citation in THIS state: a bare statement call to a free machine.
    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::Call(call) = statement else {
            continue;
        };
        let receiver_members = program.statement_table.name_path_members(call.receiver);
        if !receiver_members.is_empty() {
            continue;
        }
        let Some(callee) = program.machines().iter().find(|candidate| {
            candidate.attached_data.is_none()
                && candidate
                    .name
                    .as_str()
                    .rsplit("::")
                    .next()
                    .unwrap_or(candidate.name.as_str())
                    == call.target.as_str()
        }) else {
            continue;
        };
        let Some(entry) = program.machine_states(callee).first() else {
            continue;
        };
        let parameters = program.state_parameters(entry);
        let citation_arguments = program.statement_table.expression_handles(call.arguments);
        if parameters.len() != citation_arguments.len() {
            continue;
        }
        let map: Vec<(&str, ExpressionHandle)> = parameters
            .iter()
            .zip(citation_arguments)
            .map(|(parameter, argument)| (parameter.name.as_str(), *argument))
            .collect();

        // The callee's requires must all discharge against the site facts.
        let mut requires_ok = true;
        let mut ensures_matches = false;
        for contract in program.machine_contracts(callee) {
            match contract.kind {
                psi_typed_trees::signature::SignatureContractKind::Requires => {
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact
                        else {
                            requires_ok = false;
                            continue;
                        };
                        let ExpressionNode::Binary(binary) =
                            program.expression_table.expression(*expression)
                        else {
                            requires_ok = false;
                            continue;
                        };
                        if binary.operator != psi_typed_trees::expression::BinaryOperator::Equal {
                            requires_ok = false;
                            continue;
                        }
                        let discharged = site_facts.iter().any(|site| {
                            substituted_expression_equals(program, binary.left, &map, site.left)
                                && substituted_expression_equals(
                                    program,
                                    binary.right,
                                    &map,
                                    site.right,
                                )
                        });
                        if !discharged {
                            requires_ok = false;
                        }
                    }
                }
                psi_typed_trees::signature::SignatureContractKind::Ensures => {
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact
                        else {
                            continue;
                        };
                        if ensures_is_strict_decrease(
                            program,
                            *expression,
                            &map,
                            argument,
                            measure_name,
                        ) {
                            ensures_matches = true;
                        }
                    }
                }
                _ => {}
            }
        }
        if requires_ok && ensures_matches {
            return true;
        }
    }
    false
}

/// Resolve a single-name expression through a same-state `let` binding to
/// its initializer (one hop; anything else returns the input unchanged).
fn resolve_state_local(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return expression;
    };
    let [only] = program.expression_table.name_path_members(path.members) else {
        return expression;
    };
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let StatementNode::LocalData(local) = statement
            && local.name.as_str() == only.as_str()
            && local.initial_value.is_valid()
        {
            return local.initial_value;
        }
    }
    expression
}

struct SiteFact {
    left: ExpressionHandle,
    right: ExpressionHandle,
}

/// The callee's ensures fact, instantiated at the citation's arguments,
/// must be exactly `saturating_sub(Succ { prev: ARG }, MEASURE) == Nat::Zero`.
fn ensures_is_strict_decrease(
    program: &TypedTrees,
    fact: ExpressionHandle,
    map: &[(&str, ExpressionHandle)],
    edge_argument: ExpressionHandle,
    measure_name: &psi_typed_trees::name::Identifier,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return false;
    };
    if binary.operator != psi_typed_trees::expression::BinaryOperator::Equal {
        return false;
    }
    // RHS: the Zero constructor.
    if !expression_is_nat_zero(program, binary.right) {
        return false;
    }
    // LHS: saturating_sub(Succ { prev: X }, M).
    let ExpressionNode::Call(sub_call) = program.expression_table.expression(binary.left) else {
        return false;
    };
    if sub_call.target.as_str() != "saturating_sub" {
        return false;
    }
    let sub_arguments = program
        .expression_table
        .expression_handles(sub_call.arguments);
    let [succ_side, measure_side] = sub_arguments else {
        return false;
    };
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(*succ_side)
    else {
        return false;
    };
    if literal.case_name.as_ref().map(|name| name.as_str()) != Some("Succ") {
        return false;
    }
    let fields = program.expression_table.struct_fields(literal.fields);
    let [field] = fields else {
        return false;
    };
    if field.name.as_str() != "prev" {
        return false;
    }
    substituted_expression_equals(program, field.value, map, edge_argument)
        && substituted_name_is(program, *measure_side, map, measure_name.as_str())
}

fn expression_is_nat_zero(program: &TypedTrees, handle: ExpressionHandle) -> bool {
    match program.expression_table.expression(handle) {
        ExpressionNode::StructLiteral(literal) => {
            literal.case_name.as_ref().map(|name| name.as_str()) == Some("Zero")
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty()
        }
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == "Zero"),
        _ => false,
    }
}

/// Does the callee-side expression, with callee parameters substituted by
/// the citation's argument expressions, resolve to the single NAME `name`?
fn substituted_name_is(
    program: &TypedTrees,
    callee_side: ExpressionHandle,
    map: &[(&str, ExpressionHandle)],
    name: &str,
) -> bool {
    if let ExpressionNode::Name(path) = program.expression_table.expression(callee_side)
        && let [only] = program.expression_table.name_path_members(path.members)
    {
        if let Some((_, substituted)) = map.iter().find(|(param, _)| *param == only.as_str()) {
            return expression_is_single_name(program, *substituted, name);
        }
        return only.as_str() == name;
    }
    false
}

fn expression_is_single_name(program: &TypedTrees, handle: ExpressionHandle, name: &str) -> bool {
    matches!(
        program.expression_table.expression(handle),
        ExpressionNode::Name(path)
            if matches!(
                program.expression_table.name_path_members(path.members),
                [only] if only.as_str() == name
            )
    )
}

/// Structural equality: callee-side expression under the citation's
/// parameter substitution vs a caller-side expression. Names compare by
/// their (single) member spelling; parenthesization is transparent in the
/// table form. Conservative: unhandled node kinds compare false.
fn substituted_expression_equals(
    program: &TypedTrees,
    callee_side: ExpressionHandle,
    map: &[(&str, ExpressionHandle)],
    caller_side: ExpressionHandle,
) -> bool {
    // A callee-side parameter name substitutes to its citation argument
    // and the comparison continues caller-vs-caller.
    if let ExpressionNode::Name(path) = program.expression_table.expression(callee_side)
        && let [only] = program.expression_table.name_path_members(path.members)
        && let Some((_, substituted)) = map.iter().find(|(param, _)| *param == only.as_str())
    {
        return caller_expressions_equal(program, *substituted, caller_side);
    }
    match (
        program.expression_table.expression(callee_side),
        program.expression_table.expression(caller_side),
    ) {
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            let left = program.expression_table.name_path_members(left.members);
            let right = program.expression_table.name_path_members(right.members);
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(l, r)| l.as_str() == r.as_str())
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            if left.target.as_str() != right.target.as_str() {
                return false;
            }
            let left_arguments = program.expression_table.expression_handles(left.arguments);
            let right_arguments = program.expression_table.expression_handles(right.arguments);
            left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(l, r)| substituted_expression_equals(program, *l, map, *r))
        }
        (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
            if left.type_name.as_str() != right.type_name.as_str()
                || left.case_name.as_ref().map(|name| name.as_str())
                    != right.case_name.as_ref().map(|name| name.as_str())
            {
                return false;
            }
            let left_fields = program.expression_table.struct_fields(left.fields);
            let right_fields = program.expression_table.struct_fields(right.fields);
            left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields).all(|(l, r)| {
                    l.name.as_str() == r.name.as_str()
                        && substituted_expression_equals(program, l.value, map, r.value)
                })
        }
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => {
            left.value_i64() == right.value_i64()
        }
        _ => false,
    }
}

/// Caller-space structural equality (no substitution on either side).
fn caller_expressions_equal(
    program: &TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    substituted_expression_equals(program, left, &[], right)
}

/// The arithmetic predecessor bridge for proof recursion over an integer
/// measure.  A call nested in the TAKEN value of `transition n > 0` may pass
/// `n - 1`: the guard proves subtraction cannot underflow and the result is
/// strictly below `n`.  Keep this association syntactic and local -- an
/// unrelated positive guard elsewhere in the state must not license the
/// call.
fn guarded_integer_predecessor_call(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    entry_name: &str,
    measure_position: usize,
    argument: ExpressionHandle,
    measure_symbol: psi_symbols::SymbolHandle,
    measure_name: Option<&psi_typed_trees::name::Identifier>,
) -> bool {
    let ExpressionNode::Binary(predecessor) = program.expression_table.expression(argument) else {
        return false;
    };
    if predecessor.operator != psi_typed_trees::expression::BinaryOperator::Subtract
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
                    if wrapper.operator == psi_typed_trees::expression::BinaryOperator::Equal
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
            if positive.operator != psi_typed_trees::expression::BinaryOperator::Greater
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
    measure_symbol: psi_symbols::SymbolHandle,
    measure_name: Option<&psi_typed_trees::name::Identifier>,
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

pub(crate) fn validate_proof_machine_recursion(
    program: &TypedTrees,
    machine: &Machine,
    state: &psi_typed_trees::state::State,
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
            .unwrap_or_else(psi_symbols::SymbolHandle::invalid);
        let selects_self_entry = (call.target_symbol.is_valid()
            && call.target_symbol == entry_symbol)
            || (!call.target_symbol.is_valid() && call.target.as_str() == entry_name);
        if selects_self_entry
            && (receiver.is_empty() || matches!(receiver, [only] if only.as_str() == "self"))
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
    for root in statement_expression_roots(program, statement) {
        collect_self_entry_call_arguments(program, entry_name, root, &mut self_calls);
    }
    if self_calls.is_empty() {
        return;
    }

    let Some(subjects) =
        psi_typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
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
                || cited_strict_decrease(program, machine, state, argument, measure_name.as_ref())
                || measure_name.as_ref().is_some_and(|name| {
                    crate::contract_entailment::proof_edge_strict_decrease_judged(
                        program,
                        machine,
                        state,
                        argument,
                        name.as_str(),
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
        });
        if !descends {
            diagnostics.push(Diagnostic::error(format!(
                "`{entry_name}(..)` cannot prove the measure `{}` structurally \
                 decreases at this self-call: the call does not prove a strict \
                 predecessor of ranking subject `{}`. Pass a case-payload subterm \
                 (`Nat::Succ {{ prev }} -> .. {entry_name}(prev)`) or, for an integer \
                 measure, `n - 1` in the taken value of its dominating `n > 0` arm",
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
        StatementNode::AssemblyFact(_) => Vec::new(),
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
    measure_symbol: psi_symbols::SymbolHandle,
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
    let mut candidates: Vec<(&psi_typed_trees::state::State, usize)> = Vec::new();
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
    let parameters: Vec<(
        &psi_typed_trees::state::State,
        usize,
        psi_symbols::SymbolHandle,
    )> = states[1..]
        .iter()
        .flat_map(|state| {
            program
                .state_parameters(state)
                .iter()
                .enumerate()
                .map(move |(position, parameter)| (state, position, parameter.symbol))
        })
        .collect();
    let mut descending: Vec<psi_symbols::SymbolHandle> = Vec::new();
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
                                descending.iter().any(|known| *known == path.symbol)
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
                parameter.symbol.is_valid()
                    && descending.iter().any(|known| *known == parameter.symbol)
            })
    })
}

fn strict_subterm_of_measure(
    program: &TypedTrees,
    expression: ExpressionHandle,
    measure_symbol: psi_symbols::SymbolHandle,
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
