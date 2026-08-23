use super::{StateLocalStorage, StateMutation, StateStoragePlan};
use crate::StateStoragePlanningContext;
use crate::mutation_kind::{mutation_kind, mutation_lowering};
use omega_control_flow::StateKey;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_values::simplify_state_expression;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTableCapacity,
};
use psi_checked_trees::machine::Machine;
use psi_checked_trees::name::Identifier;
use psi_checked_trees::statement::{
    StatementNode, StatementTable, TransitionGuardNode, TransitionTargetNode,
};
use psi_checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_symbols::{SymbolHandle, SymbolKind};
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
                plan.expressions
                    .copy_from(&expressions, local.initial_value)
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
                        program,
                        local_data.is_mutable,
                        state,
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
                    let preserve_checked_provider_expression =
                        statement_has_selected_provider_plan(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                        ) && plan.locals.iter().any(|(_, local)| {
                            local.source_key == source_key
                                && expression_references_symbol(
                                    &program.expression_table,
                                    assignment.value,
                                    local.symbol,
                                    &local.name,
                                )
                        });
                    let value = if preserve_checked_provider_expression
                        || program
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
                        let authored_value = program.expression_table.to_tree(assignment.value);
                        let simplified_value = simplify_state_expression(
                            program,
                            machine,
                            state,
                            statement_index,
                            &authored_value,
                        );
                        if simplified_value == authored_value {
                            // Preserve the checked expression's source identity when
                            // simplification is a no-op. Downstream operator evidence
                            // is keyed to that exact authored node; rebuilding the same
                            // tree with `insert_tree` would discard every source span.
                            plan.expressions
                                .copy_from(&program.expression_table, assignment.value)
                        } else {
                            plan.expressions.insert_tree(&simplified_value)
                        }
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

/// A selected ProviderPlan is exact checked evidence for the authored
/// operation tree. Replacing a materialized local with its initializer while
/// collecting storage mutations can move nested operations across statement
/// identity and discard that evidence before instruction selection. Preserve
/// the complete authored assignment only when a migrated operator owns a
/// nonzero plan and the assignment reads one of this state's planned locals.
fn statement_has_selected_provider_plan(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
) -> bool {
    program
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| (operator_use.origin, operator_use.provider_plan_identity))
        .chain(
            program
                .facts
                .operators
                .named_uses
                .iter()
                .map(|(_, operator_use)| {
                    (operator_use.origin, operator_use.provider_plan_identity)
                }),
        )
        .any(|(origin, provider_plan_identity)| {
            provider_plan_identity != 0
                && matches!(
                    origin,
                    psi_checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol: candidate_machine,
                        state_symbol: candidate_state,
                        statement_index: candidate_statement,
                        ..
                    } if candidate_machine == machine_symbol
                        && candidate_state == state_symbol
                        && candidate_statement == statement_index
                )
        })
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
    state: &psi_checked_trees::state::State,
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

/// Whether an initializer contains a TRAPPING arithmetic operation (the
/// abort-as-effect carve-out in `local_data_requires_storage`). Domains
/// resolve per decision-17's operand-driven rule: the local's own declared
/// `in Trapping` (the canonical dead-let spelling), a Trapping `as` cast, or
/// an arithmetic operand naming a Trapping-declared parameter, earlier local,
/// or data field.
fn initializer_carries_trapping_arithmetic(
    program: &CheckedTrees,
    state: &psi_checked_trees::state::State,
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    initial_value: ExpressionHandle,
) -> bool {
    use psi_numerics::arithmetic::ArithmeticDomain;

    if let Some(StatementNode::LocalData(local_data)) = statements.get(local_statement_index)
        && local_data.type_reference.is_valid()
        && program
            .type_reference_table
            .arithmetic_domain(local_data.type_reference)
            == ArithmeticDomain::Trapping
        && matches!(
            expressions.expression(initial_value),
            psi_checked_trees::expression::ExpressionNode::Binary(_)
                | psi_checked_trees::expression::ExpressionNode::Cast(_)
        )
    {
        return true;
    }
    expression_contains_trapping_op(
        program,
        state,
        expressions,
        statements,
        local_statement_index,
        initial_value,
    )
}

fn expression_contains_trapping_op(
    program: &CheckedTrees,
    state: &psi_checked_trees::state::State,
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    expression: ExpressionHandle,
) -> bool {
    use psi_checked_trees::expression::{BinaryOperator, ExpressionNode};
    use psi_numerics::arithmetic::ArithmeticDomain;

    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let arithmetic = matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
            );
            let (left, right) = (binary.left, binary.right);
            (arithmetic
                && (operand_declared_trapping(
                    program,
                    state,
                    expressions,
                    statements,
                    local_statement_index,
                    left,
                ) || operand_declared_trapping(
                    program,
                    state,
                    expressions,
                    statements,
                    local_statement_index,
                    right,
                )))
                || expression_contains_trapping_op(
                    program,
                    state,
                    expressions,
                    statements,
                    local_statement_index,
                    left,
                )
                || expression_contains_trapping_op(
                    program,
                    state,
                    expressions,
                    statements,
                    local_statement_index,
                    right,
                )
        }
        ExpressionNode::Cast(cast) => {
            cast.domain == ArithmeticDomain::Trapping
                || expression_contains_trapping_op(
                    program,
                    state,
                    expressions,
                    statements,
                    local_statement_index,
                    cast.value,
                )
        }
        ExpressionNode::Borrow(inner) => expression_contains_trapping_op(
            program,
            state,
            expressions,
            statements,
            local_statement_index,
            inner.target,
        ),
        _ => false,
    }
}

