//! Closed conformance applications on the wire: the telescope, subject and
//! trait identities, realization callables, rows and commitment of one
//! application.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::decode_counted;
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRow,
};

pub(super) fn encode_closed_conformance_application(
    writer: &mut Writer,
    application: &ClosedConformanceApplication,
) -> Result<(), CodecError> {
    writer.id(application.owner);
    writer.string(
        "closed conformance declaration identity",
        &application.declaration_identity,
    )?;
    writer.len("closed conformance telescope", application.telescope.len())?;
    for binding in &application.telescope {
        writer.string("closed conformance parameter", &binding.parameter)?;
        writer.u8(match binding.kind {
            ClosedConformanceParameterKind::Lifetime => 1,
            ClosedConformanceParameterKind::Type => 2,
            ClosedConformanceParameterKind::Const => 3,
            ClosedConformanceParameterKind::Machine => 4,
        });
        writer.string("closed conformance argument", &binding.argument)?;
    }
    writer.boolean(application.subject_identity.is_some());
    if let Some(subject) = &application.subject_identity {
        writer.string("closed conformance subject identity", subject)?;
    }
    writer.string(
        "closed conformance trait identity",
        &application.trait_identity,
    )?;
    writer.strings(
        "closed conformance trait lifetime arguments",
        &application.trait_lifetime_arguments,
    )?;
    writer.strings(
        "closed conformance trait arguments",
        &application.trait_arguments,
    )?;
    writer.len(
        "closed conformance realization callables",
        application.realization_callables.len(),
    )?;
    for callable in &application.realization_callables {
        writer.string(
            "closed conformance realization callable identity",
            &callable.source_callable_identity,
        )?;
        writer.id(callable.machine);
        writer.u8(match callable.result {
            ClosedConformanceCallableResult::Unit => 1,
            ClosedConformanceCallableResult::I32 => 2,
            ClosedConformanceCallableResult::Bool => 3,
        });
    }
    writer.len("closed conformance rows", application.rows.len())?;
    for row in &application.rows {
        writer.string(
            "closed conformance row declaring trait identity",
            &row.declaring_trait_identity,
        )?;
        writer.string(
            "closed conformance row public requirement identity",
            &row.public_requirement_identity,
        )?;
        writer.strings("closed conformance row family tuple", &row.family_tuple)?;
        writer.string(
            "closed conformance row requirement identity",
            &row.requirement_identity,
        )?;
        writer.string(
            "closed conformance row realization identity",
            &row.realization_identity,
        )?;
        writer.boolean(row.realization_callable_identity.is_some());
        if let Some(identity) = &row.realization_callable_identity {
            writer.string(
                "closed conformance row realization callable identity",
                identity,
            )?;
        }
    }
    writer.u64(application.report_fingerprint);
    writer.bytes(&application.commitment.as_bytes());
    Ok(())
}

