//! Statement-position machine calls: `validate_call_node` resolves what a
//! call targets and validates its result use and arguments; the argument
//! checks shared with value-position calls live below it. Argument admission
//! has one order regardless of result use: literal destination, reference
//! authority, carrier correspondence, then element, scalar and domain facts.
//! Checking carriers first can hide an access violation behind a shape error.
//! Callee selection and execution-dependent preconditions remain with each
//! invocation site; neither grants argument authority. The rungs of the
//! target ladder are `call_gates`, `receiverless_calls` and
//! `receiver_calls`; the other children validate one concern each
//! (argument bounds, expression-position scanning, generic bounds and
//! requirements, inline assembly, recursion, result use, Unit returns and
//! write frames).

use crate::declarations::symbols::TopLevelSymbols;
use crate::proof_contracts::arithmetic_domains::ValueEnvironment;
use crate::value_custody::expression_types::declared_reference_access;
use crate::value_custody::expression_types::{
    argument_matches_carrier, expression_type_name_handle, report_cross_class_store,
    report_data_type_conflict,
};
use crate::value_custody::locals::WritableRoots;
use crate::value_custody::places::declared_place_type;
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::TableCall;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod closed_dispatch;
pub use closed_dispatch::closed_requirement_dispatch_is_valid;
pub(crate) use closed_dispatch::{capture_binding_is_exact, conformance_binding_is_exact};
mod argument_bounds;
mod call_gates;
pub(crate) mod dynamic_requirement;
mod expression_scanning;
mod generic_bounds;
mod generic_requirement;
pub use generic_requirement::{
    named_conformance_target_requirement, selected_requirement_signature,
};
mod inline_assembly;
mod machine_parameter_arguments;
mod type_arguments;
use machine_parameter_arguments::validate_machine_parameter_arguments;
pub use type_arguments::call_type_arguments;
mod receiver_calls;
mod receiverless_calls;
mod recursion;
mod result_use;
pub(crate) mod signature_queries;
mod unit_returns;
mod write_frames;

pub use unit_returns::{unit_return_call_is_admitted, unit_statement_call_is_admitted};

use argument_bounds::report_argument_bounds;
use expression_scanning::receiver_member_chain;
pub(crate) use expression_scanning::{
    declared_receiver_type_reference, report_nested_call_in_local_initializer,
    validate_value_position_calls,
};
pub use expression_scanning::{
    nested_boundary_applications_compose, result_initializer_call_is_admitted,
    unit_result_initializer_call_is_admitted,
};
use generic_bounds::{
    validate_machine_call_type_parameter_bounds, validate_resolved_target_type_parameter_bounds,
};
pub(crate) use inline_assembly::validate_asm_value_destination;
use inline_assembly::{register_read_mnemonic, user_asm_contract, validate_asm_operand_constraint};
pub use recursion::ValidatedSelfCallDescent;
pub(crate) use recursion::{
    proof_call_has_structural_descent, validate_proof_machine_recursion,
    validate_self_recursive_call_positions,
};
use result_use::validate_result_use;
pub use write_frames::CallFrameTables;
use write_frames::boundary_trait_call_signature;
pub(crate) use write_frames::machine_state_by_symbol;
pub(crate) use write_frames::named_state_transition_subgraph_is_acyclic;
pub use write_frames::{
    AssignmentStorageOrigins, AssignmentWriteTarget, CallFrameResolver, CallWriteOrigins,
    LocalWriteOrigin, LocalWriteSource, RecastSource, ReferenceResultLeafOrigin,
    ScalarStorageCoordinate, assignment_stored_value_type, frame_coordinate_path,
    frame_length_coordinate_path, frame_paths_overlap, operand_call_plans,
    state_reference_parameter_binding_is_stable,
};
pub(crate) use write_frames::{CallFrameCaches, InvocationReferenceSubject};
pub(crate) use write_frames::{
    boundary_trait_signature, expression_has_calls, free_machine_entry_state,
    statement_value_expression_roots,
};

/// What every statement-call check reads: the program, the call, the
/// machine and state it sits in, the symbol tables, the writable roots and
/// the flow-sensitive value environment.
#[derive(Clone, Copy)]
pub(super) struct CallScope<'a> {
    pub(super) program: &'a dyn crate::CheckingInput,
    pub(super) call: &'a TableCall,
    pub(super) current_machine: &'a Machine,
    pub(super) state_name: &'a str,
    pub(super) current_state: Option<&'a State>,
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
    program: &dyn crate::CheckingInput,
    call: &TableCall,
    current_machine: &typed_trees::machine::Machine,
    state_name: &str,
    current_state: Option<&typed_trees::state::State>,
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
/// (`validate_call_arguments`) and value-position
/// (`validate_value_call_arguments`) paths.
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
    program: &dyn crate::CheckingInput,
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
    // The parameter's `&mut` is the callee's spelling, not the caller's: a
    // bare window argument lends shared access until spelled `&mut ...`.
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
        ReferenceAccessRefusal::BareRangeFormsSharedView => format!(
            "argument `{}` for state `{}` is declared `&mut` (`{}`), but a bare range forms a shared view and the caller lends only immutable access; `bare-range-reference-access` spells the mutable subrange `&mut collection[start..end]`",
            parameter.name, target_name,
            program.display_type_reference_with_constraints(parameter.type_reference),
        ),
    }));
    true
}