/// Whether an arithmetic OPERAND names something whose declared type carries
/// the Trapping domain: a parameter of this state, an earlier local of this
/// state, or a data field (member path).
fn operand_declared_trapping(
    program: &CheckedTrees,
    state: &psi_checked_trees::state::State,
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    operand: ExpressionHandle,
) -> bool {
    use psi_checked_trees::expression::ExpressionNode;
    use psi_numerics::arithmetic::ArithmeticDomain;

    let reference_is_trapping = |handle: psi_checked_trees::types::TypeReferenceHandle| {
        handle.is_valid()
            && program.type_reference_table.arithmetic_domain(handle) == ArithmeticDomain::Trapping
    };

    match expressions.expression(operand) {
        ExpressionNode::Name(path) => {
            let symbol = path.symbol;
            if !symbol.is_valid() {
                return false;
            }
            program.state_parameters(state).iter().any(|parameter| {
                parameter.symbol == symbol && reference_is_trapping(parameter.type_reference)
            }) || statements
                .iter()
                .take(local_statement_index)
                .any(|statement| {
                    matches!(
                        statement,
                        StatementNode::LocalData(local)
                            if local.symbol == symbol && reference_is_trapping(local.type_reference)
                    )
                })
        }
        ExpressionNode::Member(member) => {
            if !member.member_symbol.is_valid() {
                return false;
            }
            program.data_definitions().iter().any(|data| {
                program.data_members(data).iter().any(|data_member| {
                    matches!(
                        data_member,
                        psi_checked_trees::data::DataMember::Field(field)
                            if field.symbol == member.member_symbol
                                && reference_is_trapping(field.type_reference)
                    )
                })
            })
        }
        _ => false,
    }
}

