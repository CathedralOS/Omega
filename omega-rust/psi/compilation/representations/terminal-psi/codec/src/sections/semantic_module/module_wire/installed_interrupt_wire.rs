//! Installation requests keep their exact callback contracts and slot preimage.
use super::super::wire_fields::{Bytes, Digest, Id, List, Nominal, Strings, Text, wire_record};
use terminal_psi::TerminalInstalledInterruptRegistration;

wire_record! {
    Registration: TerminalInstalledInterruptRegistration {
        operation: Id,
        installer: Id,
        service: Id,
        registrar_boundary: Id,
        registrar: Nominal,
        registrar_owner: Nominal,
        registrar_application: Text,
        registrar_declaring_application: Text,
        registrar_arguments: Strings,
        registrar_lifetimes: Strings,
        registrar_requirement: Text,
        slot_declaration: Nominal,
        slot_commitment: Digest,
        slot_encoding: Bytes,
        callbacks: List<super::callback_application_wire::Application>,
    }
}
