use super::{StateLocalStorage, StateMutation, StateStoragePlan};
use crate::StateStoragePlanningContext;
use crate::mutation_kind::{mutation_kind, mutation_lowering};
use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTableCapacity,
};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::name::Identifier;
use omega_checked_trees::statement::{
    StatementNode, StatementTable, TransitionGuardNode, TransitionTargetNode,
};
use omega_checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_core::symbols::SymbolHandle;
use omega_state_values::simplify_state_expression;
use std::sync::Arc;

pub fn build_state_storage_plan(
    program: &CheckedTrees,
    context: StateStoragePlanningContext,
) -> StateStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_storage_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(context),
        workers.handle(),
    )
}

pub fn build_state_storage_plan_owned(
    program: CheckedTrees,
    context: StateStoragePlanningContext,
) -> StateStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_storage_plan_with_workers(Arc::new(program), Arc::new(context), workers.handle())
}

pub fn build_state_storage_plan_with_workers(
    program: Arc<CheckedTrees>,
    context: Arc<StateStoragePlanningContext>,
    workers: WorkerPoolHandle,
) -> StateStoragePlan {
    if program.machines().is_empty() {
        return StateStoragePlan::default();
    }

    let machine_count = program.machines().len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines()
            .get(index)
            .expect("state-storage worker index should be in range");

        build_machine_state_storage_plan(&program, &context, machine)
    });

    let local_count = machine_plans
        .iter()
        .map(|machine_plan| machine_plan.locals.len())
        .sum();
    let mutation_count = machine_plans
        .iter()
        .map(|machine_plan| machine_plan.mutations.len())
        .sum();
    let expression_capacity = machine_plans.iter().fold(
        ExpressionTableCapacity::default(),
        |mut capacity, machine_plan| {
            capacity.saturating_add_assign(machine_plan.expressions.copy_capacity());
            capacity
        },
    );
    let mut plan =
        StateStoragePlan::with_capacities(local_count, mutation_count, expression_capacity);

    for machine_plan in machine_plans {
        let StateStoragePlan {
            expressions,
            invariant_names,
            locals,
            mutations,
            type_references,
        } = machine_plan;
        for local in locals.into_items() {
            let local_invariant_names = plan.invariant_names.insert_many(
                invariant_names
                    .span_or_empty(local.invariant_names)
                    .iter()
                    .cloned(),
            );
            let local_type_reference = plan.type_references.copy_from(
                &type_references,
                &expressions,
                &mut plan.expressions,
                local.type_reference,
            );
            let local_initial_value = if local.initial_value.is_valid() {
                plan.expressions.copy_from(&expressions, local.initial_value)
            } else {
                ExpressionHandle::invalid()
            };
            plan.locals.insert(StateLocalStorage {
                source_key: local.source_key,
                statement_index: local.statement_index,
                symbol: local.symbol,
                name: local.name,
                type_symbol: local.type_symbol,
                type_reference: local_type_reference,
                invariant_names: local_invariant_names,
                required: local.required,
                initial_value: local_initial_value,
            });
        }
        for mutation in mutations.into_items() {
            plan.mutations.append(StateMutation {
                source_key: mutation.source_key,
                statement_index: mutation.statement_index,
                target: plan.expressions.copy_from(&expressions, mutation.target),
                value: plan.expressions.copy_from(&expressions, mutation.value),
                mutation_kind: mutation.mutation_kind,
                lowering: mutation.lowering,
                required: mutation.required,
            });
        }
    }

    plan
}

