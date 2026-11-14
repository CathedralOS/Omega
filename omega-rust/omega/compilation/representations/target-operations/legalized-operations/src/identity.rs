/*
Canonical identity construction for `LegalizedOperationPlan` and its subordinate representations.

We compute collision-resistant SHA-256 identity digests (`LegalizedOperationPlanIdentity`) over
the canonical wire encoding of a legalized plan (`omega.terminal-legalized-operations.v78\0` and
`ordinary-scalar-graph.v39\0`). Every optional field uses a tagged `0` / `1 + payload` frame
(`encode_option`) so an absent row and an empty payload never share bytes.
*/

/*
Shared structural type catalogs (`StructuralTypeCatalog`) are pointer-cached across functions in
the same plan (`cached_catalog`) while still binding the actual retained access-view catalog
identity bytes (`access_view_catalog_identity_bytes`), ensuring a stale copied catalog cannot
borrow a unit's identity.
*/

mod calling;
mod normalized_foreign;
mod scalar_graph;
mod structural;
mod structural_types;

pub use calling::{
    encode_accessor_scalar_demand, encode_boundary_execution_identity, encode_call_plan,
    encode_placement, encode_shape,
};
pub use normalized_foreign::{
    encode_call_binding, encode_locator, encode_machine_function_identity,
    encode_native_callback_argument,
};
pub use structural::encode_provider_execution;
pub use structural_types::{encode_authored_field_storage, encode_structural_place};

use crate::{LegalizedOperationPlan, LegalizedOperationPlanIdentity, LegalizedStructuralContract};
use abstract_operations::ValueBinding;
use calling_conventions::ConventionalSumLayout;
use optimization_unit::{PsiProvenance, WorkSettlement};
use semantic_vocabulary::{BoundaryMachineId, IntegerType, IntegerValue, OperationId};
use structural_types::{
    encode_entry_claim, encode_structural_parameter, encode_structural_type,
    encode_target_structural_parameter,
};
use target_operations::{MachineRegister, TargetAccessCallAssociation, TargetAccessCallBinding};
use terminal_psi::{StructuralAccess, StructuralOperationResult, TerminalAccessViewInput};

pub fn legalized_operation_plan_identity(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-legalized-operations.v78\0");
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&plan.target.identity_bytes());
    encode_option(&mut bytes, plan.profile, |bytes, profile| {
        let name = profile.target_name().as_bytes();
        encode_len(bytes, name.len());
        bytes.extend_from_slice(name);
    });
    crate::codec_bytes::push_encoded(&mut bytes, plan.entry.get().to_le_bytes());
    bytes.extend_from_slice(b"ordinary-scalar-graph.v39\0");
    encode_len(&mut bytes, plan.scalar_functions.len());
    let mut cached_catalog = None;
    for function in &plan.scalar_functions {
        scalar_graph::encode(&mut bytes, function, &mut cached_catalog);
    }
    LegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}

pub fn encode_work(bytes: &mut Vec<u8>, work: &[WorkSettlement]) {
    encode_len(bytes, work.len());
    for settlement in work {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            }
        }
        crate::codec_bytes::push_encoded(bytes, settlement.units.to_le_bytes());
    }
}

pub(super) fn encode_ids(bytes: &mut Vec<u8>, values: impl IntoIterator<Item = u64>) {
    let values = values.into_iter().collect::<Vec<_>>();
    encode_len(bytes, values.len());
    for value in values {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    }
}

pub(super) fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    crate::codec_bytes::push_encoded(bytes, (len as u64).to_le_bytes());
}

pub(super) fn encode_option<T>(
    bytes: &mut Vec<u8>,
    value: Option<T>,
    encode: impl FnOnce(&mut Vec<u8>, T),
) {
    match value {
        Some(value) => {
            bytes.push(1);
            encode(bytes, value);
        }
        None => bytes.push(0),
    }
}

pub fn encode_integer(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
    }
}