fn local_data_requires_storage(
    program: &CheckedTrees,
    local_is_mutable: bool,
    state: &psi_checked_trees::state::State,
    expressions: &psi_checked_trees::expression::ExpressionTable,
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

    // `let mut` locals are reassignable -- the slot IS their storage story
    // (the simplify layer never binds them; see bindings.rs's twin note).
    if local_is_mutable {
        return true;
    }

    // A local whose initializer reads a member THROUGH A SHARED-REFERENCE
    // param (`let h = table.con_out` with `table: &EfiSystemTable`) exists to
    // MATERIALIZE the dereference -- folding it back re-creates the flat
    // frame-garbage read the let was minted to avoid (the entry-ref-param
    // face; the guard/assignment hoists synthesize exactly this shape). Keep
    // the slot whenever the local is referenced afterwards, in ANY position
    // (including assignment values, which the liveness scan otherwise skips).
    // Fires in EVERY machine, not only boundary ones: a `&Named` param holds a
    // real pointer everywhere (bug 2026-07-12, the non-boundary `own_machine`
    // deref); kept in lockstep with the read-side resolver in
    // omega-instruction-selection storage_places.rs and the simplify twin.
    if initializer_is_reference_param_member(program, state, expressions, initial_value)
        && statements
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
                ) || local_data_value_references_symbol(
                    expressions,
                    statement,
                    local_symbol,
                    local_name,
                ) || assignment_value_references_symbol(
                    expressions,
                    statement,
                    local_symbol,
                    local_name,
                )
            })
    {
        return true;
    }

    // A RECAST local is an address-bearing runtime view, not a substitutable
    // scalar alias (`simple_local_binding_value_from_table` deliberately
    // refuses to fold it). When its only uses occur in a later `let`
    // initializer or ASSIGNMENT VALUE, the final liveness scan cannot see them
    // and used to elide the view's slot. Projected reads then either decoded
    // zeros or alias-expanded back to a cast-rooted path that storage selection
    // cannot place. Keep the slot for those otherwise-invisible use shapes;
    // ordinary statement uses remain covered by the final scan below.
    if initializer_is_recast(expressions, initial_value)
        && statements
            .iter()
            .skip(local_statement_index + 1)
            .any(|statement| {
                local_data_value_references_symbol(expressions, statement, local_symbol, local_name)
                    || assignment_value_references_symbol(
                        expressions,
                        statement,
                        local_symbol,
                        local_name,
                    )
            })
    {
        return true;
    }

    // Owner ruling 2026-07-18 (the first sentence of abort-as-effect #65): a
    // TRAPPING operation's trap is an EFFECT -- a computation that can trap
    // "actually traps on paper; it's not dead code anymore". A local whose
    // initializer carries Trapping arithmetic therefore keeps its slot even
    // when nothing reads it, so the ordinary write path lowers the trap (the
    // interpreter always evaluated it -- the pinned dead_trapping_let
    // divergence was native-side elision). The backend may later optimize to
    // any trap-EQUIVALENT lowering, but never to silence.
    if initializer_carries_trapping_arithmetic(
        program,
        state,
        expressions,
        statements,
        local_statement_index,
        initial_value,
    ) {
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
        && local_or_bare_copy_used_as_arithmetic_operand(
            program,
            expressions,
            statements,
            local_statement_index,
            local_symbol,
            local_name,
        )
    {
        return true;
    }

    // A FLOAT local consumed by later arithmetic, a cast, or a compiler-owned
    // numeric builtin must preserve its authored value at its own statement.
    // Folding even a literal carrier into the consumer can move a checked
    // float operation across statement identity, where its exact
    // provider/policy evidence cannot follow by resemblance. Materialize the
    // local instead, so the consumer reads the already-computed carrier. This
    // also covers a format-conversion result consumed by a classification
    // predicate (`let narrowed = source as f32; let finite = is_finite(narrowed)`).
    let local_is_float = statements
        .get(local_statement_index)
        .and_then(|statement| match statement {
            StatementNode::LocalData(local) => {
                program.primitive_type_reference(local.type_reference)
            }
            _ => None,
        })
        .is_some_and(|primitive| {
            matches!(
                primitive,
                psi_checked_trees::types::PrimitiveType::F32
                    | psi_checked_trees::types::PrimitiveType::F64
            )
        });
    if local_is_float
        && local_or_bare_copy_used_as_arithmetic_operand(
            program,
            expressions,
            statements,
            local_statement_index,
            local_symbol,
            local_name,
        )
    {
        return true;
    }

    // A local consumed as a RUNTIME INDEX (`let m = self.k + 1; self.arr[m]`)
    // keeps its slot when its initializer is a COMPUTED value: eliding it folds
    // the initializer back into the index position, re-creating a computed
    // index (`arr[k+1]`), which has no lowering (the backend blocker refuses
    // the statement loudly). The checker proves bounds from the local's RANGE
    // TYPE, so the slotted plain-place index is sound AND lowerable. Foldable
    // initializers (a bare-Name copy, a constant, a member place) stay
    // elidable -- they fold to a plain place or a const index, which lowers.
    // No currently-passing program changes: every computed-index shape was
    // refused before this carve-out.
    if initializer_is_computed_value(expressions, initial_value)
        && local_or_bare_copy_used_as_runtime_index(
            expressions,
            statement_table,
            statements,
            local_statement_index,
            local_symbol,
            local_name,
        )
    {
        return true;
    }

    // A COMPUTED-initializer local consumed as the VALUE of a runtime-INDEXED
    // write (`let next = cur + bump; self.cells[i] = next` -- the heat_grid
    // diffusion store) keeps its slot: eliding folds the computation back
    // into the mutation value, which has no runtime-value lowering against an
    // indexed target (the loud storage-write blocker), while the slotted
    // plain place rides the storage-to-indexed copy the mutation arm already
    // lowers. Twin of the runtime-INDEX carve-out above, on the value side.
    if initializer_is_computed_value(expressions, initial_value)
        && local_used_as_runtime_indexed_write_value(
            expressions,
            statements,
            local_statement_index,
            local_symbol,
            local_name,
        )
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
                // ...or by an ASSIGNMENT value (`self.got = g[a][b]`): the
                // same aggregate has no immediate form to fold into the
                // indexed read, so the elided slot left the double-indexed
                // resolvers nothing to index (the frame-2D machine-target
                // face; the tree-route fold is guarded separately in the
                // alias substitution).
                || assignment_value_references_symbol(
                    expressions,
                    statement,
                    local_symbol,
                    local_name,
                )
            })
    {
        return true;
    }

    // A STRUCT-LITERAL local whose field backs an `as_slice`/`as_mut_slice`
    // VIEW in a later statement (`let room = Room {..}; let exits =
    // room.exits.as_mut_slice()`) must own a real frame slot: the slice
    // descriptor's data pointer needs an ADDRESS, and an rvalue literal has
    // none. Elided, the local was invisible to the final liveness scan (the
    // view rode a later `let` value), every downstream strategy silently
    // planned nothing, and the forwarded descriptor stayed ZII -- the callee
    // crashed dereferencing a null data pointer (the pinned
    // local_slice_forward_segfault). Deliberately gated on the VIEW, not on
    // any later-`let` reference like the array arm above: a plain field read
    // (`m.body`) is served by the struct-literal field-extraction fold and
    // must stay slot-less (the borrow-carrying shapes depend on that fold).
    if initializer_is_struct_literal(expressions, initial_value)
        && statements
            .iter()
            .skip(local_statement_index + 1)
            .any(|statement| {
                statement_takes_slice_view_of_symbol(
                    expressions,
                    statement_table,
                    statement,
                    local_symbol,
                    local_name,
                )
            })
    {
        return true;
    }

    // A RUNTIME-BOUNDED SUBSLICE local (`let sub = arr[self.lo..self.hi]` -- a
    // Range index with a non-literal bound) must keep its descriptor slot when
    // used later. Elided, its uses trace back to the inline subslice, whose
    // CONSTRUCTION (ptr/len from runtime bounds) has no lowering in the local
    // value paths -- `let n = sub.len` silently read 0 and took the wrong guard
    // arm (interp right, native wrong). With the slot kept, the descriptor
    // write is attempted and, until the runtime-bound construction lowers,
    // emission planning's descriptor-local verification reports it LOUDLY
    // (exactly like the already-slotted guard-use case). LITERAL-bounded
    // subslices stay elided -- their window folds are proven (length + element
    // + argument paths all lower).
    if initializer_is_runtime_bounded_subslice(expressions, initial_value)
        && statements
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
                ) || local_data_value_references_symbol(
                    expressions,
                    statement,
                    local_symbol,
                    local_name,
                ) || assignment_value_references_symbol(
                    expressions,
                    statement,
                    local_symbol,
                    local_name,
                )
            })
    {
        return true;
    }

    // A local CAPTURING a field read (`let t = self.v`) keeps its slot when the
    // SOURCE FIELD is reassigned later and the local is still used AFTER that
    // write. Slot-less, the use alias-folds back to `self.v` and reads the NEW
    // value -- verified silently wrong (`self.v = 99; let t = self.v;
    // self.v = 0; nums[i] = t` wrote 0, not 99; the stale-fold family).
    // Deliberately narrow: a use BEFORE the reassignment folds correctly and
    // stays slot-less, so no slot-offset shift touches passing programs.
    if let Some(reassignment_index) = initializer_field_reassignment_index(
        expressions,
        statements,
        local_statement_index,
        initial_value,
    ) && statements
        .iter()
        .skip(reassignment_index + 1)
        .any(|statement| {
            statement_references_local(
                expressions,
                statement_table,
                statement,
                local_symbol,
                local_name,
                uses_runtime_flow,
            ) || local_data_value_references_symbol(
                expressions,
                statement,
                local_symbol,
                local_name,
            ) || assignment_value_references_symbol(
                expressions,
                statement,
                local_symbol,
                local_name,
            )
        })
    {
        return true;
    }

    // A BOUNDARY-call-result local whose value is NOT seen by the final liveness
    // scan below (which inspects Expression/Assignment/Call/Transition statements,
    // but NOT LocalData `let` VALUES, nor a truly-unused result) still needs its
    // slot. A boundary/host-call RESULT must be MATERIALIZED into a slot -- there
    // is no value-call substitution for it, unlike a state value-call -- but the
    // ergonomic std::fs wrapper consumes those results only through later `let`s or
    // not at all:
    //   let fd = self.host.create(path, mode);   // used ONLY in later `let`s
    //   let n  = self.host.write(fd, bytes);      // (write, close) -> invisible
    //   let rc = self.host.close(fd);             // UNUSED (discarded) result
    // With the slot elided, the dependent call's `fd` argument AND each call's own
    // result operand resolve against a missing slot -> "no result storage operand".
    // GATED ON A BOUNDARY CALL specifically (not any call): a STATE value-call
    // result the normal scan would elide MUST stay slot-less (its substitution
    // covers the elided positions -- broadening this to all calls regresses the
    // canary_suite by 6 via slot-offset shifts). canary_suite stash-diff confirms
    // the boundary-gated form is exact zero-regression.
    let referenced_by_final_scan =
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
            });
    if !referenced_by_final_scan
        && initializer_is_boundary_call(program, expressions, initial_value)
    {
        return true;
    }
    referenced_by_final_scan
}