fn build_machine_state_storage_plan(
    program: &CheckedTrees,
    context: &StateStoragePlanningContext,
    machine: &Machine,
) -> StateStoragePlan {
    let (local_capacity, mutation_capacity) = estimated_machine_storage_capacity(program, machine);
    let mut plan = StateStoragePlan::with_capacities(
        local_capacity,
        mutation_capacity,
        ExpressionTableCapacity {
            expressions: mutation_capacity.saturating_mul(2),
            ..ExpressionTableCapacity::default()
        },
    );

    for state in program.machine_states(machine) {
        let source_key = StateKey {
            machine: machine.symbol,
            state: state.symbol,
            segment_index: 0,
        };
        let required = context.state_is_required_by_key(source_key);
        let uses_runtime_flow = context.state_uses_runtime_flow_by_key(source_key);

        let statements = program.statement_table.statements(state.statement_nodes);

        for (statement_index, statement) in statements.iter().enumerate() {
            match statement {
                StatementNode::LocalData(local_data) => {
                    if !local_data_requires_storage(
                        &program.expression_table,
                        &program.statement_table,
                        statements,
                        statement_index,
                        local_data.symbol,
                        &local_data.name,
                        local_data.initial_value,
                        uses_runtime_flow || required,
                    ) && !local_referenced_in_other_machine_states(
                        program,
                        machine,
                        state.symbol,
                        local_data.symbol,
                        &local_data.name,
                    ) {
                        continue;
                    }
                    plan.locals.insert(StateLocalStorage {
                        source_key,
                        statement_index,
                        symbol: local_data.symbol,
                        name: local_data.name.clone(),
                        type_symbol: program.type_reference_symbol(local_data.type_reference),
                        type_reference: plan.type_references.copy_from(
                            &program.type_reference_table,
                            &program.expression_table,
                            &mut plan.expressions,
                            local_data.type_reference,
                        ),
                        invariant_names: append_type_reference_invariant_names(
                            program,
                            local_data.type_reference,
                            &mut plan.invariant_names,
                        ),
                        required,
                        initial_value: if local_data.initial_value.is_valid() {
                            plan.expressions
                                .copy_from(&program.expression_table, local_data.initial_value)
                        } else {
                            ExpressionHandle::invalid()
                        },
                    });
                }
                StatementNode::Assignment(assignment) => {
                    let target = plan
                        .expressions
                        .copy_from(&program.expression_table, assignment.target);
                    let value = if program
                        .expression_table
                        .expression_is_literal(assignment.value)
                        || (program
                            .expression_table
                            .expression_is_direct_place_path(assignment.value)
                            && !state_has_initialized_locals_before(
                                program,
                                state,
                                statement_index,
                            )) {
                        plan.expressions
                            .copy_from(&program.expression_table, assignment.value)
                    } else {
                        let simplified_value = simplify_state_expression(
                            program,
                            machine,
                            state,
                            statement_index,
                            &program.expression_table.to_tree(assignment.value),
                        );
                        plan.expressions.insert_tree(&simplified_value)
                    };
                    let mutation_kind = mutation_kind(
                        program,
                        context,
                        source_key,
                        &program.expression_table,
                        assignment.target,
                    );
                    plan.mutations.insert(StateMutation {
                        source_key,
                        statement_index,
                        target,
                        value,
                        mutation_kind,
                        lowering: mutation_lowering(
                            context,
                            source_key,
                            statement_index,
                            mutation_kind,
                        ),
                        required,
                    });
                }
                _ => {}
            }
        }
    }

    plan
}

fn estimated_machine_storage_capacity(program: &CheckedTrees, machine: &Machine) -> (usize, usize) {
    program
        .machine_states(machine)
        .iter()
        .fold((0usize, 0usize), |(locals, mutations), state| {
            let statements = program.statement_table.statements(state.statement_nodes);
            let state_locals = statements
                .iter()
                .filter(|statement| matches!(statement, StatementNode::LocalData(_)))
                .count();
            let state_mutations = statements
                .iter()
                .filter(|statement| matches!(statement, StatementNode::Assignment(_)))
                .count();

            (
                locals.saturating_add(state_locals),
                mutations.saturating_add(state_mutations),
            )
        })
}

fn state_has_initialized_locals_before(
    program: &CheckedTrees,
    state: &omega_checked_trees::state::State,
    statement_index: usize,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .any(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(local_data) if local_data.initial_value.is_valid()
            )
        })
}

