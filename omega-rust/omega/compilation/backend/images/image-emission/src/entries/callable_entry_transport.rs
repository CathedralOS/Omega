//! Join native entry transport to the retained ordinary body, not a replacement
//! body signature. Scalar codecs come from the exact Terminal referent and the
//! runtime ABI's default scalar storage, never from pointer width or debug names.

mod body_footprint;

use std::sync::Arc;

use crate::{FunctionFragmentObjectArtifactError as Error, ObjectArtifact, PrivateFunctionObject};
use calling_conventions::{BoundaryEntryPlan, ValueClass};
use function_identity::MachineFunctionIdentity;
use machine_emission::native_entry_bridge::{
    InstalledInterruptEntryPlan, NativeEntryBridgeRequest, NativeEntryParameter,
    emit_installed_interrupt_entry_bridge, emit_native_entry_bridge,
};

/// A checked entry remains a checked entry when its ABI needs no adapter bytes.
/// Keep that result beside the optional bridge rather than representing an alias
/// as the absence of state and body correspondence evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableEntryTransport {
    pub(crate) bridge: Option<machine_emission::native_entry_bridge::NativeEntryBridge>,
    pub(crate) effects: calling_conventions::StateEffectsEvidence,
    pub(crate) inbound: BoundaryEntryPlan,
    pub(crate) ordinals: Vec<u32>,
    source_psi: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
}

pub(crate) fn validate_callable_transport(
    body: &ObjectArtifact,
    transport: &CallableEntryTransport,
) -> Result<(), Error> {
    let expected = create_callable_entry_transport(body, &transport.inbound, &transport.ordinals)?;
    if expected != *transport {
        return Err(Error::Mismatch(
            "callable entry transport belongs to another body, ABI or state promise",
        ));
    }
    Ok(())
}

/// Bind a caller-scoped private identity to its unchanged body and native entry.
/// The ordinal roster is in internal lane order, after the nominal formal join.
pub fn create_private_function_object(
    identity: MachineFunctionIdentity,
    body: Arc<ObjectArtifact>,
    inbound: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
) -> Result<PrivateFunctionObject, Error> {
    let transport = create_callable_entry_transport(&body, inbound, inbound_ordinals)?;
    Ok(PrivateFunctionObject {
        identity,
        body,
        entry_bridge: transport.bridge,
        entry_effects: transport.effects,
        inbound: inbound.clone(),
        inbound_ordinals: inbound_ordinals.to_vec(),
    })
}

/// Bind an explicitly admitted installed arrival to its ordinary handler body.
/// The plan retains the exact arrival protocol and stack domain. It grants no
/// ordinary callback the privilege or retained roots of an installed arrival.
pub fn create_installed_private_function_object(
    identity: MachineFunctionIdentity,
    body: Arc<ObjectArtifact>,
    inbound: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
    installed_plan: InstalledInterruptEntryPlan,
) -> Result<PrivateFunctionObject, Error> {
    let execution = body.installed_entry_execution().ok_or(Error::Mismatch(
        "installed private entry has no retained execution domain",
    ))?;
    if execution.entry().callback_function != identity
        || execution.entry().entry().inbound() != inbound
        || execution.entry().entry().inbound_ordinals() != inbound_ordinals
    {
        return Err(Error::Mismatch(
            "installed private entry changed its execution domain",
        ));
    }
    let request = native_entry_request(&body, inbound, inbound_ordinals)?;
    let bridge = emit_installed_interrupt_entry_bridge(request, installed_plan)
        .map_err(Error::NativeEntryBridge)?;
    Ok(PrivateFunctionObject {
        identity,
        body,
        entry_effects: bridge.complete_effects().clone(),
        entry_bridge: Some(bridge),
        inbound: inbound.clone(),
        inbound_ordinals: inbound_ordinals.to_vec(),
    })
}

