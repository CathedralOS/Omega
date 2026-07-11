mod arithmetic_domains;
mod call_cycles;
mod calls;
mod contract_entailment;
mod data;
mod domains;
mod effects;
mod entry_point;
mod expression_types;
mod invariants;
mod literals;
mod locals;
mod machine_data;
mod operators;
mod places;
mod proof_facts;
mod recasts;
mod proof_only_faces;
mod properties;
mod state_signatures;
mod struct_literals;
mod symbols;
#[cfg(test)]
mod tests;
mod traits;
mod transitions;
mod type_references;
mod wire;

use crate::calls::{
    validate_call_node, validate_self_recursive_call_positions, validate_value_position_calls,
};
use crate::contract_entailment::validate_machine_contract_entailment;
use crate::data::validate_data_field_types;
use crate::domains::validate_domain_definitions;
use crate::entry_point::validate_entry_point;
use crate::expression_types::{ExpressionTypeOwner, validate_expression_type_handle};
use crate::invariants::validate_invariant_definitions;
use crate::locals::{WritableRoots, validate_local_data_names};
use crate::machine_data::{
    validate_contained_types, validate_owned_data, validate_version_scoped_target,
};
use crate::places::validate_assignment_target_handle;
use crate::state_signatures::{
    StateSignatureOwner, validate_callable_state_signatures, validate_machine_contracts,
    validate_machine_effects,
};
use crate::symbols::MachineSymbols;
pub use crate::symbols::TopLevelSymbols;
use crate::traits::{
    validate_data_conformances, validate_machine_trait_conformances, validate_trait_requirements,
};
use crate::transitions::validate_transition_target_node;
use crate::type_references::{TypeReferenceOwner, validate_type_reference_handle};
pub use effects::validate_effect_plan;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};
/// The declared type of a simple place argument (bare name / `self.field`,
/// through the `&mut` marker), WITH its Constrained shells -- exposed for the
/// typed-trees machine-monomorphization pass's param-position inference.
pub use places::declared_place_type_raw;
pub use places::unwrapped_type_reference;
pub use properties::{declared_property_names, type_satisfies_declared_property};