fn local_data_requires_storage(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    local_symbol: SymbolHandle,
    local_name: &Identifier,
    initial_value: ExpressionHandle,
    uses_runtime_flow: bool,
) -> bool {
    if !initial_value.is_valid() {
        return true;
    }

    // A MUTATING-call initializer (`let seed = self.next_seed(&mut rng)`) must
    // keep its LocalStorage slot no matter how the local is consumed. The
    // runtime-bodies splice lays the callee's mutation operations out BETWEEN
    // the StateCall operation and the statement's LocalStorage operation, and
    // the call-result value selection is DEFERRED to that LocalStorage
    // operation so it reads the POST-mutation state. Eliding the slot (the
    // consumed-by-a-later-initializer optimization below) removes the
    // deferral's landing operation, so the call-result copy emitted at the
    // StateCall and delivered the PRE-call value.
    if expression_contains_mutating_call(expressions, initial_value) {
        return true;
    }

    // A VALUE-call-result local (`let bounded = min(self.seed, 60)`) OR a
    // RUNTIME-INDEXED-READ local (`let h = self.nums[self.i]`, synthesized by
    // the operand-hoisting normalization) whose value cannot be
    // folded/substituted is correctly elided when consumed in positions the
    // substitution covers (call arguments, slice ops -- see the `chance` /
    // subslice-chain canaries, which MUST stay slot-less). But as an
    // ARITHMETIC-BINARY or CAST operand (`let s = bounded + 70`,
    // `self.big = h as i64`) the substitution does not fire (the indexed read
    // is deliberately NOT folded back -- it has no operand-position lowering),
    // so the dependent binary/cast write silently drops its unresolved operand.
    // Keep the slot for exactly that combination -- narrow enough that call-arg
    // / slice-op uses are untouched, and no slot-offset shift can regress a
    // currently-passing program (none exercises this broken pattern).
    if (expression_contains_call(expressions, initial_value)
        || initializer_is_runtime_indexed_read(expressions, initial_value))
        && statements
            .iter()
            .skip(local_statement_index + 1)
            .any(|statement| {
                statement_uses_symbol_as_arithmetic_operand(
                    expressions,
                    statement,
                    local_symbol,
                    local_name,
                )
            })
    {
        return true;
    }

    // `statement_references_local` (the final check below) inspects Expression /
    // Assignment / Call / Transition statements but NOT a LocalData (`let`) VALUE.
    // So an AGGREGATE local (array/struct literal -- which has no immediate form to
    // fold into a later use) read only by a subsequent `let` -- e.g.
    // `let arr = [..]; let e = arr[1]` -- is invisible to the liveness scan and its
    // slot is elided; the indexed/field read then resolves against a missing slot and
    // silently drops to 0 (a native miscompile; the interpreter, which keeps the
    // value, is right). Keep the slot for exactly that combination. Gated on an
    // aggregate-literal initializer so foldable scalars (correct without a slot) are
    // untouched -- no slot-offset shift can regress them.
    if initializer_is_array_literal(expressions, initial_value)
        && statements
            .iter()
            .skip(local_statement_index + 1)
            .any(|statement| {
                local_data_value_references_symbol(
                    expressions,
                    statement,
                    local_symbol,
                    local_name,
                )
            })
    {
        return true;
    }

    statements
        .iter()
        .skip(local_statement_index + 1)
        .any(|statement| {
            statement_references_local(
                expressions,
                statement_table,
                statement,
                local_symbol,
                local_name,
                uses_runtime_flow,
            )
        })
}

/// Whether an initializer is an ARRAY literal (possibly wrapped in `Mutable`). An
/// array element read (`arr[i]`) has no immediate form and is not folded back, so a
/// later use needs the array's slot. Deliberately NOT struct literals: a
/// borrow-carrying struct (`Msg { body: &cell }`) must stay FOLDED so the borrow
/// substitutes into `msg.body` -- slotting it would store a stale pointer
/// (regressed `borrow_carrying_data_field`). Value-struct field reads fold to the
/// literal field and do not need a slot.
fn initializer_is_array_literal(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(_) => true,
        ExpressionNode::Mutable(inner) => initializer_is_array_literal(expressions, *inner),
        _ => false,
    }
}

