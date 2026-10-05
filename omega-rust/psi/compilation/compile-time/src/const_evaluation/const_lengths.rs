//! COMPTIME STAGE 1 -- const evaluation of FIXED-ARRAY LENGTHS.
//!
//! `[T; table_size()]` puts a build-time-admissible, zero-argument machine call
//! in a constant position: the position makes it comptime, the contract system
//! makes it legal (no keyword, no macro -- chapter 13's frozen direction).
//! Independent calls evaluate before checking. Calls requiring selected provider
//! execution may remain pending through preliminary package checking, then fold
//! before final publication under the actual selected plans; the fold lands
//! an ordinary literal length in the type, which final checking consumes. The
//! compile does not replay the fold against its own result.
//!
//! LEGALITY GATE: the callee's normalized effective service reach must be empty
//! and its modular operational summary must neither suspend nor block. The
//! callee must also take no parameters at all (stage 1 is zero-arg, which
//! discharges the "no `&mut`/out params" half of the predicate). The remaining
//! build-time contract axes are staged in the shared admission plan.
//!
//! TERMINATION: no new rule -- the language's existing discipline (no general
//! recursion, loops carry decreases) covers const callees. The interpreter
//! entry adds a ~100k-step fuel cap purely as defense-in-depth; exceeding it
//! is a compile error here.
//!
//! DETERMINISM: the interpreter width-adjusts the terminal value to the
//! machine's declared integer return type (the same wrap-on-write it applies
//! differentially), so the result carries TARGET integer semantics, never
//! host widths.

use crate::build_time_evaluation::EvaluationDomain;
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

use crate::BuildTimeAdmissionPlan;

/// Detached type-arena nodes retain their identity but do not own evaluation
/// work. Snapshot only lengths reached from the current declaration roster.
fn pending_array_lengths(
    typed: &TypedTrees,
    domain: &EvaluationDomain,
) -> Vec<(
    TypeReferenceHandle,
    symbols::SymbolHandle,
    String,
    source::SourceSpan,
)> {
    domain
        .references(typed)
        .type_references
        .into_iter()
        .filter_map(
            |handle| match typed.type_reference_table.type_reference(handle) {
                TypeReferenceNode::FixedArray {
                    length:
                        FixedArrayLength::ConstCall {
                            machine,
                            name,
                            source_span,
                        },
                    ..
                } => Some((handle, *machine, name.as_str().to_owned(), *source_span)),
                _ => None,
            },
        )
        .collect()
}

pub fn evaluate_const_array_lengths(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    evaluate_const_array_lengths_in(typed, selection_authority, &EvaluationDomain::all())
}

