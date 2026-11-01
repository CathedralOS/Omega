//! Replay actual callback placement after dropping source, through both the
//! retained and decoded artifacts. Perturb ABI proposals, not source programs.

use calling_conventions::{BoundaryNativeParameterOrigin, BoundaryNativeParameterShape};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi_to_abstract_operations::{
    RetainedTerminalArtifact, settle_native_callback_placements,
};

pub(super) fn inspect(retained: &RetainedTerminalArtifact) -> Result<(), String> {
    let proposal = retained
        .native_realization_proposal()
        .ok_or("callback telescope controls need a native proposal")?;
    let calling = &proposal.boundary_applications().calling;
    let selected = &proposal.providers().selection;
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("callback telescope transport: {error:?}"))?;
    let baseline = settle_native_callback_placements(
        retained.artifact().semantic_module(),
        selected,
        calling,
    )?;
    if baseline.is_empty() {
        return Err("callback telescope controls need a reached foreign callback".into());
    }
    let registrar = calling
        .iter()
        .position(|application| {
            application
                .signature
                .native_parameters
                .span(application.signature.native_parameter_rows)
                .is_some_and(|rows| {
                    rows.iter().any(|row| {
                        matches!(
                            row.origin,
                            BoundaryNativeParameterOrigin::PrivateCallback { .. }
                        )
                    })
                })
        })
        .ok_or("callback telescope controls need a direct callback argument")?;

    for artifact in [retained.artifact(), &loaded] {
        let settle = |calling: &[_]| {
            settle_native_callback_placements(artifact.semantic_module(), selected, calling)
        };
        if settle(calling)? != baseline {
            return Err("decoded Terminal callback placement changed".into());
        }
        let mut changed = calling.clone();
        let inbound = changed
            .iter_mut()
            .find(|application| {
                application.requirement_identity == baseline[0].canonical_requirement_overload
            })
            .ok_or("callback reference control lost its inbound application")?;
        let shape = inbound
            .signature
            .shapes
            .iter_mut()
            .find(|shape| shape.class == calling_conventions::BoundaryValueClass::Reference)
            .ok_or("callback reference control has no borrowed parameter")?;
        shape.byte_size = 16;
        require_refusal(
            settle(&changed),
            "reference geometry differs from the Terminal transport role",
        )?;
        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let binders = signature
            .callback_bindings
            .span_mut(signature.callback_binding_rows)
            .ok_or("callback control lost its binder rows")?;
        let binder = binders
            .first_mut()
            .ok_or("callback control needs a registrar binder")?;
        binder.invokes_synchronously = !binder.invokes_synchronously;
        require_refusal(
            settle(&changed),
            "callback binders differ from the Terminal telescope",
        )?;

        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let binders = signature
            .callback_bindings
            .span_mut(signature.callback_binding_rows)
            .ok_or("callback control lost its binder ordinals")?;
        let binder = binders
            .first_mut()
            .ok_or("callback control needs a registrar binder")?;
        binder.static_machine_ordinal += 1;
        require_refusal(
            settle(&changed),
            "callback binders differ from the Terminal telescope",
        )?;

        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let binders = signature
            .callback_bindings
            .span_mut(signature.callback_binding_rows)
            .ok_or("callback control lost its requirement identities")?;
        let binder = binders
            .first_mut()
            .ok_or("callback control needs a registrar binder")?;
        binder.requirement_identity.push_str("::substituted");
        require_refusal(
            settle(&changed),
            "callback binders differ from the Terminal telescope",
        )?;

        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let rows = signature
            .native_parameters
            .span_mut(signature.native_parameter_rows)
            .ok_or("callback control lost its native rows")?;
        let pointer = rows
            .iter_mut()
            .find(|row| {
                matches!(
                    row.origin,
                    BoundaryNativeParameterOrigin::PrivateCallback { .. }
                )
            })
            .ok_or("callback control lost its direct callback argument")?;
        pointer.shape = BoundaryNativeParameterShape::TargetFunctionPointer {
            byte_size: 4,
            alignment: 4,
        };
        require_refusal(
            settle(&changed),
            "native parameters differ from the Terminal telescope",
        )?;

        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let rows = signature
            .native_parameters
            .span_mut(signature.native_parameter_rows)
            .ok_or("callback control lost its native order")?;
        if rows.len() < 2 {
            return Err("callback telescope controls need mixed native arguments".into());
        }
        let first = rows[0].native_ordinal;
        rows[0].native_ordinal = rows[1].native_ordinal;
        rows[1].native_ordinal = first;
        require_refusal(
            settle(&changed),
            "native parameters differ from the Terminal telescope",
        )?;
    }
    Ok(())
}

fn require_refusal<T>(result: Result<T, String>, expected: &str) -> Result<(), String> {
    match result {
        Err(reason) if reason.contains(expected) => Ok(()),
        Err(reason) => Err(format!(
            "callback telescope control expected {expected}: {reason}"
        )),
        Ok(_) => Err(format!(
            "callback telescope control accepted; expected {expected}"
        )),
    }
}