/// Whether a LocalData (`let`) statement's initializer VALUE references the
/// symbol/name -- the position `statement_references_local` does not inspect.
fn local_data_value_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::LocalData(local) = statement else {
        return false;
    };
    local.initial_value.is_valid()
        && expression_references_symbol(expressions, local.initial_value, symbol, local_name)
}

/// Whether an initializer contains ANY call (a call result cannot be folded into
/// a later use). Mirrors `expression_contains_mutating_call`'s structure but any
/// `Call` qualifies. Only used (with the arithmetic-operand test) to keep a
/// call-result local's slot where substitution cannot reach.
fn expression_contains_call(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Mutable(inner) => expression_contains_call(expressions, *inner),
        ExpressionNode::ArrayLiteral(items) => expressions
            .expression_handles(*items)
            .iter()
            .copied()
            .any(|item| expression_contains_call(expressions, item)),
        ExpressionNode::Binary(binary) => {
            expression_contains_call(expressions, binary.left)
                || expression_contains_call(expressions, binary.right)
        }
        ExpressionNode::Cast(cast) => expression_contains_call(expressions, cast.value),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call(expressions, indexed.collection)
                || expression_contains_call(expressions, indexed.index)
        }
        ExpressionNode::Range(range) => {
            expression_contains_call(expressions, range.start)
                || expression_contains_call(expressions, range.end)
        }
        ExpressionNode::Member(member) => expression_contains_call(expressions, member.receiver),
        ExpressionNode::Unary(unary) => expression_contains_call(expressions, unary.operand),
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_contains_call(expressions, field.value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => false,
    }
}

/// Whether an initializer is a RUNTIME-indexed read (`arr[i]` with a
/// non-constant index), possibly wrapped in `Mutable`. Such a local is
/// materialized into its own slot by the whole-value copy path; reading it as a
/// binary/cast operand needs the slot (the indexed read is never folded back,
/// so substitution cannot supply the operand). A CONSTANT-index read folds to a
/// plain place and does not need this carve-out.
fn initializer_is_runtime_indexed_read(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => !matches!(
            expressions.expression(indexed.index),
            ExpressionNode::Integer(_)
        ),
        ExpressionNode::Mutable(inner) => {
            initializer_is_runtime_indexed_read(expressions, *inner)
        }
        _ => false,
    }
}

fn is_arithmetic_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
    )
}

/// A COMPARISON reads its operands as runtime values (`local > 5`), the same
/// unresolvable-without-a-slot position as an arithmetic-binary operand: a
/// slot-less local (e.g. one initialized from a runtime-indexed read, which is
/// deliberately NOT folded back) is not substituted into the compare, so a
/// dependent `let b = local > 5` silently drops it and reads a default. So a
/// local used as a compare operand must keep its slot, exactly like an
/// arithmetic operand. (The DIRECT `let b = arr[i] > 5` is already a clean
/// error; this closes the intermediate-local escape `let t = arr[i]; let b = t
/// > 5`, which folded to the broken direct form and miscompiled silently.)
fn is_comparison_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    )
}

/// Whether `expression` is directly a reference to `symbol` (a bare `Name`).
fn expression_is_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    matches!(
        expressions.expression(expression),
        ExpressionNode::Name(path)
            if path.head_symbol == symbol
                || expressions
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| name == local_name)
    )
}