pub(crate) fn evaluate_const_array_lengths_in(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let pending = pending_array_lengths(typed, domain);

    if pending.is_empty() {
        return Ok(());
    }

    // Admission and execution must see the same closed static applications.
    // Preparing one private graph preserves the caller's generic templates and
    // source-owned type handles; only evaluated lengths are published below.
    let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, domain)?;
    let execution = prepared.typed();
    let admission = BuildTimeAdmissionPlan::infer(execution, selection_authority);

    let mut diagnostics = Vec::new();
    let mut substitutions: Vec<(TypeReferenceHandle, usize)> = Vec::new();

    for (handle, machine, machine_name, source_span) in &pending {
        match evaluate_one(execution, &admission, *machine, *source_span) {
            Ok(value) => substitutions.push((*handle, value)),
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "fixed-array length `{}`: const evaluation of `{machine_name}` failed: {reason}",
                    typed.type_reference_table.display_name(*handle)
                )));
            }
        }
    }

    for (handle, value) in substitutions {
        typed
            .type_reference_table
            .set_fixed_array_length(handle, value);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Evaluate independent calls before deferring only the remaining work.
/// A pending provider-dependent field must not suppress an ordinary length
/// needed by the already-admitted Build invocation.
pub(crate) fn evaluate_independent_lengths(
    typed: &mut TypedTrees,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &EvaluationDomain,
) -> Result<bool, Vec<Diagnostic>> {
    let pending = pending_array_lengths(typed, domain);
    if pending.is_empty() {
        return Ok(false);
    }
    let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, domain)?;
    let execution = prepared.typed();
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(execution);
    let admission = BuildTimeAdmissionPlan::infer(execution, authority);
    let mut deferred = false;
    let mut evaluated = Vec::new();
    let mut diagnostics = Vec::new();
    for (handle, machine, name, source) in pending {
        if let Some(root) = execution.machine_by_symbol(machine)
            && admission.closure_needs_operator_selection(execution, root.symbol, &facts)
        {
            deferred = true;
        } else {
            match evaluate_one(execution, &admission, machine, source) {
                Ok(value) => evaluated.push((handle, value)),
                Err(reason) => diagnostics.push(Diagnostic::error(format!(
                    "fixed-array length `{}`: const evaluation of `{name}` failed: {reason}",
                    typed.type_reference_table.display_name(handle)
                ))),
            }
        }
    }
    for (handle, value) in evaluated {
        typed
            .type_reference_table
            .set_fixed_array_length(handle, value);
    }
    if diagnostics.is_empty() {
        Ok(deferred)
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn evaluate_selected_array_lengths(
    typed: &mut TypedTrees,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    selected: crate::SelectedBuildTimeOperators<'_>,
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let crate::SelectedBuildTimeOperators {
        operators,
        provider_bodies,
    } = selected;
    let pending = pending_array_lengths(typed, domain);
    // Admission and execution must see the same selected realization. When a
    // selected use means an ordinary checked provider body, rebind its
    // occurrence to the exact provider entry state on one private copy: the
    // provider joins the ordinary call closure and runs its real body, while
    // the caller's tree and its source-owned handles stay untouched.
    let selected_program =
        crate::machine_execution::selected_operators::prepare_selected_execution(
            typed,
            provider_bodies,
            operators,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let execution_program = domain.execution_program(selected_program.as_ref().unwrap_or(typed));
    let execution = &execution_program;
    let admission = BuildTimeAdmissionPlan::infer(execution, authority)
        .with_selected_operators(execution, operators)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut folded = Vec::new();
    for (type_reference, selected, _, source) in pending {
        let machines: Vec<_> = execution
            .machines()
            .iter()
            .filter(|machine| selected.is_valid() && machine.symbol == selected)
            .collect();
        let [machine] = machines.as_slice() else {
            return Err(vec![Diagnostic::error(
                "folded length has no unique source invocation",
            )]);
        };
        let value = evaluate_exact_invocation(execution, &admission, machine, source)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        folded.push((type_reference, value));
    }
    for (type_reference, value) in folded {
        typed
            .type_reference_table
            .set_fixed_array_length(type_reference, value);
    }
    Ok(())
}

fn evaluate_exact_invocation(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine: &Machine,
    source: source::SourceSpan,
) -> Result<usize, String> {
    let parameter_count = entry_state_parameter_count(typed, machine);
    if parameter_count > 0 {
        let machine_name = machine.name.as_str();
        return Err(format!(
            "machine `{machine_name}` takes {parameter_count} parameter(s); a const-evaluated \
             array length must call a zero-argument machine (const arguments are not supported yet)"
        ));
    }
    let value = admission.evaluate_const_evaluable_machine_symbol_for_invocation(
        typed,
        machine.symbol,
        Vec::new(),
        crate::BuildTimeInvocationCustody::Source(source),
    )?;
    let value = decode_integer_result(typed, machine, value)?;
    if value.is_negative() {
        return Err(format!(
            "the call returned {value}, but an array length must be a non-negative integer"
        ));
    }
    value
        .to_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("the call returned {value}, which does not fit an array length"))
}

fn evaluate_one(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine: symbols::SymbolHandle,
    source_span: source::SourceSpan,
) -> Result<usize, String> {
    let machine = typed
        .machine_by_symbol(machine)
        .ok_or("array length lost its exact selected machine")?;
    evaluate_exact_invocation(typed, admission, machine, source_span)
}

pub fn evaluate_zero_argument_machine(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine_name: &str,
    position: &str,
    custody: Option<crate::BuildTimeInvocationCustody>,
) -> Result<BigInt, String> {
    let machine = typed
        .realized_machine_named(machine_name)
        .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;

    // Stage 1 scope: a zero-argument machine. (This also discharges the
    // "no `&mut`/out parameters" portion of the build-time contract.)
    let parameter_count = entry_state_parameter_count(typed, machine);
    if parameter_count > 0 {
        return Err(format!(
            "machine `{machine_name}` takes {parameter_count} parameter(s); a const-evaluated \
             {position} must call a zero-argument machine (const arguments are not supported yet)"
        ));
    }

    let value = match custody {
        Some(custody) => admission.evaluate_const_evaluable_machine_for_invocation(
            typed,
            machine_name,
            Vec::new(),
            custody,
        )?,
        None => admission.evaluate_const_evaluable_machine(typed, machine_name, Vec::new())?,
    };
    decode_integer_result(typed, machine, value)
}

/// Interpreter integers carry fixed-width bits in an i64 slot. Decode using
/// the admitted callee's return carrier before a const position narrows them:
/// u64::MAX is positive, while the same bits returned as i64 denote -1.
/// Array lengths, generic arguments and range endpoints share this conversion;
/// range normalization must never see a host-signed reinterpretation.
pub(crate) fn decode_integer_result(
    typed: &TypedTrees,
    machine: &Machine,
    value: crate::BuildTimeValue,
) -> Result<BigInt, String> {
    let machine_name = machine.name.as_str();
    let primitive = entry_state(typed, machine)
        .and_then(|state| typed.primitive_type_reference(state.return_type));
    if let (Some(primitive), crate::BuildTimeValue::Int(value)) = (primitive, &value)
        && primitive.accepts_integer_literal()
    {
        return Ok(if primitive.is_signed_integer() {
            BigInt::from_i64(*value)
        } else {
            BigInt::from_u64(*value as u64)
        });
    }
    match primitive {
        Some(primitive) => Err(format!(
            "machine `{machine_name}` returns `{}`, not an integer type",
            primitive.name()
        )),
        None => Err(format!(
            "machine `{machine_name}` does not declare an integer return type"
        )),
    }
}

/// The parameter count of the machine's entry state (the body of a free
/// machine; mirrors the interpreter's entry-state selection: the state named
/// like the machine's leaf, else the first state).
fn entry_state_parameter_count(typed: &TypedTrees, machine: &Machine) -> usize {
    entry_state(typed, machine)
        .map(|state| typed.state_parameters(state).len())
        .unwrap_or(0)
}

fn entry_state<'a>(
    typed: &'a TypedTrees,
    machine: &Machine,
) -> Option<&'a typed_trees::state::State> {
    let leaf = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let states = typed.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == leaf)
        .or_else(|| states.first())
}
