//! Closed conformance applications on the wire: the telescope, subject and
//! trait identities, realization callables, rows and commitment of one
//! application.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::decode_counted;
use terminal_psi::{TerminalIdentityIndex, ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRow,
};

pub(super) fn encode_closed_conformance_application(
    writer: &mut Writer,
    application: &ClosedConformanceApplication,
) -> Result<(), CodecError> {
    writer.id(application.owner);
    writer.u32(application.declaration_identity.0);
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
    writer.u32(application.trait_identity.0);
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
        writer.u32(callable.source_callable_identity.0);
        writer.id(callable.machine);
        writer.u8(match callable.result {
            ClosedConformanceCallableResult::Unit => 1,
            ClosedConformanceCallableResult::I32 => 2,
            ClosedConformanceCallableResult::Bool => 3,
        });
    }
    writer.len("closed conformance rows", application.rows.len())?;
    for row in &application.rows {
        writer.u32(row.declaring_trait_identity.0);
        writer.u32(row.public_requirement_identity.0);
        writer.len("closed conformance row family tuple", row.family_tuple.len())?;
        for member in &row.family_tuple {
            writer.u32(member.0);
        }
        writer.u32(row.requirement_identity.0);
        writer.u32(row.realization_identity.0);
        writer.boolean(row.realization_callable_identity.is_some());
        if let Some(identity) = &row.realization_callable_identity {
            writer.u32(identity.0);
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
        declaration_identity: TerminalIdentityIndex(reader.u32()?),
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
        trait_identity: TerminalIdentityIndex(reader.u32()?),
        trait_lifetime_arguments: reader.strings("closed conformance trait lifetime arguments")?,
        trait_arguments: reader.strings("closed conformance trait arguments")?,
        realization_callables: decode_counted(reader, |reader| {
            Ok(terminal_psi::ClosedConformanceRealizationCallable {
                source_callable_identity: TerminalIdentityIndex(reader.u32()?),
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
                declaring_trait_identity: TerminalIdentityIndex(reader.u32()?) /* closed conformance row declaring trait identity */,
                public_requirement_identity: TerminalIdentityIndex(reader.u32()?) /* closed conformance row public requirement identity */,
                family_tuple: decode_counted(reader, |reader| {
                    Ok(TerminalIdentityIndex(reader.u32()?))
                })?,
                requirement_identity: TerminalIdentityIndex(reader.u32()?) /* closed conformance row requirement identity */,
                realization_identity: TerminalIdentityIndex(reader.u32()?) /* closed conformance row realization identity */,
                realization_callable_identity: if reader.boolean()? {
                    Some(TerminalIdentityIndex(reader.u32()?))
                } else {
                    None
                },
            })
        })?,
        report_fingerprint: reader.u64()?,
        commitment: ClosedConformanceApplicationCommitment::from_digest(reader.array()?),
    })
}
