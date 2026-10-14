use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use terminal_psi::TerminalCallbackApplication;

pub(super) fn encode(
    writer: &mut Writer,
    application: &TerminalCallbackApplication,
) -> Result<(), CodecError> {
    writer.u32(application.application_index);
    writer.id(application.registrar);
    writer.u32(application.static_machine_ordinal);
    writer.string("callback requirement", &application.requirement)?;
    writer.bytes(&application.published_contract);
    super::super::machine_declaration_wire::encode(writer, &application.body)
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<TerminalCallbackApplication, CodecError> {
    Ok(TerminalCallbackApplication {
        application_index: reader.u32()?,
        registrar: reader.id("callback registrar")?,
        static_machine_ordinal: reader.u32()?,
        requirement: reader.string("callback requirement")?,
        published_contract: reader.array()?,
        body: super::super::machine_declaration_wire::decode(reader)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::OperationId;

    #[test]
    fn callback_application_wire_preserves_identity_and_rejects_truncation() {
        let application = TerminalCallbackApplication {
            application_index: 17,
            registrar: OperationId::new(42).expect("nonzero operation"),
            static_machine_ordinal: 2,
            requirement: "Hooks::call(u64)".to_owned(),
            published_contract: [3; 32],
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
        encode(&mut writer, &application).expect("encode callback");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode(&mut reader).expect("decode callback"), application);
        assert_eq!(reader.remaining(), 0);
        for byte_count in 0..bytes.len() {
            assert!(
                decode(&mut Reader::new(&bytes[..byte_count])).is_err(),
                "accepted {byte_count} bytes"
            );
        }
    }
}