/// Prefer the occurrence's applied result. A declaration's raw result still
/// contains its own static and region binders; consumers must not compare those
/// foreign binders with a caller's type after preparation applied the call.
pub(crate) fn call_result_type(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let call = program.expression_table.call_node(expression)?;
    let applied = program
        .expression_table
        .constructed_value_type_reference(expression);
    applied
        .is_valid()
        .then_some(applied)
        .or_else(|| resolved_call_result_type(program, call))
}

pub(crate) fn resolved_call_result_type(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<TypeReferenceHandle> {
    if let Some((_, state)) = machine_state_by_symbol(program, call.target_symbol) {
        return state.return_type.is_valid().then_some(state.return_type);
    }
    // A selector call's slot carries the requirement's result instantiated
    // under the selector's trait arguments.
    if let Some(slot) = program.dynamic_slot_by_symbol(call.target_symbol) {
        return slot.return_type.is_valid().then_some(slot.return_type);
    }
    requirement_call_result_type(program, call.target_symbol)
}

/// The authored contract a call's operands answer to: a machine's entry
/// signature, or the trait requirement a selector call's slot instantiates.
/// The requirement's own formals stay the contract's names; the slot's
/// leading selector is an operand no formal names.
pub(crate) struct CalleeContract<'program> {
    /// Owner of the reserved `result` binder in `contracts`.
    pub(crate) callable: symbols::SymbolHandle,
    pub(crate) return_type: TypeReferenceHandle,
    pub(crate) parameters: &'program [typed_trees::signature::StateParameter],
    pub(crate) contracts: &'program [typed_trees::signature::SignatureContract],
    /// Call operands before the first formal: a selector call's selector.
    pub(crate) leading_operands: usize,
}

/// The contract a call to `target` enters through, or `None` when the target
/// is not a machine entry or selector slot. A call to a later state is a jump
/// inside its machine and owes no entry contract here.
pub(crate) fn callee_entry_contract(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<CalleeContract<'_>> {
    if let Some((machine, state)) = machine_state_by_symbol(program, target) {
        if program.machine_states(machine).first()?.symbol != state.symbol {
            return None;
        }
        return Some(CalleeContract {
            callable: machine.symbol,
            return_type: state.return_type,
            parameters: program.state_parameters(state),
            contracts: program.machine_contracts(machine),
            leading_operands: 0,
        });
    }
    let slot = program.dynamic_slot_by_symbol(target)?;
    let requirement = program.dynamic_slot_requirement(slot)?;
    Some(CalleeContract {
        callable: requirement.symbol,
        return_type: requirement.return_type,
        parameters: program.state_signature_parameters(requirement),
        contracts: program.state_signature_contracts(requirement),
        leading_operands: 1,
    })
}

/// A boundary or requirement call targets the trait's declared signature
/// symbol rather than a machine-body state. A retained closed application owns
/// its already-substituted signature, even when the authored trait has `Type`
/// parameters. Invocation lifetimes remain universal; this carrier query does
/// not establish which caller loan supplies the result. Without that exact row,
/// declaration-local `Type` parameters keep the raw result unresolved rather
/// than inventing an application from its expected destination.
fn requirement_call_result_type(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !target.is_valid() {
        return None;
    }
    if let Some(application) = program.closed_requirement_application(target) {
        let signature = application.signature;
        return (signature.type_parameters.is_empty() && signature.return_type.is_valid())
            .then_some(signature.return_type);
    }
    let (definition, signature) = selected_requirement_signature(program, target)?;
    if !definition.type_parameters.is_empty() && signature.symbol == target {
        return None;
    }
    if !signature.type_parameters.is_empty() || !signature.return_type.is_valid() {
        return None;
    }
    Some(signature.return_type)
}

/// Literal formation belongs to the argument, not to whether its call is a
/// statement or produces a value. Run landing, constant-projection and recursive
/// array formation checks together before the permission and class gates.
fn validate_literal_argument_destination(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    argument: ExpressionHandle,
    parameter_type: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    crate::value_custody::literals::validate_suffix_landing(
        program,
        argument,
        parameter_type,
        diagnostics,
    );
    crate::value_custody::literals::validate_constant_projection_destination(
        program,
        current_machine.symbol,
        argument,
        parameter_type,
        diagnostics,
    );
    if let Some(state) = current_state {
        crate::value_custody::struct_literals::validate_array_literal_elements(
            program,
            current_machine,
            state,
            argument,
            parameter_type,
            diagnostics,
        );
    }
}

