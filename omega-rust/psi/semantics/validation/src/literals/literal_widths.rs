//! D14 literal-width gate: which positions may hold a u64-magnitude literal.
//!
//! Literals are anonymous payloads with no parse-time ceiling (see
//! `numerics::literals::IntegerLiteral`), so a u64-magnitude spelling like
//! `18446744073709551615` PARSES. Every consumer reads literals through the
//! i64 value window (`value_i64()`) and defers/degrades on `None`; the backend
//! write path additionally materializes full 8-byte patterns via `bits_u64()`.
//! That is only sound because THIS gate guarantees an oversize literal reaches
//! exactly the positions that handle it:
//!
//! - **Accepted (fire C):** the direct RHS of an assignment whose target's
//!   declared primitive is u64-classed (`u64`/`usize`/`addr`) -- an 8-byte
//!   slot, so the two's-complement bit pattern the write path emits is the
//!   value.
//! - **Accepted (fire D):** a struct-literal FIELD value whose declared field
//!   type is u64-classed (`Duration { seconds: 18446744073709551615, ... }`)
//!   -- same 8-byte-slot argument; the construction write cascade reads
//!   literals through the same bits-capable resolvers. (The interval side already agrees: an oversize-positive literal's
//!   honest over-approximation `[i64::MAX, +inf)` fits only u64-classed
//!   target ranges. The gate must stay PRECISE regardless -- a `u32 in
//!   Wrapping` target bypasses the interval store-check entirely, and only
//!   this gate stands between such a slot and a silent truncation.)
//! - **Accepted (fire E, 2026-07-11n):** a LET initializer into a local whose
//!   declared type is u64-classed -- same 8-byte-slot argument; the frame
//!   write path reads through the bits-capable static resolver.
//! - **Accepted (fire F, 2026-07-11n):** an EQUALITY (`==`/`!=` ONLY) guard
//!   leg whose other operand is a u64-classed place, at any depth of the
//!   guard's boolean structure (the multi-arm desugar nests the spelled
//!   compare inside `(subject) == true`). Ordering stays refused: the
//!   compare the encoder emits is SIGNED, and a bit-pattern u64 under it is
//!   sign-blind.
//! - **Accepted (fire G, 2026-07-11, math roster N2):** literals inside
//!   CONTRACT FACT positions (`requires`/`ensures` facts, `decreases`
//!   measures) at ANY magnitude -- contracts never lower to runtime bytes;
//!   their one consumer is the proof engine, which reads literals exactly
//!   (`value_bignum`, N2 bignum coefficients). This is the position where
//!   the old i64 window silently downgraded provable facts.
//! - **Everything else** (arithmetic operands, ordering guards,
//!   call/transition arguments, narrower or signed targets): one clear
//!   error.
//!
//! Growing acceptance to a new position = extend `u64_blessed_literals` AND
//! the consumer that materializes it, in the SAME change.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::PrimitiveType;

pub(crate) fn validate_literal_widths(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let blessed = u64_blessed_literals(program);
    for (handle, node) in program.expression_table.expression_entries() {
        if let ExpressionNode::Integer(literal) = node
            && literal.value_i64().is_none()
            && !blessed.contains(&handle)
        {
            diagnostics.push(Diagnostic::error(format!(
                "integer literal `{literal}` exceeds the i64 range; only a direct \
                 assignment into a `u64`/`usize`/`addr` place accepts a u64-magnitude \
                 literal so far (bind it to such a place first, or wait for this \
                 position's typed lowering -- TASKS_TIME.md D14)"
            )));
        }
    }
}