/// Emit the native ABI bridge for an ordinary callable body. The returned
/// code is a physical product, not a private callback identity or installed
/// component entry. Both publication owners retain their own exact custody.
pub fn create_callable_entry_transport(
    body: &ObjectArtifact,
    inbound: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
) -> Result<CallableEntryTransport, Error> {
    if body.installed_entry_execution().is_some() {
        return Err(Error::Mismatch(
            "a privileged installed body cannot become an ordinary callable entry",
        ));
    }
    let request = native_entry_request(body, inbound, inbound_ordinals)?;
    let (bridge, effects) = if identity_transport(&request) {
        let effects = request
            .checked_complete_effects(&calling_conventions::StateEffectsEvidence::empty())
            .map_err(Error::NativeEntryBridge)?;
        (None, effects)
    } else {
        let bridge = emit_native_entry_bridge(request).map_err(Error::NativeEntryBridge)?;
        let effects = bridge.complete_effects().clone();
        (Some(bridge), effects)
    };
    Ok(CallableEntryTransport {
        bridge,
        effects,
        inbound: inbound.clone(),
        ordinals: inbound_ordinals.to_vec(),
        source_psi: body.psi(),
        entry: body.entry(),
    })
}

/// Resolve the native transport against the unchanged ordinary body. Image
/// roots and callback entries share this join without sharing their identity
/// or installation authority.
pub(crate) fn native_entry_request(
    body: &ObjectArtifact,
    inbound: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
) -> Result<NativeEntryBridgeRequest, Error> {
    let replay = body
        .fragment_replay
        .as_ref()
        .ok_or(Error::Mismatch("callable entry lacks body custody"))?;
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
        .ok_or(Error::Mismatch("callable body entry plan missing"))?
        .graph
        .call_plan;
    if inbound_ordinals.len() != internal.parameters.len()
        || inbound.call.parameters.len() != internal.parameters.len()
    {
        return Err(Error::Mismatch("callable entry formal count differs"));
    }
    let mut used = Vec::with_capacity(inbound_ordinals.len());
    let mut parameters = arena::Arena::with_capacity(inbound_ordinals.len());
    let module = replay.terminal_module();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == body.entry)
        .ok_or(Error::Mismatch("callable Terminal entry missing"))?;
    if entry.parameters.len() + entry.structural_parameters.len() != internal.parameters.len() {
        return Err(Error::Mismatch(
            "callable body lanes differ from Terminal signature",
        ));
    }
    for (internal_ordinal, (placement, ordinal)) in
        internal.parameters.iter().zip(inbound_ordinals).enumerate()
    {
        let ordinal_index = usize::try_from(*ordinal).map_err(|_| Error::Overflow)?;
        if used.contains(&ordinal_index) || inbound.call.parameters.get(ordinal_index).is_none() {
            return Err(Error::Mismatch(
                "callable entry ordinals are not a permutation",
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
                    "callable scalar referent has no native codec",
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
        body_footprint: Some(body_footprint::collect(body, inbound.state.preemption)?),
        parameters,
    })
}

fn identity_transport(request: &NativeEntryBridgeRequest) -> bool {
    let internal = &request.internal;
    let inbound = &request.inbound.call;
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
    // An alias still promises an external ABI. Reconstruct its transport too,
    // so removing an adapter cannot bypass the retained calling recipe.
    let installed = private
        .entry_bridge
        .as_ref()
        .and_then(|bridge| bridge.installed_interrupt());
    let expected = match installed {
        Some(plan) => create_installed_private_function_object(
            private.identity,
            Arc::clone(&private.body),
            &private.inbound,
            &private.inbound_ordinals,
            plan,
        )?,
        None => create_private_function_object(
            private.identity,
            Arc::clone(&private.body),
            &private.inbound,
            &private.inbound_ordinals,
        )?,
    };
    if expected.entry_bridge != private.entry_bridge
        || expected.entry_effects != private.entry_effects
    {
        return Err(Error::Mismatch(
            "private native entry differs from ordinary body transport",
        ));
    }
    Ok(())
}