/// Whether `initial_value` is (directly, or under a `Cast`/`Mutable` wrapper) a
/// call into a BOUNDARY trait method (`self.host.create(..)`). Mirrors
/// `omega-platform-interface`'s `expression_platform_receiver_type`: the call's
/// resolved `target_symbol` is one of a boundary trait's machine signatures. Used
/// to keep the result slot for a boundary-call `let` the liveness scan would elide
/// (a host-call result must be materialized; a state value-call result may not).
fn initializer_is_boundary_call(
    program: &CheckedTrees,
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    let target_symbol = match expressions.expression(expression) {
        ExpressionNode::Call(call) => call.target_symbol,
        ExpressionNode::Cast(cast) => {
            return initializer_is_boundary_call(program, expressions, cast.value);
        }
        ExpressionNode::Borrow(inner) => {
            return initializer_is_boundary_call(program, expressions, inner.target);
        }
        _ => return false,
    };
    if !target_symbol.is_valid() {
        return false;
    }
    program
        .traits()
        .iter()
        .filter(|trait_definition| trait_definition.is_boundary)
        .any(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .any(|machine| machine.symbol == target_symbol)
        })
}

/// Whether an initializer is a reference reinterpretation, possibly beneath
/// an `Atomic`/`Mutable` wrapper. Recasts preserve an address and therefore
/// cannot participate in the ordinary slot-less scalar alias substitution.
fn initializer_is_recast(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => initializer_is_recast(expressions, atomic.value),
        ExpressionNode::Borrow(inner) => initializer_is_recast(expressions, inner.target),
        ExpressionNode::Cast(cast) => cast.form.is_recast(),
        _ => false,
    }
}

/// Whether the local -- or any BARE-COPY of it (`let c = t;`, transitively) --
/// is used as an arithmetic/comparison/bitwise/cast or compiler-builtin value
/// operand after its declaration. The direct scan alone misses the copy chain:
/// `let t = arr[i]; let c = t; let b = c > 5` folded c -> t -> arr[i] into the
/// fenced direct form and silently read false. Bare-Name copies are matched by
/// NAME (a bare local use carries no valid symbol at this stage).
fn local_or_bare_copy_used_as_arithmetic_operand(
    program: &CheckedTrees,
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    local_symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    // One forward pass builds the transitive bare-copy set: statements are in
    // order, so a copy of a copy is seen after its source joined the set.
    let mut aliases: Vec<(SymbolHandle, Identifier)> = vec![(local_symbol, local_name.clone())];
    for statement in statements.iter().skip(local_statement_index + 1) {
        if let StatementNode::LocalData(local_data) = statement
            && local_data.initial_value.is_valid()
            && let Some(source_name) = bare_name_initializer(expressions, local_data.initial_value)
            && aliases.iter().any(|(_, name)| *name == source_name)
        {
            aliases.push((local_data.symbol, local_data.name.clone()));
        }
    }

    statements
        .iter()
        .skip(local_statement_index + 1)
        .any(|statement| {
            aliases.iter().any(|(symbol, name)| {
                statement_uses_symbol_as_arithmetic_operand(
                    program,
                    expressions,
                    statement,
                    *symbol,
                    name,
                )
            })
        })
}

/// A COMPUTED initializer (arithmetic, unary, cast, call -- peeling `Mutable`):
/// a VALUE with no place to fold back to. Bare names, constants, member reads,
/// and indexed reads are NOT "computed" here -- they fold into an index
/// position as a plain place or constant (or carry their own carve-outs).
fn initializer_is_computed_value(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Borrow(inner) => initializer_is_computed_value(expressions, inner.target),
        ExpressionNode::Binary(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Call(_) => true,
        _ => false,
    }
}

/// Whether the local (or a transitive bare-Name copy of it) is read as the
/// INDEX of an `Indexed` expression in any later statement -- the combination
/// that must keep its slot when the initializer is a computed value (see the
/// runtime-index carve-out). Mirrors
/// `local_or_bare_copy_used_as_arithmetic_operand`'s alias handling.
fn local_or_bare_copy_used_as_runtime_index(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    local_symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let mut aliases: Vec<(SymbolHandle, Identifier)> = vec![(local_symbol, local_name.clone())];
    for statement in statements.iter().skip(local_statement_index + 1) {
        if let StatementNode::LocalData(local_data) = statement
            && local_data.initial_value.is_valid()
            && let Some(source_name) = bare_name_initializer(expressions, local_data.initial_value)
            && aliases.iter().any(|(_, name)| *name == source_name)
        {
            aliases.push((local_data.symbol, local_data.name.clone()));
        }
    }

    statements
        .iter()
        .skip(local_statement_index + 1)
        .any(|statement| {
            aliases.iter().any(|(symbol, name)| {
                statement_uses_symbol_as_index(
                    expressions,
                    statement_table,
                    statement,
                    *symbol,
                    name,
                )
            })
        })
}

/// Whether any later assignment writes THIS local (a bare `Name`, through
/// `Mutable`) into a runtime-indexed target (`arr[<non-literal>] = local`).
/// Value-side twin of `local_or_bare_copy_used_as_runtime_index`.
fn local_used_as_runtime_indexed_write_value(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    local_symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    statements
        .iter()
        .skip(local_statement_index + 1)
        .any(|statement| {
            let StatementNode::Assignment(assignment) = statement else {
                return false;
            };
            let mut value = assignment.value;
            while let psi_checked_trees::expression::ExpressionNode::Borrow(inner) =
                expressions.expression(value)
            {
                value = inner.target;
            }
            let value_is_local = match expressions.expression(value) {
                psi_checked_trees::expression::ExpressionNode::Name(path) => {
                    (path.symbol.is_valid() && path.symbol == local_symbol)
                        || matches!(
                            expressions.name_path_members(path.members),
                            [only] if only.as_str() == local_name.as_str()
                        )
                }
                _ => false,
            };
            if !value_is_local {
                return false;
            }
            assignment_target_has_runtime_index(expressions, assignment.target)
        })
}

