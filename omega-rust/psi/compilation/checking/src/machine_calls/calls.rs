//! Statement-position machine calls: `validate_call_node` resolves what a
//! call targets and validates its result use and arguments; the argument
//! checks shared with value-position calls live below it. The rungs of the
//! target ladder are `call_gates`, `receiverless_calls` and
//! `receiver_calls`; the other children validate one concern each
//! (argument bounds, expression-position scanning, generic bounds and
//! requirements, inline assembly, recursion, result use, Unit returns and
//! write frames).

use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::proof_contracts::arithmetic_domains::ValueEnvironment;
use crate::value_custody::expression_types::declared_reference_access;
use crate::value_custody::expression_types::{
    argument_matches_carrier, expression_type_name_handle, report_cross_class_store,
    report_data_type_conflict,
};
use crate::value_custody::locals::WritableRoots;
use crate::value_custody::places::declared_place_type;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::TableCall;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod argument_bounds;
mod call_gates;
mod dynamic_requirement;
mod expression_scanning;
mod generic_bounds;
mod generic_requirement;
pub use generic_requirement::named_conformance_target_requirement;
mod inline_assembly;
mod machine_parameter_arguments;
mod type_arguments;
use machine_parameter_arguments::validate_machine_parameter_arguments;
pub use type_arguments::call_type_arguments;
mod receiver_calls;
mod receiverless_calls;
mod recursion;
mod result_use;
mod unit_returns;
mod write_frames;

pub use unit_returns::{unit_return_call_is_supported, unit_statement_call_is_supported};

use argument_bounds::report_argument_bounds;
use expression_scanning::receiver_member_chain;
pub(crate) use expression_scanning::{
    declared_receiver_type_reference, report_nested_call_in_local_assignment,
    report_nested_call_in_local_initializer, validate_value_position_calls,
};
pub use expression_scanning::{
    nested_boundary_applications_compose, result_initializer_call_is_supported,
    unit_result_initializer_call_is_supported,
};
use generic_bounds::{
    validate_machine_call_type_parameter_bounds, validate_resolved_target_type_parameter_bounds,
};
pub(crate) use inline_assembly::validate_asm_value_destination;
use inline_assembly::{user_asm_contract, validate_asm_operand_constraint};
pub(crate) use recursion::{
    proof_call_has_structural_descent, validate_proof_machine_recursion,
    validate_self_recursive_call_positions,
};
use result_use::validate_result_use;
pub(crate) use write_frames::CallFrameCaches;
use write_frames::boundary_trait_call_signature;
pub(crate) use write_frames::machine_state_by_symbol;
pub(crate) use write_frames::named_state_transition_subgraph_is_acyclic;
pub use write_frames::{
    AssignmentWriteTarget, CallFrameResolver, CallWriteOrigins, LocalWriteOrigin, LocalWriteSource,
    RecastSource, frame_paths_overlap, operand_call_plans,
    state_reference_parameter_binding_is_stable,
};
pub(crate) use write_frames::{
    boundary_trait_signature, expression_has_calls, free_machine_entry_state,
    statement_value_expression_roots,
};

/// What every statement-call check reads: the program, the call, the
/// machine and state it sits in, the symbol tables, the writable roots and
/// the flow-sensitive value environment.
#[derive(Clone, Copy)]
pub(super) struct CallScope<'a> {
    pub(super) program: &'a TypedTrees,
    pub(super) call: &'a TableCall,
    pub(super) current_machine: &'a Machine,
    pub(super) state_name: &'a str,
    pub(super) current_state: Option<&'a State>,
    pub(super) machine_symbols: &'a MachineSymbols<'a>,
    pub(super) symbols: &'a TopLevelSymbols<'a>,
    pub(super) writable_roots: &'a WritableRoots<'a, 'a>,
    pub(super) value_environment: &'a ValueEnvironment,
}

