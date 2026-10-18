//! Exact current semantic contracts shared by graph and selected identities.
use super::calling::{encode_call_plan, encode_placement};
use super::encoding::{encode_ids, encode_len};
use super::structural::encode_call_source;
use super::structural_types::{encode_structural_argument, encode_target_structural_argument};
use crate::legalized_operations::{
    LegalizedBoundarySettlement, LegalizedScalarArgument, LegalizedScalarCall,
    LegalizedStructuralContract,
};

pub(super) fn encode_call(bytes: &mut Vec<u8>, call: &LegalizedScalarCall) {
    super::placed_bindings::encode_placed_call_bindings(bytes, &call.placed_bindings);
    match &call.structural_result {
        None => bytes.push(0),
        Some(result) => {
            bytes.push(1);
            super::structural_result::encode_operation_result(bytes, result);
        }
    }
    encode_len(bytes, call.descriptor_origins.len());
    for origin in &call.descriptor_origins {
        match origin {
            crate::LegalizedDescriptorOrigin::Opaque => bytes.push(3),
            crate::LegalizedDescriptorOrigin::Empty => bytes.push(2),
            crate::LegalizedDescriptorOrigin::Argument {
                source,
                exact,
                indirect,
            } => {
                bytes.push(0);
                encode_structural_argument(bytes, source);
                bytes.push(u8::from(*exact));
                bytes.push(u8::from(*indirect));
            }
            crate::LegalizedDescriptorOrigin::Literal { machine, place } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
        }
    }
    encode_call_source(bytes, &call.source);
    bytes.extend_from_slice(&call.target.canonical_bytes());
    encode_call_plan(bytes, &call.call_plan);
    encode_len(bytes, call.arguments.len());
    for argument in &call.arguments {
        match argument {
            LegalizedScalarArgument::Scalar { source, placement } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                encode_placement(bytes, placement);
            }
            LegalizedScalarArgument::Structural {
                semantic, target, ..
            } => {
                bytes.push(1);
                encode_structural_argument(bytes, semantic);
                encode_target_structural_argument(bytes, target);
            }
        }
    }
    match &call.result_placement {
        Some(placement) => {
            bytes.push(1);
            encode_placement(bytes, placement);
        }
        None => bytes.push(0),
    }
    encode_len(bytes, call.claim_transfers.len());
    for transfer in &call.claim_transfers {
        crate::codec_bytes::push_encoded(bytes, transfer.claim.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, transfer.argument_index.to_le_bytes());
    }
    encode_ids(
        bytes,
        call.requirement_obligations.iter().map(|value| value.get()),
    );
    let crashes = terminal_codec::encode_crash_route_buckets(&call.crash_continuations)
        .expect("canonical call crash continuations");
    encode_len(bytes, crashes.len());
    bytes.extend_from_slice(&crashes);
}

impl LegalizedStructuralContract {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        super::plan::encode_structural_contract(&mut bytes, self);
        bytes
    }
}
impl LegalizedScalarCall {
    pub fn canonical_bytes_with_effects(
        &self,
        effect: optimization_unit::EffectLink,
        ownership: &[optimization_unit::OwnershipEvent],
    ) -> Vec<u8> {
        let mut bytes = self.canonical_bytes();
        super::structural::encode_effect(&mut bytes, effect);
        super::structural::encode_ownership_roster(&mut bytes, ownership);
        bytes
    }
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_call(&mut bytes, self);
        bytes
    }
}
impl LegalizedBoundarySettlement {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        super::structural::encode_boundary_settlement(&mut bytes, self);
        bytes
    }
}