/// Whether `expression` uses `symbol` as a DIRECT operand of an arithmetic binary
/// anywhere in its tree (`bounded + 70`, `(bounded) * k`, ...). This is the one
/// consumer position where a slot-less call-result local cannot be substituted,
/// so its presence (with a call initializer) forces the slot to be kept.
fn expression_uses_symbol_as_arithmetic_operand(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let recurse = |handle| {
        expression_uses_symbol_as_arithmetic_operand(expressions, handle, symbol, local_name)
    };
    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            ((is_arithmetic_operator(binary.operator)
                || is_comparison_operator(binary.operator))
                && (expression_is_symbol(expressions, binary.left, symbol, local_name)
                    || expression_is_symbol(expressions, binary.right, symbol, local_name)))
                || recurse(binary.left)
                || recurse(binary.right)
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        // A CAST reads its operand as a runtime value (`local as i64`), the same
        // unresolvable-without-a-slot position as an arithmetic-binary operand:
        // a slot-less local is not substituted into the cast, so the cast write
        // would drop it. Treat the cast OF the symbol as such a use.
        ExpressionNode::Cast(cast) => {
            expression_is_symbol(expressions, cast.value, symbol, local_name)
                || recurse(cast.value)
        }
        ExpressionNode::Mutable(inner) => recurse(*inner),
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Indexed(indexed) => recurse(indexed.collection) || recurse(indexed.index),
        ExpressionNode::Range(range) => recurse(range.start) || recurse(range.end),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && recurse(call.receiver))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(recurse)
        }
        ExpressionNode::ArrayLiteral(items) => {
            expressions.expression_handles(*items).iter().copied().any(recurse)
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| recurse(field.value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => false,
    }
}

/// Whether a later statement uses `symbol` as an arithmetic-binary operand (in a
/// local initializer, an assignment value, a terminal expression, or a call
/// argument).
fn statement_uses_symbol_as_arithmetic_operand(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match statement {
        StatementNode::LocalData(local_data) => expression_uses_symbol_as_arithmetic_operand(
            expressions,
            local_data.initial_value,
            symbol,
            local_name,
        ),
        StatementNode::Assignment(assignment) => expression_uses_symbol_as_arithmetic_operand(
            expressions,
            assignment.value,
            symbol,
            local_name,
        ),
        StatementNode::Expression(expression) => {
            expression_uses_symbol_as_arithmetic_operand(expressions, *expression, symbol, local_name)
        }
        StatementNode::Call(call) => expressions
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .any(|argument| {
                expression_uses_symbol_as_arithmetic_operand(
                    expressions,
                    argument,
                    symbol,
                    local_name,
                )
            }),
        StatementNode::Transition(_) => false,
    }
}

/// Whether a single statement references the local (in a transition, an
/// expression, an assignment target/value, a call argument, or a mutable use).
/// Factored so both the same-state liveness scan and the cross-state scan
/// (`local_referenced_in_other_machine_states`) agree on what "uses the local"
/// means.
fn statement_references_local(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    local_symbol: SymbolHandle,
    local_name: &Identifier,
    uses_runtime_flow: bool,
) -> bool {
    (uses_runtime_flow
        && statement_references_symbol_in_transition(
            expressions,
            statement_table,
            statement,
            local_symbol,
            local_name,
        ))
        || statement_expression_references_symbol(expressions, statement, local_symbol, local_name)
        || assignment_target_references_symbol(expressions, statement, local_symbol, local_name)
        || assignment_targets_symbol(expressions, statement, local_symbol, local_name)
        || statement_call_references_symbol(
            expressions,
            statement_table,
            statement,
            local_symbol,
            local_name,
        )
        || statement_uses_symbol_mutably(
            expressions,
            statement_table,
            statement,
            local_symbol,
            local_name,
        )
}

/// Whether any state of the machine OTHER than the declaring state references
/// the local. A machine's sub-states share its frame region, so a local
/// declared in one state (e.g. `main`) and read only in a later state (e.g.
/// `let sum = self.r.sum(s,0)` used in a `do_total` sub-state, NOT passed as an
/// argument) is live across the transition. The per-state liveness scan only
/// sees the declaring state's own statements, so without this it would wrongly
/// elide the local's slot and the sub-state would read garbage (a silent
/// miscompile). Matching is symbol-precise, so an unrelated local that happens
/// to share a name in another state does not keep this one alive. Additive:
/// it can only keep MORE locals, never drop one.
fn local_referenced_in_other_machine_states(
    program: &CheckedTrees,
    machine: &Machine,
    declaring_state_symbol: SymbolHandle,
    local_symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    program.machine_states(machine).iter().any(|state| {
        if state.symbol == declaring_state_symbol {
            return false;
        }
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                statement_references_local(
                    &program.expression_table,
                    &program.statement_table,
                    statement,
                    local_symbol,
                    local_name,
                    true,
                )
            })
    })
}