/// The u64-magnitude literals sitting in an ACCEPTED position: the direct RHS
/// of an assignment to a u64-classed place, or a struct-literal field whose
/// declared field type is u64-classed.
fn u64_blessed_literals(program: &TypedTrees) -> Vec<ExpressionHandle> {
    // A handful of entries at most -- a Vec beats hashing arena handles.
    let mut blessed = Vec::new();
    super::integer_landing::append_destination_literals(program, &mut blessed);

    // Struct-literal fields (position-independent: wherever the literal is
    // constructed, the field slot's declared type is what matters).
    for (_, node) in program.expression_table.expression_entries() {
        let ExpressionNode::StructLiteral(literal) = node else {
            continue;
        };
        let Some(data_definition) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == literal.type_name.as_str())
        else {
            continue;
        };
        for field in program.expression_table.struct_fields(literal.fields) {
            let ExpressionNode::Integer(value) = program.expression_table.expression(field.value)
            else {
                continue;
            };
            if value.value_i64().is_some() || value.value_u64().is_none() {
                continue;
            }
            let Some(field_type) = crate::struct_literals::construction_field_type(
                program,
                data_definition,
                literal.case_name.as_ref().map(|name| name.as_str()),
                field.name.as_str(),
            ) else {
                continue;
            };
            let Some(unwrapped) = crate::places::unwrapped_type_reference(program, field_type)
            else {
                continue;
            };
            let Some(primitive) = program.primitive_type_reference(unwrapped) else {
                continue;
            };
            if matches!(primitive, PrimitiveType::U64 | PrimitiveType::Addr) {
                blessed.push(field.value);
            }
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        if oversize_literal(program, assignment.value)
                            && place_is_u64_classed(program, machine, state, assignment.target)
                        {
                            blessed.push(assignment.value);
                        }
                    }
                    // Fire E (2026-07-11n): a LET initializer into a local whose
                    // DECLARED type is u64-classed -- same 8-byte-slot argument
                    // as the assignment fire; the frame-slot write path reads
                    // literals through the bits-capable static resolver.
                    StatementNode::LocalData(local) => {
                        if !oversize_literal(program, local.initial_value) {
                            continue;
                        }
                        let Some(unwrapped) =
                            crate::places::unwrapped_type_reference(program, local.type_reference)
                        else {
                            continue;
                        };
                        if primitive_is_u64_classed(program, unwrapped) {
                            blessed.push(local.initial_value);
                        }
                    }
                    // Fire F (2026-07-11n): an EQUALITY compare (== / != ONLY --
                    // ordering under two's-complement bit patterns is
                    // sign-blind and stays refused) in a transition GUARD whose
                    // other operand is a u64-classed place. The guard's static
                    // compare is an 8-byte bit-pattern equality on both
                    // engines, so the bit-cast IS the value.
                    StatementNode::Transition(transition) => {
                        if let typed_trees::statement::TransitionGuardNode::When(guard) =
                            transition.guard
                        {
                            bless_equality_guard_literals(
                                program,
                                machine,
                                state,
                                guard,
                                &mut blessed,
                            );
                        }
                        // Fire H (2026-07-16, the CR3 remaining face): a
                        // TRANSITION ARGUMENT into a u64-classed target-state
                        // parameter -- the arg IS a delivery into that
                        // declared slot (the same law the F2c float stamping
                        // rides), and the frame-slot arg writer already reads
                        // literals through the bits-capable static resolver.
                        // Same-machine Named targets only, receiver filtered.
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            let typed_trees::statement::TransitionTargetNode::Named {
                                path,
                                arguments,
                                ..
                            } = program.statement_table.transition_target(target)
                            else {
                                continue;
                            };
                            let target_members =
                                program.statement_table.name_path_members(path.members);
                            let [target_name] = target_members else {
                                continue;
                            };
                            let Some(target_state) = program
                                .machine_states(machine)
                                .iter()
                                .find(|candidate| candidate.name.as_str() == target_name.as_str())
                            else {
                                continue;
                            };
                            let parameters = program
                                .state_parameters(target_state)
                                .iter()
                                .filter(|parameter| !parameter.is_self);
                            let argument_handles =
                                program.statement_table.expression_handles(*arguments);
                            for (parameter, argument) in
                                parameters.zip(argument_handles.iter().copied())
                            {
                                if !oversize_literal(program, argument) {
                                    continue;
                                }
                                let Some(unwrapped) = crate::places::unwrapped_type_reference(
                                    program,
                                    parameter.type_reference,
                                ) else {
                                    continue;
                                };
                                if primitive_is_u64_classed(program, unwrapped) {
                                    blessed.push(argument);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Fire G: contract facts and decreases measures live in the proof
        // domain only; the exact engine is their sole reader.
        for contract in program.machine_contracts(machine) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let typed_trees::domain::ProofFact::Expression(expression) = fact {
                    bless_fact_literals(program, *expression, &mut blessed);
                }
            }
        }
        if let Some(measures) =
            typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
        {
            for measure in measures {
                bless_fact_literals(program, measure, &mut blessed);
            }
        }
    }
    blessed
}

/// Fire G's walk: every integer literal under a fact expression, any
/// magnitude (beyond-u64 included -- the engine is exact).
fn bless_fact_literals(
    program: &TypedTrees,
    expression: ExpressionHandle,
    blessed: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            if literal.value_i64().is_none() {
                blessed.push(expression);
            }
        }
        ExpressionNode::Binary(binary) => {
            bless_fact_literals(program, binary.left, blessed);
            bless_fact_literals(program, binary.right, blessed);
        }
        ExpressionNode::Unary(unary) => {
            bless_fact_literals(program, unary.operand, blessed);
        }
        ExpressionNode::Borrow(inner) => {
            bless_fact_literals(program, inner.target, blessed);
        }
        ExpressionNode::Call(call) => {
            for argument in program.expression_table.expression_handles(call.arguments) {
                bless_fact_literals(program, *argument, blessed);
            }
        }
        _ => {}
    }
}

fn oversize_literal(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            literal.value_i64().is_none() && literal.value_u64().is_some()
        }
        _ => false,
    }
}

fn primitive_is_u64_classed(
    program: &TypedTrees,
    unwrapped: typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(
        program.primitive_type_reference(unwrapped),
        Some(PrimitiveType::U64 | PrimitiveType::Addr)
    )
}

fn place_is_u64_classed(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    place: ExpressionHandle,
) -> bool {
    let Some(declared) =
        crate::places::declared_place_type_raw(program, machine, Some(state), place)
    else {
        return false;
    };
    let Some(unwrapped) = crate::places::unwrapped_type_reference(program, declared) else {
        return false;
    };
    primitive_is_u64_classed(program, unwrapped)
}

/// Fire F's walk: descend a guard expression's `&&`/`||` conjunctions and
/// bless oversize literals sitting in `==`/`!=` legs whose OTHER operand is
/// a u64-classed place.
fn bless_equality_guard_literals(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    guard: ExpressionHandle,
    blessed: &mut Vec<ExpressionHandle>,
) {
    if !guard.is_valid() {
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return;
    };
    use typed_trees::expression::BinaryOperator;
    match binary.operator {
        BinaryOperator::And | BinaryOperator::Or => {
            bless_equality_guard_literals(program, machine, state, binary.left, blessed);
            bless_equality_guard_literals(program, machine, state, binary.right, blessed);
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            for (literal, other) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if oversize_literal(program, literal)
                    && place_is_u64_classed(program, machine, state, other)
                {
                    blessed.push(literal);
                }
            }
            // The multi-arm desugar wraps an arm's guard as
            // `(subject) == true` -- the spelled compare NESTS inside an
            // equality leg. Recurse so `self.stored == <u64 literal>` is
            // found at any depth of the boolean structure.
            bless_equality_guard_literals(program, machine, state, binary.left, blessed);
            bless_equality_guard_literals(program, machine, state, binary.right, blessed);
        }
        _ => {}
    }
}