/// Whether a write target contains an `Indexed` layer whose index is not an
/// integer literal (constant indexes lower through the static element path).
fn assignment_target_has_runtime_index(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    target: psi_checked_trees::expression::ExpressionHandle,
) -> bool {
    match expressions.expression(target) {
        psi_checked_trees::expression::ExpressionNode::Indexed(indexed) => {
            !matches!(
                expressions.expression(indexed.index),
                psi_checked_trees::expression::ExpressionNode::Integer(_)
            ) || assignment_target_has_runtime_index(expressions, indexed.collection)
        }
        psi_checked_trees::expression::ExpressionNode::Member(member) => {
            assignment_target_has_runtime_index(expressions, member.receiver)
        }
        psi_checked_trees::expression::ExpressionNode::Borrow(inner) => {
            assignment_target_has_runtime_index(expressions, inner.target)
        }
        _ => false,
    }
}

fn statement_uses_symbol_as_index(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::LocalData(local_data) => expression_uses_symbol_as_index(
            expressions,
            local_data.initial_value,
            symbol,
            local_name,
        ),
        StatementNode::Assignment(assignment) => {
            expression_uses_symbol_as_index(expressions, assignment.value, symbol, local_name)
                || expression_uses_symbol_as_index(
                    expressions,
                    assignment.target,
                    symbol,
                    local_name,
                )
        }
        StatementNode::Expression(expression) => {
            expression_uses_symbol_as_index(expressions, *expression, symbol, local_name)
        }
        StatementNode::Call(call) => expressions
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .any(|argument| {
                expression_uses_symbol_as_index(expressions, argument, symbol, local_name)
            }),
        StatementNode::Transition(transition) => {
            (match transition.guard {
                TransitionGuardNode::Always => false,
                TransitionGuardNode::When(expression) => {
                    expression_uses_symbol_as_index(expressions, expression, symbol, local_name)
                }
            }) || transition_target_uses_symbol_as_index(
                expressions,
                statement_table,
                transition.target,
                symbol,
                local_name,
            ) || transition_target_uses_symbol_as_index(
                expressions,
                statement_table,
                transition.continuation,
                symbol,
                local_name,
            )
        }
    }
}

fn transition_target_uses_symbol_as_index(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    target: psi_checked_trees::statement::TransitionTargetHandle,
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
                expression_uses_symbol_as_index(expressions, expression, symbol, local_name)
            }),
        TransitionTargetNode::Value(expression) => {
            expression_uses_symbol_as_index(expressions, *expression, symbol, local_name)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
    }
}

/// Whether `symbol`/`local_name` appears as the INDEX of an `Indexed` node
/// anywhere inside the expression (the index peeled of `Mutable`).
fn expression_uses_symbol_as_index(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_is_bare_symbol(expressions, indexed.index, symbol, local_name)
                || expression_uses_symbol_as_index(
                    expressions,
                    indexed.collection,
                    symbol,
                    local_name,
                )
                || expression_uses_symbol_as_index(expressions, indexed.index, symbol, local_name)
        }
        ExpressionNode::Borrow(inner) => {
            expression_uses_symbol_as_index(expressions, inner.target, symbol, local_name)
        }
        ExpressionNode::Member(member) => {
            expression_uses_symbol_as_index(expressions, member.receiver, symbol, local_name)
        }
        ExpressionNode::Binary(binary) => {
            expression_uses_symbol_as_index(expressions, binary.left, symbol, local_name)
                || expression_uses_symbol_as_index(expressions, binary.right, symbol, local_name)
        }
        ExpressionNode::Unary(unary) => {
            expression_uses_symbol_as_index(expressions, unary.operand, symbol, local_name)
        }
        ExpressionNode::Cast(cast) => {
            expression_uses_symbol_as_index(expressions, cast.value, symbol, local_name)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_symbol_as_index(expressions, call.receiver, symbol, local_name))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(|argument| {
                        expression_uses_symbol_as_index(expressions, argument, symbol, local_name)
                    })
        }
        ExpressionNode::Range(range) => {
            expression_uses_symbol_as_index(expressions, range.start, symbol, local_name)
                || expression_uses_symbol_as_index(expressions, range.end, symbol, local_name)
        }
        ExpressionNode::ArrayLiteral(elements) => expressions
            .expression_handles(*elements)
            .iter()
            .copied()
            .any(|element| {
                expression_uses_symbol_as_index(expressions, element, symbol, local_name)
            }),
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| {
                expression_uses_symbol_as_index(expressions, field.value, symbol, local_name)
            }),
        _ => false,
    }
}

/// A bare single-member `Name` matching the local (by symbol or name), peeling
/// `Mutable`.
fn expression_is_bare_symbol(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            expression_is_bare_symbol(expressions, inner.target, symbol, local_name)
        }
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            members.len() == 1
                && ((symbol.is_valid() && path.head_symbol == symbol) || members[0] == *local_name)
        }
        _ => false,
    }
}

/// The NAME a bare-Name initializer reads (`let c = t;` -> `t`), peeling
/// `Mutable`. Anything else (binary, indexed, member, literal) is None.
fn bare_name_initializer(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> Option<Identifier> {
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => expressions
            .name_path_members(path.members)
            .first()
            .cloned()
            .filter(|_| expressions.name_path_members(path.members).len() == 1),
        ExpressionNode::Borrow(inner) => bare_name_initializer(expressions, inner.target),
        _ => None,
    }
}

