//! Native declaration telescopes; storage and contracts stay on the boundary.

use super::super::wire_fields::{Field, Flag, Id, Index, List, U32, wire_record};
use super::{CodecError, Reader, Writer};
use terminal_psi::{
    CallingCallbackBinder, CallingDirectCallback, CallingRuntimeFormal,
    CallingSignatureDeclaration, CallingSignatureTelescope,
};

wire_record! {
    Declaration: CallingSignatureDeclaration {
        boundary: Id,
        requirement_identity: Index,
        telescope: TelescopeOrRefusal,
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

/// A complete telescope, or the refusal that deferred it.
struct TelescopeOrRefusal;
impl Field<Result<CallingSignatureTelescope, String>> for TelescopeOrRefusal {
    fn write(
        writer: &mut Writer,
        value: &Result<CallingSignatureTelescope, String>,
    ) -> Result<(), CodecError> {
        writer.boolean(value.is_ok());
        match value {
            Ok(telescope) => Telescope::write(writer, telescope),
            Err(reason) => writer.string("deferred calling signature refusal", reason),
        }
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<Result<CallingSignatureTelescope, String>, CodecError> {
        Ok(if reader.boolean()? {
            Ok(Telescope::read(reader)?)
        } else {
            Err(reader.string("deferred calling signature refusal")?)
        })
    }
}