/// Validates one statement-position call by resolving what it targets, in
/// order: a named conformance requirement, a wire schema call, a declared
/// receiver, an `asm#` intrinsic, the banned self-entry call, then a
/// receiverless target or a receiver's target.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &typed_trees::machine::Machine,
    state_name: &str,
    current_state: Option<&typed_trees::state::State>,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let scope = CallScope {
        program,
        call,
        current_machine,
        state_name,
        current_state,
        machine_symbols,
        symbols,
        writable_roots,
        value_environment,
    };
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let arguments = program.statement_table.expression_handles(call.arguments);
    crate::proof_contracts::contract_entailment::validate_const_range_call_in_environment(
        program,
        current_machine,
        current_state,
        call.target_symbol,
        &call.machine_arguments,
        arguments,
        Some(value_environment),
        diagnostics,
    );
    if call_gates::validate_named_conformance_call(&scope, arguments, diagnostics) {
        return;
    }
    // `Schema::encode(...)` / `Schema::decode(...)`: the wire module owns the
    // synthesized encoder/decoder calls' diagnostics (chapter 20, wire
    // stage 2). This runs before the state-receiver gate below: a schema
    // receiver names a compile-time declaration, never current-state
    // storage, and its resolved symbol may be a module-qualified path that
    // gate cannot see.
    if crate::value_custody::wire::validate_wire_schema_call(
        program,
        call,
        current_machine,
        current_state,
        diagnostics,
    ) {
        return;
    }
    if !call_gates::receiver_is_declared(&scope, receiver_members, arguments, diagnostics) {
        return;
    }
    // Asm intrinsic statements (`asm { hlt }`, `asm { out port, value }`)
    // desugar to calls on unnameable `asm#...` targets -- known-contract
    // instructions with FIXED shapes, validated here instead of against a
    // state signature. (`asm { in dest, port }` is an assignment whose value
    // is the `asm#port_in` call; the value-call path owns it.)
    if receiver_members.is_empty() && call.target.as_str().starts_with("asm#") {
        call_gates::validate_asm_statement_call(&scope, arguments, diagnostics);
        return;
    }
    // `machine-self-call-cycle-ban` (settled 2026-07-13): a STATEMENT-position
    // call to the enclosing
    // machine's OWN ENTRY (`self.drip(n - 1);` as a trailing statement) is
    // tail recursion spelled as a call -- it lowered as a Nested-transition
    // loop and slipped the transition-arm fence. "Banned, if it reads as
    // recursion... go write this as states": repetition is a state
    // transition (`-> target(..)`), never a self-call statement.
    if call_gates::self_entry_call_is_banned(&scope, receiver_members, diagnostics) {
        return;
    }
    if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver.is_self_receiver())
    {
        receiverless_calls::validate_receiverless_call(&scope, arguments, diagnostics);
        return;
    }
    receiver_calls::validate_receiver_call(&scope, receiver_members, arguments, diagnostics);
}

/// Reports the "state `X` expects N argument(s), got M" error when `arguments`
/// does not match the callee's callable parameter count, returning `true` on a
/// mismatch so callers skip the per-argument checks (which zip the two and
/// would misalign). Every parameter corresponds to an authored argument.
/// This is the common arity contract for the statement-position
/// (`validate_call_arguments_handles`) and value-position
/// (`validate_value_call_argument_classes`) paths.
pub(crate) fn report_argument_count_mismatch(
    target_name: &str,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    report_callable_argument_count_mismatch(target_name, parameters.len(), arguments, diagnostics)
}

