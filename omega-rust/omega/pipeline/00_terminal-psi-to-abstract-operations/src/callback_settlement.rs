//! Admit callback interfaces and rejoin registrar imports to their selected calling applications.

mod registrar_imports;

pub use registrar_imports::{CallbackRegistrarOccurrence, settle_callback_registrar_imports};

use backend_plan::BoundNominalCallbackPlacement;
use provider_planning::calling_policy_plans::{
    BoundaryCallingPlanRealization, MaterializedBoundarySignature,
};
use terminal_codec::CanonicalTerminalArtifact;

/// Requirement, ABI, and call-custody validation for one produced callback
/// thunk artifact. This is deliberately not a body recognizer: any checked
/// callback body of any inbound signature is admissible while this custody
/// holds. This admits the native interface beside an already-produced Terminal
/// artifact; it neither produces Psi nor replaces proof or native publication
/// admission. Failure returns the callback-specific reason for the caller's
/// diagnostic channel.
///
/// - Requirement: the placement rejoins exactly one target calling-plan
///   realization for its satisfaction row, whose owner is the placement's
///   canonical requirement overload.
/// - ABI: the realization replays to a validated entry plan that must equal
///   the retained `placement.boundary_entry_plan` with unaltered application
///   identity, and the produced entry machine's declared interface must
///   correspond to the requirement's materialized parameter/result roster —
///   scalar slots by exact ABI shape, reference and aggregate slots by
///   structural parameter presence (their physical shape is the plan's own
///   target-selected evidence).
/// - Call custody: the compiler-private channel carries no import custody, so
///   the thunk module may declare no boundary machines.
pub fn validate_callback_thunk_settlement(
    artifact: &CanonicalTerminalArtifact,
    placement: &BoundNominalCallbackPlacement,
    calling_plan_realizations: &[BoundaryCallingPlanRealization],
) -> Result<(), String> {
    backend_plan::validate_bound_nominal_callback_placement(placement).map_err(|error| {
        format!("callback thunk placement failed boundary entry plan replay: {error}")
    })?;
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        format!("callback thunk custody replay could not decode canonical semantics: {error}")
    })?;
    let Some(machine) = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
    else {
        return Err("callback thunk module does not contain its entry machine".to_owned());
    };

    // Requirement join: exactly one target calling-plan realization owns this
    // satisfaction row, and the retained plan must be its exact evaluated
    // inbound plan — replayed, not fingerprint-trusted.
    let matching = calling_plan_realizations
        .iter()
        .filter(|realization| {
            realization.boundary_trait == placement.satisfaction_trait
                && realization.requirement_machine == placement.satisfaction_requirement
                && realization.boundary_arguments.is_empty()
        })
        .collect::<Vec<_>>();
    let [realization] = matching.as_slice() else {
        return Err(format!(
            "callback thunk requirement `{}` resolves to {} calling-plan realizations; exactly one is required",
            placement.canonical_requirement_overload,
            matching.len(),
        ));
    };
    let materialized = realization.materialized_signature();
    if materialized.owner_requirement_identity() != placement.canonical_requirement_overload {
        return Err(format!(
            "callback thunk requirement `{}` rejoins a realization owned by `{}`",
            placement.canonical_requirement_overload,
            materialized.owner_requirement_identity(),
        ));
    }
    let (validated, report_fingerprint, commitment) = realization
        .replayed_validated_application()
        .map_err(|error| {
            format!(
                "callback thunk inbound entry plan does not replay against its requirement: {error}",
            )
        })?;
    if validated.plan() != &placement.boundary_entry_plan
        || realization.exact_boundary_entry_plan() != &placement.boundary_entry_plan
        || realization.report_fingerprint != report_fingerprint
        || realization.commitment != commitment
        || placement.boundary_calling_plan_report_fingerprint
            != validated.contract_report_fingerprint()
    {
        return Err("callback thunk inbound entry plan drifted from its requirement's evaluated application".to_owned());
    }

    bind_thunk_machine_interface(machine, materialized)?;

    // Call custody: the compiler-private channel carries no import custody.
    // A thunk whose closure calls a boundary machine can never settle those
    // imports, so the module may declare none.
    if !module.boundary_machines.is_empty() {
        return Err("callback thunk module carries boundary import custody the private channel cannot settle".to_owned());
    }
    Ok(())
}

