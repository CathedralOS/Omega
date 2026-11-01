//! The existing selected-opaque protocol, with canonical storage coordinates.

use super::super::wire_fields::{Bytes, Field, Id, Index, Text, U16, U64, wire_enum, wire_record};
use super::{CodecError, Reader, Writer};
use language_semantics::opaque_representations::{
    OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
    OpaqueRepresentationLifecycleDisposition, SelectedOpaqueApplication,
};
use terminal_psi::TerminalOpaqueRepresentation;

wire_record! {
    Representation: TerminalOpaqueRepresentation {
        opaque: Id, carrier: Id,
        opaque_storage_identity: Index, carrier_storage_identity: Index,
        application: Application
    }
    Application: SelectedOpaqueApplication {
        opaque_identity: Text, carrier_identity: Text, conformance_identity: Text,
        application_report_fingerprint: U64, conformance_application_commitment: Digest,
        identity_encoding: Bytes, schema_version: U16,
        origin: Origin, lifecycle: Lifecycle, copy_disposition: CopyDisposition,
        selected_application_commitment: Digest,
    }
}

wire_enum! {
    Origin: OpaqueRepresentationApplicationOrigin, "opaque representation origin" { 1 => NamedConformance, }
    Lifecycle: OpaqueRepresentationLifecycleDisposition, "opaque representation lifecycle" { 1 => Inert, }
    CopyDisposition: OpaqueRepresentationCopyDisposition, "opaque representation copy disposition" {
        1 => PlacementOnly, 2 => CheckedSemanticCopy,
    }
}

struct Digest;
impl Field<[u8; 32]> for Digest {
    fn write(writer: &mut Writer, value: &[u8; 32]) -> Result<(), CodecError> {
        writer.bytes(value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<[u8; 32], CodecError> {
        reader.array()
    }
}
