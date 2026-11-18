//! The exact order, ABI, effects and optional adapter of a callable physical entry.
use crate::PhysicalRecipe;
use crate::native_entry_bridge::{
    NativeEntryBridgePlan, NativeEntryBridgeRequest, NativeEntryParameter,
};
use calling_conventions::BoundaryEntryPlan;
use function_identity::MachineFunctionIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableContext {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub entry: semantic_vocabulary::MachineId,
    pub effects: calling_conventions::StateEffectsEvidence,
    pub inbound: BoundaryEntryPlan,
    pub ordinals: Vec<u32>,
}

pub type CallableEntry<Origin, Cpu> = PhysicalRecipe<Origin, CallableInstructions<Origin, Cpu>>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableInstructions<Origin, Cpu> {
    pub context: CallableContext,
    pub bridge: Option<NativeEntryBridgePlan<Origin>>,
    pub cpu: Cpu,
}
impl<Origin, Cpu> CallableEntry<Origin, Cpu> {
    pub fn into_parts(self) -> CallableInstructions<Origin, Cpu> {
        self.instructions
    }
}

pub type PrivateEntry<Origin, Cpu> = PhysicalRecipe<Origin, PrivateEntryParts<Origin, Cpu>>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateEntryParts<Origin, Cpu> {
    pub identity: MachineFunctionIdentity,
    pub transport: CallableEntry<Origin, Cpu>,
}
impl<Origin, Cpu> PrivateEntry<Origin, Cpu> {
    pub fn into_parts(self) -> PrivateEntryParts<Origin, Cpu> {
        self.instructions
    }
    pub fn bridge(&self) -> Option<&NativeEntryBridgePlan<Origin>> {
        self.instructions.transport.instructions.bridge.as_ref()
    }
    pub fn cpu_state(&self) -> &Cpu {
        &self.instructions.transport.instructions.cpu
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryRequestError {
    Mismatch(&'static str),
    Unsupported(&'static str),
    Overflow,
}
use EntryRequestError as Error;
impl std::fmt::Display for EntryRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch(reason) | Self::Unsupported(reason) => f.write_str(reason),
            Self::Overflow => f.write_str("shared object coordinate exceeds supported size"),
        }
    }
}
impl std::error::Error for EntryRequestError {}

pub fn native_entry_request_from_abi(
    target: target::NativeTarget,
    abi: &target_vocabulary::LegalizedFunctionAbi,
    effects: calling_conventions::StateEffectsEvidence,
    inbound: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
) -> Result<NativeEntryBridgeRequest, Error> {
    let internal = &abi.call_plan;
    if inbound_ordinals.len() != internal.parameters.len()
        || inbound.call.parameters.len() != internal.parameters.len()
    {
        return Err(Error::Mismatch("callable entry formal count differs"));
    }
    let mut used = Vec::with_capacity(inbound_ordinals.len());
    let mut parameters = arena::Arena::with_capacity(inbound_ordinals.len());
    if abi.scalar_parameters.len() + abi.structural_parameters.len() != internal.parameters.len() {
        return Err(Error::Mismatch(
            "callable body parameters differ from its retained ABI",
        ));
    }
    for (internal_ordinal, ordinal) in inbound_ordinals.iter().enumerate() {
        let ordinal_index = usize::try_from(*ordinal).map_err(|_| Error::Overflow)?;
        if used.contains(&ordinal_index) || inbound.call.parameters.get(ordinal_index).is_none() {
            return Err(Error::Mismatch(
                "callable entry ordinals are not a permutation",
            ));
        }
        used.push(ordinal_index);
        let scalar_codec = abi
            .entry_scalar_codecs
            .get(internal_ordinal)
            .ok_or(Error::Mismatch("callable entry conversion is missing"))?
            .ok_or(Error::Unsupported(
                "callable scalar referent has no native codec",
            ))?;
        parameters.insert(NativeEntryParameter {
            inbound_ordinal: *ordinal,
            scalar_codec,
        });
    }
    Ok(NativeEntryBridgeRequest {
        target,
        internal: internal.clone(),
        inbound: inbound.clone(),
        body_footprint: Some(effects),
        parameters,
    })
}

pub fn identity_transport(request: &NativeEntryBridgeRequest) -> bool {
    let internal = &request.internal;
    let inbound = &request.inbound.call;
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