/// Bind one produced thunk entry machine's declared interface to its
/// requirement's materialized signature. The authored parameter roots order
/// the walk: a scalar-classed root consumes one scalar machine parameter whose
/// ABI shape must re-derive exactly; a reference or aggregate root consumes
/// one structural machine parameter — its physical shape is the plan's own
/// target-selected evidence, already replayed above. Either roster left over
/// means the produced machine drifted from its requirement signature.
fn bind_thunk_machine_interface(
    machine: &terminal_psi::TerminalMachine,
    materialized: &MaterializedBoundarySignature,
) -> Result<(), String> {
    use provider_planning::calling_policy_plans::BoundaryValueClass as Class;
    let mut scalar_parameters = machine.parameters.iter();
    let mut structural_parameters = machine.structural_parameters.iter();
    for (ordinal, root) in materialized.parameters().iter().copied().enumerate() {
        let Some(shape) = materialized.shapes().get(usize::from(root)) else {
            return Err(format!(
                "callback thunk requirement parameter {ordinal} names an absent shape root",
            ));
        };
        match shape.class() {
            Class::Integer | Class::Float => {
                let Some(parameter) = scalar_parameters.next() else {
                    return Err(format!(
                        "callback thunk machine declares too few scalar parameters for requirement parameter {ordinal}",
                    ));
                };
                let expected = calling_conventions::ValueShape {
                    class: match shape.class() {
                        Class::Float => calling_conventions::ValueClass::Float,
                        _ => calling_conventions::ValueClass::Integer,
                    },
                    byte_size: shape.byte_size(),
                    alignment: shape.alignment(),
                };
                if scalar_value_shape(parameter.scalar_type) != Some(expected) {
                    return Err(format!(
                        "callback thunk scalar parameter {ordinal} does not carry its requirement's ABI shape",
                    ));
                }
            }
            Class::Reference | Class::FixedArray { .. } | Class::Record { .. } => {
                if structural_parameters.next().is_none() {
                    return Err(format!(
                        "callback thunk machine declares too few structural parameters for requirement parameter {ordinal}",
                    ));
                }
            }
        }
    }
    if scalar_parameters.next().is_some() || structural_parameters.next().is_some() {
        return Err(
            "callback thunk machine declares parameters beyond its requirement signature"
                .to_owned(),
        );
    }
    match materialized.result() {
        Some(root) => {
            let Some(shape) = materialized.shapes().get(usize::from(root)) else {
                return Err(
                    "callback thunk requirement result names an absent shape root".to_owned(),
                );
            };
            match shape.class() {
                Class::Integer | Class::Float => {
                    let terminal_psi::TerminalMachineResult::Scalar(result) = &machine.result
                    else {
                        return Err("callback thunk requirement result is scalar but the machine declares none".to_owned());
                    };
                    let expected = calling_conventions::ValueShape {
                        class: match shape.class() {
                            Class::Float => calling_conventions::ValueClass::Float,
                            _ => calling_conventions::ValueClass::Integer,
                        },
                        byte_size: shape.byte_size(),
                        alignment: shape.alignment(),
                    };
                    if scalar_value_shape(result.scalar_type) != Some(expected) {
                        return Err("callback thunk scalar result does not carry its requirement's ABI shape".to_owned());
                    }
                }
                Class::Reference | Class::FixedArray { .. } | Class::Record { .. } => {
                    if !matches!(
                        machine.result,
                        terminal_psi::TerminalMachineResult::Structural(_)
                    ) {
                        return Err("callback thunk requirement result is structural but the machine declares none".to_owned());
                    }
                }
            }
        }
        None => {
            if !matches!(machine.result, terminal_psi::TerminalMachineResult::Unit) {
                return Err(
                    "callback thunk requirement has no result but the machine declares one"
                        .to_owned(),
                );
            }
        }
    }
    Ok(())
}

/// The ABI shape one scalar type occupies — fixed integers, Booleans, and
/// IEEE floats only; anything wider has no scalar-ABI custody.
fn scalar_value_shape(
    scalar_type: semantic_vocabulary::ScalarType,
) -> Option<calling_conventions::ValueShape> {
    use calling_conventions::ValueShape;
    match scalar_type {
        semantic_vocabulary::ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        semantic_vocabulary::ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary32,
        ) => Some(ValueShape::float(4)),
        semantic_vocabulary::ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        ) => Some(ValueShape::float(8)),
        semantic_vocabulary::ScalarType::Integer(integer) => {
            let bits = integer.bits();
            if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(bits, 8 | 16 | 32 | 64)
            {
                return None;
            }
            let bytes = bits / 8;
            Some(ValueShape::integer(bytes, bytes))
        }
    }
}

#[cfg(test)]
mod tests;