pub(super) fn decode_closed_conformance_application(
    reader: &mut Reader<'_>,
) -> Result<ClosedConformanceApplication, CodecError> {
    Ok(ClosedConformanceApplication {
        owner: reader.id("MachineId")?,
        declaration_identity: reader.string("closed conformance declaration identity")?,
        telescope: decode_counted(reader, |reader| {
            Ok(ClosedConformanceParameterBinding {
                parameter: reader.string("closed conformance parameter")?,
                kind: match reader.u8()? {
                    1 => ClosedConformanceParameterKind::Lifetime,
                    2 => ClosedConformanceParameterKind::Type,
                    3 => ClosedConformanceParameterKind::Const,
                    4 => ClosedConformanceParameterKind::Machine,
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "ClosedConformanceParameterKind",
                            tag,
                        ));
                    }
                },
                argument: reader.string("closed conformance argument")?,
            })
        })?,
        subject_identity: reader
            .boolean()?
            .then(|| reader.string("closed conformance subject identity"))
            .transpose()?,
        trait_identity: reader.string("closed conformance trait identity")?,
        trait_lifetime_arguments: reader.strings("closed conformance trait lifetime arguments")?,
        trait_arguments: reader.strings("closed conformance trait arguments")?,
        realization_callables: decode_counted(reader, |reader| {
            Ok(terminal_psi::ClosedConformanceRealizationCallable {
                source_callable_identity: reader
                    .string("closed conformance realization callable identity")?,
                machine: reader.id("MachineId")?,
                result: match reader.u8()? {
                    1 => ClosedConformanceCallableResult::Unit,
                    2 => ClosedConformanceCallableResult::I32,
                    3 => ClosedConformanceCallableResult::Bool,
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "ClosedConformanceCallableResult",
                            tag,
                        ));
                    }
                },
            })
        })?,
        rows: decode_counted(reader, |reader| {
            Ok(ClosedConformanceRow {
                declaring_trait_identity: reader
                    .string("closed conformance row declaring trait identity")?,
                public_requirement_identity: reader
                    .string("closed conformance row public requirement identity")?,
                family_tuple: reader.strings("closed conformance row family tuple")?,
                requirement_identity: reader
                    .string("closed conformance row requirement identity")?,
                realization_identity: reader
                    .string("closed conformance row realization identity")?,
                realization_callable_identity: reader
                    .boolean()?
                    .then(|| reader.string("closed conformance row realization callable identity"))
                    .transpose()?,
            })
        })?,
        report_fingerprint: reader.u64()?,
        commitment: ClosedConformanceApplicationCommitment::from_digest(reader.array()?),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Reader, Writer, decode_closed_conformance_application,
        encode_closed_conformance_application,
    };
    use terminal_psi::{
        ClosedConformanceApplication, ClosedConformanceCallableResult,
        ClosedConformanceRealizationCallable, ClosedConformanceRow,
        closed_conformance_application_commitment,
        closed_conformance_application_report_fingerprint,
    };

    /// The canonical tuple a `<Width: u32, Lanes: u32>` family row retains:
    /// const identities in binder declaration order, never display spellings.
    fn width_lanes_tuple() -> Vec<String> {
        vec![
            "named(integer-const(16))".to_owned(),
            "named(integer-const(4))".to_owned(),
        ]
    }

    fn row(public_requirement_identity: &str, family_tuple: Vec<String>) -> ClosedConformanceRow {
        ClosedConformanceRow {
            declaring_trait_identity: "package::Scanner".into(),
            public_requirement_identity: public_requirement_identity.into(),
            family_tuple,
            requirement_identity: "package::Scanner::scan".into(),
            realization_identity: "package::Carrier::scan".into(),
            realization_callable_identity: Some("package::Carrier::scan#callable".into()),
        }
    }

    fn application(rows: Vec<ClosedConformanceRow>) -> ClosedConformanceApplication {
        let mut application = ClosedConformanceApplication {
            owner: semantic_vocabulary::MachineId::new(1).unwrap(),
            declaration_identity: "package::CarrierImplementsScanner".into(),
            telescope: Vec::new(),
            subject_identity: Some("package::Carrier".into()),
            trait_identity: "package::Scanner".into(),
            trait_lifetime_arguments: Vec::new(),
            trait_arguments: Vec::new(),
            realization_callables: vec![ClosedConformanceRealizationCallable {
                source_callable_identity: "package::Carrier::scan#callable".into(),
                machine: semantic_vocabulary::MachineId::new(2).unwrap(),
                result: ClosedConformanceCallableResult::I32,
            }],
            rows,
            report_fingerprint: 0,
            commitment: Default::default(),
        };
        application.report_fingerprint =
            closed_conformance_application_report_fingerprint(&application);
        application.commitment = closed_conformance_application_commitment(&application);
        application
    }

    fn encode(application: &ClosedConformanceApplication) -> Vec<u8> {
        let mut writer = Writer::default();
        encode_closed_conformance_application(&mut writer, application).unwrap();
        writer.finish()
    }

    /// One nongeneric row and two family rows of the same overload that
    /// differ only in tuple round-trip as three distinct table rows.
    #[test]
    fn closed_conformance_rows_round_trip_with_family_tuples() {
        let mut reversed = width_lanes_tuple();
        reversed.reverse();
        let application = application(vec![
            row("package::Scanner::measure()", Vec::new()),
            row("package::Scanner::scan()", width_lanes_tuple()),
            row("package::Scanner::scan()", reversed),
        ]);
        let bytes = encode(&application);
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_closed_conformance_application(&mut reader),
            Ok(application.clone())
        );
        assert_eq!(reader.remaining(), 0);
        assert_eq!(encode(&application), bytes);
    }

    /// The tuple is a row coordinate: applications equal in every identity
    /// but one row's tuple encode differently, and tuple order is retained.
    #[test]
    fn family_tuple_participates_in_closed_conformance_row_bytes() {
        let nongeneric = encode(&application(vec![row(
            "package::Scanner::scan()",
            Vec::new(),
        )]));
        let width_lanes = encode(&application(vec![row(
            "package::Scanner::scan()",
            width_lanes_tuple(),
        )]));
        let mut reversed = width_lanes_tuple();
        reversed.reverse();
        let lanes_width = encode(&application(vec![row(
            "package::Scanner::scan()",
            reversed,
        )]));
        assert_ne!(nongeneric, width_lanes);
        assert_ne!(width_lanes, lanes_width);
    }

    /// Rows encoded before the tuple coordinate existed must reject rather
    /// than decode as nongeneric rows: the decoder never supplies a missing
    /// tuple. These bytes are the previous row layout written field by field.
    #[test]
    fn closed_conformance_rows_without_family_tuple_reject() {
        let application = application(vec![row("package::Scanner::scan()", Vec::new())]);
        let mut writer = Writer::default();
        writer.id(application.owner);
        writer
            .string("legacy", &application.declaration_identity)
            .unwrap();
        writer.len("legacy", 0).unwrap();
        writer.boolean(true);
        writer
            .string("legacy", application.subject_identity.as_deref().unwrap())
            .unwrap();
        writer
            .string("legacy", &application.trait_identity)
            .unwrap();
        writer.strings("legacy", &[]).unwrap();
        writer.strings("legacy", &[]).unwrap();
        writer.len("legacy", 1).unwrap();
        let callable = &application.realization_callables[0];
        writer
            .string("legacy", &callable.source_callable_identity)
            .unwrap();
        writer.id(callable.machine);
        writer.u8(2);
        writer.len("legacy", 1).unwrap();
        let row = &application.rows[0];
        writer
            .string("legacy", &row.declaring_trait_identity)
            .unwrap();
        writer
            .string("legacy", &row.public_requirement_identity)
            .unwrap();
        writer.string("legacy", &row.requirement_identity).unwrap();
        writer.string("legacy", &row.realization_identity).unwrap();
        writer.boolean(true);
        writer
            .string(
                "legacy",
                row.realization_callable_identity.as_deref().unwrap(),
            )
            .unwrap();
        writer.u64(application.report_fingerprint);
        writer.bytes(&application.commitment.as_bytes());
        let legacy = writer.finish();

        let mut reader = Reader::new(&legacy);
        let decoded = decode_closed_conformance_application(&mut reader);
        assert!(
            decoded.is_err() || reader.remaining() != 0 || decoded != Ok(application.clone()),
            "previous-layout application decoded cleanly as a current one: {decoded:?}"
        );
        assert_ne!(encode(&application), legacy);
    }
}