/// Whether an initializer is a member read through a SHARED reference-to-named
/// parameter of `state` (`table.con_out` with `table: &EfiSystemTable`) --
/// peeling `Mutable`. Such a local materializes a POINTEE dereference and must
/// never be folded back to the member.
fn initializer_is_reference_param_member(
    program: &CheckedTrees,
    state: &psi_checked_trees::state::State,
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            initializer_is_reference_param_member(program, state, expressions, inner.target)
        }
        ExpressionNode::Member(member) => {
            let ExpressionNode::Name(path) = expressions.expression(member.receiver) else {
                return false;
            };
            let members = expressions.name_path_members(path.members);
            let [only] = members else {
                return false;
            };
            program.state_parameters(state).iter().any(|parameter| {
                parameter.name.as_str() == only.as_str()
                    && parameter_is_shared_named_reference(program, parameter.type_reference)
            })
        }
        _ => false,
    }
}

/// `&Named` (shared, non-slice) -- the pointer-slot param shape.
fn parameter_is_shared_named_reference(
    program: &CheckedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    if !access.is_readable() || access.is_exclusive() {
        return false;
    }
    matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { .. }
    )
}

/// The index of the first statement after the local's declaration that ASSIGNS
/// a member field the initializer READS (`let t = self.v; ...; self.v = 0;`).
/// Matched by FIELD NAME (conservative: a same-named field on another struct
/// only costs an extra slot, never a fold).
fn initializer_field_reassignment_index(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    initial_value: ExpressionHandle,
) -> Option<usize> {
    let mut read_fields = Vec::new();
    collect_member_field_names(expressions, initial_value, &mut read_fields);
    if read_fields.is_empty() {
        return None;
    }
    statements
        .iter()
        .enumerate()
        .skip(local_statement_index + 1)
        .find_map(|(index, statement)| {
            let StatementNode::Assignment(assignment) = statement else {
                return None;
            };
            let target_field = assignment_member_field_name(expressions, assignment.target)?;
            read_fields
                .iter()
                .any(|field| *field == target_field)
                .then_some(index)
        })
}

/// Collects the member FIELD NAMES an expression reads (`self.v` -> `v`,
/// including nested receivers and operands).
fn collect_member_field_names(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    out: &mut Vec<Identifier>,
) {
    match expressions.expression(expression) {
        ExpressionNode::Member(member) => {
            out.push(member.member.clone());
            collect_member_field_names(expressions, member.receiver, out);
        }
        ExpressionNode::Borrow(inner) => collect_member_field_names(expressions, inner.target, out),
        ExpressionNode::Binary(binary) => {
            collect_member_field_names(expressions, binary.left, out);
            collect_member_field_names(expressions, binary.right, out);
        }
        ExpressionNode::Cast(cast) => collect_member_field_names(expressions, cast.value, out),
        ExpressionNode::Indexed(indexed) => {
            collect_member_field_names(expressions, indexed.collection, out);
            collect_member_field_names(expressions, indexed.index, out);
        }
        ExpressionNode::Unary(unary) => collect_member_field_names(expressions, unary.operand, out),
        _ => {}
    }
}

/// Whether an ASSIGNMENT statement's VALUE references the symbol -- the one use
/// position `statement_references_local` deliberately omits (a slot-less local
/// used as an assignment value is normally covered by the alias fold; the
/// stale-capture clause above must count it because it exists precisely to
/// BLOCK that fold).
fn assignment_value_references_symbol(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };
    expression_references_symbol(expressions, assignment.value, symbol, local_name)
}

/// The member FIELD NAME an assignment target writes (`self.v = ..` -> `v`),
/// peeling `Mutable`/`Indexed` wrappers. A plain-Name target (a local) is None.
fn assignment_member_field_name(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    target: ExpressionHandle,
) -> Option<Identifier> {
    match expressions.expression(target) {
        ExpressionNode::Member(member) => Some(member.member.clone()),
        ExpressionNode::Borrow(inner) => assignment_member_field_name(expressions, inner.target),
        ExpressionNode::Indexed(indexed) => {
            assignment_member_field_name(expressions, indexed.collection)
        }
        _ => None,
    }
}

/// Whether an initializer is an ARRAY literal (possibly wrapped in `Mutable`). An
/// array element read (`arr[i]`) has no immediate form and is not folded back, so a
/// later use needs the array's slot. Deliberately NOT struct literals: a
/// borrow-carrying struct (`Msg { body: &cell }`) must stay FOLDED so the borrow
/// substitutes into `msg.body` -- slotting it would store a stale pointer
/// (regressed `borrow_carrying_data_field`). Value-struct field reads fold to the
/// literal field and do not need a slot.
fn initializer_is_array_literal(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(_) => true,
        ExpressionNode::Borrow(inner) => initializer_is_array_literal(expressions, inner.target),
        _ => false,
    }
}

/// Whether an initializer is a STRUCT literal (under `Mutable`). Used by the
/// slice-view carve-out below: a struct-literal local whose FIELD backs an
/// `as_slice`/`as_mut_slice` view must own a real frame slot (the descriptor
/// needs an address to point at). Deliberately NOT folded into
/// `initializer_is_array_literal`'s any-later-`let` carve-out: a struct-literal
/// local consumed by a plain FIELD READ (`let m = Msg { body: &self.cell };
/// m.body`) is served by the struct-literal field-extraction FOLD and must stay
/// slot-less (slotting it breaks the fold's borrow-carrying shapes -- the
/// borrow_carrying_data_field_exit canary regression caught exactly that).
fn initializer_is_struct_literal(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::StructLiteral(_) => true,
        ExpressionNode::Borrow(inner) => initializer_is_struct_literal(expressions, inner.target),
        _ => false,
    }
}

