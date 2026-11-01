//! Native declaration telescopes; storage and contracts stay on the boundary.

use super::super::wire_fields::{
    Field, Flag, Id, Index, List, Optional, ResultField, Text, U32, wire_record,
};
use super::{CodecError, Reader, Writer};
use semantic_vocabulary::CallingGenericFormal;
use terminal_psi::{
    CallingAbiDeclaration, CallingAbiParameter, CallingAbiStorage, CallingCallbackBinder,
    CallingDirectCallback, CallingPolicyInstance, CallingPolicyRequest, CallingRuntimeFormal,
    CallingSignatureDeclaration, CallingSignatureTelescope, CallingStaticOperand,
};

wire_record! {
    AbiDeclaration: CallingAbiDeclaration {
        id: Id,
        requirement_identity: Index,
        declaring_application: Index,
        operands: List<StaticOperand>,
        formals: List<GenericFormal>,
        default_eligible: Flag,
        storage: ResultField<AbiStorage, Text>,
    }
    AbiStorage: CallingAbiStorage {
        parameters: List<AbiParameter>,
        result: Optional<Id>,
        telescope: Telescope,
    }
    StaticOperand: CallingStaticOperand { ordinal: U32, kind: OperandKind, identity: Index }
    AbiParameter: CallingAbiParameter { structural_type: Id, callback_backing: Optional<Id>, access: super::super::wire_fields::Access }
    GenericFormal: CallingGenericFormal { ordinal: U32, kind: OperandKind, callback: Optional<BinderId> }
    PolicyRequest: CallingPolicyRequest {
        declaration: ResultField<Index, Text>,
        display_name: Text,
        instances: ResultField<List<PolicyInstance>, List<Text>>,
    }
    PolicyInstance: CallingPolicyInstance {
        declaring_application: Index,
        policy: ResultField<super::target_family_wire::Callable, Text>,
        signatures: ResultField<List<Id>, Text>,
    }
    Declaration: CallingSignatureDeclaration {
        boundary: Id,
        application: ResultField<Id, Text>,
    }
    Telescope: CallingSignatureTelescope {
        parameters: List<Formal>,
        binders: List<Binder>,
        direct_callbacks: List<DirectCallback>,
    }
    Formal: CallingRuntimeFormal { identity: Parameter, native_ordinal: U32 }
    Binder: CallingCallbackBinder {
        binder: BinderId,
        requirement: Requirement,
        requirement_identity: Index,
        static_machine_ordinal: U32,
        invokes_synchronously: Flag,
    }
    DirectCallback: CallingDirectCallback {
        identity: Parameter,
        native_ordinal: U32,
        binder: BinderId,
        requirement: Requirement,
    }
}

struct OperandKind;
impl Field<semantic_vocabulary::CallingOperandKind> for OperandKind {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::CallingOperandKind,
    ) -> Result<(), CodecError> {
        use semantic_vocabulary::CallingOperandKind::*;
        writer.u8(match value {
            Type => 0,
            Const => 1,
            Value => 2,
            Machine => 3,
            Implementation => 4,
            Proposition => 5,
        });
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::CallingOperandKind, CodecError> {
        use semantic_vocabulary::CallingOperandKind::*;
        Ok(match reader.u8()? {
            0 => Type,
            1 => Const,
            2 => Value,
            3 => Machine,
            4 => Implementation,
            5 => Proposition,
            _ => {
                return Err(CodecError::MalformedStructuralFoundation(
                    "unknown calling operand kind",
                ));
            }
        })
    }
}

/// Native-declaration identities: nonzero `u64`s outside the Psi id family.
macro_rules! native_ids {
    ($($marker:ident: $ty:ident, $zero:literal;)*) => {$(
        struct $marker;
        impl Field<semantic_vocabulary::$ty> for $marker {
            fn write(writer: &mut Writer, value: &semantic_vocabulary::$ty) -> Result<(), CodecError> {
                writer.u64(value.get());
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::$ty, CodecError> {
                semantic_vocabulary::$ty::new(reader.u64()?)
                    .ok_or(CodecError::MalformedStructuralFoundation($zero))
            }
        }
    )*};
}
native_ids! {
    Parameter: NativeParameterId, "zero native parameter identity";
    BinderId: StaticMachineBinderId, "zero callback binder identity";
    Requirement: CallbackRequirementId, "zero callback requirement identity";
}