fn report_callable_argument_count_mismatch(
    target_name: &str,
    callable_parameter_count: usize,
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if arguments.len() != callable_parameter_count {
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` expects {} argument(s), got {}",
            target_name,
            callable_parameter_count,
            arguments.len()
        )));
        return true;
    }
    false
}

fn report_argument_reference_access(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    argument: ExpressionHandle,
    parameter: &StateParameter,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use crate::value_custody::reference_access::{
        ReferenceAccessRefusal, receiving_reference_access_refusal,
    };
    let Some(refusal) = receiving_reference_access_refusal(
        program,
        machine,
        state,
        prior_statements,
        argument,
        parameter.type_reference,
    ) else {
        return false;
    };
    diagnostics.push(Diagnostic::error(match refusal {
        ReferenceAccessRefusal::ExplicitWriteOnlyRequired => format!(
            "argument `{}` for state `{}` requires explicit write-only attenuation; pass `&write ...` (a bare value or `&mut ...` does not establish the no-read contract)",
            parameter.name, target_name,
        ),
        ReferenceAccessRefusal::WriteOnlyMayNotRead => format!(
            "argument `{}` for state `{}` supplies `&write` to a parameter that may read; write-only authority cannot widen to shared or mutable access",
            parameter.name, target_name,
        ),
        ReferenceAccessRefusal::ExclusiveBorrowSourceRequired { root, access } => {
            let label = if access == language_semantics::ReferenceAccess::WriteOnly { "write-only" } else { "mutable" };
            format!("{label} argument `{}` for state `{target_name}` is not writable in this state; the source cannot lend the requested exclusive access", program.symbols.name(root))
        }
        ReferenceAccessRefusal::NamedPlaceRequired { access } => {
            let label = if access == language_semantics::ReferenceAccess::WriteOnly { "write-only" } else { "mutable" };
            format!("{label} argument for state `{target_name}` must be a named place")
        }
        ReferenceAccessRefusal::MutableSourceRequired => format!(
            "argument `{}` for state `{}` is declared `&mut` (`{}`), but the caller lends only immutable access -- pass `&mut ...` or forward a `&mut` binding",
            parameter.name, target_name,
            program.display_type_reference_with_constraints(parameter.type_reference),
        ),
    }));
    true
}

pub(crate) fn resolved_call_result_type(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<TypeReferenceHandle> {
    if let Some((_, state)) = machine_state_by_symbol(program, call.target_symbol) {
        return state.return_type.is_valid().then_some(state.return_type);
    }
    requirement_call_result_type(program, call.target_symbol)
}

/// A boundary or requirement call targets the trait's declared signature
/// symbol rather than a machine-body state. Its declared result is exact
/// evidence only for a closed signature: trait- or requirement-level `Type`
/// parameters and `Self` name declaration-local subjects, so those calls stay
/// unresolved rather than inventing a caller-side carrier.
fn requirement_call_result_type(
    program: &TypedTrees,
    target: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !target.is_valid() {
        return None;
    }
    let parent = program.symbols.get(target).parent;
    let definition = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == parent)?;
    if !definition.type_parameters.is_empty() {
        return None;
    }
    let signature = program
        .trait_machine_signatures(definition)
        .iter()
        .find(|signature| signature.symbol == target)?;
    if !signature.type_parameters.is_empty()
        || !signature.return_type.is_valid()
        || type_reference_mentions_self(program, signature.return_type)
    {
        return None;
    }
    Some(signature.return_type)
}

/// `Self` names the implementing carrier inside the requirement declaration;
/// exporting that leaf as a caller's result type would invent evidence the
/// call never proved. Recurses through transparent and aggregate shells.
fn type_reference_mentions_self(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    let mut pending = vec![reference];
    let mut visited = Vec::new();
    while let Some(reference) = pending.pop() {
        if visited.contains(&reference)
            || !program
                .type_reference_table
                .contains_type_reference(reference)
        {
            continue;
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { name, .. } if name.as_str() == "Self" => return true,
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .copied(),
            ),
            _ => {}
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_arguments_handles(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    callee_state: Option<&State>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let retains_primitive_float_policy = callee_state.is_some_and(|state| {
        machine_state_by_symbol(program, state.symbol).is_some_and(|(machine, _)| {
            primitive_float_operator_retains_argument_policy(program, machine.symbol)
        })
    });
    validate_call_arguments_with_environments(
        program,
        current_machine,
        current_state,
        value_environment,
        arguments,
        target_name,
        parameters,
        writable_roots,
        retains_primitive_float_policy,
        &[],
        diagnostics,
    );
}

/// Primitive Float arithmetic reaches call validation after token lowering.
/// Its sealed operation adapter retains operand policy; an ordinary named
/// requirement cannot acquire that adapter from its namespace or result type.
pub(crate) fn primitive_float_operator_retains_argument_policy(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> bool {
    let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol) else {
        return false;
    };
    if !operator.is_boundary
        || !matches!(
            program.primitive_type_reference(operator.return_type),
            Some(PrimitiveType::F32 | PrimitiveType::F64)
        )
        || typed_trees::operator::primitive_float_binary_semantics(program, operator).is_none()
    {
        return false;
    }
    crate::proof_contracts::float_projection_bindings::semantic_operations::
        is_sealed_toolchain_float_operator(program, operator)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_arguments_with_environments(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    retains_primitive_float_policy: bool,
    argument_environments: &[ValueEnvironment],
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments_with_type_correspondence(
        program,
        current_machine,
        current_state,
        value_environment,
        arguments,
        target_name,
        parameters,
        writable_roots,
        retains_primitive_float_policy,
        argument_environments,
        |argument, required| argument_matches_carrier(program, argument, required),
        diagnostics,
    );
}

/// The selected requirement owns type substitution; permission and value/domain
/// validation remain common to ordinary and bounded calls.
#[allow(clippy::too_many_arguments)]
fn validate_call_arguments_with_type_correspondence(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    retains_primitive_float_policy: bool,
    argument_environments: &[ValueEnvironment],
    type_matches: impl Fn(ExpressionHandle, TypeReferenceHandle) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if report_argument_count_mismatch(target_name, parameters, arguments, diagnostics) {
        return;
    }

    for (argument_index, (argument, parameter)) in
        arguments.iter().zip(parameters.iter()).enumerate()
    {
        let value_environment = argument_environments
            .get(argument_index)
            .unwrap_or(value_environment);
        crate::value_custody::literals::validate_suffix_landing(
            program,
            *argument,
            parameter.type_reference,
            diagnostics,
        );
        crate::value_custody::literals::validate_constant_projection_destination(
            program,
            current_machine.symbol,
            *argument,
            parameter.type_reference,
            diagnostics,
        );
        if report_argument_reference_access(
            program,
            current_machine,
            current_state,
            writable_roots.statements,
            *argument,
            parameter,
            target_name,
            diagnostics,
        ) {
            continue;
        }

        // Mutable-to-shared attenuation changes permission, not carrier or
        // domain. Keep the same type correspondence and proof checks below.

        let expected_type =
            program.display_type_reference_with_constraints(parameter.type_reference);
        let slot_context = format!("argument `{}` for state `{target_name}`", parameter.name);

        if !type_matches(*argument, parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                target_name,
                expected_type,
                expression_type_name_handle(program, *argument)
            )));
        } else if !crate::value_custody::expression_types::report_view_element_argument_mismatch(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            &slot_context,
            diagnostics,
        ) && !report_cross_class_argument(
            program,
            current_machine,
            current_state,
            *argument,
            parameter,
            target_name,
            diagnostics,
        ) {
            // The shape gate blanket-accepts place/name arguments (`self.field`,
            // a local) against ANY primitive parameter, so a `bool` field passed
            // for an `i32` parameter slips through and the backend silently reads
            // it as garbage. Resolve the argument's scalar class and reject a
            // cross-class store, exactly as the assignment path does. Only args
            // that PASSED the shape gate reach here, so cross-class LITERALS (which
            // the shape gate already rejects above) are not double-reported. When
            // the classes DO agree (a same-class numeric arg), check the narrowing
            // obligation -- `take_i8(self.i64_field)` would silently truncate.
            report_argument_bounds(
                program,
                current_machine,
                current_state,
                value_environment,
                *argument,
                parameter,
                target_name,
                retains_primitive_float_policy,
                diagnostics,
            );
        }
        // An array-literal argument (`sink([300, ..])`) is checked element-wise
        // against the parameter's `[T; N]` element type -- the scalar guards above
        // no-op on a non-primitive (array) parameter.
        if let Some(state) = current_state {
            crate::value_custody::struct_literals::validate_array_literal_elements(
                program,
                current_machine,
                state,
                *argument,
                parameter.type_reference,
                diagnostics,
            );
        }
        // Nominal guard: the shape gate blanket-accepts a place/name argument
        // against ANY `Named` parameter, so `take_foo(&self.bar)` (a `&Bar` for a
        // `&Foo` parameter) is silently accepted and reads the wrong storage.
        // Reject when both parameter and argument resolve to concrete data types
        // that differ (every non-data form is skipped, so no false positive on
        // trait/generic parameters or computed arguments).
        report_data_type_conflict(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        // Scalar-vs-data shape guard: `take_struct(5)` (a scalar for a struct param)
        // or `take_int(self.struct)` (a struct for a scalar param). Unlike the
        // array/scalar check below, this is SAFE at the argument position -- it fires
        // only on scalar-vs-DATA-type crossings, and `&buffer`/`addr`/text args
        // involve no data type on either side, so they never trigger it.
        crate::value_custody::expression_types::report_scalar_data_shape_mismatch(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        crate::proof_contracts::domain_weakening::validate_argument_domain_weakening(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            &slot_context,
            retains_primitive_float_policy,
            Some(parameter),
            diagnostics,
        );
        // NOTE: an array/scalar SHAPE check does NOT belong at the argument position
        // -- `&self.msg` (address-of a `[u8; N]` buffer) passed to an `addr`/pointer
        // param is a valid array-value-into-scalar-target flow, and boundary/host
        // text params (`addr`, byte slices) accept text/byte values freely. The
        // reference/`addr` and text representations make args a false-positive
        // minefield; a wrong-count/type arg is already caught here + at the backend.
    }

    let _ = (writable_roots, diagnostics);
}

/// Reject a single ARGUMENT whose scalar class conflicts with its `parameter`'s
/// primitive type -- a `bool`/text value passed where a numeric parameter is
/// expected (or vice versa), which the backend would otherwise read as garbage.
/// Shared by the statement/transition path (`validate_call_arguments_handles`)
/// and the value-position path (`validate_value_call_argument_classes`). Returns
/// `true` if it reported. A non-primitive parameter (a data reference, a struct)
/// or an unresolvable argument class yields `false` -- no report.
fn report_cross_class_argument(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(parameter_primitive) = program.primitive_type_reference(parameter.type_reference)
    else {
        return false;
    };
    let slot_context = format!("argument `{}` for state `{target_name}`", parameter.name);
    report_cross_class_store(
        program,
        Some(current_machine),
        current_state,
        argument,
        parameter_primitive,
        &slot_context,
        "parameter",
        diagnostics,
    )
}

/// Reject cross-class scalar ARGUMENTS at a VALUE-position call site
/// (`let r = self.f(self.bool_field)`). The value-position path validates only
/// type-parameter bounds, so the same cross-class hole the statement/transition
/// paths had applies here. Unlike `validate_call_arguments_handles` there is no
/// shape gate ahead of this, so it also covers literal arguments.
#[allow(clippy::too_many_arguments)]
fn validate_value_call_argument_classes(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    prior_statements: &[typed_trees::statement::StatementNode],
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    callee_machine: &Machine,
    callee_state: &State,
    executes: bool,
    diagnostics: &mut Vec<Diagnostic>,
    static_arguments: &[typed_trees::expression::StaticMachineArgument],
) {
    // (The void-callee-in-value-position check lives in report_void_value_callee:
    // it consults the resolved state's return type AND the callee machine's
    // transition VALUE arms, which is what keying off `callee_state.return_type`
    // alone could not do.)

    // Arity: value-position calls (`let r = self.pick(1)`) reach only this path,
    // never `validate_call_arguments_handles`, so without this a wrong argument
    // count compiled silently (a missing arg then read its ZII default). Safe here
    // because this function runs only on a RESOLVED callee -- the resolver's blind
    // spots fall through earlier without reaching it.
    let parameters = program.state_parameters(callee_state);
    if report_callable_argument_count_mismatch(
        callee_state.name.as_str(),
        parameters.len(),
        arguments,
        diagnostics,
    ) {
        return;
    }

    let Some(type_bindings) = type_arguments::call_type_arguments(
        program,
        current_machine,
        Some(current_state),
        callee_machine,
        callee_state,
        static_arguments,
        arguments,
    ) else {
        diagnostics.push(Diagnostic::error(format!(
            "call `{}` has an invalid static type argument correspondence",
            callee_state.name
        )));
        return;
    };
    // Short-circuit operands remain statically checked, but an invocation
    // that cannot execute has no runtime precondition obligation.
    if executes {
        crate::proof_contracts::contract_entailment::reject_refuted_value_call_requires(
            program,
            current_machine,
            current_state,
            callee_machine,
            callee_state,
            arguments,
            diagnostics,
        );
    }
    let retains_primitive_float_policy =
        primitive_float_operator_retains_argument_policy(program, callee_machine.symbol);
    for (argument, parameter) in arguments
        .iter()
        .zip(program.state_parameters(callee_state).iter())
    {
        if !program.machine_type_parameters(callee_machine).is_empty()
            && !crate::value_custody::expression_types::argument_matches_carrier_with_substitutions(
                program,
                *argument,
                parameter.type_reference,
                &type_bindings,
            )
        {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                callee_state.name,
                program.display_type_reference_with_constraints(parameter.type_reference),
                expression_type_name_handle(program, *argument),
            )));
            continue;
        }
        crate::value_custody::literals::validate_suffix_landing(
            program,
            *argument,
            parameter.type_reference,
            diagnostics,
        );
        crate::value_custody::literals::validate_constant_projection_destination(
            program,
            current_machine.symbol,
            *argument,
            parameter.type_reference,
            diagnostics,
        );
        if report_argument_reference_access(
            program,
            current_machine,
            Some(current_state),
            prior_statements,
            *argument,
            parameter,
            callee_state.name.as_str(),
            diagnostics,
        ) {
            continue;
        }
        let slot_context = format!(
            "argument `{}` for state `{}::{}`",
            parameter.name,
            callee_machine.name,
            callee_state.name.as_str()
        );
        // Narrowing is checked only when the numeric classes agree, so a
        // cross-class argument is not reported twice. This matches the
        // statement/transition path in `validate_call_arguments_handles`.
        if !crate::value_custody::expression_types::report_view_element_argument_mismatch(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            diagnostics,
        ) && !report_cross_class_argument(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter,
            callee_state.name.as_str(),
            diagnostics,
        ) {
            report_argument_bounds(
                program,
                current_machine,
                Some(current_state),
                value_environment,
                *argument,
                parameter,
                callee_state.name.as_str(),
                retains_primitive_float_policy,
                diagnostics,
            );
        }
        // Nominal guard (value-position complement): `let r = self.take_foo(&self.bar)`
        // with a `&Foo` parameter is silently accepted -- the same wrong-data-type
        // hole the statement/transition path has.
        report_data_type_conflict(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        // Scalar-vs-data shape guard -- safe at the argument position (see the twin
        // call in `validate_call_arguments_handles`): fires only on scalar-vs-DATA
        // crossings, which `&buffer`/`addr`/text args never are.
        crate::value_custody::expression_types::report_scalar_data_shape_mismatch(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        crate::proof_contracts::domain_weakening::validate_argument_domain_weakening(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            retains_primitive_float_policy,
            Some(parameter),
            diagnostics,
        );
        // (No array/scalar shape check here -- see the note in
        // `validate_call_arguments_handles`: `&buffer`-into-`addr` and text/byte args
        // make the argument position a false-positive minefield.)
    }
}