/// Whether the expression contains a MUTATING call -- a call passing a `&mut`
/// argument (`self.next_seed(&mut rng)`). Syntactic: the `&mut` at the call
/// site is the caller-visible mark that the callee's body splices mutation
/// operations before the result is available.
fn expression_contains_mutating_call(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Call(call) => {
            expressions
                .expression_handles(call.arguments)
                .iter()
                .copied()
                .any(|argument| {
                    matches!(expressions.expression(argument), ExpressionNode::Mutable(_))
                        || expression_contains_mutating_call(expressions, argument)
                })
                || (call.receiver.is_valid()
                    && expression_contains_mutating_call(expressions, call.receiver))
        }
        ExpressionNode::Mutable(inner) => expression_contains_mutating_call(expressions, *inner),
        ExpressionNode::ArrayLiteral(items) => expressions
            .expression_handles(*items)
            .iter()
            .copied()
            .any(|item| expression_contains_mutating_call(expressions, item)),
        ExpressionNode::Binary(binary) => {
            expression_contains_mutating_call(expressions, binary.left)
                || expression_contains_mutating_call(expressions, binary.right)
        }
        ExpressionNode::Cast(cast) => expression_contains_mutating_call(expressions, cast.value),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_mutating_call(expressions, indexed.collection)
                || expression_contains_mutating_call(expressions, indexed.index)
        }
        ExpressionNode::Range(range) => {
            expression_contains_mutating_call(expressions, range.start)
                || expression_contains_mutating_call(expressions, range.end)
        }
        ExpressionNode::Member(member) => {
            expression_contains_mutating_call(expressions, member.receiver)
        }
        ExpressionNode::Unary(unary) => {
            expression_contains_mutating_call(expressions, unary.operand)
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_contains_mutating_call(expressions, field.value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => false,
    }
}

/// Whether a CALL statement references the local in one of its ARGUMENTS
/// (`self.console.exit_process(n)`). The call marshals the argument from the
/// local's storage slot, so the local needs one even when nothing else reads
/// it -- without this the host-call argument resolved to no place, the call
/// emitted zero bytes, and its relocation corrupted the following code.
/// Whether a later trailing EXPRESSION statement (a state's terminal return
/// value, e.g. `let v = self.n; ...; v`) references the local. Such a local
/// must keep its slot: the terminal value is delivered AFTER the state's
/// mutations (the runtime-bodies splice lays the callee's effects out before
/// the deferred result write), so folding the local into its initializer would
/// re-read post-mutation state instead of the value captured at declaration.
/// Without a slot the bare name cannot resolve as a place at selection and the
/// result write silently dropped (callee returned 0).
fn statement_expression_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Expression(expression) = statement else {
        return false;
    };
    expression_references_symbol(expressions, *expression, symbol, local_name)
}

fn statement_call_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Call(call) = statement else {
        return false;
    };
    statement_table
        .expression_handles(call.arguments)
        .iter()
        .copied()
        .any(|expression| expression_references_symbol(expressions, expression, symbol, local_name))
}

fn statement_references_symbol_in_transition(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Transition(transition) = statement else {
        return false;
    };

    transition_guard_references_symbol(expressions, transition.guard, symbol, local_name)
        || transition_target_references_symbol(
            expressions,
            statement_table,
            transition.target,
            symbol,
            local_name,
        )
        || transition_target_references_symbol(
            expressions,
            statement_table,
            transition.continuation,
            symbol,
            local_name,
        )
}

fn assignment_targets_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };

    assignment_target_head_symbol(expressions, assignment.target) == symbol
        || assignment_target_head_name(expressions, assignment.target)
            .is_some_and(|name| name == local_name)
}

fn assignment_target_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };

    expression_references_symbol(expressions, assignment.target, symbol, local_name)
}

fn assignment_target_head_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: omega_checked_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    use omega_checked_trees::expression::ExpressionNode;

    match expressions.expression(expression) {
        ExpressionNode::Name(path) => {
            let head_symbol = path.head_symbol;
            head_symbol
        }
        ExpressionNode::Member(member) => {
            assignment_target_head_symbol(expressions, member.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            assignment_target_head_symbol(expressions, indexed.collection)
        }
        ExpressionNode::Mutable(inner) => assignment_target_head_symbol(expressions, *inner),
        _ => SymbolHandle::invalid(),
    }
}