pub fn encode_register(bytes: &mut Vec<u8>, register: MachineRegister) {
    let (tag, payload) = match register {
        MachineRegister::X86Rax => (0, 0),
        MachineRegister::X86Rcx => (1, 0),
        MachineRegister::X86Rdx => (2, 0),
        MachineRegister::X86Rbx => (3, 0),
        MachineRegister::X86Rsp => (4, 0),
        MachineRegister::X86Rbp => (5, 0),
        MachineRegister::X86Rsi => (6, 0),
        MachineRegister::X86Rdi => (7, 0),
        MachineRegister::X86R8 => (8, 0),
        MachineRegister::X86R9 => (9, 0),
        MachineRegister::X86R10 => (10, 0),
        MachineRegister::X86R11 => (11, 0),
        MachineRegister::X86R12 => (12, 0),
        MachineRegister::X86R13 => (13, 0),
        MachineRegister::X86R14 => (14, 0),
        MachineRegister::X86R15 => (15, 0),
        MachineRegister::X86Xmm(index) => (16, index),
        MachineRegister::Aarch64X(index) => (17, index),
        MachineRegister::Aarch64V(index) => (18, index),
    };
    bytes.push(tag);
    bytes.push(payload);
}

pub(super) fn encode_bindings(bytes: &mut Vec<u8>, bindings: &[ValueBinding]) {
    encode_len(bytes, bindings.len());
    for binding in bindings {
        crate::codec_bytes::push_encoded(bytes, binding.parameter.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, binding.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.scalar_type);
    }
}

