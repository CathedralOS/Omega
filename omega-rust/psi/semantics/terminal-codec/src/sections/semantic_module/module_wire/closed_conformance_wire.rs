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