pub fn validate_program(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    let fact_plan = omega_facts::build_definition_fact_plan(program);

    literals::validate_literal_widths(program, &mut diagnostics);
    validate_domain_definitions(program, &symbols, &fact_plan, &mut diagnostics);
    validate_invariant_definitions(program, &fact_plan, &mut diagnostics);
    validate_callable_state_signatures(program, &symbols, &mut diagnostics);
    validate_trait_requirements(program, &mut diagnostics);
    validate_data_conformances(program, &symbols, &mut diagnostics);
    validate_data_field_types(program, &symbols, &mut diagnostics);
    // Math roster N1: recursive data is legal and PROOF-ONLY (computed, never
    // spelled); every runtime consumption face refuses with the
    // classification named.
    let proof_only = omega_typed_trees::proof_only::classify(program);
    proof_only_faces::validate_proof_only_consumption(program, &proof_only, &mut diagnostics);
    // Q6 ruling: machine call cycles are banned regardless of boundedness.
    call_cycles::validate_machine_call_cycles(program, &symbols, &mut diagnostics);
    data::validate_zero_reachable_field_ranges(program, &mut diagnostics);
    properties::validate_data_properties(program, &symbols, &mut diagnostics);
    // Bare-payload-case `==` (decision 11) is checked on the RESOLVED trees,
    // before membership lowering synthesizes its internal tag compares; see
    // omega-symbol-resolved-trees-to-typed-trees/src/equality.rs.
    struct_literals::validate_struct_literal_fields(program, &mut diagnostics);
    recasts::validate_recasts(program, &mut diagnostics);
    wire::validate_wire_schemas(program, &symbols, &mut diagnostics);
    operators::validate_operator_declarations(program, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    for machine in program.machines() {
        let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);

        // Drop bodies are NOT executed yet (ch16's lowered drop edge is
        // aspirational): a non-empty `drop` body would be a SILENT NO-OP --
        // unlock/close/flush cleanup that never runs. FENCE it (settled,
        // Zach 2026-07-07): a clean error until drop lowering lands as one
        // ownership subsystem (drop timing needs its own design brief first
        // -- Omega has no lexical scopes). EMPTY drop bodies (the declared
        // teardown-obligation shape) stay valid.
        if machine.name.as_str().ends_with("::drop")
            && program.machine_states(machine).iter().any(|state| {
                !program
                    .statement_table
                    .statements(state.statement_nodes)
                    .is_empty()
            })
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has a non-empty `drop` body, but drop bodies are not yet executed -- the cleanup would silently never run. Keep the `drop` machine empty (the teardown obligation is still tracked), or perform the cleanup explicitly before the value's last use.",
                machine.name,
            )));
        }

        validate_contained_types(program, machine, &symbols, &mut diagnostics);
        validate_owned_data(program, machine, &symbols, &mut diagnostics);
        validate_version_scoped_target(program, machine, &mut diagnostics);
        validate_machine_effects(program, machine, &mut diagnostics);
        validate_machine_contracts(program, machine, &mut diagnostics);
        validate_machine_contract_entailment(program, machine, &mut diagnostics);
        validate_machine_trait_conformances(program, machine, &mut diagnostics);

        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            // A state that DECLARES a return type but has an EMPTY body can
            // never produce the value -- callers would silently bind 0 (ZII),
            // and native/interp diverge on what that zero reads as. GENERIC
            // machines are exempt: the core/std container surface (`machine
            // Vec::as_slice<T>(&self) -> &[T] { }`) is deliberately
            // type-check-only, and value calls to generics are already fenced
            // (fence_generic_value_callee).
            if state.return_type.is_valid()
                && program.machine_type_parameters(machine).is_empty()
                && program
                    .statement_table
                    .statements(state.statement_nodes)
                    .is_empty()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` declares a return type but its body is \
                     empty -- it can never produce the value (callers would silently \
                     bind 0). Return a value or drop the `-> T`.",
                    machine.name,
                    state.name.as_str(),
                )));
            }
            validate_local_data_names(
                program.statement_table.statements(state.statement_nodes),
                &machine_symbols,
                program.state_parameters(state),
                machine.name.as_str(),
                state.name.as_str(),
                &mut diagnostics,
            );
            let writable_roots = WritableRoots {
                program,
                machine_symbols: &machine_symbols,
                statements: program.statement_table.statements(state.statement_nodes),
                parameters: program.state_parameters(state),
            };

            // S4: a per-state-body value environment tracks each place's proven
            // interval along the straight-line prefix, so the exact-overflow proof
            // can use actual values (`self.v = 10; self.v += 5`) instead of the
            // full type range. The ENTRY state (first) is pre-seeded with the
            // machine's `requires` bounds on its parameters (`requires amount <=
            // 100`), so bounded param arithmetic stays exact. Statements are
            // validated in order so the env is current at each use.
            let mut value_env = if state_index == 0 {
                arithmetic_domains::requires_value_env(program, machine)
            } else {
                // A non-entry state entered by exactly ONE guarded transition
                // may assume that guard at entry -- the target-state twin of
                // the same-state arm narrowing (`transition dir >= 0 && dir <=
                // 1 { true -> store() }` seeds `store` with dir in [0, 1]).
                arithmetic_domains::sole_incoming_guard_env(program, machine, state)
            };
            for statement in program.statement_table.statements(state.statement_nodes) {
                // VALUE-position calls inside this statement's expression trees
                // (LocalData initializers, transition arguments, guard subjects,
                // etc.) are not reached by `validate_state_statement_node`; run
                // their bound + argument checks here, BEFORE the statement records
                // its own effects, so the flow-sensitive `value_env` reflects the
                // state in which the arguments are actually evaluated (a narrowing
                // arg like `self.take_i8(self.big)` is judged against `big`'s
                // pre-statement interval).
                validate_value_position_calls(
                    program,
                    machine,
                    state,
                    statement,
                    &machine_symbols,
                    &symbols,
                    &writable_roots,
                    &value_env,
                    &mut diagnostics,
                );
                validate_self_recursive_call_positions(
                    program,
                    machine,
                    state,
                    statement,
                    &mut diagnostics,
                );
                validate_state_statement_node(
                    program,
                    machine,
                    &state.name,
                    &machine_symbols,
                    &symbols,
                    &writable_roots,
                    statement,
                    &mut value_env,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_state_statement_node(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement: &StatementNode,
    value_env: &mut arithmetic_domains::ValueEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            validate_assignment_target_handle(
                program,
                assignment.target,
                writable_roots,
                diagnostics,
                machine,
                machine_symbols.state(state_name),
                state_name,
            );
            // Indexing the `String` carrier as an assignment TARGET (`s[i] = x`) is
            // the write complement of the read rejection in `scan_expression_calls`:
            // an indexed `String` resolves to no element type, so every store check
            // below skips it and the write silently no-ops / corrupts. Same shared
            // helper, `is_write = true`.
            expression_types::report_string_index_access(
                program,
                machine,
                machine_symbols.state(state_name),
                assignment.target,
                true,
                diagnostics,
            );
            let assignment_target_type = places::declared_place_type(
                program,
                machine,
                machine_symbols.state(state_name),
                assignment.target,
            )
            // An indexed target (`self.xs[i] = ..`) has no member path, so
            // `declared_place_type` returns None -- fall back to the array/slice
            // ELEMENT type so the store checks below see the real slot type.
            .or_else(|| {
                places::declared_indexed_element_type(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.target,
                )
            });
            let assignment_target_primitive =
                assignment_target_type.and_then(|handle| program.primitive_type_reference(handle));
            // An array-literal RHS into a `[T; N]` target: check each element's
            // class + narrowing against T. The scalar guards below skip a non-
            // primitive (array) target, so this is the element-level complement.
            if let Some(current_state) = machine_symbols.state(state_name)
                && let Some(target_type) = assignment_target_type
            {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    current_state,
                    assignment.value,
                    target_type,
                    diagnostics,
                );
            }
            let owner = format!("machine `{}` state `{state_name}` assignment", machine.name);
            // Cross-class scalar guard: `self.i32 = true` (a bool into a numeric
            // field) is a soundness hole -- the backend silently stores `1` --
            // whether the bool arrives as a literal or through a `self.bool_field`
            // place. Reject the unambiguous cross-class cases before value-range
            // analysis (which assumes a class-compatible RHS).
            if let Some(target_primitive) = assignment_target_primitive {
                expression_types::report_cross_class_store(
                    program,
                    Some(machine),
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_primitive,
                    &owner,
                    "place",
                    diagnostics,
                );
            }
            // Nominal guard: `self.foo = self.bar` (a `Bar` value into a `Foo` place)
            // is silently accepted -- the wrong-data-type complement of the scalar
            // guard above.
            if let Some(target_type) = assignment_target_type {
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
                // Shape guard: `self.scalar = self.xs` (array into a scalar place) --
                // caught by the backend today (a crude `NeedsMachineOwnedWrite`); this
                // gives a clear frontend message and covers the mirror.
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
                // Scalar-vs-data shape guard: `self.struct_field = 5` / `self.scalar
                // = self.struct` (a scalar into a struct slot or the mirror), between
                // the scalar-class and nominal gates.
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
            }
            calls::report_nested_call_in_bound_value_call(
                program,
                machine,
                state_name,
                assignment.value,
                diagnostics,
            );
            calls::report_local_receiver_value_call(
                program,
                machine,
                state_name,
                assignment.value,
                diagnostics,
            );
            let before = diagnostics.len();
            let assignment_target_domain = assignment_target_type
                .map(|handle| program.arithmetic_domain_for_type_reference(handle))
                .unwrap_or(omega_core::arithmetic::ArithmeticDomain::Exact);
            let (interval, source_primitive) = arithmetic_domains::validate_value_range(
                program,
                machine,
                machine_symbols.state(state_name),
                assignment.value,
                value_env,
                assignment_target_primitive,
                assignment_target_domain,
                &owner,
                diagnostics,
            );
            // Only a CLEANLY-analyzed RHS reaches the narrowing check -- an RHS that
            // already erred (its own overflow, a type error) is not re-flagged.
            if diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    assignment_target_primitive,
                    interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
                // Containment for non-literal stores into ranged places (see
                // the local arm's twin note; literal stores refuse through
                // the proof plan).
                if let Some(handle) = assignment_target_type {
                    arithmetic_domains::check_range_containment(
                        program, handle, interval, &owner, diagnostics,
                    );
                }
            }
            arithmetic_domains::record_assignment(
                value_env,
                arithmetic_domains::place_path(program, assignment.target),
                interval,
                places::declared_place_type_raw(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.target,
                )
                .and_then(|handle| {
                    arithmetic_domains::enforced_declared_range(program, handle)
                }),
            );
        }
        StatementNode::Call(call) => {
            validate_call_node(
                program,
                call,
                machine,
                state_name,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                diagnostics,
            );
            // A call may mutate fields through `&mut`, so the linear value
            // environment is no longer trustworthy -- drop it (sound: subsequent
            // places fall back to their type bounds).
            value_env.clear();
        }
        StatementNode::Expression(expression) => {
            let Some(state) = machine_symbols.state(state_name) else {
                return;
            };

            if !state.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` has a terminal expression but no return type",
                    machine.name
                )));
                return;
            }

            let shape_before = diagnostics.len();
            validate_expression_type_handle(
                program,
                *expression,
                state.return_type,
                diagnostics,
                ExpressionTypeOwner::StateTerminalExpression {
                    machine: machine.name.as_str(),
                    state: state_name,
                },
            );
            let owner = format!(
                "machine `{}` state `{state_name}` terminal expression",
                machine.name
            );
            let return_primitive = program.primitive_type_reference(state.return_type);
            // The shape gate above rejects a cross-class LITERAL terminal return
            // (`-> i32 { true }`) but blanket-accepts place/name values, so a bool
            // FIELD returned from an `-> i32` machine slips. Add the class check for
            // those -- only when the shape gate did not already error, so a literal
            // is not double-reported.
            if diagnostics.len() == shape_before {
                let slot_context = format!("machine `{}` state `{state_name}`", machine.name);
                if let Some(target_primitive) = return_primitive {
                    expression_types::report_cross_class_store(
                        program,
                        Some(machine),
                        Some(state),
                        *expression,
                        target_primitive,
                        &slot_context,
                        "return value",
                        diagnostics,
                    );
                }
                // Nominal guard: `-> Foo { self.bar }` returns a `Bar` as a `Foo`.
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
                // Shape guard: `-> i32 { self.xs }` returns an array as a scalar.
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
            }
            let before = diagnostics.len();
            let (return_interval, source_primitive) = arithmetic_domains::validate_value_range(
                program,
                machine,
                Some(state),
                *expression,
                value_env,
                return_primitive,
                program.arithmetic_domain_for_type_reference(state.return_type),
                &owner,
                diagnostics,
            );
            // A cleanly-analyzed return value that cannot fit the declared return
            // type is a silent narrowing (`-> i8 { 300 }`), same as a store.
            if diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    return_primitive,
                    return_interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
            }
            // S4: enforce a declared return `[a..=b]` so call-site narrowing
            // that trusts it stays sound (the interval is already computed above).
            arithmetic_domains::enforce_declared_return_range(
                program,
                state.return_type,
                return_interval,
                &owner,
                diagnostics,
            );
        }
        StatementNode::LocalData(local_data) => {
            validate_type_reference_handle(
                program,
                local_data.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateLocalData {
                    machine: machine.name.as_str(),
                    state: state_name,
                    local: local_data.name.as_str(),
                    generic_depth: 0,
                },
            );
            let local_target_primitive =
                program.primitive_type_reference(local_data.type_reference);
            // An array-literal initializer (`let a: [T; N] = [300, ..]`) is checked
            // element-wise against T.
            if let Some(current_state) = machine_symbols.state(state_name) {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    diagnostics,
                );
            }
            let owner = format!(
                "machine `{}` state `{state_name}` local `{}`",
                machine.name,
                local_data.name.as_str()
            );
            // Cross-class guard: `let x: i32 = true` stores a bool into a numeric
            // local -- a silent miscompile, same as the assignment / arg / field
            // positions. Only an INITIALIZED `let` has a value to class-check (a
            // bare `let x: bool;` filled later by an `&mut` out-param has an invalid
            // initializer). (Narrowing on the initializer is checked below.)
            if local_data.initial_value.is_valid()
                && let Some(target_primitive) = local_target_primitive
            {
                expression_types::report_cross_class_store(
                    program,
                    Some(machine),
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    target_primitive,
                    &owner,
                    "local",
                    diagnostics,
                );
            }
            // Nominal guard: `let f: Foo = self.bar` binds a `Bar` value to a `Foo`
            // local -- wrong data type. Only an initialized `let` has a value.
            if local_data.initial_value.is_valid() {
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                // Shape guard: `let y: i32 = self.xs` (array -> scalar) or
                // `let xs: [i32; 3] = 5` (scalar -> array) otherwise bind a
                // wrong-shaped value silently (the array case read a ZII 0).
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
            }
            calls::report_nested_call_in_bound_value_call(
                program,
                machine,
                state_name,
                local_data.initial_value,
                diagnostics,
            );
            calls::report_local_receiver_value_call(
                program,
                machine,
                state_name,
                local_data.initial_value,
                diagnostics,
            );
            let before = diagnostics.len();
            let (interval, source_primitive) = arithmetic_domains::validate_value_range(
                program,
                machine,
                machine_symbols.state(state_name),
                local_data.initial_value,
                value_env,
                local_target_primitive,
                program.arithmetic_domain_for_type_reference(local_data.type_reference),
                &owner,
                diagnostics,
            );
            // A cleanly-analyzed initializer whose value cannot fit the declared
            // local type is a silent narrowing (`let x: i8 = 300`).
            if local_data.initial_value.is_valid() && diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    local_target_primitive,
                    interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
                // Containment against a declared Exact `[a..=b]`: literal
                // initializers refuse through the proof plan, but a
                // NON-LITERAL initializer's interval was never checked --
                // `let idx: u32 [0..=11] = <expr provably up to 12>` stored
                // unproven and the index prover then TRUSTED the range (a
                // confirmed native OOB read, found landing the R3 product
                // rule). The enforced range only exists under Exact shells,
                // so non-Exact declarations are untouched (Q9's own gate
                // already rejects range+domain combinations).
                arithmetic_domains::check_range_containment(
                    program,
                    local_data.type_reference,
                    interval,
                    &owner,
                    diagnostics,
                );
            }
            if local_data.initial_value.is_valid() {
                arithmetic_domains::record_assignment(
                    value_env,
                    Some(local_data.name.as_str().to_owned()),
                    interval,
                    local_data
                        .type_reference
                        .is_valid()
                        .then(|| {
                            arithmetic_domains::enforced_declared_range(
                                program,
                                local_data.type_reference,
                            )
                        })
                        .flatten(),
                );
            }
        }
        StatementNode::Transition(transition) => {
            validate_transition_target_node(
                program,
                machine,
                machine_symbols.state(state_name),
                value_env,
                transition.target,
                machine_symbols,
                symbols,
                writable_roots,
                diagnostics,
            );

            if transition.continuation.is_valid() {
                validate_transition_target_node(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    value_env,
                    transition.continuation,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    diagnostics,
                );
            }

            // Decision 17 completeness: an arm fires only when its guard holds, so
            // BOTH a value return (`n > 0 { true -> (n - 1) }`) and a call argument
            // (`count_down(n - 1)`) may assume the guard's bound. Narrow the env by
            // the arm's guard once, up front, for both. `guard_narrowed_env`
            // intersects each bounded place with its type range (so a one-sided
            // `n > 0` keeps the type's other end) and negates for the `false` arm,
            // so an unguarded (or wrong-arm) decrement is still correctly rejected.
            let narrowed = arithmetic_domains::guard_narrowed_env(
                program,
                machine,
                machine_symbols.state(state_name),
                &transition.guard,
                value_env,
            );

            // Fall-through complement: an exit-if-true transition (valid
            // target, no `_` arm) leaves the guard REFUTED for every later
            // statement in this state -- the MR2 terminal-tail shape's
            // `n - 1` after `transition n == 0 { true -> exit }`.
            if transition.target.is_valid() && !transition.continuation.is_valid() {
                *value_env = arithmetic_domains::fall_through_narrowed_env(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    &transition.guard,
                    value_env,
                );
            }

            // A transition VALUE target (`_ -> (expr)`) is a return value. When the
            // state's return type declares a `[a..=b]`, enforce the value is provably
            // within it (so call-site narrowing that trusts the range is sound); the
            // exact-overflow + narrowing obligations apply too. Gated inside
            // `validate_return_value_range`, so plain returns are unaffected.
            if let Some(state) = machine_symbols.state(state_name)
                && state.return_type.is_valid()
            {
                for target in [transition.target, transition.continuation] {
                    if !target.is_valid() {
                        continue;
                    }
                    if let TransitionTargetNode::Value(return_expression) =
                        program.statement_table.transition_target(target)
                    {
                        // Cross-class guard: `_ -> (true)` returning a bool from an
                        // i32-returning machine is a silent miscompile. The terminal
                        // `{ true }` return form is caught by the general shape gate;
                        // the transition-VALUE form was not. Class complement of the
                        // range/overflow/narrowing check below.
                        if let Some(return_primitive) =
                            program.primitive_type_reference(state.return_type)
                        {
                            expression_types::report_cross_class_store(
                                program,
                                Some(machine),
                                Some(state),
                                *return_expression,
                                return_primitive,
                                &format!("machine `{}` state `{state_name}`", machine.name),
                                "return value",
                                diagnostics,
                            );
                        }
                        // Nominal guard: `-> Foo { transition { _ -> (self.bar) } }`
                        // returns a `Bar` as a `Foo`.
                        expression_types::report_data_type_conflict(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        // Shape guard: an array returned as a scalar (or vice versa).
                        expression_types::report_array_scalar_shape_mismatch(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        expression_types::report_scalar_data_shape_mismatch(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        arithmetic_domains::validate_return_value_range(
                            program,
                            machine,
                            state,
                            *return_expression,
                            &narrowed,
                            &format!(
                                "machine `{}` state `{state_name}` return value",
                                machine.name
                            ),
                            diagnostics,
                        );
                    }
                }
            }

            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                if let TransitionTargetNode::Named { arguments, .. } =
                    program.statement_table.transition_target(target)
                {
                    for argument in program.statement_table.expression_handles(*arguments) {
                        arithmetic_domains::validate_arithmetic_domains(
                            program,
                            machine,
                            machine_symbols.state(state_name),
                            *argument,
                            &narrowed,
                            None,
                            omega_core::arithmetic::ArithmeticDomain::Exact,
                            &format!(
                                "machine `{}` state `{state_name}` transition argument",
                                machine.name
                            ),
                            diagnostics,
                        );
                    }
                }
            }
        }
    }
}
