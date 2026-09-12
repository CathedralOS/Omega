//! COMPTIME STAGE 1 -- const evaluation of FIXED-ARRAY LENGTHS.
//!
//! `[T; table_size()]` puts a build-time-admissible, zero-argument machine call
//! in a constant position: the position makes it comptime, the contract system
//! makes it legal (no keyword, no macro -- chapter 13's frozen direction).
//! Independent calls evaluate before checking. Calls requiring selected provider
//! execution may remain pending through preliminary package checking, then fold
//! before final publication under the actual selected plans. Their private
//! receipts retain receiving ownership and exact invocation for independent
//! replay; the resulting type carries an ordinary literal length.
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

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle};

use crate::BuildTimeAdmissionPlan;
mod receivers;

/// Evaluate every `FixedArrayLength::ConstCall` in the program and substitute
/// the concrete `Literal` length in place. Errors name the array-length
/// position (the spelled type) and the failing machine.
pub fn evaluate_const_array_lengths(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    evaluate_const_array_lengths_with_authority(typed, None)
}

pub fn evaluate_const_array_lengths_with_authority(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    let pending: Vec<(TypeReferenceHandle, String, source::SourceSpan)> = typed
        .type_reference_table
        .fixed_array_lengths()
        .filter_map(|(handle, length)| match length {
            FixedArrayLength::ConstCall { name, source_span } => {
                Some((handle, name.as_str().to_owned(), *source_span))
            }
            _ => None,
        })
        .collect();

    if pending.is_empty() {
        return Ok(());
    }

    // Admission and execution must see the same closed static applications.
    // Preparing one private graph preserves the caller's generic templates and
    // source-owned type handles; only evaluated lengths are published below.
    let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
    let execution = prepared.typed();
    let admission =
        BuildTimeAdmissionPlan::infer_with_selection_authority(execution, selection_authority);

    let mut diagnostics = Vec::new();
    let mut substitutions: Vec<(TypeReferenceHandle, usize)> = Vec::new();

    for (handle, machine_name, source_span) in &pending {
        match evaluate_one(execution, &admission, machine_name, *source_span) {
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

/// Result custody for one provider-dependent array-length substitution.
/// The source invocation survives folding, so the receiver can execute it
/// again under the same selected semantics instead of trusting the literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldedArrayLength {
    type_reference: TypeReferenceHandle,
    machine: symbols::SymbolHandle,
    source: source::SourceSpan,
    value: usize,
    receivers: Vec<receivers::Receiver>,
}

/// Evaluate independent calls before deferring only the remaining work.
/// A pending provider-dependent field must not suppress an ordinary length
/// needed by the already-admitted Build invocation.
pub(crate) fn evaluate_independent_lengths(
    typed: &mut TypedTrees,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<bool, Vec<Diagnostic>> {
    let pending: Vec<_> = typed
        .type_reference_table
        .fixed_array_lengths()
        .filter_map(|(handle, length)| {
            let FixedArrayLength::ConstCall { name, source_span } = length else {
                return None;
            };
            Some((handle, name.as_str().to_owned(), *source_span))
        })
        .collect();
    if pending.is_empty() {
        return Ok(false);
    }
    let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
    let execution = prepared.typed();
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(execution);
    let admission = BuildTimeAdmissionPlan::infer_with_selection_authority(execution, authority);
    let mut deferred = false;
    let mut evaluated = Vec::new();
    let mut diagnostics = Vec::new();
    for (handle, name, source) in pending {
        if let Some(root) = execution
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            && admission.closure_needs_operator_selection(execution, root.symbol, &facts)
        {
            deferred = true;
        } else {
            match evaluate_one(execution, &admission, &name, source) {
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

pub(crate) fn evaluate_with_selected_operators(
    typed: &mut TypedTrees,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
) -> Result<Vec<FoldedArrayLength>, Vec<Diagnostic>> {
    let pending: Vec<_> = typed
        .type_reference_table
        .fixed_array_lengths()
        .filter_map(|(handle, length)| {
            let FixedArrayLength::ConstCall { name, source_span } = length else {
                return None;
            };
            Some((handle, name.as_str().to_owned(), *source_span))
        })
        .collect();
    let admission = BuildTimeAdmissionPlan::infer_with_selection_authority(typed, authority)
        .with_selected_operators(typed, operators)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let roots = receivers::roots(typed);
    let mut folded = Vec::new();
    for (type_reference, name, source) in pending {
        let machines: Vec<_> = typed
            .machines()
            .iter()
            .filter(|machine| machine.name.as_str() == name)
            .collect();
        let [machine] = machines.as_slice() else {
            return Err(vec![Diagnostic::error(
                "folded length has no unique source invocation",
            )]);
        };
        let value = evaluate_exact_invocation(typed, &admission, machine, source)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let receivers = receivers::capture(typed, &roots, type_reference);
        receivers::require_unique(&receivers).map_err(|reason| vec![Diagnostic::error(reason)])?;
        folded.push(FoldedArrayLength {
            type_reference,
            machine: machine.symbol,
            source,
            value,
            receivers,
        });
    }
    for fold in &folded {
        typed
            .type_reference_table
            .set_fixed_array_length(fold.type_reference, fold.value);
    }
    Ok(folded)
}

/// Replay the retained invocation using current selected semantics and compare
/// both the current receiving type and recorded result to the actual value.
pub fn validate_folded_array_lengths(
    typed: &TypedTrees,
    folds: &[FoldedArrayLength],
    operators: &[crate::SelectedBuildTimeBinaryOperator],
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    let admission = BuildTimeAdmissionPlan::infer_with_selection_authority(typed, authority)
        .with_selected_operators(typed, operators)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let roots = receivers::roots(typed);
    for (index, fold) in folds.iter().enumerate() {
        if folds[..index]
            .iter()
            .any(|prior| prior.type_reference == fold.type_reference)
        {
            return Err(vec![Diagnostic::error(
                "duplicate folded array-length custody",
            )]);
        }
        receivers::validate(typed, &roots, fold.type_reference, &fold.receivers)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let machines: Vec<_> = typed
            .machines()
            .iter()
            .filter(|machine| machine.symbol == fold.machine)
            .collect();
        let [machine] = machines.as_slice() else {
            return Err(vec![Diagnostic::error(
                "folded array invocation disappeared",
            )]);
        };
        let actual = evaluate_exact_invocation(typed, &admission, machine, fold.source)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if actual != fold.value
            || !typed
                .type_reference_table
                .fixed_array_lengths()
                .any(|(handle, length)| {
                    handle == fold.type_reference && *length == FixedArrayLength::Literal(actual)
                })
        {
            return Err(vec![Diagnostic::error(
                "folded array length differs from selected source evaluation",
            )]);
        }
    }
    Ok(())
}

fn evaluate_exact_invocation(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine: &Machine,
    source: source::SourceSpan,
) -> Result<usize, String> {
    if entry_state_parameter_count(typed, machine) != 0 {
        return Err("array length requires a zero-argument machine".into());
    }
    let value = admission.evaluate_const_evaluable_machine_symbol_for_invocation(
        typed,
        machine.symbol,
        Vec::new(),
        crate::BuildTimeInvocationCustody::Source(source),
    )?;
    let crate::BuildTimeValue::Int(value) = value else {
        return Err("array length machine must return an integer".into());
    };
    usize::try_from(value)
        .map_err(|_| "array length must be nonnegative and fit the compiler range".into())
}

fn evaluate_one(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine_name: &str,
    source_span: source::SourceSpan,
) -> Result<usize, String> {
    let value = evaluate_zero_argument_machine_for_invocation(
        typed,
        admission,
        machine_name,
        "array length",
        crate::BuildTimeInvocationCustody::Source(source_span),
    )?;
    if value < 0 {
        return Err(format!(
            "the call returned {value}, but an array length must be a non-negative integer"
        ));
    }
    usize::try_from(value)
        .map_err(|_| format!("the call returned {value}, which does not fit an array length"))
}

pub fn evaluate_zero_argument_machine(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine_name: &str,
    position: &str,
) -> Result<i64, String> {
    evaluate_zero_argument_machine_with_optional_custody(
        typed,
        admission,
        machine_name,
        position,
        None,
    )
}

pub fn evaluate_zero_argument_machine_for_invocation(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine_name: &str,
    position: &str,
    custody: crate::BuildTimeInvocationCustody,
) -> Result<i64, String> {
    evaluate_zero_argument_machine_with_optional_custody(
        typed,
        admission,
        machine_name,
        position,
        Some(custody),
    )
}

fn evaluate_zero_argument_machine_with_optional_custody(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine_name: &str,
    position: &str,
    custody: Option<crate::BuildTimeInvocationCustody>,
) -> Result<i64, String> {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
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
    let crate::BuildTimeValue::Int(value) = value else {
        let entry = entry_state(typed, machine);
        return match entry.and_then(|state| typed.primitive_type_reference(state.return_type)) {
            Some(primitive) => Err(format!(
                "machine `{machine_name}` returns `{}`, not an integer type",
                primitive.name()
            )),
            None => Err(format!(
                "machine `{machine_name}` does not declare an integer return type"
            )),
        };
    };
    Ok(value)
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

#[cfg(test)]
mod tests;