/// Whether the expression contains an `as_slice`/`as_mut_slice` CALL whose
/// receiver is rooted at `symbol`/`local_name` -- an ADDRESS-TAKING view of the
/// local (`room.exits.as_mut_slice()`). Such a view requires the local to own a
/// real frame slot: the slice descriptor's data pointer must target actual
/// storage, and an rvalue aggregate literal has no address. Recursion mirrors
/// `expression_references_symbol` so a view nested in any position (a later
/// `let` value, a transition argument, a call argument) is found.
fn expression_takes_slice_view_of_symbol(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_takes_slice_view_of_symbol(expressions, atomic.value, symbol, local_name)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && (call.target.as_str() == "as_slice" || call.target.as_str() == "as_mut_slice")
                && expression_references_symbol(expressions, call.receiver, symbol, local_name))
                || (call.receiver.is_valid()
                    && expression_takes_slice_view_of_symbol(
                        expressions,
                        call.receiver,
                        symbol,
                        local_name,
                    ))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(|argument| {
                        expression_takes_slice_view_of_symbol(
                            expressions,
                            argument,
                            symbol,
                            local_name,
                        )
                    })
        }
        ExpressionNode::Borrow(inner) => {
            expression_takes_slice_view_of_symbol(expressions, inner.target, symbol, local_name)
        }
        ExpressionNode::ArrayLiteral(items) => expressions
            .expression_handles(*items)
            .iter()
            .copied()
            .any(|item| {
                expression_takes_slice_view_of_symbol(expressions, item, symbol, local_name)
            }),
        ExpressionNode::Binary(binary) => {
            expression_takes_slice_view_of_symbol(expressions, binary.left, symbol, local_name)
                || expression_takes_slice_view_of_symbol(
                    expressions,
                    binary.right,
                    symbol,
                    local_name,
                )
        }
        ExpressionNode::Cast(cast) => {
            expression_takes_slice_view_of_symbol(expressions, cast.value, symbol, local_name)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_takes_slice_view_of_symbol(
                expressions,
                indexed.collection,
                symbol,
                local_name,
            ) || expression_takes_slice_view_of_symbol(
                expressions,
                indexed.index,
                symbol,
                local_name,
            )
        }
        ExpressionNode::Range(range) => {
            expression_takes_slice_view_of_symbol(expressions, range.start, symbol, local_name)
                || expression_takes_slice_view_of_symbol(expressions, range.end, symbol, local_name)
        }
        ExpressionNode::Member(member) => {
            expression_takes_slice_view_of_symbol(expressions, member.receiver, symbol, local_name)
        }
        ExpressionNode::Unary(unary) => {
            expression_takes_slice_view_of_symbol(expressions, unary.operand, symbol, local_name)
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| {
                expression_takes_slice_view_of_symbol(expressions, field.value, symbol, local_name)
            }),
        ExpressionNode::Name(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// Whether any expression position of the statement takes a slice view of the
/// local (see `expression_takes_slice_view_of_symbol`). Covers `let` VALUES,
/// assignment targets/values, call arguments, expression statements, and
/// transition guards/target arguments -- the positions an `as_mut_slice` view
/// can ride into.
fn statement_takes_slice_view_of_symbol(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::LocalData(local) => {
            local.initial_value.is_valid()
                && expression_takes_slice_view_of_symbol(
                    expressions,
                    local.initial_value,
                    symbol,
                    local_name,
                )
        }
        StatementNode::Assignment(assignment) => {
            expression_takes_slice_view_of_symbol(
                expressions,
                assignment.target,
                symbol,
                local_name,
            ) || expression_takes_slice_view_of_symbol(
                expressions,
                assignment.value,
                symbol,
                local_name,
            )
        }
        StatementNode::Call(call) => expressions
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .any(|argument| {
                expression_takes_slice_view_of_symbol(expressions, argument, symbol, local_name)
            }),
        StatementNode::Expression(expression) => {
            expression.is_valid()
                && expression_takes_slice_view_of_symbol(
                    expressions,
                    *expression,
                    symbol,
                    local_name,
                )
        }
        StatementNode::Transition(transition) => {
            (match transition.guard {
                TransitionGuardNode::Always => false,
                TransitionGuardNode::When(expression) => expression_takes_slice_view_of_symbol(
                    expressions,
                    expression,
                    symbol,
                    local_name,
                ),
            }) || transition_target_takes_slice_view_of_symbol(
                expressions,
                statement_table,
                transition.target,
                symbol,
                local_name,
            ) || transition_target_takes_slice_view_of_symbol(
                expressions,
                statement_table,
                transition.continuation,
                symbol,
                local_name,
            )
        }
    }
}

fn transition_target_takes_slice_view_of_symbol(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    target: psi_checked_trees::statement::TransitionTargetHandle,
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
                expression_takes_slice_view_of_symbol(expressions, expression, symbol, local_name)
            }),
        TransitionTargetNode::Value(expression) => {
            expression_takes_slice_view_of_symbol(expressions, *expression, symbol, local_name)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
    }
}

/// Whether an initializer is a SUBSLICE (`base[a..b]`, a Range index) with at
/// least one RUNTIME (non-literal-integer) bound, peeling `Mutable` at both the
/// initializer and bound levels. An open bound (invalid handle) is static.
/// Literal-bounded subslices fold correctly when elided and must NOT match.
fn initializer_is_runtime_bounded_subslice(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            initializer_is_runtime_bounded_subslice(expressions, inner.target)
        }
        ExpressionNode::Indexed(indexed) => {
            let ExpressionNode::Range(range) = expressions.expression(indexed.index) else {
                return false;
            };
            [range.start, range.end].into_iter().any(|bound| {
                if !bound.is_valid() {
                    return false;
                }
                let mut handle = bound;
                while let ExpressionNode::Borrow(inner) = expressions.expression(handle) {
                    handle = inner.target;
                }
                !matches!(expressions.expression(handle), ExpressionNode::Integer(_))
            })
        }
        _ => false,
    }
}

