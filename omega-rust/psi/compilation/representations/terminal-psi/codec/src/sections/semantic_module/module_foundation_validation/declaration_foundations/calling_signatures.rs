//! Replay native-telescope relationships without reconstructing source types.

use crate::codec_error::{CodecError, malformed};
use terminal_psi::TerminalModule;

pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    super::super::super::wire::require_increasing(
        module
            .calling_signatures
            .iter()
            .map(|signature| signature.boundary),
        "calling signatures by boundary declaration",
    )?;
    for signature in &module.calling_signatures {
        let boundary = module.boundary_machine(signature.boundary).ok_or(
            CodecError::MalformedStructuralFoundation("calling signature lost its exact boundary"),
        )?;
        if boundary.declaration.is_none()
            || module
                .identities
                .text(signature.requirement_identity)
                .is_none_or(str::is_empty)
        {
            return malformed(
                "calling signature lacks its checked declaration or requirement identity",
            );
        }
        let telescope = match &signature.telescope {
            Ok(telescope) => telescope,
            Err(reason) if !reason.is_empty() => continue,
            Err(_) => return malformed("calling signature has an empty deferred refusal"),
        };
        if telescope.parameters.len() != boundary.parameter_order.len() {
            return malformed("calling telescope differs from its exact boundary parameter roster");
        }
        let mut positions =
            Vec::with_capacity(telescope.parameters.len() + telescope.direct_callbacks.len());
        for formal in &telescope.parameters {
            positions.push((formal.native_ordinal, formal.identity));
        }
        if telescope
            .parameters
            .windows(2)
            .any(|pair| pair[0].native_ordinal >= pair[1].native_ordinal)
            || telescope
                .binders
                .windows(2)
                .any(|pair| pair[0].static_machine_ordinal >= pair[1].static_machine_ordinal)
        {
            return malformed("calling telescope lost its authored formal or binder order");
        }
        for (index, binder) in telescope.binders.iter().enumerate() {
            if telescope.binders[..index]
                .iter()
                .any(|earlier| earlier.binder == binder.binder)
            {
                return malformed("calling telescope repeats a nominal callback binder");
            }
            let identity = module
                .identities
                .text(binder.requirement_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "callback binder lost its requirement identity",
                ))?;
            if semantic_vocabulary::callback_requirement_id(identity) != binder.requirement {
                return malformed("callback binder substitutes another requirement identity");
            }
        }
        for callback in &telescope.direct_callbacks {
            if !telescope.binders.iter().any(|binder| {
                binder.binder == callback.binder && binder.requirement == callback.requirement
            }) {
                return malformed("direct callback lost its exact nominal binder requirement");
            }
            positions.push((callback.native_ordinal, callback.identity));
        }
        positions.sort_by_key(|(position, _)| *position);
        for (index, (position, identity)) in positions.iter().enumerate() {
            if usize::try_from(*position).ok() != Some(index)
                || positions[..index]
                    .iter()
                    .any(|(_, earlier)| earlier == identity)
            {
                return malformed(
                    "calling telescope repeats or omits a native formal identity or position",
                );
            }
        }
    }
    Ok(())
}