/// A selected sealed float operation retains each evaluated operand policy
/// through its result adapter. Spelled arithmetic and named requirements use
/// the same identity and semantic-result contract; a namespace or float return
/// type alone grants no policy transport.
pub(crate) fn selected_float_operation_retains_argument_policy(
    program: &dyn crate::CheckingInput,
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
        || !(crate::float_result_semantic_contract(program, operator).is_some()
            || crate::numeric_float_conversion_contract(program, operator).is_some())
    {
        return false;
    }
    crate::proof_contracts::float_projection_bindings::semantic_operations::
        is_sealed_toolchain_float_operator(program, operator)
}

/// The selected requirement owns type substitution; permission and value/domain
/// validation remain common to ordinary and bounded calls.
/// `argument_scope` retains the resolved callee's diagnostic qualification;
/// it does not change admission or substitute for the selected declaration.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_arguments(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    target_name: &str,
    argument_scope: &str,
    parameters: &[StateParameter],
    prior_statements: &[typed_trees::statement::StatementNode],
    retains_float_result_policy: bool,
    argument_environments: &[ValueEnvironment],
    type_matches: impl Fn(ExpressionHandle, TypeReferenceHandle) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if report_argument_count_mismatch(target_name, parameters, arguments, diagnostics) {
        return;
    }

    argument_bounds::report_referent_handbacks(
        program,
        current_machine,
        current_state,
        prior_statements,
        arguments,
        parameters,
        target_name,
        value_environment,
        diagnostics,
    );

    for (argument_index, (argument, parameter)) in
        arguments.iter().zip(parameters.iter()).enumerate()
    {
        let value_environment = argument_environments
            .get(argument_index)
            .unwrap_or(value_environment);
        validate_literal_argument_destination(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            diagnostics,
        );
        if report_argument_reference_access(
            program,
            current_machine,
            current_state,
            prior_statements,
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
        let slot_context = format!("argument `{}` for state `{argument_scope}`", parameter.name);

        if !type_matches(*argument, parameter.type_reference) {
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "ARGUMENT_MISMATCH caller={} scope={} target={} formal={} actual={}",
                    current_machine.name,
                    argument_scope,
                    target_name,
                    expected_type,
                    expression_type_name_handle(program, *argument)
                );
            }
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
                retains_float_result_policy,
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
            retains_float_result_policy,
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
}

/// Reject a single ARGUMENT whose scalar class conflicts with its `parameter`'s
/// primitive type -- a `bool`/text value passed where a numeric parameter is
/// expected (or vice versa), which the backend would otherwise read as garbage.
/// Used by the common argument admission path (`validate_call_arguments`). Returns
/// `true` if it reported. A non-primitive parameter (a data reference, a struct)
/// or an unresolvable argument class yields `false` -- no report.
fn report_cross_class_argument(
    program: &dyn crate::CheckingInput,
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

/// Prepare a value call's substitution and execution-dependent preconditions,
/// then admit its arguments through the same judgment as statements and transfers.
#[allow(clippy::too_many_arguments)]
fn validate_value_call_arguments(
    program: &dyn crate::CheckingInput,
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

    // Reject incomplete bindings before type substitution or precondition
    // instantiation can zip arguments against the declared parameters.
    let parameters = program.state_parameters(callee_state);
    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some()
        && program
            .typed_program()
            .callable_body_has_open_parameters(callee_machine)
        && !program
            .typed_program()
            .callable_body_has_open_parameters(current_machine)
    {
        eprintln!(
            "OPEN_CALLEE caller={} callee={} entry={} static={:?} retained={:?}",
            current_machine.name,
            callee_machine.name,
            callee_state.name,
            static_arguments,
            program
                .typed_program()
                .executable_application_template(callee_machine.symbol)
        );
    }
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
    validate_call_arguments(
        program,
        current_machine,
        Some(current_state),
        value_environment,
        arguments,
        callee_state.name.as_str(),
        &format!("{}::{}", callee_machine.name, callee_state.name),
        parameters,
        prior_statements,
        selected_float_operation_retains_argument_policy(program, callee_machine.symbol),
        &[],
        |argument, required| {
            argument_matches_carrier(
                program,
                current_machine.symbol,
                argument,
                required,
                &type_bindings,
            )
        },
        diagnostics,
    );
}

mod topology;
pub(crate) use topology::target_family_members;
pub use topology::{
    call_target_parameters, call_target_return_type, call_target_state, call_target_symbol,
    call_target_type_parameters, find_machine, find_machine_by_entry_state, find_machine_head,
    find_state, find_state_for_exit, find_state_in_machine, find_state_on_machine,
    machine_state_by_symbols, machine_state_count, terminal_callable_target,
};
mod receiver_topology;
pub(crate) use receiver_topology::placed_accessor_target;
pub use receiver_topology::{
    call_receiver_parts, machine_parameter_signature_symbol, receiver_can_dispatch_to_machine,
    resolve_name_path_member_symbol, resolve_state_call_target,
    statement_call_can_dispatch_to_machine, statement_call_receiver_members,
    statement_call_receiver_path, trait_machine_signature_symbol,
};
mod occurrences;
pub use occurrences::{
    CallSite, call_is_unconditional, call_site_argument_expressions, call_site_evidence_arguments,
    collect_call_sites_in_statement, find_call_site, find_call_site_in_state,
    transition_call_target,
};
