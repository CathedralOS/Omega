//! COMPTIME STAGE 1 -- const evaluation of FIXED-ARRAY LENGTHS.
//!
//! `[T; table_size()]` puts a build-time-admissible, zero-argument machine call
//! in a constant position: the position makes it comptime, the contract system
//! makes it legal (no keyword, no macro -- chapter 13's frozen direction).
//! Psi runs this pass between typed-tree lowering and checking:
//!
//! - EARLY enough that range checking (`typed-trees-to-checked-trees`), proof
//!   facts, layout, and codegen all see an ordinary `FixedArrayLength::Literal`
//!   -- indistinguishable from a written `[T; 16]`.
//! - LATE enough that the whole program is typed, so the Psi checked-tree
//!   interpreter can evaluate the callee over the very trees the rest of the
//!   pipeline consumes. The interpreter is target-neutral and has no Omega
//!   dependency.
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

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle};

use crate::BuildTimeAdmissionPlan;

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
    let pending: Vec<(TypeReferenceHandle, String, psi_source::SourceSpan)> = typed
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

    let admission =
        BuildTimeAdmissionPlan::infer_with_selection_authority(typed, selection_authority);

    let mut diagnostics = Vec::new();
    let mut substitutions: Vec<(TypeReferenceHandle, usize)> = Vec::new();

    for (handle, machine_name, source_span) in &pending {
        match evaluate_one(typed, &admission, machine_name, *source_span) {
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

fn evaluate_one(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine_name: &str,
    source_span: psi_source::SourceSpan,
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

    match custody {
        Some(custody) => admission.require_common_floor_for_invocation(typed, machine, custody)?,
        None => admission.require_common_floor(typed, machine)?,
    }

    psi_checked_interpreter::evaluate_const_machine(typed, machine_name)
}

/// The parameter count of the machine's entry state (the body of a free
/// machine; mirrors the interpreter's entry-state selection: the state named
/// like the machine's leaf, else the first state).
fn entry_state_parameter_count(typed: &TypedTrees, machine: &Machine) -> usize {
    let leaf = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let states = typed.machine_states(machine);
    let entry = states
        .iter()
        .find(|state| state.name.as_str() == leaf)
        .or_else(|| states.first());
    entry
        .map(|state| typed.state_parameters(state).len())
        .unwrap_or(0)
}
