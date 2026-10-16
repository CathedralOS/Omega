//! Join native entry transport to the retained ordinary body, not a replacement
//! body signature. Scalar codecs come from the exact Terminal referent and the
//! runtime ABI's default scalar storage, never from pointer width or debug names.

use std::sync::Arc;

use crate::{FunctionFragmentObjectArtifactError as Error, ObjectArtifact, PrivateFunctionObject};
use calling_conventions::{CallPlan, ValueClass};
use function_identity::MachineFunctionIdentity;
use machine_emission::native_entry_bridge::{
    NativeEntryBridgeRequest, NativeEntryParameter, emit_native_entry_bridge,
};

/// Bind a caller-scoped private identity to its unchanged body and native entry.
/// The ordinal roster is in internal lane order, after the nominal formal join.
pub fn create_private_function_object(
    identity: MachineFunctionIdentity,
    body: Arc<ObjectArtifact>,
    inbound: &CallPlan,
    inbound_ordinals: &[u32],
) -> Result<PrivateFunctionObject, Error> {
    let request = native_entry_request(&body, inbound, inbound_ordinals)?;
    let entry_bridge = if identity_transport(&request) {
        None
    } else {
        Some(emit_native_entry_bridge(request).map_err(Error::NativeEntryBridge)?)
    };
    Ok(PrivateFunctionObject {
        identity,
        body,
        entry_bridge,
    })
}

/// Resolve the native transport against the unchanged ordinary body. Image
/// roots and callback entries share this join without sharing their identity
/// or installation authority.
pub(crate) fn native_entry_request(
    body: &ObjectArtifact,
    inbound: &CallPlan,
    inbound_ordinals: &[u32],
) -> Result<NativeEntryBridgeRequest, Error> {
    let replay = body
        .fragment_replay
        .as_ref()
        .ok_or(Error::Mismatch("private entry lacks body custody"))?;
    let targeted = replay
        .0
        .source()
        .source()
        .source()
        .source()
        .optimized_target()
        .target_operations();
    let internal = &targeted
        .functions
        .iter()
        .find(|function| function.machine == body.entry)
        .ok_or(Error::Mismatch("private body entry plan missing"))?
        .graph
        .call_plan;
    if inbound_ordinals.len() != internal.parameters.len()
        || inbound.parameters.len() != internal.parameters.len()
    {
        return Err(Error::Mismatch("private entry formal count differs"));
    }
    let mut used = Vec::with_capacity(inbound_ordinals.len());
    let mut parameters = arena::Arena::with_capacity(inbound_ordinals.len());
    let module = replay.terminal_module();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == body.entry)
        .ok_or(Error::Mismatch("private Terminal entry missing"))?;
    if entry.parameters.len() + entry.structural_parameters.len() != internal.parameters.len() {
        return Err(Error::Mismatch(
            "private body lanes differ from Terminal signature",
        ));
    }
    for (internal_ordinal, (placement, ordinal)) in
        internal.parameters.iter().zip(inbound_ordinals).enumerate()
    {
        let ordinal_index = usize::try_from(*ordinal).map_err(|_| Error::Overflow)?;
        if used.contains(&ordinal_index) || inbound.parameters.get(ordinal_index).is_none() {
            return Err(Error::Mismatch(
                "private entry ordinals are not a permutation",
            ));
        }
        used.push(ordinal_index);
        let scalar_codec = if placement.shape.class == ValueClass::BorrowedScalarReference {
            let structural_ordinal =
                internal_ordinal
                    .checked_sub(entry.parameters.len())
                    .ok_or(Error::Mismatch(
                        "scalar descriptor does not name a structural parameter",
                    ))?;
            let parameter = entry
                .structural_parameters
                .get(structural_ordinal)
                .ok_or(Error::Mismatch("scalar descriptor referent missing"))?;
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .ok_or(Error::Mismatch("scalar descriptor declaration missing"))?;
            let terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) = declaration.shape
            else {
                return Err(Error::Mismatch(
                    "scalar descriptor referent is not primitive",
                ));
            };
            runtime_abi::ScalarStorageCodec::from_scalar(scalar_type)
                .and_then(|codec| codec.encode())
                .ok_or(Error::Unsupported(
                    "private scalar referent has no native codec",
                ))?
        } else {
            0
        };
        parameters.insert(NativeEntryParameter {
            inbound_ordinal: *ordinal,
            scalar_codec,
        });
    }
    Ok(NativeEntryBridgeRequest {
        target: body.target,
        internal: internal.clone(),
        inbound: inbound.clone(),
        parameters,
    })
}

fn identity_transport(request: &NativeEntryBridgeRequest) -> bool {
    let internal = &request.internal;
    let inbound = &request.inbound;
    // An identity transport can alias the ordinary entry. This is an optional
    // zero-work optimization; differing transport is emitted, never relabeled.
    internal.policy == inbound.policy
        && internal.entry_control == inbound.entry_control
        && internal.result == inbound.result
        && internal.callback_materializations == inbound.callback_materializations
        && internal.stack_alignment == inbound.stack_alignment
        && internal.shadow_bytes == inbound.shadow_bytes
        && internal.ordinary_clobbers == inbound.ordinary_clobbers
        && internal
            .parameters
            .iter()
            .zip(request.parameters.iter())
            .all(|(placement, (_, parameter))| {
                inbound
                    .parameters
                    .get(parameter.inbound_ordinal as usize)
                    .is_some_and(|native| placement.has_same_parameter_transport(native))
            })
}

pub(crate) fn validate_entry_transport(
    private: &crate::ObjectCompilerPrivateFunction,
) -> Result<(), Error> {
    let Some(bridge) = &private.entry_bridge else {
        return Ok(());
    };
    let request = bridge.request();
    let ordinals = request
        .parameters
        .iter()
        .map(|(_, parameter)| parameter.inbound_ordinal)
        .collect::<Vec<_>>();
    let expected = create_private_function_object(
        private.identity,
        Arc::clone(&private.body),
        &request.inbound,
        &ordinals,
    )?;
    if expected.entry_bridge.as_ref() != Some(bridge) {
        return Err(Error::Mismatch(
            "private native entry differs from ordinary body transport",
        ));
    }
    Ok(())
}