fn assignment_target_head_name(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: omega_checked_trees::expression::ExpressionHandle,
) -> Option<&Identifier> {
    use omega_checked_trees::expression::ExpressionNode;

    match expressions.expression(expression) {
        ExpressionNode::Name(path) => expressions.name_path_members(path.members).first(),
        ExpressionNode::Member(member) => assignment_target_head_name(expressions, member.receiver),
        ExpressionNode::Indexed(indexed) => {
            assignment_target_head_name(expressions, indexed.collection)
        }
        ExpressionNode::Mutable(inner) => assignment_target_head_name(expressions, *inner),
        _ => None,
    }
}

fn statement_uses_symbol_mutably(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match statement {
        StatementNode::Assignment(assignment) => {
            expression_uses_symbol_mutably(expressions, assignment.value, symbol, local_name)
        }
        StatementNode::Call(call) => statement_table
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .any(|expression| {
                expression_uses_symbol_mutably(expressions, expression, symbol, local_name)
            }),
        StatementNode::Expression(expression) => {
            expression_uses_symbol_mutably(expressions, *expression, symbol, local_name)
        }
        StatementNode::LocalData(local_data) => {
            local_data.initial_value.is_valid()
                && expression_uses_symbol_mutably(
                    expressions,
                    local_data.initial_value,
                    symbol,
                    local_name,
                )
        }
        StatementNode::Transition(transition) => {
            transition_guard_uses_symbol_mutably(expressions, transition.guard, symbol, local_name)
                || transition_target_uses_symbol_mutably(
                    expressions,
                    statement_table,
                    transition.target,
                    symbol,
                    local_name,
                )
                || transition_target_uses_symbol_mutably(
                    expressions,
                    statement_table,
                    transition.continuation,
                    symbol,
                    local_name,
                )
        }
    }
}

fn transition_guard_uses_symbol_mutably(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    guard: TransitionGuardNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match guard {
        TransitionGuardNode::Always => false,
        TransitionGuardNode::When(expression) => {
            expression_uses_symbol_mutably(expressions, expression, symbol, local_name)
        }
    }
}

fn transition_guard_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    guard: TransitionGuardNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match guard {
        TransitionGuardNode::Always => false,
        TransitionGuardNode::When(expression) => {
            expression_references_symbol(expressions, expression, symbol, local_name)
        }
    }
}

fn transition_target_uses_symbol_mutably(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    target: omega_checked_trees::statement::TransitionTargetHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    if !target.is_valid() {
        return false;
    }

    match statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => statement_table
            .expression_handles(*arguments)
            .iter()
            .copied()
            .any(|expression| {
                expression_uses_symbol_mutably(expressions, expression, symbol, local_name)
            }),
        TransitionTargetNode::Value(expression) => {
            expression_uses_symbol_mutably(expressions, *expression, symbol, local_name)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
    }
}

fn transition_target_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    target: omega_checked_trees::statement::TransitionTargetHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    if !target.is_valid() {
        return false;
    }

    match statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => statement_table
            .expression_handles(*arguments)
            .iter()
            .copied()
            .any(|expression| {
                expression_references_symbol(expressions, expression, symbol, local_name)
            }),
        TransitionTargetNode::Value(expression) => {
            expression_references_symbol(expressions, *expression, symbol, local_name)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
    }
}