pub fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: semantic_vocabulary::ScalarType) {
    match scalar_type {
        semantic_vocabulary::ScalarType::Boolean => bytes.push(0),
        semantic_vocabulary::ScalarType::Integer(integer) => {
            bytes.push(1);
            encode_integer_type(bytes, integer);
        }
        semantic_vocabulary::ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

pub(super) fn encode_integer_type(bytes: &mut Vec<u8>, integer: IntegerType) {
    bytes.push(match integer.carrier() {
        semantic_vocabulary::IntegerCarrier::Fixed => 0,
        semantic_vocabulary::IntegerCarrier::Address => 1,
    });
    bytes.push(match integer.sign() {
        semantic_vocabulary::IntegerSign::Signed => 0,
        semantic_vocabulary::IntegerSign::Unsigned => 1,
    });
    crate::codec_bytes::push_encoded(bytes, integer.bits().to_le_bytes());
}

pub fn encode_hosted_read_byte_identity(
    bytes: &mut Vec<u8>,
    operation: OperationId,
    boundary: BoundaryMachineId,
    result: &StructuralOperationResult,
    layout: &ConventionalSumLayout,
) {
    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
    encode_read_byte_payload(bytes, boundary, result, layout);
}

pub(super) fn encode_read_byte_payload(
    bytes: &mut Vec<u8>,
    boundary: BoundaryMachineId,
    result: &StructuralOperationResult,
    layout: &ConventionalSumLayout,
) {
    crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
    structural::encode_operation_result(bytes, result);
    encode_layout(bytes, layout);
}

pub(super) fn encode_layout(bytes: &mut Vec<u8>, layout: &ConventionalSumLayout) {
    calling::encode_shape(bytes, layout.shape);
    crate::codec_bytes::push_encoded(bytes, layout.tag_byte_offset.to_le_bytes());
    calling::encode_shape(bytes, layout.tag_shape);
    encode_len(bytes, layout.common_fields.len());
    for field in &layout.common_fields {
        calling::encode_shape(bytes, field.shape);
        crate::codec_bytes::push_encoded(bytes, field.byte_offset.to_le_bytes());
    }
    crate::codec_bytes::push_encoded(bytes, layout.payload_byte_offset.to_le_bytes());
    encode_len(bytes, layout.cases.len());
    for case in &layout.cases {
        encode_len(bytes, case.fields.len());
        for field in &case.fields {
            calling::encode_shape(bytes, field.shape);
            crate::codec_bytes::push_encoded(bytes, field.byte_offset.to_le_bytes());
        }
    }
}

fn encode_access_input(bytes: &mut Vec<u8>, input: &TerminalAccessViewInput) {
    bytes.extend_from_slice(&input.machine.get().to_le_bytes());
    bytes.extend_from_slice(&input.position.to_le_bytes());
    bytes.extend_from_slice(&input.runtime_position.to_le_bytes());
    for identity in [
        input.carrier_identity,
        input.source_machine_identity,
        input.source_state_identity,
        input.source_parameter_identity,
    ] {
        bytes.extend_from_slice(&identity.0.to_le_bytes());
    }
    bytes.push(match input.access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
    bytes.push(u8::from(input.binding_is_const));
    bytes.push(u8::from(input.binding_is_mutable));
    for identity in [
        input.view_identity,
        input.policy_identity,
        input.policy_plan_machine_identity,
        input.schema_identity,
    ] {
        bytes.extend_from_slice(&identity.0.to_le_bytes());
    }
    bytes.extend_from_slice(&input.placement_report_fingerprint.to_le_bytes());
    bytes.extend_from_slice(&input.placement_commitment);
}

pub fn encode_access_call_bindings(bytes: &mut Vec<u8>, bindings: &[TargetAccessCallBinding]) {
    encode_len(bytes, bindings.len());
    for binding in bindings {
        encode_access_input(bytes, &binding.input);
        bytes.extend_from_slice(&binding.argument_index.to_le_bytes());
        match &binding.association {
            TargetAccessCallAssociation::Captured { schema, carrier } => {
                bytes.push(0);
                bytes.extend_from_slice(&schema.get().to_le_bytes());
                bytes.extend_from_slice(&carrier.get().to_le_bytes());
            }
            TargetAccessCallAssociation::CallerFormal(input) => {
                bytes.push(1);
                encode_access_input(bytes, input);
            }
        }
    }
}

pub(super) fn encode_structural_contract(
    bytes: &mut Vec<u8>,
    function: &LegalizedStructuralContract,
    cached_catalog: &mut Option<(abstract_operations::StructuralTypeCatalog, Vec<u8>)>,
) {
    encode_option(bytes, function.result.as_ref(), |bytes, result| {
        structural::encode_result(bytes, result)
    });
    if let Some((cached, encoded)) = cached_catalog.as_ref()
        && cached.ptr_eq(&function.structural_types)
    {
        bytes.extend_from_slice(encoded);
    } else {
        let start = bytes.len();
        encode_len(bytes, function.structural_types.len());
        for declaration in &function.structural_types {
            encode_structural_type(bytes, declaration);
        }
        encode_len(bytes, function.structural_types.referents().len());
        for referent in function.structural_types.referents() {
            bytes.extend_from_slice(&referent.id.get().to_le_bytes());
            bytes.extend_from_slice(&referent.identity.0.to_le_bytes());
            bytes.push(u8::from(referent.foreign_descriptor));
            bytes.push(u8::from(referent.abstract_opaque));
        }
        bytes.extend_from_slice(&optimization_unit::access_view_catalog_identity_bytes(
            &function.structural_types,
        ));
        *cached_catalog = Some((function.structural_types.clone(), bytes[start..].to_vec()));
    }
    encode_len(bytes, function.parameters.len());
    for parameter in &function.parameters {
        encode_structural_parameter(bytes, &parameter.semantic);
        encode_target_structural_parameter(bytes, &parameter.target);
    }
    encode_len(bytes, function.structural_places.len());
    for place in &function.structural_places {
        encode_structural_place(bytes, *place);
    }
    encode_len(bytes, function.entry_claims.len());
    for claim in &function.entry_claims {
        encode_entry_claim(bytes, claim);
    }
    encode_ids(
        bytes,
        function
            .published_service_ceiling
            .iter()
            .map(|service| service.get()),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn work_settlements_preserve_numeric_wire_encoding() {
        use super::{PsiProvenance, WorkSettlement, encode_work};
        use semantic_vocabulary::{EdgeId, OperationId};
        let mut bytes = Vec::new();
        encode_work(
            &mut bytes,
            &[
                WorkSettlement {
                    site: PsiProvenance::Operation(OperationId::new(7).unwrap()),
                    units: 3,
                },
                WorkSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(9).unwrap()),
                    units: 5,
                },
            ],
        );
        assert_eq!(
            bytes,
            [
                2, 0, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 1, 9, 0,
                0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
    }
}
