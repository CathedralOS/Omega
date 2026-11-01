//! Callback applications on the wire: the registrar, requirement and
//! published contract of one admitted callback body.

use super::super::wire_fields::{
    Digest, Flag, Id, MachineDeclaration, OptionalId, Text, U32, wire_record,
};
use terminal_psi::TerminalCallbackApplication;

wire_record! {
    Application: TerminalCallbackApplication {
        application_index: U32,
        registrar: Id,
        static_machine_ordinal: U32,
        requirement: Text,
        boundary: OptionalId,
        published_contract: Digest,
        entry_admitted: Flag,
        body: MachineDeclaration,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sections::semantic_module::wire::{Reader, Writer};
    use crate::sections::semantic_module::wire_fields::Field;
    use semantic_vocabulary::OperationId;

    #[test]
    fn callback_application_wire_preserves_identity_and_rejects_truncation() {
        let application = TerminalCallbackApplication {
            application_index: 17,
            registrar: OperationId::new(42).expect("nonzero operation"),
            static_machine_ordinal: 2,
            requirement: "Hooks::call(u64)".to_owned(),
            boundary: semantic_vocabulary::BoundaryMachineId::new(3),
            published_contract: [3; 32],
            entry_admitted: true,
            body: terminal_psi::CheckedMachineDeclaration {
                semantic_declaration: Default::default(),
                parameter_types: Vec::new(),
                callable: Default::default(),
                result_type: Default::default(),
                machine: terminal_psi::NominalDeclarationIdentity::new(
                    terminal_psi::TerminalIdentityIndex(11),
                ),
                state: terminal_psi::NominalDeclarationIdentity::new(
                    terminal_psi::TerminalIdentityIndex(12),
                ),
                contract: [5; 32],
                specialization: None,
            },
        };
        let mut writer = Writer::default();
        Application::write(&mut writer, &application).expect("encode callback");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            Application::read(&mut reader).expect("decode callback"),
            application
        );
        assert_eq!(reader.remaining(), 0);
        for byte_count in 0..bytes.len() {
            assert!(
                Application::read(&mut Reader::new(&bytes[..byte_count])).is_err(),
                "accepted {byte_count} bytes"
            );
        }
        let mut unadmitted = application;
        unadmitted.entry_admitted = false;
        unadmitted.boundary = None;
        let mut writer = Writer::default();
        Application::write(&mut writer, &unadmitted).unwrap();
        assert_eq!(
            Application::read(&mut Reader::new(&writer.finish())).unwrap(),
            unadmitted
        );
        let mut module = terminal_psi::TerminalModule::declarations();
        module.callback_applications.push(unadmitted);
        super::super::super::canonical_order::validate_canonical_order(&module).unwrap();
        module.callback_applications[0].entry_admitted = true;
        assert!(super::super::super::canonical_order::validate_canonical_order(&module).is_err());
        module.callback_applications[0].entry_admitted = false;
        module.callback_applications[0].boundary = semantic_vocabulary::BoundaryMachineId::new(3);
        assert!(super::super::super::canonical_order::validate_canonical_order(&module).is_err());
    }
}