fn expression_uses_symbol_mutably(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            expression_references_symbol(expressions, *inner, symbol, local_name)
        }
        ExpressionNode::ArrayLiteral(items) => expressions
            .expression_handles(*items)
            .iter()
            .copied()
            .any(|item| expression_uses_symbol_mutably(expressions, item, symbol, local_name)),
        ExpressionNode::Binary(binary) => {
            expression_uses_symbol_mutably(expressions, binary.left, symbol, local_name)
                || expression_uses_symbol_mutably(expressions, binary.right, symbol, local_name)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_symbol_mutably(expressions, call.receiver, symbol, local_name))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(|argument| {
                        expression_uses_symbol_mutably(expressions, argument, symbol, local_name)
                    })
        }
        ExpressionNode::Cast(cast) => {
            expression_uses_symbol_mutably(expressions, cast.value, symbol, local_name)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_uses_symbol_mutably(expressions, indexed.collection, symbol, local_name)
                || expression_uses_symbol_mutably(expressions, indexed.index, symbol, local_name)
        }
        ExpressionNode::Range(range) => {
            expression_uses_symbol_mutably(expressions, range.start, symbol, local_name)
                || expression_uses_symbol_mutably(expressions, range.end, symbol, local_name)
        }
        ExpressionNode::Member(member) => {
            expression_uses_symbol_mutably(expressions, member.receiver, symbol, local_name)
        }
        ExpressionNode::Unary(unary) => {
            expression_uses_symbol_mutably(expressions, unary.operand, symbol, local_name)
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| {
                expression_uses_symbol_mutably(expressions, field.value, symbol, local_name)
            }),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => false,
    }
}

fn expression_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => {
            path.head_symbol == symbol
                || expressions
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| name == local_name)
        }
        ExpressionNode::Mutable(inner) => {
            expression_references_symbol(expressions, *inner, symbol, local_name)
        }
        ExpressionNode::ArrayLiteral(items) => expressions
            .expression_handles(*items)
            .iter()
            .copied()
            .any(|item| expression_references_symbol(expressions, item, symbol, local_name)),
        ExpressionNode::Binary(binary) => {
            expression_references_symbol(expressions, binary.left, symbol, local_name)
                || expression_references_symbol(expressions, binary.right, symbol, local_name)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_references_symbol(expressions, call.receiver, symbol, local_name))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(|argument| {
                        expression_references_symbol(expressions, argument, symbol, local_name)
                    })
        }
        ExpressionNode::Cast(cast) => {
            expression_references_symbol(expressions, cast.value, symbol, local_name)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_references_symbol(expressions, indexed.collection, symbol, local_name)
                || expression_references_symbol(expressions, indexed.index, symbol, local_name)
        }
        ExpressionNode::Range(range) => {
            expression_references_symbol(expressions, range.start, symbol, local_name)
                || expression_references_symbol(expressions, range.end, symbol, local_name)
        }
        ExpressionNode::Member(member) => {
            expression_references_symbol(expressions, member.receiver, symbol, local_name)
        }
        ExpressionNode::Unary(unary) => {
            expression_references_symbol(expressions, unary.operand, symbol, local_name)
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| {
                expression_references_symbol(expressions, field.value, symbol, local_name)
            }),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => false,
    }
}

fn append_type_reference_invariant_names(
    program: &CheckedTrees,
    type_reference: TypeReferenceHandle,
    names: &mut Arena<Identifier>,
) -> HandleSpan<Identifier> {
    let mut span = HandleSpan::empty();
    collect_type_reference_invariant_names(program, type_reference, names, &mut span);
    span
}

fn collect_type_reference_invariant_names(
    program: &CheckedTrees,
    type_reference: TypeReferenceHandle,
    names: &mut Arena<Identifier>,
    span: &mut HandleSpan<Identifier>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_reference_invariant_names(program, *referee, names, span)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference_invariant_names(program, *base_type, names, span);

            for constraint in program.type_reference_table.constraints(*constraints) {
                let omega_checked_trees::types::TypeConstraintNode::Named(name) = constraint else {
                    continue;
                };

                if program
                    .facts
                    .invariants
                    .definitions
                    .iter()
                    .any(|(_, invariant)| invariant.name == *name)
                    && !names
                        .span_or_empty(*span)
                        .iter()
                        .any(|existing| existing == name)
                {
                    names.append_to_span(span, name.clone());
                }
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_type_reference_invariant_names(program, *element_type, names, span)
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_type_reference_invariant_names(program, *element_type, names, span)
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_reference_invariant_names(program, *argument, names, span);
            }
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}