/// Whether a LocalData (`let`) statement's initializer VALUE references the
/// symbol/name -- the position `statement_references_local` does not inspect.
fn local_data_value_references_symbol(
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => expression_contains_call(expressions, atomic.value),
        ExpressionNode::Call(_) => true,
        ExpressionNode::Borrow(inner) => expression_contains_call(expressions, inner.target),
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
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// Whether an initializer is a RUNTIME-indexed read (`arr[i]` with a
/// non-constant index), possibly wrapped in `Mutable`. Such a local is
/// materialized into its own slot by the whole-value copy path; reading it as a
/// binary/cast operand needs the slot (the indexed read is never folded back,
/// so substitution cannot supply the operand). A CONSTANT-index read folds to a
/// plain place and does not need this carve-out.
fn initializer_is_runtime_indexed_read(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => !matches!(
            expressions.expression(indexed.index),
            ExpressionNode::Integer(_)
        ),
        ExpressionNode::Borrow(inner) => {
            initializer_is_runtime_indexed_read(expressions, inner.target)
        }
        // A field read off a runtime-indexed element (`arr[i].field`) is the same
        // unresolvable-without-a-slot value as the bare read: it is not folded back
        // into an operand position either, so a local initialized from it and used
        // as an arithmetic/comparison/bitwise operand must keep its slot.
        ExpressionNode::Member(member) => {
            initializer_is_runtime_indexed_read(expressions, member.receiver)
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

/// A BITWISE-logical op (`local & 3`, `local | 1`, `local ^ mask`) reads its
/// operands as runtime values, the same unresolvable-without-a-slot position as
/// an arithmetic operand (shifts are already in `is_arithmetic_operator`). A
/// slot-less indexed-read local is not substituted into it, so `let m = t & 6`
/// silently drops it and reads 0 -- so a local used as a bitwise operand must
/// keep its slot too.
fn is_bitwise_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor
    )
}

/// Whether `expression` is directly a reference to `symbol` (a bare `Name`).
fn expression_is_symbol(
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    program: &CheckedTrees,
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let recurse = |handle| {
        expression_uses_symbol_as_arithmetic_operand(
            program,
            expressions,
            handle,
            symbol,
            local_name,
        )
    };
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Binary(binary) => {
            ((is_arithmetic_operator(binary.operator)
                || is_comparison_operator(binary.operator)
                || is_bitwise_operator(binary.operator))
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
            expression_is_symbol(expressions, cast.value, symbol, local_name) || recurse(cast.value)
        }
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Indexed(indexed) => recurse(indexed.collection) || recurse(indexed.index),
        ExpressionNode::Range(range) => recurse(range.start) || recurse(range.end),
        ExpressionNode::Call(call) => {
            // Compiler-known value calls are operators from the runtime-value
            // selector's perspective. A call-initialized local is deliberately
            // NOT substituted back into later expressions (single-evaluation
            // boundary), so using it directly as such an operand requires its
            // own materialized slot just like a binary/cast operand. Ordinary
            // machine-call arguments retain their established substitution path.
            let builtin_consumes_symbol = call.target_symbol.is_valid()
                && program.symbols.get(call.target_symbol).kind == SymbolKind::BuiltinFunction
                && expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(|argument| {
                        expression_is_symbol(expressions, argument, symbol, local_name)
                    });
            builtin_consumes_symbol
                || (call.receiver.is_valid() && recurse(call.receiver))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(recurse)
        }
        ExpressionNode::ArrayLiteral(items) => expressions
            .expression_handles(*items)
            .iter()
            .copied()
            .any(recurse),
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| recurse(field.value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// Whether a later statement uses `symbol` as an arithmetic-binary operand (in a
/// local initializer, an assignment value, a terminal expression, or a call
/// argument).
fn statement_uses_symbol_as_arithmetic_operand(
    program: &CheckedTrees,
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::LocalData(local_data) => expression_uses_symbol_as_arithmetic_operand(
            program,
            expressions,
            local_data.initial_value,
            symbol,
            local_name,
        ),
        StatementNode::Assignment(assignment) => expression_uses_symbol_as_arithmetic_operand(
            program,
            expressions,
            assignment.value,
            symbol,
            local_name,
        ),
        StatementNode::Expression(expression) => expression_uses_symbol_as_arithmetic_operand(
            program,
            expressions,
            *expression,
            symbol,
            local_name,
        ),
        StatementNode::Call(call) => expressions
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .any(|argument| {
                expression_uses_symbol_as_arithmetic_operand(
                    program,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_contains_mutating_call(expressions, atomic.value)
        }
        ExpressionNode::Call(call) => {
            expressions
                .expression_handles(call.arguments)
                .iter()
                .copied()
                .any(|argument| {
                    matches!(expressions.expression(argument), ExpressionNode::Borrow(_))
                        || expression_contains_mutating_call(expressions, argument)
                })
                || (call.receiver.is_valid()
                    && expression_contains_mutating_call(expressions, call.receiver))
        }
        ExpressionNode::Borrow(inner) => {
            expression_contains_mutating_call(expressions, inner.target)
        }
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
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: psi_checked_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    use psi_checked_trees::expression::ExpressionNode;

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
        ExpressionNode::Borrow(inner) => assignment_target_head_symbol(expressions, inner.target),
        _ => SymbolHandle::invalid(),
    }
}

fn assignment_target_head_name(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: psi_checked_trees::expression::ExpressionHandle,
) -> Option<&Identifier> {
    use psi_checked_trees::expression::ExpressionNode;

    match expressions.expression(expression) {
        ExpressionNode::Name(path) => expressions.name_path_members(path.members).first(),
        ExpressionNode::Member(member) => assignment_target_head_name(expressions, member.receiver),
        ExpressionNode::Indexed(indexed) => {
            assignment_target_head_name(expressions, indexed.collection)
        }
        ExpressionNode::Borrow(inner) => assignment_target_head_name(expressions, inner.target),
        _ => None,
    }
}

fn statement_uses_symbol_mutably(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match statement {
        StatementNode::AssemblyFact(_) => false,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    target: psi_checked_trees::statement::TransitionTargetHandle,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    target: psi_checked_trees::statement::TransitionTargetHandle,
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
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_uses_symbol_mutably(expressions, atomic.value, symbol, local_name)
        }
        ExpressionNode::Borrow(inner) => {
            expression_references_symbol(expressions, inner.target, symbol, local_name)
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
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn expression_references_symbol(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_references_symbol(expressions, atomic.value, symbol, local_name)
        }
        ExpressionNode::Name(path) => {
            path.head_symbol == symbol
                || expressions
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| name == local_name)
        }
        ExpressionNode::Borrow(inner) => {
            expression_references_symbol(expressions, inner.target, symbol, local_name)
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
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
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
                let psi_checked_trees::types::TypeConstraintNode::Named(name) = constraint else {
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
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}
